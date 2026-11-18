# Benchmark run: bench-omp-omp-anthropic-claude-sonnet-5-5

| | |
|---|---|
| Editor and MCP server commit | `6b5162eab2502a9d1d71b121c156e65bb3bc02b3` |
| Shipped guide SHA-256 | `e7b841c5298bcdb44faca2526a7463c69e522bead4e7ebe50249f4b768087dbe` |
| Agent | omp omp/18.21.0 |
| Model | anthropic/claude-sonnet-5-5 |
| Variant | none |
| Trials per task | 1 |
| Suite | bench/tasks |
| Host | big |
| Date | 2026-11-17 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

3 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10359 cache read, 349 cache written, $0.0020. **Cost of the run, trials and pre-flight turn: $0.3895.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.3876 | $0.1292 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| handback | 2 | 2 | 100% [100%, 100%] | 1:1.00 | $0.2295 | $0.1148 | 0 |
| place | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1580 | $0.1580 | 0 |
| read | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.0831 | $0.0831 | 0 |
| refusal | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1464 | $0.1464 | 0 |
| relations | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1580 | $0.1580 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `bed-against-the-north-wall` | 1 | 1 | 100% | 1:1.00 | 51 | 51 | 8.0 | 0 | 0 | 0 | 20/3386 (cache 296608/37788) | $0.1580 | 0 | 0 |
| `count-what-the-bedroom-holds` | 1 | 1 | 100% | 1:1.00 | 21 | 21 | 3.0 | 0 | 0 | 0 | 14/1594 (cache 134011/21503) | $0.0831 | 0 | 0 |
| `refuse-a-piece-the-project-lacks` | 1 | 1 | 100% | 1:1.00 | 21 | 21 | 5.0 | 0 | 0 | 0 | 16/1564 (cache 216017/43658) | $0.1464 | 0 | 0 |

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_guide` ×3, `grimoire_list_pieces` ×3, `grimoire_present` ×3, `grimoire_state` ×3, `grimoire_apply_operation` ×1, `grimoire_decide` ×1, `grimoire_list_operations` ×1, `grimoire_preview_operation` ×1
- refusals by tool: none
- wishes, by the tool wished for: none
