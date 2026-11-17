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
# Measured 2026-11-16 beside a resident olmocr (13698 MiB), laya (1528), the reranker (920) and
# distill's embedder released (292): this server holds 7022 MiB, leaving 337 MiB on the card. 7150 is
# that plus the ~130 MiB a request allocates while it runs. The old 8192 (and the 7664 a `-c 8192` load
# cost) refused every attempt: olmocr grew from 12670 to 13698 MiB since 11-15 and the card did not.
NEED_MIB=${D3_NEED_MIB:-7150}
# One slot of 2048 tokens, 1024 ubatch. Context is split across slots, so `-c 8192` gave 4 slots of 8192
# and cost 7664 MiB; `-np 1` gives the whole context to one request instead of 2048 each. 1024 ubatch
# halves the compute buffer — at 2048 the image request's own 120 MiB allocation failed even though the
# model had loaded. The vision budget is capped at 1024 tokens by ubatch either way, so an image keeps
# its full input: the oracle fixture is 1074 input tokens and answers bit-identically to the 11-15 run
# (has_ui 0.9195, map_view 0.9003, clarity 2.606) in 0.77-1.09 s.
CTX=${D3_CTX:-2048}
UBATCH=${D3_UBATCH:-1024}
SLOTS=${D3_SLOTS:-1}
WHO=${WHO:-d3-flash@big}
SWAP=${D3_SWAP:-http://127.0.0.1:8081}
DISTILL_RELEASE=${D3_DISTILL_RELEASE:-http://127.0.0.1:7434/embedder/release}
SECRETS=${D3_SECRETS:-$HOME/.home-still/secrets.env}
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
# The embedder is the model that decides whether this fits. On 2026-11-15 it sat at 292 MiB and d3-flash
# loaded at 8192; by 11-16 it held 5430 MiB and every load was refused — that, not the olmocr check, is
# what broke the endpoint. Ask it to give the card back, exactly as gpu-tenant's prepare_card does under
# a lease; it reloads on the next search, so nothing is lost but a cold embed.
#
# Its release is asynchronous and home-still can reload it between our ask and the check (measured: the
# indexer retries every 30 s and "released on request released=0" means it was already gone). So ask, then
# poll the card for up to a minute instead of trusting one sleep: a single 8 s wait left three of four
# requests to fail on a stale reading.
token=$(sed -n 's/^[[:space:]]*\(export[[:space:]]*\)\{0,1\}HS_BACKEND_TOKEN=//p' "$SECRETS" 2>/dev/null \
        | head -1 | tr -d '"'"'")
free=0
if [ -n "$token" ]; then
  waited=0
  while [ "$waited" -lt 60 ]; do
    status=$(curl -sS -m 20 -o /dev/null -w '%{http_code}' -X POST "$DISTILL_RELEASE" \
               -H "Authorization: Bearer $token" 2>/dev/null || echo 000)
    [ "$status" = 200 ] || echo "d3-flash: distill release answered $status" >&2
    free=$(nvidia-smi --query-gpu=memory.free --format=csv,noheader,nounits | head -n 1 || echo 0)
    [ "${free:-0}" -ge "$NEED_MIB" ] && break
    sleep 5
    waited=$((waited + 5))
  done
else
  echo "d3-flash: no HS_BACKEND_TOKEN in $SECRETS; cannot ask distill's embedder to free the card" >&2
fi

if [ "${free:-0}" -lt "$NEED_MIB" ]; then
  echo "d3-flash needs $NEED_MIB MiB of VRAM and big has $free MiB free (gpu-tenant status)" >&2
  exit 75
fi

# One more ask, with no sleep after it. The check above can pass and the embedder still return inside the
# 4-5 s llama-server takes to page in 6.6 GB — measured 2026-11-16 (18:54:23): the check saw 7 366 free,
# the load then failed with `unable to allocate CUDA0 buffer` and llama-swap answered 500 after 13.3 s.
# Asking twice narrows the window to the load itself; it cannot be closed, because distill's indexer is a
# separate process deciding on its own timer. A load that loses this race fails in seconds and the next
# request starts it again, which is llama-swap's normal cold-start path.
if [ -n "$token" ]; then
  curl -sS -m 20 -o /dev/null -X POST "$DISTILL_RELEASE" \
    -H "Authorization: Bearer $token" 2>/dev/null || true
fi

# No RAM-capped scope here (unlike `gpu-tenant run`): inside llama-swap's own cgroup the reaper counts this server
# as a resident (llama-swap.service) and does not flag it as unclaimed GPU use. Fully offloaded, it holds ~1 GB of host RAM.
exec "$D/llama-server" -m "$S/vllm-d3-flash-$QUANT.gguf" --mmproj "$S/mmproj-vllm-d3-flash-Q8_0.gguf" \
  -ngl 99 -c "$CTX" --ubatch-size "$UBATCH" -np "$SLOTS" \
  --host "${HOST:-127.0.0.1}" --port "$PORT" --alias d3-flash
