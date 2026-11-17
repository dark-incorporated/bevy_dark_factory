# Benchmark run: style-r7-with-style

| | |
|---|---|
| Editor and MCP server commit | `c16441584c0eef5cc03aff187ad02124bfa71b28` |
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

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 330 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.7758.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.7739 | $0.2580 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1352 | $0.1352 | 0 |
| light | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.2016 | $0.2016 | 0 |
| place | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.4372 | $0.4372 | 0 |
| style | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.7739 | $0.2580 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 1 | 100% | 1:1.00 | 122 | 122 | 29.0 | 0 | 1 | 0 | 46/15028 (cache 1105980/70481) | $0.4372 | 1 | 0 |
| `light-the-gate-at-night` | 1 | 1 | 100% | 1:1.00 | 45 | 45 | 18.0 | 4 | 0 | 0 | 24/5940 (cache 370171/42034) | $0.2016 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 1 | 1 | 100% | 1:1.00 | 28 | 28 | 6.0 | 1 | 0 | 0 | 18/2940 (cache 227492/33189) | $0.1352 | 0 | 0 |

## Where the agent stumbles (dev tasks)

- tools called most: `blueprint_apply_operation` ×33, `blueprint_style` ×5, `blueprint_guide` ×3, `blueprint_list_operations` ×3, `blueprint_present` ×3, `blueprint_state` ×2, `blueprint_capture_still` ×1, `blueprint_list_pieces` ×1
- refusals by tool: `blueprint_apply_operation` ×4, `blueprint_style` ×1
- wishes, by the tool wished for: `placement_move: change a placement's x,z by id, with a dry run listing what it would replace or overlap.` ×1
