//! **The marks the editor draws as characters** (#493, `docs/design/icon-vocabulary.md`).
//!
//! A mark that stands **in a text run or beside text** — a fold's disclosure, a direction, a fill arrow — is a character
//! from the shipped face, and it is named here so that "which character does the fold draw" has one answer the code, the
//! document and the tests all read. A mark that stands **in a fixed square on its own** — a toolbar command, a panel's
//! tab — is a cell of the 16 px atlas in [`crate::icons`], and is not named here.
//!
//! # Why these characters, and what makes them safe
//!
//! The shipped face, `assets/fonts/FiraMono-Regular.ttf`, maps U+25BC `▼`, U+25BA `►`, U+25B2 `▲`, U+25C4 `◄`,
//! U+2190–U+2193 `←↑→↓` and `×` U+00D7. It does **not** map the half-quadrant chevrons `▾` (U+25BE) and `▸` (U+25B8)
//! that most editors use, so the fold's disclosure is the small solid triangle rather than the half-quadrant: a character
//! the face lacks draws a tofu box, and the asset-free route's whole risk is exactly that. The measurement is in
//! `docs/design/icon-vocabulary.md` §3, and `tests/every_character_a_mark_draws_is_in_its_face.rs` holds this face to it.
//!
//! # Why a mark carries no `Pickable`
//!
//! A mark stands *inside* the button that owns it (a palette header, a pack band). It is spawned with no `Pickable` at
//! all, deliberately: `bevy_ui`'s picking backend treats a node without one as the hit that blocks what is under it, so a
//! press on the mark targets **the mark** and `PointerClick` then propagates to its parent — the `Button` whose
//! `Activate` folds the group. One press, one fold, and the mark's own rect is the target, which is what #493's first
//! acceptance item asks for.
//!
//! The alternative, `Pickable::IGNORE`, also folds — measured by mutation, because an ignoring node does not block and
//! the button under it is hit instead — but then the mark is not the entity a press lands on, and this editor's shell is
//! explicit that a pickable child inside a non-hoverable row turns `Hovered` on up the whole tree (`chrome::Frame`'s
//! header). A mark that only *looks* pressable is the fault #493 names.

use bevy::prelude::*;

/// **A fold's disclosure when it is closed**: U+25BA `►`.
///
/// The character `docs/design/icon-vocabulary.md` §5 names, and one the shipped face maps.
pub const FOLD_CLOSED: char = '\u{25ba}';

/// **A fold's disclosure when it is open**: U+25BC `▼`.
pub const FOLD_OPEN: char = '\u{25bc}';

/// **The mark a fold draws**: [`FOLD_CLOSED`] or [`FOLD_OPEN`].
///
/// **Derived from those two constants and nowhere else**, so the character a fold draws has exactly one definition: a
/// test that finds a mark the face lacks fails on the constant that names it.
pub fn fold_glyph(folded: bool) -> char {
    if folded { FOLD_CLOSED } else { FOLD_OPEN }
}

/// **A fold's disclosure as a `Text`**, carrying the module's no-`Pickable` rule: see the module note for why the press
/// has to propagate to the button around it rather than being blocked at the mark.
pub fn fold_mark(folded: bool) -> Text {
    Text::new(fold_glyph(folded).to_string())
}

/// **A column's fill mark**: U+2193 `↓`, because the armed verb runs down the column.
///
/// The Fill page's lattice header, which used to draw the ASCII letter `v` (#493): a letter reads as a character in a
/// listing rather than as a control, and `↓` says what pressing it does.
pub const FILL_COLUMN: char = '\u{2193}';

/// **A row's fill mark**: U+2192 `→`, because the armed verb runs across the row.
pub const FILL_ROW: char = '\u{2192}';

/// **The mark a list row wears when it is the one a click would act on**: U+25CF `●` (#486).
///
/// A shape and not a hue, because the row already says "this one" with an accent rail and a selected fill — an encoding a
/// person who cannot separate the two colours does not read, and a photograph of the screen shows only as a slightly
/// lighter box. The palette's armed piece is the case: a row is armed by a click on it and spent by the next click on the
/// map, and nothing said so in a shape until this mark.
pub const ARMED_MARK: char = '\u{25cf}';

/// **The words a row's mark column holds**: [`ARMED_MARK`] when the row is the armed one, and nothing when it is not.
///
/// The blank is the same alphabet as the mark: a row that is not armed says so by drawing nothing, in a column that is
/// laid out either way (see `editor::PaletteArmedMark`) so no name moves when the armed row changes.
pub fn armed_mark(armed: bool) -> String {
    if armed { ARMED_MARK.to_string() } else { String::new() }
}
