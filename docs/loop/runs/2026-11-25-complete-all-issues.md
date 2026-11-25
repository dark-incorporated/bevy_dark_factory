# 2026-11-25 complete-all-issues — the queue, worked

## Counts

| | |
|---|---|
| opened at start | 30 |
| **closed** | **8** (#614 #615 #616 #617 #623 #630 #631 #632) |
| still open | 22 |
| gate on `main` | **5,211/5,211 passed, 13 skipped, 0 failures, 1,324 s** at `3db6c23e` |
| display-free checks | **3/3 PASS in 834 s** on `big_mac` (clippy 210 s, doc 70 s, doctests 554 s) |

## Closed, with the evidence each was closed on

| issue | fix | proof |
|---|---|---|
| **#630** | `PLAIN_TEXT_KINDS`: a project's `.rs`/`.py`/`.ts`/`.js`/`.sh`/`.csv` are finder rows — one cause behind both symptoms | red-then-green pair; 2 failed before, 2 passed after, +21 neighbours |
| **#631** | `listing` returns `(rows, refusal)`; the panel draws it; `shown` compares both so a repaired registry loses its sentence | `a_refused_registry_names_why_it_was_refused`, 5 cases |
| **#614** | `cursor_ground` is **20** call sites, not "fourteen"; `On<Activate>` is **117 across 32 files**, not "twenty-four" | measured both sides |
| **#617** | `CHIP_PAD` → the sheet's class; `COL_TIGHT` → `Col::Tight`; `editor_skin.yaml` → `src/style/editor.yaml` | grep 0 for each |
| **#616** | the seven `0.19` citations, including the line number wrong by twelve | `grep '0\.19'` → 0 |
| **#615** | the census refusal names `STEP_TIGHT/ROW/GROUP` and `chrome::Col` | old text → 9 faults, new → 0 |
| **#623** | `keys.md` chord, the RON block in a YAML page, the 117-vs-5 call-site count | measured each against the code |
| **#632** | `a_session_knows_when_its_shell_has_left` runs alone | `wall_clock_filter.py`: *"23 tests named, all read a clock"* |

## What I got wrong, and fixed

1. **A guard that flagged 140 legitimate names.** `a_refusal_message_names_only_symbols_that_exist` could not tell
   prose (`VEIL`) from a dead constant. Reverted in `c5f4a748`; the census message fix stayed.
2. **Two of #632's three tests read no clock.** `wall_clock_filter.py` refused them and was right.
3. **My `#617` comment edit shifted two line citations** in `layout-model.md` by one. The gate caught it
   (`a_documents_citations_resolve`); fixed in `9c235636`. **A comment edit that moves a line can break a citation
   elsewhere** — that is now a recorded lesson.

## Filed

**#633** the pre-push hook and rust-analyzer share a `CARGO_TARGET_DIR` (cost an hour, three "hung" pushes that
were queued) · **#634** a worker branched from a dirty worktree and inherited half a refactor · **#635** two tests
pass on big and fail on mac_air (a symlinked temp path; 35 terminal tests needing real PTYs) · **#636** every gate
compiles on 1 of 24 cores with the host 96% idle.

## The throughput answer

The queue was **serialised through one host**, and that host was idle while it worked:

| setting | cost |
|---|---|
| `CARGO_BUILD_JOBS=1` | one core compiles, 23 wait |
| `NEXTEST_TEST_THREADS=4` | four test, 20 wait (**has evidence**, #527 — keep) |
| the heavy lock | held for the whole push, so no second job may start |

**What is now different:** clippy, `cargo doc` and the doctests run on `big_mac` (834 s, 3/3 PASS) while big keeps
the one thing only it can do — the 5,211 editor tests. That is ~47% of a gate's non-test work off the serialised
host. **Still owed** (#636): measure `CARGO_BUILD_JOBS` 1 vs 2 against memory, and release the heavy lock before
the git transfer.

## Left open, by category

- **Owner decisions:** #541 (a UI budget the owner sets), #610 (a theme setting: skin or palette), #611 (an LSP:
  which server), #612 (which VSCode polish behaviours), #621 (a protected command orders `reset --hard`), #622
  (which consumer label is canonical).
- **Unstarted scope:** #555 (audition loads no sound), #536 #551 #552 #559 (the gooey forks), #540 #542 (the Files
  editor), #576 #580 #593 #613 (harness and gate cost), #596 (the PLOTS legend), #624 #625 #626 #627 #628 (gate
  cost, each with a measurement now).
- **In flight:** #635's fix is written and building on mac_air.
