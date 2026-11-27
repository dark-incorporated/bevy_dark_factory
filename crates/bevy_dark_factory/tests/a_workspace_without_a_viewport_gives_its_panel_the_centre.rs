//! **A workspace with nothing to show in the middle gives its panel the centre** (#694; the owner chose "the panel
//! fills the centre", 2026-11-27).
//!
//! The review measured Sound, Agent, Project and Game (before Play) at 1360×850: everything a person reads or clicks
//! in one dock about 310 px wide, and the viewport beside it black, 39 to 47% of the window (`blueprint/state`'s
//! `rects.viewport`). Those four draw no map and have no page of their own in the middle, as Files and Design do, so
//! the viewport they kept was room taken from the panel that has the work. Now that panel's pane is the whole body:
//! the viewport and its divider are out of the layout and the panel is as wide as the body less its own margins.
//! Every workspace that shows the map, and the two with a page of their own, keep their centre.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use bevy_dark_factory::chrome::{Frame, PanelWidth, Side};
use bevy_dark_factory::harness;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// Frames the editor is given to lay out after it opens and after a resize.
const SETTLE_FRAMES: usize = 12;

/// The issue's window, 1360×850 logical, in physical texels at the harness's `UiScale` of 1.2.
const ISSUE_WINDOW: (u32, u32) = (1632, 1020);

/// A panel stands off its pane by the sheet's panel margin on each side; this is the most two of them may come to.
const MARGINS_PX: f32 = 40.0;

fn editor_on(workspace: Workspace) -> App {
    let root = fixtures::Fixture::new(&format!("centre_{}_{}", workspace.id(), std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, workspace.opens_on()).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, ISSUE_WINDOW.0, ISSUE_WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "the editor did not open on {}", workspace.title());
    app
}

/// A laid-out node's rectangle in logical px, `None` when it lays out to nothing.
fn rect(app: &App, entity: Entity) -> Option<Rect> {
    let node = app.world().get::<ComputedNode>(entity)?;
    let at = app.world().get::<UiGlobalTransform>(entity)?;
    let scale = node.inverse_scale_factor();
    let size = node.size() * scale;
    (size.x > 0.0 && size.y > 0.0).then(|| Rect::from_center_size(at.translation * scale, size))
}

/// The widest laid-out panel of the left dock: the workspace's own panel.
fn the_left_panel(app: &mut App) -> Option<Rect> {
    let panels: Vec<Entity> = {
        let world = app.world_mut();
        world.query::<(Entity, &PanelWidth)>().iter(world).filter(|(_, w)| w.side == Side::Left).map(|(e, _)| e).collect()
    };
    panels.into_iter().filter_map(|p| rect(app, p)).max_by(|a, b| a.width().total_cmp(&b.width()))
}

/// The room the body leaves the docks and the centre: the body, less the activity bar at its left end.
fn the_room(app: &mut App) -> Rect {
    let body = app.world().resource::<Frame>().body;
    let mut room = rect(app, body).expect("the body is laid out");
    let bars: Vec<Entity> = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_dark_factory::activity_bar::ActivityBar>>().iter(world).collect()
    };
    if let Some(bar) = bars.into_iter().filter_map(|b| rect(app, b)).next() {
        room.min.x = bar.max.x;
    }
    room
}

/// **The four take the centre; the six that draw something there keep it.** For each workspace, the panel's width
/// and the viewport's are printed, which is the before-and-after the acceptance asks to be quoted.
///
/// Fails before the change: on Sound, Agent, Project and Game the viewport was laid out beside a panel of ~310 px.
#[test]
fn a_workspace_without_a_viewport_gives_its_panel_the_centre() {
    let mut seen = Vec::new();
    for workspace in Workspace::ALL {
        let mut app = editor_on(workspace);
        let viewport = rect(&app, app.world().resource::<Frame>().viewport);
        let room = the_room(&mut app);
        let panel = the_left_panel(&mut app);
        seen.push((workspace.title(), panel.map(|p| p.width().round()), viewport.map(|v| v.width().round())));
        if workspace.panel_takes_the_centre() {
            assert_eq!(viewport, None, "{} draws nothing in the middle and still lays out a viewport", workspace.title());
            let panel = panel.unwrap_or_else(|| panic!("{} has no panel laid out", workspace.title()));
            assert!(
                room.width() - panel.width() <= MARGINS_PX,
                "{}'s panel is {} px wide in a room of {} px: it does not take the centre",
                workspace.title(),
                panel.width(),
                room.width()
            );
        } else {
            let viewport = viewport.unwrap_or_else(|| panic!("{} lost its centre", workspace.title()));
            assert!(viewport.width() > 0.0 && viewport.height() > 0.0, "{}'s centre is laid out", workspace.title());
        }
    }
    println!("workspace, left panel px, viewport px: {seen:?}");
}
