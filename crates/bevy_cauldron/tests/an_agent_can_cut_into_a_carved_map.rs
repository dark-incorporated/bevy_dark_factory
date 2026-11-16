//! **A carved map's interior is what a hidden terrain shows** (#472), proved with the mesher.
//!
//! A carved map's rock is **one solid**: the hill's outer skin and the carved rooms inside it are one
//! surface. So hiding the terrain must **cut** the solid rather than despawn it — a cull of its front faces
//! leaves its *inner* surface drawn, which is the crypt's floor, its walls, its vault and the sides of the
//! columns standing in it. That is "the carved void, and nothing else, from the same camera", with no second
//! mesh and no cut plane's magic height.
//!
//! **Needs `isomesh`**, the mesher a carved volume is sampled through (off by default, as
//! `volume_wears_its_cover.rs` is gated for the same reason):
//! `cargo test -p bevy_cauldron --features isomesh --test editor an_agent_can_cut_into_a_carved_map::`.

#![cfg(feature = "isomesh")]

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_cauldron::project::Project;
use bevy_cauldron::terrain_view::{Surface, TerrainView};
use serde_json::{Value, json};

use crate::fixtures;

const GROUND: &str = "{origin: [0.0, 0.0], cell_m: 1.0, width: 8, depth: 8, heights: [10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0]}";

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: Value) -> Result<Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut().run_system_with(id, Some(params)).map_err(|e| format!("{method}: {e:?}"))?.map_err(|e| format!("{method}: {e:?}"))
}

fn apply(app: &mut App, operation: &str, parameters: Value) -> Value {
    call(app, "grimoire/operation_apply", json!({"operation": operation, "parameters": parameters, "dry_run": false}))
        .unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

fn write(app: &App, dir: &str, file: &str, text: &str) {
    let dir = app.world().resource::<Project>().grimoire_dir.join(dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    std::fs::write(dir.join(file), text).unwrap_or_else(|e| panic!("{dir:?}/{file}: {e}"));
}

fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < 8.0 {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(40));
    }
    panic!("never saw: {what}");
}

fn drawn(app: &mut App, surface: Surface) -> Vec<Entity> {
    let world = app.world_mut();
    let mut found: Vec<Entity> = world.query::<(Entity, &Surface)>().iter(world).filter(|(_, s)| **s == surface).map(|(e, _)| e).collect();
    found.sort();
    found
}

fn visible(app: &App, entity: Entity) -> bool {
    app.world().entity(entity).get::<Visibility>().is_some_and(|v| *v != Visibility::Hidden)
}

/// **A lair in miniature**: 10 m of rock over an 8 x 8 m ground, one room carved out of it at 4 m.
fn a_lair(tag: &str) -> App {
    let mut app = editor(tag);
    write(&app, "terrain", "m.yaml", GROUND);
    until(&mut app, "the ground drawn", |a| !drawn(a, Surface::Terrain).is_empty());
    let rock = json!({"shape": {"Box": {"center": [4.0, 8.0, 4.0], "half_m": [4.0, 6.0, 4.0]}}, "mode": "Add"});
    let room = json!({"shape": {"Box": {"center": [3.0, 4.0, 3.0], "half_m": [2.0, 1.5, 2.0]}}, "mode": "Carve"});
    assert_eq!(apply(&mut app, "volume_edit", json!({"map": "m", "edit": rock}))["outcome"], "applied");
    assert_eq!(apply(&mut app, "volume_edit", json!({"map": "m", "edit": room}))["outcome"], "applied");
    until(&mut app, "the carved ground drawn", |a| !drawn(a, Surface::Volume).is_empty());
    app
}

#[test]
fn hiding_the_terrain_leaves_the_carved_rooms_drawn() {
    let mut app = a_lair("lair_rooms");
    let chunks = drawn(&mut app, Surface::Volume);
    assert!(!chunks.is_empty(), "a carved map draws its chunks");
    assert!(chunks.iter().all(|e| visible(&app, *e)), "and every chunk is visible to begin with");

    let answer = call(&mut app, "grimoire/surface_hide", json!({"hide": ["terrain"]})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(answer["hidden"], json!(["terrain"]), "{answer}");
    app.update();
    assert!(drawn(&mut app, Surface::Volume).iter().all(|e| visible(&app, *e)), "the carved ground is the picture");
}

#[test]
fn the_cut_is_a_cull_so_the_rooms_are_the_same_meshes_drawn_from_inside() {
    let mut app = a_lair("lair_cut");
    let chunks: Vec<(Entity, Handle<Mesh>)> = {
        let world = app.world_mut();
        world.query::<(Entity, &Surface, &Mesh3d)>().iter(world).filter(|(_, s, _)| **s == Surface::Volume).map(|(e, _, m)| (e, m.0.clone())).collect()
    };
    let meshes: Vec<Mesh> = {
        let assets = app.world().resource::<Assets<Mesh>>();
        chunks.iter().map(|(_, h)| assets.get(h).cloned().expect("the chunk's mesh is there")).collect()
    };
    call(&mut app, "grimoire/surface_hide", json!({"hide": ["terrain"]})).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    // **The mesh is untouched**: the cut is the material's cull mode, so the same chunks draw the interior
    // rather than a second mesh standing for the inside.
    {
        let assets = app.world().resource::<Assets<Mesh>>();
        for ((_, handle), before) in chunks.iter().zip(&meshes) {
            assert_eq!(assets.get(handle), Some(before), "a hidden terrain rebuilt no chunk");
        }
    }
    assert!(drawn(&mut app, Surface::Terrain).iter().all(|e| !visible(&app, *e)), "the hill's skin is out of the picture");
}

#[test]
fn hiding_the_volume_takes_the_carved_ground_out_too() {
    let mut app = a_lair("lair_volume_hidden");
    call(&mut app, "grimoire/surface_hide", json!({"hide": ["volume"]})).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    // **Both hidden is a map with no ground at all**, which is a picture of the pieces standing in it.
    assert!(drawn(&mut app, Surface::Volume).iter().all(|e| !visible(&app, *e)), "the carved ground is out of the picture");
    assert!(drawn(&mut app, Surface::Terrain).iter().all(|e| visible(&app, *e)), "and the heightfield ground is drawn");
}

#[test]
fn the_eye_survives_a_carve_after_it_was_set() {
    let mut app = a_lair("lair_rebuild");
    call(&mut app, "grimoire/surface_hide", json!({"hide": ["terrain"]})).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    // **A carve after the view was set** respawns the chunks; the eye must still hold, or a still taken
    // after a correction would show the skin again with no call having changed it.
    let deeper = json!({"shape": {"Box": {"center": [6.0, 5.0, 6.0], "half_m": [1.0, 1.0, 1.0]}}, "mode": "Carve"});
    assert_eq!(apply(&mut app, "volume_edit", json!({"map": "m", "edit": deeper}))["outcome"], "applied");
    until(&mut app, "the cut held across the rebuild", |a| {
        !drawn(a, Surface::Volume).is_empty() && drawn(a, Surface::Terrain).iter().all(|e| !visible(a, *e))
    });
}

/// **`Surface` was added beside `TerrainView`**, so a test that asks about the drawing's kinds answers the
/// same as before: a carved chunk is the volume surface, the heightfield is the terrain, water is water.
#[test]
fn the_drawing_still_names_its_kinds() {
    let mut app = a_lair("lair_kinds");
    let world = app.world_mut();
    let kinds: Vec<(TerrainView, Surface)> = world.query::<(&TerrainView, &Surface)>().iter(world).map(|(v, s)| (*v, *s)).collect();
    assert!(!kinds.is_empty(), "the drawing is made of the surfaces it names");
    for (view, surface) in kinds {
        let same = match view {
            TerrainView::Ground => matches!(surface, Surface::Terrain),
            TerrainView::Volume => matches!(surface, Surface::Volume),
            TerrainView::Water => matches!(surface, Surface::Water),
        };
        assert!(same, "a drawn kind and its surface name the same thing: {view:?} {surface:?}");
    }
}
