//! **The ground, water and carved volume are redrawn when the project index says one of their files moved, and not on a clock** (#538;
//! the half-second poll of `terrain_view::sync_terrain_view`, which no constant named, retired).
//!
//! The view stat'd the three files twice a second. It reads the index's stamp of each now. The test writes the ground and then the water the way
//! an agent or a hand does (plain `std::fs`) and waits, on the wall clock, for the meshes of each to stand. The clock is frozen (a manual time step
//! of nothing) and the index's sweep is at its floor, so a view that still waited for half a second of `Time` could not pass.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::project::Project;
use bevy_dark_factory::project_index::{MIN_SWEEP, ProjectIndex};
use bevy_dark_factory::terrain_view::TerrainView;

use crate::fixtures;

/// How long a test waits for the view to follow a file.
const WAIT_SECONDS: f64 = 30.0;

const GROUND: &str = "{origin: [0.0, 0.0], cell_m: 1.0, width: 2, depth: 2, heights: [0.0, 0.0, 0.0, 0.0]}";
const WATER: &str = "{origin: [0.0, 0.0], cell_m: 1.0, width: 2, depth: 2, depth_m: [0.5, 0.0, 0.0, 0.0], settled: true, steps: 3}";

fn count(app: &mut App, view: TerrainView) -> usize {
    let world = app.world_mut();
    world.query::<&TerrainView>().iter(world).filter(|v| **v == view).count()
}

fn write(app: &App, dir: &str, file: &str, text: &str) {
    let dir = app.world().resource::<Project>().blueprint_dir.join(dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    std::fs::write(dir.join(file), text).unwrap_or_else(|e| panic!("{dir:?}/{file}: {e}"));
}

fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    let started = Instant::now();
    while started.elapsed().as_secs_f64() < WAIT_SECONDS {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("never happened within {WAIT_SECONDS} s: {what}");
}

#[test]
fn a_ground_and_a_water_file_written_outside_the_editor_are_drawn_with_the_clock_stopped() {
    let root = fixtures::Fixture::new(&format!("terrain_follows_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    for _ in 0..4 {
        app.update();
    }
    app.world().resource::<ProjectIndex>().set_sweep(MIN_SWEEP);
    assert_eq!(count(&mut app, TerrainView::Ground), 0, "no ground is drawn before the file is written");

    write(&app, "terrain", "m.yaml", GROUND);
    until(&mut app, "the ground stands", |a| count(a, TerrainView::Ground) == 1);
    assert_eq!(count(&mut app, TerrainView::Water), 0, "and no water");

    write(&app, "water", "m.field.yaml", WATER);
    until(&mut app, "the water stands on the ground", |a| count(a, TerrainView::Water) == 1);
}
