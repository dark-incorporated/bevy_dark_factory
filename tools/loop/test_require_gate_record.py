#!/usr/bin/env python3
"""**A release tag whose commit carries no gate record is refused** (#579).

`require-gate-record.sh` runs in the release workflow, where a tag becomes a DMG. 7 of the last 9 release commits
reached `main` with no record because the pre-push hook is not installed everywhere; this is the reading side.

Behavioural: a scratch repository and scratch notes, so the script's answer is read from git rather than from its text.

    python3 -m unittest tools/loop/test_require_gate_record.py
"""

import subprocess
import tempfile
import unittest
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "loop" / "require-gate-record.sh"
IDENTITY = ["-c", "user.name=test", "-c", "user.email=test@example.invalid"]


def git(cwd, *args, check=True):
    return subprocess.run(["git", *args], cwd=cwd, check=check, capture_output=True, text=True)


class RequireGateRecord(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix="gate-record-"))
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        self.work = self.dir / "work"
        git(self.dir, "init", "-q", "-b", "main", str(self.work))
        (self.work / "f").write_text("x\n")
        git(self.work, "add", "-A")
        git(self.work, *IDENTITY, "commit", "-q", "-m", "a release")
        self.sha = git(self.work, "rev-parse", "HEAD").stdout.strip()

    def run_script(self, *args):
        return subprocess.run(["bash", str(SCRIPT), *args], cwd=self.work, capture_output=True, text=True, timeout=60)

    def test_a_commit_with_no_record_is_refused(self):
        r = self.run_script(self.sha)
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("REFUSED", r.stderr)
        self.assertIn(self.sha, r.stderr)

    def test_a_commit_with_a_record_passes(self):
        git(self.work, "notes", "--ref", "gated", "add", "-m", "gated on big", self.sha)
        r = self.run_script(self.sha)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("carries its gate record", r.stdout)

    def test_an_owner_exception_is_recorded_and_passes(self):
        r = self.run_script("--allow", "a hotfix the owner took by hand", self.sha)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("OWNER EXCEPTION", r.stdout)

    def bump(self):
        """A version bump on top of `self.sha`: the manifest line changed and nothing else."""
        (self.work / "Cargo.toml").write_text('version = "1.2.4"\n')
        git(self.work, "add", "-A")
        git(self.work, *IDENTITY, "commit", "-q", "-m", "release: 1.2.4")
        return git(self.work, "rev-parse", "HEAD").stdout.strip()

    def test_a_version_bump_whose_parent_is_gated_passes(self):
        # The first tag after this script landed refused 0.8.2 for exactly this: a bump runs no tests by design, so it has
        # no record of its own, and its diff is version lines only.
        git(self.work, "notes", "--ref", "gated", "add", "-m", "gated on big", self.sha)
        bump = self.bump()
        r = self.run_script(bump)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("is a version bump whose parent", r.stdout)

    def test_a_bump_whose_parent_is_not_gated_is_refused(self):
        bump = self.bump()
        r = self.run_script(bump)
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("REFUSED", r.stderr)

    def test_a_commit_that_only_looks_like_a_bump_is_refused(self):
        # A source line beside the version line makes it not a bump: it must carry its own gate.
        git(self.work, "notes", "--ref", "gated", "add", "-m", "gated on big", self.sha)
        (self.work / "Cargo.toml").write_text('version = "1.2.4"\n')
        (self.work / "src.rs").write_text("fn main() {}\n")
        git(self.work, "add", "-A")
        git(self.work, *IDENTITY, "commit", "-q", "-m", "release: 1.2.4 with a fix")
        fake = git(self.work, "rev-parse", "HEAD").stdout.strip()
        r = self.run_script(fake)
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("REFUSED", r.stderr)

    def test_an_unknown_commit_is_refused_rather_than_passing(self):
        r = self.run_script("0" * 40)
        self.assertEqual(r.returncode, 2, r.stdout + r.stderr)

    def test_a_missing_argument_is_a_usage_error(self):
        r = self.run_script()
        self.assertEqual(r.returncode, 64, r.stdout + r.stderr)


if __name__ == "__main__":
    unittest.main()
