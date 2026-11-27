//! **Panning cannot walk the map's drawn geometry out of the camera's depth range**
//! (`feedback-1794921159460`).
//!
//! > There's something cutting off the edge of the map as I use WASD to navigate. Maybe camera not in
//! > right place or incorrect culling?
//!
//! The Map camera is orthographic with a depth range measured each frame from
//! `camera.translation`: `view::depth_range` walks the map's `bounds` against the camera's forward
//! and returns `(nearest - slack, farthest + slack)`. An orthographic camera draws what lies between
//! `near` and `far` along its forward, and cuts the rest at a straight edge — the shape B-010 was.
//!
//! # Why the pan shows it
//!
//! `depth_range` measures the **bounds box**. The map draws more than that box: the terrain heightfield
//! is authored on its own grid, and `liches_lair`'s covers `0..80 m` in x and z while the map's bounds
//! are `80 x 40 x 80` centred on the origin (`-40..40`). Three quarters of the drawn ground is thus
//! outside the box the depth range is measured from, and the pan system moves the focus with no bound
//! at all — so a WASD walk puts the camera past the ground's far corner, which falls *behind* the near
//! plane and disappears along a straight line.
//!
//! The capture's own numbers: `cell [47, 44]` on `liches_lair`, the camera at `(73.08, 32, 73.05)`,
//! forward `(-0.577, -0.577, -0.577)`, its terrain `41 x 41` cells at `2.0 m` from origin `(0, 0)`.
//! At that pose the ground's `(80, 20, 80)` corner is at depth `-1.1 m`: behind the camera.
//!
//! # What this proves
//!
//! The corners of **everything the map draws** — its bounds box and its terrain patch — lie inside
//! `(near, far)` at four pan extremes, at `MIN_ZOOM` and at the map's own `max_zoom`, on a `500 x 40 x
//! 500` map (`deadite_caverns`) and an `80 x 40 x 80` one (`liches_lair`).

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::{PrimaryWindow, Window};
use bevy_dark_factory::view::{MainCamera, Rig};
use blueprint_model::heightfield::Heightfield;

use crate::fixtures;

/// One frame of the stepped clock, in seconds: 60 Hz.
const FRAME_SECS: f32 = 1.0 / 60.0;

/// **How many frames a key is held for one extreme.** At the pan's rate (`16/18` of what is on screen
/// per second) and [`MAX_STAGE_HEIGHT_M`] this walks the focus far past the map's edge in every
/// direction — the condition the capture shows.
const HOLD_FRAMES: usize = 240;

/// The tallest view this test asks for, in metres of map the viewport's height shows: `view::MAX_ZOOM`.
const MAX_STAGE_HEIGHT_M: f32 = 80.0;

/// **The terrain's own grid**, as `liches_lair`'s file states it: 41 x 41 cells of 2 m from the origin,
/// so the drawn ground covers `0..80 m` — three quarters of it outside an 80 m map's `-40..40` bounds.
const TERRAIN_ORIGIN: (f32, f32) = (0.0, 0.0);
const TERRAIN_CELL_M: f32 = 2.0;
const TERRAIN_CELLS: usize = 41;

/// How tall the test's ground stands, metres: high enough that the capture's own camera pose puts the
/// patch's far corner behind the near plane, which is the cut the report shows.
const TERRAIN_HEIGHT_M: f32 = 20.0;

fn send(app: &mut App, key: KeyCode, state: ButtonState) {
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

/// **An editor on a map of `side` metres with a terrain patch on its own grid**, a stepped clock, and a
/// window for the keys to reach.
fn editor(tag: &str, side: f32) -> App {
    let root = fixtures::Fixture::new(&format!("pan_depth_{tag}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .side(side)
        .build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // The ground is drawn from the project's file, the same way a game reads it.
    let project = app.world().resource::<bevy_dark_factory::project::Project>().clone();
    std::fs::create_dir_all(project.blueprint_dir.join("terrain")).unwrap();
    let ground = Heightfield::flat(TERRAIN_ORIGIN, TERRAIN_CELL_M, TERRAIN_CELLS, TERRAIN_CELLS, TERRAIN_HEIGHT_M);
    std::fs::write(
        project.blueprint_dir.join("terrain/m.yaml"),
        blueprint_model::yaml::to_string(&ground).unwrap(),
    )
    .unwrap();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME_SECS)));
    // The terrain is read off the disk on its own beat: give it the half second it waits.
    for _ in 0..40 {
        app.update();
    }
    let drawn = app.world_mut().query::<&bevy_dark_factory::terrain_view::TerrainView>().iter(app.world()).count();
    assert!(drawn > 0, "the fixture's ground was never drawn, so this test would prove nothing");
    app
}

/// Every point the map draws, as world positions: the eight corners of its bounds box, and the four
/// corners of its terrain patch at the top and the bottom of the ground's own height.
fn drawn_points(app: &mut App) -> Vec<Vec3> {
    let open = app.world().resource::<bevy_dark_factory::project::OpenMap>();
    let (centre, size) = bevy_dark_factory::editor::map_bounds(open);
    let half = size * 0.5;
    let mut out = Vec::new();
    for sx in [-1.0, 1.0] {
        for sy in [-1.0, 1.0] {
            for sz in [-1.0, 1.0] {
                out.push(centre + half * Vec3::new(sx, sy, sz));
            }
        }
    }
    // The terrain patch as its own file states it: from its origin over `cells * cell_m` metres.
    let span = TERRAIN_CELLS as f32 * TERRAIN_CELL_M;
    for (x, z) in [(0.0, 0.0), (span, 0.0), (0.0, span), (span, span)] {
        for y in [0.0, TERRAIN_HEIGHT_M] {
            out.push(Vec3::new(TERRAIN_ORIGIN.0 + x, y, TERRAIN_ORIGIN.1 + z));
        }
    }
    out
}

/// The camera's own transform and projection.
fn camera(app: &mut App) -> (GlobalTransform, Projection) {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(&GlobalTransform, &Projection), With<MainCamera>>();
    let (tf, proj) = q.single(world).expect("the editor has one main camera");
    (*tf, proj.clone())
}

/// **Where a world point falls along the camera's view axis**, in metres from the camera: positive in
/// front of it, which is the axis `near` and `far` are measured on.
fn depth_of(tf: &GlobalTransform, point: Vec3) -> f32 {
    (point - tf.translation()).dot(*tf.forward())
}

/// The points outside `(near, far)`, with their depths, for the message.
fn outside(app: &mut App) -> (Vec<(Vec3, f32)>, f32, f32) {
    let (tf, proj) = camera(app);
    let Projection::Orthographic(ortho) = proj else {
        panic!("the Map's camera is orthographic; this measures an orthographic depth range")
    };
    let points = drawn_points(app);
    let out: Vec<(Vec3, f32)> = points
        .iter()
        .map(|p| (*p, depth_of(&tf, *p)))
        .filter(|(_, d)| *d < ortho.near || *d > ortho.far)
        .collect();
    (out, ortho.near, ortho.far)
}

/// Hold `key` for [`HOLD_FRAMES`], then release it and let the view settle.
fn hold(app: &mut App, key: KeyCode) {
    send(app, key, ButtonState::Pressed);
    for _ in 0..HOLD_FRAMES {
        app.update();
    }
    send(app, key, ButtonState::Released);
    for _ in 0..2 {
        app.update();
    }
}

fn zoom_to(app: &mut App, height: f32) {
    app.world_mut().resource_mut::<Rig>().height = height;
    for _ in 0..2 {
        app.update();
    }
}

/// **The capture's own pose, before any key**: `cell [47, 44]`, camera `(73.08, 32, 73.05)`, forward
/// `(-0.577, -0.577, -0.577)`.
fn at_the_reported_pose(app: &mut App) {
    {
        let mut rig = app.world_mut().resource_mut::<Rig>();
        rig.focus = Vec3::new(61.076, 20.0, 61.052);
        rig.yaw = 0.0;
        rig.goal_yaw = 0.0;
        rig.height = 18.0;
    }
    for _ in 0..2 {
        app.update();
    }
}

/// **The reported pose already cuts the ground**, which is what the capture shows: the far corner of
/// `liches_lair`'s terrain patch stands behind the near plane.
#[test]
fn the_reported_pose_draws_the_whole_ground() {
    let mut app = editor("reported", 80.0);
    at_the_reported_pose(&mut app);
    let (out, near, far) = outside(&mut app);
    let (tf, _) = camera(&mut app);
    assert!(
        out.is_empty(),
        "at the reported pose (camera ({:.2}, {:.2}, {:.2})) the camera draws depths {near:.1}..{far:.1} m, \
         and these points the map draws fall outside it — that is the straight edge the report shows: {:?}",
        tf.translation().x,
        tf.translation().y,
        tf.translation().z,
        out,
    );
}

/// The two zooms every direction is panned at, by name.
const ZOOMS: [&str; 2] = ["min_zoom", "max_zoom"];

/// The four keys and a held diagonal, by name.
const DIRECTIONS: [(&str, &[KeyCode]); 5] = [
    ("forward", &[KeyCode::KeyW]),
    ("back", &[KeyCode::KeyS]),
    ("left", &[KeyCode::KeyA]),
    ("right", &[KeyCode::KeyD]),
    ("forward_right", &[KeyCode::KeyW, KeyCode::KeyD]),
];

/// **`liches_lair`: 80 x 40 x 80 m**, the map the capture was taken on.
const SMALL: (&str, f32) = ("liches_lair", 80.0);
/// **`deadite_caverns`: 500 x 40 x 500 m**, the larger of the two maps the acceptance names.
const LARGE: (&str, f32) = ("deadite_caverns", 500.0);

/// **One direction, on one map, at one zoom**, in a fresh editor: each (zoom, direction) is its own test so nextest runs
/// the ten of a map side by side (#672) — panning forward says nothing about panning back.
fn one_extreme_holds((tag, side): (&str, f32), height_name: &str, name: &str) {
    let want = match height_name {
        "min_zoom" => MIN_ZOOM_M,
        "max_zoom" => max_zoom_m(side),
        other => panic!("`{other}` is not one of {ZOOMS:?}"),
    };
    let keys = DIRECTIONS
        .iter()
        .find(|(direction, _)| *direction == name)
        .map(|(_, keys)| *keys)
        .unwrap_or_else(|| panic!("`{name}` is not a row of DIRECTIONS"));
    let mut app = editor(&format!("{tag}_{height_name}_{name}"), side);
    zoom_to(&mut app, want);
    for key in keys {
        hold(&mut app, *key);
    }
    let (out, near, far) = outside(&mut app);
    let focus = app.world().resource::<Rig>().focus;
    assert!(
        out.is_empty(),
        "on a {side} m map at {height_name} ({want:.1} m), panning {name} to focus ({:.1}, {:.1}) left \
         the camera drawing depths {near:.1}..{far:.1} m, and these points the map draws fall outside \
         it — that is the straight edge the report shows: {out:?}",
        focus.x,
        focus.z,
    );
}

/// `view::MIN_ZOOM`: the wheel's floor, in metres of map the viewport's height shows.
const MIN_ZOOM_M: f32 = 0.25;

/// **This map's ceiling**: `view::max_zoom` on a square viewport, which `MAX_ZOOM` floors and a large
/// map raises until all of it can be seen at any yaw.
fn max_zoom_m(side: f32) -> f32 {
    MAX_STAGE_HEIGHT_M.max(1.25 * side.hypot(side))
}

/// **One `#[test]` per (map, zoom, direction)**, and [`EXTREMES`] listing them in the order written, so
/// [`every_map_zoom_and_direction_is_its_own_test`] can say that none of the twenty was dropped.
macro_rules! one_test_per_extreme {
    ($($test:ident: $map:ident, $zoom:literal, $direction:literal;)*) => {
        const EXTREMES: &[(&str, &str, &str)] = &[$(($map.0, $zoom, $direction)),*];
        $(
            #[test]
            fn $test() {
                one_extreme_holds($map, $zoom, $direction);
            }
        )*
    };
}

one_test_per_extreme! {
    a_small_map_at_min_zoom_panned_forward: SMALL, "min_zoom", "forward";
    a_small_map_at_min_zoom_panned_back: SMALL, "min_zoom", "back";
    a_small_map_at_min_zoom_panned_left: SMALL, "min_zoom", "left";
    a_small_map_at_min_zoom_panned_right: SMALL, "min_zoom", "right";
    a_small_map_at_min_zoom_panned_forward_right: SMALL, "min_zoom", "forward_right";
    a_small_map_at_max_zoom_panned_forward: SMALL, "max_zoom", "forward";
    a_small_map_at_max_zoom_panned_back: SMALL, "max_zoom", "back";
    a_small_map_at_max_zoom_panned_left: SMALL, "max_zoom", "left";
    a_small_map_at_max_zoom_panned_right: SMALL, "max_zoom", "right";
    a_small_map_at_max_zoom_panned_forward_right: SMALL, "max_zoom", "forward_right";
    a_large_map_at_min_zoom_panned_forward: LARGE, "min_zoom", "forward";
    a_large_map_at_min_zoom_panned_back: LARGE, "min_zoom", "back";
    a_large_map_at_min_zoom_panned_left: LARGE, "min_zoom", "left";
    a_large_map_at_min_zoom_panned_right: LARGE, "min_zoom", "right";
    a_large_map_at_min_zoom_panned_forward_right: LARGE, "min_zoom", "forward_right";
    a_large_map_at_max_zoom_panned_forward: LARGE, "max_zoom", "forward";
    a_large_map_at_max_zoom_panned_back: LARGE, "max_zoom", "back";
    a_large_map_at_max_zoom_panned_left: LARGE, "max_zoom", "left";
    a_large_map_at_max_zoom_panned_right: LARGE, "max_zoom", "right";
    a_large_map_at_max_zoom_panned_forward_right: LARGE, "max_zoom", "forward_right";
}

/// **Both maps × both zooms × every direction has its own test**, in that order, none twice: a direction added to
/// [`DIRECTIONS`] without a test, or a test dropped from the list above, fails here rather than going unpanned.
#[test]
fn every_map_zoom_and_direction_is_its_own_test() {
    let want: Vec<(&str, &str, &str)> = [SMALL.0, LARGE.0]
        .into_iter()
        .flat_map(|map| ZOOMS.into_iter().flat_map(move |zoom| DIRECTIONS.iter().map(move |(direction, _)| (map, zoom, *direction))))
        .collect();
    assert_eq!(EXTREMES, want.as_slice(), "one test per (map, zoom, direction)");
}
