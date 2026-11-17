# Do agents ask, and does asking help? Two arms, measured (#457, DM-06)

**Status: the switch is built and tested, and both arms are running on big (started 13:24 UTC 2026-11-17); the runs
have not finished, so this file holds no measurement yet.**
Everything below that is a number was measured; everything that would be a result says where it will come from and
is not filled in. A reader should treat this file as **incomplete** until its "The four numbers" section holds
values read from `trial.json`.

The measurement this file is for: `ask-with-guide` against `ask-without-guide` on the 20 BM-01 dev tasks, 3 trials
each, omp on `anthropic/claude-sonnet-5-5`, decision model d3-flash (`where: local`), on big.

## How the two arms differ

**One argument, not a second copy of the guide.** `#457`'s decision is that the without arm is the guide minus
DM-05's section, and that two copies of the prose drift. So:

- `crates/bevy_cauldron_mcp/src/guide.md` is still the only copy of the guide, compiled in once.
- The server gained `--omit-guide-sections <heading,...>`, which cuts a named `##` section out of the shipped text
  **at serve time**. `--list-guide-sections` prints the headings it accepts; `--print-guide` prints the shipped
  layer as an arm serves it, so a run can name and hash what an agent was given.
- `bench/tasks/variants.yaml` states the two arms and they differ by one line:

  | arm | line that differs |
  |---|---|
  | `ask-with-guide` | `omit_guide_sections: []` |
  | `ask-without-guide` | `omit_guide_sections: [Check your work by asking]` |

- The arm reaches the MCP server the same way `--hide-tools` does: through the editor's own `grimoire/agent_setup`,
  as `server_args`. There is no second launch path.
- **`grimoire_decide` is offered in both arms.** The difference measured is what the guide says about the tool, not
  whether the tool exists — which is #412's `--hide-tools` question, a different one.

**A heading the guide has not is refused twice**: by the server at start (exit 2, naming the headings that exist)
and by `cauldron_bench::run` before any trial runs, so an arm that would remove nothing is one sentence rather than
60 harness errors that read as the editor's fault.

## What is proved, and by what

Run and green on the Mac, at `b3f215cf`:

| test | proves | result |
|---|---|---|
| `crates/bevy_cauldron_mcp/tests/the_guide_has_one_arm_switch.rs` | the with arm serves `guide.md` unchanged; the without arm is shorter by exactly the section's byte range and keeps each neighbour's text; the tool list is identical; a bad heading is refused at start; `--list-guide-sections` equals the file's own headings | 5 passed |
| `crates/bevy_cauldron_bench/tests/an_arm_omits_a_section_of_the_guide.rs` | the suite's two arms differ by one field; the section they name is one the guide has; an arm naming a heading the guide has not is refused before any trial | 3 passed |
| `cargo nextest run -p bevy_cauldron_mcp` | nothing else in the server moved | 109 passed |
| `cargo nextest run -p bevy_cauldron_bench` | nothing else in the runner moved | 170 passed, 9 skipped |

## Which binaries the run measures, and why that is the right pair

**The editor is the push gate's binary at `6cba1be5`; the MCP server and `cauldron-bench` are this branch's own
build** (2026-11-17 07:11 and 07:13, `CARGO_TARGET_DIR=~/.cargo-target-gate`). That is not a compromise:

- `git diff --name-only 6cba1be5 HEAD -- crates/bevy_cauldron/` is **empty on this branch**, so the gate's editor at
  `6cba1be5` is the editor at this commit, byte for byte. A cold build of it would have changed nothing the run reads.
- The server **is** this branch's, which is the point: it is the binary that carries `--omit-guide-sections`.
- `cauldron-bench` is this branch's, so the runner that passes the arm through `agent_setup` and refuses an unknown
  heading is the one measured.

Big was at ~274 MB free with the CI runners building when this decision was taken; four of six cold Bevy builds there
died. A measurement whose editor is provably the same file does not need a fifth attempt, and saying which binary ran
is worth more than a rebuild that cannot change the result.

## A void cell, recorded because it nearly became a result

**The first attempt started an arm against the previous editor binary.** The chain that gates the run on the
build checked only the MCP server (`--print-guide`, which only this commit has) and not `bevy_cauldron`, so the with
arm began at 13:19:16 UTC against a `bevy_cauldron` from 06:17 while the editor at this commit was still compiling.
It was stopped, and its directory was renamed `ask-two-arms-with-VOID-stale-editor` on big rather than deleted: of its
60 `trial.json` files, **59 are `harness_error`** — 58 "the run was interrupted", 1 "this trial was stopped", plus one
`pass`. **No figure in this file comes from it.**

The gating is now on all three binaries: the builder touches `~/.cargo-target-gate/debug/.askhelps-b3f215cf` only
after `cargo build` exits 0, and the chain refuses to start without it *and* without `bevy_cauldron` being newer
than the build's start. That is the fix for a defect this ticket found in its own harness, and it is written here
because a stale-binary run looks exactly like a real one in a report.

## The four numbers

**Not yet measured.** They come from `2026-11-17-ask-two-arms-analyse.py <with-run> <without-run>`, which reads
each trial's `trial.json` and `transcript.jsonl` and nothing else. The four are:

1. **asked share** — trials that called `grimoire_decide` with ≥4 questions after their last visible operation, over
   trials that applied at least one visible operation;
2. **mean questions per call**;
3. **success** with and without the guide, with the paired interval `cauldron-bench compare` gives;
4. **latency** — the `latency_ms` each answer reported, median and mean.

A run whose decision endpoint was down is **void, not a result** (`#457`'s rule): the script counts those cells and
names them, so they are re-run rather than scored.

## Cost, before spending

Printed before the run, from the 2026-11-15 run's own `cost_total_usd` ($10.2682 / 60 trials with the guide,
$10.0444 / 60 without): **≈ $20 for 3 trials × 20 tasks × 2 arms**, plus ~$0.03 per run for the pre-flight turn.
That is over the $8 ceiling, so **N = 3 and no more** — the assignment's own instruction for a run above $8.

## How to run it

```sh
ssh big 'bash -s' < docs/bench/2026-11-17-ask-two-arms-run.sh     # both arms, --jobs 1, d3-flash kept resident
python3 docs/bench/2026-11-17-ask-two-arms-analyse.py ask-two-arms-with ask-two-arms-without
```

`--jobs 1` is not a preference: each trial's editor draws on the card beside d3-flash, and `--jobs 2` OOM'd 56 of
60 trials on 2026-11-15 (`docs/bench/2026-11-15-ask-experiment.md`). The keep-alive in the script holds d3-flash
resident (ttl 60 s) and holds no GPU lease.

## Limits, stated before the numbers are in

- Three trials per task, one model, one decision model: the interval on success is wide enough that a few points
  either way is not a difference.
- Dev tasks only; the held-out set is not run here.
- The arms differ in the guide's served text and in nothing else **as built**; any effect of DM-05's other changes
  (the tool's description, the answer's `model`/`where`/`latency_ms`) is present in both arms and is not measured.
- Today's guide is DM-06's revision, not the guide the 2026-11-15 run measured: that run found the four-or-five
  demand was not followed, and the section was rewritten in response (`a07cc65c`, `f457de69`). So these numbers are
  about **the shipped guide as it now reads**, and are not a re-measurement of the earlier text.
