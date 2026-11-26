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
