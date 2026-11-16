//! **`volume_edit` says which entry it changed** (#473).
//!
//! The answer used to be `volume m: edited` and nothing else, so the one thing a carve list makes
//! load-bearing — *which* entry of it a call touched — was not reportable. An agent building a
//! 26-entry lair read `edited` twenty-six times and learned nothing from any of them.
//!
//! The answer now names the index the edit stands at, how many entries the list holds, what kind of
//! shape it is and whether it adds or carves; the index and the count are in `after` as well, so the
//! log line carries them and a later reader can find the entry a change made.

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_grimoire_core::volume::Volume;
use serde_json::json;

use crate::fixtures;

fn project(tag: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_cauldron::project::Project>().clone();
    let file = project.grimoire_dir.join("volume").join("m.yaml");
    let open = app.world().resource::<bevy_cauldron::project::OpenMap>().clone();
    (Ctx { project: Some(project), open: Some(open), session: "test".to_owned(), ..Ctx::default() }, file)
}

fn load(file: &std::path::Path) -> Volume {
    bevy_grimoire_core::yaml::from_str(&std::fs::read_to_string(file).unwrap(), "test").unwrap()
}

/// **The change**, or the refusal's own sentence: a test that asserts on a refused call says why.
fn change(r: Result<Applied, ops::Refusal>) -> (String, serde_json::Value) {
    match r {
        Ok(Applied::Done { outcome, .. }) => (outcome.summary, outcome.after.clone().unwrap_or(serde_json::Value::Null)),
        Ok(Applied::Refused(e)) | Err(e) => panic!("the call was refused: {}", e.0),
    }
}

fn box_edit(y: f32, mode: &str) -> serde_json::Value {
    json!({"shape": {"Box": {"center": [0.0, y, 0.0], "half_m": [1.0, 1.0, 1.0]}}, "mode": mode})
}

/// Three entries: a hill added, a tunnel carved through it, and a column added back inside the tunnel.
fn a_lair(tag: &str) -> (Ctx, std::path::PathBuf) {
    let (mut ctx, file) = project(tag);
    for edit in [box_edit(4.0, "Add"), box_edit(1.0, "Carve"), box_edit(0.0, "Add")] {
        change(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "edit": edit})));
    }
    assert_eq!(load(&file).edits.len(), 3);
    (ctx, file)
}

#[test]
fn an_added_edit_answers_with_the_index_it_landed_at() {
    let (mut ctx, file) = a_lair("volume_says_added");
    assert_eq!(load(&file).edits.len(), 3, "three entries stand before the call");
    let (said, after) = change(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "edit": box_edit(-4.0, "Carve")})));
    assert!(said.contains("index 3"), "an append says the index it landed at: {said}");
    assert!(said.contains("Carve"), "and what it does there: {said}");
    assert!(said.contains("4"), "and how many entries the list holds now: {said}");
    assert_eq!(after["index"], 3, "the index travels in the log line's `after`: {after}");
    assert_eq!(after["entries"], 4, "and so does the count: {after}");
    assert_eq!(after["mode"], "Carve", "{after}");
    assert_eq!(load(&file).edits.len(), 4);
}

#[test]
fn the_index_reads_the_list_as_it_stands_now_not_as_it_was() {
    let (mut ctx, file) = a_lair("volume_says_growing");
    // A second append is index 3 again only if the first was counted: the count is the list's own.
    let (first, after) = change(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "edit": box_edit(-4.0, "Carve")})));
    assert_eq!(after["index"], 3, "{first}");
    let (second, after) = change(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "edit": box_edit(-8.0, "Carve")})));
    assert_eq!(after["index"], 4, "the next append lands after the last: {second}");
    assert_eq!(after["entries"], 5, "{second}");
    assert_eq!(load(&file).edits.len(), 5);
}

#[test]
fn a_preview_names_the_index_the_apply_would_write_to() {
    let (mut ctx, file) = a_lair("volume_says_previewed");
    let params = json!({"map": "m", "edit": box_edit(-4.0, "Carve")});
    let op = ops::find("volume_edit").expect("volume_edit is registered");
    let planned = ops::preview(&ctx, op.as_ref(), &params).unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(planned["index"], 3, "a preview says where it would land: {planned}");
    assert_eq!(planned["entries"], 4, "counting the entry it would add, as its apply does: {planned}");
    assert_eq!(load(&file).edits.len(), 3, "and writes nothing");
    let (_, after) = change(ops::dispatch(&mut ctx, "volume_edit", &params));
    assert_eq!(after["index"], 3, "and the apply lands exactly where it said it would");
    assert_eq!(after["entries"], 4);
}
