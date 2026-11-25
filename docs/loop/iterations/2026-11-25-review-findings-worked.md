# 2026-11-25 — the review's findings, worked

## What is on `main` now (gate-verified)

| closed | evidence |
|---|---|
| **#618** closed-grep guard | gate #8: 5,208/5,208, 13 skipped, clippy clean, 1,248 s |
| **#619** tool-count guard | both defects: the early return *and* an off-by-one (33 vs 32) |

`main` also carries `release: 0.8.4`, the `review.md` rules and `c59aeb2f` (`#631` half one: `listing`
reports its refusal).

## Five fixes on `integrate/wave12` (`9b84233a`)

| issue | the change | proof |
|---|---|---|
| **#630** | `PLAIN_TEXT_KINDS`: `.rs`, `.py`, `.ts`, `.js`, `.sh`, `.csv` are finder rows. One cause behind both symptoms — the index never held them, so `big_source` matched nothing *and* Enter ran nothing | worker's red-then-green pair, 2 failed before, 2 passed after, +21 neighbouring |
| **#631** | `listing` returns `(rows, refusal)`; the DATA panel draws it in danger ink; `shown` compares **both**, so a repaired registry loses its sentence | `a_refused_registry_names_why_it_was_refused`, green in the gate at 0.016 s |
| **#614** | `cursor_ground` call sites **20**, not "fourteen"; `On<Activate>` **117 across 32 files**, not "twenty-four / twenty-two" | measured before and after |
| **#617** | `CHIP_PAD` → the sheet's `.chrome-chip`; `COL_TIGHT` → `Col::Tight`'s `chrome-col-tight`; `editor_skin.yaml` → `src/style/editor.yaml`, which really holds `text_size: 11.0` | grep 0 for all three |
| **#615** | the census refusal names `STEP_TIGHT/ROW/GROUP` and `Col`; a new test reads the advice | old message → **9 faults**, new → 0 |

## The new guards, and what they can catch

Both were **proved to fail** before being trusted:

- `a_closed_comment_issues_grep_stays_clean` — a planted `MIN_FIELD_H` reported `#543 … filter.rs:529`
  and nothing else; a planted `CHIP_PAD` reported `#617 … anim_tab.rs` and nothing else.
- `a_refusal_message_names_only_symbols_that_exist` — reads every backticked `SCREAMING_CASE` or
  `Pascal::Case` name in the test sources. **Known limit: `PAD` and `MARGIN` pass as substrings**, so a
  short name is weaker evidence than a long one. It is a floor, not a proof.

## Filed this pass

| issue | finding |
|---|---|
| **#632** | three timing tests pass alone and fail when two whole-suites run at once — **the 70% contention showing up as a red gate**, not a slow one |
| **#633** | the pre-push hook and rust-analyzer share one `CARGO_TARGET_DIR`, so a push queues behind the editor. Cost this session **an hour and three kills** of pushes that looked hung |
| **#634** | a worker branched from a dirty worktree and inherited half a refactor; its own commit was clean, its tree did not compile |
| **#629** | **mis-filed and corrected.** `+` spawns live bash (`M629-42` computed by bash, 184 cols matching the pane). `Ctrl+J` *does* start a shell when nothing is inviting. The decision lives in `pump_the_terminal`'s `if !terminal.invites_the_agent()` — a function I never opened before filing |

## What I got wrong, twice, and the rules it produced

1. **#598**: the sweep pressed **Super** where gooey binds **Ctrl** on Linux. The chord was never pressed.
2. **#629**: I read `pty.rs`, `terminal/mod.rs` and the `+` handler, concluded "the terminal never spawns
   a shell", and was wrong — the condition is one function I had not opened.

Both are now `review.md` rules: **a chord pressed over BRP must be the chord the host binds**, and **a code
reading is not a measurement; find the branch that decides, not the function that would perform**.

## Still open and unworkable by a pass

- **#501**, **#525**, **#531**, **#601**, **#610–#613** — owner decisions or hardware.
- **#621** orders `git reset --hard` in a protected command file; **#622** asks which consumer label is
  canonical. Both need the owner's word.
- **#624**'s wait is **fixed** (one headroom number, 18 GB = the old floors' sum; three gates paid 30 min
  each before, two since started in seconds). Its 70% contention figure stands and is #632's subject.
