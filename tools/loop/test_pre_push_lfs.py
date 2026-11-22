#!/usr/bin/env python3
"""**A push that is not to main still uploads its Git LFS objects** (measured 2026-11-21).

`core.hooksPath=tools/loop/hooks` replaces git-lfs's own `pre-push`, and the repo's hook returned early for any push
that was not to main, before its `git lfs pre-push` step. A branch that carried a new changelog clip was then refused
by GitHub with GH008 ("2 unknown Git LFS objects") and needed a hand-run `git lfs push` first.

The check is behavioural: the real hook runs in a scratch repository against a scratch remote, with a `git-lfs` that
records how it was called, so a comment that merely names the step cannot satisfy it.

Run: `python3 -m unittest tools/loop/test_pre_push_lfs.py`.
"""

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HOOK = ROOT / "tools" / "loop" / "hooks" / "pre-push"

# What the stand-in git-lfs does: log the call and the refs it was fed, and fail on request.
STUB = """#!/usr/bin/env bash
echo "git-lfs $*" >> "$LFS_LOG"
if [ "${1:-}" = pre-push ]; then
  cat >> "$LFS_LOG"
  [ -z "${LFS_FAIL:-}" ] || exit 1
fi
"""
IDENTITY = ["-c", "user.name=test", "-c", "user.email=test@example.invalid"]


def git(cwd, *args, env=None, check=True):
    return subprocess.run(["git", *args], cwd=cwd, env=env, check=check, capture_output=True, text=True)


class TestABranchPushUploadsItsLfsObjects(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix="pre-push-lfs-"))
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        remote = self.dir / "remote.git"
        git(self.dir, "init", "-q", "--bare", str(remote))
        self.work = self.dir / "work"
        git(self.dir, "clone", "-q", str(remote), str(self.work))
        git(self.work, *IDENTITY, "commit", "-q", "--allow-empty", "-m", "base")
        git(self.work, "push", "-q", "origin", "HEAD:main")
        hooks = self.dir / "hooks"
        hooks.mkdir()
        shutil.copy(HOOK, hooks / "pre-push")
        (hooks / "pre-push").chmod(0o755)
        git(self.work, "config", "core.hooksPath", str(hooks))
        bin_dir = self.dir / "bin"
        bin_dir.mkdir()
        (bin_dir / "git-lfs").write_text(STUB)
        (bin_dir / "git-lfs").chmod(0o755)
        self.log = self.dir / "lfs.log"
        self.env = {**os.environ, "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}", "LFS_LOG": str(self.log)}
        git(self.work, "checkout", "-q", "-b", "feature")
        (self.work / "clip.txt").write_text("a changelog clip would be here\n")
        git(self.work, "add", "clip.txt")
        git(self.work, *IDENTITY, "commit", "-q", "-m", "feature")

    def push(self, extra_env=None):
        env = {**self.env, **(extra_env or {})}
        return git(self.work, "push", "origin", "feature:feature", env=env, check=False)

    def test_the_lfs_step_runs_with_the_pushed_refs(self):
        result = self.push()
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.log.read_text().splitlines()
        self.assertTrue(any(line.startswith("git-lfs pre-push") for line in calls), f"git lfs pre-push was not called: {calls}")
        sha = git(self.work, "rev-parse", "HEAD").stdout.strip()
        self.assertIn(f"refs/heads/feature {sha} refs/heads/feature", self.log.read_text(), "the hook must feed git-lfs the refs it was given")

    def test_a_failed_upload_refuses_the_push(self):
        result = self.push({"LFS_FAIL": "1"})
        self.assertNotEqual(result.returncode, 0, "a push whose LFS objects did not upload must not go through")
        self.assertIn("GH008", result.stderr)
        remote_branches = git(self.dir, "--git-dir", str(self.dir / "remote.git"), "branch", "--list", "feature").stdout
        self.assertEqual(remote_branches.strip(), "", "the branch must not have reached the remote")


if __name__ == "__main__":
    unittest.main()
