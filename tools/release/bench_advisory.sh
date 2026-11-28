#!/bin/bash
# The owner's ruling (2026-11-16): a release is NOT gated on a benchmark run until the editor is built out.
# The check still runs and prints its verdict, so a regression is visible in the release log; it never refuses.
set -u
python3 tools/release/bench_gate.py check "$@" || true
# **The workspace switch and the terminal raise** (#678, #541): the newest two committed runs of tools/demo/switch_drive.py, side
# by side, so a release shows whether a switch got slower. The run happens where an editor can open a window, as the bench's does.
python3 tools/demo/switch_drive.py --report docs/bench/switch || true
echo "bench gate: advisory (owner, 2026-11-16): a release is not refused on a benchmark result"
exit 0
