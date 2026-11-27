"""classify_run.py on the five shapes a CI job ends in: a test failure, a kill, a runner that never started,
the `tee` shape #572 closed, and a cancelled run. Each fixture is the step list GitHub reports for that shape,
so the classifier is held to the runs #674's census had to sort by hand.
"""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import classify_run  # noqa: E402


def step(number, name, started=None, concluded=None, conclusion="success"):
    return {
        "number": number,
        "name": name,
        "started_at": started,
        "completed_at": concluded,
        "conclusion": conclusion,
    }


# **The classifier's own job reads itself** (#674). Run 46023866009 taught this the hard way: the API returns the
# `classify` job too, and while it is reading, that job's current step has a start and no conclusion -- so the
# first version called a run "stopped by the host" over its own healthy test job. Every fixture below is therefore
# a *whole run*: the job under test plus the classifier's job, still in progress.
SELF = {
    "id": 140618983967,
    "name": "what stopped the run",
    "status": "in_progress",
    "conclusion": None,
    "steps": [
        step(1, "Set up job", "2026-11-27T16:17:41Z", "2026-11-27T16:17:42Z"),
        step(3, "Run actions/checkout@v4", "2026-11-27T16:17:42Z", None, None),
    ],
}


# 45263369724: the step that died has a start and no conclusion, and the steps after it never started.
KILLED_MID_STEP = {
    "id": 138003727733,
    "name": "test (workspace)",
    "status": "completed",
    "conclusion": "failure",
    "steps": [
        step(1, "Set up job", "2026-11-23T20:49:45Z", "2026-11-23T20:49:48Z"),
        step(6, "Name the build inputs", "2026-11-23T20:50:07Z", "2026-11-23T20:50:07Z"),
        step(7, "workspace tests (nextest)", "2026-11-23T20:50:07Z", None, None),
        step(8, "doctests (nextest does not run them)", None, None, None),
        step(10, "Run actions/upload-artifact@v4", None, None, None),
    ],
}

# 44363791421 and its neighbours: the runner was gone before the job began, so there is not even a step.
NEVER_STARTED = {"name": "test (workspace)", "conclusion": "failure", "steps": []}

# 45202786364: the one red in the census that was the code's.
TEST_FAILED = {
    "name": "test (workspace)",
    "status": "completed",
    "conclusion": "failure",
    "steps": [
        step(1, "Set up job", "2026-11-23T15:13:26Z", "2026-11-23T15:13:29Z"),
        step(7, "workspace tests (nextest)", "2026-11-23T15:13:48Z", "2026-11-23T16:02:48Z", "failure"),
        step(9, "Report the cargo-slot wait", "2026-11-23T16:02:48Z", "2026-11-23T16:02:49Z", "skipped"),
    ],
}

# The shape #572 closed: every step green, the job red anyway because `| tee` ate cargo's status.
ALL_STEPS_GREEN = {
    "name": "test (workspace)",
    "status": "completed",
    "conclusion": "failure",
    "steps": [
        step(1, "Set up job", "2026-11-20T07:22:49Z", "2026-11-20T07:22:52Z"),
        step(7, "workspace tests (nextest)", "2026-11-20T07:23:13Z", "2026-11-20T07:28:13Z"),
        step(8, "doctests (nextest does not run them)", "2026-11-20T07:28:13Z", "2026-11-20T07:32:42Z"),
    ],
}

CANCELLED = {
    "name": "test (workspace)",
    "status": "completed",
    "conclusion": "cancelled",
    "steps": [
        step(1, "Set up job", "2026-11-24T10:00:00Z", "2026-11-24T10:00:03Z"),
        step(7, "workspace tests (nextest)", "2026-11-24T10:00:20Z", "2026-11-24T10:04:00Z", "cancelled"),
    ],
}


class Classify(unittest.TestCase):
    def run_on(self, job, logs=None):
        with tempfile.TemporaryDirectory() as tmp:
            payload = Path(tmp) / "jobs.json"
            payload.write_text(json.dumps({"jobs": [job, SELF]}))
            args = [sys.executable, str(HERE / "classify_run.py"), str(payload)]
            if logs:
                logdir = Path(tmp) / "logs"
                logdir.mkdir()
                for job_id, text in logs.items():
                    (logdir / f"{job_id}.log").write_text(text)
                args.append(str(logdir))
            done = subprocess.run(args, text=True, capture_output=True)
            self.assertEqual(done.stderr, "")
            return done.returncode, done.stdout.strip()

    def test_a_step_with_no_conclusion_is_the_host_stopping_the_run(self):
        rc, out = self.run_on(KILLED_MID_STEP)
        self.assertEqual(rc, 1, out)
        self.assertTrue(out.startswith("host-stopped:"), out)
        self.assertIn("workspace tests (nextest)", out)
        self.assertIn("no conclusion", out)

    def test_a_job_with_no_steps_never_started(self):
        rc, out = self.run_on(NEVER_STARTED)
        self.assertEqual(rc, 1, out)
        self.assertIn("never started it", out)

    def test_a_failed_step_is_the_tests(self):
        rc, out = self.run_on(TEST_FAILED)
        self.assertEqual(rc, 0, out)
        self.assertTrue(out.startswith("test-failed:"), out)

    def test_a_sigkill_in_a_step_log_names_the_host(self):
        # cargo says `signal: 9 (SIGKILL)` when the kernel reaped the rustc it was waiting on. That line is
        # worth more than the structural guess: it says the memory, not the clock.
        logs = {
            138003727733: "   Compiling bevy_dark_factory v0.8.2\n"
            "error: could not compile `bevy_dark_factory` (lib)\n"
            "Caused by:\n"
            "  process didn't exit successfully: `rustc ...` (signal: 9, SIGKILL: kill)\n"
        }
        job = {
            "id": 138003727733,
            "name": "test (workspace)",
            "conclusion": "failure",
            "steps": [step(7, "workspace tests (nextest)", "2026-11-23T20:50:07Z", "2026-11-23T21:06:45Z", "failure")],
        }
        rc, out = self.run_on(job, logs)
        self.assertEqual(rc, 1, out)
        self.assertIn("signal: 9", out)

    def test_a_137_is_the_same_kill_seen_from_the_shell(self):
        logs = {138003727733: "xvfb-run: error: Xvfb failed to start\nbash: line 3: 1234 Killed ...\n##[error]Process completed with exit code 137.\n"}
        job = {
            "id": 138003727733,
            "name": "test (workspace)",
            "conclusion": "failure",
            "steps": [step(7, "workspace tests (nextest)", "2026-11-23T20:50:07Z", "2026-11-23T21:06:45Z", "failure")],
        }
        rc, out = self.run_on(job, logs)
        self.assertEqual(rc, 1, out)
        self.assertIn("137", out)

    def test_every_step_green_and_the_job_red_is_not_filed_under_either(self):
        rc, out = self.run_on(ALL_STEPS_GREEN)
        self.assertEqual(rc, 2, out)
        self.assertTrue(out.startswith("unknown:"), out)

    def test_a_cancelled_run_is_its_own_word(self):
        rc, out = self.run_on(CANCELLED)
        self.assertEqual(rc, 0, out)
        self.assertTrue(out.startswith("cancelled:"), out)

    def test_a_run_whose_test_job_passed_is_not_stopped_by_the_host(self):
        # 46023866009's first version: the classify job read the API while it was still running, saw its own
        # current step with a start and no conclusion, and reported "the host stopped this run" over a test job
        # that had concluded. A job that has not finished says nothing about the run.
        passed = {
            "name": "test (workspace)",
            "status": "completed",
            "conclusion": "success",
            "steps": [step(7, "workspace tests (nextest)", "2026-11-27T15:20:00Z", "2026-11-27T16:17:20Z")],
        }
        rc, out = self.run_on(passed)
        self.assertEqual(rc, 0, out)
        self.assertIn("in-progress: what stopped the run", out)
        self.assertNotIn("host-stopped", out)

    def test_a_host_stop_outranks_a_job_the_classifier_cannot_explain(self):
        # One job it can name, one it cannot: the run is still one the host stopped, and reporting `unknown` over
        # that would throw the useful verdict away.
        puzzling = {
            "name": "something else",
            "status": "completed",
            "conclusion": "failure",
            "steps": [step(1, "Set up job", "2026-11-27T15:20:00Z", "2026-11-27T15:20:01Z")],
        }
        with tempfile.TemporaryDirectory() as tmp:
            payload = Path(tmp) / "jobs.json"
            payload.write_text(json.dumps({"jobs": [KILLED_MID_STEP, puzzling, SELF]}))
            done = subprocess.run([sys.executable, str(HERE / "classify_run.py"), str(payload)],
                                  text=True, capture_output=True)
        self.assertEqual(done.returncode, 1, done.stdout)
        self.assertIn("host-stopped: test (workspace)", done.stdout)


if __name__ == "__main__":
    unittest.main()
