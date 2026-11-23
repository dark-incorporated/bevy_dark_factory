#!/usr/bin/env python3
"""**Every clone that can push `main` is checked for the gate hook, and a wrong one is named** (#579).

`check-gate-hook.sh` is the command the runbook points at and the `--fix` a setup step runs. These tests prove its
reading and its refusal against scratch clones, so the list of hosts cannot silently become decoration.

    python3 -m unittest tools/loop/test_check_gate_hook.py
"""

import subprocess
import tempfile
import unittest
from pathlib import Path
import shutil
import os

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "tools" / "loop" / "check-gate-hook.sh"


class CheckGateHook(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix="gate-hook-"))
        self.addCleanup(shutil.rmtree, self.dir, ignore_errors=True)
        self.repo = self.dir / "repo"
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True, capture_output=True)

    def run_check(self, *args):
        """The real script, with `GATE_HOOK_CLONES` naming this one clone so no other machine is contacted."""
        env = dict(os.environ, GATE_HOOK_CLONES=f"big {self.repo}")
        return subprocess.run(["bash", str(SCRIPT), *args], env=env, capture_output=True, text=True, timeout=60)

    def test_a_clone_with_the_hook_passes(self):
        subprocess.run(["git", "-C", str(self.repo), "config", "--local", "core.hooksPath", "tools/loop/hooks"], check=True)
        r = self.run_check()
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("ok", r.stdout)

    def test_an_unset_hook_path_is_refused_and_named(self):
        r = self.run_check()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("WRONG", r.stderr)
        self.assertIn("<unset>", r.stderr)

    def test_a_devnull_hook_path_is_refused(self):
        subprocess.run(["git", "-C", str(self.repo), "config", "--local", "core.hooksPath", "/dev/null"], check=True)
        r = self.run_check()
        self.assertEqual(r.returncode, 1, r.stdout + r.stderr)
        self.assertIn("/dev/null", r.stderr)

    def test_fix_sets_it_and_the_clone_passes(self):
        r = self.run_check("--fix")
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        got = subprocess.run(["git", "-C", str(self.repo), "config", "--get", "core.hooksPath"],
                             capture_output=True, text=True).stdout.strip()
        self.assertEqual(got, "tools/loop/hooks")


if __name__ == "__main__":
    unittest.main()
