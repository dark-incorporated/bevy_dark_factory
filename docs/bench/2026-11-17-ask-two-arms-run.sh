#!/usr/bin/env bash
# **The two arms of the look-then-ask question, run on big** (#457, DM-06).
#
# One arm per run, both at the same commit, the same tasks, the same trials and the same agent and model. The only
# difference is the suite arm: `ask-with-guide` serves the shipped guide, `ask-without-guide` serves it with
# "Check your work by asking" cut out (`--omit-guide-sections`, through the editor's own `agent_setup`).
#
# `--jobs 1` because each trial's editor draws on the card beside d3-flash, and two OOM'd 56 of 60 trials on
# 2026-11-15 (see docs/bench/2026-11-15-ask-experiment.md). A keep-alive holds d3-flash resident: its ttl is 60 s
# and there is no room to load it again beside a trial's editor.
set -u
BIN=$HOME/.cargo-target-gate/debug
REPO=/home/ladvien/askhelps-big
STATE=$HOME/.local/state/cauldron-bench
TRIALS=${TRIALS:-3}

echo "== arms: $TRIALS trials of each dev task, $(ls "$REPO"/bench/tasks | grep -vc 'yaml\|README') tasks, omp anthropic/claude-sonnet-5-5"
date -u

# **d3-flash stays resident.** One question every 20 s, no lease, so the card keeps the model between trials.
(
  while true; do
    curl -s -m 20 -o /dev/null -X POST http://big.local:8081/upstream/d3-flash/v1/systemone \
      -H 'content-type: application/json' \
      -d '{"model":"d3-flash","state":"a room","questions":{"there":{"type":"noul","instructions":"The room has a floor."}}}'
    sleep 20
  done
) &
KEEPALIVE=$!
trap 'kill $KEEPALIVE 2>/dev/null' EXIT

cd "$REPO" || exit 1
for ARM in with without; do
  echo "== arm $ARM: $(date -u)"
  "$BIN/cauldron-bench" run \
    --agent omp --model anthropic/claude-sonnet-5-5 \
    --variant "ask-$ARM-guide" --trials "$TRIALS" --jobs 1 \
    --run "ask-two-arms-$ARM" 2>&1 | tail -40
  # **The text the arm actually served**, hashed. The run's header carries `guide.md`'s own SHA-256, which is
  # the same in both arms because the file is the same; this is the served layer's hash, and the two together
  # say which file and which of its sections an arm was given.
  case $ARM in
    with) "$BIN/bevy_cauldron_mcp" --print-guide | sha256sum | cut -d" " -f1 ;;
    without) "$BIN/bevy_cauldron_mcp" --omit-guide-sections "Check your work by asking" --print-guide | sha256sum | cut -d" " -f1 ;;
  esac > "$STATE/ask-two-arms-$ARM.served-guide.sha256"
  echo "== arm $ARM served guide: $(cat "$STATE/ask-two-arms-$ARM.served-guide.sha256")"
  echo "== arm $ARM done: $(date -u)"
done

for ARM in with without; do
  echo "== report $ARM"
  "$BIN/cauldron-bench" report "$STATE/ask-two-arms-$ARM" > "$STATE/ask-two-arms-$ARM.report.md" 2>&1
  head -20 "$STATE/ask-two-arms-$ARM.report.md"
done
