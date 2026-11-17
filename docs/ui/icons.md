# The icon vocabulary: characters in the face, and the atlas for what a character cannot do

Decided before the atlas grows again, from the owner's note in capture `feedback-1794921635359` (dark_factory 0.7.0,
mac_air, 2026-11-17 07:20):

> *"The "v" should collapse the text. And we should consider an icon solution for icons like up and down arrows, etc.
> Like font awesome, etc."*

## The two defects behind the note

1. **A fold's caret was the letter `v`.** `bevy_gooey`'s code editor draws `"v"` while open and `">"` while closed
   (`code_overlay.rs:1099`, read in the pinned checkout), so a control read as prose in a gutter. The editor drew the same pair in its
   own two fold controls — the PLACE panel's category bands (`editor.rs`) and the MESHES panel's pack bands (`tiles.rs`). Both of
   the editor's now draw `▼`/`►`.
2. **The shape was not the target — in the code editor.** There the chevron is `Pickable::IGNORE` and the whole line folds. That
   widget is `bevy_gooey`'s, not this repository's file, so it is named for the crate's own repository and **not** fixed here. The
   editor's own two fold controls have their caret inside a band that is already a `Button`, so a press on the glyph bubbles to it,
   and `tests/a_press_on_a_fold_caret_folds_its_band.rs` presses at the caret's own rect to prove it.

The second is why "add an icon font" is not the whole answer: a vocabulary does not make an affordance clickable. The
first is the question this page answers.

## What is being chosen between

The editor already draws two kinds of icon and the choice is which one a new mark joins.

||**Unicode characters** in the shipped face|**An embedded icon font** (a subset, e.g. Font Awesome Free)|**Grow the atlas** (`crates/bevy_dark_factory/src/icons.rs`)|
|---|---|---|---|
|Licence|none beyond the face's own SIL OFL 1.1, already carried (`assets/fonts/FiraMono-OFL.txt`)|SIL OFL 1.1 for the font, **CC BY 4.0 for the icons**: attribution must ship in the build and in the About/credits surface|ours; no obligation|
|Coverage|whatever the face's `cmap` maps — **measured 1350 code points** in `FiraMono-Regular.ttf`|exactly the subset built, and only that|whatever `tools/ui/make_icons.py` draws; every new mark is a repaint of the sheet **and** a name in `ICONS`|
|Size|any: it is text, so it scales with `text_scale` and the type role|any, resolved like text|**16 px, fixed** (`CELL`); a second size is a second art pass|
|Colour|the run's own ink, per glyph, from the skin|any|one white, tinted by `IconTone`|
|Sits in a line of text|yes — that is what it is|yes|no: a 16 px raster beside 11 px text has no baseline and no reflow|
|Cost of a new mark|one constant|one subset rebuild|repaint + test that reads the PNG back + a name|

**Measured coverage of `FiraMono-Regular.ttf`, read from its own `cmap` (not from a list someone typed):**

|Mark|Code point|In the face?|
|---|---|---|
|`▼` `▲` `►` `◄`|U+25BC, U+25B2, U+25BA, U+25C4|yes|
|`●` `○` `◉` `◆` `◇`|U+25CF, U+25CB, U+25C9, U+25C6, U+25C7|yes (55 code points in U+25A0–U+25FF)|
|`←` `↑` `→` `↓` `⇦` `⇧` `⇨` `⇩`|U+2190–U+21E9|yes|
|`×` `•` `·` `…` `›` `»`|U+00D7, U+2022, U+00B7, U+2026, U+203A, U+00BB|yes|
|`▾` `▸` `▶` `◀` `▷` `◁`|U+25BE, U+25B8, U+25B6, U+25C0, U+25B7, U+25C1|**no** — the small triangles are absent, and so is the medium `▶`; the medium `▼`/`►` are there|
|`✓` `✗` `✕` `☰`|U+2713, U+2717, U+2715, U+2630|**no**|

The absence of `▾`/`▸` and of `▶`/`◀` is the fact that decides the fold's glyphs: the triangles a chevron is usually
drawn with are not in this face (only `◄`/`►` of the medium set are, and `▲`/`▼`), so the caret is the **medium** pair
`▼`/`►`. A vocabulary that guessed would have shipped tofu on the one control the owner pointed at.

## The decision

**Characters in the shipped face for every mark that must sit in a line of text; the atlas stays for what it is good at,
a toolbar's identical 16 px button faces.**

It costs nothing to license, carries no attribution obligation, scales with the person's text size and takes the ink of
the line it is in. Its one risk — a face without the glyph draws a box — is answered by a test rather than by care:
`tests/every_mark_is_in_the_face_that_draws_it.rs` parses the shipped `.ttf` **and** the running `Assets<Font>` face with
`ttf-parser` and fails on any mark in `crate::glyphs::MARKS` that either lacks. `src/ui.rs` already holds the same
relation for the chord glyphs a Mac spells.

A mark goes in `crate::glyphs`, never inline in a widget: one list to check, one place to add to, and
`glyphs::MARKS` is what the test reads.

## When this decision is revisited

- A mark the editor needs that the face lacks and that cannot be spelled another way (the `▾`/`▸` shape is the near
  miss): the answer is still not an icon font with an attribution obligation, it is `▾`-as-`▼` at a smaller role, or a
  glyph added to the atlas and drawn as an image node.
- If the atlas is ever needed above 16 px, or in a second colour per glyph, that is the moment to price an icon font
  properly — with its CC BY 4.0 credit in `docs/ui.md` and in the About surface — and not before.

## What the owner sees now

Both of the editor's own fold controls — the PLACE panel's category bands and the MESHES panel's pack bands — draw
`▼`/`►` from `crate::glyphs`, and the armed row in the PLACE list wears `●` in a reserved column of its own so the state
is a shape and not the accent rail's colour alone (WCAG 2.2, 1.4.1).
