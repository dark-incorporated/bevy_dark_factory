# Benchmark run: ask-without-guide

| | |
|---|---|
| Editor and MCP server commit | `c21f8ff35aeea835a3555ba49fd26365f58f140b` |
| Shipped guide SHA-256 | `335567f7646c23988d3bf55f0d24a41ecacf647de9a4ab50cf001247cc76def8` |
| Agent | omp omp/18.18.9 |
| Model | anthropic/claude-sonnet-5-5 |
| Variant | none |
| Trials per task | 3 |
| Suite | bench/tasks |
| Host | big |
| Date | 2026-11-16 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

60 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

## Agent errors

6 call(s) to a tool that does not exist in `cover-the-hallway-floor` trial 3, `hedge-along-the-south-edge` trial 2, `lamp-on-the-bedside-table` trial 2, `make-a-cellar-map` trial 3. The agent was told so by its harness and went on, so each trial is scored: these are the agent's own mistakes and not the harness's. By the name called: `xd__mcp__dark_factory_blueprint_guide` ×3, `xd___read` ×1, `xd__mcp__dark_factory_blueprint_list_operations` ×1, `xd__mcp__dark_factory_blueprint_state` ×1.

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 20 | 60 | 57 | 95% [85%, 100%] | 1:0.95 2:0.92 3:0.90 | $10.0444 | $0.1762 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| compose | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.5273 | $0.1758 | 0 |
| data | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.2935 | $0.0978 | 0 |
| fix | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.2899 | $0.2150 | 0 |
| handback | 4 | 12 | 83% [50%, 100%] | 1:0.83 2:0.75 3:0.75 | $1.3156 | $0.1316 | 0 |
| maps | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.3915 | $0.1305 | 0 |
| mount | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.4284 | $0.1428 | 0 |
| notes | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.7702 | $0.2567 | 0 |
| place | 3 | 9 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.8520 | $0.2058 | 0 |
| read | 1 | 3 | 33% [0%, 100%] | 1:0.33 2:0.00 3:0.00 | $0.2107 | $0.2107 | 0 |
| refusal | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.7480 | $0.1247 | 0 |
| relations | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.4236 | $0.2373 | 0 |
| remove | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.3377 | $0.1126 | 0 |
| runs | 1 | 3 | 67% [0%, 100%] | 1:0.67 2:0.33 3:0.00 | $0.7042 | $0.3521 | 0 |
| terrain | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.1500 | $0.1917 | 0 |
| tiles | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.6342 | $0.2114 | 0 |
| turn | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.5938 | $0.1979 | 0 |
| undo | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.1844 | $0.0615 | 0 |
| verify | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.3569 | $0.1190 | 0 |
| water | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.6067 | $0.2022 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `bed-against-the-north-wall` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 39 | 40 | 8.7 | 3 | 0 | 0 | 66/10108 (cache 1383372/136977) | $0.5820 | 0 | 0 |
| `chair-west-of-the-desk-facing-it` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 56 | 59 | 11.3 | 3 | 0 | 0 | 92/15246 (cache 2299507/183617) | $0.8416 | 0 | 0 |
| `clear-the-overlap-keep-both` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 52 | 56 | 14.0 | 8 | 0 | 0 | 98/13330 (cache 2729617/212799) | $0.9385 | 0 | 0 |
| `count-what-the-bedroom-holds` | 3 | 1 | 33% | 1:0.33 2:0.00 3:0.00 | 19 | 21 | 3.0 | 0 | 0 | 0 | 38/4493 (cache 366307/51622) | $0.2107 | 0 | 0 |
| `cover-the-hallway-floor` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 36 | 36 | 11.0 | 1 | 0 | 0 | 70/7520 (cache 1508327/163210) | $0.6342 | 0 | 0 |
| `fix-the-floating-lamp` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 37 | 40 | 8.0 | 3 | 0 | 0 | 66/6784 (cache 841359/79743) | $0.3515 | 0 | 0 |
| `furnish-a-reading-nook` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 57 | 59 | 18.3 | 2 | 0 | 0 | 76/12424 (cache 1271215/110326) | $0.5273 | 0 | 0 |
| `hedge-along-the-south-edge` | 3 | 2 | 67% | 1:0.67 2:0.33 3:0.00 | 73 | 75 | 17.0 | 7 | 0 | 0 | 82/13358 (cache 1818178/155450) | $0.7042 | 0 | 0 |
| `is-the-lamp-on-the-table` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 38 | 39 | 4.0 | 0 | 0 | 0 | 56/7918 (cache 701606/82987) | $0.3569 | 0 | 0 |
| `lamp-on-the-bedside-table` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 48 | 55 | 11.3 | 2 | 0 | 0 | 82/9845 (cache 1143681/86162) | $0.4284 | 0 | 0 |
| `make-a-cellar-map` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 30 | 30 | 8.0 | 0 | 0 | 0 | 50/6170 (cache 729150/102730) | $0.3915 | 0 | 0 |
| `move-the-locked-bed` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 23 | 24 | 4.3 | 0 | 0 | 0 | 40/4916 (cache 451462/83196) | $0.3024 | 0 | 0 |
| `pond-in-the-lowest-corner` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 48 | 65 | 9.7 | 1 | 1 | 0 | 72/12895 (cache 1418546/134313) | $0.6067 | 0 | 0 |
| `raise-a-hill-in-the-north-east` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 43 | 48 | 9.0 | 0 | 0 | 0 | 64/11956 (cache 1080534/126225) | $0.5433 | 0 | 0 |
| `refuse-a-piece-the-project-lacks` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 26 | 27 | 5.0 | 0 | 0 | 0 | 44/5712 (cache 654822/129161) | $0.4456 | 0 | 0 |
| `remove-both-crates` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 35 | 39 | 9.3 | 1 | 0 | 0 | 60/6228 (cache 816262/77482) | $0.3377 | 0 | 0 |
| `set-the-gates-health` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 28 | 31 | 5.0 | 0 | 0 | 0 | 56/5357 (cache 648368/69991) | $0.2935 | 0 | 0 |
| `take-back-the-last-change` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 25 | 32 | 4.3 | 0 | 3 | 0 | 38/4521 (cache 337227/42148) | $0.1844 | 0 | 0 |
| `turn-the-wardrobe-into-the-room` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 51 | 51 | 7.3 | 0 | 0 | 0 | 70/11142 (cache 1319665/140094) | $0.5938 | 0 | 0 |
| `work-the-open-note` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 56 | 77 | 16.0 | 0 | 0 | 0 | 102/13585 (cache 2134996/168266) | $0.7702 | 1 | 0 |

## Failed trials (dev tasks)

### `count-what-the-bedroom-holds`, trial 2

- first failing check: `handback_contains` — wanted a hand-back stating three, dresser; measured the hand-back \`Bedroom census: 3 pieces, dresser nearest north wall\` does not state \`three\`
- its last 3 call(s), oldest first:
  - `blueprint_guide {}`
  - `blueprint_state {}`
  - `blueprint_present {"check":[{"placements":["dresser"],"what":"The dresser sits closest to the north wall"}],"inferred":["The bedroom map is bench_bedroom, the map open in the edi…`

### `count-what-the-bedroom-holds`, trial 3

- first failing check: `handback_contains` — wanted a hand-back stating three, dresser; measured the hand-back \`Bedroom: 3 pieces, dresser nearest the north wall\` does not state \`three\`
- its last 3 call(s), oldest first:
  - `blueprint_guide {}`
  - `blueprint_state {}`
  - `blueprint_present {"check":[{"placements":["dresser"],"what":"The dresser, the piece nearest the north wall"}],"inferred":["North is -Z, as the map file's header says, so the nor…`

### `hedge-along-the-south-edge`, trial 2

- first failing check: `placements_within` — wanted 6 × a \`plant_b\`, all inside x -4.00..4.00, z 2.20..3.00; measured 0 × a \`plant_b\` on the map, 0 of them inside x -4.00..4.00, z 2.20..3.00, and the task wants 6 all inside
- its last 5 call(s), oldest first:
  - `blueprint_preview_operation null`
  - `blueprint_list_operations null`
  - `blueprint_list_pieces null`
  - `blueprint_preview_operation null`
  - `blueprint_preview_operation null`

## Where the agent stumbles (dev tasks)

- tools called most: `blueprint_apply_operation` ×97, `blueprint_list_operations` ×91, `blueprint_state` ×73, `blueprint_present` ×65, `blueprint_guide` ×62, `blueprint_capture_still` ×36, `blueprint_list_pieces` ×34, `blueprint_preview_operation` ×33
- refusals by tool: `blueprint_decide` ×16, `blueprint_apply_operation` ×10, `blueprint_preview_operation` ×4, `blueprint_present` ×1
- wishes, by the tool wished for: `A piece-facing lookup that returns a placement's world facing as a compass face, and the yaw that gives a requested face.` ×1
