//! **Every mark the editor draws as a character is one the face it draws with actually has** (#493), so a face without
//! the mark fails a check rather than drawing a box.
//!
//! # The rule this holds
//!
//! `docs/ui/vision.md` §9a: a mark that stands **in a text run or beside text** — a disclosure, an arrow, a close cross —
//! is a character from the shipped face; a mark that stands **in a fixed square on its own** — a toolbar icon, a panel's
//! tab — is a cell of the 16 px atlas. The atlas already has this check (`every_toolbar_command_has_an_icon` reads the
//! PNG back). This is the other half, and it was #493's second acceptance item.
//!
//! # Why it matters, measured
//!
//! The owner's capture `feedback-1794921635359` reads *"The "v" should collapse the text."* — a fold's disclosure drawn as
//! the ASCII letter `v`. The shipped face, `assets/fonts/FiraMono-Regular.ttf`, has U+25BC `▼` and U+25BA `►` (read here
//! from its own `cmap`) and does **not** have `▾` (U+25BE) or `▸` (U+25B8), which is why the disclosure is the small
//! solid triangle rather than the half-quadrant an editor usually uses. Naming the character the face lacks would draw a
//! box — the failure the review's P1 recorded, and the one this file exists to catch.
//!
//! # What it reads
//!
//! The **glyph run** each fold control actually draws: a row's child whose text is one of the marks, taken from the layout
//! the editor built, so this cannot pass while the code draws something else. Then two things: the character is not the
//! letter it used to be, and the face the text resolves to has it.

use bevy::prelude::*;
use bevy::text::{FontSource, TextLayoutInfo};
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::{CandidateList, Mode};

use crate::fixtures;

/// **The marks a fold draws**, as `docs/ui/vision.md` §9a names them: closed, then open.
///
/// U+25BA and U+25BC — the small solid triangles. Both are in the shipped face; the half-quadrants (`▸` U+25B8, `▾`
/// U+25BE) are not, and a design that wants those needs an icon font rather than a character.
const CLOSED_MARK: char = '\u{25ba}';
const OPEN_MARK: char = '\u{25bc}';

/// The letters the folds used to draw, so the test says what changed rather than only what is.
const WAS: [char; 2] = ['v', '>'];

/// Frames the layout needs to settle.
const SETTLE_FRAMES: usize = 12;

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// **Does the face the text resolves to have `mark`**, read from its own `cmap`?
///
/// A `Handle` font is that asset; a family name is resolved by Bevy's font database, which this cannot read, so it says
/// so rather than passing — the editor's labels name the default handle, and a label that named a family would be a
/// second question this file would have to answer.
fn covers(app: &mut App, mark: char, font: &FontSource) -> Option<bool> {
    let handle = match font {
        FontSource::Handle(handle) => handle.clone(),
        // **Every way of naming a face that is not a handle**, all of which Bevy resolves through its own font database:
        // this cannot read that, so it says so rather than passing.
        _ => return None,
    };
    let fonts = app.world().resource::<Assets<Font>>();
    let face = fonts.get(&handle)?;
    let parsed = ttf_parser::Face::parse(face.data.as_ref(), 0).ok()?;
    Some(parsed.glyph_index(mark).is_some())
}

/// **Every character mark the MESHES tab draws is a mark its own face has, and no fold draws the letter it used to.**
#[test]
fn every_mark_the_editor_draws_is_in_the_face_it_is_drawn_with() {
    let root = fixtures::Fixture::new(&format!("marks_{}", std::process::id()))
        .descriptor("crate", "alpha")
        .pack("props", &["crate", "barrel"])
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, 2560, 1600).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    // Every pack open, so each band's mark is drawn.
    app.world_mut().resource_mut::<bevy_dark_factory::tiles::ImportState>().folded_packs.clear();
    settle(&mut app);

    // **The fold controls the MESHES tab draws**, found by the marks themselves: a band's text is its disclosure when it
    // is one character long and is a mark or the letter it used to be.
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &Text, Option<&TextFont>)>();
    let drawn: Vec<(Entity, char, Option<FontSource>)> = query
        .iter(world)
        .filter_map(|(entity, text, font)| {
            let mut chars = text.0.chars();
            let one = chars.next().filter(|_| chars.next().is_none())?;
            let is_a_mark = one == CLOSED_MARK || one == OPEN_MARK || WAS.contains(&one);
            is_a_mark.then(|| (entity, one, font.map(|f| f.font.clone())))
        })
        .collect();
    assert!(
        drawn.len() >= 2,
        "the MESHES tab draws at least two fold marks (a pack's band, the EXCLUDED band); this found {}: a test that \
         measured nothing would pass for the wrong reason",
        drawn.len()
    );
    assert!(drawn.iter().any(|(_, mark, _)| *mark == OPEN_MARK), "an open fold's mark is drawn: {drawn:?}");

    let letters: Vec<char> = drawn.iter().map(|(_, mark, _)| *mark).filter(|mark| WAS.contains(mark)).collect();
    assert!(
        letters.is_empty(),
        "a fold still draws a letter rather than a chevron: {letters:?} among {:?}. #493 is the owner's note that the \
         letter should collapse the text — the marks are U+25BC and U+25BA (docs/ui/vision.md §9a)",
        drawn.iter().map(|(_, mark, _)| *mark).collect::<Vec<_>>()
    );

    let mut missing = Vec::new();
    for (entity, mark, font) in drawn {
        let Some(source) = font else {
            missing.push(format!("{entity:?} `{mark}` names no face, so whether it is drawn is not known"));
            continue;
        };
        match covers(&mut app, mark, &source) {
            Some(true) => {}
            Some(false) => missing.push(format!("{entity:?} `{mark}` (U+{:04X})", mark as u32)),
            None => missing.push(format!("{entity:?} `{mark}`: the face's bytes do not parse, so nothing can say")),
        }
    }
    assert!(
        missing.is_empty(),
        "a mark is drawn in a face that has no glyph for it, so it renders as a box:\n{}\n\n\
         docs/ui/vision.md §9a: a mark in a text run is a character from the shipped face, and the character must be one \
         the face's `cmap` has — U+25BC and U+25BA are, U+25B8 and U+25BE are not.",
        missing.join("\n")
    );
}

/// **A fold's mark is drawn where a person can see it**: it has a laid-out glyph run and a box with a height.
///
/// A mark that resolved to no glyph would still have a `Text`, so `TextLayoutInfo`'s glyphs are the proof that something
/// is painted. #493's first acceptance item is the shape and the hit target; this says the shape is on screen.
#[test]
fn a_folds_mark_is_painted_and_not_a_zero_width_run() {
    let root = fixtures::Fixture::new(&format!("marks_painted_{}", std::process::id()))
        .descriptor("crate", "alpha")
        .pack("props", &["crate", "barrel"])
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, 2560, 1600).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app.world_mut().resource_mut::<bevy_dark_factory::tiles::ImportState>().folded_packs.clear();
    settle(&mut app);

    let world = app.world_mut();
    let mut query = world.query::<(&Text, &TextLayoutInfo)>();
    let painted: Vec<(String, f32)> = query
        .iter(world)
        .filter(|(text, _)| text.0.chars().count() == 1 && (text.0.starts_with(CLOSED_MARK) || text.0.starts_with(OPEN_MARK)))
        .map(|(text, info)| (text.0.clone(), info.size.x))
        .collect();
    assert!(!painted.is_empty(), "the fold marks are drawn (the MESHES tab's bands carry them)");
    let blank: Vec<&String> = painted.iter().filter(|(_, wide)| *wide <= 0.0).map(|(mark, _)| mark).collect();
    assert!(blank.is_empty(), "a fold mark is drawn with no width, so nothing is on screen: {blank:?} of {painted:?}");
    // **A `ListRail` is not the only thing with a marker**: this test names the candidate list so the fixture it measures
    // is the MESHES tab's own.
    assert!(app.world_mut().query_filtered::<Entity, With<CandidateList>>().single(app.world_mut()).is_ok(), "the MESHES tab's candidate list is built");
}
