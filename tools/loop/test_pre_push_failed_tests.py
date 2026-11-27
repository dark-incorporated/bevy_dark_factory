#!/usr/bin/env python3
"""A refusal keeps the failing tests by tree, and the next push of that tree runs those alone first (#580).

Seven of the 93 refused pushes in `gate.log` to 2026-11-23 failed a test that passed at the same commit on the next attempt, and
each retry re-ran all 5,159 tests to learn what the failing names would have said. The hook now writes the names beside
`gate.log` keyed by the tree and runs them alone, at full parallelism, before deciding anything.

`cargo` here is a stub that prints a canned nextest failure and records every invocation, so the checks are about the hook's
decisions and not about any real test.

    python3 tools/loop/test_pre_push_failed_tests.py
"""
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

HOOK = Path(__file__).resolve().parent / "hooks" / "pre-push"

# What nextest prints for a failing run: the FAIL line is the one the hook reads.
FAILING_RUN = """\
        FAIL [   1.840s] (3/10) bevy_dark_factory::editor a_short_row_stands_where_it_stood::a_short_row_stands_where_it_stood
        FAIL [  30.328s] (1/2) bevy_dark_factory::editor the_design_follows_the_project_index::an_area_written_outside_the_editor_reaches_the_overlay_with_the_clock_stopped
     Summary [  32.168s] 5157 tests run: 5155 passed, 2 failed, 0 skipped
"""

STUB = """\
#!/bin/sh
echo "cargo $*" >> "$CARGO_LOG"
case "$*" in
  *nextest*)
    if [ -n "$CARGO_NEXTEST_ALONE_PASSES" ]; then exit 0; fi
    printf '%s' "$CARGO_NEXTEST_FAILURE"
    exit 100 ;;
  *) exit 0 ;;
esac
"""


def git(cwd, *args, check=True):
    env = dict(os.environ, GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_SYSTEM="/dev/null", GIT_AUTHOR_NAME="t",
               GIT_AUTHOR_EMAIL="t@t", GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@t")
    r = subprocess.run(["git", *args], cwd=cwd, env=env, text=True, capture_output=True)
    if check and r.returncode != 0:
        raise AssertionError(f"git {' '.join(args)}: {r.stdout}{r.stderr}")
    return r


class FailedTestsAreKeptAndRunAlone(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix="pre-push-580-"))
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        git(self.dir, "init", "-q", "-b", "main")
        git(self.dir, "init", "-q", "--bare", str(self.dir / "remote.git"))
        (self.dir / "docs").mkdir()
        (self.dir / "docs" / "note.md").write_text("one\n")
        # The hook reads the tools it checks; a stub that always passes keeps those out of the way, and it is committed so the
        # hook's "uncommitted changes under tools/" refusal does not fire on the harness rather than on the hook.
        for tool in ("tools/loop/backlog.py", "tools/changelog/changelog.py", "tools/loop/version.py", "tools/loop/wall_clock_filter.py", "tools/loop/require-gate-record.sh"):
            path = self.dir / tool
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("import sys\nsys.exit(0)\n")
        (self.dir / "crates").mkdir()
        (self.dir / "crates" / "keep.rs").write_text("")
        # A real remote and local LFS, because the hook uploads the pushed refs' LFS objects before it gates anything.
        git(self.dir, "remote", "add", "origin", str(self.dir / "remote.git"))
        git(self.dir, "add", ".")
        git(self.dir, "commit", "-qm", "first")
        git(self.dir, "update-ref", "refs/remotes/origin/main", "HEAD")
        (self.dir / "docs" / "note.md").write_text("two\n")
        git(self.dir, "commit", "-qam", "second")
        self.sha = git(self.dir, "rev-parse", "HEAD").stdout.strip()
        self.tree = git(self.dir, "rev-parse", "HEAD^{tree}").stdout.strip()
        self.bin = self.dir / "bin"
        self.bin.mkdir()
        stub = self.bin / "cargo"
        stub.write_text(STUB)
        stub.chmod(0o755)
        self.cargo_log = self.dir / "cargo.log"

    def push(self, alone_passes=False):
        zero = "0" * 40
        env = dict(os.environ, PATH=f"{self.bin}:{os.environ['PATH']}", CARGO_LOG=str(self.cargo_log),
                   CARGO_NEXTEST_FAILURE=FAILING_RUN,
                   CARGO_NEXTEST_ALONE_PASSES="yes" if alone_passes else "",
                   GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_SYSTEM="/dev/null", LOOP_GATE_TEST="nextest")
        return subprocess.run([str(HOOK), "origin", "file:///dev/null"], cwd=self.dir, env=env, text=True,
                              capture_output=True,
                              input=f"refs/heads/main {self.sha} refs/heads/main {zero}\n")

    def kept(self):
        return self.dir / ".git" / "gate-failed" / self.tree

    def calls(self):
        return self.cargo_log.read_text().splitlines() if self.cargo_log.exists() else []

    def test_a_refusal_keeps_the_failing_tests_by_tree(self):
        r = self.push()
        self.assertNotEqual(r.returncode, 0, "the failing suite refuses the push")
        self.assertTrue(self.kept().exists(), "the names are kept beside gate.log, keyed by the tree")
        kept = self.kept().read_text().splitlines()
        self.assertEqual(kept, [
            "bevy_dark_factory::editor a_short_row_stands_where_it_stood::a_short_row_stands_where_it_stood",
            "bevy_dark_factory::editor the_design_follows_the_project_index::an_area_written_outside_the_editor_reaches_the_overlay_with_the_clock_stopped",
        ], "one nextest id a line, in the order nextest printed them")

    def test_a_second_push_of_the_same_tree_runs_those_tests_alone_first(self):
        self.push()
        self.cargo_log.write_text("")
        r = self.push(alone_passes=True)
        self.assertEqual(r.returncode, 0, f"the named tests pass alone: {r.stderr}")
        calls = self.calls()
        # `--test-threads`, the flag nextest has (#580, `933bb44b`); `--threads-required` is a config key, not a flag.
        self.assertTrue(any("--test-threads num-cpus" in c for c in calls), f"alone, at full parallelism: {calls}")
        # The recorded id's two halves, each in the predicate that reads it (#580, `a0782796`).
        self.assertTrue(any("binary_id(bevy_dark_factory::editor) & test(=a_short_row_stands_where_it_stood::a_short_row_stands_where_it_stood)" in c
                            for c in calls), f"the failing test is named in the filterset: {calls}")
        self.assertFalse(any("--no-fail-fast" in c and "test(" not in c for c in calls),
                         f"and the whole suite was not run again: {calls}")

    def test_a_test_that_fails_alone_twice_still_refuses(self):
        self.push()
        self.cargo_log.write_text("")
        second = self.push(alone_passes=False)
        self.assertNotEqual(second.returncode, 0, "failing alone is a real failure")
        self.assertIn("failed alone at this tree", second.stderr)
        self.cargo_log.write_text("")
        third = self.push(alone_passes=False)
        self.assertNotEqual(third.returncode, 0, "and a third attempt is refused too")
        self.assertIn("twice", third.stderr, "the third attempt says it has now been alone twice")


if __name__ == "__main__":
    unittest.main()
