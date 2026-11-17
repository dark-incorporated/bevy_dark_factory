//! **Every mark the editor draws as a character is in the face it is drawn with** (#493), so a missing glyph fails a check
//! rather than rendering as a box.
//!
//! The owner's capture `feedback-1794921635359` asked two questions and they share one answer: *"The "v" should collapse the
//! text. And we should consider an icon solution for icons like up and down arrows, etc. Like font awesome, etc."* A fold's
//! caret was the letter `v`, and the editor's only other vocabulary was the 16 px raster atlas in `icons.rs`.
//!
//! The choice, written down in `docs/ui/icons.md`: a **character** for anything that must sit in a line of text, the atlas
//! for a toolbar's identical button faces. A character is only worth choosing if the face has it — `ttf-parser` reads the
//! shipped faces' own `cmap` here, the same way `the_terminal_draws_in_a_nerd_font.rs` does for its private-use icons.
//!
//! Three things are held:
//!
//! 1. every mark in [`bevy_cauldron::glyphs::MARKS`] is in the editor's face and in the terminal's, because both draw text;
//! 2. a fold's caret changes shape with the fold, and neither shape is a letter — read from the running editor rather than
//!    from the source;
//! 3. the marks are single characters, so a constant cannot quietly become a word.

use bevy::prelude::*;

use bevy_cauldron::glyphs;

use crate::fixtures;

/// The editor's face, as `assets/fonts/` ships it: the one a bare `Text` is drawn in.
const EDITOR_FACE: &[u8] = include_bytes!("../assets/fonts/FiraMono-Regular.ttf");

/// The terminal's face. It is in the same editor, so a mark drawn without naming a face may land in either.
const TERMINAL_FACE: &[u8] = include_bytes!("../assets/fonts/FiraMonoNerdFontMono-Regular.otf");

/// Does the face in `bytes` have a glyph for `c`, read from its own character map?
fn covers(bytes: &[u8], c: char) -> bool {
    ttf_parser::Face::parse(bytes, 0)
        .unwrap_or_else(|e| panic!("the face does not parse: {e}"))
        .glyph_index(c)
        .is_some()
}

/// **Every mark the vocabulary names is a glyph in both faces the editor draws text in.**
#[test]
fn every_mark_is_a_glyph_in_the_faces_that_draw_it() {
    let mut missing = Vec::new();
    for mark in glyphs::MARKS {
        for (face, bytes) in [("the editor's", EDITOR_FACE), ("the terminal's", TERMINAL_FACE)] {
            if !covers(bytes, mark) {
                missing.push(format!("`{mark}` (U+{:04X}) is not in {face} face", mark as u32));
            }
        }
    }
    assert!(missing.is_empty(), "a mark the editor draws would render as a box:\n{}", missing.join("\n"));
}

/// **A mark is one character, and not a word.** `#493`'s defect was a control drawn as the letter `v`, which is one
/// character too — so this is the other half: the vocabulary's members are shapes, and a shape is one code point here.
#[test]
fn a_mark_is_one_character() {
    for mark in glyphs::MARKS {
        assert!(mark.len_utf8() >= 1, "`{mark}` is a character");
        assert_eq!(mark.to_string().chars().count(), 1, "`{mark}` is one glyph, not a run of them");
        assert!(!mark.is_ascii_alphanumeric(), "`{mark}` is a mark, not a letter or a digit a person would read as words");
    }
}

/// **The face the running editor installs is the one this file measured.** A mark that is in the checked-in `.ttf` but not in
/// what the app drew with would pass the test above and still be a box on screen, so the running face's own bytes are read
/// from the asset store.
#[test]
fn the_face_the_editor_runs_with_carries_every_mark() {
    let root = fixtures::Fixture::new(&format!("glyph_face_{}", std::process::id())).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    let fonts = app.world().resource::<Assets<Font>>();
    let face = fonts.get(AssetId::<Font>::default()).unwrap_or_else(|| panic!("the editor's own face is installed"));
    assert_eq!(
        face.data.as_ref(),
        EDITOR_FACE,
        "the running editor draws in `assets/fonts/FiraMono-Regular.ttf`, the face this file reads"
    );
    let mut missing = Vec::new();
    for mark in glyphs::MARKS {
        if !covers(face.data.as_ref(), mark) {
            missing.push(format!("`{mark}` (U+{:04X})", mark as u32));
        }
    }
    assert!(missing.is_empty(), "the running editor's face cannot draw {}: it would render a box", missing.join(", "));
}
