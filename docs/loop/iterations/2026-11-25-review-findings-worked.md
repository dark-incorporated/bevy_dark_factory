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

## The `PRIMARY` collision, and why one chord took three attempts

`terminal.clear` was placed wrongly twice, and each placement looked right on the host it was read on. The
gate settled it on the first Linux run.

| attempt | chord | collides on macOS | collides on Linux |
|---|---|---|---|
| original | `Modifiers::META` + `K` | **yes** — `META` *is* `PRIMARY`, so `palette.open_alt` (`primary(K)`) | no |
| "fix" | `Modifiers::CTRL` + `K` | no | **yes** — `CTRL` *is* `PRIMARY`, same chord |
| fix | `primary_shift(K)` | no | no |

The failure the gate printed, which reading could not have produced:

```
**two fixed chords are the same key**:
Modifiers { ctrl: true, shift: false, alt: false, meta: false } KeyK
  -> ["palette.open_alt", "terminal.clear"]
```

**`Modifiers::PRIMARY` is a runtime value**: `META` on macOS, `CTRL` elsewhere. Any chord written with
`PRIMARY`, `META` or `CTRL` has to be checked with **both** substitutions, and the only thing that decides it
is `no_two_fixed_chords_are_the_same_key` on the opposite platform. That test earned its keep — it was the
sole failure of a 5,213-test run (5,212 passed), so it isolated one mistake in an otherwise sound branch.

**What the rest of that gate said:** nothing. 5,212 of 5,213 passed, so `#637`'s move and everything merged
beside it were fine; the chord table was the only defect.

## The reboot that looked like five bugs

Every symptom chased for an hour was one event: **big rebooted at 16:42**, 68 minutes before I started
diagnosing.

| symptom | cause |
|---|---|
| `/tmp/fleet` missing, launcher failing at its own postflight line | tmpfs cleared by the reboot |
| `git fetch` → *"did not send all necessary objects"* | a **zero-byte** `refs/notes/gated-origin`, left by a fetch interrupted at shutdown |
| the gate cycling with `Failed with result 'exit-code'`, no error in its log | its log file was gone, so `systemd-run`'s redirect failed at start |
| zram holding 23 GB of unreclaimable pages | **cleared by the reboot**: 7 GB → 23 GB free swap |

`#624`'s recommendation was right and is now demonstrated: a reboot clears the compressed pages that stopping
containers cannot, and the gate went from *30 minutes then death at 64%* to `starting with 41 GB headroom and
22 GB free swap`. It is not a fix for the cause — zram refills as the same services run — but it is the only
thing that buys the room back, and `docs/loop/RUNNING-A-LOOP.md` now says to `mkdir -p /tmp/fleet` after one.

**A log was lost to this, again.** big's `gate-tests/` compile and run timings died with `/tmp`; big_mac's five
copies are the only survivors. That is the second time a log I needed was not kept somewhere durable, which is
the argument for memos living in the shared `.git` (`#673`) rather than in a worktree.

## Two hosts, and the two bugs only the second one could see

The objective was to use every host. Measured: `big` (24 cores) was at load 74 while `ps` accounted for
~600% CPU — **I/O wait**, not arithmetic — and `big_mac` (10 cores) sat at load 2. `bmb` cannot help: 3.6 GB
free. So the fleet ran on two hosts, and the same commit gated on both.

**That was not redundancy. Each host caught a bug the other could not see.**

| bug | Linux gate | arm64 gate | why |
|---|---|---|---|
| `command_k_clears_the_terminal` pressed `ControlLeft` for a `primary_shift(K)` chord | **green** | **red** — scrollback stayed 293, never 0 | `Modifiers::PRIMARY` is `Command` on macOS, `Control` on Linux |
| `the_text_roles_citations_land_on_their_symbols` pins the strings `chrome.rs:531`/`:827` | red | red | not platform — a real conflict, below |

The first is the class of bug a single-host gate *structurally cannot find*: the test is correct on the host
that runs it and wrong on the other. `no_two_fixed_chords_are_the_same_key` is the same shape, and it was the
sole failure of a 5,213-test Linux run earlier tonight.

**The conflict, for #680.** The docs merge rewrote comments in `chrome.rs` (+31 −24 = **+7 net**), which moved
`pub mod text` 531 → 533 and `pub fn section` 827 → 833. Two protected tests now disagree:

- `a_documents_citations_resolve` needs the page's citation to **reach the symbol's real line** → 533/833.
- `the_text_roles_citations_land_on_their_symbols` asserts the page **contains the literal `chrome.rs:531`**.

Both were green on `e309c056`. On `main` no choice satisfies both, so the issue carries the three options and a
recommendation (derive the line; same shape as #613) and is labelled `blocked` — a protected test is not mine
to re-point, even when the new number is the right one.

**What I got wrong:** I saw this on the Linux gate first and read it as a stale citation, so I re-pointed the
docs. That fixed the resolving test and broke the pinning one. One host's failure told me *what* broke and not
*which side was authoritative*; only running both did.
