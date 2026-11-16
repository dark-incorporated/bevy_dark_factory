# Benchmark run: ask-with-guide

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
| Date | 2026-11-15 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

60 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

## Agent errors

4 call(s) to a tool that does not exist in `bed-against-the-north-wall` trial 2, `clear-the-overlap-keep-both` trial 1, `set-the-gates-health` trial 1, `set-the-gates-health` trial 2. The agent was told so by its harness and went on, so each trial is scored: these are the agent's own mistakes and not the harness's. By the name called: `xd__mcp__cauldron_grimoire_guide` ×1, `xd__read` ×1, `xd_read` ×1, `xd_read_placeholder` ×1.

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 20 | 60 | 55 | 92% [78%, 100%] | 1:0.92 2:0.88 3:0.85 | $10.2682 | $0.1867 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| compose | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.5718 | $0.1906 | 0 |
| data | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.2743 | $0.0914 | 0 |
| fix | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.0974 | $0.1829 | 0 |
| handback | 4 | 12 | 75% [25%, 100%] | 1:0.75 2:0.75 3:0.75 | $1.4677 | $0.1631 | 0 |
| maps | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.4105 | $0.1368 | 0 |
| mount | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.5846 | $0.1949 | 0 |
| notes | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.8355 | $0.2785 | 0 |
| place | 3 | 9 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.8650 | $0.2072 | 0 |
| read | 1 | 3 | 0% [0%, 0%] | 1:0.00 2:0.00 3:0.00 | $0.2264 | unknown | 0 |
| refusal | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.8910 | $0.1485 | 0 |
| relations | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.2803 | $0.2134 | 0 |
| remove | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.4092 | $0.1364 | 0 |
| runs | 1 | 3 | 67% [0%, 100%] | 1:0.67 2:0.33 3:0.00 | $0.7560 | $0.3780 | 0 |
| terrain | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.2023 | $0.2004 | 0 |
| tiles | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.6177 | $0.2059 | 0 |
| turn | 1 | 3 | 67% [0%, 100%] | 1:0.67 2:0.33 3:0.00 | $0.5480 | $0.2740 | 0 |
| undo | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.2127 | $0.0709 | 0 |
| verify | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.3503 | $0.1168 | 0 |
| water | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.6809 | $0.2270 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `bed-against-the-north-wall` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 40 | 54 | 10.0 | 2 | 0 | 0 | 72/9454 (cache 1523449/139593) | $0.5960 | 0 | 0 |
| `chair-west-of-the-desk-facing-it` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 58 | 60 | 9.7 | 0 | 0 | 0 | 80/13082 (cache 1654375/155158) | $0.6843 | 0 | 0 |
| `clear-the-overlap-keep-both` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 60 | 80 | 11.0 | 2 | 0 | 0 | 86/9936 (cache 1854643/167409) | $0.7035 | 0 | 0 |
| `count-what-the-bedroom-holds` | 3 | 0 | 0% | 1:0.00 2:0.00 3:0.00 | 24 | 44 | 3.0 | 0 | 0 | 0 | 40/4185 (cache 394508/58019) | $0.2264 | 0 | 0 |
| `cover-the-hallway-floor` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 35 | 39 | 9.3 | 0 | 0 | 0 | 70/7948 (cache 1417729/158509) | $0.6177 | 0 | 0 |
| `fix-the-floating-lamp` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 34 | 37 | 7.3 | 0 | 1 | 0 | 64/8145 (cache 856846/90639) | $0.3939 | 0 | 0 |
| `furnish-a-reading-nook` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 43 | 44 | 13.0 | 3 | 0 | 0 | 70/11441 (cache 1226400/133862) | $0.5718 | 0 | 0 |
| `hedge-along-the-south-edge` | 3 | 2 | 67% | 1:0.67 2:0.33 3:0.00 | 70 | 76 | 15.3 | 4 | 0 | 0 | 86/17076 (cache 1770561/163217) | $0.7560 | 0 | 0 |
| `is-the-lamp-on-the-table` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 36 | 40 | 3.7 | 0 | 0 | 0 | 58/7405 (cache 741917/80791) | $0.3503 | 0 | 0 |
| `lamp-on-the-bedside-table` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 49 | 50 | 10.3 | 0 | 0 | 0 | 70/10671 (cache 1279103/139949) | $0.5846 | 0 | 0 |
| `make-a-cellar-map` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 31 | 33 | 7.7 | 0 | 0 | 0 | 54/6383 (cache 805486/106421) | $0.4105 | 0 | 0 |
| `move-the-locked-bed` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 20 | 23 | 4.3 | 0 | 0 | 0 | 44/4729 (cache 522274/87070) | $0.3173 | 0 | 0 |
| `pond-in-the-lowest-corner` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 55 | 59 | 10.3 | 0 | 0 | 0 | 86/14451 (cache 1697726/146595) | $0.6809 | 0 | 0 |
| `raise-a-hill-in-the-north-east` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 40 | 44 | 9.7 | 0 | 0 | 0 | 68/11182 (cache 1144754/117962) | $0.5213 | 0 | 0 |
| `refuse-a-piece-the-project-lacks` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 34 | 36 | 6.3 | 0 | 0 | 0 | 54/7169 (cache 994102/160991) | $0.5737 | 0 | 0 |
| `remove-both-crates` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 31 | 33 | 7.7 | 0 | 0 | 0 | 64/7103 (cache 943663/97466) | $0.4092 | 0 | 0 |
| `set-the-gates-health` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 25 | 25 | 4.7 | 0 | 0 | 0 | 60/4511 (cache 686218/64199) | $0.2743 | 0 | 0 |
| `take-back-the-last-change` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 22 | 24 | 4.7 | 0 | 3 | 0 | 44/4245 (cache 420922/51228) | $0.2127 | 0 | 0 |
| `turn-the-wardrobe-into-the-room` | 3 | 2 | 67% | 1:0.67 2:0.33 3:0.00 | 49 | 53 | 8.3 | 0 | 0 | 0 | 76/10517 (cache 1391407/121429) | $0.5480 | 0 | 0 |
| `work-the-open-note` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 63 | 73 | 15.3 | 0 | 0 | 0 | 100/13597 (cache 2381786/184467) | $0.8355 | 1 | 0 |

## Failed trials (dev tasks)

### `count-what-the-bedroom-holds`, trial 1

- first failing check: `handback_contains` — wanted a hand-back stating three, dresser; measured the hand-back \`Bedroom census: 3 pieces, dresser nearest the north wall\` does not state \`three\`
- its last 3 call(s), oldest first:
  - `grimoire_guide {}`
  - `grimoire_state {}`
  - `grimoire_present {"check":[{"placements":["dresser"],"what":"The dresser is the piece closest to the north wall"}],"inferred":["'The bedroom map' is the open map bench_bedroom (…`

### `count-what-the-bedroom-holds`, trial 2

- first failing check: `handback_contains` — wanted a hand-back stating three, dresser; measured the hand-back \`Bedroom: 3 pieces, dresser nearest the north wall\` does not state \`three\`
- its last 3 call(s), oldest first:
  - `grimoire_guide {}`
  - `grimoire_state {}`
  - `grimoire_present {"check":[],"inferred":["North is -Z, as the map file's header says, so the north wall is the z = -2.5 plane","Each placement's 'at' is [x, z] in map metres"],"…`

### `count-what-the-bedroom-holds`, trial 3

- first failing check: `handback_contains` — wanted a hand-back stating three, dresser; measured the hand-back \`Bedroom census: 3 pieces, dresser nearest the north wall\` does not state \`three\`
- its last 3 call(s), oldest first:
  - `grimoire_guide {}`
  - `grimoire_state {}`
  - `grimoire_present {"check":[{"placements":["dresser"],"what":"The dresser sits closest to the north wall (z = -2.0 vs wall at z = -2.5)"}],"inferred":["'The bedroom map' is bench…`

### `hedge-along-the-south-edge`, trial 3

- first failing check: `placements_within` — wanted 6 × a \`plant_b\`, all inside x -4.00..4.00, z 2.20..3.00; measured 0 × a \`plant_b\` on the map, 0 of them inside x -4.00..4.00, z 2.20..3.00, and the task wants 6 all inside
- its last 5 call(s), oldest first:
  - `grimoire_style {}`
  - `grimoire_list_pieces {}`
  - `grimoire_preview_operation (refused) {"operation":"placement_add","parameters":{"at":[-2.5,2.6],"piece":"plant_b"}}`
  - `grimoire_list_operations {"query":"placement"}`
  - `grimoire_preview_operation (refused) {"operation":"placement_add","parameters":{"at":[-2.5,2.6],"piece":"plant_b","replace":true}}`

### `turn-the-wardrobe-into-the-room`, trial 3

- first failing check: `oplog_has` — wanted at least 1 applied \`placement_set_yaw\` line\(s\); measured 0 applied \`placement_set_yaw\` line\(s\), and the task wants at least 1
- its last 5 call(s), oldest first:
  - `grimoire_capture_still {"label":"before_wardrobe"}`
  - `grimoire_preview_operation {"operation":"placement_set_pose","parameters":{"id":"wardrobe","tip":[0,0],"yaw":90}}`
  - `grimoire_apply_operation {"operation":"placement_set_pose","parameters":{"id":"wardrobe","tip":[0,0],"yaw":90}}`
  - `grimoire_decide {"questions":{"faces":{"instructions":"The tall dresser's doors/drawer fronts face into the room, away from the wall.","type":"noul"},"stays":{"instructions":"T…`
  - `grimoire_present {"check":[{"placements":["wardrobe"],"what":"Doors face the room (south) while the wardrobe stays flush on the north wall"}],"inferred":["The descriptor's front…`

## Where the agent stumbles (dev tasks)

- tools called most: `grimoire_apply_operation` ×82, `grimoire_list_operations` ×73, `grimoire_state` ×65, `grimoire_guide` ×61, `grimoire_present` ×60, `grimoire_decide` ×38, `grimoire_preview_operation` ×36, `grimoire_list_pieces` ×32
- refusals by tool: `grimoire_preview_operation` ×7, `grimoire_style` ×3, `grimoire_run_command` ×1
- wishes, by the tool wished for: `A read-only tool that gives a placement's world-facing compass side, and the yaw that would put its front on a named side.` ×1
