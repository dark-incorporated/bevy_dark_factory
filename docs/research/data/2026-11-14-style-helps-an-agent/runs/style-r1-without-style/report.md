# Benchmark run: style-r1-without-style

> **Unfit for comparison.** 2 of 3 trials are harness errors (67%), over the 10% a comparison tolerates: these numbers are about the harness, not the agent.

| | |
|---|---|
| Editor and MCP server commit | `c16441584c0eef5cc03aff187ad02124bfa71b28` |
| Shipped guide SHA-256 | `58bdbbdf8bf7904d734d16ac22d410c10d38ed5232b31a11ccb7ac7eb6c766d5` |
| Agent | omp omp/18.16.0 |
| Model | anthropic/claude-sonnet-5-5 |
| Variant | without-style |
| Trials per task | 1 |
| Suite | bench/tasks |
| Host | bmb |
| Date | 2026-11-14 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

3 trial(s), 2 harness error(s). A harness error is never scored as a pass or a fail.

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 330 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.1882.**

## Harness errors

Why the harness, and not the agent, failed these trials:

| trials | reason | which |
|---|---|---|
| 2 | the transcript shows a tool \`xd__mcp__cauldron_grimoire_guide\` that is neither the editor's nor the harness's own set, so the profile was not isolated | `dress-the-market-area` trial 1, `write-the-gatekeepers-greeting` trial 1 |

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 1 | 1 | 0 | 0% [0%, 0%] | 1:0.00 | $0.1863 | unknown | 2 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 0 | 0 | - | - | unknown | unknown | 1 |
| light | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.1863 | unknown | 0 |
| place | 0 | 0 | - | - | unknown | unknown | 1 |
| style | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.1863 | unknown | 2 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 0 | 0 | - | - | - | - | - | 0 | 0 | 0 | unknown/unknown (cache unknown/unknown) | unknown | 0 | 1 |
| `light-the-gate-at-night` | 1 | 0 | 0% | 1:0.00 | 48 | 48 | 11.0 | 1 | 0 | 0 | 32/4269 (cache 522187/36526) | $0.1863 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 0 | 0 | - | - | - | - | - | 0 | 0 | 0 | unknown/unknown (cache unknown/unknown) | unknown | 0 | 1 |

## Failed trials (dev tasks)

### `light-the-gate-at-night`, trial 1

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":8}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":8.5}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"amber"}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `grimoire_present {"check":[{"what":"intensity_lm 900 and radius_m 8.0 together: does the lit pool read well against ambient_lux 2.5?"},{"what":"tint 'amber': confirm the game ac…`

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×7, `grimoire_guide` ×1, `grimoire_list_operations` ×1, `grimoire_present` ×1, `grimoire_state` ×1
- refusals by tool: `grimoire_apply_operation` ×1
- wishes, by the tool wished for: none
