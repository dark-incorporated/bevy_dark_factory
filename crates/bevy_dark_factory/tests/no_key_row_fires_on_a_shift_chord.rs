//! **A modified key row never fires on a fixed Shift chord with the same key** (#678).
//!
//! A `keys::BINDINGS` row built with `b()` is indifferent to Shift, so `Cmd+F` (filter the list) also fired on
//! `Cmd+Shift+F` (the frame profile, a `chords::CHORDS` default): the profile chord put a filter box in charge of
//! the keyboard, and every workspace switch after it was refused. A row that shares a key with a Shift chord must
//! ask for Shift up.

use bevy_dark_factory::{chords, keys};

#[test]
fn no_modified_row_answers_a_shift_chord_on_its_key() {
    let mut clashes = Vec::new();
    for chord in chords::CHORDS {
        let shortcut = (chord.default)();
        let m = shortcut.modifiers;
        if !(m.shift && !m.alt && (m.ctrl || m.meta)) {
            continue;
        }
        for row in keys::BINDINGS {
            let fires_with_shift = row.needs_shift != Some(false);
            if row.key == shortcut.key && row.needs_mod && fires_with_shift && !row.needs_alt {
                clashes.push(format!("{:?} on {:?} also fires on `{}`", row.action, row.key, chord.id));
            }
        }
    }
    assert!(
        clashes.is_empty(),
        "**a Shift-indifferent modified row shares its key with a Shift chord** (#678), so one press fires both: \
         {clashes:#?}. Build the row with `bs(.., false, ..)` so it needs Shift up"
    );
}
