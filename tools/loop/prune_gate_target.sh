#!/usr/bin/env bash
# **Keep the newest test executable of each name in the push gate's target, and delete the older ones.**
#
# Why (measured 2026-11-21 on big): `~/.cargo-target-gate` was 331 GB, 219 GB of it 738 test executables of 50 MB to 3.4 GB, and 162 GB of those
# (533 files) were older generations of a test that had a newer build (three hashes of `a_handback_becomes_a_task_draft`, three of each
# `every_task_is_valid`...). Cargo names a test executable `<test>-<16 hex of its metadata>` and never reuses an older hash once the tree
# or a dependency moves on, so every gate left another 3 GB per test behind. `/home` was at 158 GB free, under the 150 GB a loop pass
# wants (`WIGGUM_MIN_FREE_GB`); pruning took it to 308 GB. The loops' watchdog prunes the loops' targets, never this one.
#
# Only extensionless files over MIN_MB in `debug/deps` are considered (test executables): never an rlib, a `.d` file or a build script.
# A gate is never pruned under: `fleet-gate-rel` active, or any `cargo`/`rustc` running with this target, makes it refuse (exit 3).
#
# **A stated ceiling, because "nothing runs it" was the defect** (#578). The script was correct and nothing scheduled it, so
# the target was 283 GB on 2026-11-23 with 195 superseded executables (129.4 GB) cargo could never reuse and `/home` at 99%
# full. `tools/loop/gate-prune.timer` runs it daily; `CEILING_GB` is the number it is held to, and a prune that still leaves
# the target over it exits 4 so the timer's failure is a line in the journal rather than a quiet month of growth.
#
# Usage: tools/loop/prune_gate_target.sh            dry run: what it would delete
#        tools/loop/prune_gate_target.sh --apply    delete it
# Environment: GATE_TARGET (default ~/.cargo-target-gate), MIN_MB (default 50), GATE_UNIT (the systemd user unit that runs a gate,
# default fleet-gate-rel), CEILING_GB (default 120: the newest generation of every test on 2026-11-23 was 57.6 GB, so 120 is
# twice that and half the 283 GB the target reached without a pruner).
set -u
TARGET="${GATE_TARGET:-$HOME/.cargo-target-gate}"
MIN_MB="${MIN_MB:-50}"
GATE_UNIT="${GATE_UNIT:-fleet-gate-rel}"
CEILING_GB="${CEILING_GB:-120}"
DEPS="$TARGET/debug/deps"
APPLY=0
case "${1:-}" in
  "") ;;
  --apply) APPLY=1 ;;
  *) echo "usage: $0 [--apply]" >&2; exit 64 ;;
esac
[ -d "$DEPS" ] || { echo "no $DEPS: nothing to prune" >&2; exit 2; }

# A running gate, or any process building into or running out of this target, makes its newest executables the ones a test is about to
# run. An environment variable is not part of a command line, so the match is the target's path: a test executable is started by its
# path under `debug/deps`, and cargo hands rustc `--out-dir <target>/debug/deps`.
if command -v systemctl >/dev/null 2>&1 && [ "$(systemctl --user is-active "$GATE_UNIT" 2>/dev/null)" = active ]; then
  echo "refusing: $GATE_UNIT is active" >&2; exit 3
fi
if pgrep -f -- "$TARGET/" >/dev/null 2>&1; then
  echo "refusing: a process is using $TARGET:" >&2; pgrep -fl -- "$TARGET/" | head -3 | cut -c1-160 >&2; exit 3
fi

# `find -printf` is GNU; the gate runs on Linux only. Newest first, so the first of each name is the one kept.
find "$DEPS" -maxdepth 1 -type f ! -name '*.*' -size "+${MIN_MB}M" -printf '%T@ %s %p\n' | sort -rn | python3 -c '
import os, re, sys
apply = sys.argv[1] == "1"
seen = {}
doomed = []
kept_bytes = doomed_bytes = 0
for line in sys.stdin:
    _stamp, size, path = line.rstrip("\n").split(" ", 2)
    size = int(size)
    name = re.sub(r"-[0-9a-f]{16}$", "", os.path.basename(path))
    if name not in seen:
        seen[name] = path
        kept_bytes += size
    else:
        doomed.append((path, size))
        doomed_bytes += size
GB = 1 << 30
print(f"{len(seen)} test names; newest of each kept: {kept_bytes / GB:.1f} GB; older generations: {len(doomed)} files, {doomed_bytes / GB:.1f} GB")
if not apply:
    print("dry run: nothing deleted (pass --apply)")
    sys.exit(0)
freed = 0
for path, size in doomed:
    try:
        os.remove(path)
        freed += size
    except FileNotFoundError:
        pass
print(f"deleted {len(doomed)} files, {freed / GB:.1f} GB")
' "$APPLY"
status=$?
echo "free on $(df -P "$TARGET" | awk 'NR==2{print $6}'): $(df -h "$TARGET" | awk 'NR==2{print $4}')"
# A dry run has not deleted anything, so it reports the target as it stands and does not judge the ceiling.
[ "$APPLY" = 1 ] || exit $status
target_gb=$(du -s --block-size=1G "$TARGET" | cut -f1)
echo "target is ${target_gb} GB (ceiling ${CEILING_GB} GB)"
[ "$target_gb" -le "$CEILING_GB" ] || {
  echo "OVER CEILING: ${target_gb} GB > ${CEILING_GB} GB after pruning the superseded executables; something besides test builds is growing $TARGET" >&2
  exit 4
}
exit $status
