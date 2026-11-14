# Benchmark run: style-r6-with-style

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

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 330 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.7722.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.7702 | $0.2567 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1430 | $0.1430 | 0 |
| light | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.2002 | $0.2002 | 0 |
| place | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.4271 | $0.4271 | 0 |
| style | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.7702 | $0.2567 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 1 | 100% | 1:1.00 | 135 | 135 | 42.0 | 3 | 2 | 0 | 46/16570 (cache 966416/65852) | $0.4271 | 1 | 0 |
| `light-the-gate-at-night` | 1 | 1 | 100% | 1:1.00 | 43 | 43 | 14.0 | 1 | 0 | 0 | 32/4548 (cache 545572/40025) | $0.2002 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 1 | 1 | 100% | 1:1.00 | 30 | 30 | 7.0 | 1 | 0 | 0 | 24/2380 (cache 348573/33711) | $0.1430 | 0 | 0 |

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×37, `grimoire_list_operations` ×5, `grimoire_style` ×5, `grimoire_guide` ×3, `grimoire_present` ×3, `grimoire_state` ×3, `grimoire_undo_operation` ×2, `grimoire_capture_still` ×1
- refusals by tool: `grimoire_style` ×2, `grimoire_apply_operation` ×1, `grimoire_preview_operation` ×1, `grimoire_schema` ×1
- wishes, by the tool wished for: `grimoire_preview_operation reporting, for placement_add, which existing placements the new piece would replace, before anything is applied.` ×1
