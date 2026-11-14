# Benchmark run: style-r2-with-style

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

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 330 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.8269.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.8249 | $0.2750 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.1120 | $0.1120 | 0 |
| light | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.2099 | $0.2099 | 0 |
| place | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.5031 | $0.5031 | 0 |
| style | 3 | 3 | 100% [100%, 100%] | 1:1.00 | $0.8249 | $0.2750 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 1 | 100% | 1:1.00 | 126 | 126 | 26.0 | 1 | 1 | 0 | 48/14320 (cache 1381874/88638) | $0.5031 | 0 | 0 |
| `light-the-gate-at-night` | 1 | 1 | 100% | 1:1.00 | 44 | 44 | 15.0 | 2 | 0 | 0 | 36/4766 (cache 633722/39516) | $0.2099 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 1 | 1 | 100% | 1:1.00 | 33 | 33 | 7.0 | 1 | 0 | 0 | 20/3743 (cache 197490/21897) | $0.1120 | 0 | 0 |

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×26, `grimoire_style` ×4, `grimoire_guide` ×3, `grimoire_list_operations` ×3, `grimoire_present` ×3, `grimoire_state` ×3, `grimoire_preview_operation` ×2, `grimoire_capture_still` ×1
- refusals by tool: `grimoire_apply_operation` ×1, `grimoire_preview_operation` ×1, `grimoire_schema` ×1, `grimoire_style` ×1
- wishes, by the tool wished for: none
