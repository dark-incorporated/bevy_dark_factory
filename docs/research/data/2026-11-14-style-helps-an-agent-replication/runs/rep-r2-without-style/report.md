# Benchmark run: rep-r2-without-style

| | |
|---|---|
| Editor and MCP server commit | `dc3a2d26ed55f3d4e64ba52352e60c31fe73a15b` |
| Shipped guide SHA-256 | `58bdbbdf8bf7904d734d16ac22d410c10d38ed5232b31a11ccb7ac7eb6c766d5` |
| Agent | omp omp/18.16.0 |
| Model | anthropic/claude-sonnet-5-5 |
| Variant | without-style |
| Trials per task | 1 |
| Suite | bench/tasks |
| Host | bmb |
| Date | 2026-11-14 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

3 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 331 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.6020.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 0 | 0% [0%, 0%] | 1:0.00 | $0.6000 | unknown | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.1386 | unknown | 0 |
| light | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.1764 | unknown | 0 |
| place | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.2850 | unknown | 0 |
| style | 3 | 3 | 0% [0%, 0%] | 1:0.00 | $0.6000 | unknown | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 0 | 0% | 1:0.00 | 78 | 78 | 23.0 | 0 | 0 | 0 | 30/8109 (cache 574058/58563) | $0.2850 | 0 | 0 |
| `light-the-gate-at-night` | 1 | 0 | 0% | 1:0.00 | 43 | 43 | 11.0 | 2 | 0 | 0 | 26/4507 (cache 394914/36725) | $0.1764 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 1 | 0 | 0% | 1:0.00 | 31 | 31 | 5.0 | 0 | 0 | 0 | 22/2627 (cache 274749/33936) | $0.1386 | 0 | 0 |

## Failed trials (dev tasks)

### `dress-the-market-area`, trial 1

- first failing check: `placements_within` — wanted 3 × a \`market_stall\`, all inside x -10.00..-2.00, z 8.50..9.50; measured 3 × a \`market_stall\` on the map, 0 of them inside x -10.00..-2.00, z 8.50..9.50, and the task wants 3 all inside; outside: \`market_stall@2\` at \(-8.50, 3.50\), \`market_stall@3\` at \(-6.00, 3.50\), \`market_stall@4\` at \(-3.50, 3.50\)
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-8.5,8],"piece":"bench_plank"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-4,6.5],"piece":"crate_wood"}}`
  - `grimoire_apply_operation {"operation":"placement_add","parameters":{"at":[-8.5,6.5],"piece":"barrel_oak"}}`
  - `grimoire_capture_still {"label":"market_dressed"}`
  - `grimoire_present {"check":[{"placements":["market_stall@2","market_stall@3","market_stall@4","lantern_oil@12","lantern_oil@13"],"what":"Stall row and the pennants and lanterns a…`

### `light-the-gate-at-night`, trial 1

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"warm"}}`
  - `grimoire_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":7.5}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `grimoire_present {"check":[{"what":"Whether intensity_lm 1500 and radius_m 8.0 read as too bright or too wide against ambient_lux 2.5 in the running game; these are the two I le…`

### `write-the-gatekeepers-greeting`, trial 1

- first failing check: `file_value` — wanted \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` contains \`toll\`; measured \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` is "Halt there, traveller. This is the courtyard gate, and I keep it. State your name and your business, and I'll tell you whether you may pass.", and it should contain \`toll\`
- its last 5 call(s), oldest first:
  - `grimoire_guide {}`
  - `grimoire_state {}`
  - `grimoire_list_operations {}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"dialogue/gatekeeper","path":"gatekeeper.greeting.text","value":"Halt there, traveller. This is the courtyard gate,…`
  - `grimoire_present {"check":[{"what":"Tone against the game's intended voice: the library mixes market stalls and oil lanterns with neon signs and vending machines, so a medieval …`

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×24, `grimoire_guide` ×3, `grimoire_list_operations` ×3, `grimoire_present` ×3, `grimoire_state` ×3, `grimoire_capture_still` ×1, `grimoire_list_pieces` ×1, `grimoire_preview_operation` ×1
- refusals by tool: `grimoire_apply_operation` ×2
- wishes, by the tool wished for: none
