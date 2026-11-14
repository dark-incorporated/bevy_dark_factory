# Benchmark run: style-r1-with-style

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

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 330 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.6798.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.6779 | $0.2260 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1426 | $0.1426 | 0 |
| light | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1975 | $0.1975 | 0 |
| place | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.3378 | $0.3378 | 0 |
| style | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.6779 | $0.2260 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 1 | 100% | 1:1.00 | 97 | 97 | 29.0 | 0 | 3 | 0 | 36/12101 (cache 741141/57040) | $0.3378 | 0 | 0 |
| `light-the-gate-at-night` | 1 | 1 | 100% | 1:1.00 | 42 | 42 | 17.0 | 4 | 0 | 0 | 26/5208 (cache 420352/41333) | $0.1975 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 1 | 1 | 100% | 1:1.00 | 31 | 31 | 7.0 | 1 | 0 | 0 | 20/3139 (cache 253070/34330) | $0.1426 | 0 | 0 |

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×31, `grimoire_style` ×4, `grimoire_guide` ×3, `grimoire_list_operations` ×3, `grimoire_present` ×3, `grimoire_state` ×3, `grimoire_undo_operation` ×3, `grimoire_preview_operation` ×2
- refusals by tool: `grimoire_apply_operation` ×4, `grimoire_style` ×1
- wishes, by the tool wished for: none
