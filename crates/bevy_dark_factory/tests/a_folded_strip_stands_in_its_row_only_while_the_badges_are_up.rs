//! **A folded strip stands in its own row while the shortcuts key is held, and folds back to the exact layout on release**
//! (#670 with the badges; the owner's call "A", 2026-11-27).
//!
//! Folded into the chrome bar, the strip shares one row with the way out and the title, and their three badges do not fit
//! in it (measured: the strip's badge is 627 px, the title's 232 to 411, and the way out's had the folded tab against it).
//! So for the length of the hold the strip stands in its row, where its badge has the row and the way out's has the bar's
//! free ground, and on release it folds back: the body is the rect it was, to the bit, so the hold leaves nothing behind.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use bevy_dark_factory::chrome::{Control, Frame};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::{Action, ControlId, binding};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// Frames for a key change to reach the layout: the reparent is a command, and layout lands a frame after it.
const SETTLE_FRAMES: usize = 8;

/// The height the strip's row has when it stands in it, logical px.
const ROW_H: f32 = 35.0;

fn rect(app: &App, entity: Entity) -> Option<Rect> {
    let node = app.world().get::<ComputedNode>(entity)?;
    let at = app.world().get::<UiGlobalTransform>(entity)?;
    Some(Rect::from_center_size(at.translation, node.size()))
}

/// The strip's parent: the chrome bar when it is folded, the door strip's band when it stands in its row.
fn strip_parent(app: &mut App) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(&Control, &ChildOf)>();
    let parents: Vec<Entity> = q.iter(world).filter(|(c, _)| c.0 == ControlId::WorkspaceStrip).map(|(_, p)| p.parent()).collect();
    assert_eq!(parents.len(), 1, "one workspace strip: {parents:?}");
    parents[0]
}

fn step(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

#[test]
fn a_folded_strip_unfolds_while_k_is_held_and_the_body_returns_to_its_exact_rect() {
    let root = fixtures::Fixture::new(&format!("unfold_{}", std::process::id())).descriptor("floor", "alpha").place("floor", (0.0, 0.0)).build("m");
    let key = binding(Action::Shortcuts).key;
    for workspace in [Workspace::Map, Workspace::Agent, Workspace::Design] {
        assert!(workspace.folds_its_tabs_into_the_chrome_bar(), "{} has one panel", workspace.title());
        let mut app = harness::build_headless_at(&root, "m", None, workspace.opens_on()).unwrap_or_else(|e| panic!("{e}"));
        step(&mut app);
        let (bar, band, body) = {
            let frame = app.world().resource::<Frame>();
            (frame.chrome_bar, frame.door_strip, frame.body)
        };
        let before = rect(&app, body).unwrap_or_else(|| panic!("{}'s body is laid out", workspace.title()));
        assert_eq!(strip_parent(&mut app), bar, "{}'s strip starts folded into the chrome bar", workspace.title());

        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
        step(&mut app);
        assert_eq!(strip_parent(&mut app), band, "{}: with the key held the strip stands in its own row", workspace.title());
        let scale = app.world().get::<ComputedNode>(band).map_or(1.0, |n| n.inverse_scale_factor());
        let row = rect(&app, band).map_or(0.0, |r| r.height() * scale);
        assert!((row - ROW_H).abs() <= 0.5, "{}: the row is {row} px tall while the key is held, not {ROW_H}", workspace.title());

        app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key);
        step(&mut app);
        assert_eq!(strip_parent(&mut app), bar, "{}: on release the strip folds back into the chrome bar", workspace.title());
        let after = rect(&app, body).unwrap_or_else(|| panic!("{}'s body is laid out", workspace.title()));
        assert_eq!(after, before, "{}: the body is not the rect it was before the hold", workspace.title());
        let row = rect(&app, band).map_or(0.0, |r| r.height());
        assert_eq!(row, 0.0, "{}: the row lays out to nothing again", workspace.title());
    }
}
