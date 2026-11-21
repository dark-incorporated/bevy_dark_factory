# Do agents ask, and does asking help? Measured on today's main (#457, DM-06)

**Status: PARTIAL.** The arm is **running** (6 of 60 trials at the time of writing, no harness errors) and
continues in the background; the numbers below are from those 6 and are **not a conclusion**. What *is*
concluded is §1 (the analyser is trustworthy — it reproduces the published 2026-11-15 figures exactly), §3
(arm A cannot be expressed on today's main, with the smallest change that fixes it), §4 (latency, measured)
and §5 (the decision endpoint was down; two host faults, both fixed and recorded).

**Run:** 2026-11-20/21, big, `fleet/dm06` at `c763194e` (+ the macro edit), omp 19.0.1 +
`anthropic/claude-sonnet-5-5`, decision model `d3-flash` (`where: local`), `--jobs 1`. Suite: the 20 BM-01 dev
tasks x 3 trials. Guide served: `sha256 ae87fc6b…`, which is `crates/dark_factory_mcp/src/guide.md` **on
main today** — the revised text, not the four-or-five text the 2026-11-15 run measured.

## 1. The analyser reproduces the published numbers first

Before trusting any new number, `docs/bench/analyse.py` was run against the **2026-11-15** run directories and
checked against `docs/bench/2026-11-15-ask-experiment.md`. It reproduces **every** published figure:

| figure | published (2026-11-15) | this analyser, re-run |
|---|---|---|
| with-guide asked ≥4 of eligible | 22/43 = 51% | **22/43 = 51%** (Wilson 95% [37%, 65%]) |
| with-guide asked ≥1 | 35/43 | **35/43 = 81%** |
| with-guide mean questions/call | 3.33 (1:1, 2:11, 3:5, 4:18, 5:4) | **3.33, same histogram** |
| with-guide trials that called it | 37/60, 39 calls | **37/60, 39 calls** |
| with-guide answered / latency | 38; median 543 ms | **38; median 543, mean 538, p90 679, max 753** |
| with-guide success | 55/60 = 92% | **55/60 = 92%** |
| without-guide asked ≥4 | 0/44 | **0/44 = 0%** |
| without-guide asked ≥1 | 5/44 | **5/44 = 11%** |
| without-guide trials that called it | 20/60, 33 calls | **20/60, 33 calls** |
| without-guide answered / latency | 5; median 503 ms | **5; median 503, mean 562** |
| without-guide success | 57/60 = 95% | **57/60 = 95%** |

That is the control on the instrument, and it needed two fixes to get there, both recorded in the script: it
reads the transcript for `latency_ms` (trial.json's `tool_calls` is a list of names), and it normalises the
**rebrand** — the old runs route tools as `mcp__cauldron_grimoire_decide`, today's as
`mcp__dark_factory_blueprint_decide`, so both the route prefix and the `blueprint_` prefix are stripped before
anything compares.

## 2. Do agents ask, on the revised guide? — PARTIAL, 17/60 trials

| measure | 2026-11-15 (the four-or-five guide) | today's revised guide, 6 trials |
|---|---|---|
| eligible trials | 43 | 13 |
| asked ≥1 after the last visible operation | 35/43 = 81% | **13/13 = 100%** (Wilson 95% [77%, 100%]) |
| asked ≥4 (the old bar) | 22/43 = 51% | **2/13 = 15%** (Wilson 95% [4%, 42%]) |
| mean questions per call | 3.33 | **2.53** (histogram {1:3, 2:4, 3:8, 4:2}) |
| trials that called it | 37/60 = 62% | **14/17 = 82%**, 17 calls |
| success | 55/60 = 92% | **15/17 = 88%** |
| latency of an answer | median 543 ms | **median 965 ms** (n=17) |

**This is not evidence of an improvement, and must not be reported as one.** n = 13 eligible, and every number above moved when 4 more trials landed — which is the whole point of saying it now rather than after 60. The wide intervals overlap
the old figures entirely; the ≥4 drop from 51% to 33% is what the revision *intends* (the count demand is
gone), but at this n it is a direction, not a finding. When the run finishes the same table with n ≈ 43–60
eligible trials on each side will, and only then, say whether the revision moved anything.

## 3. Does asking help? (arm A vs arm B) — NOT EXPRESSIBLE ON TODAY'S MAIN

**The claim to test:** an agent given the DM-05 guide produces better work than the same agent without it.

**Why main cannot run arm A.** The shipped guide is `include_str!("guide.md")` compiled into the MCP server
binary (`crates/dark_factory_mcp/src/docs.rs:627`). Main serves exactly that text and no argument serves a
different one (`grep -rn 'omit.guide|omit_guide' crates/ docs/` on main returns nothing). `blueprint_guide`
*appends* the person's and the project's layers, so a layer can only **add** prose. The bench's `variants.yaml`
offers two arm mechanisms and neither subtracts:

- `hide_tools` removes a **tool**, not a section — it would confound "no guide" with "no tool".
- `overlay` lays project **files** over a trial's copy, which for a guide means adding a project layer.

**The smallest change that makes it runnable** — one field, one flag, one helper, no second bench:

1. `docs.rs`: `SECTION_PREFIX`, `sections(text)`, `section_headings()`, `guide_without(&[String])` — cut a named
   `##` section out of the compiled-in `GUIDE` by its own byte range.
2. `main.rs`: an `omitted_guide_sections` field, `--omit-guide-sections <heading,...>`, plus `--print-guide` /
   `--list-guide-sections` so a run can name and hash what an arm served. A heading the guide has not is
   refused twice (at start, and in `run()` before any trial) — an arm that removed nothing must not report
   that it ran.
3. `suite.rs`: `Variant.omit_guide_sections`, passed through `run.rs` into `server_args` beside `--hide-tools`
   (the path `--hide-tools` already takes through `blueprint/agent_setup`, so no second launch path).
4. `variants.yaml`: two arms differing by one line — `omit_guide_sections: []` vs
   `omit_guide_sections: [Check your work by asking]`.

`blueprint_decide` stays served in **both** arms: the measured difference is what the guide *says*, not whether
the tool exists (that is #412's separate question). The heading exists on main today
(`crates/dark_factory_mcp/src/guide.md:55`), so no prose change is needed either.

This is exactly `fleet/ask-helps` at `1de35adb` (12 files, +1267/−30), which predates the
`bevy_cauldron_*` → `dark_factory_*` rebrand and is being rebased by the main agent. **This ticket did not port
it and did not build a parallel bench.**

**What the 2026-11-15 pair already says**, and what a finished arm A/B would have to overturn: 92% with the
guide against 95% without, 60 trials each, paired interval [−11.7, +3.3] points — which contains zero. The
shippable claim from that pair is *"asking did not measurably change success on 20 dev tasks, 3 trials each;
the interval rules out an improvement larger than about 3 points and is too wide to rule out a small one."*

## 4. How fast does the decision model answer? — measured, three figures

Against `http://127.0.0.1:8081/upstream/d3-flash/v1/systemone`, four `noul` questions every time:

| the call | median | n |
|---|---|---|
| the model alone, text only, no picture | **151 ms** | 25 |
| the model alone, one 27 kB still attached | **279 ms** | 25 |
| `blueprint_decide` as an agent actually calls it (`view: true`: the editor also takes the still) | **1200 ms** | 11, live run |

**The third is the number that matters** — it is what an agent waits for. The 2026-11-15 benchmark found
**543 ms** for the same shape, so the capture cost varies with what is on screen; `blueprint_decide`'s
description now states the honest rounded figure, *"about a second and a half (the model itself answers in
150-280 ms; the editor taking the still is the rest)"*, replacing *"in about a second"*. **A cold call is
seconds**: 5.34 s for the first request after llama-swap unloaded the model (that is `ttl`, and the keep-alive
exists for it).

## 5. The decision endpoint was down — two host faults, both fixed and recorded

The issue's rule (*a run whose endpoint was down is void*) applied at the start: every request answered
`500 {"error":"unspecific error: upstream command exited prematurely"}` in 0.25 s.

1. **`PATH`.** `llama-swap`'s systemd user unit inherits the user manager's `PATH`
   (`/usr/local/bin:/usr/bin`). `~/.home-still/run-d3-flash.sh` calls `gpu-tenant`
   (`~/.local/bin/gpu-tenant`), so the launcher was `command not found` at line 50 and exited 127 in ~0.1 s.
   Reproduced with `env -i HOME=… PATH=/usr/local/bin:/usr/bin bash run-d3-flash.sh`; the same script from a
   login shell loads the model in 13 s. Fixed with an **additive drop-in**,
   `~/.config/systemd/user/llama-swap.service.d/10-path.conf`, putting `~/.local/bin` **first**; reverting is
   deleting one file and the packaged unit is untouched.
2. **The `olmocr` precondition.** Even with a correct `PATH`, the launcher exits 75 unless `olmocr` is already
   `ready`. A **restarted llama-swap has no model resident**, so the first `d3-flash` request after *any*
   restart fails identically. Warm `olmocr` once (~39 s cold, then `ttl: 0` keeps it).

Both are now written into the launcher's own comments, because the two faults produce the identical 500 and
look like a model that cannot load.

**Proof the endpoint works** (a `/models` 200 alone is not proof):

```
GET  /v1/models                             -> 200, lists d3-flash
POST /upstream/d3-flash/v1/systemone (cold) -> 200 in 5.34 s
   {"model":"d3-flash","answers":{"there":{"type":"noul","noul":0.9932},"clear":{"noul":0.9001}}}
POST … (warm)                               -> 200; median 151 / 279 ms by shape
```

## Reproducing

```sh
cd /home/ladvien/worktrees/bevy_dark_factory-dm06
CARGO_TARGET_DIR=$HOME/.cargo-target-fleet/dm06 \
  flock -s /tmp/fleet/big-heavy.lock \
  cargo build -p bevy_dark_factory -p dark_factory_mcp -p bevy_dark_factory_bench   # ~11 min, editor dominates

# warm olmocr once (a llama-swap restart leaves nothing resident), then the arm
bash docs/bench/run-ask-revised.sh                      # ~85 s/trial, ~85 min for 60

# read it (the prefix names which era's tool routing the transcripts use)
TOOL_PREFIX=mcp__dark_factory_ python3 docs/bench/analyse.py ask-revised-guide
```

**Limits, stated before the numbers:** three trials a task; one model; one decision model; dev tasks only; the
guide measured is today's revised text, so the `≥4` row compares *different instructions*, not a like-for-like
improvement; and **§2 is n = 6 and says nothing yet.**
