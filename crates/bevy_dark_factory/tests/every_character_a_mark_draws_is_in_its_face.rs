//! **Every mark the editor draws as a character is a glyph the face it is drawn with actually has** (#493).
//!
//! # The rule this holds
//!
//! `docs/design/icon-vocabulary.md`: a mark that stands **in a text run or beside text** — a fold's disclosure, an arrow,
//! a close cross — is a character from the shipped face; a mark that stands **in a fixed square on its own** — a toolbar
//! command, a panel's tab — is a cell of the 16 px atlas. The atlas already has this check
//! (`every_toolbar_command_has_an_icon` reads the PNG back and fails on a name with no art). This is the other half, and
//! #493's second acceptance item: before it, a character the face lacked drew a tofu box with nothing to fail.
//!
//! # What it enumerates, and why not a list
//!
//! **The editor's own live trees**, not a hand-written list. Every door is opened, and then *every* `Text` on screen is
//! read: each character of each run is resolved in the face that text names, through the face's own `cmap`. A mark added
//! tomorrow is measured tomorrow — there is nothing here to fall out of step with the code.
//!
//! Two kinds of character are exempt, and only two:
//!
//! - **control characters** (a newline in a wrapped sentence), which have no glyph by definition;
//! - **the Private Use Area**, which is the terminal's own face's business (`the_terminal_draws_in_a_nerd_font`) and never
//!   the editor's — no editor `Text` draws one, and if one appears this fails rather than skipping it.
//!
//! The faces are read from the assets the build holds, so a face a project supplies is the face measured.

use std::collections::BTreeMap;

use bevy::prelude::*;
use bevy::text::FontSource;
use bevy_dark_factory::commands;
use bevy_dark_factory::harness;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// Frames for a door to build and settle.
const SETTLE_FRAMES: usize = 12;

/// **The two chevrons the fold draws** — closed then open, the pair `docs/design/icon-vocabulary.md` names.
const CLOSED: char = '\u{25ba}';
const OPEN: char = '\u{25bc}';

/// **The Private Use Area**, where a Nerd Font's icons live: the terminal's face, never the editor's.
const PRIVATE_USE: std::ops::RangeInclusive<char> = '\u{e000}'..='\u{f8ff}';

/// **One door's worth of the editor, settled.**
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("icon_vocabulary_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .pack("props", &["crate", "barrel"])
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, 2560, 1600).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    app
}

/// **Open one workspace by its own command**, as the chord and the menu do, so its panels are built and drawn.
fn switch_to(app: &mut App, workspace: Workspace) {
    let id = format!("workspace.{}", workspace.id());
    let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "`{id}` did not open the {} workspace", workspace.title());
}

/// **Every character of every text drawn, with the face it is drawn in** — `(char, face)` pairs.
///
/// A text with no `TextFont` names no face, which is reported rather than skipped: it is drawn in the interface's face by
/// Bevy's default and whether that face has the character is exactly the question.
fn drawn(app: &mut App) -> Vec<(char, Option<FontSource>)> {
    let world = app.world_mut();
    let mut query = world.query::<(&Text, Option<&TextFont>)>();
    let mut out = Vec::new();
    for (text, font) in query.iter(world) {
        let face = font.map(|f| f.font.clone());
        out.extend(text.0.chars().map(|c| (c, face.clone())));
    }
    out
}

/// **Does the face `source` names have a glyph for `c`**, read from the asset's own `cmap`?
///
/// `None` when the face cannot be read — a family name Bevy resolves through its own database, or bytes that do not parse.
/// The caller reports that rather than passing it.
fn covers(app: &mut App, c: char, source: &FontSource) -> Option<bool> {
    let FontSource::Handle(handle) = source else { return None };
    let fonts = app.world().resource::<Assets<Font>>();
    let face = fonts.get(handle)?;
    let parsed = ttf_parser::Face::parse(face.data.as_ref(), 0).ok()?;
    Some(parsed.glyph_index(c).is_some())
}

#[test]
fn every_character_the_editor_draws_is_in_the_face_it_is_drawn_with() {
    let mut app = editor("faces");
    // **Every door**, because a mark lives on the panels of the door that shows it: the PLACE palette's fold is on the Map,
    // the pack bands are on the Kit, the fill headers on the Kit's Tiles page, and a close cross could be anywhere.
    let mut seen = 0usize;
    let mut missing: BTreeMap<(u32, String), String> = BTreeMap::new();
    for workspace in Workspace::ALL {
        if !workspace.is_built() {
            continue;
        }
        switch_to(&mut app, workspace);
        let chars = drawn(&mut app);
        seen += chars.len();
        assert!(!chars.is_empty(), "the {} workspace drew no text at all, so this door measured nothing", workspace.title());
        for (c, face) in chars {
            if c.is_control() || PRIVATE_USE.contains(&c) {
                continue;
            }
            let Some(face) = face else {
                missing.insert((c as u32, format!("{c:?}")), format!("{c:?} (U+{:04X}) names no face", c as u32));
                continue;
            };
            match covers(&mut app, c, &face) {
                Some(true) => {}
                Some(false) => {
                    missing.insert((c as u32, format!("{c:?}")), format!("{c:?} (U+{:04X}) in {face:?}", c as u32));
                }
                None => {
                    missing.insert(
                        (c as u32, format!("{c:?}")),
                        format!("{c:?} (U+{:04X}) is drawn in {face:?}, whose bytes cannot be read here, so nothing can say", c as u32),
                    );
                }
            }
        }
    }
    assert!(seen > 500, "every door's text was read ({seen} characters): a check over nothing would pass for the wrong reason");
    assert!(
        missing.is_empty(),
        "{} character(s) the editor draws are not in the face they are drawn with, so each renders as a box:\n{}\n\n\
         docs/design/icon-vocabulary.md: a mark in a text run is a character from the shipped face, and the character must \
         be one the face's `cmap` has. U+25BC `\u{25bc}` and U+25BA `\u{25ba}` are; U+25BE `\u{25be}` and U+25B8 `\u{25b8}` \
         are not.",
        missing.len(),
        missing.values().cloned().collect::<Vec<_>>().join("\n")
    );
}

/// **The two chevrons the fold uses are in the shipped face, and the two an editor usually uses are not** — the measurement
/// `docs/design/icon-vocabulary.md` records, held here so the document cannot drift from the asset.
///
/// It also says the mark the editor drew before is *in* the face, which is why the old one drew fine and was still wrong:
/// coverage is necessary and not sufficient, and #493's first item is the shape.
#[test]
fn the_face_has_the_two_chevrons_the_fold_uses_and_lacks_the_half_quadrants() {
    let face = ttf_parser::Face::parse(include_bytes!("../assets/fonts/FiraMono-Regular.ttf"), 0)
        .unwrap_or_else(|e| panic!("the shipped face does not parse: {e}"));
    for (c, name) in [(OPEN, "U+25BC open fold"), (CLOSED, "U+25BA closed fold")] {
        assert!(face.glyph_index(c).is_some(), "the shipped face has no glyph for {name} ({c:?}), so the fold draws a box");
    }
    for (c, name) in [('\u{25be}', "U+25BE"), ('\u{25b8}', "U+25B8")] {
        assert!(
            face.glyph_index(c).is_none(),
            "the shipped face now has {name} ({c:?}); docs/design/icon-vocabulary.md's option-1 constraint is that it does \
             not, and the recommendation should be revisited rather than the document quietly left wrong"
        );
    }
    for (c, name) in [('v', "the letter `v`"), ('>', "the letter `>`")] {
        assert!(
            face.glyph_index(c).is_some(),
            "{name} is in the face — which is exactly why #493's first item is the shape and not coverage"
        );
    }
}
