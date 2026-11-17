# Benchmark run: rep-r1-without-style

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

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 331 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.7232.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 0 | 0% [0%, 0%] | 1:0.00 | $0.7213 | unknown | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.1307 | unknown | 0 |
| light | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.2549 | unknown | 0 |
| place | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.3357 | unknown | 0 |
| style | 3 | 3 | 0% [0%, 0%] | 1:0.00 | $0.7213 | unknown | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 0 | 0% | 1:0.00 | 72 | 72 | 23.0 | 0 | 0 | 0 | 28/8091 (cache 658003/75584) | $0.3357 | 0 | 0 |
| `light-the-gate-at-night` | 1 | 0 | 0% | 1:0.00 | 51 | 51 | 11.0 | 2 | 0 | 0 | 32/4742 (cache 675278/55954) | $0.2549 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 1 | 0 | 0% | 1:0.00 | 28 | 28 | 5.0 | 0 | 0 | 0 | 22/2370 (cache 271495/31909) | $0.1307 | 0 | 0 |

## Failed trials (dev tasks)

### `dress-the-market-area`, trial 1

- first failing check: `placements_within` — wanted 3 × a \`market_stall\`, all inside x -10.00..-2.00, z 8.50..9.50; measured 3 × a \`market_stall\` on the map, 0 of them inside x -10.00..-2.00, z 8.50..9.50, and the task wants 3 all inside; outside: \`market_stall@2\` at \(-8.50, 3.50\), \`market_stall@3\` at \(-6.00, 3.50\), \`market_stall@4\` at \(-3.50, 3.50\)
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-9.4,9.4],"piece":"crate_wood"}}`
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-2.6,9.4],"piece":"barrel_oak"}}`
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-6,9.5],"piece":"sack_grain"}}`
  - `blueprint_capture_still {"label":"market_dressed"}`
  - `blueprint_present {"check":[{"placements":["market_stall@2","market_stall@3","market_stall@4"],"what":"Stalls: market_stall@2 (-8.5,3.5), @3 (-6,3.5), @4 (-3.5,3.5). Check which …`

### `light-the-gate-at-night`, trial 1

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.tint","value":"amber"}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker","value":true}}`
  - `blueprint_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `blueprint_present {"check":[{"what":"Is intensity 1500 lm and radius 8 m too bright/wide for a flame light at night? Those two are the likeliest to need your taste."},{"what":"Do…`

### `write-the-gatekeepers-greeting`, trial 1

- first failing check: `file_value` — wanted \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` contains \`toll\`; measured \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` is "Hold there, traveller. This is the courtyard gate. Tell me your name and what brings you, and I'll see you through.", and it should contain \`toll\`
- its last 5 call(s), oldest first:
  - `blueprint_guide {}`
  - `blueprint_state {}`
  - `blueprint_list_operations {}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"dialogue/gatekeeper","path":"gatekeeper.greeting.text","value":"Hold there, traveller. This is the courtyard gate.…`
  - `blueprint_present {"check":[{"what":"Whether the tone matches the game's setting: the registered pieces mix lanterns and market stalls with a neon sign and a vending machine."}],…`

## Where the agent stumbles (dev tasks)

- tools called most: `blueprint_apply_operation` ×23, `blueprint_guide` ×3, `blueprint_list_operations` ×3, `blueprint_present` ×3, `blueprint_state` ×3, `blueprint_preview_operation` ×2, `blueprint_capture_still` ×1, `blueprint_list_pieces` ×1
- refusals by tool: `blueprint_apply_operation` ×1, `blueprint_preview_operation` ×1
- wishes, by the tool wished for: none
