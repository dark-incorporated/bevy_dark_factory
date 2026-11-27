#!/usr/bin/env python3
"""**Say whether a CI run failed on the code or was stopped by the host** (#674, item 3).

A run killed by the host is indistinguishable from a test failure in the run list: the step whose process died
has no conclusion at all, and GitHub reports the job `failure`. Of the 17 runs in #674's census, 7 were killed
mid-step and 6 never started a job, against 1 that failed on a test — 13 reds that had to be sorted by hand.

The evidence a stopped run leaves, in the order it is trustworthy:

* **`memory.events` / a `signal: 9` in the log.** A compiler or linker killed for memory writes
  `signal: 9 (SIGKILL)` and cargo prints the command it was running. This is the one signature that names the
  host without any inference: nothing in this repository SIGKILLs itself.
* **Exit 137** from a step, which is `128 + 9`: the same kill seen from the shell rather than from cargo.
* **A heavy step with no conclusion.** The runner died while that step's process was running, so GitHub never
  recorded an end for it. A step that started and has a conclusion is not this.

Everything else is the code's: a test that failed, a compile that failed, a step that exited non-zero on its
own. A run whose steps all concluded `success` and whose job still failed is reported as neither, because that
is the `tee` shape #572 closed and it should not be quietly filed under a test failure.

Usage: classify_run.py <job-json-file> [<log-file-or-dir>]

  <job-json-file> is `gh api repos/O/R/actions/runs/<id>/jobs` -- the `jobs` array, as JSON.
  <log-file-or-dir> is the job's whole log (one file), or a directory of `<step-number>.log` files, if the
                    run's logs are still there. A kill found anywhere in a job's log names that job.

Writes one line per job to stdout and exits 1 when a job was stopped by the host, so a workflow step can wear that
status, and 2 when it cannot account for a job at all.
"""

import json
import os
import re
import sys

# A kill by the kernel's OOM killer, or by systemd-oomd on the slice's swap rule, says one of these. cargo
# prints the second from the child it was waiting on; the shell prints the third when it is the one reaped.
KILLED = [
    re.compile(r"signal: 9\b"),
    re.compile(r"signal 9\b"),
    re.compile(r"exit code 137\b"),
    re.compile(r"Killed process \d+"),
    re.compile(r"Out of memory: Killed"),
    re.compile(r"oom-kill"),
    re.compile(r"memory cgroup out of memory"),
]


def job_log(log_dir, job_id):
    """The whole log of one job, if the run still has it."""
    if not log_dir:
        return ""
    path = os.path.join(log_dir, f"{job_id}.log")
    if not os.path.exists(path):
        return ""
    with open(path, encoding="utf-8", errors="replace") as handle:
        return handle.read()


def where_a_kill_is_spelled(text):
    """The line that names a kill, so the report can quote it instead of asserting one."""
    for line in text.splitlines():
        for pattern in KILLED:
            if pattern.search(line):
                return line.strip()[:200]
    return ""


def classify(job, log_dir):
    """(verdict, the line that says why) for one job."""
    # **A job that has not finished is not evidence of anything** (#674). Measured on run 46023866009: the
    # classifier's own job was still in progress while it read the API, so its current step had a start and no
    # conclusion, and it reported itself as "the host stopped this run" over a job that was merely running. The
    # caller passes every job in the run, including this one.
    if job.get("status") not in (None, "completed"):
        return "in-progress", f"still {job.get('status')}, so it is not evidence about the run yet"
    steps = job.get("steps") or []
    text = job_log(log_dir, job.get("id"))
    if text:
        killed = where_a_kill_is_spelled(text)
        if killed:
            return "host-stopped", f"its log names the kill: {killed}"
    # No log, or none that names a kill: the structural signature is a heavy step with no conclusion. "Set up
    # job" and "Set up runner" legitimately have none when the runner never started, which is the third case.
    started_never_ended = [
        step for step in steps if step.get("started_at") and not step.get("conclusion")
    ]
    if started_never_ended:
        first = started_never_ended[0]
        return "host-stopped", (
            f"step {first.get('number')} ({first.get('name')}) started at {first.get('started_at')} and has no "
            f"conclusion: the runner died while its process was running"
        )
    if not steps:
        return "host-stopped", "the job has no steps: the runner never started it"
    failed = [step for step in steps if step.get("conclusion") == "failure"]
    if failed:
        first = failed[0]
        return "test-failed", f"step {first.get('number')} ({first.get('name')}) concluded failure"
    cancelled = [step for step in steps if step.get("conclusion") == "cancelled"]
    if cancelled:
        return "cancelled", f"step {cancelled[0].get('number')} was cancelled"
    if job.get("conclusion") == "success":
        return "passed", "every step concluded and the job passed"
    return "unknown", "every step concluded and none failed, so nothing here explains the job's failure"


def main(argv):
    if not 1 <= len(argv) <= 2:
        print(__doc__.strip().splitlines()[0], file=sys.stderr)
        print("usage: classify_run.py <job-json-file> [<job-log-dir>]", file=sys.stderr)
        return 2
    with open(argv[0], encoding="utf-8") as handle:
        payload = json.load(handle)
    jobs = payload.get("jobs", payload) if isinstance(payload, dict) else payload
    log_dir = argv[1] if len(argv) == 2 else ""
    # **`host-stopped` outranks `unknown`.** It is the answer to the question this script exists for; `unknown`
    # only means the script has nothing to say about one job. A run with one of each is still a run the host
    # stopped, and reporting `unknown` over that would throw the useful verdict away.
    host_stopped = unknown = False
    for job in jobs:
        verdict, why = classify(job, log_dir)
        print(f"{verdict}: {job.get('name')} — {why}")
        host_stopped = host_stopped or verdict == "host-stopped"
        unknown = unknown or verdict == "unknown"
    if host_stopped:
        return 1
    return 2 if unknown else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
