# Benchmark run: rep-r5-with-style

| | |
|---|---|
| Editor and MCP server commit | `dc3a2d26ed55f3d4e64ba52352e60c31fe73a15b` |
| Shipped guide SHA-256 | `58bdbbdf8bf7904d734d16ac22d410c10d38ed5232b31a11ccb7ac7eb6c766d5` |
| Agent | omp omp/18.16.0 |
| Model | anthropic/claude-sonnet-5-5 |
| Variant | with-style |
| Trials per task | 1 |
| Suite | bench/tasks |
| Host | bmb |
| Date | 2026-11-14 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

3 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 331 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.7801.**

## Agent errors

1 call(s) to a tool that does not exist in `write-the-gatekeepers-greeting` trial 1. The agent was told so by its harness and went on, so each trial is scored: these are the agent's own mistakes and not the harness's. By the name called: `xd__mcp__dark_factory_blueprint_guide` ×1.

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.7781 | $0.2594 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1931 | $0.1931 | 0 |
| light | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1958 | $0.1958 | 0 |
| place | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.3892 | $0.3892 | 0 |
| style | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.7781 | $0.2594 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 1 | 100% | 1:1.00 | 103 | 103 | 32.0 | 0 | 1 | 0 | 44/12016 (cache 1005125/67389) | $0.3892 | 0 | 0 |
| `light-the-gate-at-night` | 1 | 1 | 100% | 1:1.00 | 42 | 42 | 17.0 | 4 | 0 | 0 | 26/5041 (cache 419343/41371) | $0.1958 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 1 | 1 | 100% | 1:1.00 | 27 | 27 | 7.0 | 1 | 0 | 0 | 22/2766 (cache 338280/52621) | $0.1931 | 0 | 0 |

## Where the agent stumbles (dev tasks)

- tools called most: `blueprint_apply_operation` ×36, `blueprint_style` ×4, `blueprint_guide` ×3, `blueprint_list_operations` ×3, `blueprint_present` ×3, `blueprint_state` ×3, `blueprint_capture_still` ×1, `blueprint_list_pieces` ×1
- refusals by tool: `blueprint_apply_operation` ×4, `blueprint_style` ×1
- wishes, by the tool wished for: none
