//! **An agent can see inside a carved map** (#472): the eye that hides a map's own surfaces.
//!
//! A map whose rooms are `volume_edit` carves draws the rock they are cut into as the terrain's ground, so
//! the carved void is inside it and **every still from any outside angle is the hill's skin**. An agent
//! building a 26-entry lair took seven stills and not one let it judge a room; it voxel-sampled the YAML by
//! hand instead, six times.
//!
//! # What is proved here, and what needs a mesher
//!
//! This file proves the door and the state: the command exists and answers, an empty `hide` changes nothing,
//! a typo is refused naming the list, `replace: false` adds, the terrain's own entity is hidden, and **no
//! map data changes** — `blueprint_state.map.dirty` stays `false` and no line joins `oplog.jsonl`, which is
//! the reason this is a view verb and not an operation.
//!
//! **The carved ground itself needs `isomesh`** (the mesher a volume is sampled through, off by default), so
//! the tests that count chunks and check they stay drawn while the skin is cut are
//! `an_agent_can_cut_into_a_carved_map.rs`, gated on that feature.

use std::time::Duration;

use bevy::prelude::*;
use bevy_dark_factory::outliner::{Eyes, HIDABLE};
use bevy_dark_factory::project::{OpenMap, Project};
use bevy_dark_factory::terrain_view::Surface;
use serde_json::{Value, json};

use crate::fixtures;
use crate::settle_support::Frames;

/// The pause between two frames of a wait here.
const PAUSE: Duration = Duration::from_millis(40);

/// **Frames a wait steps before it gives up** (#644): a count, not a wall-clock window, so a starved host does not cut it short;
/// at [`PAUSE`] apart they cover at least 6 s.
const SEEN_WITHIN: Frames = Frames::covering(Duration::from_secs(6), PAUSE);

/// **Frames the eye must have held through a rebuild before it counts**: at [`PAUSE`] apart they cover 0.6 s.
const HELD_FOR: Frames = Frames::covering(Duration::from_millis(600), PAUSE);

const GROUND: &str = "{origin: [0.0, 0.0], cell_m: 1.0, width: 8, depth: 8, heights: [10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0]}";

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
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

fn write(app: &App, dir: &str, file: &str, text: &str) {
    let dir = app.world().resource::<Project>().blueprint_dir.join(dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    std::fs::write(dir.join(file), text).unwrap_or_else(|e| panic!("{dir:?}/{file}: {e}"));
}

fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    for _ in 0..SEEN_WITHIN.0 {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(PAUSE);
    }
    panic!("never saw: {what} (waited {SEEN_WITHIN:?})");
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

/// **The log's length**, so a call that wrote a line is told apart from one that did not.
fn oplog_lines(app: &App) -> usize {
    let root = app.world().resource::<Project>().root.clone();
    std::fs::read_to_string(bevy_dark_factory::ops::oplog_path_at(&root)).map_or(0, |t| t.lines().count())
}

/// **A map with its ground drawn**, and nothing carved: the terrain's own entity is the subject.
fn a_hill(tag: &str) -> App {
    let mut app = editor(tag);
    write(&app, "terrain", "m.yaml", GROUND);
    until(&mut app, "the ground drawn", |a| !drawn(a, Surface::Terrain).is_empty());
    app
}

#[test]
fn hiding_the_ground_is_one_call_and_putting_it_back_is_another() {
    let mut app = a_hill("surface_ground");
    let ground = drawn(&mut app, Surface::Terrain);
    assert!(!ground.is_empty() && ground.iter().all(|e| visible(&app, *e)), "the ground is drawn to begin with");

    let answer = call(&mut app, "blueprint/surface_hide", json!({"hide": ["terrain"]})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(answer["hidden"], json!(["terrain"]), "the answer names what is hidden now: {answer}");
    assert_eq!(answer["drawn"], json!(["volume", "water"]), "and what a still would still draw: {answer}");
    app.update();
    assert!(drawn(&mut app, Surface::Terrain).iter().all(|e| !visible(&app, *e)), "the ground is out of the picture");

    let back = call(&mut app, "blueprint/surface_hide", json!({"hide": []})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(back["hidden"], json!([]), "{back}");
    app.update();
    assert!(drawn(&mut app, Surface::Terrain).iter().all(|e| visible(&app, *e)), "and it is drawn again, the same entities");
    assert_eq!(drawn(&mut app, Surface::Terrain), ground, "the drawing was not rebuilt to hide or show it");
}

#[test]
fn hiding_a_surface_is_not_a_change_to_the_map() {
    let mut app = a_hill("surface_view_only");
    let before = oplog_lines(&app);
    assert!(!app.world().resource::<OpenMap>().dirty(), "the map was saved, so nothing is dirty before the call");
    call(&mut app, "blueprint/surface_hide", json!({"hide": ["terrain", "water"]})).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    app.update();
    assert_eq!(oplog_lines(&app), before, "**no line is appended to oplog.jsonl**: a view is not a change to the person's project");
    assert!(!app.world().resource::<OpenMap>().dirty(), "and the map is not marked dirty");
    let state = call(&mut app, "blueprint/state", json!({})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(state["map"]["dirty"], json!(false), "blueprint_state says so too: {state}");
}

#[test]
fn with_no_hide_nothing_is_hidden() {
    let mut app = a_hill("surface_none");
    assert!(app.world().resource::<Eyes>().is_empty(), "nothing is hidden to begin with");
    let answer = call(&mut app, "blueprint/surface_hide", json!({})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(answer["hidden"], json!([]), "an empty `hide` hides nothing, so the default changes nothing: {answer}");
    assert_eq!(answer["drawn"], json!(HIDABLE), "and every surface is drawn: {answer}");
    app.update();
    assert!(drawn(&mut app, Surface::Terrain).iter().all(|e| visible(&app, *e)));
}

#[test]
fn a_surface_the_editor_does_not_draw_is_refused_naming_the_list() {
    let mut app = a_hill("surface_typo");
    let said = call(&mut app, "blueprint/surface_hide", json!({"hide": ["terain"]})).expect_err("a typo is refused");
    assert!(said.contains("terain"), "the refusal names what was asked for: {said}");
    for surface in HIDABLE {
        assert!(said.contains(surface), "and every name that would have worked: {said}");
    }
    assert!(app.world().resource::<Eyes>().is_empty(), "and nothing was hidden");
}

#[test]
fn replace_false_adds_to_what_is_hidden_and_the_default_replaces_it() {
    let mut app = a_hill("surface_adds");
    call(&mut app, "blueprint/surface_hide", json!({"hide": ["terrain"]})).unwrap_or_else(|e| panic!("{e}"));
    let both = call(&mut app, "blueprint/surface_hide", json!({"hide": ["water"], "replace": false})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(both["hidden"], json!(["terrain", "water"]), "a call that adds keeps what was hidden: {both}");
    let replaced = call(&mut app, "blueprint/surface_hide", json!({"hide": ["water"]})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(replaced["hidden"], json!(["water"]), "and the default replaces it: {replaced}");
}

#[test]
fn a_water_only_hide_leaves_the_ground_alone() {
    let mut app = a_hill("surface_water");
    let depths = vec!["0.5"; 64].join(", ");
    write(
        &app,
        "water",
        "m.field.yaml",
        &format!("{{origin: [0.0, 0.0], cell_m: 1.0, width: 8, depth: 8, depth_m: [{depths}], settled: true, steps: 3}}"),
    );
    until(&mut app, "the pool drawn", |a| !drawn(a, Surface::Water).is_empty());
    call(&mut app, "blueprint/surface_hide", json!({"hide": ["water"]})).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    assert!(drawn(&mut app, Surface::Water).iter().all(|e| !visible(&app, *e)), "the water is out of the picture");
    assert!(drawn(&mut app, Surface::Terrain).iter().all(|e| visible(&app, *e)), "and the ground is untouched");
}

#[test]
fn the_eye_survives_a_drawing_rebuild() {
    let mut app = a_hill("surface_rebuild");
    call(&mut app, "blueprint/surface_hide", json!({"hide": ["terrain"]})).unwrap_or_else(|e| panic!("{e}"));
    app.update();
    // **A second terrain edit** respawns the ground; the eye must still hold, or a still taken after a
    // correction would show the skin again with no call having changed it.
    write(&app, "terrain", "m.yaml", &GROUND.replace("10.0", "11.0"));
    for frame in 0..SEEN_WITHIN.0 {
        app.update();
        if drawn(&mut app, Surface::Terrain).iter().all(|e| !visible(&app, *e)) && frame > HELD_FOR.0 {
            return;
        }
        std::thread::sleep(PAUSE);
    }
    panic!("the terrain was drawn again after a rebuild: the eye did not hold");
}
