#!/usr/bin/env python3
"""**The gate-target prune keeps the newest executable of each test and nothing else changes** (2026-11-21).

The check is behavioural: the real script runs over a scratch target holding several generations of three tests, an rlib, a `.d`
file, a build script and a small executable, and the result is read back from the directory, not from the script's own report.

Run: `python3 -m unittest tools/loop/test_prune_gate_target.py`. Linux only (`find -printf`), like the gate it serves.
"""

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "loop" / "prune_gate_target.sh"
MIN_MB = 1  # the script's floor, lowered so the fixture files stay small
BIG = MIN_MB * 1024 * 1024 + 1


def hexhash(n):
    return f"{n:016x}"


@unittest.skipUnless(sys.platform.startswith("linux"), "the script uses GNU find -printf, as the gate's host does")
class PruneGateTarget(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="prune-gate-"))
        self.target = self.tmp / "target"
        self.deps = self.target / "debug" / "deps"
        self.deps.mkdir(parents=True)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def make(self, name, age_s, size=BIG):
        path = self.deps / name
        path.write_bytes(b"\0" * size)
        stamp = 1_800_000_000 - age_s
        os.utime(path, (stamp, stamp))
        return path

    def run_script(self, *args):
        # GATE_UNIT names a unit that does not exist, so the host's real gate (which makes the script refuse) cannot decide the result.
        env = dict(os.environ, GATE_TARGET=str(self.target), MIN_MB=str(MIN_MB), GATE_UNIT="prune-test-no-such-unit")
        return subprocess.run(["bash", str(SCRIPT), *args], env=env, capture_output=True, text=True, timeout=60)

    def test_only_older_generations_of_a_test_are_removed(self):
        newest = {
            "a_handback_becomes_a_task_draft": self.make(f"a_handback_becomes_a_task_draft-{hexhash(3)}", 10),
            "every_task_is_valid": self.make(f"every_task_is_valid-{hexhash(9)}", 5),
            "the_only_one": self.make(f"the_only_one-{hexhash(1)}", 99_999),
        }
        stale = [
            self.make(f"a_handback_becomes_a_task_draft-{hexhash(1)}", 90_000),
            self.make(f"a_handback_becomes_a_task_draft-{hexhash(2)}", 50_000),
            self.make(f"every_task_is_valid-{hexhash(8)}", 70_000),
        ]
        untouched = [
            self.make(f"libserde-{hexhash(4)}.rlib", 999_999),  # an rlib: has an extension
            self.make(f"serde-{hexhash(4)}.d", 999_999),  # dep-info
            self.make(f"build_script_build-{hexhash(5)}", 999_999, size=10),  # extensionless but small
        ]
        result = self.run_script("--apply")
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        for name, path in newest.items():
            self.assertTrue(path.exists(), f"the newest generation of {name} was deleted")
        for path in stale:
            self.assertFalse(path.exists(), f"{path.name} is an older generation and should be gone")
        for path in untouched:
            self.assertTrue(path.exists(), f"{path.name} is not a test executable and must not be touched")

    def test_a_dry_run_deletes_nothing_and_says_what_it_would(self):
        old = self.make(f"every_task_is_valid-{hexhash(1)}", 90_000)
        self.make(f"every_task_is_valid-{hexhash(2)}", 10)
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertTrue(old.exists(), "a dry run deleted a file")
        self.assertIn("dry run", result.stdout)

    def test_a_process_using_the_target_makes_it_refuse(self):
        """A test executable running out of the target is started by its path, so the path on a command line is the signal."""
        old = self.make(f"every_task_is_valid-{hexhash(1)}", 90_000)
        self.make(f"every_task_is_valid-{hexhash(2)}", 10)
        # `sleep 30; true` is not one simple command, so bash does not exec `sleep` and drop its own arguments: the path stays on the command line.
        busy = subprocess.Popen(["bash", "-c", "sleep 30; true", "_", str(self.deps / "every_task_is_valid-running")])
        try:
            result = self.run_script("--apply")
        finally:
            busy.kill()
            busy.wait()
        self.assertEqual(result.returncode, 3, result.stdout + result.stderr)
        self.assertTrue(old.exists(), "it deleted while a process was using the target")

    def test_a_missing_target_is_refused_not_invented(self):
        shutil.rmtree(self.target)
        result = self.run_script("--apply")
        self.assertEqual(result.returncode, 2, result.stdout + result.stderr)

    def test_an_unknown_argument_is_refused(self):
        result = self.run_script("--everything")
        self.assertEqual(result.returncode, 64, result.stdout + result.stderr)

    def test_a_target_that_is_still_over_its_ceiling_after_pruning_says_so(self):
        """#578: the timer's failure has to be a line in the journal, so a target the prune cannot get under its ceiling
        exits 4 rather than reporting a successful prune."""
        self.make(f"every_task_is_valid-{hexhash(1)}", 90_000)
        self.make(f"every_task_is_valid-{hexhash(2)}", 10)
        result = subprocess.run(
            ["bash", str(SCRIPT), "--apply"],
            env=dict(os.environ, GATE_TARGET=str(self.target), MIN_MB=str(MIN_MB),
                     GATE_UNIT="prune-test-no-such-unit", CEILING_GB="0"),
            capture_output=True, text=True, timeout=60,
        )
        self.assertEqual(result.returncode, 4, result.stdout + result.stderr)
        self.assertIn("OVER CEILING", result.stderr)

    def test_a_target_under_its_ceiling_prunes_quietly(self):
        """The ceiling only speaks when it is broken; a normal prune is exit 0."""
        self.make(f"every_task_is_valid-{hexhash(1)}", 90_000)
        self.make(f"every_task_is_valid-{hexhash(2)}", 10)
        result = subprocess.run(
            ["bash", str(SCRIPT), "--apply"],
            env=dict(os.environ, GATE_TARGET=str(self.target), MIN_MB=str(MIN_MB),
                     GATE_UNIT="prune-test-no-such-unit", CEILING_GB="120"),
            capture_output=True, text=True, timeout=60,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("ceiling 120 GB", result.stdout)


if __name__ == "__main__":
    unittest.main()
