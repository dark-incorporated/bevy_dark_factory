//! **A click that never moved is one piece, whatever the map did underneath it** (#494's cluster).
//!
//! Found on the gate that merges the panes cluster: `the_editor_follows_an_agents_change`'s
//! `a_person_placing_a_piece_leaves_the_camera_where_it_was` began placing **two** pieces from one click.
//! Nothing was placing twice. The panes cluster narrowed the shell's divider from 24 to 12 px
//! (`chrome::DIVIDER_W_PX`), which moves the map camera's viewport **1 px** between the frame of the press
//! and the frame of the release — and `drive_place` decided *drag or click* from the **map** point the
//! cursor resolved to, so a cursor that never moved went from `(20.0, 18.0)` to `(19.9, 18.1)`, 0.14 m
//! apart. Both snap to the grid, a metre apart, so `CLICK_EPS` saw a drag and `box_fill_between` filled the
//! two cells between them.
//!
//! # What is held here
//!
//! The press's **screen** point is remembered, and a release that has not travelled [`DRAG_SLOP_PX`] of
//! surface pixels is a click however the map moved. Two tests:
//!
//! 1. **A viewport that shifts under a stationary hand places one piece.** The camera's viewport is moved
//!    between the press and the release by a few pixels, which is the whole mechanism above, and the
//!    placed count must stay at one.
//! 2. **A hand that really does sweep places a box**, so the guard cannot be a silent brake on the fill
//!    gesture: the same two frames with the pointer a hundred pixels apart fill every cell between.

use bevy::input::{ButtonInput, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};

use bevy_dark_factory::editor::EditorState;
use bevy_dark_factory::harness;
use bevy_dark_factory::project::{OpenMap, Project};
use bevy_dark_factory::view::MainCamera;

use crate::fixtures;

/// **The window the pointer's pixels are in**, physical px: the shape a real editor runs at, so a
/// viewport nudge of a few pixels is what a divider's own width change makes.
const WINDOW: (u32, u32) = (1280, 800);

/// How far the camera's viewport is nudged between the press and the release, in surface pixels: the
/// 1 px the panes cluster's divider moved it, plus the rounding a different host adds.
const NUDGE_PX: i32 = 1;

/// Frames the editor needs before a gesture, and after one, to settle.
const SETTLE_FRAMES: usize = 6;

/// One frame's worth of person: the pointer and a mouse press or release.
#[derive(Default)]
struct Frame {
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
}

fn drive(mut script: ResMut<Script>, mut mouse: ResMut<ButtonInput<MouseButton>>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    let Some(frame) = script.next.take() else {
        return;
    };
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

/// **A 64 m map with one piece already on it**, so a click's own piece is the only new row.
fn room(tag: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .side(64.0)
        .descriptor("bed", "alpha")
        .place("bed", (-28.0, -28.0))
        .build("m")
}

fn editor(tag: &str) -> App {
    let root = room(tag);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    // The piece is armed as a palette click arms it: the click is the only thing under test.
    let index = app
        .world()
        .resource::<Project>()
        .library
        .descriptors
        .iter()
        .position(|d| d.id == "bed")
        .expect("the fixture defines bed");
    app.world_mut().resource_mut::<EditorState>().brush = Some(index);
    app.update();
    app
}

/// **The pixel the world point is drawn at**, so the test points at a place on the map rather than at a
/// number it guessed.
fn pixel_over(app: &mut App, at: (f32, f32)) -> Vec2 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    let world = Vec3::new(origin.0 + at.0, 0.0, origin.2 + at.1);
    let mut cameras = app.world_mut().query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
    let (camera, transform) = cameras.single(app.world()).expect("one main camera");
    camera.world_to_viewport(transform, world).unwrap_or_else(|e| panic!("the spot is not on screen: {e:?}"))
}

/// **Move the map camera's viewport by `by` surface pixels**, which is what a dock's width change does to
/// it — the mechanism the panes cluster's divider tripped.
fn nudge_the_viewport(app: &mut App, by: IVec2) {
    let mut cameras = app.world_mut().query_filtered::<&mut Camera, With<MainCamera>>();
    let mut camera = cameras.single_mut(app.world_mut()).expect("one main camera");
    let viewport = camera.viewport.clone().expect("the map camera draws in a viewport");
    let at = viewport.physical_position.as_ivec2() + by;
    camera.viewport = Some(bevy::camera::Viewport {
        physical_position: at.as_uvec2(),
        ..viewport
    });
}

/// Every placement on the map but the one the fixture put there.
fn placed(app: &mut App) -> usize {
    app.world()
        .resource::<OpenMap>()
        .map
        .placements
        .iter()
        .filter(|p| p.id != "bed@0")
        .count()
}

/// **A press and a release that never moved on screen is one piece, even when the viewport moves under
/// the hand between them.**
///
/// Failed before the fix: the panes cluster's narrower divider moved the camera's viewport a pixel between
/// the two frames, the same stationary cursor resolved to a different cell, and the release filled the two
/// cells between — `left: 2, right: 1` on this file's own sibling test.
#[test]
fn a_click_that_never_moved_places_one_piece_though_the_viewport_shifts() {
    let mut app = editor("place_one");
    let at = pixel_over(&mut app, (20.0, 18.0));
    step(&mut app, Frame { pointer: Some(at), mouse: Some(true) });
    // The viewport moves under the stationary hand, as a dock's width change makes it.
    nudge_the_viewport(&mut app, IVec2::new(NUDGE_PX, 0));
    step(&mut app, Frame { pointer: Some(at), mouse: Some(false) });
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert_eq!(
        placed(&mut app),
        1,
        "a click that never moved on screen placed more than one piece: the release read the map under it \
         as a drag because the viewport had moved a pixel"
    );
}

/// **A hand that really sweeps still fills**, so the guard is not a brake on the fill gesture.
#[test]
fn a_drag_across_the_map_still_fills_a_box() {
    let mut app = editor("place_box");
    // **Thirty metres apart, because the map camera sees the whole 64 m room**: measured, 3 m of map is
    // 14 surface px here, so a sweep has to cross most of the room to be a sweep.
    let from = pixel_over(&mut app, (-18.0, 0.0));
    let to = pixel_over(&mut app, (12.0, 0.0));
    assert!(
        (to.x - from.x).abs() > 100.0,
        "the two points are {:.0} px apart, which is not a swept hand",
        (to.x - from.x).abs()
    );
    step(&mut app, Frame { pointer: Some(from), mouse: Some(true) });
    step(&mut app, Frame { pointer: Some(to), mouse: Some(false) });
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert!(
        placed(&mut app) >= 2,
        "a hand that swept across the map filled one cell: the drag to fill was taken for a click"
    );
}
