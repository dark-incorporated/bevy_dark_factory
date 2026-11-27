//! **The status band's `Art` control reads the art's share of the window, and one press gives the art the
//! window** (#669).
//!
//! The owner's decision (2026-11-27): *"two states, Work and Look. A status-band control labelled with the game
//! art's share of the window (e.g. `Art 32%`); click cycles Work (today's layout) ↔ Look (both side docks folded
//! and the chrome bar, toolbar, panel tab row and status band hidden except this control, so the art gets the
//! window)."*
//!
//! What each test asserts is what a person and an agent read: the label's words, `blueprint/state`'s
//! `rects`, and the laid-out size of the viewport against the window's, measured here from the frame's own
//! nodes rather than through the function the control is drawn from — so the label is checked against the
//! layout, not against itself.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use serde_json::{Value, json};

use bevy_dark_factory::chrome::Frame;
use bevy_dark_factory::shell_view::{ArtShare, ShellView, art_share_words};

use crate::fixtures;

/// **Look's floor at 1600 × 1000**: the art is the whole window, less what a frame's rounding can take
/// (2 px of 1600 and of 1000 is 0.32%). Work's share on this map is about a third, so a press that gave
/// back anything less than nearly all of the window fails here.
const LOOK_FLOOR: f32 = 0.99;

/// The window every number here is measured at: the issue's own size.
const WINDOW: (u32, u32) = (1600, 1000);

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    bevy_dark_factory::harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

/// Frames for a fold, a move and a layout to land, and for the label to read the layout they made.
fn settle(app: &mut App) {
    for _ in 0..12 {
        app.update();
    }
}

fn call(app: &mut App, method: &str, params: Value) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("no method `{method}` is registered"),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

/// `blueprint/state`'s `rects`, as an agent reads them.
fn state_rects(app: &mut App) -> Value {
    call(app, "blueprint/state", json!({}))["rects"].clone()
}

/// A node's laid-out size in logical px.
fn size_of(app: &App, entity: Entity) -> Vec2 {
    app.world().get::<ComputedNode>(entity).map_or(Vec2::ZERO, |n| n.size() * n.inverse_scale_factor())
}

/// **The art's share, measured from the frame's own two nodes**: the viewport's area over the root's.
fn measured_share(app: &App) -> f32 {
    let frame = app.world().resource::<Frame>();
    let (viewport, window) = (size_of(app, frame.viewport), size_of(app, frame.root));
    assert!(window.x * window.y > 0.0, "the window is laid out: {window:?}");
    viewport.x * viewport.y / (window.x * window.y)
}

/// The control: its entity and its words.
fn control(app: &mut App) -> (Entity, String) {
    let world = app.world_mut();
    let found: Vec<Entity> = world.query_filtered::<Entity, With<ArtShare>>().iter(world).collect();
    assert_eq!(found.len(), 1, "one `Art` control, and it is {found:?}");
    let entity = found[0];
    let words = world
        .get::<Children>(entity)
        .and_then(|kids| kids.iter().find_map(|kid| world.get::<Text>(kid).map(|text| text.0.clone())))
        .expect("the control says something");
    (entity, words)
}

/// **A click through `bevy_picking`'s own input**, on the image the editor draws its interface into, at the
/// centre of `entity` (as `the_terminal_starts_collapsed_with_a_label_in_the_status_bar` clicks its label).
fn click(app: &mut App, entity: Entity) {
    let at = app.world().get::<UiGlobalTransform>(entity).expect("the control is laid out").translation;
    let handle = app.world().get_resource::<bevy_dark_factory::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    for action in [PointerAction::Move { delta: Vec2::ZERO }, PointerAction::Press(PointerButton::Primary), PointerAction::Release(PointerButton::Primary)] {
        let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle: handle.clone(), scale_factor: 1.0 }), position: at };
        app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
        app.update();
    }
    settle(app);
}

/// **The control is on screen**: laid out to a size, and every corner of it inside the window.
fn assert_reachable(app: &mut App, entity: Entity, when: &str) {
    let size = size_of(app, entity);
    let window = size_of(app, app.world().resource::<Frame>().root);
    let node = app.world().get::<ComputedNode>(entity).expect("the control is a node");
    let centre = app.world().get::<UiGlobalTransform>(entity).expect("the control is laid out").translation * node.inverse_scale_factor();
    let (low, high) = (centre - size / 2.0, centre + size / 2.0);
    assert!(size.x > 0.0 && size.y > 0.0, "{when}: the control lays out to nothing ({size:?}), so there is no way to press it");
    assert!(
        low.x >= 0.0 && low.y >= 0.0 && high.x <= window.x && high.y <= window.y,
        "{when}: the control spans {low:?}..{high:?}, outside the {window:?} window"
    );
}

/// **The label is the measured share, and `blueprint/state` reports the same number** (#669's first change):
/// the control and the thing a reader checks it against are one number.
#[test]
fn the_label_is_the_measured_share_and_the_state_reports_it() {
    let mut app = editor("art_share_label");
    let share = measured_share(&app);
    let (_, words) = control(&mut app);

    assert_eq!(words, art_share_words(Some(share)), "the label says {words:?} where the art measures {share} of the window");
    assert!(share < LOOK_FLOOR, "Work leaves the editor's chrome on screen: the art is {share} of the window");

    let rects = state_rects(&mut app);
    let reported = rects["art_share"].as_f64().unwrap_or_else(|| panic!("`rects.art_share` is a number once laid out: {rects}"));
    assert!(
        (reported - f64::from(share)).abs() < 1e-6,
        "`rects.art_share` is {reported} and the laid-out viewport over the window is {share}: {rects}"
    );
    let (viewport, window) = (&rects["viewport"], &rects["window"]);
    let area = |r: &Value| r[2].as_f64().unwrap_or_default() * r[3].as_f64().unwrap_or_default();
    assert!(
        (reported - area(viewport) / area(window)).abs() < 1e-6,
        "`art_share` is the viewport's w*h over the window's, from the same section: {rects}"
    );
}

/// **One click is Look and the art gets the window; a second click is Work, and the rects are exactly what
/// they were** (#669's tests). The control stays on screen in Look, because it is the way back.
#[test]
fn one_click_gives_the_art_the_window_and_a_second_gives_work_back_exactly() {
    let mut app = editor("art_share_click");
    let work_rects = state_rects(&mut app);
    let work_share = measured_share(&app);
    let (art, work_words) = control(&mut app);
    assert_reachable(&mut app, art, "in Work");
    assert!(!app.world().resource::<ShellView>().zen, "the editor opens in Work");

    click(&mut app, art);

    assert!(app.world().resource::<ShellView>().zen, "one click is Look, which is Zen mode");
    let look_share = measured_share(&app);
    assert!(
        look_share >= LOOK_FLOOR,
        "Look gives the art the window: it is {look_share} of it at {WINDOW:?} (Work was {work_share}), and the floor is {LOOK_FLOOR}"
    );
    let (floating, look_words) = control(&mut app);
    assert_eq!(floating, art, "the control in Look is the one from the band");
    assert_eq!(look_words, art_share_words(Some(look_share)), "and its label is Look's share");
    assert_eq!(
        state_rects(&mut app)["art_share"].as_f64().map(|s| (s - f64::from(look_share)).abs() < 1e-6),
        Some(true),
        "the state reports Look's share too"
    );
    assert_reachable(&mut app, art, "in Look");

    click(&mut app, art);

    assert!(!app.world().resource::<ShellView>().zen, "the second click is Work");
    assert_eq!(state_rects(&mut app), work_rects, "Work's rects come back exactly");
    let (_, back_words) = control(&mut app);
    assert_eq!(back_words, work_words, "and the label says Work's share again");
    let band = app.world().resource::<Frame>().status;
    assert_eq!(
        app.world().get::<ChildOf>(art).map(ChildOf::parent),
        Some(band),
        "the control is back in the status band"
    );
}

/// **`blueprint/run_command` reaches the control** — the press is `shell.zen`'s, so the agent's door, the chord
/// and the click are one press, and the label follows whichever made it.
#[test]
fn run_command_reaches_the_control() {
    let mut app = editor("art_share_run");
    let (_, work_words) = control(&mut app);

    let on = call(&mut app, "blueprint/run_command", json!({"id": "shell.zen"}));
    settle(&mut app);
    assert_eq!(on["outcome"], json!("ran"), "{on}");
    let look_share = measured_share(&app);
    let (_, look_words) = control(&mut app);
    assert!(look_share >= LOOK_FLOOR, "the door gave the art {look_share} of the window");
    assert_eq!(look_words, art_share_words(Some(look_share)), "and the control reads it");

    let off = call(&mut app, "blueprint/run_command", json!({"id": "shell.zen"}));
    settle(&mut app);
    assert_eq!(off["outcome"], json!("ran"), "{off}");
    assert_eq!(control(&mut app).1, work_words, "the second call is Work, and the control says so");
}
