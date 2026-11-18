# Benchmark run: style-r8-with-style

> **Unfit for comparison.** 1 of 3 trials are harness errors (33%), over the 10% a comparison tolerates: these numbers are about the harness, not the agent.

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

3 trial(s), 1 harness error(s). A harness error is never scored as a pass or a fail.

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 330 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.5405.**

## Harness errors

Why the harness, and not the agent, failed these trials:

| trials | reason | which |
|---|---|---|
| 1 | the transcript shows a tool \`xd___mcp__dark_factory_blueprint_guide\` that is neither the editor's nor the harness's own set, so the profile was not isolated | `write-the-gatekeepers-greeting` trial 1 |

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 2 | 2 | 2 | 100% [100%, 100%] | 1:1.00 | $0.5386 | $0.2693 | 1 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 0 | 0 | - | - | unknown | unknown | 1 |
| light | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.2030 | $0.2030 | 0 |
| place | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.3356 | $0.3356 | 0 |
| style | 2 | 2 | 100% [100%, 100%] | 1:1.00 | $0.5386 | $0.2693 | 1 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 1 | 100% | 1:1.00 | 89 | 89 | 23.0 | 0 | 1 | 0 | 34/11170 (cache 691312/61862) | $0.3356 | 0 | 0 |
| `light-the-gate-at-night` | 1 | 1 | 100% | 1:1.00 | 46 | 46 | 18.0 | 4 | 0 | 0 | 26/5508 (cache 423567/42213) | $0.2030 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 0 | 0 | - | - | - | - | - | 0 | 0 | 0 | unknown/unknown (cache unknown/unknown) | unknown | 0 | 1 |

## Where the agent stumbles (dev tasks)

- tools called most: `blueprint_apply_operation` ×27, `blueprint_style` ×3, `blueprint_guide` ×2, `blueprint_list_operations` ×2, `blueprint_present` ×2, `blueprint_state` ×2, `blueprint_capture_still` ×1, `blueprint_list_pieces` ×1
- refusals by tool: `blueprint_apply_operation` ×4
- wishes, by the tool wished for: none
