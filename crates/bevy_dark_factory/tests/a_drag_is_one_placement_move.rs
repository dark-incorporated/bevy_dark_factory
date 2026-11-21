//! **A drag is an operation** (#500): one `placement_move`, on disk, with one log line and one Cmd+Z.
//!
//! The move tool used to be a second path that skipped the registry: `drop_piece` called
//! `stack::move_placement` and recorded an in-memory `Undo::Moved`, so the file was not written, the
//! operation log never heard of the move, and an agent — whose only door is the registry — had no tool
//! for the verb a level editor is used most for.
//!
//! What is proved here:
//!
//! 1. **The drag is the operation.** `drop_for_test` (which calls the real `drop_piece` the click calls)
//!    moves a piece, and the map file on disk holds the new `at` and one `placement_move` line.
//! 2. **An agent's `placement_move` reaches the same result**, so the file and the map cannot disagree.
//! 3. **One Cmd+Z puts the piece and everything riding on it back**, and the file agrees.

use bevy::prelude::*;
use bevy_dark_factory::editor::{EditorState, drop_for_test, undo_for_test};
use bevy_dark_factory::harness;
use bevy_dark_factory::ops::{self, Entry};
use bevy_dark_factory::project::{OpenMap, Project};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place_as("wall@0", "site/wall", (0.0, 0.0))
        .place_as("wall@1", "site/wall", (3.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // The registered methods an agent's request arrives through.
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn held(app: &App) -> blueprint_model::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

/// **The map as the file has it**, parsed from disk — the assertion the drag used to fail.
fn on_disk(app: &App) -> blueprint_model::map::Map {
    let path = app.world().resource::<OpenMap>().map_path.clone();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    blueprint_model::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"))
}

fn at(map: &blueprint_model::map::Map, id: &str) -> Option<(f32, f32)> {
    map.placements.iter().find(|p| p.id == id).map(|p| p.at)
}

fn log(app: &App) -> Vec<Entry> {
    let path = ops::oplog_path(app.world().resource::<Project>());
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str::<Entry>(line).unwrap_or_else(|e| panic!("{e}: {line}")))
        .collect()
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app
        .world()
        .resource::<RemoteMethods>()
        .get("blueprint/operation_apply")
        .cloned()
    else {
        panic!("blueprint/operation_apply is not registered");
    };
    app.world_mut()
        .run_system_with(
            id,
            Some(serde_json::json!({
                "operation": operation,
                "parameters": parameters,
                "dry_run": false,
            })),
        )
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"))
}

/// **A drag writes the file, leaves one `placement_move` line, and one Cmd+Z puts the piece back.**
#[test]
fn a_drag_is_one_placement_move_on_disk_with_one_undo() {
    let mut app = editor("drag_is_move");
    assert_eq!(at(&held(&app), "wall@0"), Some((0.0, 0.0)));

    drop_for_test(app.world_mut(), "wall@0", (1.0, 4.0));

    assert_eq!(at(&held(&app), "wall@0"), Some((1.0, 4.0)), "the map holds the new place");
    assert_eq!(
        at(&on_disk(&app), "wall@0"),
        Some((1.0, 4.0)),
        "the file holds it too — a drag that only changes memory is not an operation"
    );
    let moves: Vec<Entry> = log(&app).into_iter().filter(|e| e.op == "placement_move").collect();
    assert_eq!(moves.len(), 1, "a drag is one line in the operation log: {moves:?}");
    assert_eq!(moves[0].params["id"], "wall@0", "the line names the piece it moved");
    assert!(matches!(moves[0].source, ops::Source::Human), "{:?}", moves[0].source);
    assert_eq!(app.world().resource::<EditorState>().undo_depth(), 1, "one act, one undo entry");

    undo_for_test(app.world_mut());
    assert_eq!(at(&held(&app), "wall@0"), Some((0.0, 0.0)), "Cmd+Z puts it back");
    assert_eq!(
        at(&on_disk(&app), "wall@0"),
        Some((0.0, 0.0)),
        "and the file agrees, so the watcher has nothing to argue with"
    );
}

/// **An agent's `placement_move` cannot leave the file and the map disagreeing.**
#[test]
fn an_agents_placement_move_writes_the_map_it_reports() {
    let mut app = editor("agent_move");
    let answer = apply(
        &mut app,
        "placement_move",
        serde_json::json!({"id": "wall@1", "at": [-2.0, 5.5]}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    assert_eq!(answer["saved"], true, "the operation says the file was written: {answer}");
    assert_eq!(at(&held(&app), "wall@1"), Some((-2.0, 5.5)));
    assert_eq!(
        at(&on_disk(&app), "wall@1"),
        Some((-2.0, 5.5)),
        "the map and the file are the same map"
    );
    // **And the relative form moves by a delta**, so a drag that computes a delta has one field.
    let by = apply(
        &mut app,
        "placement_move",
        serde_json::json!({"id": "wall@1", "by": [1.0, -0.5]}),
    );
    assert_eq!(by["outcome"], "applied", "{by}");
    assert_eq!(at(&on_disk(&app), "wall@1"), Some((-1.0, 5.0)));
    assert_eq!(
        log(&app).iter().filter(|e| e.op == "placement_move").count(),
        2,
        "each move is its own line"
    );
}
