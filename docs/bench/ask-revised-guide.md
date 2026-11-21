# Do agents ask, and does asking help? Measured on today's main (#457, DM-06)

**Status: FINISHED — 60/60 trials, zero harness errors.** §2 has a result and gives its interval. The latency
section was corrected after review and is §4. Concluded:
§1 (the analyser reproduces the published figures it claims), §3 (arm A is not expressible on main), §4
(the decision model's own latency, corrected), §5 (the endpoint was down; two host faults, both fixed).

**Run:** 2026-11-20/21, big, `fleet/dm06`, omp 19.0.1 + `anthropic/claude-sonnet-5-5`, decision model
`d3-flash` (`where: local`), `--jobs 1`. Suite: the 20 BM-01 dev tasks x 3 trials. Guide served:
`sha256 ae87fc6b…` = `crates/dark_factory_mcp/src/guide.md` at `c763194e`, i.e. **main's revised guide**, not
the four-or-five text the 2026-11-15 run measured.

## 1. The analyser reproduces the published figures it claims

`docs/bench/analyse.py` run against the 2026-11-15 directories reproduces these, each read from
`docs/bench/2026-11-15-ask-experiment.md`:

| figure | published (2026-11-15) | this analyser |
|---|---|---|
| with-guide asked ≥4 of eligible | 22/43 = 51% | **22/43 = 51%** (Wilson 95% [37%, 65%]) |
| with-guide mean questions/call | 3.33 (1:1, 2:11, 3:5, 4:18, 5:4) | **3.33, same histogram** |
| with-guide trials that called it | 37/60, 39 calls | **37/60, 39 calls** |
| with-guide answered / latency | 38; median 543 ms | **38; median 543, mean 538, p90 679, max 753** |
| with-guide success | 55/60 = 92% | **55/60 = 92%** |
| without-guide asked ≥4 | 0/44 | **0/44 = 0%** |
| without-guide mean questions/call | 0.55 | **0.55** (histogram {0:24, 1:3, 2:3, 3:3}) |
| without-guide trials that called it | 20/60, 33 calls | **20/60, 33 calls** |
| without-guide answered / latency | 5; median 503 ms | **5; median 503, mean 562** |
| without-guide success | 57/60 = 95% | **57/60 = 95%** |

**Not claimed:** the rows the published file derives rather than states — "asked ≥1" (81% / 11%) is printed
here but the 2026-11-15 document gives no such figure for both arms, so it is not a reproduction. Two defects
found in review were fixed to reach the table above: question counts carried as an **int** by the `eval` route
were being dropped (mean read 3.26 / 0.39 against the published 3.33 / 0.55), and a `questions` **list** was
counted as zero where the published `nq()` counts it.

Run it:

```sh
BENCH_ROOT=$HOME/.local/state/cauldron-bench python3 docs/bench/analyse.py ask-with-guide-final
BENCH_ROOT=$HOME/.local/state/cauldron-bench python3 docs/bench/analyse.py ask-without-guide-final
```

`BENCH_ROOT` is needed because those runs were written by the pre-rebrand binary under `cauldron-bench`; the
era's tool prefix (`mcp__cauldron_grimoire_` against today's `mcp__dark_factory_`) follows from that root, and
`TOOL_PREFIX` overrides it.

## 2. Do agents ask, on the revised guide? — FINISHED, 60/60 trials

| measure | 2026-11-15 (four-or-five guide) | today's revised guide |
|---|---|---|
| trials / harness errors | 60, 0 | **60, 0** |
| eligible trials (a visible operation applied) | 43 | **43** |
| asked ≥1 after the last visible operation | — (not published) | **36/43 = 84%** (Wilson 95% [70%, 92%]) |
| **asked ≥4 (the old bar)** | **22/43 = 51%** | **3/43 = 7%** (Wilson 95% [2.4%, 18.6%]) |
| mean questions per call | 3.33 | **2.24** (histogram {1:9, 2:23, 3:15, 4:3}) |
| trials that called it | 37/60 = 62% | **40/60 = 67%**, 50 calls |
| success | 55/60 = 92% | **56/60 = 93%** (Wilson [84%, 97%]; the bench's own report says 93% [83%, 100%]) |
| latency of an answer (`latency_ms`) | median 543 ms | **median 965 ms** (n=48) |

### The finding: the revision moved how many questions agents ask, and the interval excludes 51%

**3/43 = 7%, Wilson 95% [2.4%, 18.6%]**, against the 2026-11-15 figure of 51%. The upper bound is **18.6%**, so
the interval excludes 51% by 32 points. On the same 20 tasks, the same agent and model and the same
`--jobs 1`, an agent given the revised guide asks four or more questions **far less often** than one given the
text that demanded four or five — which is exactly what the revision set out to do: the count demand was
removed and the guide now asks only for questions whose answer would change the next operation.

**And they still ask.** 84% of eligible trials ask at least one question (36/43 [70%, 92%]), and the mean is
2.24 questions per call against 3.33. So the effect is **narrower and shallower questioning, not less
asking**: the agent kept using the check and stopped padding it.

**This run cannot say whether asking helps.** The issue's second question — *does asking help?* — is not
answered here: §3 explains why arm A is not expressible on today's main, and success moving 92% → 93% is not a
difference at this n. What this section measures is only *how many questions agents ask*.

**What this does not say.** It does not say asking *helped* — §3 is untouched by it, and success moved 92% →
93%: three trials, and the run's own report puts `pass^k` at 1:0.93, 2:0.88, 3:0.85. That is not a difference
at this n, and the two runs' success intervals overlap almost completely. The two runs also differ in the guide's text only as built; any
effect of DM-05's other changes is in both. And a comparison of 51% to 7% is a comparison of **two different
instructions**, which is the point: the revision changed what the text asks for, and the text was obeyed.

### The two void cells, reported rather than removed

2 of the 50 decision calls failed at the endpoint (`status 500 … upstream command exited prematurely`) in
`raise-a-hill-in-the-north-east` trials 2 and 3. Under this issue's rule a run whose endpoint was down is void,
so those cells are named here: **both trials still passed**, the agent carried on after the refusal (which is
advisory), and neither is one of the 4 failures (`count-what-the-bedroom-holds` ×2, `hedge-along-the-south-edge`,
`turn-the-wardrobe-into-the-room`). 48 of 50 calls answered.

**If that 500 recurs, check the launcher's `olmocr` precondition first.** It is the same fault class as the two
host bugs in §5 and it produces this exact string: `run-d3-flash.sh` exits 75 unless `olmocr` is already `ready`,
so any llama-swap restart — or the reaper moving the card — turns d3-flash into a 500 in 0.25 s that reads as a
model that cannot load. §5 has the reproduction and the fix.

## 3. Does asking help? (arm A vs arm B) — NOT EXPRESSIBLE ON TODAY'S MAIN

The shipped guide is `include_str!("guide.md")` compiled into the MCP server (`docs.rs:627`). `blueprint_guide`
*appends* the person's and the project's layers, so a layer can only **add** prose. The bench offers two arm
mechanisms and neither subtracts: `hide_tools` removes a **tool** (confounding guide with tool), and `overlay`
lays project **files** over a trial's copy (adding a project layer).

**Evidence, at `c763194e`, each command returning 0:**

```sh
git grep -nE 'omit[_-]guide' c763194e -- crates/ docs/ | wc -l          # 0 -- no such switch
git show c763194e:bench/tasks/variants.yaml | grep -cE 'omit_guide'      # 0 -- no such field
git grep -nE '\-\-omit-guide|--print-guide' c763194e -- crates/ | wc -l  # 0 -- no such flag
```

(An earlier version of this file cited `grep -rn 'omit.guide|omit_guide' crates/ docs/`, which is a **basic**
regex where `|` is literal — it finds nothing whatever the code says, so it proved nothing. The three
`git grep` commands above are the proof.)

**The smallest change that makes arm A runnable** — one field, one flag, one helper, no second bench:

1. `docs.rs`: `SECTION_PREFIX`, `sections(text)`, `section_headings()`, `guide_without(&[String])` — cut a named
   `##` section out of the compiled-in `GUIDE` by its own byte range.
2. `main.rs`: an `omitted_guide_sections` field, `--omit-guide-sections <heading,...>`, plus `--print-guide` /
   `--list-guide-sections` so a run can name and hash what an arm served. A heading the guide has not is
   refused twice (at start, and in `run()` before any trial).
3. `suite.rs`: `Variant.omit_guide_sections`, through `run.rs` into `server_args` beside `--hide-tools` (the
   path `--hide-tools` already takes through `blueprint/agent_setup`, so no second launch path).
4. `variants.yaml`: two arms differing by one line.

`blueprint_decide` stays served in **both** arms: the difference measured is what the guide *says*. This is
`fleet/ask-helps` at `1de35adb`, which predates the rebrand and is being rebased by the main agent; **this
ticket did not port it and built no parallel bench.**

The 2026-11-15 pair already says 92% against 95% on 60 trials each, paired interval [−11.7, +3.3] points — it
contains zero.

## 4. The decision model's own latency — corrected

**What `latency_ms` actually measures**: `decide.rs` wraps only `Transport::send` in the timer (`:480-483`)
and says so (`:448-449`: "the sum of its answers' round trips, so the pictures' encoding is not counted").
**It is decision-model time. It is not what an agent waits.**

| the call | median | n | how |
|---|---|---|---|
| the model, 1 question, text only | **36 ms** | 5 | varied `state` per call |
| the model, 1 question, with a 27 kB still | **76 ms** | 5 | " |
| the model, 3 questions, text only | **107 ms** | 5 | " |
| the model, 3 questions, with the still | **203 ms** | 5 | " |
| the model, 4 questions, text only | **150 ms** | 5 | " |
| the model, 4 questions, with the still | **261 ms** | 5 | " |
| the model, 8 questions, text only | **320 ms** | 5 | " |
| the model, 8 questions, with the still | **597 ms** | 5 | " |
| **`blueprint_decide`, `view: true`, 4-ish questions, in a live run** | **965 ms** | 48 | the run's own `latency_ms` |

The repo's own fixture re-measured the same way: **3 questions + its 164 kB image, median 613 ms** (samples
1321, 519, 613, 528, 632) while the benchmark arm was running, against the 397 ms
`docs/decide/serving-d3-flash.md` recorded on an idle card.

**Those are two measurements of one thing on different loads, and they are not averaged or reconciled.** 613 ms
is what the fixture costs with a trial's editor and a keep-alive sharing the card; 397 ms is what it costs idle.
The gap is **contention** — the other tenant on the 3090 — and it is the honest reason the shipped wording is
rounded rather than a millisecond promise. A blended number would hide the one fact worth having: the same
request can cost 1.5× more when the card is busy, so any latency a reader plans around should be the busy one.

**An earlier version of this section was wrong and is withdrawn.** It reported 151 ms and 279 ms for four
questions with no derivation, and said "the editor taking the still is the rest" — a claim nobody measured.
The 151 ms was also **a prompt-cache artifact**: it sent one identical body 25 times, and `llama-server` caches
prompts, so after the first call the number measured the cache. Every figure above varies `state` per call.
`probe_latency.py` in this directory is the script; it takes the still and the port.

**What the shipped text now says**, and why: `blueprint_decide`'s description carries the model's own round
trips and says the rest is unmeasured. It does **not** name the capture as the cost, because that was never
measured. **What an agent waits includes the editor's capture, which is not measured here.**

**A cold call is seconds**: 5.34 s for the first request after llama-swap unloaded the model (that is `ttl`;
the benchmark's keep-alive exists for it).

## 5. The endpoint was down — two host faults, both fixed and recorded

Every request answered `500 {"error":"unspecific error: upstream command exited prematurely"}` in 0.25 s.

1. **`PATH`.** `llama-swap`'s systemd user unit inherits the user manager's `PATH`
   (`/usr/local/bin:/usr/bin`). `~/.home-still/run-d3-flash.sh` calls `gpu-tenant` (`~/.local/bin/gpu-tenant`),
   so the launcher was `command not found` at line 50 and exited 127 in ~0.1 s. Reproduced with
   `env -i HOME=… PATH=/usr/local/bin:/usr/bin bash run-d3-flash.sh`; the same script from a login shell loads
   in 13 s. Fixed with an **additive drop-in**, `~/.config/systemd/user/llama-swap.service.d/10-path.conf`,
   putting `~/.local/bin` first; reverting is deleting one file.
2. **The `olmocr` precondition.** Even with a correct `PATH` the launcher exits 75 unless `olmocr` is `ready`,
   and a **restarted llama-swap has no model resident** — so the first request after *any* restart fails
   identically. Warm `olmocr` once (~39 s cold; `ttl: 0` then keeps it).

Both are in the launcher's own comments, because the two faults produce the identical 500.

**Proof the endpoint works** (a `/models` 200 alone is not proof):

```
GET  /v1/models                             -> 200, lists d3-flash
POST /upstream/d3-flash/v1/systemone (cold) -> 200 in 5.34 s
POST … (warm, varied bodies)                -> 200; the table in §4
```

## Reproducing

```sh
cd /home/ladvien/worktrees/bevy_dark_factory-dm06
CARGO_TARGET_DIR=$HOME/.cargo-target-fleet/dm06 \
  flock -s /tmp/fleet/big-heavy.lock \
  cargo build -p bevy_dark_factory -p dark_factory_mcp -p bevy_dark_factory_bench   # ~11 min

bash docs/bench/run-ask-revised.sh                                  # ~85 s/trial, ~85 min for 60
python3 docs/bench/analyse.py ask-revised-guide                     # the arm
python3 docs/bench/probe_latency.py <still.png> 8081                # the model's own latency
```

**A footnote on the run script, because a reader will find it in the log.** `run-ask-revised.sh` printed
`syntax error near unexpected token` after the run finished. The cause is that it was **edited while bash was
executing it**, so bash resumed reading at a byte offset into text that had moved. **The run is unaffected**, and
the evidence is better than this paragraph: 60 trials were written, the harness's own `report.md` exists, and the
guide hash `ae87fc6b…` is in `run.json`, written by the binary rather than by the script. What was lost is one
redundant step after everything else — a second copy of a hash `run.json` already carried. No number in this file
depends on it.

**Limits:** three trials a task; one model; one decision model; dev tasks only; the guide measured is today's
revised text, so the ≥4 row compares *different instructions*, not a like-for-like improvement; **§2's finding is about *how many questions agents ask*, not about whether asking helps**; and **the editor's capture is unmeasured**, so no figure here is the latency an agent
experiences end to end.
