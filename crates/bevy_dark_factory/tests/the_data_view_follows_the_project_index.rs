//! **The data view reads `data.yaml` and the files it lists when the project index says one moved, and no clock of its own** (#501,
//! item 5: `data_view`'s `RECHECK_SECS`, 1.0 s, is retired).
//!
//! - an edit to a registered file made while the editor is open is named within the index's debounce, and the leaf the verdict names is
//!   the one an independent read of the file shows wrong;
//! - the view reads again **only** when a file it follows moved: a file beside them that is written and changed makes it read nothing
//!   (the retired timer re-read every file once a second whatever happened, so this count fails on it);
//! - a registered file the index cannot follow (outside the project) is refused with the reason, and is not read.

use bevy::prelude::*;
use bevy_dark_factory::data_view::DataView;
use bevy_dark_factory::ops::Ctx;
use serde_json::json;

use crate::index_view_support as support;

const REGISTRY: &str = "version: 1\nfiles:\n- id: tuning/mob\n  path: mob.yaml\n  bounds:\n    mob.speed:\n      min: 0.0\n      max: 9.0\n    mob.hp:\n      min: 1.0\n";
const GOOD: &str = "# tuning\nmob:\n  speed: 1.50\n  name: orc\n  hp: 7\nfast: false\n";
const SPEED_A_WORD: &str = "mob:\n  speed: fast\n  name: orc\n  hp: 7\nfast: false\n";

fn dir(ctx: &Ctx) -> std::path::PathBuf {
    ctx.project.as_ref().unwrap_or_else(|| panic!("a project")).blueprint_dir.clone()
}

fn reads(app: &App) -> u64 {
    app.world().resource::<DataView>().reads()
}

/// A project whose registry and file read, looked at once so that is the baseline.
fn project(tag: &str) -> (App, Ctx) {
    let (mut app, ctx) = support::editor(tag);
    std::fs::write(dir(&ctx).join("mob.yaml"), GOOD).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(dir(&ctx).join("data.yaml"), REGISTRY).unwrap_or_else(|e| panic!("{e}"));
    let seen = reads(&app);
    // Both files are new: the view reads the registry and the file it lists.
    support::until_seen(&mut app, "the registry and its file read", |a| reads(a) >= seen + 2);
    support::settle(&mut app);
    (app, ctx)
}

#[test]
fn an_edit_made_while_the_editor_is_open_is_named_within_the_index_debounce() {
    let (mut app, ctx) = project("data_follow_edit");
    assert_eq!(support::verdict(&mut app, "game_data"), serde_json::Value::Null, "a file seen reading for the first time is not news");
    std::fs::write(dir(&ctx).join("mob.yaml"), SPEED_A_WORD).unwrap_or_else(|e| panic!("{e}"));
    let took = support::until_seen(&mut app, "the speed named", |a| support::verdict(a, "game_data")["outcome"] == json!("refused"));
    assert!(took < support::SEEN_WITHIN, "the edit took {took:?} to be named");
    // **Ground truth is a plain read of the file**: the leaf the test broke.
    let on_disk = std::fs::read_to_string(dir(&ctx).join("mob.yaml")).unwrap_or_else(|e| panic!("{e}"));
    let broken = on_disk.lines().find(|l| l.trim_start().starts_with("speed: ") && !l.contains(|c: char| c.is_ascii_digit())).map(|l| l.trim().trim_end_matches(':'));
    assert_eq!(broken, Some("speed: fast"), "the file says the speed is a word");
    assert_eq!(support::verdict(&mut app, "game_data")["field"], json!("mob.speed"));

    std::fs::write(dir(&ctx).join("mob.yaml"), GOOD).unwrap_or_else(|e| panic!("{e}"));
    let took = support::until_seen(&mut app, "the mended file applied", |a| support::verdict(a, "game_data")["outcome"] == json!("applied"));
    assert!(took < support::SEEN_WITHIN, "the repair took {took:?} to be seen");
}

#[test]
fn a_file_beside_the_data_files_that_changes_makes_the_view_read_nothing() {
    let (mut app, ctx) = project("data_follow_neighbour");
    support::a_neighbour_is_not_read(&mut app, &dir(&ctx).join("mob.yaml"), reads);
}

/// **A registered file the index cannot follow is refused, and said, and not read.**
#[test]
fn a_registered_file_outside_the_project_is_refused_and_not_read() {
    let (mut app, ctx) = project("data_follow_outside");
    // **The file is there, and it reads**: it is the index's refusal that keeps it from being read, not its absence. Above the project is the folder that holds the project: as many `..` as the kit's directory is deep in the project, and one more.
    let root = ctx.project.as_ref().unwrap_or_else(|| panic!("a project")).root.clone();
    let depth = dir(&ctx).strip_prefix(&root).unwrap_or_else(|e| panic!("{e}")).components().count() + 1;
    let up = "../".repeat(depth);
    let outside_file = dir(&ctx).join(format!("{up}outside-the-project.yaml"));
    std::fs::write(&outside_file, "mob:\n  hp: 7\n").unwrap_or_else(|e| panic!("{outside_file:?}: {e}"));
    let outside = format!("version: 1\nfiles:\n- id: tuning/mob\n  path: {up}outside-the-project.yaml\n  bounds:\n    mob.hp:\n      min: 1.0\n");
    let before = reads(&app);
    std::fs::write(dir(&ctx).join("data.yaml"), &outside).unwrap_or_else(|e| panic!("{e}"));
    support::until_seen(&mut app, "the outside file refused", |a| support::verdict(a, "game_data")["outcome"] == json!("refused"));
    let v = support::verdict(&mut app, "game_data");
    assert!(v["reason"].as_str().unwrap_or_default().contains("not under the project root"), "the refusal says why: {v}");
    assert_eq!(reads(&app) - before, 1, "the registry was read, and the file outside the project was not");
    std::fs::remove_file(&outside_file).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn a_file_beside_a_registry_that_is_not_there_makes_the_view_look_at_nothing() {
    let (mut app, ctx) = support::editor("data_follow_neighbour_missing");
    let beside = dir(&ctx).join("data.yaml");
    assert!(!beside.exists(), "this project registers no data files");
    support::a_neighbour_is_not_read(&mut app, &beside, reads);
}
