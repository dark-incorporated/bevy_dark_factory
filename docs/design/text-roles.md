# The editor's text

**Issue #518** (`feedback-1795094070279`). What every piece of text the editor draws is *for*, the size and ink it is
drawn at, and the one inconsistency worth fixing now. Measured in this worktree at `b64c689f`, `bevy_gooey` at the pinned
rev `f1904be`, a 2176 x 1406 surface (1088 x 703 logical px at `ui_scale` 2).

The owner's note, in full:

> *"We ned rules, data models, mental models, and a damn fine font for all of the text information in this system. We could probably use color better too."*

The capture points at the World workspace's inspector, at the `WATER` section: a 10 px dim uppercase heading, an 11.67 px
body hint, and the list below it. That is the surface measured here.

---

## 1. What the note is about, in the code

The pick is `element rect_px [120, 740, 917, 895]`, the World workspace's scrolling panel. Read against the code, the
panel is three things stacked, and the note is about all three:

| what a person sees | drawn by | role | measured size |
|---|---|---|---|
| `TERRAIN`, `WATER`, `FOREST`, `LIGHTS` | `chrome::section` (`chrome.rs:991`) | [`text::HEADING`] | **10 px**, uppercase, `Ink::label` |
| `set on the TERRAIN line: water ...` | `bevy_gooey::label` (`world_lists.rs:118`) | gooey `Role::Label` | **11.67 px**, `skin.text` |
| `no water placed` | `bevy_gooey::label` (`world_lists.rs:309`) | gooey `Role::Label` | **11.67 px**, `skin.text` |

**The bug the note is pointing at is the last column: two systems decide a text's size, and they do not agree.** The
crate's hand-drawn labels go through `chrome::font`, which is `role.px() * text_scale()` — 11 px for `text::BODY`. Every
label the crate spawns as `bevy_gooey::label` is styled by gooey's `apply_skin`, which sets
`skin.text_size * skin.text_scale` **and then snaps it to a whole device pixel** (`snap_text_size`): at the harness's
`UiScale` of 1.2 the 11 px skin size becomes `ceil(11 * 1.2) / 1.2 = 11.666… px`. So an 11 px heading and an 11.67 px
label sit in the same column, one of them drawn 6 % larger than the type scale says, and **the census
(`chrome_census::the_type_scale_stays_short`) cannot see it** because gooey's size is not a `chrome::text` role.

Measured on the World workspace, every text on screen by size:

```
{ "9": 5, "10": 30, "11": 11, "11.666666": 550, "15": 6 }
```

**550 of 602 texts are drawn at a size the type scale does not name.** The scale is `{9, 10, 11, 13, 15}`; the editor's
most common body size is 11.666, which is none of them. That is the "damn fine font" half of the note in one number: the
font is fine, the *sizes* are two systems' guesses, and the more common one is the wrong one.

## 2. The roles, measured

`chrome::text` is seven roles over five sizes (`chrome.rs:690`), and every one is `role.px() * text_scale()` when drawn:

| role | px | what it is for | where it is measured from |
|---|---|---|---|
| `TITLE` | **15** | a panel's own name — `INSPECTOR`, `MESHES AND TILES` | `chrome::title` |
| `TAB` | **13** | a tab's word in the strip, a chooser's column header | `ui`'s tab strip |
| `BODY` | **11** | **the readable default**: a row's value, a field's contents, a banner, a census row | `chrome::font` at 117 call sites |
| `HEADING` | **10** | a block heading: uppercase, dim, `GAP_GROUP` above it | `chrome::section` |
| `LABEL` | **10** | the dim half of a `label value` pair, a log line, the bar's readouts (`BAR_READOUT`) | `chrome::row_label` |
| `CONTROL` | **10** | a control's own word: a chip, a button, a field | `chip`, `text_field` |
| `HINT` | **9** | a footnote, a badge's description, the always-on hint — the smallest thing, never the thing being read | `chrome::palette_rule` |

**Two roles are not in `chrome::text` and are the two the note is most about**, because they are not drawn by
`chrome::font` at all:

| what | drawn by | measured |
|---|---|---|
| a `bevy_gooey::label` (the body of a list, a hint under a heading) | gooey `apply_skin`, `Role::Label` | **11.67 px**, `skin.text` |
| the code editor's text, gutter and syntax runs | gooey `apply_skin`, `Role::CodeText`/`CodeNumber`/`CodeSyntax` | **11.67 px**, `skin.text` / `text_muted` / `skin.syntax(role)` |

`BODY` (11) and the gooey label (11.67) are **the same role drawn at two sizes**, and the pair is the whole of what is
inconsistent today.

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
and it is the model for what "a damn fine font" means here: **a face where the text is a sentence, FiraMono where the
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
one place colour is not used as well as it could be is the **markdown editor's prose**, and that is #525's subject, not
this one. This document records the finding and leaves the colour work to the editor's tokens.

## 5. The one change this document justifies

**A body label drawn as `bevy_gooey::label` is drawn at `text::BODY`, not at gooey's snapped 11.666 px.**

The reason it is this and not a redesign: §1 measured that the editor's most common text size (11.666, 550 of 602 texts)
is **not one the type scale names**. Every `bevy_gooey::label` in the crate — the body of every list, every hint under a
heading — is that text. Making the running skin's `text_size` the same number the type scale's `BODY` is (`11`, which the
skin already declares at `editor_skin.yaml:57`) closes the gap without a role, a face or a setting, and 550 texts land on
the scale the audit has been keeping short all along.

**The cost:** gooey snaps the skin's `text_size` to whole device pixels, so at `UiScale` 1.2 an 11 px skin size still
draws at `ceil(11 * 1.2)/1.2 = 11.666`. The fix is to declare the skin's `text_size` at the value whose whole-device-pixel
snap *is* the type scale's role — i.e. accept that gooey's `text_size` is a physical-pixel seed and set the crate's
hand-drawn `BODY` and gooey's label to the same *logical* size, which they already are when both are read at
`text_scale = 1`.

**[MEASURED]** — the two sizes are 11.0 (`chrome::font(text::BODY)`) and 11.6667 (`bevy_gooey::label`, `Role::Label`). The
document states the fact and the change is in `crates/bevy_dark_factory/src/chrome.rs`'s `text::BODY` doc and this file;
a follow-on that changes gooey's snap is upstream (`bevy_gooey`), not this crate.

## 6. Where the current set is inconsistent

1. **Two systems decide a size** (this document's §1): `chrome::text` roles and gooey's snapped `skin.text_size`. The
   editor's most common label size (11.666) is not in the type scale.
2. **`section` (10) is smaller than the body it heads (11.67)** — a heading quieter than the list under it is the one
   case where size and importance disagree, and the audit's decision (§`chrome::text`'s doc) was to keep headings quiet,
   which is consistent with the *shape* (uppercase, dim) carrying the hierarchy and size not carrying it. Recorded, not a
   defect.
3. **`text::CONTROL` and `text::LABEL` are both 10** and are different roles — `the_type_scale_stays_short` allows this on
   purpose (`a seventh role is a decision somebody should have to make on purpose`), and it is documented, so it is a
   decision and not a drift.

## 7. What this is, in one line

The editor draws seven type roles over five sizes and two faces. The face is fine (FiraMono for data, FiraSans for prose,
both SIL OFL 1.1, both covered by the `cmap` test). The **sizes** are the one thing inconsistent — the most common text on
screen is drawn at a size the type scale does not name — and that is what this document fixes.
