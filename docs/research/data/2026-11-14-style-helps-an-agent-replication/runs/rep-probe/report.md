# Benchmark run: rep-probe

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

1 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 0 cache read, 11025 cache written, $0.0276. **Cost of the run, trials and pre-flight turn: $0.2057.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 1 | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1781 | $0.1781 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| light | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1781 | $0.1781 | 0 |
| style | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1781 | $0.1781 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `light-the-gate-at-night` | 1 | 1 | 100% | 1:1.00 | 38 | 38 | 13.0 | 1 | 0 | 0 | 30/4544 (cache 449271/35070) | $0.1781 | 0 | 0 |

## Where the agent stumbles (dev tasks)

- tools called most: `blueprint_apply_operation` ×9, `blueprint_guide` ×1, `blueprint_list_operations` ×1, `blueprint_present` ×1, `blueprint_style` ×1
- refusals by tool: `blueprint_apply_operation` ×1
- wishes, by the tool wished for: none
