# The editor's icon vocabulary

**Issue #493** (`feedback-1794921635359`). A recommendation, with the measurement that justifies it, written down before the
atlas grows again.

The owner's note, in full:

> *"The "v" should collapse the text. And we should consider an icon solution for icons like up and down arrows, etc. Like font awesome, etc."*

**Status: decided and implemented.** The fold controls draw `▼`/`►` (U+25BC/U+25BA) and the mark is inside the button that
folds, so a press on the mark itself folds. The rule below is what the next mark follows. `docs/ui/vision.md` §9a states
the same rule in one paragraph, beside §9's rule about which face a text is drawn in.

Measured in this worktree at `718e0146`, `bevy_gooey` at the pinned rev `f1904be`.

---

## 1. The two halves of the note are one question

The note says two things and they are the same thing.

**The `v` is a letter.** `editor.rs`'s PLACE palette drew a category header's disclosure as the ASCII letter `v`, and `>`
when folded (`Text::new(if folded { ">" } else { "v" })`). `tiles.rs` drew the same pair in four more places: the MESHES
pack band, the `EXCLUDED` band, and the Fill page's column and row headers. A person reading a gutter sees a character
from a code listing, not a control.

**The `v` is not the control.** In `bevy_gooey`'s code editor (`code_overlay.rs` at the pin) the chevron is spawned with
`Pickable::IGNORE`, so the mark that looks like the affordance is not hit; only the gutter band beside it takes the click.
That half is gooey's and is filed there as G-82 (`dark-incorporated/bevy_gooey#107`). In this workspace the marks were
already inside a `Button` (a gooey `band`, a header row), so a press on the mark already reached the fold — but nothing
said so, and nothing tested it.

Both halves ask the same upstream question: **what is the editor's vocabulary for a small mark?** Today it has exactly one
answer — `crates/bevy_dark_factory/src/icons.rs`, an embedded PNG atlas.

## 2. What the atlas is, measured

| what | measured |
|---|---|
| the sheet | `crates/bevy_dark_factory/assets/ui/icons.png`, **128 × 48 px**, **976 bytes** |
| cells | 8 columns × 3 rows = **24 cells**, each **16 × 16 px** |
| drawn by | `tools/ui/make_icons.py`, by hand from primitives — no model, so the licence is the repo's |
| named at | `icons.rs`'s `ICONS` (24 names: `edit.undo`, `panel.library`, …, between the `icons:begin` / `icons:end` markers) |
| checked by | `tests/every_toolbar_command_has_an_icon.rs`, which reads the PNG back and fails on a name with no art, art with no name, or two glyphs drawn alike |

It is a good fit for exactly one thing: **a fixed 16 px square standing on its own** — a toolbar command, a panel's tab.
It is a bad fit for everything else, and the three reasons are structural rather than a matter of taste:

1. **It is raster at one size.** A mark in a line of 11 px text is 11 px of art; at the person's 175 % text size it is
   19 px. The atlas has one drawing.
2. **It cannot take a role's ink.** The sheet is white-on-transparent and gooey tints it (`IconTone`); a mark in a text
   run takes the ink its `Role` gives it — a band's pressed state, a row's hover, a selected tab — for free.
3. **It is a repaint per glyph.** A new mark means editing `make_icons.py`, regenerating a binary, and the PNG-reading
   test; a character mark is one literal in the file that draws it.

## 3. What the shipped face actually has

Read from the face's own `cmap` (`ttf_parser::Face::parse`, the same read `tests/the_terminal_draws_in_a_nerd_font.rs` and
`ui.rs`'s chord check make). The face is `crates/bevy_dark_factory/assets/fonts/FiraMono-Regular.ttf`, **174,632 bytes**,
SIL OFL 1.1, already vendored, already installed over Bevy's default asset id.

| has | codepoint | | lacks | codepoint |
|---|---|---|---|---|
| `▼` | U+25BC | | `▾` | U+25BE |
| `►` | U+25BA | | `▸` | U+25B8 |
| `▲` | U+25B2 | | `▶` | U+25B6 |
| `◄` | U+25C4 | | `◀` | U+25C0 |
| `▲`/`▼` up/down | U+25B2, U+25BC | | `⌄`/`⌃` | U+2304, U+2303 |
| `×` | U+00D7 | | `✕` | U+2715 |
| `›` | U+203A | | `⋮` | U+22EE |
| `◆` `●` `○` | U+25C6, U+25CF, U+25CB | | `⋯` | U+22EF |
| `←` `↑` `→` `↓` | U+2190–U+2193 | | `▽` `▷` | U+25BD, U+25B7 |
| `…` `·` `—` `≡` `•` | U+2026, U+00B7, U+2014, U+2261, U+2022 | | | |

**That table is the whole argument.** The shapes an editor normally reaches for — the half-quadrant chevrons `▾ ▸` — are
the ones this face does not have. The small solid triangles are. So option 1 is available here, but only with the
constraint that the mark must be one the face maps; and a face without the mark draws a **tofu box** with nothing to fail,
which is the risk the whole option carries.

## 4. The three options, with their cost

| # | option | licence | coverage cost | what it costs here |
|---|---|---|---|---|
| **1** | **Unicode characters in the shipped face** | none beyond FiraMono's OFL, already vendored | **real**: the face has `▼ ►` and not `▾ ▸`, so the shape is the solid triangle, not the half-quadrant | sizes with the text and takes the role's ink for free; no asset, no atlas pass, no regeneration. Needs a coverage check or a face without the mark draws a box |
| **2** | **An embedded icon font** (a subset of the marks) | a font licence to choose and vendor — OFL and MIT subsets exist, so this is cheap but not free | one file, any size, any colour, no atlas regeneration | **a second face in the binary** and a second place a glyph can be missing, which means the coverage check is still needed and now over two faces. Buys the half-quadrants and a designer's shapes; buys nothing the shipped face does not already give for `▼ ► × › ↑ ↓ →` |
| **3** | **Grow the 16 px atlas** | none (the repo draws it) | none new | **worst fit for a text-flow mark**: raster at one size, cannot take a role's ink, cannot sit in a line of text; every mark is a repaint of the sheet plus a PNG-reading test. Right for a toolbar square, wrong for a chevron |

**Measured cost of option 1 where it is used: 0 bytes.** The two marks are codepoints in a face already compiled in; no
asset is added, and the atlas is unchanged at 976 bytes. **Option 2 starts at the size of a subset font** (a 12-glyph
subset of a typical icon font is a few kilobytes) plus a licence file, plus a second `Assets<Font>` entry, plus the same
coverage check. Option 3 costs a new cell in a 24-cell sheet that has 0 free cells left — it is exactly full at 8 × 3.

## 5. The recommendation

**Option 1: Unicode characters in the shipped face**, with the constraint that the mark must be one the face maps.

**The rule, so this is not re-argued.** A mark that stands **in a text run or beside text** — a disclosure, an arrow, a
close cross, a drag handle — is a character from the shipped face. A mark that stands **in a fixed square on its own** — a
toolbar command, a panel's tab — is a cell of the 16 px atlas. The atlas is not deprecated and does not grow for a
text-flow mark; the fold chevrons are the first marks that are characters and the atlas gains nothing from them.

**Why not option 2 now.** It is the right answer for a mark the shipped face cannot draw — the half-quadrant chevrons, a
designer's glyph set — and it is not needed for any mark this editor draws today. Buying a second face before a single
mark needs it is a licence, a vendored file and a second coverage surface paid for nothing.

**Why not option 3 at all for this.** A 16 px raster cannot be one line of a text run. That is a property of the format,
not of the drawing.

**What makes option 1 safe.** Its whole risk is a face without the glyph, and the risk is closed with a test, not with
care: `tests/every_character_a_mark_draws_is_in_its_face.rs` opens every built workspace, reads **every `Text` the editor
draws**, and resolves each character in the face that text names through the face's own `cmap`. There is no hand-written
list to drift from the code — a mark added tomorrow is measured tomorrow. The atlas's own half of the rule keeps its
existing check (`tests/every_toolbar_command_has_an_icon.rs`).

## 6. What changed under #493

- `editor.rs`: the PLACE palette's category header draws `▼`/`►` instead of `v`/`>`.
- `tiles.rs`: the MESHES pack band and the `EXCLUDED` band draw `▼`/`►`; the Fill page's column and row headers draw
  `↓` (U+2193) and `→` (U+2192), because what those headers do is run the armed verb down a column or across a row, and a
  direction says that where a letter did not.
- The hit target: every one of those marks already stood inside the `Button` that folds (a gooey `band`, a header row), so
  the mark itself was already pressable. `tests/a_fold_chevron_is_the_control_that_folds.rs` now proves it by driving a
  press at the mark's own rect through `bevy_picking`, which is what #493's first acceptance item asked for and what
  nothing measured before. The half of the note that is gooey's code editor is filed as G-82
  (`dark-incorporated/bevy_gooey#107`).
- The coverage check is `tests/every_character_a_mark_draws_is_in_its_face.rs`, #493's second acceptance item.

## 7. When to revisit

- **A mark the face lacks becomes necessary** — the editor wants `▾ ▸`, a designer supplies a shape set, or a face a
  project installs lacks a mark the editor draws. Option 2 is the answer; price the subset and its licence then.
- **The face itself changes.** `tests/every_character_a_mark_draws_is_in_its_face.rs` also asserts the two half-quadrants
  are absent from the shipped face. If a future FiraMono adds them, that test fails and this document is revisited rather
  than left quietly wrong — the constraint in §5 was that the face does not have them.
- **The atlas fills.** It is full at 24 cells; a 25th toolbar command needs a second row or a second sheet, which is the
  moment to ask whether a 16 px raster is still the right half of the rule for whatever that command is.
