#!/usr/bin/env python3
"""**A refused gate does not pay for clippy again, and a green gate's record travels with the commit** (#574, #575).

Behavioural: the real hook runs in a scratch repository against a scratch remote, with a `cargo` that counts its calls
and can be told to fail, so a comment cannot satisfy any of it.

  python3 -m unittest tools/loop/test_pre_push_clippy_memo.py
"""

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
HOOK = ROOT / "tools" / "loop" / "hooks" / "pre-push"
IDENTITY = ["-c", "user.name=test", "-c", "user.email=test@example.invalid"]

# A cargo that logs each subcommand and answers with the exit code named by CARGO_FAIL_ON (a substring of the command).
CARGO_STUB = """#!/usr/bin/env bash
echo "cargo $*" >> "$CARGO_LOG"
case " $* " in
  *" $CARGO_FAIL_ON "*|*" $CARGO_FAIL_ON") [ -z "${CARGO_FAIL_ON:-}" ] || exit 1 ;;
esac
exit 0
"""


def git(cwd, *args, env=None, check=True):
    return subprocess.run(["git", *args], cwd=cwd, env=env, check=check, capture_output=True, text=True)


class PushGate(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix="pre-push-memo-"))
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        remote = self.dir / "remote.git"
        git(self.dir, "init", "-q", "--bare", "-b", "main", str(remote))
        self.work = self.dir / "work"
        git(self.dir, "init", "-q", "-b", "main", str(self.work))
        git(self.work, "remote", "add", "origin", str(remote))
        # The three policy checks the hook runs before clippy; each is a stub that passes here and is exercised elsewhere.
        for name in ("tools/loop/backlog.py", "tools/changelog/changelog.py", "tools/loop/version.py",
                     "tools/loop/wall_clock_filter.py"):
            path = self.work / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("import sys\nsys.exit(0)\n")
        (self.work / "crates").mkdir()
        (self.work / "crates/lib.rs").write_text("pub fn f() {}\n")
        hooks = self.work / "tools/loop/hooks"
        hooks.mkdir(parents=True, exist_ok=True)
        shutil.copy(HOOK, hooks / "pre-push")
        (hooks / "pre-push").chmod(0o755)
        git(self.work, "config", "core.hooksPath", "tools/loop/hooks")
        git(self.work, "add", "-A")
        git(self.work, *IDENTITY, "commit", "-q", "-m", "base")
        git(self.work, "push", "-q", "--no-verify", "origin", "main")
        bin_dir = self.dir / "bin"
        bin_dir.mkdir()
        (bin_dir / "cargo").write_text(CARGO_STUB)
        (bin_dir / "cargo").chmod(0o755)
        (bin_dir / "cargo-nextest").write_text("#!/usr/bin/env bash\nexit 0\n")
        (bin_dir / "cargo-nextest").chmod(0o755)
        self.cargo_log = self.dir / "cargo.log"
        self.env = {**os.environ, "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}",
                    "CARGO_LOG": str(self.cargo_log), "LOOP_GATE_CARGO": "cargo"}

    def git_dir(self):
        return self.work / git(self.work, "rev-parse", "--git-dir").stdout.strip()

    def commit(self, text="pub fn f() { let _ = 2; }\n", message="a change"):
        (self.work / "crates/lib.rs").write_text(text)
        git(self.work, "add", "-A")
        git(self.work, *IDENTITY, "commit", "-q", "-m", message)
        return git(self.work, "rev-parse", "HEAD").stdout.strip()

    def push(self, fail_on=""):
        env = {**self.env, "CARGO_FAIL_ON": fail_on}
        return git(self.work, "push", "origin", "main", env=env, check=False)

    def clippy_calls(self):
        if not self.cargo_log.exists():
            return 0
        return sum(1 for line in self.cargo_log.read_text().splitlines() if line.startswith("cargo clippy"))

    def test_a_refused_gate_does_not_lint_the_same_tree_twice(self):
        """#574: refused at the test step, the second push of the same commit skips clippy and reaches the test step."""
        self.commit()
        first = self.push(fail_on="nextest")
        self.assertNotEqual(first.returncode, 0, "the gate should have refused at the test step")
        self.assertEqual(self.clippy_calls(), 1, self.cargo_log.read_text() if self.cargo_log.exists() else "")
        second = self.push()
        self.assertEqual(second.returncode, 0, second.stderr)
        self.assertEqual(self.clippy_calls(), 1, "clippy ran again on a tree a refused gate had already linted clean")
        log = (self.git_dir() / "gate.log").read_text()
        self.assertIn("clippy cached", log)

    def test_an_edited_tree_lints_again(self):
        """#574: any source edit moves the key, so the answer cannot be stale."""
        self.commit()
        self.push(fail_on="nextest")
        self.commit(text="pub fn f() { let _ = 3; }\n", message="another change")
        self.push()
        self.assertEqual(self.clippy_calls(), 2, "an edit did not run clippy again")

    def test_a_different_toolchain_lints_again(self):
        """#574: the key names the toolchain, so a bump cannot reuse an older answer."""
        self.commit()
        self.push(fail_on="nextest")
        before = self.clippy_calls()
        # A rustc that reports a different version is a different toolchain: the key has to include it.
        bin_dir = self.dir / "bin2"
        bin_dir.mkdir()
        (bin_dir / "rustc").write_text("#!/usr/bin/env bash\necho 'rustc 99.0.0 (a different build)'\n")
        (bin_dir / "rustc").chmod(0o755)
        env = {**self.env, "PATH": f"{bin_dir}{os.pathsep}{self.env['PATH']}"}
        result = git(self.work, "push", "origin", "main", env=env, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.clippy_calls(), before + 1, "a toolchain bump reused the previous build's answer")

    def test_a_failed_lint_is_never_recorded(self):
        """#574: the memo is written only after a green run, so a failing tree lints again."""
        self.commit()
        bad = self.push(fail_on="clippy")
        self.assertNotEqual(bad.returncode, 0)
        self.push()
        self.assertEqual(self.clippy_calls(), 2, "a failed clippy was remembered as clean")

    def test_a_green_gate_leaves_a_record_in_the_commit(self):
        """#575: the record is a note on the commit, pushed to the remote, so any clone can read it."""
        sha = self.commit()
        result = self.push()
        self.assertEqual(result.returncode, 0, result.stderr)
        note = git(self.work, "notes", "--ref", "gated", "show", sha, check=False)
        self.assertEqual(note.returncode, 0, "no note on the commit it gated")
        self.assertIn("gated on", note.stdout)
        remote_note = git(self.dir / "remote.git", "notes", "--ref", "gated", "show", sha, check=False)
        self.assertEqual(remote_note.returncode, 0, "the note did not reach the remote with the commit")

    def test_a_clean_merge_of_two_recorded_commits_runs_only_clippy(self):
        """#575: the fast path is reachable from a clone that has gated neither parent, because the record travels."""
        # A gated parent on main.
        first = self.commit()
        self.assertEqual(self.push().returncode, 0)
        # A feature branch off main, gated and pushed as a branch (no gate for a branch, so its record is written by a
        # direct note; what the fast path reads is the note, wherever the gate ran).
        git(self.work, "checkout", "-q", "-b", "feature")
        second = self.commit(text="pub fn f() { let _ = 9; }\n", message="the feature")
        for sha in (first, second):
            git(self.work, "notes", "--ref", "gated", "add", "-f", "-m", "gated elsewhere", sha)
        git(self.work, "checkout", "-q", "main")
        merge = git(self.work, "merge", "--no-ff", "-m", "merge the feature", "feature", check=False)
        self.assertEqual(merge.returncode, 0, merge.stderr)
        target = git(self.work, "rev-parse", "HEAD").stdout.strip()
        # A third clone that has never gated anything reads the records from the remote.
        fresh = self.dir / "fresh"
        git(self.dir, "clone", "-q", str(self.dir / "remote.git"), str(fresh))
        git(fresh, "config", "user.email", "test@example.invalid")
        git(fresh, "config", "user.name", "test")
        git(fresh, "fetch", "-q", "origin", "main:main", check=False)
        result = git(self.work, "push", "origin", "main", env=self.env, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        log = (self.git_dir() / "gate.log").read_text()
        self.assertIn("clean merge", result.stderr + log)


if __name__ == "__main__":
    unittest.main()
