#!/bin/sh
# Serve vllm-sr/d3-flash's /v1/systemone on big (issue #452, DM-01), as a llama-swap model.
#
# llama-swap (:8081, ~/.home-still/llama-swap.yaml) starts this on the first request to
#   POST http://big.local:8081/upstream/d3-flash/v1/systemone
# and stops it after its `ttl` of idle. `gpu-tenant` unloads every model on :8081 when a lease or the
# coding model takes the card, so d3-flash is evicted without holding a lease itself, and `guard`
# below keeps it from loading again while the card is yielded to somebody else.
#
# Install (llama-swap runs the installed copy, so a worktree can go away):
#   cp tools/decide/serve-d3-flash.sh ~/.home-still/run-d3-flash.sh
# then the llama-swap.yaml entry in docs/decide/serving-d3-flash.md.
#
# Needs llama.cpp b11982 (PR #30383 has d3): `cuda-ece963` (build 1498) cannot serve it, and without the
# cudart directory on LD_LIBRARY_PATH the CUDA backend silently fails to load and every request runs on the
# CPU at ~14 s.
#
#   serve-d3-flash.sh [PORT]     PORT is what llama-swap passes as ${PORT}; default 8093
#   D3_QUANT=Q8_0                the 9.5 GB weights instead of Q4_K_M (download them first)
#   D3_NEED_MIB=N                VRAM that must be free before loading (default: measured footprint + margin)
#   HOST=0.0.0.0                 bind beyond loopback (standalone runs only: llama-swap proxies on loopback)
#   D3_STANDALONE=1              skip the olmocr check (a run under your own lease: WHO=<you>@big)
set -eu
PORT=${1:-${PORT:-8093}}
B=${D3_LLAMA:-$HOME/.local/llama.cpp/w3-b11982}
D=$B/llama-b11982
QUANT=${D3_QUANT:-Q4_K_M}
NEED_MIB=${D3_NEED_MIB:-8192}
WHO=${WHO:-d3-flash@big}
SWAP=${D3_SWAP:-http://127.0.0.1:8081}
S=$(ls -d "$HOME"/.cache/huggingface/hub/models--ggml-org--vllm-d3-flash-GGUF/snapshots/* | head -1)
export LD_LIBRARY_PATH=$D:$B/cudart-llama-b11982-bin-ubuntu-cuda-12.8-x64

# 75 (EX_TEMPFAIL) is what home-still's launch scripts exit with when the card is not theirs to take.
gpu-tenant guard --who "$WHO" || exit 75
# Never instead of home-still: olmocr (14 GB, the card's default tenant) cannot start beside d3-flash (7.7 GB),
# so d3-flash loads only beside an olmocr that is already resident. The reaper brings olmocr back within a
# minute of a lease ending; loading olmocr later evicts d3-flash (llama-swap.yaml, group `home`).
if [ -z "${D3_STANDALONE:-}" ] && ! curl -sf -m 3 "$SWAP/running" | grep -q '"model":"olmocr","state":"ready"'; then
  echo "d3-flash loads beside home-still's olmocr and olmocr is not resident (a lease or the coding model holds the card, or olmocr is loading): ask again in a minute (gpu-tenant status)" >&2
  exit 75
fi
free=$(nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits | head -n 1)
if [ "$free" -lt "$NEED_MIB" ]; then
  echo "d3-flash needs $NEED_MIB MiB of VRAM and big has $free MiB free (gpu-tenant status)" >&2
  exit 75
fi

# No RAM-capped scope here (unlike `gpu-tenant run`): inside llama-swap's own cgroup the reaper counts this server
# as a resident (llama-swap.service) and does not flag it as unclaimed GPU use. Fully offloaded, it holds ~1 GB of host RAM.
exec "$D/llama-server" -m "$S/vllm-d3-flash-$QUANT.gguf" --mmproj "$S/mmproj-vllm-d3-flash-Q8_0.gguf" \
  -ngl 99 -c 8192 --ubatch-size 4096 --host "${HOST:-127.0.0.1}" --port "$PORT" --alias d3-flash
