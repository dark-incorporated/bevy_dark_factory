# The feedback rows the old loop could not finish: re-checked 2026-11-15

Written by fleet worker `Rows9` against the shipped **Dark Factory 0.6.8** (main `c21f8ff3`), driven over BRP on bmb
(macOS arm64, window 2560 x 1440 physical, scale factor 2, UI scale 2.4, i.e. the captures' own scales).
This replaces the pass-399 note, which is stale: bevy_gooey#44, #45, #79 and the editor's #344 are all CLOSED.
Evidence pictures are on bmb under `~/fleetscratch/rows9/shots/`.

| row (stem `feedback-<id>`) | verdict | evidence | issue |
|---|---|---|---|
| 1793927671762 key-cap letter off centre | **FIXED** | gooey#44 closed 2026-11-06. Live: the `X` keycap of the Remove-mode tooltip is box `401..446` px, the glyph is 17 px from the left inner edge and 17 px from the right (`row1_keycap_X_0.6.8.png`) | gooey#44 (closed) |
| 1793927744511 shortcut "?" / tofu | **FIXED** | `edb51e56` spells chords in words (gooey `ChordGlyphs::Words`, gooey#45 closed, v0.37.0). Live: Undo draws `Cmd+Z`, Redo `Shift+Cmd+Z`, in the tooltip and the menu row; no glyph the face lacks (`row2_row3_tooltip_Shift+Cmd+Z_0.6.8.png`) | gooey#45 (closed) |
| 1794096613481 shortcut tooltip changes width | **FIXED** | gooey#79 closed (4850058, v0.44.0, `tests/tooltip_cap.rs`). Live: hovering Erase, Undo, Redo and sampling every text rect 80 to 120 times (about 6 s each): `Remove mode` stays `x300.0 y276.9 179x33`, its `X` keycap `424.0, 276.4 45x46`, `Cmd+Z` `837.0 99x46`, `Shift+Cmd+Z` `917.0 197x46`; one value each, no flip | gooey#79 (closed) |
| 1794024825941 What's new asks for a prose face | **NEEDS A DECISION** | still FiraMono everywhere, sentences set one column per character (`row4_whatsnew_0.6.8.png`); contrast is 10.93:1, so only the face is the ask. gooey already has `GooeyFonts {body, heading, mono}`. Options: keep monospace; Fira Sans for prose only (recommended); a setting | dark-incorporated/bevy_dark_factory#469 |
| 1794103491165 launcher card needs two presses | **FIXED** (decided) | #344 closed; one press opens (`dbf1147c`, `062521f6`, `734478b2`; owner exception 2026-11-10 re-pinned the tests). Live: one real press on the project card moved `blueprint/state.screen` from `launcher` to `project` | #344 (closed) |
| 1793928159971 menu hover highlight 38.5 px below the cursor | **NOT REPRODUCING** | in the View menu (27 rows, taller than half the window), at 5 cursor heights fresh and 5 after 3 wheel notches, the item bevy_picking hits, the item gooey fills and the item whose layout rect holds the cursor are the same row every time; a one-notch wheel scroll with the pointer still re-aims the hit and the fill in 50 ms; the drawn band is at the cursor in the screenshot (pixel check). Only caveat: a disabled row (the Meshes menu while on the Map tab) is never filled, by design | none |
| 1793969010186 a real click on Settings does nothing | **NOT REPRODUCING** | real injected press (cursor, Press, Release) on the sidebar's Settings chip opens the dialog from the launcher and from the project page, at the centre and 4 offsets of the chip (`row7_settings_dialog_after_real_click.png`). The capture's own pick (274.9, 1180.1 physical) was on empty sidebar, not on the chip, which sat at y 1477. `escape_closes_settings_opened_by_the_homes_button` also clicks it through picking. The cause of the old failure was never identified | none |
| 1793928091805 menu "?" and ragged column | **FIXED** | same cause and fix as 1793927744511: shortcuts are words now, and the right edges of the shortcut column are one value to within 1 px (998 / 997 physical in View, aligned in `row8_menu_shortcuts_words_0.6.8.png`) | gooey#45 (closed) |
| 1793928048623 Pieces panel overflow on the Meshes tab | **STILL REPRODUCES** | Kit workspace, MESHES tab: `dimensional_crab` (65 tall) and `kenney_prototype-kit/Models/GLB format` (130 tall) sit in 58-tall bands and overlap the rows around them by 26 and 12 px; the hint `right: meshes / left: tiles` ends 32 px past the content edge; `surveillance_console.glb` is drawn to the window edge, past the panel border (`row9_pieces_panel_0.6.8.png`) | dark-incorporated/bevy_dark_factory#468; suspected upstream bevy-coven/bevy_gooey#105 |

## Not in the nine, seen on the way
- `feedback-1793927950267` (the What's new tab): a real press on the sidebar's What's new chip switched the page on 0.6.8; not tested further.
- `feedback-1793968919654` (packs open by default): not re-checked.

## Traps worth keeping
- Pressing Escape with no menu open raises "Leave this map?"; a hover test that sends Escape first ends up hovering the dialog's scrim (`HoverMap` names a `Role::Scrim`).
- The Meshes menu's rows are disabled on the Map tab, so they never fill under the pointer: use View or Edit to test hover.
- `bevy_picking::hover::HoverMap` and the `PickingInteraction`/`Hovered` components are readable over BRP: they are the picking/hit report B-class rows needed.
