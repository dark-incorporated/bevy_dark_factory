//! **A Shift-drag on the map selects what it covers** (UI-52, #505), and Shift-click still adds one.
//!
//! The only way to grow the selection used to be Shift-click, one piece at a time. The removal tool's
//! rectangle already existed and already took every piece a box covers, so this is that rectangle's
//! rule pointed at the selection instead of the delete — and the rule is the *selection's*, so a
//! marquee writes no file, dispatches no operation and adds no undo step (`selection.rs` says a
//! selection is where a person is looking, not a fact about the map).
//!
//! Proved here:
//!
//! 1. **A box over four pieces selects four**, and nothing reaches the log or the undo stack.
//! 2. **A click after it replaces the set** — the `follow` rule this must not break.
//! 3. **A piece the outliner's eye has hidden is not in a marquee's set**, matching `pick_at_seen`.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_dark_factory::editor::EditorState;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::SHIFT_KEYS;
use bevy_dark_factory::ops::{self, Entry};
use bevy_dark_factory::outliner::Eyes;
use bevy_dark_factory::project::{OpenMap, Project};
use bevy_dark_factory::selection::Selection;
use bevy_dark_factory::view::MainCamera;

use crate::fixtures;

/// One frame's worth of person: keys held, the pointer, and a mouse press or release.
#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

fn drive(
    mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let frame = script.next.take();
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let Some(frame) = frame else {
        return;
    };
    for key in &frame.keys {
        keys.press(*key);
    }
    script.held = frame.keys;
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(frame.pointer);
    }
    match frame.mouse {
        Some(true) => mouse.press(MouseButton::Left),
        Some(false) => mouse.release(MouseButton::Left),
        None => {}
    }
}

fn step(app: &mut App, frame: Frame) {
    app.world_mut().resource_mut::<Script>().next = Some(frame);
    app.update();
}

fn pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let world = Vec3::new(origin.0 + at.0, 0.0, origin.2 + at.1);
    let mut cameras = app
        .world_mut()
        .query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras
        .single(app.world())
        .expect("the editor has one main camera");
    camera
        .world_to_viewport(transform, world)
        .unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
}

/// **Four pieces on one row**, so a box can take all four and an eye can take one out.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place_as("wall@0", "site/wall", (0.0, 0.0))
        .place_as("wall@1", "site/wall", (2.0, 0.0))
        .place_as("wall@2", "site/wall", (4.0, 0.0))
        .place_as("wall@3", "site/wall", (6.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn selected(app: &App) -> Vec<String> {
    app.world().resource::<Selection>().ids().to_vec()
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

/// **A Shift-drag from one spot to another**: the press holds Shift, the release lets go of it.
fn shift_drag(app: &mut App, from: Vec2, to: Vec2) {
    step(
        app,
        Frame {
            keys: vec![SHIFT_KEYS[0]],
            pointer: Some(from),
            mouse: Some(true),
            ..default()
        },
    );
    step(
        app,
        Frame {
            keys: vec![SHIFT_KEYS[0]],
            pointer: Some(to),
            ..default()
        },
    );
    step(
        app,
        Frame {
            pointer: Some(to),
            mouse: Some(false),
            ..default()
        },
    );
    step(app, Frame::default());
}

/// **A box over four pieces selects four, primary first, and records nothing.**
#[test]
fn a_marquee_over_four_pieces_selects_four_and_records_nothing() {
    let mut app = editor("marquee_four");
    assert!(selected(&app).is_empty(), "nothing is selected before the drag");
    let before_log = log(&app).len();
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), 0);

    // A box that covers the row: from below-left of the first to above-right of the last.
    let from = pixel_over(&mut app, (-1.0, -1.0));
    let to = pixel_over(&mut app, (7.0, 1.0));
    shift_drag(&mut app, from, to);

    assert_eq!(
        selected(&app),
        ["wall@0", "wall@1", "wall@2", "wall@3"],
        "the box selected every piece it covered, primary first"
    );
    assert_eq!(
        log(&app).len(),
        before_log,
        "a selection is not an operation: the log did not move"
    );
    assert_eq!(
        app.world().resource::<EditorState>().undo_depth(),
        0,
        "and there is no Cmd+Z entry for it — selection.rs's rule"
    );
}

/// **A click after the marquee replaces the set**, which is the rule this must not break.
#[test]
fn a_click_after_a_marquee_replaces_the_set() {
    let mut app = editor("marquee_then_click");
    let from = pixel_over(&mut app, (-1.0, -1.0));
    let to = pixel_over(&mut app, (7.0, 1.0));
    shift_drag(&mut app, from, to);
    assert_eq!(selected(&app).len(), 4);

    // **The rule is `Selection::follow`'s**: a lock that moves to one piece replaces the set. The
    // lock is moved the way a click's target and the toast's Show button move it —
    // `editor::lock_on_placement`, the one function that says "the target is this piece" — because a
    // plain click on a piece also *carries* it (`drive_move`), and this test is about the selection
    // the lock anchors, not about what the carry does with the press.
    assert!(
        bevy_dark_factory::editor::lock_on_placement(app.world_mut(), "wall@2"),
        "the third piece is on the map"
    );
    app.update();
    assert_eq!(
        selected(&app),
        ["wall@2"],
        "one piece is selected, and it does not leave the box's pieces beside it"
    );
}

/// **A piece the outliner's eye has hidden is not in a marquee's set**, the rule `pick_at_seen` holds.
#[test]
fn a_hidden_piece_is_not_in_a_marquees_set() {
    let mut app = editor("marquee_hidden");
    {
        let mut eyes = app.world_mut().resource_mut::<Eyes>();
        eyes.set("wall@1", true);
    }
    // Let the mirror put the hidden set where the pick reads it.
    for _ in 0..3 {
        app.update();
    }

    let from = pixel_over(&mut app, (-1.0, -1.0));
    let to = pixel_over(&mut app, (7.0, 1.0));
    shift_drag(&mut app, from, to);

    let got = selected(&app);
    assert_eq!(
        got,
        ["wall@0", "wall@2", "wall@3"],
        "the piece the eye took out is not swept up by a box over it: {got:?}"
    );
}
