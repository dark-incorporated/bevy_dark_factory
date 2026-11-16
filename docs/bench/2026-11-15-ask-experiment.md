# Do agents ask, and does asking help? (DM-06, #457)

Measured on the benchmark (BM-04's 20 dev tasks) at main `c21f8ff35aeea835a3555ba49fd26365f58f140b`, with the local decision model, on big. The two reports are `runs/2026-11-15-ask-with-guide.{md,json}` and `runs/2026-11-16-ask-without-guide.{md,json}`; the numbers below come from them and from `2026-11-15-ask-experiment-analyse.py`, which reads each trial's transcript (`python3 docs/bench/2026-11-15-ask-experiment-analyse.py` on the run directories).

## The answer

- **Do they ask? Half do, and not as the guide says.** With DM-05's guide, an agent called `grimoire_decide` with four or more questions after its last visible operation in **22 of 43** eligible trials (**51%**, Wilson 95% interval 37% to 65%). It called it with at least one question in 35 of 43 (81%). The target was 80% at four or more, so **the target is missed**. Without the section, the share was 0 of 44 (0% to 8%).
- **Does it help? Not measurably.** Task success was 55 of 60 trials with the guide (92%) and 57 of 60 without it (95%). `cauldron-bench compare` says **no measurable change**: the with-guide arm is 3.3 points lower, paired 95% bootstrap interval [-11.7, +3.3] over the 20 tasks. Cost per success was $0.1867 with and $0.1762 without (+6%); the arms cost $10.27 and $10.04.
- **What asking changed: nothing the benchmark can see.** In the 14 with-guide trials whose first answer held a probability under 0.5, **none** applied another operation afterwards (the guide says to fix and ask again); all 14 passed anyway. Over all 37 trials with an answered decision, 2 applied a further operation, 1 asked a second time. An agent that asked scored 22 of 22 and one that did not 20 of 21 (a difference of nothing, since the tasks are ones the agent already does).

## What was run

- **Arm with the guide:** the shipped MCP server of `c21f8ff3`. **Arm without:** the same tree, with the one file `crates/bevy_cauldron_mcp/src/guide.md` taken from `322634b8^` (the commit before DM-05), so the guide has no "Check your work by asking" section. Both servers were built here with `cargo build -p bevy_cauldron_mcp` (dev profile), the editor is the gate's `bevy_cauldron` of `c21f8ff3`, byte-identical in both arms (`cmp`), and each arm's editor sits beside its server (the editor hands the agent the server beside itself). **The tool's own description and the server's instructions are DM-05's in both arms**: only the guide differs. The report header of the without arm therefore names the with arm's guide hash; this file is the record of the difference.
- **Agent and model:** `omp` 18.18.9, `anthropic/claude-sonnet-5-5`. **Decision model:** d3-flash, `where: local`, endpoint `http://big.local:8081/upstream/d3-flash/v1/systemone`, reported by the endpoint as model `d3-flash`. Dev suite only (the held-out tasks are out of this measurement): 20 tasks, 3 trials each, per arm.
- **`--jobs 1`, and why.** The first attempt used `--jobs 2`: with d3-flash resident (7.7 GB beside olmocr, laya, the reranker and distill) big's card has about 1.1 GB free, and every trial editor draws on the NVIDIA GPU (about 490 MiB each; big has no software Vulkan), so the second editor ran out of memory (`Caught rendering error: Out of Memory`; 56 of 60 trials were harness errors). That run was discarded. `--jobs 1` fits. A background keep-alive request every 20 s held d3-flash resident (its `ttl` is 60 s and there is no room to load it beside the editors); it held no lease.
- **Voided cells, re-run.** d3-flash was unreachable for about 19 minutes during the first with-arm run (17:54 to 18:13; `status 500 ... upstream command exited prematurely`: big's memory was near wedging and olmocr reloaded) and for two trials of the without arm (19:07). Under the issue's rule (a run whose decision endpoint was down is void) every cell with such a call was run again: 7 tasks times 3 trials for the with arm (`cover-the-hallway-floor`, `fix-the-floating-lamp`, `furnish-a-reading-nook`, `hedge-along-the-south-edge`, `lamp-on-the-bedside-table`, `make-a-cellar-map`, `pond-in-the-lowest-corner`) and `turn-the-wardrobe-into-the-room` for the without arm. **Each published report is the first run with those cells replaced by the re-run**; no cell was re-run because of its result. In the published runs **no decision call failed for the endpoint** (38 answered with the guide, 5 without). The raw first runs stay on big under `~/.local/state/cauldron-bench/ask-*-guide`.
- **Spend:** about $27 in all by the runner's listed prices: the two first runs $20.6, the re-runs $4.6, the discarded `--jobs 2` attempt about $1.7.

## The decision calls

| | with the guide | without it |
|---|---|---|
| decision calls | 39 (all `view: true`) | 33 (5 with `view: true`) |
| of which answered | 38 | 5 |
| calls refused for bad arguments | 1 (an `eval` script error, not the tool) | 28 (a named `check` with no capture, `questions` as an array, no `check` or `questions`) |
| mean questions per call | 3.33 (1:1, 2:11, 3:5, 4:18, 5:4) | 0.55 |
| latency of an answer (the call's `latency_ms`) | median 543 ms, mean 538, p90 679, max 753 | median 503 ms (5 answers) |
| `where` and `model` | `local`, `d3-flash` (38 of 38) | `local`, `d3-flash` (5 of 5) |
| trials that called it at all | 37 of 60 | 20 of 60 |

Without the section 20 trials still reached for `grimoire_decide`, because its description leads with its use; 28 of those 33 calls were refused, because the tool's grammar is taught by the section. That is the section's measured effect: **it made the calls well formed** (38 of 39 answered against 5 of 33).

## Eligible trials and the misses

An eligible trial applied at least one operation (`grimoire_apply_operation`, not refused); 43 of 60 with the guide and 44 of 60 without did. Asked is: a `grimoire_decide` call with four or more questions after the last such operation. The 21 eligible trials with the guide that did not ask at four or more are of three kinds: **13 asked with two or three questions** after their last operation (`cover-the-hallway-floor` 2, `fix-the-floating-lamp` 2, `make-a-cellar-map` 3, `pond-in-the-lowest-corner` 3, `turn-the-wardrobe-into-the-room` 3; 17 of the 39 calls had fewer than four questions), **1 asked only before** its last operation (`work-the-open-note`), and **7 never called the tool** (`set-the-gates-health` x3, which changes data and shows nothing; `clear-the-overlap-keep-both` x2; `work-the-open-note` x2). Excluding the data task, the shares are 22 of 40 (55%) at four or more questions and 35 of 40 (88%) at one or more.

| task | with: trials, passed, eligible, asked | without: trials, passed, eligible, asked |
|---|---|---|
| `bed-against-the-north-wall` | 3 3 3 3 | 3 3 3 0 |
| `chair-west-of-the-desk-facing-it` | 3 3 3 3 | 3 3 3 0 |
| `clear-the-overlap-keep-both` | 3 3 3 1 | 3 3 3 0 |
| `count-what-the-bedroom-holds` | 3 0 0 0 | 3 1 0 0 |
| `cover-the-hallway-floor` | 3 3 3 1 | 3 3 3 0 |
| `fix-the-floating-lamp` | 3 3 2 0 | 3 3 3 0 |
| `furnish-a-reading-nook` | 3 3 3 3 | 3 3 3 0 |
| `hedge-along-the-south-edge` | 3 2 2 2 | 3 2 2 0 |
| `is-the-lamp-on-the-table` | 3 3 0 0 | 3 3 0 0 |
| `lamp-on-the-bedside-table` | 3 3 3 3 | 3 3 3 0 |
| `make-a-cellar-map` | 3 3 3 0 | 3 3 3 0 |
| `move-the-locked-bed` | 3 3 0 0 | 3 3 0 0 |
| `pond-in-the-lowest-corner` | 3 3 3 0 | 3 3 3 0 |
| `raise-a-hill-in-the-north-east` | 3 3 3 3 | 3 3 3 0 |
| `refuse-a-piece-the-project-lacks` | 3 3 0 0 | 3 3 0 0 |
| `remove-both-crates` | 3 3 3 3 | 3 3 3 0 |
| `set-the-gates-health` | 3 3 3 0 | 3 3 3 0 |
| `take-back-the-last-change` | 3 3 0 0 | 3 3 0 0 |
| `turn-the-wardrobe-into-the-room` | 3 2 3 0 | 3 3 3 0 |
| `work-the-open-note` | 3 3 3 0 | 3 3 3 0 |

## What this says for the guide

The issue's rule applies: below 80% of eligible tasks the miss is reported and the guide is revised before the series is called done. The transcripts are on big: `~/.local/state/cauldron-bench/ask-with-guide-final/<task>/trial-<n>/transcript.jsonl`. Three things to revise in the section, each from the table above and not from taste: the section says four or five questions and agents send two or three (17 of 39 calls); it says to fix and ask again, and in 14 trials with a first answer under 0.5 nobody did; and it says to ask after every visible change, and 7 trials (a data edit, an overlap fix, a note reply) never asked. The 14 low answers also deserve a look from the model's side before the guide is blamed: on `lamp-on-the-bedside-table` and `pond-in-the-lowest-corner` the lowest answer of the first call read 0.09 to 0.13 on every trial, on trials that passed the grader, so d3-flash may be answering a different question than the task checks.

## Limits

- Three trials per task, 20 tasks, one model, one decision model: the interval on success is wide, and a 3-point difference is inside it. Only the dev tasks were run.
- Two runs on the same day are one draw each; the first baseline of this suite moved by one trial between two runs of the same code.
- The without arm differs from the with arm in the guide text only, as built; any effect of DM-05's other changes (the description that leads with the use, the answer's `model`, `where` and `latency_ms`) is in both arms and is not measured here.
- `docs/bench/runs/` now holds two reports at 3 trials with no held-out total at a commit whose `crates/` tree a later release may share: `bench_gate.py` would pick the newer of them as the release's report and refuse it (trials is not 5 and there is no held-out total). A release at this `crates/` tree must carry a baseline-style report dated after them, or these two must move out of `runs/`.
