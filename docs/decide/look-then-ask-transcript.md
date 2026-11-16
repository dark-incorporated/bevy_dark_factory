# Look, then ask: a live transcript (#456, 2026-11-15)

What an agent does after a visible change: take one still, ask the decision model about it, and hand back with the answer.
Recorded on big against the gate build of main `c21f8ff3`, a project served by `bevy_cauldron_mcp --project <project>` attached to a
running editor (Xvfb), the Local decision model (d3-flash at `http://big.local:8081/upstream/d3-flash/v1/systemone`).

## 1. The calls, made by hand through the MCP server

`grimoire_capture_still {"label": "after_change"}` (0.17 s) answered `fingerprint: 9de91617633d0307`, `bytes: 345656`, `source: surface`,
with the editor's state beside it.

`grimoire_decide` with the still given by path (no `view: true`, which cannot run while a capture sequence is recording) and three typed questions:

```json
{
 "state": "A map with two wooden chairs placed on a grid floor.",
 "images": [
  {
   "name": "view",
   "path": "/home/ladvien/clip/work/k5/proj/target/loop/captures/after_change/still-1794793642178.png"
  }
 ],
 "questions": {
  "chairs_visible": {
   "type": "noul",
   "instructions": "Is at least one wooden chair visible in the picture?"
  },
  "something_floating": {
   "type": "noul",
   "instructions": "Is anything in the picture floating above the floor?"
  },
  "blank": {
   "type": "noul",
   "instructions": "Is the picture blank or one flat colour?"
  }
 }
}
```

Answer (first call after the model was loaded: 9.1 s; warm calls: 1.1 to 1.8 s on this 1088 x 640, 345 KB still):

```json
{
 "answers": {
  "blank": {
   "noul": 0.009410909138304257,
   "type": "noul"
  },
  "chairs_visible": {
   "noul": 0.984160839187657,
   "type": "noul"
  },
  "something_floating": {
   "noul": 0.17907222017820745,
   "type": "noul"
  }
 },
 "endpoint": "http://big.local:8081/upstream/d3-flash/v1/systemone",
 "latency_ms": 1653,
 "model": "d3-flash",
 "model_source": "reported",
 "where": "local"
}
```

`where: local`: the picture did not leave the LAN. `model` and `model_source` name what answered; `latency_ms` is the round trip.

## 2. The live agent: omp, asked "make a small forest clearing"

omp (the Anthropic login on big) was started with `omp -p --no-session --cwd <project>` and the project's `.omp/mcp.json` naming the MCP server
(the editor's own self test is a different path). The prompt was: *make a small forest clearing on this map with the grimoire tools (if this build has
no tree tools, clear a small open area among wall pieces instead); before you answer, check your work the way the Cauldron guide says (look at a fresh
still, then ask the decision model), and tell me which tool calls you made and what the model answered.* The `(if ...)` clause is there because the gate
build has no `bevy_flora`: omp's own first attempt was refused with *"this build has no forest: rebuild the editor with `--features bevy_flora`"*.
omp's own report, verbatim (its print mode shows the final message, not the call log; the calls are the ones it lists):

Working...
This build has no tree tools, so I used your fallback. I left a small open area of about 9 m on the `courtyard` map with wall pieces in a rough ring around it, centred near (0.5, −4.5). The decision model agreed it is a ring around an open middle.

**What I built:** five new wall pieces (`wall@4` to `wall@8`) at (4,−1), (4,−8), (1,−9), (−3,−7) and (−3,−1). Together with the three walls already on the map, they make a ring of eight with nothing inside it. The edit checks found no overlaps or other faults. I kept the ring north of the south-west market. In this project the "wall" piece is drawn as a chair, so on screen it is a ring of chairs. The editor also now flags the eight identical walls as repetition, because they all face the same way.

**What the model answered** (d3-flash, running locally, 2.2 s), looking at a fresh still with the whole ring in frame:

| Question | Answer |
|---|---|
| The pieces form a rough ring around an open space | 0.97 |
| The middle of the ring is empty, with no piece in it | 0.91 |
| Size of the open space | "small" 0.97 (none 0.01, huge 0.02) |
| No piece floats above the ground | 0.97 |
| The whole ring is visible | 0.82 |

**Tool calls, in order:**
1. `grimoire_guide`
2. `grimoire_state`, `grimoire_project_feedback_list` (it had no reports), and `grimoire_list_operations` searching for tree, forest and clear.
3. `grimoire_list_operations` for `forest_scatter`, then `grimoire_read_oplog`, `grimoire_list_pieces` (only `floor` and `wall` exist) and `grimoire_style` for courtyard.
4. `grimoire_capture_still` named `clearing_before`.
5. `grimoire_preview_operation` for `forest_scatter`. It was refused: the editor was built without its forest feature (`bevy_flora`), so I switched to the fallback.
6. Five `grimoire_apply_operation` calls with `placement_add`, operations `0002-4005` to `0006-4005`.
7. `grimoire_capture_still` named `clearing_after`. It still showed one chair close up, so it couldn't show the ring.
8. `grimoire_frame_view` on the eight walls, then `grimoire_capture_still` named `clearing_after_framed`. The guide says not to frame the view after a change, but the editor's own camera move hadn't framed the ring, and I needed it in view to check it.
9. `grimoire_decide` with the five questions above.
10. `grimoire_present`. The first call was refused: it said the courtyard style page hadn't been served to me in this session, but step 3 had served it. That looks like a bug. I called `grimoire_style` again with `at` and the second `grimoire_present` was accepted as `present-1`.
11. `grimoire_wish` (`wish-1`): a still that frames the placements from given operations by itself, so step 8 isn't needed.

The result is waiting in the editor: Keep it, or Undo all to remove the five new walls.

The numbers in that table are the `answers` of omp's `grimoire_decide` call; its 2.2 s is the `latency_ms` that call reported (d3-flash, `where: local`).

## 3. Numbers to read it against

| Measure | Value |
|---|---|
| DM-01, 3 questions, one still, from big | median 397 ms |
| DM-01, the same from mac_air | median 651 ms |
| bench457 (BenchBase), answered `grimoire_decide` calls | median 543 ms, p90 679 ms, max 753 ms over 39 answered calls |
| This recording, warm, 1088 x 640 still (345 KB), 3 questions | 1.14 to 1.83 s (4 calls: 1.83, 1.15, 1.16, 1.12 s round trip at the MCP client; model `latency_ms` 1141 to 1827) |
| omp's live call, 5 questions, a framed still | 2.2 s |
| First call after the model was evicted | 9.1 s (it loads on demand; olmocr must be `ready` beside it) |

The recording numbers are larger than the benchmark's: they carry a real 1088 x 640 editor still and the editor was running beside the model on the same card.

## 4. What this shows, and what it does not

- The path works end to end: capture_still, decide with the still by path, a probability for every answer, the model's name, where it ran and the latency.
- omp took the look-then-ask path on its own once told to check its work, and handed back with `grimoire_present`.
- Two things omp met that are not in this issue: its first `grimoire_present` was refused ("the style page was not served this session") although it had
  called `grimoire_style` earlier in the run; a second `grimoire_style` with `at` cleared it. And the gate build has no forest tools.
