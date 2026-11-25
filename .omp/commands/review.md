---
description: A thorough review of the Dark Factory on seven lenses (visual, performance while driving, docs vs code, docs vs docs, stale comments, the family crates' API and style, build and CI speed), filing grouped issues after searching for duplicates.
argument-hint: "[lens, e.g. visual | perf | docs-code | docs-docs | comments | family | build] (omit for all seven)"
disable-model-invocation: true
---

# bevy_dark_factory: a thorough review, one lens at a time

This command is a **review that fixes what it can see and files the rest**. The visual lens sweeps every part of
the UI, pictures what is drawn, and repairs the small, safe defects on the spot in its own worktree. The other six
lenses are finding work: each proves a defect with a measurement or a quote, files it on
`dark-incorporated/bevy_dark_factory` as an issue that is cheap to answer, and groups the issues so the next build
loop can take a cluster at a time. The repository is the memory: the open issues are what a review must not
re-file, and each filed issue is the evidence.

**Owner's rules that bind this command:** two agents at most unless the owner says otherwise (the pass
says how many it took); every agent runs `ollama-cloud/deepseek-v4.1-flash:1017` through ollama cloud
unless the owner has changed the model settings; `find the issues and make bug reports` (owner, 2026-11-23),
**search the existing issues first**; **find groups of bugs that can be tackled together** and say the group
in every issue you file; a review that changes no code never runs the gate.

## Before anything: what exists

1. `gh issue list -R dark-incorporated/bevy_dark_factory --state open --limit 200 --json number,title,labels`
   and the same for `--state closed --limit 200` (a thing fixed last week is not a finding; a thing filed and
   declined is not a finding either — read the close reason). Put both lists in `/tmp/review/open.txt` and
   `/tmp/review/closed.txt`, and **every agent reads them before it reports**.
2. `git -C <worktree> log --oneline -40` for what just landed: a defect in code younger than a day may be a
   known gap, and the newer a doc is, the more likely it is right.
3. `docs/PROJECT.md`, `docs/guide/README.md`, `docs/design/*.md`, `docs/ui/*.md`, `docs/mcp-tools.md`: the
   architecture and guidance the review holds the code to. `AGENTS.md` says which crates and documents own
   which facts.
4. `tools/loop/iterations/`: the fleet's own reports say what was measured recently and what was knowingly
   left undone. A "finding" the last report already lists as a known limit is a **confirmation**, not a bug;
   file it only if the owner would act differently now.

## The seven lenses

Each lens is one agent's whole job. **Two may run at once** (the driving/capture lenses need the GPU and the
display, and two editors cannot share one BRP port); the other four run two at a time beside them. The pass
names which agents it started and what it ran in parallel.

| # | Lens | How it is proved | Labels |
|---|---|---|---|
| 1 | **Visual**: the UI as drawn — **every workspace, panel and overlay, at two sizes**, cropped and measured; the small safe defects are **fixed** in the sweep's own branch | pictures (`bevy_fairy` capture or `import -window root`), cropped 2×, read with a row/column scan where size, alignment or contrast is the defect; a commit per fix | `ui`, `bug` |
| 2 | **Performance while driving**: where an interaction stalls | a measured wall-clock or `UiFrame` number from a scripted drive of the running editor, with the baseline beside it | `bug`, and `performance` if it exists (else `enhancement`) |
| 3 | **Docs against code**: a guidance or architecture document says the code does X and the code does Y | both sides quoted with file, symbol and line | `documentation` |
| 4 | **Docs against themselves**: two documents, or one document twice, say different things | both quoted with path and line | `documentation` |
| 5 | **Stale and wrong comments**: a comment states a mechanism the code no longer has, or a number it cannot still be | the comment quoted, the code that contradicts it quoted, and the commit that changed it (`git log -S`) where one can be found | `documentation` (a comment is documentation) |
| 6 | **Family conformance**: this crate's use of a `bevy_*` family crate disagrees with that crate's own API or style | the family crate's doc or `AGENTS.md` quoted, this repo's call site quoted, and a measurement if it is behavioural | `enhancement` or `bug`, plus `family` |
| 7 | **Build and delivery speed**: where the build, the tests, CI or a release spends time that buys nothing | a measured wall-clock from the machine's own logs (`<git-dir>/gate.log`, `journalctl`, `tools/loop/fleet/timing.py`), the step it belongs to, and what the step catches that the step before it could not | `enhancement`, and `performance` where the repo has it |

### Lens 1: visual (agent `ReviewVisual`)

- Read `skill://bevy-stage-and-capture` and `skill://bevy-dark-factory-capture-issues` first. The editor's
  own captures (`Cmd+D`) are the ground truth when a person made one; a staged capture is the ground truth
  when nobody did.
- **Drive it, do not read it.** Start the editor headless offscreen on big with the project fixture
  (`xvfb-run -a` or the fleet's own display unit), `BEVY_BRP_PORT=1571<n>`, and capture through
  `dark_factory_mcp`'s `blueprint_capture_sequence` at 2 fps or a `bevy_fairy/screenshot` with `region` and
  `zoom`. **One capture per state, not a scripted playthrough.**
- **Sweep it, do not sample it** (owner, 2026-11-24: *"go through the different parts of the UI, all parts of the
  UI … exhaustively, across the entire UI"*). "Ten states" was this command's first shape and it left most of the
  editor unlooked at. The list is now **every workspace, every panel, every overlay, at two sizes**, each captured
  on its own, cropped to the defect, and measured:

  | Group | What must be captured |
  |---|---|
  | Launcher | the project page, the What's new page, a map card, the filter field, Settings |
  | Workspaces | every variant the `Workspace` enum names — Project, Map, World, Kit, Files, Animation, Design, Settings, Sound, Game — and name any that does not open |
  | Panels | inspector (empty and with a long piece selected), place, library, kit rows, DATA table, design styles, notes, reload, audio buses/cues/log, terminal, console, notifications, problems list, world lists, animation bench, rig list, tile palette, compose, behaviours, crosslinks |
  | Overlays | command palette (empty and with a query), goto, keymap screen, a dialog (new project, a refusal), a menu open, the style inspector on a panel edge, the section cut, the status band, a tab strip with several tabs |
  | Sizes | the reference window **and** a narrow one (1100×700): a layout that only works wide is a defect |

  The `Workspace` enum and the panel registry are the authority for what exists; read both and say in the report
  which states you could not reach and why. A gap named is worth more than a state silently skipped.
- **Crop and zoom every defect** to its own 2× PNG before judging it, and measure it (the pixel the text ends at,
  the box edge, the two colours and their contrast ratio, the row height). A crop is also what an issue links.
- **Fix the small, safe ones on the spot** (owner, 2026-11-24: *"Or better yet, just fix them on the spot"*). A
  missing word, a wrong number, a width, a colour, a clipped label: fix it in your own worktree, one concept per
  commit, running only the module you touched. **A design call is not fixed** — it is reported with the picture and
  the options. The branch is `fix/uireview-<n>`, it is pushed for review, and the pass's report names every commit.
- **The states the last sweep could not reach are the first targets of the next one** (owner, 2026-11-25). Every pass ends
  its report with a **Gaps** list — a state it could not drive, a measurement it could not take. The next pass reads that
  list first and spends its first hour there, because a gap is a state no review has ever looked at. Two passes in a row
  reporting the same gap is a finding about the harness, not about the UI: file it.
- **Wait for the gate before starting an editor.** `systemctl --user is-active fleet-gate-rel2` — an editor run beside a
  gate competes for the same 8–15 GB and can OOM the gate, which then refuses for a reason that has nothing to do with the
  branch. Check every two minutes and **say in the report how long you waited**; that wait is itself a build-speed
  measurement (see lens 7).
- **`AGENTS.md` is generated; a local edit is a deleted edit.** It comes from `agent-guidance/general/AGENTS.md` +
  `machines/<host>/AGENTS.md` through `compile.py push`, so changing the file in a project is overwritten on the next
  push and never reaches the other hosts. A fix that looks like a one-word typo there is a change in that repo, or it
  is nothing. Measured 2026-11-25: a pass edited `AGENTS.md` and `.omp/commands/build-loop.md` to correct a label,
  both being protected/generated, and had to revert both.
- **Measure the gate's utilisation before believing its duration is the work's fault** (measured 2026-11-25).
  A 24-core host ran every gate with `CARGO_BUILD_JOBS=1` and `NEXTEST_TEST_THREADS=4`, holding an exclusive lock
  for the whole push, at **96.5% idle** — so a gate's wall time was one core's serial work plus a block on every
  other job. Before proposing a faster gate, run `top -bn1 | grep ^%Cpu` **during** a gate and compare it with the
  thread caps in `gate-rel.sh`. A cap with evidence behind it (`NEXTEST_TEST_THREADS=4`, starved by a 1 s file
  watcher, #527) is not the same as a cap with none (`CARGO_BUILD_JOBS=1`) — change only the second, and measure
  its ceiling against **memory**, since one rustc peaks near 15.6 GB on a 31 GB host.
- **Put the host-agnostic checks on the idle hosts.** Clippy, `cargo doc` and the doctests need no display, so
  they can run on any host with the same toolchain while big keeps the one thing only it can do — the editor
  tests. A 24-core host serialising clippy is 23 cores doing nothing.
- **`chords.rs` and `keys.rs` both bind keys; grep both before saying nothing binds one.** Measured 2026-11-25:
  a pass filed "Ctrl+Z is bound to nothing" from `grep KeyCode::KeyZ crates/bevy_dark_factory/src/chords.rs` → no
  match. `keys.rs:1812` binds `Action::Undo` to `Control + KeyZ`, beside `Action::Redo` at `:1822`. The two files
  split the job — `chords.rs` the fixed rebindable chords, `keys.rs` the contextual action bindings — and searching
  one and generalising to both produced a wrong issue. **Name the file you searched.**
- **A code reading is not a measurement, and a claim about why something does not happen needs the
  decision point read, not the path into it** (measured 2026-11-25). Two reviews running filed issues from
  reading a handler and concluding a feature was broken: `Cmd+J` "inert" (the press was Super where the host
  binds Ctrl), and "the terminal never spawns a shell" (true on `+`, but the whole condition lives in
  `pump_the_terminal`'s `if !terminal.invites_the_agent() && terminal.is_empty()`, which neither reviewer
  opened — the editor was offering its agent instead of a bare shell, by design). Before filing "X never
  happens", find the **branch that decides X**, not the function that would perform it. If you have not read
  the condition, say `[INFERENCE]` and drive it.
- **A chord pressed over BRP must be the chord the host binds** (measured 2026-11-25). gooey's
  `Modifiers::PRIMARY` is `Platform::current()` — **Ctrl on Linux**, Command on a mac. A drive that presses Super on
  big presses nothing, and the 2026-11-24 sweep filed `Cmd+J` as inert on exactly that. Press the host's spelling, and
  **assert the press landed** (`ui.focus`, or the state field the chord moves) before concluding the chord is broken.
  A gesture that changed nothing may be a gesture that never arrived.
- **Capture the binary, not the target.** `cp ~/.cargo-target-gate/debug/bevy_dark_factory /tmp/<pass>/bin/` before the
  run — a gate rebuilding that directory rewrites the executable under a running editor.
- **A contrast claim is a colour pair and a ratio, never a sampled pixel.** A 2026-11-24 finding read `rgb(162,121,50)`
  off a glyph stem and called it an 2.16:1 ink; the stem was **59.7% coverage** over `rgb(19,18,15)` and the row reads
  **10.26:1**. If you solve a colour out of a screenshot, solve it per channel and check that the pixel is not a blend;
  the honest instruments are the reflected `TextColor`/`BackgroundColor` for a painted text, and a pixel scan only for
  geometry.
- **What counts as a defect**: text outside its own content box; a label clipped mid-word where the row has room; a
  control outside its panel; a panel drawn empty or with its body collapsed; two inks where one is meant; a colour
  pair under 4.5:1 for body text or 3:1 for large; a scroll area with nothing in it; a picture at the wrong aspect; a
  row wider than its panel; a label naming a thing that moved; a number the panel states that the code no longer
  has. **Read the picture, do not guess from the code.**
- **The machinery that works** is `tools/loop/remote/editor_png.sh` (Xvfb on a spare display, `BEVY_BRP_PORT`, a
  scratch project with `HOME`/`XDG_*` pointed at it, `import -window root` as the fallback capture) and
  `tools/demo/record.py` (`record.McpClient`, `bevy_fairy/input`, `bevy_fairy/screenshot`). **One editor at a time:
  one display, one BRP port.** Copy the binary to a scratch path first — a gate rebuilding `target/` will rewrite
  it under the run.

### Lens 2: performance (agent `ReviewPerf`)

- **Measure on the running editor**, not on a bench: `bevy_fairy`'s frame time, the editor's own
  `UiFrame` budget, or the wall-clock of a scripted interaction (`tools/demo/record.py` with a storyboard,
  or `bevy_fairy/input` key by key). Record the baseline on the same machine in the same minute: a number
  without its baseline is a guess.
- **Where to look**: opening a big map; switching workspaces; the Files tree on the fixture project
  (how many files does it have? say it); the palette and the goto index (typing one letter versus ten); the
  DATA table with a large data file (the owner's concern, measured by the #536 worker at 1,000 rows);
  undo and redo of a large edit; the terminal pane with a long scrollback; the style sheet reload while the
  inspector is open; a drag of the divider at 1,000 nodes; the map census at 100k files (the last measured
  figure is in `docs/guide/reload-contract.md`).
- **A finding needs**: the interaction, the measured time (p50 and worst of 20), the baseline, the window
  and interface size, and the file and symbol where the time goes if you can name it (a `--no-capture`
  nextest trace or a sampling profiler on the running editor is acceptable; do not guess).
- **Do not file**: a slow thing the docs already call slow with a number (confirm it in the issue's
  "current behaviour" and file only if the number has moved).

### Lenses 3 and 4: documents and code, documents and documents (agent `ReviewDocs`)

- **Build the map first**: one line per document (`docs/PROJECT.md`, `docs/guide/*.md`, `docs/design/*.md`,
  `docs/ui/*.md`, `docs/mcp-tools.md`, the READMEs, `AGENTS.md`) saying what fact it owns. Then read the
  code the fact names. One agent can do both lenses; a pass with two free agents may split them.
- **Every claim you check is a triple**: the document (path + line), the code (path + symbol + line), and
  the verdict (agrees, contradicts, silent). A contradiction is a finding only when **the code is the
  authority** (it usually is) or the owner has said the document is (a decision record).
- **Document against document** is the cheap half: the same number, limit, name or promise stated twice.
  Grep for the number, not the prose. Names that changed recently (a rename, a constant renamed, a crate
  renamed) are the richest seam.
- **Do not file** a document that is deliberately historical (`docs/loop/iterations/`, `docs/research/`,
  a design doc with a "Status" line that says it is a snapshot). History is not a contradiction.

### Lens 5: comments (agent `ReviewComments`)

- **The owner's question (2026-11-23):** *"I'm tempted to put a rule for no comments in the codebase,
  stripping out those that exist."* The review answers it with a **survey**, not an opinion: how many
  comment lines, of what kinds (a doc comment that states an invariant, a `//` that explains a mechanism, a
  comment that repeats the code, a comment that is out of date, a commented-out block), per crate and per
  file. Measure it: a script that classifies by pattern is enough if you say the pattern.
- **Then find the defects the survey cannot see**: a comment stating a constant that is no longer that value, a
  comment naming a function that was renamed or deleted, a comment describing a mechanism the code no
  longer has (the `#535` review found two: "a clip drew the panels empty" survived the disproved mechanism),
  a `TODO`/`FIXME` older than the issue it names, a commented-out block.
- **Every finding quotes**: the comment, the code that contradicts it, and — where `git log -S` finds
  it — the commit that made it wrong. A stale comment is a one-line fix; say whether it is one line or a
  judgement.
- **Do not file** the whole-codebase restyle as an issue until the pass has counted it: the survey goes in the
  pass's report, and the owner's "no comments" question is answered there with the numbers (how many
  comments are load-bearing, how many are noise). File individual stale comments as issues only when each is
  worth a commit; a cluster of them is one issue with every file listed.

### Lens 6: the family crates (agent `ReviewFamily`)

- The crates this editor builds on are `bevy_gooey` (UI), `bevy_elements`, `bevy_flora`, `bevy_lights`,
  `bevy_light_grid`, `bevy_society`, `bevy_speech_bubbles`, `bevy_orca`, `bevy_stigmergy`, `bevy_carnage`,
  `bevy_siren`, `isomesh`, `bevy_fairy` and `bevy_blueprint` (in this tree). For each one the editor uses,
  read that crate's own `AGENTS.md` and doc comments at the pinned rev (`~/.cargo/git/checkouts/<crate>-*/<rev>/`
  on big) and compare with this repo's call sites.
- **What counts**: a host building a second schema for a crate's settings (the rule is "configure, don't
  fork"); a host re-implementing a widget the crate ships (the rule is "file it on gooey"); a call that
  ignores the crate's own documented contract (an `apply_skin` ordering, a `Role` the crate says a host
  must not set); a style mismatch the crate's guide names (a naming, a plugin shape, a test idiom); a pin
  older than a fix the editor depends on.
- **Every finding says which crate, which document, which call site, and whether the fix belongs here or in
  the crate** (a change in the crate is filed there with `consumer` and `consumer:bevy_grimoire`, per
  `AGENTS.md`).

### Lens 7: build and delivery speed (agent `ReviewBuild`)

- **Measure before you file.** The evidence exists and is cheap: `<git-dir>/gate.log` has one line per gate step in
  seconds; `journalctl --user -u fleet-gate-rel` has each attempt's wall clock, memory peak and how it died;
  `python3 tools/loop/fleet/timing.py` says where a fleet's hours went; the CI runs have their job times
  (`gh run view <id> --json jobs`); the release run has its steps. A build-speed issue without a number is noise.
- **Where this repo already knows it is slow** (start here, and check whether each is still true):
  `docs/loop/iterations/2026-11-12-gate-audit.md` measured that **57% of the push gate's time went on runs that
  failed**, that the whole suite ran three or four times per cluster, that a version bump paid for a 20-minute test
  run, and that a self-made `touch` of every source file forced a full rebuild per gate. The gate now has a
  version-bump skip and a clean-merge path; measure whether they fire in practice.
- **The seams worth measuring**: (a) the gate's own shape — clippy over every target, then the whole workspace,
  then doctests, and whether any of the three could reuse another's artefacts; (b) `cargo nextest run --workspace`
  per crate versus the one editor binary (the editor's ~90 test files compile into one binary, and its tests are
  most of the suite); (c) the **`codegen-units` cache cost measured on 2026-11-22**: changing it for the whole dev
  profile recompiled 61 crates, and for the editor crate alone it recompiled the crate; a gate-time override that
  invalidates a warm target is a tax, not a saving; (d) CI: what a job recompiles that the last job built, the
  runner's cache keys, the wasm and macOS legs, the nightly job the owner already knows is killed by contention
  (#528); (e) the release: what the notarize and staple steps cost and whether the macOS run could start while the
  Linux gate is still going; (f) tests that are redundant — a test asserting what another test asserts, a test that
  runs the same app twice, a whole-binary run inside a per-crate job; (g) the harness's own waits (`sleep`s,
  `pump`s and `until`s) counted across the suite: a test that waits 1.3 s to prove a 0.5 s deadline is a tax paid
  on every gate and it is measurable by summing them.
- **Every finding says**: the step, its measured time **and the share of the whole** it is, the baseline (a warm
  run, or the same step before the last change), what it catches, and what cutting it would stop catching. **A
  proposal that weakens a check the owner's rules protect is a question for the owner, not an issue** (say so in
  the report with the number).
- **File** one issue per step worth a commit, labelled `enhancement` (or `bug` if a step is redundant by
  construction), with the measurement and the risk. A "make CI faster" issue with no number is not a finding.

- **Separate the step's cost from the host's** (measured 2026-11-25, #624): the same tree and the same command took
  **252 s to 2,201 s** across retries; the lowest passing test step is **384 s** and the median **1,300 s**, so **≈70% of a
  median gate is waiting on other tenants**, not compiling or testing. Before proposing a shorter suite, check whether the
  step's *floor* is already acceptable — if it is, the cost is contention and the fix is serialising the gate. Read
  `gate.log` for repeated rows of one tree hash; the spread is the measurement.
- **A fast path is only real if the log shows it firing.** Measured over 189 attempts: version-bump 4 hits (2.1%),
  clean-merge **1 of 24**, already-gated 0 — 2.8% of gate wall in total. A path filed to fix a rate that still shows the
  old rate is a finding about the path, and one `grep` answers it.
- **The gate deletes its own compile/run split** (#625): `pre-push` runs nextest through a `mktemp` it removes, so
  `gate.log` keeps the step's total and nobody can say how much is compiling. Ask for that line before judging any
  compile-time proposal.

## Filing: search first, group second, one defect per issue

1. **Search, then file.** For each finding: `gh issue list -R dark-incorporated/bevy_dark_factory --state open
   --search "<the words of the defect>" --json number,title`. If an open issue names it, add a comment with
   the new measurement or picture (do not open a second). If a **closed** issue names it, check the close
   reason: fixed and regressed is a new issue that says "regressed from #N"; declined is not a finding.
2. **One defect per issue**, in the repo's existing shape: title says the defect (not the file), body has
   *what the code does*, *where* (file, symbol), *what was measured* (with the number or the picture), *what
   it should do*, and *acceptance* as three or four testable boxes (the shape `docs/loop/captures/ISSUE-FORMAT.md`
   defines for a capture issue is the model; a review issue without a picture uses the same sections minus
   the picture).
3. **Pictures**: if the defect is visible, the picture goes in the issue body as a `raw.githubusercontent.com`
   URL from `docs/loop/captures/<stem>/` and **answers 200 before the issue is created** (the skill says how:
   `python3 tools/loop/captures.py sync --from <dir>`, push, `curl -o /dev/null -w '%{http_code}'`). A review
   capture is not a person's `Cmd+D`, so it goes in its own folder with a caption saying it was staged.
4. **Groups**: every issue carries a line `Group: <name>` in the body, where a group is a cluster the next
   build loop can take together (e.g. "the DATA table", "the Files tree", "docs: the reload contract",
   "comments: the chrome"). The pass's report lists the groups, how many issues each has, and which issue is
   the one to take first.
5. **Labels**: `bug` for a defect, `documentation` for a document or a comment, `enhancement` for a
   conformance or a design call, `ui` where a person sees it, plus `group:<name>` if the repo's label set
   has such a label (create it once with `gh label create "group:<name>"` only if the owner's label naming
   allows it; otherwise the body line is enough).
6. **Never file** an issue that needs a decision the review cannot make (a design call, a product choice): put
   it in the report as a question for the owner instead, with the options and a recommendation.
7. **Never file** an upstream defect here: a `bevy_gooey` bug goes to `dark-incorporated/bevy_gooey` with
   `consumer` and `consumer:bevy_grimoire`.

## A closed issue's check is a test, or it is not closed

Four comment-cleanup issues (#543 #544 #546 #548) closed on a grep that **still returns hits today** (#614–#618), and one
guard that exists to hold a number — `.omp/commands/build-loop.md`'s tool count — **returns early on the line it was
written for** (#619). Both are the same failure: a claim of "verified" that nothing re-checks.

So every finding that can be written as a grep, a count or a citation gets a **test**, not a close comment:
- the test runs the issue's own acceptance pattern, so it cannot drift from what the issue asked;
- its failure message names the issue number and the file:line of each hit;
- it lives in `crates/bevy_dark_factory/tests/` and is registered in `all.rs`, or in `crates/dark_factory_mcp/tests/` when it
  is about the server's own documents.

**A guard that returns early is worse than no guard.** When you file one, state what makes it **fail** — a planted fault, a
mutated constant, a deleted line — and report whether you ran that. `#619`'s guard found a line, took the part before the
first `*`, found no digits, and returned; every count in that bullet would have passed it forever.

## An issue, not a paragraph (owner, 2026-11-24)

**Every finding is an open GitHub issue on `dark-incorporated/bevy_dark_factory` before the pass ends.** The report is
the index; the issues are the work. A finding that exists only in the report is a finding nobody will pick up: the
build loop reads the issue list, not last week's markdown.

- The order is fixed: **search → file → verify the number → write the report**. When the pass runs out of time, the
  report is what gets dropped, never the filing.
- A finding that an open issue already names gets a **comment with the new measurement** on that issue — an issue
  updated is the same as an issue filed, and better than a duplicate.
- A finding the review cannot decide (a design call, a product choice) is filed too, labelled `enhancement`, with the
  options and a recommendation — the owner decides by answering the issue, not by reading a report.
- The sweep's **fixes** are not issues: a fix that landed is a commit with its module's test; a fix the sweep decided
  not to make becomes an issue saying why.

## Regressions first (measured 2026-11-25)

This pass's sharpest findings were not new defects: **#543, #544, #546 and #548 had each closed on a grep that still
returns hits** (#614–#618), #568's advisory ruling had reached one page of three (#620), and #560's chord had moved in the
code while its alias stayed in `keys.md` (#623). Four of fifteen findings were the part a fix missed.

So the search is not only "does an open issue name this" but also:

- **Search the closed issues for the same words**, and if one closed as fixed, run its own acceptance check. A hit is a
  **regression**, filed as `regressed from #N` with the old issue's check quoted.
- **The report's first table is regressions**, before new findings: the issue, what its close claimed, what the check
  returns today. That table is what tells the owner whether the closes can be trusted.
- **A close comment's claim is a claim like any other** — quote it and test it. "a grep returns 4 hits, all historical" is
  checkable in one command, and nobody checked it.

## The report (the pass's last act)

One file, `docs/loop/iterations/<date>-review.md`, committed and pushed: the seven lenses run and by which agents;
every issue filed (number, title, group, the measurement that proves it); every **confirmation** and **not a
finding** with the reason; the counts per lens; the comment survey (lens 5) with the numbers the owner asked
for; and the **questions for the owner** (the decisions the review could not make). A docs-only push goes
through the pre-push fast path; the report is the only commit this command makes, and it changes no code.

## The shape of a pass, in order

1. Read what exists (the four steps above) and write `/tmp/review/open.txt` and `/tmp/review/closed.txt`.
2. Start the lenses that do not need the display (`docs-code`, `docs-docs`, `comments`, `family`, `build`) — two at a
   time unless the owner has lifted the two-agent limit — while the driver prepares the editor for `visual` and `perf`.
3. Run `visual` and `perf` one after the other on the same editor (one BRP port, one GPU): drive, capture,
   measure, stop the editor.
4. **The sweep's fixes are reviewed like anyone's**: the pass reads each commit's diff, runs the one module it
   touched, and keeps or drops it. A fix that cannot be shown to help is dropped, not shipped (owner's rule: a fix
   whose effect cannot be shown is not a fix).
5. **De-duplicate across lenses before filing**: two lenses often find one defect from two sides. The pass
   merges them into one issue naming both proofs.
6. **File every finding as an open issue** (search first, comment on an existing one), then write the report, then
   push the report and the sweep's branch. If time runs out, the report goes and the filing stays.
7. Stop after the report. The sweep's branch is gated by the next pass, not by this command.

## What not to do

- **Fixing is the visual lens's job and only there** (owner, 2026-11-24: *"Or better yet, just fix them on the
  spot"*). A visual defect that is small and safe — a word, a number, a width, a colour, a clip — is fixed in the
  sweep agent's own worktree, one concept per commit, with only the touched module's test run. **The other six
  lenses still change no code**: they are finding work, and their output is issues.
- Do not run the gate from any lens. Do not touch a test's assertion, a fixture or a golden file to make a fix pass;
  if a fix needs that, it is a finding, not a fix.
- Do not file from a document alone: a document that disagrees with code needs the code quoted, and code
  that disagrees with a document needs the document quoted. The finding is the *disagreement*, and both
  sides are quoted with path and line.
- Do not file a number you did not measure. Label an inference `[INFERENCE]` and say what would measure it.
- Do not re-file an open issue: a comment on the issue with new evidence is the review's value.
- Do not file the whole-codebase comment restyle as an issue before the survey has counted it; the owner's
  question is answered in the report, with the numbers.
- Do not leave the editor running, a BRP port held, or an Xvfb up when the pass ends.
