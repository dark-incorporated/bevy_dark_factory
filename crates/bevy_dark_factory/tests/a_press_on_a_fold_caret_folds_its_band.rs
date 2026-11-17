//! **A fold's caret is a chevron, and a press on the chevron itself folds** (#493).
//!
//! The owner's capture `feedback-1794921635359` said two things and they are two: *"The "v" should collapse the text. And we
//! should consider an icon solution for icons like up and down arrows, etc."* This file holds the editor's own half of the first,
//! and `docs/ui/icons.md` is the second, written down before the atlas grows again.
//!
//! # What was wrong
//!
//! The PLACE panel's category bands and the MESHES panel's pack bands drew their fold control as the **letter** `v` (`>` when
//! folded) — a character a person reads as prose in a gutter rather than as the disclosure control it is. Both now draw
//! [`bevy_cauldron::glyphs::FOLD_OPEN`] (`▼`) and [`bevy_cauldron::glyphs::FOLD_CLOSED`] (`►`).
//!
//! # The second half, and where it lives
//!
//! The capture points at the **file editor**, whose caret is `bevy_gooey`'s `code_overlay.rs` and is `Pickable::IGNORE` — a
//! second, un-editable copy of the same defect, filed as `bevy-coven/bevy_gooey#107`. The editor's own carets sit inside a band
//! that is itself a `Button`, so a press on the glyph bubbles to it; this file proves that rather than assuming it, because "the
//! shape that looks like the control is not the control" is exactly what the owner reported.
//!
//! # What is held
//!
//! 1. A fold's caret is one of the vocabulary's two shapes, and neither is a letter.
//! 2. A press **at the caret's own rect** toggles the fold it belongs to: the band's row count changes and the caret's shape
//!    changes with it.
//!
//! Nothing here reads the source of the spawn; both assertions read the running editor's components and its laid-out rects, so a
//! later panel that draws its caret another way is measured as it is.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use bevy_cauldron::editor::{CategoryHeader, PaletteList};
use bevy_cauldron::glyphs;
use bevy_cauldron::harness;

use crate::fixtures;

/// Frames a layout needs to settle after the palette is rebuilt.
const SETTLE_FRAMES: usize = 12;

/// The window a test lays the interface out at, physical px: the size the captures were taken at. Without a surface size there is
/// no laid-out rect for a press to aim at, and `ComputedNode` is what a press at a rect reads.
const WINDOW: (u32, u32) = (2560, 1604);

/// The fixture's pieces. They share one kind token, so the palette draws them under **one** category band — which is what a fold
/// has something to hide.
const PIECES: [&str; 3] = ["crate", "barrel", "wall_low"];

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// An editor on a project with one category of pieces, so the PLACE panel draws a band worth folding.
fn editor(tag: &str) -> App {
    // The fixture ships the one `prop` kind, so all three pieces fall under the palette's single category band.
    let mut fixture = fixtures::Fixture::new(&format!("fold_chevron_{tag}_{}", std::process::id()));
    for piece in PIECES {
        fixture = fixture.descriptor(piece, "alpha");
    }
    let root = fixture.build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

/// Every laid-out text under `root`, with the entity it is drawn on and the rect it covers.
fn texts_under(app: &mut App, root: Entity) -> Vec<(Entity, String, Rect)> {
    let world = app.world_mut();
    let mut found = Vec::new();
    let mut next = vec![root];
    let mut seen: Vec<Entity> = vec![root];
    while let Some(at) = next.pop() {
        if let Some(text) = world.get::<Text>(at) {
            let computed = world.get::<ComputedNode>(at).map(|c| c.size());
            let at_xy = world.get::<UiGlobalTransform>(at).map(|t| t.translation);
            if let (Some(size), Some(xy)) = (computed, at_xy) {
                found.push((at, text.0.clone(), Rect::from_center_size(xy, size)));
            }
        }
        if let Some(children) = world.get::<Children>(at) {
            for kid in children.iter() {
                if !seen.contains(&kid) {
                    seen.push(kid);
                    next.push(kid);
                }
            }
        }
    }
    found
}

/// The palette's own root: the scrolling list the bands are drawn in.
fn palette(app: &mut App) -> Entity {
    let world = app.world_mut();
    world
        .query_filtered::<Entity, With<PaletteList>>()
        .single(world)
        .unwrap_or_else(|e| panic!("the Map door draws the PLACE palette: {e}"))
}

/// **The band a person is looking at, and the fold caret it draws — read in one snapshot so the two cannot disagree.**
///
/// The palette spawns a band per door, and only the showing door's is laid out: the others are `Display::None` and their rows carry
/// no `ComputedNode`. It also **rebuilds** the bands whenever the library or the filter moves, so an entity read from one query and
/// used after another is an entity that may no longer be the band. Both are therefore found and used here, with nothing between.
///
/// Returns the band, its category, the caret's entity, the shape it draws, and the rect that shape covers.
fn a_band_and_its_caret(app: &mut App) -> (Entity, String, Entity, String, Rect) {
    let bands = {
        let world = app.world_mut();
        world
            .query::<(&CategoryHeader, Entity)>()
            .iter(world)
            .map(|(header, entity)| (entity, header.0.clone()))
            .collect::<Vec<_>>()
    };
    assert!(!bands.is_empty(), "the fixture's one kind draws a category band");
    let mut refused = Vec::new();
    for (band, category) in bands {
        let mut carets = texts_under(app, band)
            .into_iter()
            .filter(|(_, words, _)| words == glyphs::FOLD_OPEN || words == glyphs::FOLD_CLOSED)
            .collect::<Vec<_>>();
        match carets.len() {
            1 => {
                let (caret, shape, rect) = carets.remove(0);
                return (band, category, caret, shape, rect);
            }
            0 => refused.push(format!("`{category}` draws no fold caret")),
            n => refused.push(format!("`{category}` draws {n} fold carets")),
        }
    }
    panic!("no category band draws exactly one fold caret: {}", refused.join("; "));
}

/// **The fold caret is one of the vocabulary's two shapes, and neither is a letter.**
#[test]
fn a_category_bands_caret_is_a_chevron() {
    let mut app = editor("shape");
    let (_, category, _, words, _) = a_band_and_its_caret(&mut app);
    assert!(
        words == glyphs::FOLD_OPEN || words == glyphs::FOLD_CLOSED,
        "the `{category}` band's fold caret draws {words:?}; it must be `{:?}` while open or `{:?}` while folded",
        glyphs::FOLD_OPEN,
        glyphs::FOLD_CLOSED
    );
    assert!(
        !words.is_ascii(),
        "the caret {words:?} is ASCII, so it is the letter the owner read as prose rather than a shape"
    );
}

/// **A press at the caret's own rect folds the band, and the caret changes shape with it** (#493's acceptance item).
///
/// This is the half the owner was actually reporting: *the "v" should collapse the text*. The press is aimed at the
/// **caret's** reported rect, not the band's centre — a press on the band's middle lands on its words, which is a different
/// question (and the reason `bevy_gooey`'s own `testing::click` is not used here: it reads a node's centre for itself).
#[test]
fn a_press_on_the_caret_folds_the_band() {
    let mut app = editor("press");
    let (_, category, caret, shape, rect) = a_band_and_its_caret(&mut app);
    assert_eq!(shape, glyphs::FOLD_OPEN, "the band starts open, so the fixture proves a fold rather than an unfold");

    // **The pointer and the press, at the caret's own centre** on the image the interface is drawn into — the target
    // `bevy_ui`'s picking backend reads. `PointerClick` is a press and a release on the same target; a press alone leaves the
    // pointer down and emits nothing.
    let centre = rect.center();
    // **What the palette holds before the press.** Read here rather than earlier so the band the press aims at and the count it is
    // judged by come from the same moment.
    let before = {
        let list = palette(&mut app);
        texts_under(&mut app, list).len()
    };
    let handle = app
        .world()
        .get_resource::<bevy_cauldron::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    let location = || bevy::picking::pointer::Location {
        target: bevy::camera::NormalizedRenderTarget::Image(bevy::camera::ImageRenderTarget {
            handle: handle.clone(),
            scale_factor: 1.0,
        }),
        position: centre,
    };
    for action in [
        bevy::picking::pointer::PointerAction::Move { delta: Vec2::ONE },
        bevy::picking::pointer::PointerAction::Press(bevy::picking::pointer::PointerButton::Primary),
        bevy::picking::pointer::PointerAction::Release(bevy::picking::pointer::PointerButton::Primary),
    ] {
        app.world_mut()
            .write_message(bevy::picking::pointer::PointerInput::new(bevy::picking::pointer::PointerId::Mouse, location(), action));
        app.update();
    }
    settle(&mut app);

    // **The band folded**: the pieces under it are gone, and the caret now says closed. Both are read from the running editor, so
    // a press that reached the band through a change of nothing else could not pass.
    let palette = palette(&mut app);
    let after = texts_under(&mut app, palette).len();
    assert!(
        after < before,
        "a press at the `{category}` caret (at {centre:?}) left the palette with {after} texts, the same as before ({before}): \
         the shape that looks like the control did not fold anything"
    );
    // **The band is drawn again, not edited in place**: the palette rebuilds its rows whenever the fold moves, so the band this
    // reads is the one now on screen rather than the entity the press was aimed at. That is the editor's own design (a row is a
    // function of the state), and a test that held the old entity would be holding a despawned row.
    let (_, _, _, shape_after, _) = a_band_and_its_caret(&mut app);
    assert_eq!(category, "prop", "the fixture's one kind is what the palette groups under, so the band read is the same band");
    assert_eq!(
        shape_after,
        glyphs::FOLD_CLOSED,
        "the band folded but its caret still draws {shape_after:?}; the fold control must change shape with what it folds"
    );
    let _ = caret;
}
