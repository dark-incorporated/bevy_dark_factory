//! **`volume_set` replaces a map's whole carve list, in one logged, undoable change** (#474).
//!
//! `volume_edit` appends one entry, and `at`/`remove` correct one entry. Neither can say "the list is
//! this now", and correcting a 26-entry carve list therefore had one path left: **rewrite the YAML by
//! hand**, which the operation log never saw. The hand-back a person was shown offered *Undo all*, and
//! it would have left the broken geometry standing while telling them their work was reverted.
//!
//! **So the whole list is one change**: the file the one writer writes, one `volume_set` line in
//! `oplog.jsonl` whose `inverse` is the file as it was, and a list containing one entry the
//! single-edit path refuses refused whole, naming the index, with nothing written.

use bevy_dark_factory::ops::{self, Applied, Ctx};
use blueprint_model::volume::{Mode, Shape, Volume};
use serde_json::json;

use crate::fixtures;

fn project(tag: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_dark_factory::project::Project>().clone();
    let file = project.blueprint_dir.join("volume").join("m.yaml");
    let open = app.world().resource::<bevy_dark_factory::project::OpenMap>().clone();
    (Ctx { project: Some(project), open: Some(open), session: "test".to_owned(), ..Ctx::default() }, file)
}

fn load(file: &std::path::Path) -> Volume {
    blueprint_model::yaml::from_str(&std::fs::read_to_string(file).unwrap(), "test").unwrap()
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

/// The lair of the issue in miniature, as the broken list and the corrected one: a crypt carved out,
/// a dais and a column added inside it, and a gallery that eats the dais. The corrected list keeps
/// every entry and moves the gallery's `z` until it stops short.
fn broken() -> Vec<serde_json::Value> {
    vec![
        json!({"shape": {"Box": {"center": [0.0, 0.0, 0.0], "half_m": [4.0, 2.0, 4.0]}}, "mode": "Carve"}),
        json!({"shape": {"Box": {"center": [0.0, 2.0, 0.0], "half_m": [1.0, 1.0, 1.0]}}, "mode": "Add"}),
        json!({"shape": {"Capsule": {"a": [-6.0, 2.0, 0.0], "b": [6.0, 2.0, 0.0], "radius_m": 1.0}}, "mode": "Carve"}),
    ]
}

fn corrected() -> Vec<serde_json::Value> {
    let mut list = broken();
    list[2] = json!({"shape": {"Capsule": {"a": [-6.0, 3.5, 0.0], "b": [6.0, 3.5, 0.0], "radius_m": 1.0}}, "mode": "Carve"});
    list
}

/// The text of the volume file after the same list, written the only other way there is: one
/// `volume_edit` append per entry.
fn by_appending(tag: &str, list: &[serde_json::Value]) -> String {
    let (mut ctx, file) = project(tag);
    for edit in list {
        done(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "edit": edit})));
    }
    std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()))
}

#[test]
fn the_whole_list_replaces_the_file_in_one_change() {
    let by_hand = by_appending("volume_set_append", &broken());
    let (mut ctx, file) = project("volume_set_replace");
    let (said, _) = done(ops::dispatch(&mut ctx, "volume_set", &json!({"map": "m", "edits": broken()})));
    assert!(said.contains('3'), "the answer says how many entries the list holds: {said}");
    assert_eq!(load(&file).edits.len(), 3);
    // **Byte-identical to the file the append path writes**, because both go through the one writer:
    // a `volume_set` is not a second file format, it is a second way to say the same list.
    assert_eq!(std::fs::read_to_string(&file).unwrap(), by_hand, "the same list, written two ways, is the same file");
}

#[test]
fn a_broken_list_is_corrected_and_one_undo_puts_it_back_exactly() {
    let (mut ctx, file) = project("volume_set_undo");
    for edit in broken() {
        done(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "edit": edit})));
    }
    let before: String = std::fs::read_to_string(&file).unwrap();

    let (_, id) = done(ops::dispatch(&mut ctx, "volume_set", &json!({"map": "m", "edits": corrected()})));
    let v = load(&file);
    assert!(matches!(v.edits[2].shape, Shape::Capsule { a, .. } if a[1] == 3.5), "the corrected entry is in the file");
    assert_eq!(v.edits.len(), 3, "a whole-list set adds and removes nothing by itself");

    // **One line, and it is the operation's own name**, carrying the previous list as its inverse.
    let log = ctx.project.as_ref().unwrap().project_dir.join("oplog.jsonl");
    let text = std::fs::read_to_string(&log).unwrap_or_else(|e| panic!("{}: {e}", log.display()));
    let lines: Vec<serde_json::Value> = text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).expect("a log line"))
        .collect();
    let last = lines.last().expect("the set wrote a line");
    assert_eq!(last["op"], json!("volume_set"), "the line says which operation wrote it");
    assert_eq!(last["operation_id"], json!(id), "and it is the id the answer gave");
    assert_eq!(last["inverse"]["put_back"]["text"], json!(before), "its inverse is the file as it stood");
    let sets = lines.iter().filter(|l| l["op"] == json!("volume_set")).count();
    assert_eq!(sets, 1, "one call, one line");

    ops::undo(&mut ctx, Some(&id)).unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before, "one undo puts the broken list back byte for byte");
}

#[test]
fn a_list_with_one_refused_entry_is_refused_whole_naming_the_index() {
    let (mut ctx, file) = project("volume_set_refused");
    done(ops::dispatch(&mut ctx, "volume_set", &json!({"map": "m", "edits": broken()})));
    let before = std::fs::read_to_string(&file).unwrap();

    let mut list = corrected();
    // A radius of zero is what `Volume::push` refuses; a whole list must refuse it the same way, and
    // say where, or an agent would hunt through twenty-six entries for the one it wrote wrong.
    list[1] = json!({"shape": {"Sphere": {"center": [0.0, 2.0, 0.0], "radius_m": 0.0}}, "mode": "Add"});
    let said = refused(ops::dispatch(&mut ctx, "volume_set", &json!({"map": "m", "edits": list})));
    assert!(said.contains("edit 1"), "the refusal names the index: {said}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before, "and nothing was written");

    // A list that is not a list of edits at all is refused by name, not quietly read as empty.
    let said = refused(ops::dispatch(&mut ctx, "volume_set", &json!({"map": "m", "edits": "all of them"})));
    assert!(said.contains("edits"), "the refusal names the field: {said}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
}

#[test]
fn an_empty_list_empties_the_map_and_one_undo_fills_it_again() {
    let (mut ctx, file) = project("volume_set_empty");
    done(ops::dispatch(&mut ctx, "volume_set", &json!({"map": "m", "edits": broken()})));
    let before = std::fs::read_to_string(&file).unwrap();
    let (said, id) = done(ops::dispatch(&mut ctx, "volume_set", &json!({"map": "m", "edits": []})));
    assert!(said.contains('0'), "the answer says the list is empty: {said}");
    let v = load(&file);
    assert!(v.is_empty(), "the list is what was asked for, and an empty one is a list");
    assert_eq!(v.edits, Volume::default().edits);
    ops::undo(&mut ctx, Some(&id)).unwrap_or_else(|e| panic!("{}", e.0));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
}

#[test]
fn volume_edit_appends_one_entry_and_is_not_a_whole_list() {
    // #474's own acceptance: the one-edit path is unchanged, so existing callers keep working.
    let (mut ctx, file) = project("volume_set_unchanged");
    let (said, _) = done(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "edit": broken()[0]})));
    assert!(said.contains("index 0"), "{said}");
    done(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "edit": broken()[1]})));
    let v = load(&file);
    assert_eq!(v.edits.len(), 2);
    assert_eq!(v.edits[1].mode, Mode::Add);
}
