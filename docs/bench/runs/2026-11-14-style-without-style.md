# Benchmark run: style-without-style

> **Unfit for comparison.** 14 of 24 trials are harness errors (58%), over the 10% a comparison tolerates: these numbers are about the harness, not the agent.

| | |
|---|---|
| Editor and MCP server commit | `c16441584c0eef5cc03aff187ad02124bfa71b28` |
| Shipped guide SHA-256 | `58bdbbdf8bf7904d734d16ac22d410c10d38ed5232b31a11ccb7ac7eb6c766d5` |
| Agent | omp omp/18.16.0 |
| Model | anthropic/claude-sonnet-5-5 |
| Variant | without-style |
| Trials per task | 8 |
| Suite | bench/tasks |
| Host | bmb |
| Date | 2026-11-14 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

24 trial(s), 14 harness error(s). A harness error is never scored as a pass or a fail.

## Harness errors

Why the harness, and not the agent, failed these trials:

| trials | reason | which |
|---|---|---|
| 9 | the transcript shows a tool \`xd__mcp__dark_factory_blueprint_guide\` that is neither the editor's nor the harness's own set, so the profile was not isolated | `dress-the-market-area` trial 1, `dress-the-market-area` trial 3, `dress-the-market-area` trial 5, `dress-the-market-area` trial 8, `light-the-gate-at-night` trial 4, `write-the-gatekeepers-greeting` trial 1, `write-the-gatekeepers-greeting` trial 5, `write-the-gatekeepers-greeting` trial 6, `write-the-gatekeepers-greeting` trial 7 |
| 5 | the transcript shows a tool \`xd___mcp__dark_factory_blueprint_guide\` that is neither the editor's nor the harness's own set, so the profile was not isolated | `dress-the-market-area` trial 2, `dress-the-market-area` trial 4, `write-the-gatekeepers-greeting` trial 3, `write-the-gatekeepers-greeting` trial 4, `write-the-gatekeepers-greeting` trial 8 |

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 10 | 0 | 0% [0%, 0%] | 1:0.00 | $2.0662 | unknown | 14 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.1218 | unknown | 7 |
| light | 1 | 7 | 0% [0%, 0%] | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 6:0.00 7:0.00 | $1.4032 | unknown | 1 |
| place | 1 | 2 | 0% [0%, 0%] | 1:0.00 2:0.00 | $0.5412 | unknown | 6 |
| style | 3 | 10 | 0% [0%, 0%] | 1:0.00 | $2.0662 | unknown | 14 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 2 | 0 | 0% | 1:0.00 2:0.00 | 83 | 94 | 24.0 | 0 | 0 | 0 | 56/17377 (cache 980262/107713) | $0.5412 | 0 | 6 |
| `light-the-gate-at-night` | 7 | 0 | 0% | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 6:0.00 7:0.00 | 41 | 48 | 11.6 | 14 | 0 | 0 | 202/30791 (cache 3463258/299411) | $1.4032 | 0 | 1 |
| `write-the-gatekeepers-greeting` | 1 | 0 | 0% | 1:0.00 | 27 | 27 | 4.0 | 0 | 0 | 0 | 18/2624 (cache 201279/30166) | $0.1218 | 0 | 7 |

## Failed trials (dev tasks)

### `dress-the-market-area`, trial 6

- first failing check: `placements_within` — wanted 3 × a \`market_stall\`, all inside x -10.00..-2.00, z 8.50..9.50; measured 3 × a \`market_stall\` on the map, 0 of them inside x -10.00..-2.00, z 8.50..9.50, and the task wants 3 all inside; outside: \`market_stall@2\` at \(-8.50, 3.50\), \`market_stall@3\` at \(-6.00, 3.50\), \`market_stall@4\` at \(-3.50, 3.50\)
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-4,8],"piece":"bench_plank"}}`
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-9.3,9.3],"piece":"crate_wood"}}`
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-2.8,9.3],"piece":"barrel_oak"}}`
  - `blueprint_capture_still {"label":"market_dressed"}`
  - `blueprint_present {"check":[{"placements":["market_stall@2","market_stall@3","market_stall@4","lantern_oil@11","lantern_oil@12"],"what":"Stall row and the goods and lanterns arou…`

### `dress-the-market-area`, trial 7

- first failing check: `placements_within` — wanted 3 × a \`market_stall\`, all inside x -10.00..-2.00, z 8.50..9.50; measured 3 × a \`market_stall\` on the map, 0 of them inside x -10.00..-2.00, z 8.50..9.50, and the task wants 3 all inside; outside: \`market_stall@2\` at \(-8.50, 3.50\), \`market_stall@3\` at \(-6.00, 3.50\), \`market_stall@4\` at \(-3.50, 3.50\)
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-6,7.5],"piece":"lantern_oil"}}`
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-9.3,9.3],"piece":"crate_wood"}}`
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-8.4,9.3],"piece":"barrel_oak"}}`
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-3,9.3],"piece":"sack_grain"}}`
  - `blueprint_present {"check":[{"placements":["market_stall@2","market_stall@3","market_stall@4"],"what":"Stall facing: yaw 0 assumed, goods placed on the south side"},{"placements"…`

### `light-the-gate-at-night`, trial 1

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":8}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":8.5}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"amber"}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `blueprint_present {"check":[{"what":"intensity_lm 900 and radius_m 8.0 together: does the lit pool read well against ambient_lux 2.5?"},{"what":"tint 'amber': confirm the game ac…`

### `light-the-gate-at-night`, trial 2

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 10.5, and it should be at most 5
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":3}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.radius_m","value":10.5}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":7.5}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":3.5}}`
  - `blueprint_present {"check":[{"what":"Tint 'amber': confirm the game reads that string as a tint (the file gave no list of valid names)."},{"what":"Intensity 2000 lm / radius 10.5…`

### `light-the-gate-at-night`, trial 3

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 10.5, and it should be at most 5
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker","value":true}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":7.5}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"warm"}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `blueprint_present {"check":[{"what":"assets/blueprint/furniture/data/gate_light.yaml: whether tint 'warm' is a value the game accepts, and whether 2500 lm / 10.5 m suit the courty…`

### `light-the-gate-at-night`, trial 5

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker","value":true}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"warm"}}`
  - `blueprint_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `blueprint_present {"check":[{"what":"tint: 'warm' is a free string; confirm the game recognises it (the day value was 'white')"},{"what":"intensity_lm 1500 and radius_m 8.0 may b…`

### `light-the-gate-at-night`, trial 6

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker","value":true}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"amber"}}`
  - `blueprint_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `blueprint_present {"check":[{"what":"tint: 'amber' is accepted by whatever reads the file; the only prior value was 'white'"},{"what":"Whether flicker at 12 Hz looks right as a f…`

### `light-the-gate-at-night`, trial 7

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker","value":true}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"amber"}}`
  - `blueprint_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `blueprint_present {"check":[{"what":"tint 'amber' is a value the game reads (no list of allowed tints exists in the project)"},{"what":"1500 lm / 8 m radius is bright and wide en…`

### `light-the-gate-at-night`, trial 8

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"amber"}}`
  - `blueprint_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":8.5}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `blueprint_present {"check":[{"what":"tint: 'amber' - confirm the game accepts that tint name (the previous value was 'white')"},{"what":"intensity_lm 1500 / radius_m 8.0 left at …`

### `write-the-gatekeepers-greeting`, trial 2

- first failing check: `file_value` — wanted \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` contains \`toll\`; measured \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` is "Halt there, traveller. This is the courtyard gate, and I keep it. State your name and your business, and if both are honest, you may pass.", and it should contain \`toll\`
- its last 4 call(s), oldest first:
  - `blueprint_guide {}`
  - `blueprint_list_operations {"query":"data_set"}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"dialogue/gatekeeper","path":"gatekeeper.greeting.text","value":"Halt there, traveller. This is the courtyard gate,…`
  - `blueprint_present {"check":[{"what":"Whether the stern 'Halt there' opening suits the game's intended tone; it could be warmer."},{"what":"Whether the gatekeeper should demand a …`

## Where the agent stumbles (dev tasks)

- tools called most: `blueprint_apply_operation` ×89, `blueprint_guide` ×10, `blueprint_list_operations` ×10, `blueprint_present` ×10, `blueprint_state` ×9, `blueprint_list_pieces` ×2, `blueprint_capture_still` ×1, `blueprint_preview_operation` ×1
- refusals by tool: `blueprint_apply_operation` ×14
- wishes, by the tool wished for: none
