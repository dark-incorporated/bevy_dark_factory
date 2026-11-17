//! **`placement_add` no longer replaces an overlapping piece unasked** (#443, a wish from the style study).
//!
//! In the study's runs an awning replaced a stall: `placement_add` onto a footprint that overlaps a piece took the piece out
//! and said nothing an agent that asked to ADD would read. The click keeps its replace-on-click behaviour (a person sees
//! the piece go, and the status line names it); the operation, which an agent calls blind, now asks.
//!
//! # What is proved
//!
//! 1. Adding onto an occupied footprint is **refused naming both pieces**, and changes nothing: the map, its file and
//!    the operation log are as they were. A preview refuses the same way.
//! 2. `replace: true` replaces, and the answer **names what it replaced** (id, kind, at, cell) beside the new id.
//! 3. The schema says so, so an agent that reads `blueprint_list_operations` learns it before it is refused.
//! 4. A **hand-back that covers a replacement lists it as a replacement**: the state, the modal's body and the card.
//! 5. A piece that is only flush with another is not an overlap, and the exact same piece is still "already there".

use bevy::prelude::*;
use bevy_dark_factory::night_shift::{NightShiftItem, HandBackCard, HandBackHere};
use bevy_dark_factory::harness;
use bevy_dark_factory::project::OpenMap;
use bevy_gooey::ValueChange;
use serde_json::{Value, json};

use crate::fixtures;

/// **A map holding one crate at (2, 2)**, with a barrel and a crate in the library, both a metre across.
fn project(name: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(&format!("{name}_{}", std::process::id()))
        .descriptor("crate", "alpha")
        .descriptor("barrel", "alpha")
        .place("crate", (2.0, 2.0))
        .build("m")
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    app.add_plugins(bevy_dark_factory::launcher::LauncherPlugin);
    app.world_mut().spawn((Window::default(), bevy::window::PrimaryWindow));
    updates(&mut app, 8);
    app
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn call(app: &mut App, method: &str, params: Value) -> Result<Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    let answer = app.world_mut().run_system_with(id, Some(params)).map_err(|e| format!("{method}: {e:?}"))?.map_err(|e| format!("{method}: {e:?}"))?;
    updates(app, 4);
    Ok(answer)
}

fn apply(app: &mut App, parameters: Value) -> Value {
    call(app, "blueprint/operation_apply", json!({"operation": "placement_add", "parameters": parameters, "dry_run": false})).unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

fn preview(app: &mut App, parameters: Value) -> Value {
    call(app, "blueprint/operation_preview", json!({"operation": "placement_add", "parameters": parameters})).unwrap_or_else(|e| panic!("the preview answers: {e}"))
}

fn placements(app: &App) -> Vec<String> {
    app.world().resource::<OpenMap>().map.placements.iter().map(|p| p.id.clone()).collect()
}

fn oplog_lines(root: &std::path::Path) -> usize {
    let path = bevy_dark_factory::ops::oplog_path_at(root);
    std::fs::read_to_string(path).map(|text| text.lines().count()).unwrap_or(0)
}

/// **1. An add onto an occupied footprint is refused, naming both pieces, and changes nothing.**
#[test]
fn an_add_onto_an_occupied_footprint_is_refused_naming_both_and_changes_nothing() {
    let root = project("overlap_refused");
    let mut app = editor(&root);
    let before = placements(&app);
    assert_eq!(before, vec!["crate@0".to_owned()], "the fixture starts with one crate");
    let file_before = std::fs::read_to_string(&app.world().resource::<OpenMap>().map_path).expect("the map file");
    let log_before = oplog_lines(&root);

    let refused = apply(&mut app, json!({"piece": "barrel", "at": [2.0, 2.0]}));
    assert_eq!(refused["outcome"], "refused", "an add onto a crate replaced it, unasked: {refused}");
    let why = refused["refusal"].as_str().unwrap_or_default();
    assert!(why.contains("barrel"), "the refusal does not name the piece being added: {why:?}");
    assert!(why.contains("crate@0") && why.contains("(crate)"), "the refusal does not name the piece in the way, with its kind: {why:?}");
    assert!(why.contains("replace"), "the refusal does not say how to replace it on purpose: {why:?}");
    assert!(why.contains("(2.00, 2.00)"), "the refusal does not say where: {why:?}");

    assert_eq!(placements(&app), before, "a refused add changed the map");
    let file_after = std::fs::read_to_string(&app.world().resource::<OpenMap>().map_path).expect("the map file");
    assert_eq!(file_after, file_before, "a refused add wrote the file");
    assert_eq!(oplog_lines(&root), log_before, "a refused add left an applied line in the log");

    // The same refusal comes from a preview, so an agent that checks first is told first.
    let previewed = preview(&mut app, json!({"piece": "barrel", "at": [2.0, 2.0]}));
    assert_eq!(previewed["outcome"], "refused", "a preview of an overlapping add was allowed: {previewed}");
    assert!(previewed["refusal"].as_str().unwrap_or_default().contains("crate@0"), "{previewed}");
}

/// **2. `replace: true` replaces, and the answer says what went.**
#[test]
fn replace_true_replaces_and_reports_what_it_replaced() {
    let root = project("overlap_replaced");
    let mut app = editor(&root);

    let done = apply(&mut app, json!({"piece": "barrel", "at": [2.0, 2.0], "replace": true}));
    assert_eq!(done["outcome"], "applied", "{done}");
    let after = &done["log_entry"]["after"];
    let id = after["id"].as_str().unwrap_or_else(|| panic!("the answer names the new id: {done}")).to_owned();
    assert_eq!(
        after["replaced"],
        json!([{"id": "crate@0", "kind": "crate", "at": [2.0, 2.0], "cell": [2, 2]}]),
        "the answer does not name what it replaced (id, kind, at, cell): {done}"
    );
    assert_eq!(after["removed"], json!(["crate@0"]), "the ids that were taken out stay as they were: {done}");
    assert!(done["log_entry"]["result"].as_str().unwrap_or_default().starts_with("replaced `crate@0`"), "{done}");
    assert_eq!(placements(&app), vec![id.clone()], "the barrel stands where the crate was");

    // And one undo of the replacement puts the crate back.
    let undone = call(&mut app, "blueprint/operation_undo", json!({"operation_id": done["operation_id"], "dry_run": false}));
    let undone = undone.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(undone["outcome"], "applied", "{undone}");
    assert_eq!(placements(&app), vec!["crate@0".to_owned()], "the undo did not put the crate back and nothing else");

    // An add that replaces nothing reports an empty list, not a missing key.
    let free = apply(&mut app, json!({"piece": "crate", "at": [9.0, 9.0]}));
    assert_eq!(free["outcome"], "applied", "{free}");
    assert_eq!(free["log_entry"]["after"]["replaced"], json!([]), "{free}");
}

/// **3. The schema says it, so the agent learns before it is refused.**
#[test]
fn the_schema_tells_an_agent_about_replace() {
    let root = project("overlap_schema");
    let mut app = editor(&root);
    let listed = call(&mut app, "blueprint/operation_list", json!({"query": "placement_add"})).unwrap_or_else(|e| panic!("{e}"));
    let row = listed["operations"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["operation"] == "placement_add"))
        .unwrap_or_else(|| panic!("placement_add is listed: {listed}"));
    let schema = &row["parameters"];
    let replace = &schema["properties"]["replace"];
    assert_eq!(replace["type"], "boolean", "the schema has no boolean `replace`: {schema}");
    let said = format!("{} {}", replace["description"].as_str().unwrap_or_default(), schema["description"].as_str().unwrap_or_default());
    assert!(
        said.contains("refused") && said.contains("overlap") && said.contains("replace"),
        "the schema does not say an overlap is refused unless replace is true: {said:?}"
    );
}

/// **4. A hand-back that covers a replacement lists it as a replacement.**
#[test]
fn a_handback_lists_a_replacement_as_a_replacement() {
    let root = project("overlap_handback");
    let mut app = editor(&root);
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<NightShiftItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(&mut app, 6);
    app.world_mut().resource_mut::<HandBackHere>().0 = true;
    updates(&mut app, 2);

    let laid = apply(&mut app, json!({"piece": "crate", "at": [6.0, 6.0]}));
    let swapped = apply(&mut app, json!({"piece": "barrel", "at": [2.0, 2.0], "replace": true}));
    let ids: Vec<Value> = [&laid, &swapped].iter().map(|a| a["operation_id"].clone()).collect();
    let made = call(&mut app, "blueprint/present", json!({"title": "the yard", "summary": "Laid a crate and a barrel.", "operation_ids": ids})).unwrap_or_else(|e| panic!("{e}"));
    assert!(made["presentation"].is_string(), "{made}");

    // The state an agent reads back.
    let state = call(&mut app, "blueprint/state", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let replaced = state["presentation"]["replaced"].as_array().unwrap_or_else(|| panic!("the presentation lists no replacements: {state}"));
    assert_eq!(replaced.len(), 1, "only the second operation replaced anything: {replaced:?}");
    let line = replaced[0].as_str().unwrap_or_default();
    assert!(line.contains("crate@0") && line.contains("(crate)") && line.contains("barrel@"), "the line names what went and what took its place: {line:?}");

    // The card the person reads.
    let world = app.world_mut();
    assert_eq!(world.query_filtered::<Entity, With<HandBackCard>>().iter(world).count(), 1, "the hand-back is a card in the block");
    let shown = world.query::<&Text>().iter(world).map(|text| text.0.clone()).collect::<Vec<_>>().join("\n");
    assert!(shown.contains("REPLACED"), "the card has no REPLACED row: {shown}");
    assert!(shown.contains("crate@0"), "the card does not say which piece was replaced: {shown}");

    // And the modal's body, which is what a closed block asks.
    let presented = app.world().resource::<bevy_dark_factory::present::Presenting>().0.clone().expect("a presentation");
    assert!(presented.body.contains("Replaced:") && presented.body.contains("crate@0"), "{}", presented.body);
}

/// **A hand-back that replaced nothing says nothing about replacing.**
#[test]
fn a_handback_that_replaced_nothing_has_no_replaced_row() {
    let root = project("overlap_handback_none");
    let mut app = editor(&root);
    let laid = apply(&mut app, json!({"piece": "crate", "at": [6.0, 6.0]}));
    call(&mut app, "blueprint/present", json!({"title": "the yard", "summary": "Laid a crate.", "operation_ids": [laid["operation_id"]]})).unwrap_or_else(|e| panic!("{e}"));
    let state = call(&mut app, "blueprint/state", json!({})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(state["presentation"]["replaced"], json!([]), "{state}");
    let presented = app.world().resource::<bevy_dark_factory::present::Presenting>().0.clone().expect("a presentation");
    assert!(!presented.body.contains("Replaced"), "{}", presented.body);
}

/// **5. Flush is not an overlap, and the same piece in the same place is still "already there".**
#[test]
fn a_piece_beside_another_is_not_an_overlap_and_the_same_piece_is_still_already_there() {
    let root = project("overlap_flush");
    let mut app = editor(&root);
    let beside = apply(&mut app, json!({"piece": "barrel", "at": [3.0, 2.0]}));
    assert_eq!(beside["outcome"], "applied", "a barrel flush against the crate was refused: {beside}");
    assert_eq!(beside["log_entry"]["after"]["replaced"], json!([]), "{beside}");

    let again = apply(&mut app, json!({"piece": "crate", "at": [2.0, 2.0], "replace": true}));
    assert_eq!(again["outcome"], "refused", "{again}");
    assert!(again["refusal"].as_str().unwrap_or_default().contains("already there"), "{again}");
}
