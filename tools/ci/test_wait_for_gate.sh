#!/usr/bin/env bash
# **The four things `wait-for-gate.sh` must do, measured** (#674). Every one of these was got wrong in a first
# version and caught by running it, not by reading it, so each is a test here.
#
#   1. The lock covers the command's whole life and nothing longer. `exec cmd {fd}<&-` was measured to *drop* the
#      lock the instant the step started; running the command as a child with the descriptor closed keeps it.
#   2. Nothing the command spawns inherits the lock. A test that leaks a daemon held the gate off for 13 minutes
#      on 2026-11-13, and a CI step must not be able to do that by accident.
#   3. A step started while a gate holds the lock exclusively runs when the gate finishes, not before.
#   4. A host with no gate (no lock file) runs at once rather than hanging, and says so.
#
# Run: `bash tools/ci/test_wait_for_gate.sh` on Linux or a Mac with flock (util-linux; big and the runners have it).
set -uo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
helper="$here/wait-for-gate.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
lock="$work/big-heavy.lock"
: >"$lock"

fails=0
ok() { printf 'ok    %s\n' "$*"; }
no() { printf 'FAIL  %s\n' "$*"; fails=$((fails + 1)); }
# Is the lock free to take exclusively? That is the question "does someone hold it" means.
free() { if flock -n -x "$lock" true 2>/dev/null; then echo free; else echo held; fi; }

# **Probe the capability, not `command -v`.** An interactive shell on a Mac without util-linux may still have a
# `flock` name (a function, an alias, another package's binary) that does not take an exclusive lock on a file
# descriptor. The test is the first thing it does, and only a real `flock -x` passes it.
if ! : >"$lock" || ! flock -n -x "$lock" true 2>/dev/null; then
  echo "skip: no usable flock on this host (this is a Mac without util-linux; the runners and big have it)" >&2
  exit 0
fi
# And the helper must agree with this probe, or the test would be measuring a different mechanism than CI runs.
if GHR_HEAVY_LOCK="$lock" "$helper" true 2>&1 | grep -q 'no flock'; then
  echo "skip: wait-for-gate.sh cannot use flock here even though this shell can" >&2
  exit 0
fi

# 1. Held while the command runs, released when it ends.
GHR_HEAVY_LOCK="$lock" "$helper" sleep 2 &
step=$!
sleep 0.7
[ "$(free)" = held ] && ok "the lock is held while the command runs" || no "the lock was free while the step ran"
wait "$step"
[ "$(free)" = free ] && ok "the lock is released when the command ends" || no "the lock was still held after the step"

# 2. A daemon the command leaves behind does not keep it.
GHR_HEAVY_LOCK="$lock" "$helper" bash -c 'setsid -f sleep 30 >/dev/null 2>&1' >/dev/null 2>&1
sleep 0.3
[ "$(free)" = free ] && ok "a daemon the step leaked inherits nothing" || no "a leaked daemon kept the lock"

# 3. Queued behind an exclusive holder, it starts when the holder lets go.
( flock -x "$lock" sleep 3 ) &
gate=$!
sleep 0.4
started=$SECONDS
GHR_HEAVY_LOCK="$lock" "$helper" true
waited=$((SECONDS - started))
wait "$gate"
[ "$waited" -ge 2 ] && ok "a step waits ${waited}s behind a gate holding the lock" \
  || no "the step ran after only ${waited}s while a gate held the lock"

# 4. No lock file at all: run at once, and say why.
out=$(GHR_HEAVY_LOCK="$work/no-such-lock" "$helper" echo ran 2>&1)
case "$out" in
  *"does not exist"*ran*) ok "a host with no gate runs at once and says there is nothing to wait for" ;;
  *) no "no lock file did not run at once: $out" ;;
esac

# And the command's own exit status survives, or a failing test would be reported as a passing one.
GHR_HEAVY_LOCK="$lock" "$helper" bash -c 'exit 42' >/dev/null 2>&1
[ $? -eq 42 ] && ok "the command's exit status is the step's" || no "the exit status was lost"

printf '\n%s\n' "$([ "$fails" -eq 0 ] && echo "all checks passed" || echo "$fails check(s) failed")"
[ "$fails" -eq 0 ]
