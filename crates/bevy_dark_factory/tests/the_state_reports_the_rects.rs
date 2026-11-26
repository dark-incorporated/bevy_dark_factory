//! **`blueprint/state` reports the viewport's rect and the window's** (#665).
//!
//! An agent judging *"the art is 32% of the window"* had no way to read either rectangle: it had to
//! `world.query` for `Camera` and `ComputedNode` and subtract. The state contract now carries both, and
//! this test is the reading — the numbers must come from the editor's own laid-out nodes, at the sizes
//! the node reports, in logical pixels.
//!
//! The helper is exercised directly: `editor_state` is a BRP method and a test has no socket, but the
//! function that produces the section takes nothing but the query, so it is testable on its own and the
//! only untested part is the one line that forwards it.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_dark_factory::chrome::ViewportSlot;
use bevy_dark_factory::ops_brp::viewport_and_window;

/// **A laid-out app**: one root the width of a window, and one `ViewportSlot` inside it.
///
/// The sizes are physical pixels with `inverse_scale_factor` left at the default, so the logical numbers
/// the section reports are the ones written here.
fn laid_out(root: Vec2, viewport: Vec2) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    let root_entity = app
        .world_mut()
        .spawn((Node::default(), ComputedNode::default(), UiGlobalTransform::default()))
        .id();
    let viewport_entity = app
        .world_mut()
        .spawn((
            ViewportSlot,
            Node::default(),
            ComputedNode::default(),
            UiGlobalTransform::default(),
        ))
        .id();
    // **`ComputedNode::size` is a public field**, so a test writes the laid-out numbers the layout
    // would have written. `inverse_scale_factor` stays at its default of 1, so logical == physical.
    app.world_mut()
        .get_mut::<ComputedNode>(root_entity)
        .expect("a node")
        .size = root;
    app.world_mut()
        .get_mut::<ComputedNode>(viewport_entity)
        .expect("a node")
        .size = viewport;
    // The viewport sits below the chrome bar, which is where the issue's [485, 153, …] came from.
    *app.world_mut()
        .get_mut::<UiGlobalTransform>(viewport_entity)
        .expect("a transform") = UiGlobalTransform::from_translation(Vec2::new(485.0, 153.0));
    app.update();
    app
}

/// **The section names both rectangles, and they are the nodes' own numbers.**
#[test]
fn the_state_reports_the_viewport_and_the_window() {
    let mut app = laid_out(Vec2::new(1600.0, 1000.0), Vec2::new(810.0, 638.0));
    let world = app.world_mut();
    let mut query =
        world.query::<(&ComputedNode, &UiGlobalTransform, Has<ViewportSlot>)>();
    let section = viewport_and_window(&query);

    let viewport = &section["viewport"];
    let window = &section["window"];
    assert!(
        !viewport.is_null(),
        "**the state names no viewport rect** (#665), so an agent cannot read the art's share of the \
         window without a `world.query`. The whole section was: {section}"
    );
    assert!(
        !window.is_null(),
        "**the state names no window rect** (#665). The whole section was: {section}"
    );
    assert_eq!(
        viewport,
        &serde_json::json!([485.0, 153.0, 810.0, 638.0]),
        "the viewport's own laid-out rect, in logical px: where `ViewportSlot`'s node is drawn and how \
         big it is"
    );
    assert_eq!(
        window,
        &serde_json::json!([0.0, 0.0, 1600.0, 1000.0]),
        "and the window's, which is the root the shell is sized from"
    );
}

/// **The window is the widest node that is not the viewport**, which is how the root is found without a
/// marker — and a narrow dock beside it must not be mistaken for it.
#[test]
fn a_dock_is_not_reported_as_the_window() {
    let mut app = laid_out(Vec2::new(1600.0, 1000.0), Vec2::new(810.0, 638.0));
    // A left dock, narrower than the root.
    let mut dock = ComputedNode::default();
    dock.size = Vec2::new(426.0, 638.0);
    app.world_mut()
        .spawn((Node::default(), dock, UiGlobalTransform::default()));
    app.update();

    let world = app.world_mut();
    let mut query =
        world.query::<(&ComputedNode, &UiGlobalTransform, Has<ViewportSlot>)>();
    let section = viewport_and_window(&query);
    assert_eq!(
        section["window"],
        serde_json::json!([0.0, 0.0, 1600.0, 1000.0]),
        "the 426 px dock is not the window: the widest node is 1600 px across"
    );
}
