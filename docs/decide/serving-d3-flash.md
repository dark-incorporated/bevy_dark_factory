# Serving d3-flash (DM-01, #452)

**Path that works: big's llama-swap (`:8081`) starts `llama-server` on the first request and proxies `POST /upstream/d3-flash/v1/systemone` to the model's `/v1/systemone`.** llama-swap has no `/v1/systemone` route of its own (`POST /v1/systemone` answers 404), but its `/upstream/<model>/<any path>` route starts the model and forwards any path, so no second server and no lease are needed. The endpoint every editor uses is

```
http://big.local:8081/upstream/d3-flash/v1/systemone
```

(`bevy_oracle::LOCAL_ENDPOINT`). `big.local` is the name every LAN host resolves (mDNS, checked from mac_air, bmb and big_mac); a bare `big` resolves only on big. Not the IP: the name is what survives a DHCP change.

|Item|Value|
|---|---|
|Server|llama.cpp release b11982 (`8a199ad31`), which has d3 support (PR #30383); `cuda-ece963` (build 1498) cannot load it. `LD_LIBRARY_PATH` carries the b11982 cudart directory (without it the CUDA backend silently fails to load and every request takes ~14 s on the CPU)|
|Model|`ggml-org/vllm-d3-flash-GGUF`: `vllm-d3-flash-Q4_K_M.gguf` (6.6 GB, sha256 `b8753da4…`) and `mmproj-vllm-d3-flash-Q8_0.gguf` (0.62 GB, `0291125a…`); 8.95 B parameters, context 8192 of 262144 trained|
|Launch|`tools/decide/serve-d3-flash.sh PORT` (installed as `~/.home-still/run-d3-flash.sh`): `gpu-tenant guard`, an olmocr-resident check, a **distill-embedder release** (see below), a VRAM check, then `llama-server -ngl 99 -c 2048 --ubatch-size 1024 -np 1 --alias d3-flash`|
|Memory|**7022 MiB VRAM** for the server, beside olmocr (13698), laya (1528), the reranker (920) and distill's embedder released (292): 337 MiB left on the card. The script refuses to load below 7 150 MiB free|
|`/v1/models`|`architecture.output_modalities: ["decisions"]`, inputs text, image, video, `n_ctx` 8192, `ftype` Q4_K - Medium|

## How it stays out of everyone's way

It holds no lease. The three ways the card is taken from it:

|Event|What happens to d3-flash|
|---|---|
|`gpu-tenant claim` / `wait` / `run` (a job lease)|`prepare_card` unloads every model on `:8081`: d3-flash is gone four seconds after the grant (board: `prepare: unloaded d3-flash; unloaded olmocr; distill at 292 MiB`). The lease writes the yield file, so the launch script's `gpu-tenant guard` refuses (exit 75) to load it again until the release|
|`qwen3.8-27b` takes the card (`serve-coding`)|the same `prepare_card` unloads it; the coding model's yield makes the guard refuse a reload while it is resident|
|olmocr is not resident (a lease just ended, glm-ocr swapped it out)|the script refuses to load: olmocr (14 GB) cannot start beside d3-flash, so d3-flash loads only beside an olmocr that is already there, never in front of it. The reaper loads olmocr within about a minute of a release, and the next request after that loads d3-flash|

It also stops itself: `ttl: 60` in `llama-swap.yaml` unloads it a minute after the last request, because 7.7 GB beside olmocr leaves distill's embedder (5 GB) no room to load.

A request that arrives while it cannot load is **not** left to wait: llama-swap's start fails at once (the script exits 75) and answers `500 {"error":"unspecific error: upstream command exited prematurely","src":"llama-swap"}` in 0.25 s. `bevy_oracle::Http` turns that (and any 502, 503 or 504) into `Error::Unreachable` carrying the status, the gateway's words and the remedy: *the server is up but its model is not loaded: a GPU lease or the coding model evicts it, and it reloads on the first request after the card is free and home-still's olmocr is back (a minute or two); on big, `gpu-tenant status` names the holder and `gpu-tenant board -n 10` says since when*. A 500 that is not llama-swap's (the model's own error) stays `Error::Refused`.

### Install on big

```sh
cp tools/decide/serve-d3-flash.sh ~/.home-still/run-d3-flash.sh   # llama-swap runs the installed copy
```

and in `~/.home-still/llama-swap.yaml` (llama-swap reloads it on save, which stops olmocr for a minute: do it when scribe has no conversation in flight, `curl -s localhost:7435/readiness`):

```yaml
models:
  "d3-flash":
    cmd: bash /home/ladvien/.home-still/run-d3-flash.sh ${PORT}
    proxy: "http://127.0.0.1:${PORT}"
    ttl: 60
    concurrencyLimit: 16

groups:
  "home":                       # without groups every model shares one exclusive group and a
    swap: true                  # decision request would evict olmocr in the middle of a conversion
    exclusive: false
    members: ["olmocr", "glm-ocr"]
  "decide":
    swap: true
    exclusive: false
    members: ["d3-flash"]
```

**Do not set `exclusive: true` on `home`.** It was tried, so that loading olmocr would evict d3-flash; a d3-flash request racing olmocr's start then wedged the whole llama-swap instance (olmocr requests included, eight client connections with unread bodies) until it was restarted. With `exclusive: false`, 24 requests racing olmocr's start all failed fast (500 in 0.25 s), the instance stayed responsive, and the first request after olmocr was ready loaded d3-flash in 7.9 s.

## Measured 2026-11-15 on big (RTX 3090, llama-swap v229, olmocr resident)

Fixture request (`crates/bevy_oracle/fixtures/request.json` with `docs/changelog/media/2026-10-24-S1.5.png`: 3 questions, 1074 input tokens; clef-flash read the same request as 619).

|What|Value|
|---|---|
|Load (llama-swap spawn to health check passing)|about 5 s (13:59:49 to 13:59:54 in the log); a standalone `llama-server` start to `/health` took 8.0 s (Q4_K_M)|
|First call after a cold start, from mac_air|6.5 s the very first; 7.1, 4.0 and 6.2 s for three cold reloads after `/unload` (model file in the page cache)|
|VRAM|7708 MiB (server), card 23 496 / 24 576 MiB|
|Warm, 5 calls, median, **on big** (loopback)|1 question **132 ms** (131-134), 3 questions (the fixture) **397 ms**, 4 questions **531 ms**, 8 questions **1072 ms** (1061-1155)|
|Warm, 5 calls, median, **from mac_air** (LAN, 220 KB of base64 image per request)|1 question 387 ms, 3 questions 651 ms, 4 questions 793 ms, 8 questions 1322 ms. The LAN costs about 250 ms on top, the image upload|

d3 runs one forward pass per question: about 130 ms for the first and 135 ms for each more, against clef-flash's one joint pass (187 ms for the three-question fixture). The model card's numbers (MI325X bf16, 1.6 MP image: 166 ms for 1 question, 1.09 s for 8) are what the 3090 does at Q4_K_M.

### The lease, 2026-11-15 14:24 (`gpu-tenant claim coding --who dm01@big --ttl 5m`)

|Time|Event|
|---|---|
|14:24:05|before: request answers (2.1 s, loading cold)|
|14:24:08|claim granted in 4 s; `curl :8081/running` is `[]`: d3-flash and olmocr unloaded|
|14:24:13 and 14:24:34|requests during the lease: `Error::Unreachable` with the remedy after 0.94 s and 0.87 s (0.25 s of that is llama-swap), not a timeout|
|14:24:57|lease released (44 s held)|
|14:24:57 to 14:26:09|requests fail the same way while olmocr reloads (reaper warm-up, about a minute)|
|14:26:14|first success, d3-flash loaded again: 35 s for that call, measured while a 9.5 GB download was thrashing big's page cache|
|14:43:11 to 14:45:05|a second cycle, after the Q4/Q8 lease released at 14:43:10 (big busy with a gate build): olmocr was back after 115 s, and the first request after that **loaded d3-flash cold and answered in 5.9 s**; the next, 0.52 s|

### Q4_K_M against Q8_0 (same fixture, same flags, one lease, standalone ports)

|Quant|`has_ui` yes|`dominant` map_view / text / blank|`clarity` score|VRAM (server)|Load to health|Warm median 1 / 3 / 4 / 8 questions|
|---|---|---|---|---|---|---|
|Q4_K_M (6.6 GB)|0.9195|0.8998 / 0.0885 / 0.0116|2.636|7 708 MiB|8.0 s|136 / 404 / 537 / 1074 ms|
|Q8_0 (9.5 GB)|0.9212|0.8974 / 0.0844 / 0.0181|2.646|10 012 MiB|14.0 s|134 / 402 / 540 / 1075 ms|

Five calls each were identical to the last digit. **The quant moves no probability by more than 0.007 and no answer** (same winner, same score band), and costs nothing in latency (the 3090 is not bound by the weights). Q8_0 does not fit beside olmocr (10 012 MiB against 8 393 free), so Q4_K_M is the served quant. Whether 0.007 matters for a check's threshold is DM-04's calibration to say; if it does, Q8_0 needs `D3_QUANT=Q8_0` and a card without olmocr (a lease).

## What broke it, measured 2026-11-16

The endpoint answered `500 {"error":"unspecific error: upstream command exited prematurely","src":"llama-swap"}` in 0.25 s — the launch script exiting 75 — for every request. Two independent causes, both measured, not inferred:

1. **distill's embedder had grown from 292 MiB to 5430 MiB since 11-15.** On 11-15 the card held olmocr (12 670), laya (1 528), the reranker (920) and the embedder (292), leaving 8 393 MiB, and d3-flash fit at 8 192. By 11-16 the embedder held 5 430, leaving **3 424 MiB** — under half what the script required. `POST http://127.0.0.1:7434/embedder/release` with `HS_BACKEND_TOKEN` from `~/.home-still/secrets.env` returns its VRAM (measured: 200 `{"released":1}`, and 8 s later the card reported 7 366 free). The script now asks, exactly as `gpu-tenant`'s `prepare_card` does under a lease. It reloads on the next search, so the cost is one cold embed.
2. **The old flags cost more than the card has.** `-c 8192` with the default slot count is *four* slots of 8192 tokens (`n_slots = 4, n_ctx_slot = 8192` in the log) and held **7 664 MiB**; olmocr itself had grown 12 670 → 13 698 since 11-15. `-c 2048 --ubatch-size 1024 -np 1` holds **7022 MiB** and still answers the fixture bit-identically.

Three things that look like fixes and are not:

- **`-np 1` at a larger context.** `-c 8192 -np 1` needs 7 514 MiB and does not fit; `-c 3072 -np 1` fits at 7 212 but leaves 149 MiB, and the image request's own 120 MiB `cudaMalloc` then fails *after* a successful load (server aborts on the first call).
- **A smaller ubatch at a larger context.** `-ub 1024` with `-c 4096 -np 2` still refused: the free-check ran while the embedder was reloading. Both the free-check and the load have to see the card with the embedder gone, which is why the script now polls for up to 60 s after asking rather than sleeping once.
- **The vision budget is not reduced.** The image-token cap is applied to `n_ubatch`, not to context: both `-c 8192` and `-c 2048` log `cap image_max_tokens (original=1600) to 1024`, so an image keeps its full 1024 tokens at either size.

Verified through llama-swap after the fix (`POST /upstream/d3-flash/v1/systemone`, the oracle fixture with `2026-10-24-S1.5.png`): cold start 13.4 s, then 0.76-0.85 s, four calls, `has_ui` 0.9195, `dominant` map_view 0.9003, `clarity` 2.606, `input_tokens` 1074, `output_tokens` 0 — identical to the 11-15 baseline table above.

**A residual limitation, not a bug:** home-still's indexer retries every 30 s, so a decision request that lands while the embedder is reloading may still lose the race (observed: three of four calls before the retry loop was added). The retry loop closes it in practice; a request during a running conversion of olmocr's own cold start still fails fast, which is the intended `Error::Unreachable` path.

## Protocol

**`/v1/systemone` speaks the same protocol v1 as clef-flash**: `state`, `images`, `questions` keyed by id with `instructions` (and `criteria` for `choice`/`score`) in; `{"model","answers":{id:{"type", "noul" | "choice"+"probabilities"+"confidence" | "score"+"legend"+"probabilities"+"confidence"}},"usage":{"input_tokens","output_tokens":0}}` out. `bevy_oracle::Request`/`Response` need no change. `output_tokens` is 0 on every call.

### A.4 checks, from every host

`/v1/models`, a text-only `noul`, and the fixture request, through `http://big.local:8081/upstream/d3-flash`:

|From|`/v1/models`|text-only `noul`|fixture request|
|---|---|---|---|
|mac_air|200, decisions|200, 0.9969, 0.71 s|200, 0.99 s; `has_ui` 0.914, `dominant` map_view with probabilities summing to 1 over `blank, map_view, text`, `clarity` 2.61 with `legend` 0-4 summing to 1, `output_tokens` 0|
|bmb|200, decisions|200, 0.9965, 0.24 s|200, 0.61 s, the same answers|
|big (by `big.local`)|200, decisions|200, 0.9965, 0.03 s|200, 0.39 s, the same answers|
|big_mac|**not testable from here**: its ssh sessions cannot open a connection to any LAN host (the gateway, bmb and big all time out), so it is a macOS local-network permission of the ssh session, not a fault of big; an editor started in its GUI session was not tried||

The fixture's recorded clef-flash answers were `has_ui` 0.929, `dominant` map_view 0.977, `clarity` 2.14; d3-flash reads the easy image the same way (0.914, map_view 0.900, 2.61).

## What this found

- **A name costs 57 s unless the client is IPv4-only.** `big.local` resolves to a link-local and two global IPv6 addresses as well as `192.168.1.110`, and big's firewall drops IPv6 SYNs to `:8081` (ssh on `:22` answers over IPv6). `curl` races the families and never notices; `ureq` tries the addresses in order, and the same `bevy_oracle` request that takes 0.44 s by IP took **56.9 s** by name, the whole 60 s timeout. `bevy_oracle::Http` now asks over IPv4 only (`a_name_is_resolved_to_its_ipv4_address_only`): 0.9 s by name. The firewall is not mine to change (needs root); opening `:8081` over IPv6 would make this unnecessary.
- **VRAM is tight, and it moved under the endpoint.** On 11-15 olmocr (12.7 GB), laya (1.5), the reranker (0.9) and distill (0.3) left 8 393 MiB and d3-flash took 7 708. On 11-16 olmocr held 13 698 and distill's embedder 5 430, leaving 3 424 free — the same script and the same model, refused. The lesson is that the free-check must be re-measured whenever home-still's own models grow, not that the number 8 192 was wrong. See "What broke it" above for the two fixes. With d3-flash resident distill's embedder cannot load: `ttl: 60` keeps that to a minute after the last decision.
- **Changing `llama-swap.yaml` costs olmocr its load.** llama-swap stops every model on a reload; the reaper loads olmocr again within a minute, about 60 s of no olmocr (scribe had no conversation in flight both times).
- **The first `hf download` ran at 1.4-7 MB/s on big and 36 MB/s on mac_air.** Ranged parallel `curl` brought 6.6 GB down in about 10 minutes.
