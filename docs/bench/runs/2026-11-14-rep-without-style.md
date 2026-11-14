# Benchmark run: rep-without-style

| | |
|---|---|
| Editor and MCP server commit | `dc3a2d26ed55f3d4e64ba52352e60c31fe73a15b` |
| Shipped guide SHA-256 | `58bdbbdf8bf7904d734d16ac22d410c10d38ed5232b31a11ccb7ac7eb6c766d5` |
| Agent | omp omp/18.16.0 |
| Model | anthropic/claude-sonnet-5-5 |
| Variant | without-style |
| Trials per task | 5 |
| Suite | bench/tasks |
| Host | bmb |
| Date | 2026-11-14 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

15 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

## Agent errors

3 call(s) to a tool that does not exist in `write-the-gatekeepers-greeting` trial 3, `write-the-gatekeepers-greeting` trial 4, `write-the-gatekeepers-greeting` trial 5. The agent was told so by its harness and went on, so each trial is scored: these are the agent's own mistakes and not the harness's. By the name called: `xd__mcp__cauldron_grimoire_guide` ×2, `xd___mcp__cauldron_grimoire_guide` ×1.

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 15 | 0 | 0% [0%, 0%] | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 | $3.1929 | unknown | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 5 | 0% [0%, 0%] | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 | $0.6868 | unknown | 0 |
| light | 1 | 5 | 0% [0%, 0%] | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 | $1.0511 | unknown | 0 |
| place | 1 | 5 | 0% [0%, 0%] | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 | $1.4550 | unknown | 0 |
| style | 3 | 15 | 0% [0%, 0%] | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 | $3.1929 | unknown | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 5 | 0 | 0% | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 | 72 | 76 | 21.6 | 0 | 0 | 0 | 140/37681 (cache 2841214/317502) | $1.4550 | 0 | 0 |
| `light-the-gate-at-night` | 5 | 0 | 0% | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 | 45 | 49 | 11.8 | 10 | 0 | 0 | 140/23528 (cache 2462928/227716) | $1.0511 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 5 | 0 | 0% | 1:0.00 2:0.00 3:0.00 4:0.00 5:0.00 | 28 | 34 | 5.2 | 0 | 0 | 0 | 120/11925 (cache 1545547/165091) | $0.6868 | 0 | 0 |

## Failed trials (dev tasks)

### `dress-the-market-area`, trial 1

- first failing check: `placements_within` — wanted 3 × a \`market_stall\`, all inside x -10.00..-2.00, z 8.50..9.50; measured 3 × a \`market_stall\` on the map, 0 of them inside x -10.00..-2.00, z 8.50..9.50, and the task wants 3 all inside; outside: \`market_stall@2\` at \(-8.50, 3.50\), \`market_stall@3\` at \(-6.00, 3.50\), \`market_stall@4\` at \(-3.50, 3.50\)
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-9.4,9.4],"piece":"crate_wood"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-2.6,9.4],"piece":"barrel_oak"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-6,9.5],"piece":"sack_grain"}}`
  - `grimoire_capture_still {"label":"market_dressed"}`
  - `grimoire_present {"check":[{"placements":["market_stall@2","market_stall@3","market_stall@4"],"what":"Stalls: market_stall@2 (-8.5,3.5), @3 (-6,3.5), @4 (-3.5,3.5). Check which …`

### `dress-the-market-area`, trial 2

- first failing check: `placements_within` — wanted 3 × a \`market_stall\`, all inside x -10.00..-2.00, z 8.50..9.50; measured 3 × a \`market_stall\` on the map, 0 of them inside x -10.00..-2.00, z 8.50..9.50, and the task wants 3 all inside; outside: \`market_stall@2\` at \(-8.50, 3.50\), \`market_stall@3\` at \(-6.00, 3.50\), \`market_stall@4\` at \(-3.50, 3.50\)
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-8.5,8],"piece":"bench_plank"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-4,6.5],"piece":"crate_wood"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-8.5,6.5],"piece":"barrel_oak"}}`
  - `grimoire_capture_still {"label":"market_dressed"}`
  - `grimoire_present {"check":[{"placements":["market_stall@2","market_stall@3","market_stall@4","lantern_oil@12","lantern_oil@13"],"what":"Stall row and the pennants and lanterns a…`

### `dress-the-market-area`, trial 3

- first failing check: `placements_within` — wanted 3 × a \`market_stall\`, all inside x -10.00..-2.00, z 8.50..9.50; measured 3 × a \`market_stall\` on the map, 0 of them inside x -10.00..-2.00, z 8.50..9.50, and the task wants 3 all inside; outside: \`market_stall@2\` at \(-8.50, 3.50\), \`market_stall@3\` at \(-6.00, 3.50\), \`market_stall@4\` at \(-3.50, 3.50\)
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-2.25,3.5],"piece":"pennant_bright"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-6,8],"piece":"bench_plank"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-6,6.5],"piece":"lantern_oil"}}`
  - `grimoire_capture_still {"label":"market_dressed"}`
  - `grimoire_present {"check":[{"placements":["market_stall@2","market_stall@3","market_stall@4","crate_wood@5","barrel_oak@10"],"what":"Stall row with goods in front, lanterns and …`

### `dress-the-market-area`, trial 4

- first failing check: `placements_within` — wanted 3 × a \`market_stall\`, all inside x -10.00..-2.00, z 8.50..9.50; measured 3 × a \`market_stall\` on the map, 0 of them inside x -10.00..-2.00, z 8.50..9.50, and the task wants 3 all inside; outside: \`market_stall@2\` at \(-8.50, 4.00\), \`market_stall@3\` at \(-6.00, 4.00\), \`market_stall@4\` at \(-3.50, 4.00\)
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-4.75,4],"piece":"lantern_oil","yaw":0}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-9.6,2.5],"piece":"pennant_bright","yaw":0}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-2.4,2.5],"piece":"pennant_bright","yaw":0}}`
  - `grimoire_capture_still {"label":"market_dressed"}`
  - `grimoire_present {"check":[{"placements":["market_stall@2","market_stall@3","market_stall@4"],"what":"The three stalls: facing at yaw 0 and the 0.5 m gaps between them"},{"place…`

### `dress-the-market-area`, trial 5

- first failing check: `placements_within` — wanted 3 × a \`market_stall\`, all inside x -10.00..-2.00, z 8.50..9.50; measured 3 × a \`market_stall\` on the map, 0 of them inside x -10.00..-2.00, z 8.50..9.50, and the task wants 3 all inside; outside: \`market_stall@2\` at \(-8.50, 3.80\), \`market_stall@3\` at \(-6.00, 3.80\), \`market_stall@4\` at \(-3.50, 3.80\)
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-8.5,6.5],"piece":"barrel_oak"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-3.5,6.5],"piece":"crate_wood"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-2.7,6.5],"piece":"sack_grain"}}`
  - `grimoire_capture_still {"label":"market_dressed"}`
  - `grimoire_present {"check":[{"placements":["market_stall@2","market_stall@3","market_stall@4","lantern_oil@8","lantern_oil@9"],"what":"The three stalls and their goods and lanter…`

### `light-the-gate-at-night`, trial 1

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"amber"}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker","value":true}}`
  - `grimoire_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `grimoire_present {"check":[{"what":"Is intensity 1500 lm and radius 8 m too bright/wide for a flame light at night? Those two are the likeliest to need your taste."},{"what":"Do…`

### `light-the-gate-at-night`, trial 2

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"warm"}}`
  - `grimoire_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":7.5}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `grimoire_present {"check":[{"what":"Whether intensity_lm 1500 and radius_m 8.0 read as too bright or too wide against ambient_lux 2.5 in the running game; these are the two I le…`

### `light-the-gate-at-night`, trial 3

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"warm"}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker","value":true}}`
  - `grimoire_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `grimoire_present {"check":[{"what":"Whether intensity_lm 1500 / radius_m 8.0 still feel right against the much darker ambient (2.5 lux), and whether flicker_hz 12.0 reads as fla…`

### `light-the-gate-at-night`, trial 4

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":8}}`
  - `grimoire_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":7.5}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `grimoire_present {"check":[{"what":"tint 'warm' is my guess: the file lists no allowed values; the editor accepted it"},{"what":"intensity_lm 1500 may be harsh for a flame at ni…`

### `light-the-gate-at-night`, trial 5

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 10.5, and it should be at most 5
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.radius_m","value":10.5}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":6.5}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `grimoire_present {"check":[{"what":"intensity_lm 2400 and ambient_lux 2.5: is the night bright enough at the gate, and dark enough elsewhere?"},{"what":"tint 'amber': confirm th…`

### `write-the-gatekeepers-greeting`, trial 1

- first failing check: `file_value` — wanted \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` contains \`toll\`; measured \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` is "Hold there, traveller. This is the courtyard gate. Tell me your name and what brings you, and I'll see you through.", and it should contain \`toll\`
- its last 5 call(s), oldest first:
  - `grimoire_guide {}`
  - `grimoire_state {}`
  - `grimoire_list_operations {}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"dialogue/gatekeeper","path":"gatekeeper.greeting.text","value":"Hold there, traveller. This is the courtyard gate.…`
  - `grimoire_present {"check":[{"what":"Whether the tone matches the game's setting: the registered pieces mix lanterns and market stalls with a neon sign and a vending machine."}],…`

### `write-the-gatekeepers-greeting`, trial 2

- first failing check: `file_value` — wanted \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` contains \`toll\`; measured \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` is "Halt there, traveller. This is the courtyard gate, and I keep it. State your name and your business, and I'll tell you whether you may pass.", and it should contain \`toll\`
- its last 5 call(s), oldest first:
  - `grimoire_guide {}`
  - `grimoire_state {}`
  - `grimoire_list_operations {}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"dialogue/gatekeeper","path":"gatekeeper.greeting.text","value":"Halt there, traveller. This is the courtyard gate,…`
  - `grimoire_present {"check":[{"what":"Tone against the game's intended voice: the library mixes market stalls and oil lanterns with neon signs and vending machines, so a medieval …`

### `write-the-gatekeepers-greeting`, trial 3

- first failing check: `file_value` — wanted \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` contains \`toll\`; measured \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` is "Hold there, traveller. This is the courtyard gate, and I keep it. State your name and your business, and if it is honest, I will see you through.", and it should contain \`toll\`
- its last 5 call(s), oldest first:
  - `grimoire_state {}`
  - `grimoire_list_operations {}`
  - `grimoire_apply_operation {"dry_run":true,"operation":"data_set","parameters":{"file":"dialogue/gatekeeper","path":"gatekeeper.greeting.text","value":"Hold there, traveller. This is the …`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"dialogue/gatekeeper","path":"gatekeeper.greeting.text","value":"Hold there, traveller. This is the courtyard gate,…`
  - `grimoire_present {"check":[{"what":"Tone against the game's intended voice. The market-courtyard props mix old (lanterns, oak barrels) and modern (neon sign, vending machine), s…`

### `write-the-gatekeepers-greeting`, trial 4

- first failing check: `file_value` — wanted \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` contains \`toll\`; measured \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` is "Hold there, traveller. This is the courtyard gate. Tell me your name and your business, and I'll see you through.", and it should contain \`toll\`
- its last 5 call(s), oldest first:
  - `grimoire_guide {}`
  - `grimoire_state {}`
  - `grimoire_list_operations {}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"dialogue/gatekeeper","path":"gatekeeper.greeting.text","value":"Hold there, traveller. This is the courtyard gate.…`
  - `grimoire_present {"check":[{"what":"Tone and spelling ('traveller', 'Hold there') against how you want the gatekeeper to sound; the project has no written dialogue style"},{"wha…`

### `write-the-gatekeepers-greeting`, trial 5

- first failing check: `file_value` — wanted \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` contains \`toll\`; measured \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` is "Halt there, traveller. This is the courtyard gate, and I keep it. State your name and your business, and I'll tell you whether you may pass.", and it should contain \`toll\`
- its last 5 call(s), oldest first:
  - `grimoire_guide {}`
  - `grimoire_state {}`
  - `grimoire_list_operations {}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"dialogue/gatekeeper","path":"gatekeeper.greeting.text","value":"Halt there, traveller. This is the courtyard gate,…`
  - `grimoire_present {"check":[{"what":"Whether the stern tone suits the game: the courtyard's props (neon sign, vending machine, traffic cone, oil lanterns) mix eras, and I did not…`

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×116, `grimoire_guide` ×15, `grimoire_list_operations` ×15, `grimoire_present` ×15, `grimoire_state` ×15, `grimoire_capture_still` ×5, `grimoire_list_pieces` ×5, `grimoire_preview_operation` ×5
- refusals by tool: `grimoire_apply_operation` ×9, `grimoire_preview_operation` ×1
- wishes, by the tool wished for: none
