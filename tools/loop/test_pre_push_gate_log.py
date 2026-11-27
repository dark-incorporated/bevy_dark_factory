#!/usr/bin/env python3
"""**`gate.log` says what a gate's seconds were spent on** (#625, #626, #633, #636).

Behavioural: the real hook runs in a scratch repository against a scratch remote, with a `cargo` that prints what cargo
and nextest print around a test run, so a comment cannot satisfy any of it.

- the test step's line is followed by `test-split compile=<s> run=<s>`, read from cargo's `Finished` line and nextest's
  `Summary` line (#625);
- every gate that compiles writes one `env` line with the settings its step times depend on (#636, #624), and that line
  shows the hook's own target dir when the caller's shell exported the shared one (#633);
- a merge that pays the full gate says which clean-merge condition it missed, and still runs the whole suite (#626).

  python3 -m unittest tools/loop/test_pre_push_gate_log.py
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

# A cargo that logs each call and, for the test run, prints cargo's and nextest's own closing lines as they appear in a
# log big kept (`gate-tests/b6e730609.log`, 2026-11-27): 7m 08s of build and 364.487 s of run.
CARGO_STUB = """#!/usr/bin/env bash
echo "cargo $*" >> "$CARGO_LOG"
case "$*" in
  "nextest run"*)
    echo '   Compiling bevy_dark_factory v0.8.5 (/w/crates/bevy_dark_factory)'
    echo '    Finished `test` profile [optimized + debuginfo] target(s) in 7m 08s'
    echo '    Starting 2 tests across 1 binaries'
    echo '     Summary [ 364.487s] 2 tests run: 2 passed, 0 skipped' ;;
esac
exit 0
"""


def git(cwd, *args, env=None, check=True):
    return subprocess.run(["git", *args], cwd=cwd, env=env, check=check, capture_output=True, text=True)


class GateLog(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix="pre-push-log-"))
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        remote = self.dir / "remote.git"
        git(self.dir, "init", "-q", "--bare", "-b", "main", str(remote))
        self.work = self.dir / "work"
        git(self.dir, "init", "-q", "-b", "main", str(self.work))
        git(self.work, "remote", "add", "origin", str(remote))
        # The policy checks the hook runs before clippy; each is a stub that passes here and is exercised elsewhere.
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
        self.target_dir = self.dir / "own-target"
        env = {k: v for k, v in os.environ.items() if k not in ("NEXTEST_TEST_THREADS", "CARGO_BUILD_JOBS")}
        self.env = {**env, "PATH": f"{bin_dir}{os.pathsep}{os.environ['PATH']}", "CARGO_LOG": str(self.cargo_log),
                    "LOOP_GATE_CARGO": "cargo", "CARGO_BUILD_JOBS": "3", "CARGO_TARGET_DIR": str(self.target_dir)}

    def commit(self, text="pub fn f() { let _ = 2; }\n", message="a change"):
        (self.work / "crates/lib.rs").write_text(text)
        git(self.work, "add", "-A")
        git(self.work, *IDENTITY, "commit", "-q", "-m", message)
        return git(self.work, "rev-parse", "HEAD").stdout.strip()

    def push(self, **env):
        return git(self.work, "push", "origin", "main", env={**self.env, **env}, check=False)

    def log_lines(self):
        common = git(self.work, "rev-parse", "--git-common-dir").stdout.strip()
        return (self.work / common / "gate.log").read_text().splitlines()

    def lines(self, step):
        return [line for line in self.log_lines() if line.split()[2:3] == [step]]

    def test_the_test_step_is_split_into_compile_and_run(self):
        """#625: cargo's `Finished ... in 7m 08s` and nextest's `Summary [ 364.487s]` become one line beside the step."""
        sha = self.commit()
        result = self.push()
        self.assertEqual(result.returncode, 0, result.stderr)
        split = self.lines("test-split")
        self.assertEqual(len(split), 1, f"one split line for one test step: {self.log_lines()}")
        self.assertEqual(split[0].split()[1], sha[:9], "the line names the commit like every other gate.log line")
        self.assertIn("compile=428s run=364s", split[0])
        self.assertRegex(split[0], r" load=[0-9.]+$", "and the load the run ended under")
        self.assertEqual(len(self.lines("test")), 1, "the step's own total line is unchanged")
        self.assertEqual(self.lines("clean-merge"), [], "a commit that is not a merge has no merge line")

    def test_every_gate_that_compiles_says_what_it_ran_under(self):
        """#636, #624: the jobs, the test threads, the target dir, the host's cores and its load, once per gate."""
        self.commit()
        self.assertEqual(self.push(NEXTEST_TEST_THREADS="4").returncode, 0)
        env = self.lines("env")
        self.assertEqual(len(env), 1, f"one env line per gate: {self.log_lines()}")
        for field in ("CARGO_BUILD_JOBS=3", "NEXTEST_TEST_THREADS=4", f"CARGO_TARGET_DIR={self.target_dir}"):
            self.assertIn(field, env[0])
        self.assertRegex(env[0], r" host=\S+ cores=[0-9]+ load=[0-9.]+ ")

    def test_a_push_from_a_shell_that_exports_the_shared_dir_gates_in_its_own(self):
        """#633: the shared dir rust-analyzer uses is replaced by the gate's own; any other dir is kept as given."""
        shared = self.dir / ".cargo-target-shared" / "global"
        self.commit()
        self.assertEqual(self.push(CARGO_TARGET_DIR=str(shared)).returncode, 0)
        env = self.lines("env")[-1]
        self.assertIn(f"CARGO_TARGET_DIR={self.dir / '.cargo-target-shared-gate'} ", env + " ")
        self.assertNotIn(str(shared), env)

    def test_a_merge_the_fast_path_cannot_take_says_which_condition_it_missed(self):
        """#626: main's side carries a record and is on main; the branch's side has neither, so the full suite runs."""
        self.commit()
        self.assertEqual(self.push().returncode, 0, "the main side is gated through the hook")
        git(self.work, "checkout", "-q", "-b", "feature", "HEAD~1")
        # Another file than main's change touched, so the merge is exactly `git merge-tree`'s.
        (self.work / "crates/feature.rs").write_text("pub fn g() {}\n")
        git(self.work, "add", "-A")
        git(self.work, *IDENTITY, "commit", "-q", "-m", "the feature, pushed as a branch, so never gated")
        branch = git(self.work, "rev-parse", "HEAD").stdout.strip()
        git(self.work, "checkout", "-q", "main")
        merge = git(self.work, *IDENTITY, "merge", "--no-ff", "-q", "-m", "merge the feature", "feature", check=False)
        self.assertEqual(merge.returncode, 0, merge.stderr)
        self.cargo_log.write_text("")
        result = self.push()
        self.assertEqual(result.returncode, 0, result.stderr)
        miss = [line for line in self.log_lines() if " clean-merge path not taken: " in line]
        self.assertEqual(len(miss), 1, f"one reason for one merge: {self.log_lines()}")
        self.assertIn("no parent has a gate record while the other is on main", miss[0])
        self.assertIn(f"{branch[:9]} record=no on-main=no", miss[0])
        self.assertIn("record=yes on-main=yes", miss[0], "the main side is named with its record")
        self.assertIn("not one the clean-merge path takes", result.stderr, "and the pusher is told too")
        calls = self.cargo_log.read_text()
        self.assertIn("cargo nextest run --workspace", calls, "the merge still paid the whole suite")


if __name__ == "__main__":
    unittest.main()
