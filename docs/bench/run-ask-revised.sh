#!/usr/bin/env bash
# **One arm of the ask measurement on today's main** (#457, DM-06, 2026-11-20).
#
# The question: with the **revised** guide (the four-or-five demand removed after the 2026-11-15 run found
# only 51% complied), what share of eligible trials now ask at least one question, and how many questions
# per call? The 2026-11-15 figure is the thing to compare against, so this run must be shaped like it:
# same 20 dev tasks, same agent, same model, same `--jobs 1`.
#
# **Why one arm and not two.** Arm A ("no guide section at all") needs `--omit-guide-sections`, which main
# does not have: the shipped guide is `include_str!`-compiled into the server binary and no argument serves
# a different text. The overlay mechanism in `variants.yaml` *adds* a project guide layer and cannot
# subtract a section of the shipped one, so arm A is not expressible here at all. Main is rebasing that
# switch. What today's main *can* answer is "did the revised guide raise asking", which is the question the
# 2026-11-15 run leaves open and the sharper use of the time.
#
# `--jobs 1`: every trial editor draws on the NVIDIA card beside d3-flash, big has no software Vulkan, and
# `--jobs 2` OOM'd 56 of 60 trials on 2026-11-15.
set -euo pipefail
BIN=$HOME/.cargo-target-fleet/dm06/debug
REPO=/home/ladvien/worktrees/bevy_dark_factory-dm06
STATE=$HOME/.local/state/dark_factory-bench
TRIALS=${TRIALS:-3}
RUN=${RUN:-ask-revised-guide}

echo "== $TRIALS trials of each dev task, omp + $(ls "$REPO"/bench/tasks | grep -vc 'yaml\|README') tasks"
date -u

# **d3-flash stays resident.** Its ttl is 60 s and there is no room to reload it beside a trial's editor.
# One question every 20 s, no lease: the keep-alive must not evict the editors it is sitting beside.
(
  while true; do
    curl -s -m 20 -o /dev/null -X POST http://127.0.0.1:8081/upstream/d3-flash/v1/systemone \
      -H 'content-type: application/json' \
      -d '{"model":"d3-flash","state":"a room","questions":{"there":{"type":"noul","instructions":"The room has a floor."}}}'
    sleep 20
  done
) &
KEEPALIVE=$!
trap 'kill $KEEPALIVE 2>/dev/null' EXIT

cd "$REPO" || exit 1
"$BIN/dark_factory-bench" run \
  --agent omp --model anthropic/claude-sonnet-5-5 \
  --trials "$TRIALS" --jobs 1 \
  --run "$RUN" 2>&1 | tail -40

echo "== report"
"$BIN/dark_factory-bench" report "$STATE/$RUN" > "$STATE/$RUN.report.md" 2>&1
head -25 "$STATE/$RUN.report.md"

# **The guide this arm served, hashed.** `--print-guide` is not an argument this build has (it arrives with
# `--omit-guide-sections`, which is what makes an arm's guide differ at all), and the run's own `run.json`
# already carries `guide_sha256` -- the file the server compiled in. So hash the file, and refuse to write a
# hash at all if it cannot be read: a pipeline whose failure is swallowed records `e3b0c442` (the hash of
# empty input) as "the text the arm served", which is a silent corruption of the very record the report uses.
GUIDE=$REPO/crates/dark_factory_mcp/src/guide.md
if [ -r "$GUIDE" ]; then
  sha256sum "$GUIDE" | cut -d' ' -f1 > "$STATE/$RUN.served-guide.sha256"
  echo "== guide sha256: $(cat "$STATE/$RUN.served-guide.sha256")  ($GUIDE)"
else
  echo "== the guide at $GUIDE cannot be read: NOT writing a served-guide hash, because an empty one is a lie" >&2
fi
echo "== done $(date -u)"
