# 2026-11-26 (second pass) — the gate refuses a tree that cannot compile, three issue premises decay, and 0.8.4 is cut

## What landed on `main`

| commit | what |
|---|---|
| `349c2e49` | the runbook says `worktree add -b <branch> <path> origin/main`, never a working tree (#634) |

`44cda8df` (the audio panel asks the asset server for its sounds) and `82b884bb` (a chord id is refused with the
key it stands for) landed earlier in the same day and **carried a gate record**: `2026-11-26T09:39:12, clippy over
every target, nextest and doctests passed`.

## The guard: `launch-gate.sh` refuses an uncompilable tree in 37 s

A worker that branches from a dirty worktree inherits a half-applied refactor. Its own commit is clean, its report
is honest, the tree holds one half of a two-sided change, and the gate fails at minute four with a compile error
that reads as "that issue's fix is broken" (#634).

`launch-gate.sh` now runs `cargo check -p bevy_dark_factory --all-targets` under the heavy lock before it takes the
unit, and refuses:

```
checking the tree compiles before gating it...
REFUSING: the tree does not compile as it stands. The first errors:
error[E0308]: mismatched types
error: could not compile `bevy_dark_factory` (lib) due to 1 previous error
```

37 s against a 726 s gate, and it ran on the real path today before the 0.8.4 gate.

**It cost three wrong turns, all worth recording:**

1. **My first probe put the error in a `#[cfg]`-gated module and passed.** `audio_panel.rs` is behind
   `bevy_siren` (`lib.rs:167`), so the default build never compiles it and neither probe nor gate can see an error
   there. A gated module needs the feature passed to the check.
2. **`cargo check` said `Finished` in 0.30 s with the error present**, and kept saying it with `RUSTC_WRAPPER=`
   cleared and a fresh `CARGO_PROFILE_DEV_PACKAGE_*_CODEGEN_UNITS`. It was right each time — the file was gated out
   — but a fast `Finished` on a tree you believe is broken reads exactly like a stale fingerprint or an `sccache`
   lie. Check `#[cfg]` before blaming the cache.
3. **A clean `CARGO_TARGET_DIR` took 227 s** to answer the same question. Do not use one to test a pre-check.

The same error in `chrome.rs`, which is unconditional, is refused at once.

## Three premises had decayed, and the measurements say so

- **#613 "56 layout tests pin a literal width"**: at today's HEAD, `divider_w: 29.0` fails **3 tests, not 56** —
  verified twice, once by a worker and once by me on the full suite (`2,851 tests run: 2,848 passed, 3 failed`).
  None of the three is a literal to derive: one is a doc-pin that *should* fail when the metric moves, one is a
  653-vs-654 px rounding question, one measures a slide. Four real literals did get derived (`the_g87_nodes_are_
  gooeys.rs` ×2, `the_agent_blocks_lines_are_gooey_rows.rs`), and one was left literal with the reason measured:
  deriving it from `viewport_share` made the test reject the layout it accepts today.
- **#672 "six of the slowest tests hold independent cases in one serial test"**: the chip sweep is not a boundary
  search. Its predicate is every width, and all 901 widths pass in both directions; a binary search would "pass"
  while measuring nothing. The split into six parts did land: **29.8 s → 22.1 s** wall, longest test **29.5 s →
  11.9 s**. Six parts is measured, not chosen (3 parts: 26.5 s; 9 parts: 26.5 s).
- **#634's second item** is done and proved: the compile guard above.

## #645: a document tab is gooey's `edge_tab`

`session::draw_the_tabs` built each tab by hand — a `row_with`, a literal `Spacing`, a 3 px `BorderPx`, an
`InkOverride`, two colours baked at spawn — and **no `Role`**, so the skin never styled it and there was no hover
state at all. It is now `bevy_gooey::edge_tab`, the same widget `tiles.rs` builds its strip from, with selection
through gooey's own `Selected` contract.

The test failed before with **"2 of 2 document tabs carry no `Role::EdgeTab`"** and passes after. That matters:
the previous session's attempt reported `[]` — zero tabs — which was its own bug, not the strip's guard. Two
documents do draw two tabs.

## The gate's record now reaches origin, and the release refuses a tag without one

`refs/notes/gated` held 23 notes locally on `big` and 17 on origin. Merged and pushed: **26**, origin at
`f1e919d4`, and a fresh fetch on this Mac shows `44cda8df`'s record. `release-macos.yml` then **refused** the
0.8.4 tag I cut on `349c2e49`:

```
require-gate-record: REFUSED: 349c2e496f638db8494eec843ef29d92616c2534 has no record in refs/notes/gated
```

Right refusal, and it exposes a hole: `349c2e49` is docs-only, so the hook's fast path skipped the suite, and
every release tag is cut on whatever main's tip is. `git diff acc26aab 349c2e49 -- crates Cargo.toml Cargo.lock
tools` is empty, so gating `acc26aab` proves the same suite for the tag's code.

## #531 bit again: six zero-byte objects stopped every fetch

`big`'s `dark-incorporated/bevy_dark_factory/.git/objects` held **six zero-byte loose objects** and a stale
`maintenance.lock` from 2026-11-08. `git fetch` failed `loose object ... is corrupt`, `unpack-objects failed`,
which is why the notes could not be pushed. Moved aside to `/tmp/fleet/corrupt-objects-20261126-*` (not deleted);
git reads 57,323 objects again. This is what an unclean reboot leaves behind.

## The lock that is not a lock: a shared `CARGO_TARGET_DIR` starves the gate

Measured again from the other side today, and it cost about forty minutes of wall-clock across one gate run.

Three workers and one gate were all pointed at `$HOME/.cargo-target-gate`. The heavy `flock` I give each cargo call
does not help, because **cargo takes its own build-directory lock underneath it**:

```
pre-push gate: clippy and the test suite on c5d222aaa; this takes minutes
    Blocking waiting for file lock on build directory
```

The gate held `big-heavy.lock` and sat idle while a worker's `cargo nextest` owned the directory. Both were
"running"; neither was compiling. The `flock` wrapper rations *memory* — it serialises compiles so two Bevy builds
cannot drive the host into swap — but a shared `CARGO_TARGET_DIR` serialises them again inside cargo, where no
wrapper can see it and neither party can report it.

**The rule, now in the briefs:** one `CARGO_TARGET_DIR` per role.

| role | directory |
|---|---|
| the gate | `$HOME/.cargo-target-gate` |
| a worker | `$HOME/.cargo-target-w<issue>` |
| a person, or `rust-analyzer` | `$HOME/.cargo-target-shared/global` |

The hook already diverts a caller who inherits `…-shared/global` into `…-gate` (line 37), so a person's push is
safe. What it cannot fix is an agent that follows a brief naming `-gate` explicitly — which mine did. One cold
build per role, paid once, against a stall that is unbounded and silent.

**The lock is not the bug.** The gate holding `big-heavy.lock` exclusively is correct: it keeps a 5,200-test suite
from racing a worker's compile for RAM. The shared directory is.

## Open

**42 issues, 462 branches.** The fleet has been generating branches faster than they merge; that backlog, not any
single defect, is now the work.
