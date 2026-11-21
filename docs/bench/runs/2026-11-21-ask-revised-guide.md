# Benchmark run: ask-revised-guide

| | |
|---|---|
| Editor and MCP server commit | `c763194ed68024180b0d0fe112ea388a7df1531f+dirty` |
| Shipped guide SHA-256 | `ae87fc6b12b0b00d224fcd66240696ef389a89d5838995298299f6498fa0143f` |
| Agent | omp omp/19.0.1 |
| Model | anthropic/claude-sonnet-5-5 |
| Variant | none |
| Trials per task | 3 |
| Suite | bench/tasks |
| Host | big |
| Date | 2026-11-21 |
| Price per million tokens | input $2, output $10, cache read $0.1, cache write $2.5 |

60 trial(s), 0 harness error(s). A harness error is never scored as a pass or a fail.

Pre-flight turn (one real model turn before any trial, in no trial's cost): 4 fresh in, 4 out, 0 cache read, 11585 cache written, $0.0290. **Cost of the run, trials and pre-flight turn: $10.7186.**

## The suite

| tasks | tasks scored | trials | successes | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|---|
| dev | 20 | 60 | 56 | 93% [83%, 100%] | 1:0.93 2:0.88 3:0.85 | $10.6896 | $0.1909 | 0 |

## By tag (dev tasks)

| tag | tasks | trials | mean success (95% interval) | mean pass^k | cost | cost per success | harness errors |
|---|---|---|---|---|---|---|---|
| compose | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.6969 | $0.2323 | 0 |
| data | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.2264 | $0.0755 | 0 |
| fix | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.2668 | $0.2111 | 0 |
| handback | 4 | 12 | 83% [50%, 100%] | 1:0.83 2:0.75 3:0.75 | $1.3028 | $0.1303 | 0 |
| maps | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.5153 | $0.1718 | 0 |
| mount | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.4414 | $0.1471 | 0 |
| notes | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.9352 | $0.3117 | 0 |
| place | 3 | 9 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.7371 | $0.1930 | 0 |
| read | 1 | 3 | 33% [0%, 100%] | 1:0.33 2:0.00 3:0.00 | $0.2104 | $0.2104 | 0 |
| refusal | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.7737 | $0.1290 | 0 |
| relations | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.2958 | $0.2160 | 0 |
| remove | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.4784 | $0.1595 | 0 |
| runs | 1 | 3 | 67% [0%, 100%] | 1:0.67 2:0.33 3:0.00 | $0.6450 | $0.3225 | 0 |
| terrain | 2 | 6 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $1.5091 | $0.2515 | 0 |
| tiles | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.6138 | $0.2046 | 0 |
| turn | 1 | 3 | 67% [0%, 100%] | 1:0.67 2:0.33 3:0.00 | $0.5901 | $0.2951 | 0 |
| undo | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.1726 | $0.0575 | 0 |
| verify | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.3186 | $0.1062 | 0 |
| water | 1 | 3 | 100% [100%, 100%] | 1:1.00 2:1.00 3:1.00 | $0.9436 | $0.3145 | 0 |

## By task (dev tasks)

| task | n | c | c/n | pass^k | median s | p90 s | calls | refused | undos | behind the editor's back | tokens in/out (cache read/write) | cost | wishes | harness errors |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `bed-against-the-north-wall` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 49 | 57 | 9.7 | 0 | 0 | 0 | 78/10830 (cache 1131028/119996) | $0.5215 | 0 | 0 |
| `chair-west-of-the-desk-facing-it` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 83 | 100 | 11.3 | 4 | 0 | 0 | 94/18269 (cache 1908588/160193) | $0.7742 | 0 | 0 |
| `clear-the-overlap-keep-both` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 43 | 56 | 16.0 | 8 | 0 | 0 | 116/14217 (cache 3061103/194876) | $0.9357 | 0 | 0 |
| `count-what-the-bedroom-holds` | 3 | 1 | 33% | 1:0.33 2:0.00 3:0.00 | 10 | 10 | 3.0 | 0 | 0 | 0 | 36/3215 (cache 318784/58535) | $0.2104 | 0 | 0 |
| `cover-the-hallway-floor` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 21 | 22 | 13.3 | 0 | 0 | 0 | 64/7311 (cache 1289048/164681) | $0.6138 | 0 | 0 |
| `fix-the-floating-lamp` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 21 | 24 | 7.3 | 0 | 1 | 0 | 60/6342 (cache 734967/77631) | $0.3311 | 0 | 0 |
| `furnish-a-reading-nook` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 41 | 42 | 14.7 | 7 | 0 | 0 | 90/12202 (cache 1824941/156883) | $0.6969 | 0 | 0 |
| `hedge-along-the-south-edge` | 3 | 2 | 67% | 1:0.67 2:0.33 3:0.00 | 38 | 39 | 13.0 | 3 | 0 | 0 | 86/12165 (cache 1653969/143116) | $0.6450 | 0 | 0 |
| `is-the-lamp-on-the-table` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 19 | 22 | 3.3 | 0 | 0 | 0 | 52/6617 (cache 602737/76837) | $0.3186 | 0 | 0 |
| `lamp-on-the-bedside-table` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 26 | 29 | 8.7 | 1 | 0 | 0 | 68/8581 (cache 1003293/102040) | $0.4414 | 0 | 0 |
| `make-a-cellar-map` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 27 | 37 | 9.0 | 0 | 0 | 0 | 62/8428 (cache 1042996/130651) | $0.5153 | 0 | 0 |
| `move-the-locked-bed` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 18 | 19 | 4.0 | 0 | 0 | 0 | 42/3916 (cache 470255/89726) | $0.3106 | 0 | 0 |
| `pond-in-the-lowest-corner` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 76 | 80 | 15.7 | 0 | 1 | 0 | 116/15881 (cache 2910002/197429) | $0.9436 | 0 | 0 |
| `raise-a-hill-in-the-north-east` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 46 | 52 | 13.3 | 2 | 0 | 0 | 92/12252 (cache 1465685/118488) | $0.5655 | 0 | 0 |
| `refuse-a-piece-the-project-lacks` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 22 | 23 | 5.3 | 0 | 0 | 0 | 46/5016 (cache 716492/136489) | $0.4631 | 0 | 0 |
| `remove-both-crates` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 28 | 28 | 9.0 | 0 | 0 | 0 | 68/7459 (cache 1088291/117930) | $0.4784 | 0 | 0 |
| `set-the-gates-health` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 14 | 16 | 5.3 | 0 | 0 | 0 | 58/4102 (cache 579539/50928) | $0.2264 | 0 | 0 |
| `take-back-the-last-change` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 9 | 14 | 4.0 | 0 | 3 | 0 | 38/3320 (cache 308139/43391) | $0.1726 | 0 | 0 |
| `turn-the-wardrobe-into-the-room` | 3 | 2 | 67% | 1:0.67 2:0.33 3:0.00 | 29 | 44 | 8.3 | 0 | 0 | 0 | 72/10608 (cache 1259527/143170) | $0.5901 | 0 | 0 |
| `work-the-open-note` | 3 | 3 | 100% | 1:1.00 2:1.00 3:1.00 | 45 | 59 | 12.7 | 0 | 0 | 0 | 88/16549 (cache 2167569/221125) | $0.9352 | 0 | 0 |

## Failed trials (dev tasks)

### `count-what-the-bedroom-holds`, trial 1

- first failing check: `handback_contains` — wanted a hand-back stating three, dresser; measured the hand-back \`Bedroom: 3 pieces; dresser is nearest the north wall\` does not state \`three\`
- its last 3 call(s), oldest first:
  - `blueprint_guide {}`
  - `blueprint_state {}`
  - `blueprint_present {"inferred":["North is -Z, from the map file's header comment, so the north wall is the z = -2.5 plane","Distance is measured from each piece's placement point …`

### `count-what-the-bedroom-holds`, trial 3

- first failing check: `handback_contains` — wanted a hand-back stating three, dresser; measured the hand-back \`Bedroom contents: 3 pieces, dresser nearest the north wall\` does not state \`three\`
- its last 3 call(s), oldest first:
  - `blueprint_guide {}`
  - `blueprint_state {}`
  - `blueprint_present {"inferred":["'Bedroom map' means the open map bench_bedroom","North wall is the z = -2.5 plane, from the map file's header comment","Distance is measured from …`

### `hedge-along-the-south-edge`, trial 2

- first failing check: `placements_within` — wanted 6 × a \`plant_b\`, all inside x -4.00..4.00, z 2.20..3.00; measured 0 × a \`plant_b\` on the map, 0 of them inside x -4.00..4.00, z 2.20..3.00, and the task wants 6 all inside
- its last 5 call(s), oldest first:
  - `blueprint_guide {}`
  - `blueprint_state {}`
  - `blueprint_list_operations {}`
  - `blueprint_list_operations {"operation":"placement_add"}`
  - `blueprint_preview_operation (refused) {"operation":"placement_add","parameters":{"at":[-1.25,2.6],"piece":"plant_b"}}`

### `turn-the-wardrobe-into-the-room`, trial 3

- first failing check: `placement_at` — wanted \`wardrobe\` at \(0.000, -2.200\) ±0.050 m and facing 90.0° ±10.00°; measured its centre is at \(0.000, -2.200\), 0.000 m from \(0.000, -2.200\); it faces 0.0°, 90.00° from 90.0° — facing is off by 90.00°, more than 10.00°
- its last 5 call(s), oldest first:
  - `blueprint_decide {"questions":{"faces":{"instructions":"The wardrobe's doors face into the room, away from the north wall.","type":"noul"},"wall":{"instructions":"The wardrobe's…`
  - `blueprint_capture_still {"label":"wardrobe_after"}`
  - `blueprint_apply_operation {"operation":"placement_set_yaw","parameters":{"id":"wardrobe","yaw":0}}`
  - `blueprint_capture_still {"label":"wardrobe_yaw0"}`
  - `blueprint_present {"check":[{"placements":["wardrobe"],"what":"The wardrobe's fronts face +z (open floor) at yaw 0, and it is still at (0.0, -2.2) against the north wall"}],"infe…`

## Where the agent stumbles (dev tasks)

- tools called most: `blueprint_apply_operation` ×106, `blueprint_state` ×66, `blueprint_list_operations` ×64, `blueprint_guide` ×63, `blueprint_present` ×62, `blueprint_decide` ×50, `blueprint_capture_still` ×40, `blueprint_preview_operation` ×39
- refusals by tool: `blueprint_preview_operation` ×11, `blueprint_apply_operation` ×9, `blueprint_decide` ×2, `blueprint_capture_still` ×1, `blueprint_present` ×1, `blueprint_style` ×1
- wishes, by the tool wished for: none
