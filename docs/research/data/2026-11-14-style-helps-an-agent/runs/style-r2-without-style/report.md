# Benchmark run: style-r2-without-style

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

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 10698 cache read, 330 cache written, $0.0019. **Cost of the run, trials and pre-flight turn: $0.3805.**

## Harness errors

Why the harness, and not the agent, failed these trials:

| trials | reason | which |
|---|---|---|
| 1 | the transcript shows a tool \`xd___mcp__cauldron_grimoire_guide\` that is neither the editor's nor the harness's own set, so the profile was not isolated | `dress-the-market-area` trial 1 |

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 2 | 2 | 0 | 0% [0%, 0%] | 1:0.00 | $0.3785 | unknown | 1 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| dialogue | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.1218 | unknown | 0 |
| light | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.2567 | unknown | 0 |
| place | 0 | 0 | - | - | unknown | unknown | 1 |
| style | 2 | 2 | 0% [0%, 0%] | 1:0.00 | $0.3785 | unknown | 1 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `dress-the-market-area` | 0 | 0 | - | - | - | - | - | 0 | 0 | 0 | unknown/unknown (cache unknown/unknown) | unknown | 0 | 1 |
| `light-the-gate-at-night` | 1 | 0 | 0% | 1:0.00 | 46 | 46 | 15.0 | 3 | 0 | 0 | 28/5346 (cache 558125/58955) | $0.2567 | 0 | 0 |
| `write-the-gatekeepers-greeting` | 1 | 0 | 0% | 1:0.00 | 27 | 27 | 4.0 | 0 | 0 | 0 | 18/2624 (cache 201279/30166) | $0.1218 | 0 | 0 |

## Failed trials (dev tasks)

### `light-the-gate-at-night`, trial 1

- first failing check: `file_value` — wanted \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` ≥ 2 and ≤ 5; measured \`gate_light.radius_m\` in \`furniture/data/gate_light.yaml\` is 10.5, and it should be at most 5
- its last 5 call(s), oldest first:
  - `grimoire_apply_operation (refused) {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":3}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.radius_m","value":10.5}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.flicker_hz","value":7.5}}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"lighting/gate","path":"gate_light.ambient_lux","value":3.5}}`
  - `grimoire_present {"check":[{"what":"Tint 'amber': confirm the game reads that string as a tint (the file gave no list of valid names)."},{"what":"Intensity 2000 lm / radius 10.5…`

### `write-the-gatekeepers-greeting`, trial 1

- first failing check: `file_value` — wanted \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` contains \`toll\`; measured \`gatekeeper.greeting.text\` in \`furniture/data/gatekeeper.yaml\` is "Halt there, traveller. This is the courtyard gate, and I keep it. State your name and your business, and if both are honest, you may pass.", and it should contain \`toll\`
- its last 4 call(s), oldest first:
  - `grimoire_guide {}`
  - `grimoire_list_operations {"query":"data_set"}`
  - `grimoire_apply_operation {"operation":"data_set","parameters":{"file":"dialogue/gatekeeper","path":"gatekeeper.greeting.text","value":"Halt there, traveller. This is the courtyard gate,…`
  - `grimoire_present {"check":[{"what":"Whether the stern 'Halt there' opening suits the game's intended tone; it could be warmer."},{"what":"Whether the gatekeeper should demand a …`

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×11, `grimoire_guide` ×2, `grimoire_list_operations` ×2, `grimoire_present` ×2, `grimoire_preview_operation` ×1, `grimoire_state` ×1
- refusals by tool: `grimoire_apply_operation` ×3
- wishes, by the tool wished for: none
