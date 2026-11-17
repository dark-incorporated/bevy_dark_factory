# An omp run, and a matrix of two models, for real (#416)

Measured on big at main `6b5162eab2502a9d1d71b121c156e65bb3bc02b3`, with omp 18.21.0 and omp's own
Anthropic provider (the owner's decision, 2026-11-16: omp with the `anthropic` provider, not the expired
`claude` CLI login). The editor and the MCP server are the binaries built for the push gate at that commit
(`~/.cargo-target-gate/debug/bevy_dark_factory`, `dark_factory_mcp`), and the runner is the gate's
`dark_factory-bench` of the same build.

## Acceptance 1: a real omp run writes trials with tokens and calls from the transcript

`dark_factory-bench run --agent omp --model anthropic/claude-sonnet-5-5 --trials 1 --run omp-real` on the three
BM-01 example tasks (`bed-against-the-north-wall`, `refuse-a-piece-the-project-lacks`,
`count-what-the-bedroom-holds`), `--jobs 1`. The report is `runs/2026-11-17-omp-real.{md,json}`.

| task | outcome | tool calls | tokens in/out | cache read/write | cost | wall s |
|---|---|---|---|---|---|---|
| `bed-against-the-north-wall` | pass 6/6 | 8 | 22 / 3520 | 347134 / 51197 | $0.1979 | 39 |
| `refuse-a-piece-the-project-lacks` | pass 4/4 | 5 | 16 / 1763 | 251468 / 45698 | $0.1571 | 24 |
| `count-what-the-bedroom-holds` | **fail** 3/4 (`handback_contains`) | 3 | 14 / 1489 | 131625 / 20393 | $0.0791 | 19 |

3 trials, **2 passed, 1 failed, 0 harness errors**. The suite cost $0.4341, and the pre-flight turn $0.0268
(4 fresh in, 4 out, 10686 cache written), so the run's total was **$0.4608**.

**Where each number comes from.** `tokens_in` and `tokens_out` are summed from the `usage` objects of the
`message_end` events whose `message.role` is `assistant`, in each trial's own `transcript.jsonl` (written by
`omp -p --mode json`). `tool_calls` is every `tool_execution_end` in the same transcript. The tool list the
agent was offered is the `tools-offered.jsonl` sidecar the trial's omp extension writes, because omp's stream
carries no tool list of its own. The trial records that were checked hold `tokens_in` = 22/14/16 and
`tokens_out` = 3520/1489/1763, non-null in all three.

**The one failure is the agent's, and it is the read task.** The prompt asks for the count and the piece
nearest the north wall in a `blueprint_present` summary; the trial made 3 calls and its hand-back did not
contain `three` and `dresser`. No harness error was recorded, and nothing was substituted.

## Acceptance 2: a matrix of two pairs writes one report, and refuses to mix commits

`dark_factory-bench matrix --pairs omp:anthropic/claude-sonnet-5-5,omp:anthropic/claude-haiku-4-5 --trials 1`
on the same three tasks wrote one report, `matrices/2026-11-17-bench-omp.{md,json}`:

| pair | mean success (95% interval) | mean pass^k | mean wall s | mean calls | cost per trial | cost per success | frontier |
|---|---|---|---|---|---|---|---|
| `omp:anthropic/claude-sonnet-5-5` | 100% [100%, 100%] | 1:1.00 | 29 | 5.7 | $0.1410 | $0.1410 | **on the frontier** |
| `omp:anthropic/claude-haiku-4-5` | 33% [0%, 100%] | 1:0.33 | 43 | 6.3 | $0.0725 | $0.2176 | **on the frontier** |

Both are placed: sonnet is the most successful and haiku the cheapest, so neither dominates the other
(Kapoor et al., `10.48550/arXiv.2407.01502`). The pairs are `omp` twice over, because the issue's other
harness (Claude Code) cannot be run on this host — see below.

**The refusal was measured, not assumed.** A first matrix run under `--label` had pair 1 at
`6b5162ea` and pair 2 at `6b5162ea+dirty`: `run --label` publishes into `docs/bench/README.md`, so the
first pair's own publish dirtied the checkout the second pair's header then read. `Matrix::build` refused
with

> `dark_factory-bench: the matrix compares pairs on the same tasks, trials and editor commit, so the only
> difference is the agent and model; `omp:anthropic/claude-sonnet-5-5` and `omp:anthropic/claude-haiku-4-5`
> differ in their editor commit`

and wrote no matrix. The rerun, with a clean tree and no `--label`, has one commit for both rows. That is a
real defect in the runner's `--label` path for matrices (a label dirties the tree it is measuring), left as
a finding rather than papered over: the report above is the unlabelled run.

## What was not run, and why

- **`claude-code`**, the issue's second harness, needs a `claude` login. The owner retired it: the `claude`
  CLI login on big is expired and must not be revived, so the second row is a second omp model instead.
  The harness code and its reading of the stream-json transcript are covered by the crate's own tests
  (`agents_are_compared_on_the_benchmark`, 10 tests, all passing), which stand a recording in for `claude`.
- **One trial per task**, as the acceptance asks. `pass^k` above 1 is meaningless at n=1 and is shown as the
  one curve point it is.

## Build note

Building `bevy_dark_factory` from this checkout on big was repeatedly killed by `systemd-oomd` (llama-swap's
unit, 272–276 processes a kill, four times between 20:33 and 20:50) while `hs-distill` and the model servers
held the host's RAM. The runs above therefore use the gate's already-built binaries at `6b5162ea`, which are
the same commit and the same profile the push gate tests.
