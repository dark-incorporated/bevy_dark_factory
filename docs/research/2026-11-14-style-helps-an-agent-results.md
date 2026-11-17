# Does the style help an agent? Results of the pre-registered measurement on the sample game (SG-10, #409)

**Registration:** `docs/research/2026-11-14-style-helps-an-agent.md` at `137f717ac1cad3de6e4dfa1c98ea847fc474a59f` (on main since the merge `058ac9df`; the file and its analysis script are unchanged in the commit run from). **Run from:** `c16441584c0eef5cc03aff187ad02124bfa71b28` (main), editor, MCP server and `dark_factory-bench` built at that commit on host `bmb` (macOS, Apple silicon), 2026-11-14, model `anthropic/claude-sonnet-5-5` for both arms and the probe. The registered analysis script was run unedited on all runs, and its output is below verbatim.

## What was run

- Step 0 (rehearsal with `oracle`, two rounds, assemble, report, compare, the script): passed. Both arms scored 24 of 24 lines, `compare` said no measurable change, the assembled `run.json` said `trials: 2`. The script's selftest passed.
- Step 1: `--agent nothing` failed 3 of 3 tasks (the greeting 3 of 7 checks held) and `--agent oracle` passed 3 of 3 with no variant and under each variant, on the real editor, at the runner above.
- Step 3, the probe (`style-probe`, unscored, with-style, `light-the-gate-at-night`): both pages were served (no `withheld` anywhere in the transcript), `blueprint_style` was called twice by `write xd://mcp__dark_factory_blueprint_style` (once bare, once with `{"map":"courtyard"}`), no design page was opened by `read`/`bash`/`grep`, and the agent's `blueprint_present` carried a `followed` list. Read by hand against the transcript, that is route `tool`, `followed` true, which is what `analyse.py --routes` printed. The 46 tools it was offered held no `eval`.
- Step 4, the 30 registered runs: rounds 1 to 5, each task once in each arm, the arm order alternating, `--trials 1 --jobs 3`.
- **13 of the 30 registered runs were harness errors** (without-style 10 of 15, with-style 3 of 15), so the registration's unfit-arm rule applied (more than 10%). **Rounds 6 to 8 were run as the make-up rounds the registration names**, same conditions, alternating the arm order: 18 of the 48 runs are harness errors in all (without-style 14 of 24, with-style 4 of 24).
- Cost: **7.38 USD** for the probe, the 48 runs and 17 pre-flight turns (the registered estimate was 3.7 to 9 USD, and the new runner's pre-flight turn costs about 0.03 USD per invocation, 0.5 USD in all). The 15 USD guard was not approached.

## The analysis script's output, verbatim

**Rounds 1 to 5 (the 30 registered runs):**

```
harness errors (not scored): 13
UNFIT, left out of both arms (a task with no scored run in an arm): dress-the-market-area
light-the-gate-at-night: without 3.00/9  with 9.00/9  (4 and 5 runs)
write-the-gatekeepers-greeting: without 3.00/6  with 6.00/6  (1 and 3 runs)
lines satisfied per task-triple (max 15): without 6.00  with 15.00
PRIMARY  difference with - without: +9.00  95% bootstrap interval [+9.00, +9.00]  ->  better
second reading, exact permutation test (arm labels exchanged within a task), two-sided p = 0.0020
route of the with-style runs: tool 8, file 0, both 0, neither 0  (called blueprint_style, with or without a page: 100%; the tool alone, no page: 100%; needed 70% of the first)
  D over the 8 with-style runs by route `tool` (2 tasks): +9.00  [+9.00, +9.00]  (descriptive, a thin cell)
CLAIM: `blueprint_style` changes what an agent builds: the tool share was met and D is better
with-style runs that called blueprint_style before their first edit: 8 of 8
with-style runs whose own blueprint_present call cited followed lines: 8 of 8
without-style runs offered blueprint_style (must be 0): 0
without-style runs that tried the hidden blueprint_style by its device path: 0 of 5 (0 writes, 0 reads)
wishes made by with-style runs: 0
```

**Rounds 1 to 8 (the 30 registered runs and the three make-up rounds), the primary reading of this report:**

```
##### ALL 8 ROUNDS (5 registered + 3 make-up)
harness errors (not scored): 18
dress-the-market-area: without 4.50/9  with 9.00/9  (2 and 7 runs)
light-the-gate-at-night: without 3.00/9  with 9.00/9  (7 and 8 runs)
write-the-gatekeepers-greeting: without 3.00/6  with 6.00/6  (1 and 5 runs)
lines satisfied per task-triple (max 24): without 10.50  with 24.00
PRIMARY  difference with - without: +13.50  95% bootstrap interval [+13.00, +14.00]  ->  better
second reading, exact permutation test (arm labels exchanged within a task), two-sided p = 0.0000
route of the with-style runs: tool 17, file 0, both 3, neither 0  (called blueprint_style, with or without a page: 100%; the tool alone, no page: 85%; needed 70% of the first)
  D over the 17 with-style runs by route `tool` (3 tasks): +13.50  [+13.00, +14.00]  (descriptive, a thin cell)
  D over the 3 with-style runs by route `both` (2 tasks): +7.50  [+7.00, +8.00]  (descriptive, a thin cell)
CLAIM: `blueprint_style` changes what an agent builds: the tool share was met and D is better
with-style runs that called blueprint_style before their first edit: 20 of 20
with-style runs whose own blueprint_present call cited followed lines: 20 of 20
without-style runs offered blueprint_style (must be 0): 0
without-style runs that tried the hidden blueprint_style by its device path: 0 of 10 (0 writes, 0 reads)
wishes made by with-style runs: 2
  - dress-the-market-area {"after": {"presentation": "present-1"}, "example_call": "blueprint_preview_operation {\"operation\":\"placement_add\",\"parameters\":{\"piece\":\"awning_canvas\",\"at\":[-8.8,9.0]}}", "hardship": "Placing an awning on a stall's spot silently replaced the stall (two apply+undo rounds), and nothing said placement_add replaces overlapping pieces. I had to remove and re-place all three stalls to make room.", "task": "Dressed the market rectangle with 3 stalls, awnings, lanterns, stock", "tool": "blueprint_preview_operation reporting, for placement_add, which existing placements the new piece would replace, before anything is applied."}
  - dress-the-market-area {"after": {"presentation": "present-1"}, "example_call": "blueprint_apply_operation {\"operation\":\"placement_move\",\"parameters\":{\"id\":\"market_stall@4\",\"at\":[-3.5,8.5]},\"dry_run\":true}", "hardship": "An awning placed over a stall silently replaced the stall, and a dry run only showed it in 'replaces'. I had to undo, remove a stall and re-place it, because there is no move operation.", "task": "dressed the market rectangle with stalls, awnings, lanterns, barrels and sacks", "tool": "placement_move: change a placement's x,z by id, with a dry run listing what it would replace or overlap."}
assembled style-without-style: {'dress-the-market-area': 8, 'light-the-gate-at-night': 8, 'write-the-gatekeepers-greeting': 8} trials per task, header trials = 8, commit c16441584c0eef5cc03aff187ad02124bfa71b28, date 2026-11-14

published: docs/bench/runs/2026-11-14-style-without-style.md and docs/bench/runs/2026-11-14-style-without-style.json
assembled style-with-style: {'dress-the-market-area': 8, 'light-the-gate-at-night': 8, 'write-the-gatekeepers-greeting': 8} trials per task, header trials = 8, commit c16441584c0eef5cc03aff187ad02124bfa71b28, date 2026-11-14

published: docs/bench/runs/2026-11-14-style-with-style.md and docs/bench/runs/2026-11-14-style-with-style.json
##### compare
dark_factory-bench: the first run (`style-without-style`) is unfit for comparison and will not be compared: 14 of 24 trials are harness errors (58%), over the 10% a comparison tolerates: these numbers are about the harness, not the agent. Rerun it; a run is fit when at most 10% of its trials are harness errors
```

`dark_factory-bench compare` (the registered second reading) **could not be produced**: it refuses a run with more than 10% harness errors, and it said "the first run (`style-without-style`) is unfit for comparison and will not be compared: 14 of 24 trials are harness errors (58%)". The published per-arm reports are in `docs/bench/runs/2026-11-14-style-{with,without}-style.{md,json}`.

## Reading, honestly

**By the registered rule the with-style arm did better, and the tool claim's condition is met.** D, the lines satisfied per run summed over the tasks, is +13.50 (interval +13.00 to +14.00, exact permutation p below 0.0001): 24.0 of 24 with the style against 10.5 without. Every with-style run called `blueprint_style` before its first edit (20 of 20) and cited `followed` lines (20 of 20); 17 used the tool alone and 3 also opened a page, so the share that called the tool is 100% (85% for the tool alone) against the 70% the registration needs, and none used only the files. On the registration's own terms that is a result for the tool, not only for the style being available. In the with-style arm every scored run satisfied every line (9, 9 and 6 of 9, 9 and 6).

**But the evidence is much weaker than those numbers look, and the report would be wrong to leave that out.**

1. **The arms lost very different numbers of runs, and the lost runs are not random.** All 18 harness errors are one thing: the model called a tool named `xd__mcp__dark_factory_blueprint_guide` (or `xd___mcp__…`), a mangled form of omp's device path, and the runner's isolation check records any tool outside the allowed set as a harness error instead of a result. It happened in 14 of 24 without-style runs and 4 of 24 with-style runs (without-style: 6 of 8 market runs, 7 of 8 greeting runs, 1 of 8 light runs). The runs that survive are the ones where the agent did not make that call, and nothing says that group is like the rest. The scored cells of the without-style arm are 2, 7 and 1 runs (market, light, greeting), against 7, 8 and 5 with-style runs; the greeting task's without-style number is one run. The bootstrap interval is narrow because the with-style arm is at its ceiling, not because the difference is well measured, and the registration said a percentile bootstrap of this size is anti-conservative.
2. **The registered primary result for the 30 registered runs left out a task.** With rounds 1 to 5 alone the market task had no scored without-style run, so it was excluded from both arms by the script's own rule, and the result is on the other two tasks (D +9.00 of a ceiling of 15, 8 with-style runs).
3. **A style written to be scored, followed by an agent that is told to read it, is satisfied.** The without-style arm cannot know that a lantern should be two, or that a greeting says "dusk", and satisfies about 10.5 of 24 where a do-nothing run satisfies 9 (the Don't lines that hold in the starting scene, plus a little more); the with-style arm reads the lines and follows them. That the style, once read, is followed was the least surprising possible outcome; whether it helps a person's game is not measured, as the registration's Limits said.
4. **The shipped guide tells the with-style agent to call `blueprint_style` first and to cite the lines it followed**, so 20 of 20 on both is the guide at work as much as a finding.
5. The two wishes the with-style runs made were about `placement_add` silently replacing an overlapping piece (the awning replaced the stall), not about the style, so **they give no signal for changing the style templates (SG-08)**. The without-style arm made no attempt to call the hidden tool (0 of 10 scored runs) and was never offered it (0 of 24).

**So:** the registered success rule is met on the data that exists, the tool claim's 70% condition is met, and the result should be quoted with the attrition: it rests on 20 scored with-style runs against 10 scored without-style runs, with 58% of the without-style arm's runs lost to one harness error that the arm shows far more than the other. A cleaner measurement would first stop that error (for example by making the runner tolerate or retry the mangled tool name, or by finding why the without-style arm provokes it), and then rerun; the registration's arm A' (pages on disk, tool hidden) is still the follow-up that would tell the tool from the files, and with a 100% tool share here it is not urgent.

## Deviations from the registration

1. **The runner is main's `c16441584`, not the registration commit `137f717ac`.** Registered: the runner at the commit that holds the file. Main had moved `crates/bevy_dark_factory_bench` (+27 files), `bevy_dark_factory` (+112 files) and added `revoke_text` to `blueprint_model`'s `trust.rs`; the suite (`games/sample`), tasks, fixture, `docs/research` (registration and analysis script) have no diff between the two. The runner changes that matter: omp calls through `eval` are seen (#434) and an unseen hand-back is graded from the editor's state; a pre-flight turn ("reply ok") before each invocation, counted in the report; the runner waits for the editor's map before a trial; a bare model name is refused; credentials are scrubbed from kept files and swept; a tool-call budget stop is recorded. They change what the runner can see and what a report costs; they do not change the arms, the tasks, the checks, the model or the analysis. The manager chose this runner (decision B) because the registered route stratification is only measurable where `eval`-routed calls are visible.
2. **`eval` was disabled for the trial agent in both arms**, because the registered script cannot read an `eval`-routed call (a synthetic `eval` call to `blueprint_style` came out as route `neither`), and a runner code change was not allowed. Without a runner change: a one-line wrapper named `omp`, first on `PATH`, adds `--config /tmp/studyrun-cfg/no-eval.yml` (`eval.py: false`, `eval.js: false`) to an agent run (first argument `-p`) and passes every other invocation through. Checked: a probe of omp with and without the overlay lists `eval` only without it, and the runner's own record of the tools each trial was offered held no `eval` in any of the 48 trials. Every editor call in every transcript therefore went through the `xd://` read and write route the script reads. The arms still differ only by the style (the same wrapper, the same overlay).
3. **The trust gate was opened by an editor wrapper, not by the registered `XDG_DATA_HOME` store.** The data directory honours `XDG_DATA_HOME` on Linux only; this host is macOS, where the trust store is under the trial's own `HOME`. The runner's `--editor` was a script that writes `<trial HOME>/Library/Application Support/Blueprint/trusted.yaml` naming that trial's project (the same `version: 3`, `projects:` store the registration writes) and then execs the real editor. It is the registered condition (the gate is pre-opened for the study's own trial paths and is not what is tested), proved by the probe's served pages. It applied to both arms.
4. **The host is bmb (macOS), not big, and the editor was a dev-profile build with no debug info** (`CARGO_PROFILE_DEV_DEBUG=0`; the first build with debug info filled the disk guard). The three tasks ran three at a time (`--jobs 3`) on the host's display as registered.
5. **Make-up rounds 6 to 8** are the registration's rule for an unfit arm ("make the missing runs up in further rounds, k 6, 7, 8"); the arm order alternated as in `k % 2`. They were run after the first five rounds were analysed, and the first five rounds' output is above separately. The registration's step 2 store naming those rounds is replaced by deviation 3.
6. **`dark_factory-bench compare` was not produced** (refused, unfit), and step 5's assembly and `report` ran over all eight rounds (the assembled `run.json` says `trials: 8`).
7. A **step-1 detail:** the `nothing` and `oracle` runs used the same wrappers and ran before the model runs, on this host.

## Data

`docs/research/data/2026-11-14-style-helps-an-agent/` holds every run's `trial.json` and `run.json` (48 trials in 16 runs, and the probe), the analysis outputs, the invocation logs and the two wrappers. Transcripts are not committed (about 300 MB); they are on bmb under `~/.local/state/dark_factory-bench/style-r*/` until that directory is cleaned.
