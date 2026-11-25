# 2026-11-25 complete-all-issues — closing report

## Counts

| | |
|---|---|
| opened when this pass began | 34 |
| **closed by this pass** | **14** |
| still open | 25 |
| blocked, correctly labelled | 4 (#540 crate release, #625 #633 #626 gate) |
| needs the owner | 3 (#636 #621 #622) |
| merged to `main` | `3db6c23e` → `478e6754`, gate **5,211/5,211, 0 failures, 1,324 s** |
| awaiting a gate | `fleet/w633-635` at `ea1a3a39`, six commits |
| display-free checks offloaded | **3/3 PASS, 834 s** on `big_mac` |

## Closed, and on what evidence

| issue | evidence |
|---|---|
| **#630** | one cause behind both symptoms: the finder's index held no `.rs` rows. Red-then-green pair |
| **#631** | `listing` returns `(rows, refusal)`; the panel draws it; `shown` compares both |
| **#614** | `cursor_ground` **20** call sites; `On<Activate>` **117 / 32 files** |
| **#615** | the census refusal names `STEP_TIGHT/ROW/GROUP` and `chrome::Col` |
| **#616** | seven `0.19` citations → 0 |
| **#617** | three dead names → what the tree holds |
| **#620** | three docs now match `bench_advisory.sh`'s own `\|\| true; exit 0` |
| **#623** | the chord, the RON block, the 117-vs-5 count |
| **#632** | the one test of three that actually reads a clock |
| **#635** | **verified on big_mac: 6 tests run, 6 passed** — both `/var` sites |
| **#629** | **not planned**: `+` spawns live bash; my own "placement toggle only" was also wrong |
| **#628** | **answered**: `-D warnings` alone does NOT catch a private-item link; `cargo doc` with that lint does. The step stays |
| **#627** | **answered**: the doctest step is 8x the doc step (554 s vs 70 s) and its cost is the *build*, so it does not duplicate `cargo doc` |

## What was got wrong, and corrected

1. **A guard that flagged 140 legitimate names.** Reverted; the message fix stayed.
2. **Two of #632's three tests read no clock.** `wall_clock_filter.py` refused them, correctly.
3. **The `#617` comment edit shifted two line citations by one** — the gate caught it.
4. **The `#635` fix was incomplete.** I canonicalised line 39, grepped, found one hit, stopped. The verifier on
   `big_mac` **ran the module** and found line 108 with the same defect. Running found what reading did not, for
   the second time this pass.

## Throughput — the answer to the question asked

**Before:** every gate on big, `CARGO_BUILD_JOBS=1`, **96.5% idle**, holding an exclusive lock for the whole push.

**Now:** clippy, `cargo doc` and the doctests run on `big_mac` — **210 s + 70 s + 554 s, 3/3 PASS**, off big's
queue entirely.

**And the real constraint turned out to be memory, not cores.** The gate was **OOM-killed at test 3,315 of 5,211**
after 1h20m of CPU in 19m of wall clock — already parallel. Root cause: **big's swap is zram**, `DATA 23.2G`
compressed into **10.1 GB of its 31 GB RAM**, mostly from exited processes. Stopping containers reclaims nothing
(they total 235 MB).

**Two fixes shipped:** the gate now requires **`SwapFree >= 12 GB`** as well as headroom (against today's numbers
the old rule said START, the new one says WAIT), and the pre-push hook gives the gate **its own target dir** so a
push no longer queues behind rust-analyzer (that cost an hour and three killed-as-hung pushes).

**Blocked, for the owner** (#636, `needs-owner-approval`): either **reboot big** to clear zram, or add a
**disk-backed swap file** so the headroom test measures disk. Until then, every gate on big starts with ~10 GB of
its RAM pre-spent.

## The one thing a fresh session should do first

**`ssh big 'zramctl'`** — if `COMPR` is near 10 GB, the gate will refuse and the work belongs on `big_mac` or must
wait for the reboot. Then gate `fleet/w633-635` (`ea1a3a39`): six commits, all docs/tests/hooks, unproven.
