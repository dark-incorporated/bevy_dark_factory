# Benchmark run: rep-with-style

| | |
|---|---|
| Editor and MCP server commit | `dc3a2d26ed55f3d4e64ba52352e60c31fe73a15b` |
| Shipped guide SHA-256 | `58bdbbdf8bf7904d734d16ac22d410c10d38ed5232b31a11ccb7ac7eb6c766d5` |
| Agent | omp omp/18.16.0 |
| Model | anthropic/claude-sonnet-5-5 |
| Variant | with-style |
| Trials per task | 5 |
| Suite | bench/tasks |
| Host | bmb |
| Date | 2026-11-14 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

15 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

## Agent errors

3 call(s) to a tool that does not exist in `write-the-gatekeepers-greeting` trial 3, `write-the-gatekeepers-greeting` trial 4, `write-the-gatekeepers-greeting` trial 5. The agent was told so by its harness and went on, so each trial is scored: these are the agent's own mistakes and not the harness's. By the name called: `xd__mcp__dark_factory_blueprint_guide` ×3.

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 15 | 15 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $3.8624 | $0.2575 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 5 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $0.8422 | $0.1684 | 0 |
| light | 1 | 5 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $1.0279 | $0.2056 | 0 |
| place | 1 | 5 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $1.9923 | $0.3985 | 0 |
| style | 3 | 15 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $3.8624 | $0.2575 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 5 | 5 | 100% | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | 99 | 121 | 24.0 | 3 | 2 | 0 | 200/58909 (cache 4936135/363697) | $1.9923 | 1 | 0 |
| `light-the-gate-at-night` | 5 | 5 | 100% | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | 42 | 47 | 17.2 | 19 | 0 | 0 | 120/26360 (cache 1969627/226841) | $1.0279 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 5 | 5 | 100% | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | 33 | 38 | 7.8 | 7 | 0 | 0 | 116/17731 (cache 1689070/198294) | $0.8422 | 1 | 0 |

## Where the agent stumbles (dev tasks)

- tools called most: `blueprint_apply_operation` ×139, `blueprint_style` ×22, `blueprint_present` ×16, `blueprint_state` ×16, `blueprint_guide` ×15, `blueprint_list_operations` ×15, `blueprint_capture_still` ×5, `blueprint_list_pieces` ×5
- refusals by tool: `blueprint_apply_operation` ×22, `blueprint_style` ×5, `blueprint_present` ×1, `blueprint_preview_operation` ×1
- wishes, by the tool wished for: `A blueprint_preview_operation report for placement_add that lists which existing placements the new piece would replace or overlap, before anything is written.` ×1, `blueprint_list_operations with a name filter, returning only the named operation's schema.` ×1
