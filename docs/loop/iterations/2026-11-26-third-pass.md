# 2026-11-26 (third pass) — the gate is the reviewer, and the host is the bottleneck

## What the gate proved, in one sentence each

**A 5,233-test gate ran to completion on the integration branch (main + #645 + #647 + #650): `5,229 passed,
4 failed, 13 skipped, 445 s of suite`.** Every one of the four is named, and three are real.

| test | verdict |
|---|---|
| `a_documents_citations_resolve::the_design_pages_citations_land_on_their_symbols` | **real** — six design-page citations pointed at lines #650 moved. Fixed, re-pinned (`d18b0670`). |
| `a_panels_inset_follows_the_skins_density::without_the_override_…` | **real** — the test asserted a capability that does not exist. `Sides::overlaid`'s `over.top.or(self.top)` means a style layer can narrow a side but never clear one. |
| `the_status_bands_kind_draws_a_mark::a_warning_draws_the_skins_accent_…` | **real: the test compared a value with itself.** `mark_ink(&app, &clean)` was read after the warning was recorded. Fixed, `3 tests run: 3 passed`. |
| `the_file_editor_edits_project_files::an_unknown_key_is_a_diagnostic_…` | **flake** — passes alone on the same tree (`PASS [6.495s]`) and on the baseline. #580. |

A second gate run on all four branches (`5,236 tests, 5,232 passed, 404 s`) found three more, and each was
somebody's mistake finally meeting a compiler:

- **two raw-node ratchets** — `chrome.rs carries 3 raw `Node` literals and is counted at 2`. #551 wrote
  `Node::default()` where `bevy_gooey::row(0.0)` is `bevy_gooey::chip`'s own first part. The ratchet is right, and
  the fix is gooey's part, not a raised count.
- **`a_reload_is_measured_on_the_clock`** — 12 hours apart (`18.5` vs `6.0`). Bisected across all four branches:
  passes at #645 alone, at #647 alone, at #645+#650, and at all four merged. It fails only in the tree that had
  none of the compile fixes. The clock tests are **not** broken by any branch.

**And #650's other three tests passed by execution**, which upgrades its report: the 5.0 gap, the 4.0 corner and
the 12.0 inset were measured on a real gate, not reasoned from source. My earlier "no green run" note was too
broad; corrected on the issue.

## The finding that matters more than any of the four

**A style layer cannot un-state a property.** `StyleDecl::padding` is `Sides<Space>`, merged per side by
`Sides::overlaid` with `over.top.or(self.top)` (`bevy_gooey` `src/box_model.rs:117-125`). A `None` keeps the value
below it. So:

- `{margin: !Step 2}` leaves the shipped `padding: !Step 2` in force → 12 px (what the test measured);
- `padding: !Step 0` draws 3 px, `padding: Zero` draws 0 → neither lets `skin.pad()` through.

There is no "unset" in the vocabulary. The only place a declaration can be *removed* is the layer that owns it —
the base sheet, which is a live file the editor watches. That is the instrument, and it is what `PanelRole650` is
measuring now.

## The host is the bottleneck, not the code

Measured today, in order:

- **`ollama`'s `llama-server` held 350 % CPU and 3.7 GB for over two hours** on a GPU lease I held. It is a
  *system* unit, so `gpu-tenant`'s eviction (which unloads llama-swap models) never touches it.
- **`sccache` had no systemd unit**, so it died with every ssh disconnect — "The server looks like it shut down
  unexpectedly, compiling locally instead" — and every build after a disconnect was cold. It is now
  `sccache.service`, `Type=oneshot`, `RemainAfterExit=yes`, enabled.
- **The gate is OOM-killed at `MemoryMax=20G`** with services holding ~7 GB of a 31 GB host. Raised to 26 GB.
- **A gate on `big_mac` is not the same gate**: no `git-lfs`, so two `bevy_oracle` image tests fail on un-smudged
  pointers. Both tests are right.

Net effect: a 445-second suite took **three hours and four attempts** to run, and one of those attempts was
killed at minute eight with 35 processes.

## Open

**39 issues.** Four branches are built and reviewed; three are mergeable once the status-band mark is understood,
and #551 (`d56c3106`) is ready to join them.

## What the gate found, in the order it found it

Five separate defects, none of which any worker's report could contain, because every worker's runs were
OOM-killed before reaching the compile:

| found | what it was |
|---|---|
| `borrow of moved value: panels` | #650's 167-line test **had never compiled** |
| `6 citation(s) land on something else` | #650 moved `chrome.rs` +42 net; six design-page citations pointed at unrelated lines |
| `left: (12.0, 12.0)`, expected `(6.0, 6.0)` | the density test asserted a lever the style vocabulary cannot express |
| `left: Srgba(0.95, 0.71, 0.3)` == `right: Srgba(0.95, 0.71, 0.3)` | the status-band test compared a colour **with itself** |
| `chrome.rs carries 3 raw Node literals and is counted at 2` | #551 wrote `Node::default()` where `bevy_gooey::row(0.0)` is the chip's own first part |

**Every one was fixed, and every fix was in a test or a comment except the raw node.** The `src/` changes the
workers made were sound; what they could not do was see their own work compile.

## The style vocabulary has no unset

The density failure is the most useful finding of the day, because it is a limit someone will hit again.

`StyleDecl::overlaid` merges a box per side with `over.top.or(self.top)` (`bevy_gooey`
`src/box_model.rs:117-125`). A `None` contributes nothing. So:

| a layer writes | drawn padding |
|---|---|
| `{margin: !Step 2}` | 12 px — the shipped `padding` survives |
| `padding: !Step 0` | 3 px — narrower, not a clear |
| `padding: Zero` | 0 px |
| no matching rule states `padding` | **`skin.pad()` — 6 px** |

A later layer can narrow a property but never clear one. Only the layer that **owns** the declaration can drop
it, and that layer is the base sheet — a live file the editor watches. So the honest claim for #650 is narrow: *a
person who edits their own sheet's `.chrome-panel` rule gets `skin.pad()` behind their panels.* Before the change
the panel was `Role::Sized`, whose arm writes no padding, so the same edit left every panel at **0 px**.

## The host, measured

| fault | cost |
|---|---|
| `ollama`'s `llama-server` at 350–480 % CPU for 3 h 40 m | a *system* unit, so `gpu-tenant`'s focus eviction (which unloads llama-swap models) never touched it, even while I held the lease |
| `sccache` with no systemd unit | died with every ssh disconnect: "The server looks like it shut down unexpectedly, compiling locally instead". Every build after a disconnect was cold. |
| the gate at `MemoryMax=20G` | OOM-killed with 35 processes at minute eight; services held ~7 GB of 31 |
| my own `sccache.service` at `SCCACHE_CACHE_SIZE=80G` | `systemd-oomd` killed 72 processes in its unit, because a `oneshot` unit with `RemainAfterExit` owns every descendant |

**The recipe that finally worked:** `RUSTC_WRAPPER=` cleared entirely, `CARGO_BUILD_JOBS=3`, and a host at load 8
with 8 GB free. A 445-second suite had taken four attempts and three hours.

## End of the day: a fully green gate, and the release flow's deadlock

**`ab3fc1d8` passed the whole workspace: `5,233 tests run: 5,233 passed, 13 skipped, 371 s`,
`pre-push gate: passed in 941s`.** The note is on the commit and pushed: `gated on big
2026-11-26T18:53:48: clippy over every target, nextest and doctests passed`. That carries #645, #647 and #650 to
`main`, and all three are closed.

**Fourteen gate runs produced that one.** The others were: two OOM kills, one `sccache` failure mid-compile, one
gate that gated a tree without the fixes, one that gated a tree whose merge had dropped a test file, and seven
that found a real defect. The seven are the value; the other seven are the host.

## The release flow cannot cut a tag

`f76bebee` is the `release: 0.8.5` commit — two lines, the manifest version and `Cargo.lock`. The hook's bump fast
path exits at `pre-push:134`, **before** `record_gated` is reached, so the bump carries no note. Then
`require-gate-record.sh` refuses the tag:

```
require-gate-record: REFUSED: f76bebeef47d has no record in refs/notes/gated
```

**The deadlock is structural:** every release commit is a bump, every bump skips the tests, and only a test run
writes the note. So no release tag can ever pass, and the check that exists to stop an ungated release stops all
of them.

**The fix, which is the honest one rather than the fast one:** a bump now writes a note that *names the parent it
inherits from* rather than claiming a run of its own —

```
version bump f76bebee inherits the gate of ab3fc1d8: gated on big 2026-11-26T18:53:48: ...
```

— so the record is true about which commit's tests the tag's commit rides on. `notes_ref` is named inside that
branch because the file declares it further down, past the fast paths, and an unset variable there would have
silently skipped the note.
