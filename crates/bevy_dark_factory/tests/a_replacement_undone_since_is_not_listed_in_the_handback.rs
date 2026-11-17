//! **A hand-back does not list a replacement the agent later undid** (#450, from the #443 review).
//!
//! `present::replacements` read the log's `after.replaced` of every operation the hand-back covers, even when that add had been undone since, so the REPLACED row
//! (the card, the modal's "Replaced:" block and `blueprint_state.presentation.replaced`) named a replacement that no longer stands: the piece it says is gone is
//! back on the map. A replacement counts only while its change is in effect, by the one rule the undo itself uses (the chain of the line, flipped by each undo).
//!
//! - **an undone replacement is not listed**: no REPLACED row, no "Replaced:" block, an empty `presentation.replaced`;
//! - **a replacement that stands is still listed**, including one undone and then put back, and including one beside an unrelated undone add.

use bevy::prelude::*;
use bevy_dark_factory::night_shift::{NightShiftItem, HandBackCard, HandBackHere};
use bevy_dark_factory::harness;
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

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// An editor with the AGENT block open, so a hand-back is a card as well as a body.
fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    app.add_plugins(bevy_dark_factory::launcher::LauncherPlugin);
    app.world_mut().spawn((Window::default(), bevy::window::PrimaryWindow));
    updates(&mut app, 8);
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<NightShiftItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    updates(&mut app, 6);
    app.world_mut().resource_mut::<HandBackHere>().0 = true;
    updates(&mut app, 2);
    app
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

fn add(app: &mut App, parameters: Value) -> String {
    let answer = call(app, "blueprint/operation_apply", json!({"operation": "placement_add", "parameters": parameters, "dry_run": false})).unwrap_or_else(|e| panic!("the apply answers: {e}"));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    answer["operation_id"].as_str().expect("an operation id").to_owned()
}

fn undo(app: &mut App, operation_id: &str) -> String {
    let answer = call(app, "blueprint/operation_undo", json!({"operation_id": operation_id, "dry_run": false})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(answer["outcome"], "applied", "{answer}");
    answer["operation_id"].as_str().expect("the undo's own id").to_owned()
}

fn present(app: &mut App, ids: &[&str]) {
    let made = call(app, "blueprint/present", json!({"title": "the yard", "summary": "Laid pieces.", "operation_ids": ids})).unwrap_or_else(|e| panic!("{e}"));
    assert!(made["presentation"].is_string(), "{made}");
}

/// What the person and the agent read of the last hand-back: `(state's replaced, the modal's body, every text on screen)`.
fn read(app: &mut App) -> (Value, String, String) {
    let state = call(app, "blueprint/state", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let body = app.world().resource::<bevy_dark_factory::present::Presenting>().0.clone().expect("a presentation").body;
    let world = app.world_mut();
    assert_eq!(world.query_filtered::<Entity, With<HandBackCard>>().iter(world).count(), 1, "the hand-back is a card in the block");
    let shown = world.query::<&Text>().iter(world).map(|text| text.0.clone()).collect::<Vec<_>>().join("\n");
    (state["presentation"]["replaced"].clone(), body, shown)
}

/// **A replacing add, undone, then presented: no REPLACED row, no "Replaced:" block, an empty `presentation.replaced`.**
#[test]
fn a_replacement_undone_since_is_not_listed() {
    let mut app = editor(&project("undone_replacement"));
    let swapped = add(&mut app, json!({"piece": "barrel", "at": [2.0, 2.0], "replace": true}));
    undo(&mut app, &swapped);
    present(&mut app, &[&swapped]);
    let (replaced, body, shown) = read(&mut app);
    assert_eq!(replaced, json!([]), "the state names a replacement that no longer stands");
    assert!(!body.contains("Replaced"), "the modal's body names a replacement that no longer stands: {body}");
    assert!(!shown.contains("REPLACED"), "the card has a REPLACED row for a replacement that no longer stands: {shown}");
}

/// **A replacing add that stands is still listed**, and so is one that was undone and then put back.
#[test]
fn a_replacement_that_stands_is_listed_even_after_it_was_undone_and_put_back() {
    let mut app = editor(&project("standing_replacement"));
    let swapped = add(&mut app, json!({"piece": "barrel", "at": [2.0, 2.0], "replace": true}));
    let took_back = undo(&mut app, &swapped);
    undo(&mut app, &took_back);
    present(&mut app, &[&swapped]);
    let (replaced, body, shown) = read(&mut app);
    let lines = replaced.as_array().unwrap_or_else(|| panic!("no replacements listed: {replaced}"));
    assert_eq!(lines.len(), 1, "the replacement stands again: {lines:?}");
    assert!(body.contains("Replaced:") && body.contains("crate@0"), "{body}");
    assert!(shown.contains("REPLACED") && shown.contains("crate@0"), "{shown}");
}

/// **Only the replacement that was undone is dropped**: one that stands beside it is listed.
#[test]
fn only_the_undone_replacement_is_dropped() {
    let mut app = editor(&project("one_of_two"));
    let laid = add(&mut app, json!({"piece": "crate", "at": [6.0, 6.0]}));
    let first = add(&mut app, json!({"piece": "barrel", "at": [6.0, 6.0], "replace": true}));
    let second = add(&mut app, json!({"piece": "barrel", "at": [2.0, 2.0], "replace": true}));
    // The newest change is the one an undo may take back.
    undo(&mut app, &second);
    present(&mut app, &[&laid, &first, &second]);
    let (replaced, ..) = read(&mut app);
    let lines = replaced.as_array().unwrap_or_else(|| panic!("no replacements listed: {replaced}"));
    assert_eq!(lines.len(), 1, "only the replacement that stands is listed: {lines:?}");
    assert!(lines[0].as_str().unwrap_or_default().contains("crate@1"), "the one that stands replaced the crate laid at (6, 6): {lines:?}");
}
