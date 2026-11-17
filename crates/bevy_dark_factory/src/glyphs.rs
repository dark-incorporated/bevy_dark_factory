//! **The editor's text-sized marks** (#493): the small shapes a widget draws *inside* its own words — a fold's caret, an
//! armed row's dot, a direction — as characters from the monospace face the editor already ships, never as pixels from
//! [`crate::icons`]' atlas.
//!
//! # Why two vocabularies, and which is which
//!
//! The owner's note, in full: *"The "v" should collapse the text. And we should consider an icon solution for icons like up
//! and down arrows, etc. Like font awesome, etc."* Two things are wrong in that first sentence and they are different:
//!
//! 1. a fold caret was drawn as the **letter** `v`, which reads as prose in a gutter rather than as a control;
//! 2. the caret was `Pickable::IGNORE` in `bevy_gooey`, so the shape that looked like the control was not the hit target.
//!
//! The second is `bevy_gooey`'s and is filed there; the first is this file's, and it is the same question as "an icon
//! solution": **what draws a 10 px mark that must sit on a line of 11 px text, take that text's colour and grow with the
//! person's text size?** A 16 px raster cell cannot: it is one size, one colour, and it must be repainted to add a glyph.
//!
//! # The three ways, and what each costs
//!
//! | Way | Licence | Coverage | Size | Colour |
//! |---|---|---|---|---|
//! | **Unicode text glyphs** in the shipped face (this file) | none beyond the face's own OFL, already carried | whatever FiraMono's `cmap` has — 1350 code points, of which the arrows, the geometric shapes and `×` are the useful ones | any, it is text | the text's own ink |
//! | An embedded icon **font** (a subset, e.g. Font Awesome Free) | SIL OFL 1.1 for the font, **CC BY 4.0 for the icons**, which must be credited in the build | every glyph the subset carries, and only those | any | any |
//! | Grow the **atlas** ([`crate::icons`]) | the sheet is ours | whatever `tools/ui/make_icons.py` draws, and every new mark is a repaint plus a test that reads the PNG back | **16 px, fixed** | one white, tinted by the skin |
//!
//! **The choice is the first**, recorded in `docs/ui/icons.md` with the measurements: the atlas stays for what it is good at
//! (a toolbar's identical 16 px button faces), and anything that must **sit in a line of text** is a character. A face's own
//! `cmap` is the check that a glyph exists — `tests/every_mark_is_in_the_face_that_draws_it.rs` fails rather than drawing
//! tofu, which is the failure the review named as P1.
//!
//! # What a mark may be
//!
//! Nothing here is a word. [`FOLD_OPEN`], [`FOLD_CLOSED`] and [`ARMED`] are shapes a person learned from every
//! disclosure control they have used; a mark whose meaning needs a legend is a caption, and the caption goes in
//! `chrome::palette_rule`'s one line instead.

/// **A fold's caret while its text is showing**: `U+25BC BLACK DOWN-POINTING TRIANGLE`. A caret, not the letter `v`, which
/// is what the owner saw and read as prose in the gutter.
pub const FOLD_OPEN: &str = "▼";

/// **A fold's caret while its text is hidden**: `U+25BA BLACK RIGHT-POINTING POINTER`. The pair differs by direction, which
/// survives being glanced at and survives a colour-blind reader; the letters `v`/`>` differ by both being wrong.
pub const FOLD_CLOSED: &str = "►";

/// **The row a click would use**: `U+25CF BLACK CIRCLE`, filled. A reserved column of these says which row is armed in a
/// shape, so the state does not rest on the accent rail's colour alone (WCAG 2.2, 1.4.1).
pub const ARMED: &str = "●";

/// **What every mark this module names is drawn in**: the code point, and the one place a reader compares the list against a
/// face's own character map. A mark added here and nowhere else is still checked, because the test reads this array.
pub const MARKS: [char; 3] = ['\u{25BC}', '\u{25BA}', '\u{25CF}'];
