//! **The workbench split moves: the left pane slides shut and open, and a collapsed one peeks** (UI-R06, dark-incorporated/bevy_dark_factory#336).
//!
//! Cmd+B (View ▸ Show left panel) used to cut the left pane out of the layout in one frame (`Display::None` and a fraction of
//! nothing), so the viewport jumped. It collapses the shell's gooey split now, the way Enter on the divider does: the panes
//! slide on gooey's spring (G-62's size motion), and the collapsed pane **peeks** (gooey's #69) when the pointer rests on its
//! edge: it floats over the viewport at the width a restore would give it, **without the viewport changing size or place**.
//! The headless editor runs with motion off so every other test reads a result in the frame it is made; this puts the motion
//! back and steps a clock the test sets, so a frame is a known length.

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::ComputedNode;
use bevy::window::PrimaryWindow;
use bevy_dark_factory::chrome::{Frame, ShellSplit, ViewportSlot};
use bevy_dark_factory::shell_view::ShellView;
use bevy_gooey::{MotionSettings, Role, SplitMin, SplitPeek, SplitSaved};

use crate::fixtures;

/// One frame of the test clock, seconds.
const FRAME: f32 = 1.0 / 60.0;
/// A frame's slack in a laid-out size, logical px: layout is physical and rounds.
const SLACK_PX: f32 = 2.0;
/// A pane narrower than this is shut for the purposes of this file, logical px.
const SHUT_PX: f32 = 0.5;
/// Frames that settle any glide or peek of this file on the clock above (the spring takes a quarter of a second).
const SETTLE_FRAMES: usize = 90;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    for _ in 0..8 {
        app.update();
    }
    // The motion back on, as a person has it.
    app.insert_resource(MotionSettings::default());
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    app
}

fn frames(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// **Cmd+B, pressed and let go**: a frame down, a frame up, so a count of frames after it is a count from the press.
fn cmd_b(app: &mut App) {
    let keys = [bevy_dark_factory::keys::MOD_KEYS[0], KeyCode::KeyB];
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in keys {
            let logical_key = if key == KeyCode::KeyB { Key::Character("b".into()) } else { Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified) };
            app.world_mut().write_message(KeyboardInput { key_code: key, logical_key, state, text: None, repeat: false, window: Entity::PLACEHOLDER });
        }
        app.update();
    }
}

fn split(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<ShellSplit>>().single(world).expect("one shell split")
}

/// The split's children by role: the first pane, the divider.
fn parts(app: &App, split: Entity) -> (Entity, Entity) {
    let kids: Vec<Entity> = app.world().get::<Children>(split).expect("children").iter().collect();
    let by = |want: Role| kids.iter().copied().find(|&kid| app.world().get::<Role>(kid) == Some(&want)).expect("a part");
    (by(Role::SplitPane), by(Role::SplitDivider))
}

/// A laid-out box, logical px: its left edge and its width.
fn laid(app: &App, entity: Entity) -> (f32, f32) {
    let node = app.world().get::<ComputedNode>(entity).expect("laid out");
    let at = app.world().get::<bevy::ui::UiGlobalTransform>(entity).expect("placed").translation;
    let scale = node.inverse_scale_factor();
    ((at.x - node.size().x * 0.5) * scale, node.size().x * scale)
}

fn viewport(app: &mut App) -> (f32, f32) {
    let world = app.world_mut();
    let entity = world.query_filtered::<Entity, With<ViewportSlot>>().single(world).expect("one viewport").to_owned();
    laid(app, entity)
}

/// **One device pixel in logical px**, the grain layout rounds every edge to. The shell's divider is `DIVIDER_W_PX` (12 logical, `SplitThickness`): at
/// the editor's `UiScale` that is a fractional number of device pixels, so the viewport's left edge sits at a fraction and a tiny change in the floating
/// pane's residue can flip it to the next whole device pixel without the viewport having moved.
///
/// **Measured (#108, this test's own run):** with the divider at 12 px the viewport's rounded `(left, width)` is `(36.667, 828.333)` collapsed and
/// `(37.5, 827.5)` under the peek (its left edge one device pixel on, its right edge the same), and its width before rounding is `828.2318` then
/// `827.7318`. With gooey's own 24 px (the same test with `SplitThickness` removed) it is `(49.167, 815.833)` both times and `816.2318` then
/// `816.0651`. The collapsed widths differ by exactly 12 logical px, the divider's change; the residue under the peek is 0.5 logical px now and
/// 0.167 then, both under one device pixel (0.833). What makes the residue larger is not traced.
fn device_px(app: &mut App) -> f32 {
    let world = app.world_mut();
    let entity = world.query_filtered::<Entity, With<ViewportSlot>>().single(world).expect("one viewport").to_owned();
    world.get::<ComputedNode>(entity).expect("laid out").inverse_scale_factor()
}

/// The viewport's width before layout rounds it, logical px.
fn unrounded_width(app: &mut App) -> f32 {
    let world = app.world_mut();
    let entity = world.query_filtered::<Entity, With<ViewportSlot>>().single(world).expect("one viewport").to_owned();
    let node = world.get::<ComputedNode>(entity).expect("laid out");
    node.unrounded_size().x * node.inverse_scale_factor()
}

/// **Whether the viewport moved between two measurements**: its right edge by more than `SLACK_PX / 4`, or its left edge or width by more than one
/// device pixel (the grain above). A real move is many device pixels.
fn viewport_moved(a: (f32, f32), b: (f32, f32), one_device_px: f32) -> bool {
    ((a.0 + a.1) - (b.0 + b.1)).abs() > SLACK_PX / 4.0 || (a.0 - b.0).abs() > one_device_px + 1e-3 || (a.1 - b.1).abs() > one_device_px + 1e-3
}

fn left_dock_shown(app: &App) -> bool {
    let left = app.world().resource::<Frame>().left;
    app.world().get::<Node>(left).expect("a node").display != Display::None
}

fn cursor(app: &mut App, to: Option<Vec2>) {
    app.world_mut().resource_mut::<bevy_debugger_bevy::DebugCursor>().0 = to;
}

/// The centre of a node in the physical px the injected cursor is in.
fn centre(app: &App, entity: Entity) -> Vec2 {
    app.world().get::<bevy::ui::UiGlobalTransform>(entity).expect("placed").translation
}

#[test]
fn cmd_b_slides_the_left_pane_shut_and_open_and_the_viewport_follows_it() {
    let mut app = editor("shell_split_slides");
    let split = split(&mut app);
    let (pane, _) = parts(&app, split);
    let wide = laid(&app, pane).1;
    let (world_x, world_w) = viewport(&mut app);
    assert!(wide > 100.0, "the left pane is on screen to begin with: {wide}");
    let first_floor = app.world().get::<SplitMin>(split).expect("a minimum").first;
    assert!(first_floor > 0.0, "and it keeps a floor while it is open: {first_floor}");

    cmd_b(&mut app);
    frames(&mut app, 2);
    let mid = laid(&app, pane).1;
    let (_, world_mid) = viewport(&mut app);
    assert!(mid > SHUT_PX && mid < wide - SLACK_PX, "two frames after Cmd+B the pane is on its way, neither whole nor shut: {mid} of {wide}");
    assert!(world_mid > world_w + SLACK_PX, "and the viewport has begun to take the room: {world_w} to {world_mid}");

    frames(&mut app, SETTLE_FRAMES);
    assert!(laid(&app, pane).1 < SHUT_PX, "the pane is shut at the end: {}", laid(&app, pane).1);
    let (_, world_shut) = viewport(&mut app);
    assert!(world_mid < world_shut, "the viewport was still growing on the way: {world_mid} before {world_shut}");
    assert!(world_shut >= world_w + wide - SLACK_PX, "and it has the pane's room: {world_w} + {wide} against {world_shut}");
    assert!(app.world().get::<SplitSaved>(split).is_some(), "the split is collapsed the way gooey's own collapse is");
    assert!(!left_dock_shown(&app), "a shut pane shows no dock to a badge, a census or a Tab stop");
    assert!(app.world().resource::<ShellView>().left_hidden);

    cmd_b(&mut app);
    frames(&mut app, 2);
    let mid = laid(&app, pane).1;
    assert!(mid > SHUT_PX && mid < wide - SLACK_PX, "Cmd+B again: the pane opens on its way, it does not appear: {mid} of {wide}");
    frames(&mut app, SETTLE_FRAMES);
    assert!((laid(&app, pane).1 - wide).abs() <= SLACK_PX, "it is the width it was: {} against {wide}", laid(&app, pane).1);
    let (back_x, back_w) = viewport(&mut app);
    assert!((back_x - world_x).abs() <= SLACK_PX && (back_w - world_w).abs() <= SLACK_PX, "and the viewport is where it was: ({back_x}, {back_w}) against ({world_x}, {world_w})");
    assert!(app.world().get::<SplitSaved>(split).is_none(), "no longer collapsed");
    assert_eq!(app.world().get::<SplitMin>(split).expect("a minimum").first, first_floor, "and the pane has its floor back");
    assert!(left_dock_shown(&app));
    assert!(!app.world().resource::<ShellView>().left_hidden);
}

#[test]
fn a_collapsed_dock_peeks_under_the_pointer_and_the_viewport_does_not_move() {
    let mut app = editor("shell_split_peeks");
    let split = split(&mut app);
    let (pane, divider) = parts(&app, split);
    let wide = laid(&app, pane).1;

    cmd_b(&mut app);
    frames(&mut app, SETTLE_FRAMES);
    assert!(laid(&app, pane).1 < SHUT_PX, "collapsed");
    assert!(!left_dock_shown(&app));
    let before = viewport(&mut app);
    let before_unrounded = unrounded_width(&mut app);
    let one = device_px(&mut app);
    assert!(!app.world().get::<SplitPeek>(split).expect("a peek").is_open());

    // Rest the pointer on the collapsed edge.
    let edge = centre(&app, divider);
    cursor(&mut app, Some(edge));
    frames(&mut app, SETTLE_FRAMES);
    assert!(app.world().get::<SplitPeek>(split).expect("a peek").is_open(), "the pointer resting on the edge opens the peek");
    let floating = laid(&app, pane).1;
    assert!(floating > wide * 0.9 && floating < wide + SLACK_PX, "the pane floats as wide as a restore makes it: {floating} against {wide}");
    assert!(left_dock_shown(&app), "and its dock is there to be read and clicked");
    let during = viewport(&mut app);
    let during_unrounded = unrounded_width(&mut app);
    assert!(!viewport_moved(before, during, one), "the viewport has not moved or changed size under the peek: {before:?} to {during:?} (one device px is {one})");
    assert!((during_unrounded - before_unrounded).abs() <= one, "and its width before rounding is the same: {before_unrounded} to {during_unrounded}");
    assert!(app.world().get::<SplitSaved>(split).is_some(), "the split is still collapsed");
    assert!(app.world().resource::<ShellView>().left_hidden, "and the view still says the left panel is hidden");

    // Away from it: the pane slides back and the dock goes.
    cursor(&mut app, Some(Vec2::new(900.0, 500.0)));
    frames(&mut app, SETTLE_FRAMES);
    assert!(!app.world().get::<SplitPeek>(split).expect("a peek").is_open(), "the pointer leaving closes it");
    assert!(laid(&app, pane).1 < SHUT_PX);
    assert!(!left_dock_shown(&app));
    let after = viewport(&mut app);
    assert!(!viewport_moved(before, after, one), "and the viewport is where it was: {before:?} to {after:?} (one device px is {one})");
}
