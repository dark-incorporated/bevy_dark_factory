# Benchmark run: rep-r4-without-style

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

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 331 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.6171.**

## Agent errors

1 call(s) to a tool that does not exist in `write-the-gatekeepers-greeting` trial 1. The agent was told so by its harness and went on, so each trial is scored: these are the agent's own mistakes and not the harness's. By the name called: `xd___mcp__dark_factory_blueprint_guide` ×1.

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 0 | 0% [0%, 0%] | 1:0.00 | $0.6152 | unknown | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.1308 | unknown | 0 |
| light | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.2485 | unknown | 0 |
| place | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.2359 | unknown | 0 |
| style | 3 | 3 | 0% [0%, 0%] | 1:0.00 | $0.6152 | unknown | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 1 | 0 | 0% | 1:0.00 | 54 | 54 | 18.0 | 0 | 0 | 0 | 24/6404 (cache 397680/52825) | $0.2359 | 0 | 0 |
| `light-the-gate-at-night` | 1 | 0 | 0% | 1:0.00 | 45 | 45 | 13.0 | 2 | 0 | 0 | 24/5324 (cache 464660/59487) | $0.2485 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 1 | 0 | 0% | 1:0.00 | 25 | 25 | 5.0 | 0 | 0 | 0 | 24/2141 (cache 285102/32326) | $0.1308 | 0 | 0 |

## Failed trials (dev tasks)

### `dress-the-market-area`, trial 1

- first failing check: `placements_within` — wanted 3 × a \`market_stall\`, all inside x -10.00..-2.00, z 8.50..9.50; measured 3 × a \`market_stall\` on the map, 0 of them inside x -10.00..-2.00, z 8.50..9.50, and the task wants 3 all inside; outside: \`market_stall@2\` at \(-8.50, 4.00\), \`market_stall@3\` at \(-6.00, 4.00\), \`market_stall@4\` at \(-3.50, 4.00\)
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-4.75,4],"piece":"lantern_oil","yaw":0}}`
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-9.6,2.5],"piece":"pennant_bright","yaw":0}}`
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[-2.4,2.5],"piece":"pennant_bright","yaw":0}}`
  - `blueprint_capture_still {"label":"market_dressed"}`
  - `blueprint_present {"check":[{"placements":["market_stall@2","market_stall@3","market_stall@4"],"what":"The three stalls: facing at yaw 0 and the 0.5 m gaps between them"},{"place…`

### `light-the-gate-at-night`, trial 1

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 8.0, and it should be at most 5
- its last 5 call(s), oldest first:
  - `blueprint_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":8}}`
  - `blueprint_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":7.5}}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":2.5}}`
  - `blueprint_present {"check":[{"what":"tint 'warm' is my guess: the file lists no allowed values; the editor accepted it"},{"what":"intensity_lm 1500 may be harsh for a flame at ni…`

### `write-the-gatekeepers-greeting`, trial 1

- first failing check: `file_value` — wanted \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` contains \`toll\`; measured \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` is "Hold there, traveller. This is the courtyard gate. Tell me your name and your business, and I'll see you through.", and it should contain \`toll\`
- its last 5 call(s), oldest first:
  - `blueprint_guide {}`
  - `blueprint_state {}`
  - `blueprint_list_operations {}`
  - `blueprint_apply_operation {"operation":"data_set","parameters":{"file":"dialogue/gatekeeper","path":"gatekeeper.greeting.text","value":"Hold there, traveller. This is the courtyard gate.…`
  - `blueprint_present {"check":[{"what":"Tone and spelling ('traveller', 'Hold there') against how you want the gatekeeper to sound; the project has no written dialogue style"},{"wha…`

## Where the agent stumbles (dev tasks)

- tools called most: `blueprint_apply_operation` ×20, `blueprint_guide` ×3, `blueprint_list_operations` ×3, `blueprint_present` ×3, `blueprint_state` ×3, `blueprint_capture_still` ×1, `blueprint_list_pieces` ×1, `blueprint_preview_operation` ×1
- refusals by tool: `blueprint_apply_operation` ×2
- wishes, by the tool wished for: none
