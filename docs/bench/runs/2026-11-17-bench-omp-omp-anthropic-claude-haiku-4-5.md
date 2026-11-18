# Benchmark run: bench-omp-omp-anthropic-claude-haiku-4-5

| | |
|---|---|
| Editor and MCP server commit | `6b5162eab2502a9d1d71b121c156e65bb3bc02b3+dirty` |
| Shipped guide SHA-256 | `e7b841c5298bcdb44faca2526a7463c69e522bead4e7ebe50249f4b768087dbe` |
| Agent | omp omp/18.21.0 |
| Model | anthropic/claude-haiku-4-5 |
| Variant | none |
| Trials per task | 1 |
| Suite | bench/tasks |
| Host | big |
| Date | 2026-11-17 |
| Price per million tokens | input $1, output $5, cache read $0.1, cache write $1.25 |

3 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

Pre-flight turn (one real model turn before any trial, in no trial's cost): 10 fresh in, 54 out, 0 cache read, 8351 cache written, $0.0107. **Cost of the run, trials and pre-flight turn: $0.2685.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 3 | 3 | 1 | 33% [0%, 100%] | 1:0.33 | $0.2577 | $0.2577 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| handback | 2 | 2 | 50% [0%, 100%] | 1:0.50 | $0.0852 | $0.0852 | 0 |
| place | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.1725 | unknown | 0 |
| read | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.0379 | unknown | 0 |
| refusal | 1 | 1 | 100% [100%, 100%] | 1:1.00 | $0.0473 | $0.0473 | 0 |
| relations | 1 | 1 | 0% [0%, 0%] | 1:0.00 | $0.1725 | unknown | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `bed-against-the-north-wall` | 1 | 0 | 0% | 1:0.00 | 94 | 94 | 15.0 | 2 | 1 | 0 | 153/7220 (cache 674607/55054) | $0.1725 | 0 | 0 |
| `count-what-the-bedroom-holds` | 1 | 0 | 0% | 1:0.00 | 32 | 32 | 2.0 | 0 | 0 | 0 | 49/2758 (cache 83701/12553) | $0.0379 | 0 | 0 |
| `refuse-a-piece-the-project-lacks` | 1 | 1 | 100% | 1:1.00 | 30 | 30 | 5.0 | 0 | 0 | 0 | 65/2152 (cache 138560/18097) | $0.0473 | 0 | 0 |

## Failed trials (dev tasks)

### `bed-against-the-north-wall`, trial 1

- first failing check: `placement_near_wall` — wanted a \`bed_double\` against the north wall within 0.050 m, centred within 0.200 m; measured \`bed_double@1\` is -0.110 m from the north wall \(through it\), and the task wants within 0.050 m and 0.000 m off the middle of it
- its last 5 call(s), oldest first:
  - `blueprint_undo_operation {"operation_id":"0000-3602"}`
  - `blueprint_apply_operation {"operation":"placement_add","parameters":{"at":[0,-1.5],"piece":"bed_double"}}`
  - `blueprint_capture_still {"label":"bed_final"}`
  - `blueprint_decide {"questions":{"bed_kind":{"criteria":{"no":"not a bed","yes":"clearly a bed with mattress and pillows"},"instructions":"The furniture placed is a bed.","type":"…`
  - `blueprint_present {"inferred":["The room is 6 m wide (x) and 5 m deep (z), with north at z = -2.5. The bed_double piece was used. Placement at (0.0, -1.5) centres the bed horizon…`

### `count-what-the-bedroom-holds`, trial 1

- first failing check: `handback_contains` — wanted a hand-back stating three, dresser; measured the hand-back \`Bedroom inventory count\` does not state \`three\`
- its last 2 call(s), oldest first:
  - `blueprint_state {}`
  - `blueprint_present {"specified":["Count pieces and identify the one nearest the north wall"],"summary":"The bedroom holds 3 pieces. The dresser (dresser_tall) is nearest the north…`

## Where the agent stumbles (dev tasks)

- tools called most: `blueprint_list_pieces` ×3, `blueprint_present` ×3, `blueprint_preview_operation` ×3, `blueprint_state` ×3, `blueprint_apply_operation` ×2, `blueprint_capture_still` ×2, `blueprint_decide` ×2, `blueprint_guide` ×2
- refusals by tool: `blueprint_preview_operation` ×2
- wishes, by the tool wished for: none
