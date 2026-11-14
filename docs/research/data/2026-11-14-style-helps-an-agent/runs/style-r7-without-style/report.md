# Benchmark run: style-r7-without-style

> **Unfit for comparison.** 1 of 3 trials are harness errors (33%), over the 10% a comparison tolerates: these numbers are about the harness, not the agent.

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

3 trial(s), 1 harness error(s). A harness error is never scored as a pass or a fail.

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 330 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.5097.**

## Harness errors

Why the harness, and not the agent, failed these trials:

| trials | reason | which |
|---|---|---|
| 1 | the transcript shows a tool \`xd__mcp__cauldron_grimoire_guide\` that is neither the editor's nor the harness's own set, so the profile was not isolated | `write-the-gatekeepers-greeting` trial 1 |

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 2 | 2 | 0 | 0% [0%, 0%] | 1:0.00 | $0.5078 | unknown | 1 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 0 | 0 | - | - | unknown | unknown | 1 |
| light | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.2394 | unknown | 0 |
| place | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.2683 | unknown | 0 |
| style | 2 | 2 | 0% [0%, 0%] | 1:0.00 | $0.5078 | unknown | 1 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 0 | 0% | 1:0.00 | 97 | 97 | 25.0 | 0 | 0 | 0 | 28/9159 (cache 489670/51093) | $0.2683 | 0 | 0 |
| `light-the-gate-at-night` | 1 | 0 | 0% | 1:0.00 | 40 | 40 | 9.0 | 1 | 0 | 0 | 28/4267 (cache 572265/55784) | $0.2394 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 0 | 0 | - | - | - | - | - | 0 | 0 | 0 | unknown/unknown (cache unknown/unknown) | unknown | 0 | 1 |

## Failed trials (dev tasks)

### `dress-the-market-area`, trial 1

- first failing check: `placements_within` — wanted 3 × a \`market_stall\`, all inside x -10.00..-2.00, z 8.50..9.50; measured 3 × a \`market_stall\` on the map, 0 of them inside x -10.00..-2.00, z 8.50..9.50, and the task wants 3 all inside; outside: \`market_stall@2\` at \(-8.50, 3.50\), \`market_stall@3\` at \(-6.00, 3.50\), \`market_stall@4\` at \(-3.50, 3.50\)
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-6,7.5],"piece":"lantern_oil"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-9.3,9.3],"piece":"crate_wood"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-8.4,9.3],"piece":"barrel_oak"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-3,9.3],"piece":"sack_grain"}}`
  - `grimoire_present {"check":[{"placements":["market_stall@2","market_stall@3","market_stall@4"],"what":"Stall facing: yaw 0 assumed, goods placed on the south side"},{"placements"…`

### `light-the-gate-at-night`, trial 1

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker","value":true}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"amber"}}`
  - `grimoire_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `grimoire_present {"check":[{"what":"tint 'amber' is a value the game reads (no list of allowed tints exists in the project)"},{"what":"1500 lm / 8 m radius is bright and wide en…`

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×25, `grimoire_guide` ×2, `grimoire_list_operations` ×2, `grimoire_present` ×2, `grimoire_state` ×2, `grimoire_list_pieces` ×1
- refusals by tool: `grimoire_apply_operation` ×1
- wishes, by the tool wished for: none
