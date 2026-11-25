# The editor's text

**Issue #518** (`feedback-1795094070279`). What every piece of text the editor draws is *for*, the size and ink it is
drawn at, and the one inconsistency worth fixing now. Measured in this worktree at `b64c689f`, `bevy_gooey` at the pinned
rev `f1904be`, a 2176 x 1406 surface (1088 x 703 logical px at `ui_scale` 2).

**Since 2026-11-22** the type sizes and the body measures named below (`BODY_CHAR_W`, `BODY_LINE_H`, and the group gap
that was `GAP_GROUP`, now the sheet's `.chrome-gap-group` rule) come from `metrics.type` and the class rules of the
editor's live style sheet, `crates/bevy_dark_factory/src/style/editor.yaml`
([data files](../guide/data-files.md#the-editors-style-sheet)). **The `px` column of the role table in §2 is read from that sheet by
`tests/the_documents_state_the_codes_own_counts.rs`**, so it cannot drift; the `measured` columns, **§1.1's before-census**
and the capture numbers quoted in §1 are this page's snapshot at `b64c689f`, where `HEADING` was drawn at 10 and a
gooey label at 11.67. **Those are history**: §5 is the change that ended them, and the sheet is today's state.

The owner's note, in full:

> *"We ned rules, data models, mental models, and a damn fine font for all of the text information in this system. We could probably use color better too."*

The capture points at the World workspace's inspector, at the `WATER` section: a 10 px dim uppercase heading, an 11.67 px
body hint, and the list below it. **That is the surface measured here, and those are the sizes it was drawn at before
#518 and §5** — the heading is 11 and the hint is drawn through `chrome::body` at 11 today.

---

## 1. What the note is about, in the code

The pick is `element rect_px [120, 740, 917, 895]`, the World workspace's scrolling panel. Read against the code, the
panel is three things stacked, and the note is about all three:

| what a person sees | drawn by | role | size then (before #518) | size now |
|---|---|---|---|---|
| `TERRAIN`, `WATER`, `FOREST`, `LIGHTS` | `chrome::section` (`chrome.rs:827`) | [`text::HEADING`] | 10 px, uppercase, `Ink::label` | **11 px**, the sheet's `metrics.type.heading` |
| `set on the TERRAIN line: water ...` | `chrome::body` (`world_lists.rs:128`) | [`text::BODY`] | 11.67 px, gooey `Role::Label` | **11 px**, the sheet's `metrics.type.body` |
| `no water placed` | `chrome::body` (`world_lists.rs:128`) | [`text::BODY`] | 11.67 px, gooey `Role::Label` | **11 px**, the sheet's `metrics.type.body` |

**The bug the note pointed at was the last column: two systems decided a text's size, and they did not agree.** The
crate's hand-drawn labels go through `chrome::font`, which is `role.px() * text_scale()`. A label the crate spawned as a
`bevy_gooey::label` was styled by gooey's `apply_skin`, which sets `skin.text_size * skin.text_scale` **and then snaps it
to a whole device pixel** (`snap_text_size`): at the harness's `UiScale` of 1.2 the 11 px skin size became
`ceil(11 * 1.2) / 1.2 = 11.666… px`. So an 11 px heading and an 11.67 px label sat in the same column, one of them drawn
6 % larger than the type scale said, and **the census (`chrome_census::the_type_scale_stays_short`) could not see it**
because gooey's size is not a `chrome::text` role.

## 1.1 The before-census, and the change that ended it

**This subsection is the census as it was measured at `b64c689f`, before the change §5 describes — it is history, not
today's state.** Measured on the World workspace then, every text on screen by size:

```
{ "9": 5, "10": 30, "11": 11, "11.666666": 550, "15": 6 }
```

**550 of 602 texts were drawn at a size the type scale did not name.** The scale was `{9, 10, 11, 13, 15}`; the
editor's most common body size was 11.666, which is none of them. That is the "damn fine font" half of the note in one
number: the font is fine, the *sizes* were two systems' guesses, and the more common one was the wrong one.

**§5 is the change that ended it**: a list row and a hint are drawn through `chrome::body`, at `text::BODY`, so the two
sizes the window held are one. `#518` (owner, 2026-11-21) then moved `HEADING` from 10 to 11, and the sheet's
`metrics.type` is `{title: 15, tab: 13, body: 11, heading: 11, label: 10, control: 10, hint: 9}`. Every number in the
table above is read from that sheet by `the_text_roles_table_states_the_sheets_sizes`; the two `drawn by` cells are
resolved against `chrome.rs` and `world_lists.rs` by `the_text_roles_citations_land_on_their_symbols`.

## 2. The roles, measured

`chrome::text` is seven roles over five sizes (`chrome.rs:531`), and every one is `role.px() * text_scale()` when drawn:

| role | px | what it is for | where it is measured from |
|---|---|---|---|
| `TITLE` | **15** | a panel's own name — `INSPECTOR`, `MESHES AND TILES` | `chrome::title` |
| `TAB` | **13** | a tab's word in the strip, a chooser's column header | `ui`'s tab strip |
| `BODY` | **11** | **the readable default**: a row's value, a field's contents, a banner, a census row | `chrome::font` at 5 call sites, or `chrome::typed` (which picks the role) at 120 |
| `HEADING` | **11** | a block heading: uppercase, dim, the sheet's `.chrome-gap-group` above it | `chrome::section` |
| `LABEL` | **10** | the dim half of a `label value` pair, a log line, the bar's readouts (`BAR_READOUT`) | `chrome::row_label` |
| `CONTROL` | **10** | a control's own word: a chip, a button, a field | `chip`, `text_field` |
| `HINT` | **9** | a footnote, a badge's description, the always-on hint — the smallest thing, never the thing being read | `chrome::palette_rule` |

**Two roles are not in `chrome::text`**, because they are not drawn by `chrome::font` at all:

| what | drawn by | measured |
|---|---|---|
| a `bevy_gooey::label` (a Design tab's path and status, the board's empty line) | gooey `apply_skin`, `Role::Label` | **11.67 px**, `skin.text` |
| the code editor's text, gutter and syntax runs | gooey `apply_skin`, `Role::CodeText`/`CodeNumber`/`CodeSyntax` | **11.67 px**, `skin.text` / `text_muted` / `skin.syntax(role)` |

`BODY` (11) and the gooey label (11.67) are **the same role drawn at two sizes**, and the pair was the whole of what was
inconsistent at `b64c689f`. §5 moved every list row and hint this crate draws onto `chrome::body`, so a body line the
*editor* draws is one number; a `bevy_gooey::label` in a surface this crate does not draw (a Design tab's own chrome)
still snaps to gooey's size.

### The metrics, so a layout can be measured without a round trip

- **`BODY_CHAR_W = 6.6`** — one glyph's advance at `text::BODY`, logical px. Exact, because the shipped face is
  monospace: `chars * 6.6` measures a string.
- **`BODY_LINE_H = 11 * 1.2 = 13.2`** — one line of `text::BODY` (Bevy 0.20's default `LineHeight`). Multiplied by
  `text_scale()` for a live box via `body_line_h()`.

## 3. The face, and whether a second one is warranted

**The shipped face is FiraMono Regular** — `crates/bevy_dark_factory/assets/fonts/FiraMono-Regular.ttf`, **174,632
bytes**, SIL OFL 1.1, installed over Bevy's default asset id (`harness::install_font`). It is monospace, and one column
per character is exactly right for keys, ids, paths, the Inspector's columns and the code editor — which is most of this
editor's text.

**It has the glyphs the editor draws.** `tests/every_character_a_mark_draws_is_in_its_face.rs` opens every built
workspace, reads every `Text` on screen, and resolves each character in the face that text names through the face's own
`cmap`. It passes, so every mark this editor draws today is in FiraMono (or in the one mark that stands outside it).
Coverage is not the question.

**A second face already exists, and it is the right kind of second face.** FiraSans Regular, **456,996 bytes**, SIL OFL
1.1, drawn on every `Text` carrying `[Prose]` (`prose.rs`): a sentence meant to be read, not terminal output. That is
already 457 KB of a second face in the binary, spent to make the What's new page read as prose instead of a terminal —
and it is the model for what the note's "damn fine font" means here: **a face where the text is a sentence, FiraMono where the
text is data.**

**A third face is not warranted, and the cost is measurable.** Every face is:
- bytes in the binary (FiraMono 174 KB + FiraSans 457 KB = **632 KB of font today**);
- a `cmap` coverage surface that `every_character_a_mark_draws_is_in_its_face` must now iterate over a second face;
- a licence file beside it, kept with the face;
- a rule for *which* text takes it — and the editor already has one (`[Prose]` vs everything else) that a third face would
  only make ambiguous.

**The recommendation is not a new face. It is to make the two faces already here agree on sizes** — §5. FiraMono and
FiraSans are the same family, licensed the same way, and they already carry the two kinds of text this editor has. What
the note asks for (a fine font for all the text information) is answered by FiraMono for the data and FiraSans for the
prose, once a body label is drawn at the size the type scale names instead of at gooey's snapped 11.67.

## 4. Colour, the note's second sentence

The note says *"we could probably use color better too"*. The colours are one system already: `Ink` (chrome.rs) names
every ink a hand-drawn label takes, and `ui.rs::editor_inks` holds the nine of them to a contrast bar on every ground. The
one place colour was not used as well as it could be is the **markdown editor's prose** — an emphasis and a strong
emphasis drew in the body ink like a paragraph — and that is #525's, done there: `**strong**` takes the `Type` hue and
`*em*` the `Function` hue, both distinct from the `String` of a code span and the `Reference` of a link. So the note's
second sentence is answered beside its first: the sizes on the scale (§5), the emphases in their own colours (#525).

## 5. The one change this document justifies

**A list row or a hint is drawn through `chrome::body`, at `text::BODY`, and not through `bevy_gooey::label`.**

The reason it is this and not a redesign is **§1.1's before-census**: the editor's most common text size *was* 11.666
(550 of 602 texts on the World workspace), **a size the type scale does not name**, and every one of those texts was a
`bevy_gooey::label` — the body of every list, every hint under a heading. `chrome::body(parent, words, ink)` spawns the line with `chrome::typed`, so it is
`text::BODY` (11 px) at 100 % text like every label `chrome::font` already gives, and — unlike a gooey label — it follows
the person's text size through [`resize_the_labels`], instead of being recomputed from `skin.text_scale` on gooey's own
pass. **No role, no face, no setting, and no new dependency:** one helper, and the two sizes the window held become one.
`the_world_sections_text_is_at_the_type_scales_sizes` holds it, and §1's table reads the result from the sheet.

**What was measured after the change**, on the same surface, the same way §1 was: the `WATER` section's hint, its heading
and its rows draw at `11`, `10` and `11` — all three on the scale. That was `b64c689f`; **#518 (owner, 2026-11-21) then moved
`HEADING` from 10 to 11**, so the same three now draw at `11`, `11` and `11`. `tests/the_world_sections_text_is_at_the_type_scales_sizes.rs`
holds the three: the hint is `text::BODY` exactly; every World-section text is a size the scale names; the hint stays in
`Ink::dim`, so this is a size and not a repaint.

**Why not change gooey's snap.** The 11.666 px is `snap_text_size(11, 1.2)` — gooey rounding a text size up to a whole
device pixel, which is the right thing for sharp glyphs and is gooey's to decide. Overriding it from this crate would be a
second text-size authority, which is the arrangement §1.1 is complaining about. A `bevy_gooey::label` stays available and
stays at the skin's size; a body line the *editor* draws goes through `chrome::body`, and the two are one number again.

## 6. Where the current set is inconsistent

1. **Two systems decide a size** (this document's §1): `chrome::text` roles and gooey's snapped `skin.text_size`. The
   editor's most common label size *was* 11.666 — a size the type scale does not name, as §1.1's before-census measured —
   and §5 is the change that moved every editor-drawn body line onto `text::BODY`. A `bevy_gooey::label` still snaps, so
   the second authority is still there for anything this crate does not draw itself; the World workspace draws none.
2. **`section` was 10, smaller than the body it heads (11.67)** — a heading quieter than the list under it was the one
   case where size and importance disagreed, and the audit's decision was to keep headings quiet, which left the *shape*
   (uppercase, dim) carrying the hierarchy and size not carrying it. **#518 (owner, 2026-11-21) reversed that: `HEADING` is
   11 now**, a pixel above `LABEL`'s 10 and level with the body it heads, so the hierarchy is carried by size as well as by
   case and ink. `chrome.rs`'s doc on `text::HEADING` records why, and the role table in §2 reads the sheet's 11.
   Recorded, no longer a defect.
3. **`text::CONTROL` and `text::LABEL` are both 10** and are different roles — `the_type_scale_stays_short` allows this on
   purpose (`a seventh role is a decision somebody should have to make on purpose`), and it is documented, so it is a
   decision and not a drift.

## 7. What this is, in one line

The editor draws seven type roles over five sizes and two faces. The face is fine (FiraMono for data, FiraSans for prose,
both SIL OFL 1.1, both covered by the `cmap` test). The **sizes** were the one thing inconsistent — the most common text
on screen *was* drawn at a size the type scale did not name — and `chrome::body` is what put it back on the scale: the
World workspace's every text is now a size `metrics.type` names, which `the_world_sections_text_is_at_the_type_scales_sizes`
holds.
