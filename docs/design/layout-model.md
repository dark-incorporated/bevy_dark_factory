# A layout model: the workbench stated as data

**Issue #521** (`feedback-1795094182023`). Research, with measurements. **Nothing was re-implemented.**

**Since 2026-11-22** the measures this page once named as constants (`CONTROLS_W`, `LIST_W`, `BOTTOM_DOCK_H`,
`WORKBENCH_FLOOR_H`, `PAD`, `GAP_ROW`, …) are **`metrics.layout` values and class rules** of the editor's live style
sheet, and the bare names no longer exist in `chrome.rs`:
`crates/bevy_dark_factory/src/style/editor.yaml` ([data files](../guide/data-files.md#the-editors-style-sheet)). **§2.2's fixed
list is read from that sheet and `metrics.layout` by `tests/the_documents_state_the_codes_own_counts.rs`**, so it cannot drift;
every other number here is as measured at the time.

The owner's note, in full:

> *"This layout is a hotmess. We need a mental and data models for layouts like this. Ultimately, we need a damn good UX."*

**Status: a model and the defects it exposes. No layout code changed.** Every number below was measured
in this worktree at `f1e65d76` and `b64c689f`, `bevy_gooey` at the pinned rev `f1904be`, on the capture's
own frame — `feedback-1795094182023`, Dark Factory 0.7.1, `ui_scale` 2.4, window 2176x1406 physical
(1088x703 logical), editor `bedroom`, `Place` armed, Meshes door.

---

## 1. What the picture is

The pick is `Sized > FillRegion > Split > SplitPane > Sized > Sized > ScrollViewport`, `rect_px`
`[120, 533, 553, 1214]` — the **INSPECTOR** panel's scrolling body, on the **left**, at `ui_scale` 2.4.

Read off the frame at 2.4:

| Part | Frame, physical px | Logical px | Where it comes from |
|---|---|---|---|
| INSPECTOR panel | x `89..583`, y `335..1314` | **206 x 408** | `chrome::panel_root(Side::Left, controls_w = 380, …)` — the sheet's `metrics.layout.controls_w` |
| its scrolling body (the pick) | x `120..552`, y `533..1213` | **180 x 283** | `chrome::scroll_list` inside the panel |
| PIECES panel | x `1767..2146`, y `335..1314` | **158 x 408** | `chrome::panel_root(Side::Right, list_w = 264, …)` — the sheet's `metrics.layout.list_w` |
| its rail | x `2112..2115` | 1.7 wide, 30 px in from the panel's right border | gooey's `ListRail` |
| the viewport (the map/world hole) | x `~645` to `~1767` | ~467 wide | what the two docks leave |
| the split divider | the light column `636..644` | 3.3 visible inside the target this build drew (24; `metrics.layout.divider_w` is 12 since #476) | gooey's `Role::SplitDivider` |

Two things to note before any defect is named:

- **The panel asks for 380 and is drawn at 206.** The controls panel's preferred width is 380 logical px, the sheet's
  **`metrics.layout.controls_w`** (`style/editor.yaml`, read as `layout.controls_w`; the constant `CONTROLS_W` this page
  used to cite is gone), and `fit_docks_to_the_window` (`chrome.rs:1193`) squeezes it when the window cannot pay both
  docks and the viewport's share. At this window the squeeze is `206 / 380 = 0.54`.
- **The panel's content is 180 px wide because the panel is 206 and keeps a margin of 12 each side, a pad of 12 each
  side, a 1 px border, and the scroll viewport's own `scroll_gutter()` (`scroll_bar_w` 5 + `scroll_bar_inset` 1) on its
  right.** So the panel a person reads is 54% of the width the panel's builder was told to want.

---

## 2. The current layout, as data

### 2.1 The containers

The workbench is a **tree of four kinds of container**, and nothing else:

```text
window root                                   Sized, 100% x 100vh
├─ chrome_bar                                 row, 100% x bar_h (26)      1 px bottom edge
├─ door_strip                                 row, 100% x auto            2 px gaps, STEP_PAD left
├─ body                                       fill_region, row, 100% x flex:1
│  ├─ split (ShellSplit)                      SplitAxis::Horizontal, SplitFraction, SplitMin
│  │  ├─ SplitPane  #1 (left)                 grid 1fr/1fr, flex_grow .5, overflow clip
│  │  │  └─ Frame::left  (left dock)          column, as wide as its widest visible panel
│  │  │     └─ … panels, each panel_root()    Sized, width = PanelWidth.preferred (squeezed)
│  │  │        ├─ title / heading             text
│  │  │        └─ scroll_list → ScrollViewport  fills what is left, scroll_y, rail as sibling
│  │  ├─ SplitDivider                         `metrics.layout.divider_w` (12 px): the room the shell's split takes
│  │  └─ SplitPane  #2 (right of the split)   grid 1fr/1fr
│  │     └─ Frame::viewport  (ViewportSlot)   fill column, row of …
│  │        └─ ArrangementDock (cover)        gooey Dock, absolute over the whole hole
│  └─ Frame::right  (right dock)              column, as wide as its widest visible panel
│     └─ … panels, e.g. PIECES                list_w = 264, squeezed to 158 here
├─ bottom  (BottomDock)                       dock, 100% x bottom_dock_h (148), ShownFollow
└─ status                                     row, 100% x bar_h, 1 px top edge
```

The whole tree is built by `spawn_frame` (`chrome.rs:1296`), which is why every measure in it is a local read of the
sheet rather than a constant.

### 2.2 The sizing rules, which are the model

Each part answers exactly one question, and the whole model is these four answers:

| Rule | What holds it | Where |
|---|---|---|
| **A panel's preferred width** | a sheet value per panel role: `metrics.layout.controls_w` 380 for a controls panel, `metrics.layout.list_w` 264 for a list; and the one that is still a constant, `WORKSPACE_PAGE_W` 560, for the Project page | the two dock widths are **sheet values** in `crates/bevy_dark_factory/src/style/editor.yaml` (`controls_w`, `list_w`), read as `layout.controls_w` and `layout.list_w`, because the constants that held them are gone; `WORKSPACE_PAGE_W` (`project_page.rs:354`) |
| **A dock's width** | **not stated**: it is its widest *visible* panel plus that panel's 2x margin, because a gooey dock takes its widest child (its automatic minimum) | `fit_docks_to_the_window` (`chrome.rs:1193`) |
| **A panel's drawn width** | its preferred width, squeezed by `fit_docks_to_the_window` so the docks + the split divider + the viewport's owed share fit the window | `fit_docks_to_the_window` (`chrome.rs:1193`) |
| **A pane's width** | the split's `SplitFraction` of the row, less the divider, held to `SplitMin` for each pane — *except* that `size_the_shell_split` overwrites the fraction to make the first pane as wide as the panel in it, and `the_left_panel_follows_its_pane` writes the panel back from the pane once a person has dragged | `size_the_shell_split` (`chrome.rs:3826`), `the_left_panel_follows_its_pane` (`chrome.rs:3907`) |

**Which are fixed and which flex:**

- **Flex**: the two docks' *widths* (they shrink toward the sheet's `dock_min_w()`, `list_w * dock_min_share`), the
  panels inside them (their contents reflow), the `SplitPane` widths (the fraction), the scroll viewports (fill the
  panel's rest), the bottom dock's *display* (it appears only when `metrics.layout.workbench_floor_h` 420 is still
  kept).
- **Fixed**: `metrics.layout.bar_h` 26, `metrics.layout.bottom_dock_h` 148, a panel's margin and pad (12 each,
  the sheet's `chrome-panel` rule), the row step 5, the 1 px borders (`metrics.layout.edge_w`), the shell split's
  **`divider_w` 12** (the owner's half-thickness divider, #476, which the editor sets and `bevy_gooey` draws; its own
  default channel is `bevy_gooey`'s `MIN_TARGET_PX` 24 and its private grip is `SPLIT_GRIP_THICKNESS` 4, neither of
  which this editor's sheet names), `scroll_gutter()` = `scroll_bar_w + scroll_bar_inset` = 6, `dock_min_w()` =
  `list_w * dock_min_share` = 158.4, `metrics.layout.viewport_min_w` 420, `viewport_share` 0.52.

### 2.3 The mental model, in one sentence

**A panel says how wide it would like to be; the frame answers with what the window can pay, and a
panel is allowed to lose its width but never its rows.** That is the model the code implements — and
it is a good one. What the picture shows is the two places where it is enforced inconsistently, and
one place where "never its rows" is silently false.

---

## 3. Where the picture shows the model breaking

Three defects. Each is stated as what a person sees, what the model says should happen, the
measurement, and what would settle it. **None of them is fixed here** — that was the issue's
instruction.

### D1 — The panel is squeezed to 54% but its contents are not re-planned for it

**Seen**: the INSPECTOR at 206 logical px reads as a column of broken phrases. `SUBGRID`'s first
value line `1 x 2 x 1 cells of 500 mm` is 180 px of text in a 180 px viewport — it fits on exactly one
line and any one more character wraps; `50 mm /` is clipped through the glyphs at the viewport's top
edge (measured: the ink at `y=626..638` physical is the bottom half of a line whose top is above
`y=533`); `770 tris` is clipped at the bottom edge (`y=1210` physical has ink, `y=1214` none).

**Model says**: `fit_docks_to_the_window` shrinks a panel *down to* `DOCK_MIN_W` (158.4) and the
rows reflow (`WordOrCharacter`), so a squeezed panel is legal. `the_left_pane_reflows_when_the_divider_moves`
(#482) already holds the *box* inside the pane.

**Measurement**: the panel is at **206** of its **380** preferred — 54%. `DOCK_MIN_W` is 158.4, so
206 is legal and 206 is what a 1088-logical-px window can pay with the PIECES dock at 158 and the
viewport owed 420.

**What it exposes**: a panel's layout is authored **once, for its preferred width**. There is no
statement anywhere of a panel's width *range*, so the layout cannot reflow between "this is a
two-column table" and "this is a column of phrases". The squeezed panel is not *broken* — nothing
overflows — it is *unreadable*, and the model has no word for that. A width-range per panel (`compact`
below X, `regular` above) is the missing datum.

**Settles it**: a test per panel that measures, at `DOCK_MIN_W`, the number of lines every
label/value row wraps to and asserts it is at most two — that is a number the model can hold.

### D2 — The scroll viewport's clip cuts rows, and its rail says so at an angle

**Seen**: the INSPECTOR's body shows a half line at the top and a half line at the bottom at once; the
thumb (measured light column `x=549..552`, i.e. flush with the viewport's right edge) spans nearly the
whole track, so a person cannot tell how much is above or below.

**Model says**: a scroll viewport clips at its box and its rail shows `content > size`
(`update_lists`, `list.rs:99`). Both are true here. What is not in the model is **why the box is 283
px tall** on a 408 px panel: `title` + the sheet's `chrome-panel` padding + heading + the body's own
group gap (was `GAP_GROUP`, now the `.chrome-gap-group` rule) + the status line under the viewport take
the rest, and *those are content-sized*, so the body gets the leftover. A
panel whose fixed chrome is 30% of it has a body that shows 4 rows of a 20-row list.

**Measurement**: panel 408 tall; viewport 283; the chrome above and below it 125 px — 31%.

**What it exposes**: the model has a preferred *width* per panel and **no height budget at all**.
`BOTTOM_DOCK_H` 148 and `WORKBENCH_FLOOR_H` 420 are the only vertical numbers, and both are the
bottom dock's. Nothing says "a panel's fixed chrome may not exceed N% of its height".

**Settles it**: a measurement of every panel at the reference window (1280x720) of
`(fixed chrome) / (panel height)`, and a floor on the viewport's share.

### D3 — Three scrollbars in one eye-line, and only one of them is this panel's

**Seen**: the frame shows the INSPECTOR's thumb at `x≈550`, the divider's grip at `x≈640`, and the
PIECES rail at `x≈2113` — three vertical light marks at three different insets (30 px from a border,
centred in the divider target that build drew, flush with a viewport's edge). Nothing on screen says which belongs to what.

**Model says**: each is right by its own rule — a rail is placed over its viewport's right edge
(`place_scroll_rails`), a divider's grip is centred in its target (`split_divider`), and a dock's rail
is inset by the panel's `MARGIN`. There is **no rule that they line up**, and no rule that says two
bars in one view are one gesture or two.

**Measurement**: viewport rail flush at its viewport's right edge, divider grip centred in the divider target
(the capture's build drew 24; `metrics.layout.divider_w` is 12 since #476), panel rail 30 physical px in from
the panel's border.

**What it exposes**: the model has no notion of **chrome adjacency** — what two controls next to each
other mean together. `SCROLL_GUTTER` exists so content does not run *under* a rail; nothing exists so
a rail does not run *beside another rail's gesture*.

**Settles it**: a capture of the three marks at one scale with their insets measured, and a decision
(either inset the viewport rail like a list's, or take the divider's grip out of the eye-line when a
rail is beside it).

---

## 4. What is missing from the model, stated as data

The three defects are one gap seen from three sides. The model is a tree of flex boxes with
per-panel preferred widths. What it does not have, and what the picture asks for:

1. **A width range per panel.** A panel is authored for one width and squeezed arbitrarily. It needs
   `{ compact: N, regular: M }` so the layout can say what it becomes.
2. **A height budget per panel.** Fixed chrome vs the scrolling body is decided by whatever the content
   measures. It needs a stated floor on the body's share.
3. **Chrome adjacency.** What a rail, a divider and another rail mean when they share an eye-line. It
   needs to be a rule, not three systems' coincidences.

Those three are the "data model for layouts like this" the note asks for. The tree, the four sizing
rules, and the fixed/flex split in §2 are the mental model, and they are already right — they are
just not written down anywhere a person changing a panel would find them.

---

## 5. Defects, and what each needs

| # | Defect | Needs | Why a `TODO` is not enough |
|---|---|---|---|
| D1 | A panel squeezed to 54% is a column of broken phrases | a width-range per panel, and a per-panel wrap measurement at `DOCK_MIN_W` | "make it wider" is not available at this window: the viewport is owed 420 and the other dock 158 |
| D2 | The body is 31% of the panel, showing half rows top and bottom | a stated height floor on a panel's scrolling body | the number is measurable today (283 of 408) and nobody measures it |
| D3 | Three vertical marks at three insets, unrelated | an adjacency rule, or one inset for every rail | each is right by its own system's rule, so no single system's test can see it |

Every one of these is measurable on the shipped frame, at the capture's own `ui_scale`. None of them
needs a new capture to be stated — only a decision.
