#!/usr/bin/env bash
# **Run a heavy step only while no push gate is in its memory-heavy phase** (#674).
#
# The push gate on `big` takes `/tmp/fleet/big-heavy.lock` *exclusively* around its whole run
# (`tools/loop/hooks/pre-push`, `docs/loop/RUNNING-A-LOOP.md`), and every ad hoc cargo run on that host takes
# it *shared*. A CI container could see neither the lock nor the directory, so a weekly run and a gate could
# both be in their compile-and-test phase on a 32 GB host whose swap is 31 GB of zram: whoever needed more was
# killed, and the kill was reported as a test failure. `gh-runner-run` now mounts the gate's directory at
# `/fleet` read-only, and this script takes the same lock the gate and the workers already use, so the two
# queue against each other instead of over-committing RAM.
#
# Usage: wait-for-gate.sh <command> [args...]
#
#   GHR_HEAVY_LOCK  the lock to take (default /fleet/big-heavy.lock)
#   GHR_GATE_LOG    where to append the one line a person reads afterwards (default /dev/null)
#
# **The lock is held by this shell, not by the command's children.** `bash` opens the lock file on a dynamic
# descriptor and takes it shared, then runs the command as a *child* with that descriptor closed (`{fd}<&-`);
# measured, `exec cmd {fd}<&-` would hand the descriptor over and drop the lock the instant the step began, and
# without `{fd}<&-` a test binary that leaks a daemon would hold the gate off for as long as the daemon lived
# (the 2026-11-13 stall: a leaked `Xvfb` held the shared lock for 13 minutes). This way the lock covers exactly
# the command's own lifetime, and nothing it spawns can inherit it.
#
# A host with no gate at all (a laptop, or a container started without the mount) has no lock file. That is not
# an error: the step says there is nothing to wait for and runs at once, because waiting forever on a lock that
# does not exist would turn a green run red on a host that has no gate.
set -u
lock="${GHR_HEAVY_LOCK:-/fleet/big-heavy.lock}"
log="${GHR_GATE_LOG:-/dev/null}"

say() {
  printf '%s\n' "$*" >&2
  [ "$log" = /dev/null ] || printf '%s\n' "$*" >>"$log" 2>/dev/null || true
}

if [ "$#" -eq 0 ]; then
  echo "wait-for-gate: usage: $0 <command> [args...]" >&2
  exit 2
fi

if ! command -v flock >/dev/null 2>&1; then
  say "gate lock: no flock on this host, so a gate cannot be excluded; running at once"
  "$@"
  exit $?
fi
if [ ! -e "$lock" ]; then
  say "gate lock: $lock does not exist, so no push gate runs on this host; running at once"
  "$@"
  exit $?
fi

fd=""
if ! exec {fd}<"$lock"; then
  say "gate lock: $lock cannot be opened, so a gate cannot be excluded; running at once"
  "$@"
  exit $?
fi
if flock -n -s "$fd" 2>/dev/null; then
  say "gate lock: $lock was free; took it shared for this step"
else
  # Exclusive means the gate's own run: compiling and testing the whole workspace is the memory-heavy phase
  # this step must not join, so it waits here, before any compiler starts, where waiting is free.
  say "gate lock: a push gate holds $lock; waiting for it to finish before this step starts"
  started=$SECONDS
  flock -s "$fd"
  say "gate lock: took the shared lock after waiting $((SECONDS - started))s behind the gate"
fi
"$@" {fd}<&-
exit $?
