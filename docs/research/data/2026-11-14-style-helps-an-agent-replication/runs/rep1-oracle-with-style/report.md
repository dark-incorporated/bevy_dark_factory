# Benchmark run: rep1-oracle-with-style

| | |
|---|---|
| Editor and MCP server commit | `dc3a2d26ed55f3d4e64ba52352e60c31fe73a15b` |
| Shipped guide SHA-256 | `58bdbbdf8bf7904d734d16ac22d410c10d38ed5232b31a11ccb7ac7eb6c766d5` |
| Agent | oracle scripted |
| Model | none (no model) |
| Variant | with-style |
| Trials per task | 1 |
| Suite | bench/tasks |
| Host | bmb |
| Date | 2026-11-14 |
| Price per million tokens | none listed, so cost is unknown |

3 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 3 | 100% [100%, 100%] | 1:1.00 | unknown | unknown | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 100% [100%, 100%] | 1:1.00 | unknown | unknown | 0 |
| light | 1 | 1 | 100% [100%, 100%] | 1:1.00 | unknown | unknown | 0 |
| place | 1 | 1 | 100% [100%, 100%] | 1:1.00 | unknown | unknown | 0 |
| style | 3 | 3 | 100% [100%, 100%] | 1:1.00 | unknown | unknown | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 1 | 100% | 1:1.00 | 0 | 0 | 12.0 | 0 | 0 | 0 | unknown/unknown (cache unknown/unknown) | unknown | 0 | 0 |
| `light-the-gate-at-night` | 1 | 1 | 100% | 1:1.00 | 0 | 0 | 9.0 | 0 | 0 | 0 | unknown/unknown (cache unknown/unknown) | unknown | 0 | 0 |
| `write-the-gatekeepers-greeting` | 1 | 1 | 100% | 1:1.00 | 0 | 0 | 2.0 | 0 | 0 | 0 | unknown/unknown (cache unknown/unknown) | unknown | 0 | 0 |

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×20, `grimoire_present` ×3
- refusals by tool: none
- wishes, by the tool wished for: none
