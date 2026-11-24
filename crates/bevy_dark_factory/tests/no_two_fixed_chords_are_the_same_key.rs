//! **No two fixed chords are the same key** (#560).
//!
//! `chords::CHORDS` is the editor's fixed-chord table, and two rows once named `Cmd+Shift+D`: `profile.open`
//! and `report.project`. Both handlers run in `keys::Phase::Act` on the same frame, so one press opened both
//! panels — the frame profile *and* the project report — and `docs/guide/keys.md` gave the chord to only one
//! of them. A table cannot answer "who owns this key" if two rows say they do.

use std::collections::BTreeMap;

/// **Every pair of chords that resolve to the same key and modifiers**, by the spelling a person reads, so a
/// failure names both ids rather than two `Shortcut` structs.
#[test]
fn no_two_fixed_chords_are_the_same_key() {
    let mut by_key: BTreeMap<String, Vec<&'static str>> = BTreeMap::new();
    for chord in bevy_dark_factory::chords::CHORDS {
        let shortcut = (chord.default)();
        // The host's spelling is enough to compare: two chords that spell the same are the same key, and a
        // rebinding a person makes is a setting, not this table.
        let key = format!("{:?} {:?}", shortcut.modifiers, shortcut.key);
        by_key.entry(key).or_default().push(chord.id);
    }

    let collisions: Vec<String> = by_key
        .iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(key, ids)| format!("{key} -> {ids:?}"))
        .collect();
    assert!(
        collisions.is_empty(),
        "**two fixed chords are the same key**: {}. One press would run both handlers in `keys::Phase::Act`, \
         so the editor would do both things at once (#560).",
        collisions.join("; ")
    );
}
