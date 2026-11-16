//! **`volume_edit` can correct an entry, and refuses one that is not there** (#474).
//!
//! It could only append, so correcting a 26-entry carve list had two paths and both were wrong:
//! append a compensating edit (the file becomes a diary — dead geometry accumulating), or rewrite
//! the YAML by hand (which the oplog never saw, so the person's Undo all left the broken geometry
//! standing while telling them their work was reverted).
//!
//! **`at: <index>` replaces the entry at that index**, in place and in one logged, undoable change,
//! and `remove: <index>` takes one out. **An index the list has not got is refused**, naming it and
//! the list's length: no silent append, no edit landing somewhere the caller did not ask for.

use bevy_cauldron::ops::{self, Applied, Ctx};
use bevy_grimoire_core::volume::{Mode, Shape, Volume};
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

fn done(r: Result<Applied, ops::Refusal>) -> (String, String) {
    match r {
        Ok(Applied::Done { outcome, operation_id, .. }) => (outcome.summary, operation_id),
        Ok(Applied::Refused(e)) | Err(e) => panic!("the call was refused: {}", e.0),
    }
}

fn refused(r: Result<Applied, ops::Refusal>) -> String {
    match r {
        Ok(Applied::Refused(e)) | Err(e) => e.0,
        Ok(Applied::Done { outcome, .. }) => panic!("the call was accepted: {}", outcome.summary),
    }
}

fn box_edit(center: [f32; 3], mode: &str) -> serde_json::Value {
    json!({"shape": {"Box": {"center": center, "half_m": [1.0, 1.0, 1.0]}}, "mode": mode})
}

/// The lair of the issue in miniature: a rock mass, a gallery carved through it, and a column added
/// inside the gallery — the order that makes a later carve able to eat an earlier `Add`.
fn a_lair(tag: &str) -> (Ctx, std::path::PathBuf) {
    let (mut ctx, file) = project(tag);
    for edit in [box_edit([0.0, 4.0, 0.0], "Add"), box_edit([0.0, 1.0, 0.0], "Carve"), box_edit([0.0, 0.0, 0.0], "Add")] {
        done(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "edit": edit})));
    }
    assert_eq!(load(&file).edits.len(), 3);
    (ctx, file)
}

#[test]
fn at_replaces_the_entry_that_stands_there_and_adds_nothing() {
    let (mut ctx, file) = a_lair("volume_corrects_replace");
    let column = json!({"shape": {"Capsule": {"a": [0.0, 0.0, 0.0], "b": [0.0, 4.0, 0.0], "radius_m": 0.6}}, "mode": "Add"});
    let (said, _) = done(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "at": 2, "edit": column})));
    assert!(said.contains("index 2"), "the answer names the entry: {said}");
    assert!(said.contains("Box"), "and what stood there before: {said}");
    assert!(said.contains("Capsule"), "and what replaced it: {said}");
    let v = load(&file);
    assert_eq!(v.edits.len(), 3, "a replace adds nothing");
    assert!(matches!(v.edits[2].shape, Shape::Capsule { radius_m, .. } if radius_m == 0.6));
    assert!(matches!(v.edits[0].shape, Shape::Box { .. }), "the entries around it are untouched");
    assert_eq!(v.edits[1].mode, Mode::Carve);
}

#[test]
fn a_replace_is_one_undo_and_puts_the_entry_it_replaced_back() {
    let (mut ctx, file) = a_lair("volume_corrects_undo");
    let was = load(&file).edits[2];
    let (_, id) = done(ops::dispatch(
        &mut ctx,
        "volume_edit",
        &json!({"map": "m", "at": 2, "edit": {"shape": {"Sphere": {"center": [0.0, 0.0, 0.0], "radius_m": 1.5}}, "mode": "Carve"}}),
    ));
    assert_eq!(load(&file).edits[2].mode, Mode::Carve, "the replace landed");
    ops::undo(&mut ctx, Some(&id)).unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(load(&file).edits[2], was, "one undo puts the entry back exactly, and the list keeps its length");
    assert_eq!(load(&file).edits.len(), 3);
}

#[test]
fn remove_takes_one_entry_out_and_leaves_the_rest_in_order() {
    let (mut ctx, file) = a_lair("volume_corrects_remove");
    let (said, id) = done(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "remove": 1})));
    assert!(said.contains("index 1"), "the answer names what went: {said}");
    let v = load(&file);
    assert_eq!(v.edits.len(), 2, "one entry fewer");
    assert_eq!(v.edits[0].mode, Mode::Add);
    assert_eq!(v.edits[1].mode, Mode::Add, "the column moved up one, unchanged");
    ops::undo(&mut ctx, Some(&id)).unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(load(&file).edits.len(), 3, "and one undo puts it back");
    assert_eq!(load(&file).edits[1].mode, Mode::Carve);
}

#[test]
fn an_index_past_the_end_is_refused_naming_it_and_writes_nothing() {
    let (mut ctx, file) = a_lair("volume_corrects_absent");
    let before = std::fs::read_to_string(&file).unwrap();
    // **No silent append**: the old path would have appended a fourth entry and said `edited`.
    let said = refused(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "at": 9, "edit": box_edit([0.0, 2.0, 0.0], "Carve")})));
    assert!(said.contains("index 9"), "the refusal names the index given: {said}");
    assert!(said.contains('3'), "and the list that has no such index: {said}");
    assert!(said.contains("Nothing was changed"), "{said}");
    let said = refused(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "remove": 3})));
    assert!(said.contains("index 3"), "a removal is refused the same way: {said}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before, "the file is byte-for-byte what it was");
}

#[test]
fn an_entry_the_single_edit_path_would_refuse_is_refused_in_place_too() {
    let (mut ctx, file) = a_lair("volume_corrects_bad_edit");
    let before = std::fs::read_to_string(&file).unwrap();
    // A radius of zero is what `Volume::push` refuses; a replace must refuse it the same way, or the
    // file would carry an entry no reader accepts.
    let said = refused(ops::dispatch(
        &mut ctx,
        "volume_edit",
        &json!({"map": "m", "at": 0, "edit": {"shape": {"Sphere": {"center": [0.0, 0.0, 0.0], "radius_m": 0.0}}, "mode": "Add"}}),
    ));
    assert!(!said.is_empty(), "a refusal says why");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before, "and nothing was written");
}

#[test]
fn at_and_remove_together_are_refused_because_one_call_changes_one_entry() {
    let (mut ctx, file) = a_lair("volume_corrects_both");
    let before = std::fs::read_to_string(&file).unwrap();
    let said = refused(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "at": 1, "remove": 0})));
    assert!(said.contains("one"), "the refusal says a call changes one entry: {said}");
    let said = refused(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "at": 1, "edit": box_edit([0.0; 3], "Carve"), "clear": true})));
    assert!(said.contains("one"), "a replace cannot also clear the list: {said}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before, "nothing was written either time");
}

#[test]
fn an_append_still_works_with_no_index_named() {
    let (mut ctx, file) = a_lair("volume_corrects_append");
    // The one-edit behaviour is unchanged (#474's own acceptance): existing callers keep working.
    let (said, _) = done(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "edit": box_edit([0.0, -4.0, 0.0], "Carve")})));
    assert!(said.contains("index 3"), "{said}");
    let v = load(&file);
    assert_eq!(v.edits.len(), 4);
    assert!(matches!(v.edits[3].shape, Shape::Box { center, .. } if center[1] == -4.0), "appended at the end, as it always was");
}
