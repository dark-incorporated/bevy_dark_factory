# Benchmark run: style-with-style

> **Unfit for comparison.** 4 of 24 trials are harness errors (17%), over the 10% a comparison tolerates: these numbers are about the harness, not the agent.

| | |
|---|---|
| Editor and MCP server commit | `c16441584c0eef5cc03aff187ad02124bfa71b28` |
| Shipped guide SHA-256 | `58bdbbdf8bf7904d734d16ac22d410c10d38ed5232b31a11ccb7ac7eb6c766d5` |
| Agent | omp omp/18.16.0 |
| Model | anthropic/claude-sonnet-5-5 |
| Variant | with-style |
| Trials per task | 8 |
| Suite | bench/tasks |
| Host | bmb |
| Date | 2026-11-14 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

24 trial(s), 4 harness error(s). A harness error is never scored as a pass or a fail.

## Harness errors

Why the harness, and not the agent, failed these trials:

| trials | reason | which |
|---|---|---|
| 3 | the transcript shows a tool \`xd__mcp__cauldron_grimoire_guide\` that is neither the editor's nor the harness's own set, so the profile was not isolated | `dress-the-market-area` trial 3, `write-the-gatekeepers-greeting` trial 3, `write-the-gatekeepers-greeting` trial 5 |
| 1 | the transcript shows a tool \`xd___mcp__cauldron_grimoire_guide\` that is neither the editor's nor the harness's own set, so the profile was not isolated | `write-the-gatekeepers-greeting` trial 8 |

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 20 | 20 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $5.0605 | $0.2530 | 4 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 5 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $0.6812 | $0.1362 | 3 |
| light | 1 | 8 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 6:1.00 7:1.00 8:1.00 | $1.6752 | $0.2094 | 0 |
| place | 1 | 7 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 6:1.00 7:1.00 | $2.7042 | $0.3863 | 1 |
| style | 3 | 20 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | $5.0605 | $0.2530 | 4 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 7 | 7 | 100% | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 6:1.00 7:1.00 | 120 | 130 | 28.7 | 4 | 9 | 0 | 278/91844 (cache 6311206/461624) | $2.7042 | 2 | 1 |
| `light-the-gate-at-night` | 8 | 8 | 100% | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 6:1.00 7:1.00 8:1.00 | 45 | 47 | 17.0 | 28 | 0 | 0 | 222/42907 (cache 3741739/348595) | $1.6752 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 5 | 5 | 100% | 1:1.00 2:1.00 3:1.00 4:1.00 5:1.00 | 31 | 32 | 6.8 | 5 | 0 | 0 | 102/15626 (cache 1279882/158691) | $0.6812 | 0 | 3 |

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×228, `grimoire_style` ×29, `grimoire_list_operations` ×22, `grimoire_guide` ×20, `grimoire_present` ×20, `grimoire_state` ×19, `grimoire_undo_operation` ×9, `grimoire_list_pieces` ×7
- refusals by tool: `grimoire_apply_operation` ×26, `grimoire_style` ×6, `grimoire_schema` ×3, `grimoire_preview_operation` ×2
- wishes, by the tool wished for: `grimoire_preview_operation reporting, for placement_add, which existing placements the new piece would replace, before anything is applied.` ×1, `placement_move: change a placement's x,z by id, with a dry run listing what it would replace or overlap.` ×1
