# Does the style help an agent? The replication, on the fixed runner (SG-10, #409)

This is the replication of the run in `docs/research/2026-11-14-style-helps-an-agent-results.md`, which stays as it is and stays in the record. **That first run lost 58% of its without-style arm (14 of 24 runs) to a harness error, and its scored cells were 2, 7 and 1 runs; this replication lost none, scored all 30 registered runs, and is the stronger evidence.**

**Registration:** `docs/research/2026-11-14-style-helps-an-agent.md` at `137f717ac1cad3de6e4dfa1c98ea847fc474a59f`; the registered analysis script is unedited. **Run from:** main `dc3a2d26ed55f3d4e64ba52352e60c31fe73a15b`, editor, MCP server and `dark_factory-bench` built there on host `bmb` (macOS), 2026-11-14, `anthropic/claude-sonnet-5-5` for both arms and the probe. The suite, tasks, fixture, registration and registered script have no diff against `137f717a`; the runner moved (BM-11, #441: a tool name omp refuses is the agent's mistake and is scored, a real leak stays a harness error, and an editor tool called by its own direct name `mcp__dark_factory_blueprint_*` is counted in `tool_log` with its arguments).

## What was run

Step 0 (rehearsal with `oracle`, two rounds, assemble, report, compare, the script): passed, 24 of 24 lines in both arms, `compare` no measurable change, header `trials: 2`. Step 1: `nothing` failed 3 of 3 tasks (5 of 11, 1 of 11 and 3 of 7 checks held), `oracle` passed 3 of 3 with no variant and under each variant. The probe (`rep-probe`, unscored): both pages served, `blueprint_style` called with `{"map":"courtyard"}`, and the agent also read `design/style.md` and `design/levels/courtyard/style.md` from disk, so route `both`, which is what the script printed and what the transcript's calls show. The 30 registered runs: five rounds, each task once in each arm, the arm order alternating, `--trials 1 --jobs 3`. **There were no harness errors and no make-up rounds.** Cost: 7.28 USD for the probe and the 30 runs (the 15 USD guard was not approached). Conditions as in the first run: omp's `eval` disabled in both arms by an `omp` wrapper adding a config overlay (`eval.py` and `eval.js` false), the trial's trust gate opened by an `--editor` wrapper that writes the trial HOME's own trust store (macOS), a dev-profile build with no debug info, `--jobs 3`.

## The registered analysis script, unedited, and the second script

The first block is the registered script's output verbatim. The second block is a **second, post-hoc script** (`docs/research/2026-11-14-style-helps-an-agent-analyse-2.py`, with its own selftest). **It is a post-hoc amendment, made after the first run's data had been seen** (the first run is in the record with its 58% harness-error attrition, and the registered script reads only the `xd://` route, so a direct-name editor call would be labelled `neither`). It imports the registered script's statistic and does not re-implement it. It differs in exactly three ways: tool use is read from `trial.json` `tool_log` (every editor call the runner saw, `xd://` and direct), a `followed` citation is read from the agent's own `blueprint_present` call through either route, and it reports `agent_errors` per arm with a Fisher exact test. A design page opened by `read`/`bash` is still read from the transcript with the registered reader.

```
##### REGISTERED SCRIPT, UNEDITED
harness errors (not scored): 0
dress-the-market-area: without 4.40/9  with 9.00/9  (5 and 5 runs)
light-the-gate-at-night: without 3.00/9  with 9.00/9  (5 and 5 runs)
write-the-gatekeepers-greeting: without 3.00/6  with 6.00/6  (5 and 5 runs)
lines satisfied per task-triple (max 24): without 10.40  with 24.00
PRIMARY  difference with - without: +13.60  95% bootstrap interval [+13.20, +14.00]  ->  better
second reading, exact permutation test (arm labels exchanged within a task), two-sided p = 0.0000
route of the with-style runs: tool 10, file 0, both 4, neither 1  (called blueprint_style, with or without a page: 93%; the tool alone, no page: 67%; needed 70% of the first)
  D over the 10 with-style runs by route `tool` (3 tasks): +13.60  [+13.20, +14.00]  (descriptive, a thin cell)
  D over the 4 with-style runs by route `both` (3 tasks): +13.60  [+13.20, +14.00]  (descriptive, a thin cell)
  D over the 1 with-style runs by route `neither` (1 tasks): +3.00  [+3.00, +3.00]  (descriptive, a thin cell)
CLAIM: `blueprint_style` changes what an agent builds: the tool share was met and D is better
with-style runs that called blueprint_style before their first edit: 14 of 15
with-style runs whose own blueprint_present call cited followed lines: 15 of 15
without-style runs offered blueprint_style (must be 0): 0
without-style runs that tried the hidden blueprint_style by its device path: 0 of 15 (0 writes, 0 reads)
wishes made by with-style runs: 2
  - dress-the-market-area {"after": {"presentation": "present-1"}, "example_call": "blueprint_preview_operation {\"operation\":\"placement_add\",\"parameters\":{\"piece\":\"awning_canvas\",\"at\":[-8.85,9.0],\"yaw\":180}}", "hardship": "placement_add silently replaced a stall when I put an awning at the same spot; I only found out from the result line and had to undo. I also could not tell whether a piece can be lifted overhead, and could not set lantern light because bevy_lights is not in family.yaml.", "task": "dressed the market rectangle with three stalls, awnings, lanterns and a few props", "tool": "A blueprint_preview_operation report for placement_add that lists which existing placements the new piece would replace or overlap, before anything is written."}
  - write-the-gatekeepers-greeting {"after": {"presentation": "present-1"}, "example_call": "blueprint_list_operations {\"name\": \"data_set\"}", "hardship": "blueprint_style refused topic `dialogue` (only `style` exists) after I guessed it; the data_set parameters were buried in a 29KB operations list I had to grep from an artifact", "task": "wrote the gatekeeper's greeting into dialogue/gatekeeper", "tool": "blueprint_list_operations with a name filter, returning only the named operation's schema."}

=== SECOND (post-hoc) ANALYSIS: tool use read from trial.json tool_log, direct and xd:// calls ===
harness errors (not scored): 0
dress-the-market-area: without 4.40/9  with 9.00/9  (5 and 5 runs)
light-the-gate-at-night: without 3.00/9  with 9.00/9  (5 and 5 runs)
write-the-gatekeepers-greeting: without 3.00/6  with 6.00/6  (5 and 5 runs)
lines satisfied per task-triple (max 24): without 10.40  with 24.00
PRIMARY  difference with - without: +13.60  95% bootstrap interval [+13.20, +14.00]  ->  better
second reading, exact permutation test (arm labels exchanged within a task), two-sided p = 0.0000
route of the with-style runs: tool 11, file 0, both 4, neither 0  (called blueprint_style, with or without a page: 100%; the tool alone, no page: 73%; needed 70% of the first)
  D over the 11 with-style runs by route `tool` (3 tasks): +13.60  [+13.20, +14.00]  (descriptive, a thin cell)
  D over the 4 with-style runs by route `both` (3 tasks): +13.60  [+13.20, +14.00]  (descriptive, a thin cell)
CLAIM: `blueprint_style` changes what an agent builds: the tool share was met and D is better
with-style runs that called blueprint_style before their first edit: 15 of 15
with-style runs whose own blueprint_present call cited followed lines: 15 of 15
without-style runs offered blueprint_style (must be 0): 0
without-style runs that tried the hidden blueprint_style by its device path: 0 of 15 (0 writes, 0 reads)
wishes made by with-style runs: 2
  - dress-the-market-area {"after": {"presentation": "present-1"}, "example_call": "blueprint_preview_operation {\"operation\":\"placement_add\",\"parameters\":{\"piece\":\"awning_canvas\",\"at\":[-8.85,9.0],\"yaw\":180}}", "hardship": "placement_add silently replaced a stall when I put an awning at the same spot; I only found out from the result line and had to undo. I also could not tell whether a piece can be lifted overhead, and could not set lantern light because bevy_lights is not in family.yaml.", "task": "dressed the market rectangle with three stalls, awnings, lanterns and a few props", "tool": "A blueprint_preview_operation report for placement_add that lists which existing placements the new piece would replace or overlap, before anything is written."}
  - write-the-gatekeepers-greeting {"after": {"presentation": "present-1"}, "example_call": "blueprint_list_operations {\"name\": \"data_set\"}", "hardship": "blueprint_style refused topic `dialogue` (only `style` exists) after I guessed it; the data_set parameters were buried in a 29KB operations list I had to grep from an artifact", "task": "wrote the gatekeeper's greeting into dialogue/gatekeeper", "tool": "blueprint_list_operations with a name filter, returning only the named operation's schema."}
agent errors, without-style: 3 of 15 scored runs had at least one (3 calls); direct-name editor calls counted: 0; tools refused: xd___mcp__dark_factory_blueprint_guide, xd__mcp__dark_factory_blueprint_guide
agent errors, with-style: 3 of 15 scored runs had at least one (3 calls); direct-name editor calls counted: 7; tools refused: xd__mcp__dark_factory_blueprint_guide
Fisher exact, runs with an agent error (with-style 3 of 15 against without-style 3 of 15), two-sided p = 1.0000
harness errors (not scored): without-style 0, with-style 0
```

**What the two blocks say.** D, the lines satisfied per run summed over the three tasks, is **+13.60** (interval +13.20 to +14.00, exact permutation p below 0.0001): 24.0 of 24 lines with the style and 10.4 without, in both scripts (they read the same checks). They differ only in the route. The registered script, reading `xd://` only, put one with-style run (round 4, the greeting task, which called `blueprint_style` and `blueprint_apply_operation` by their direct names) in `neither`, and so reports 93% called the tool and 14 of 15 before the first edit; the post-hoc script reads `tool_log` and reports 100% (11 tool alone, 4 tool and a page, 0 pages alone, 0 neither), 15 of 15 before the first edit and 15 of 15 citing `followed` lines. **The claim rule is met on both readings:** at least 70% of with-style runs called `blueprint_style` (93% and 100%; the tool alone, 67% and 73%) and the verdict is `better`. The published `dark_factory-bench compare` (the registered second reading, now producible) says `better`: every task passed 0 of 5 without the style and 5 of 5 with it, and agent errors were 3 without and 3 with.

## Reading, honestly

1. **Replicated, and on a cleaner measurement.** All 15 with-style runs satisfied every line (9, 9 and 6), and the 15 without-style runs satisfied 4.4 of 9, 3.0 of 9 and 3.0 of 6; the rule is met with all 30 runs scored and the run-1 result (D +13.50, 24 against 10.5) is reproduced within 0.1 line. No tool claim rests on a thin cell this time: the cells are 5 and 5 in every task.
2. **The mangled tool name is not a with-style/without-style effect.** In the first run the model called a tool named `xd__mcp__dark_factory_blueprint_guide` (omp's `xd://mcp__…` route written as a function name) and the old runner counted that as a harness error, which hit 14 of 24 without-style runs against 4 of 24 with-style (Fisher exact two-sided p = 0.0065, from the first run's records). On the fixed runner it is an agent error and the trial is scored: **3 of 15 without-style runs and 3 of 15 with-style runs had one (Fisher p = 1.0)**, every one on the greeting task in rounds 3 to 5, every one the agent's first call (the guide read), and all recovered from. So the first run's unequal attrition was the old runner's accounting of the same model habit, applied to arms that do not differ in it here; the habit exists in both arms equally. It is still a finding about the product: the model writes the device route as a function name, here in 6 of the 10 greeting runs (rounds 3 to 5) and in no run of the other two tasks.
3. **The limits the registration stated still hold.** The without-style arm cannot know the style's numbers and words, and the with-style arm is told by the shipped guide to call `blueprint_style` and cite what it followed, so 15 of 15 on both is the guide at work as much as a finding; the style was written to be scored, and the with-style arm sits at the ceiling. Whether it helps a person's game is not measured. A do-nothing run satisfies 9 of 24 lines and the without-style arm 10.4.
4. **The wishes** are two, both about the product's tools and not about the style: `placement_add` silently replaced an overlapping stall (again), and the data operation's parameters were buried in a 29 KB operations list; one wish also says `blueprint_style` refused the topic `dialogue` after the agent guessed it, because only `style` exists, a small signal about the topic arguments and not about the style's text. They give no signal for changing the style templates (SG-08). No without-style run tried to call the hidden tool (0 of 15).
5. **The arms still differ in two things** (the tool and the files), as the registration said. Here no run used the files alone, so the route stratification cannot separate them: the 4 runs that also opened a page scored the same as the 11 that only called the tool (+13.60 each), and arm A' (pages on disk, tool hidden) is still the follow-up that would tell the tool from the files.

## Deviations from the registration

1. The runner is main's `dc3a2d26`, not the registration commit (`137f717ac`): BM-11 (#441) changes how a refused tool name and a direct-name call are recorded; the arms, tasks, checks, model and the registered analysis are unchanged.
2. `eval` disabled in both arms, the trust gate opened by an editor wrapper, a dev-profile build without debug info, on `bmb` (all as in the first run's deviations 2 to 4; the wrappers are in the data directory).
3. The second, post-hoc script and its reading above (a post-hoc amendment, labelled).
4. No make-up rounds (none needed). Step 2's `XDG_DATA_HOME` store is replaced by the wrapper, as in the first run.

## Data

`docs/research/data/2026-11-14-style-helps-an-agent-replication/` holds every run's `trial.json` and `run.json` (30 trials in 10 runs, and the probe), the analysis output, the logs and the wrappers. Transcripts (about 290 MB) are not committed; they are on bmb under `~/.local/state/dark_factory-bench/rep-r*/`. The published reports are `docs/bench/runs/2026-11-14-rep-{with,without}-style.{md,json}`.
