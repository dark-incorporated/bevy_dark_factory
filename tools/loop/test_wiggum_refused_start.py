#!/usr/bin/env python3
"""**A start the driver refuses does not spend a pass number** (measured 2026-11-21, gooey's loop on big).

`one_pass` wrote `pass-count` before it checked the disk. When `/home` fell under `WIGGUM_MIN_FREE_GB` the pass was
refused ("ABORTED: 61GB free < 100GB after pruning"), but its number was already spent, and the relauncher tried again
every two minutes: 133 refusals took the counter from 267 to `WIGGUM_MAX=400`, and from then on the driver exited
"reached WIGGUM_MAX" at once, for fourteen days, having done no work since the disk filled.

The check is behavioural: the real driver runs one pass in a scratch clone with a free-space floor no disk can meet,
and a stand-in `omp` that records whether it was ever launched. A positive control runs the same pass with the floor
removed, so a test that merely never reaches the guard cannot pass.

Run: `python3 -m unittest tools/loop/test_wiggum_refused_start.py`.
"""

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DRIVER = ROOT / "tools" / "loop" / "wiggum.sh"

# A floor no disk meets: the guard refuses before it launches anything.
IMPOSSIBLE_FLOOR_GB = 999_999_999

# The stand-in omp: records that it was launched, then ends the pass cleanly.
STUB = """#!/usr/bin/env bash
echo "omp $*" >> "$OMP_LOG"
exit 0
"""


class RefusedStart(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp(prefix="wiggum-refused-"))
        self.repo = self.tmp / "clone"
        self.state = self.tmp / "state"
        self.bin = self.tmp / "bin"
        self.omp_log = self.tmp / "omp.log"
        self.bin.mkdir()
        (self.bin / "omp").write_text(STUB)
        (self.bin / "omp").chmod(0o755)
        self.repo.mkdir()
        for args in (["init", "-q"], ["config", "user.email", "t@example.com"], ["config", "user.name", "t"],
                     ["commit", "-q", "--allow-empty", "-m", "root"]):
            subprocess.run(["git", "-C", str(self.repo), *args], check=True)

    def tearDown(self):
        shutil.rmtree(self.tmp, ignore_errors=True)

    def one_pass(self, floor_gb):
        env = dict(os.environ)
        env.update({
            "WIGGUM_REPO": str(self.repo),
            "WIGGUM_STATE": str(self.state),
            "WIGGUM_NO_FETCH": "1",
            "WIGGUM_MIN_FREE_GB": str(floor_gb),
            "WIGGUM_COMMAND": "build-loop",
            "WIGGUM_PASS_TIMEOUT": "60",
            "WIGGUM_CHECKPOINT": "0",
            "OMP_LOG": str(self.omp_log),
            "PATH": f"{self.bin}{os.pathsep}{env['PATH']}",
        })
        return subprocess.run(["bash", str(DRIVER), "one"], env=env, capture_output=True, text=True, timeout=120)

    def counter(self):
        path = self.state / "pass-count"
        return int(path.read_text().strip()) if path.exists() else 0

    def test_a_refused_start_spends_no_pass_number(self):
        """Ten refusals in a row leave the counter where it was, and the model is never launched."""
        for attempt in range(1, 11):
            result = self.one_pass(IMPOSSIBLE_FLOOR_GB)
            self.assertNotEqual(result.returncode, 0, f"attempt {attempt}: the pass was not refused: {result.stdout}{result.stderr}")
            self.assertIn("ABORTED", result.stderr, f"attempt {attempt}: refused, but not by the disk guard: {result.stderr}")
        self.assertEqual(self.counter(), 0, "ten refused starts spent pass numbers: WIGGUM_MAX would be reached with no work done")
        self.assertFalse(self.omp_log.exists(), "the driver launched omp past a refused start")

    def test_a_start_that_goes_ahead_does_spend_one(self):
        """The control: the same pass with the floor removed launches omp and the counter says 1."""
        self.one_pass(0)
        self.assertTrue(self.omp_log.exists(), "the control pass never launched omp, so the refusal test proves nothing")
        self.assertEqual(self.counter(), 1, "a pass that started should have spent exactly one number")

    def test_a_refusal_then_a_start_numbers_the_start_one(self):
        """A refusal is not remembered: the first pass that really starts is pass 1, not pass 2."""
        self.one_pass(IMPOSSIBLE_FLOOR_GB)
        self.one_pass(0)
        self.assertEqual(self.counter(), 1)


if __name__ == "__main__":
    unittest.main()
