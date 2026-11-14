# Benchmark run: rep-r2-with-style

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

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 331 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.7596.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.7576 | $0.2525 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1622 | $0.1622 | 0 |
| light | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1943 | $0.1943 | 0 |
| place | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.4011 | $0.4011 | 0 |
| style | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.7576 | $0.2525 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 1 | 100% | 1:1.00 | 90 | 90 | 19.0 | 1 | 0 | 0 | 36/10664 (cache 958375/79440) | $0.4011 | 0 | 0 |
| `light-the-gate-at-night` | 1 | 1 | 100% | 1:1.00 | 43 | 43 | 17.0 | 3 | 0 | 0 | 24/5122 (cache 375924/42157) | $0.1943 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 1 | 1 | 100% | 1:1.00 | 35 | 35 | 7.0 | 1 | 0 | 0 | 22/3869 (cache 313885/36849) | $0.1622 | 0 | 0 |

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×24, `grimoire_style` ×5, `grimoire_guide` ×3, `grimoire_list_operations` ×3, `grimoire_present` ×3, `grimoire_state` ×3, `grimoire_capture_still` ×1, `grimoire_list_pieces` ×1
- refusals by tool: `grimoire_apply_operation` ×4, `grimoire_style` ×1
- wishes, by the tool wished for: none
