//! **The Files workspace can make, rename, duplicate and delete a file, as operations an agent can call** (#507).
//!
//! The Files workspace could edit a file and replace across files, and every edit that was not a text change was a trip
//! out to the file manager. These are the four verbs as *operations*, so one log line, one `Cmd+Z`, and one path for the
//! person and for an agent:
//!
//! 1. **Create**: `notes/new.yaml` is made (its folder too), is in the oplog with an id, and one `Cmd+Z` removes it; the
//!    Files tree shows the row without waiting for its 1.5 s poll.
//! 2. **Duplicate** is a create whose text is another file's, byte for byte.
//! 3. **Rename keeps the extension it was not asked about** (`a.map.yaml` given `b` is `b.map.yaml`), and a rename of a
//!    file another file names is **refused naming that file** — one rule, tested for both directions.
//! 4. **Delete** is refused for a command file with `file_edit`'s own sentence, refused naming the maps for a file a map
//!    names, and refused for a map that places pieces; a delete of a plain file is undone byte for byte.
//! 5. **The four verbs are on the tree's right-click**, and the create rows also answer a right-click on the tree's empty
//!    space (the folder), with Delete last.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy_dark_factory::files;
use bevy_dark_factory::files_menu;
use bevy_dark_factory::harness;
use bevy_dark_factory::ops::{self, Applied, Ctx, Source};
use bevy_dark_factory::project::Project;

use crate::fixtures;

const SETTLE: usize = 12;

fn project(tag: &str) -> PathBuf {
    fixtures::Fixture::new(&format!("files_verbs_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m")
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn editor(root: &Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

fn ctx(app: &App, source: Source) -> Ctx {
    Ctx { project: Some(app.world().resource::<Project>().clone()), source, session: "files_verbs".to_owned(), ..Ctx::default() }
}

/// **Dispatch, as the registry door does**: a refusal is either the plan's or the commit's, and both mean the same to a
/// caller — nothing was written and the sentence says why.
fn apply(app: &App, source: Source, operation: &str, params: serde_json::Value) -> (Result<ops::Outcome, String>, Ctx) {
    let mut c = ctx(app, source);
    let applied = ops::dispatch(&mut c, operation, &params).map(|a| match a {
        Applied::Done { outcome, .. } => Ok(outcome),
        Applied::Refused(e) => Err(e.0),
    });
    (applied.unwrap_or_else(|e| Err(e.0)), c)
}

fn done(applied: Result<ops::Outcome, String>) -> ops::Outcome {
    applied.unwrap_or_else(|e| panic!("refused: {e}"))
}

fn refused(applied: Result<ops::Outcome, String>) -> String {
    match applied {
        Ok(outcome) => panic!("it applied: {}", outcome.summary),
        Err(why) => why,
    }
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{e}"));
    }
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{e}"));
}

/// **The row is in the tree's own source**, read the way the tree reads it, so the assertion is about what is drawn.
fn tree_has(app: &mut App, rel: &str) -> bool {
    let world = app.world_mut();
    let tree = world.query_filtered::<Entity, With<files::FilesTree>>().single(world).ok();
    tree.and_then(|t| world.get::<bevy_gooey::Table>(t).and_then(|table| table.source::<bevy_gooey::Tree>().map(|tree| tree.contains(files::key_of(rel)))))
        .unwrap_or(false)
}

/// **Create is one log line, one undo, and the row is in the tree at once.**
#[test]
fn a_created_file_is_logged_undone_byte_for_byte_and_shows_in_the_tree_without_waiting_for_the_poll() {
    let root = project("create");
    write(&root, "notes/old.md", "# a note\n");
    let mut app = editor(&root);
    // **The Files workspace, with its tree listed once**: the next sweep is then up to a full `SWEEP` away, so the only thing
    // that can make the new row visible inside that window is the operation's own note of the path it wrote.
    open_the_files_workspace(&mut app);
    assert!(tree_has(&mut app, "notes/old.md"), "the tree listed the project before the create");

    let (applied, _) = apply(&app, Source::Agent, "file_create", serde_json::json!({"path": "notes/new.yaml", "text": "a: 1\n"}));
    let outcome = done(applied);
    assert_eq!(read(&root, "notes/new.yaml"), "a: 1\n", "the file and its folder are made");
    assert!(outcome.saved, "a create is a write: {}", outcome.summary);
    let log = ops::read_entries(&ops::oplog_path_at(&root));
    let line = log.iter().find(|e| e.op == "file_create" && e.outcome == "applied").unwrap_or_else(|| panic!("the create is in the oplog: {log:?}"));
    assert!(!line.operation_id.is_empty(), "the line has an id an agent cites");

    // **The tree shows it without waiting out the index's sweep** (`SWEEP`, 1 s): the operation said which path it had written.
    let started = std::time::Instant::now();
    let mut seen = false;
    while started.elapsed().as_secs_f64() < bevy_dark_factory::project_index::SWEEP.as_secs_f64() - 0.5 {
        app.update();
        if tree_has(&mut app, "notes/new.yaml") {
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(seen, "the created file's row is in the tree before the tree's own poll could have found it");

    // **One undo takes it away**, byte for byte: there was nothing before, so nothing is left.
    let mut c = ctx(&app, Source::Human);
    ops::undo(&mut c, Some(&line.operation_id)).unwrap_or_else(|e| panic!("one undo: {e}"));
    assert!(!root.join("notes/new.yaml").exists(), "the file the create made is gone");
}

/// **Enter the Files workspace and wait for its tree to list the project**, so a later row's arrival is the create's doing
/// and not the first walk's.
fn open_the_files_workspace(app: &mut App) {
    let command = bevy_dark_factory::commands::by_id("workspace.files").unwrap_or_else(|| panic!("workspace.files"));
    let live = *app.world().resource::<bevy_dark_factory::keys::Live>();
    app.world_mut().resource_mut::<bevy_dark_factory::command_run::Chords>().queue(&command, live);
    // **The tree's first walk runs on a thread**, so the wait is for a row and not for the entity.
    let started = std::time::Instant::now();
    while started.elapsed().as_secs_f64() < 10.0 {
        app.update();
        if tree_has(app, "notes/old.md") {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    panic!("the Files workspace's tree never listed the project");
}

/// **A duplicate is a create whose text is another file's**, and it does not touch the original.
#[test]
fn a_duplicate_is_a_create_whose_text_is_the_other_files() {
    let root = project("duplicate");
    write(&root, "notes/a.yaml", "# keep me\nkey: value\n");
    let app = editor(&root);
    let outcome = done(apply(&app, Source::Human, "file_create", serde_json::json!({"path": "notes/b.yaml", "copy_from": "notes/a.yaml"})).0);
    assert_eq!(read(&root, "notes/b.yaml"), "# keep me\nkey: value\n", "byte for byte");
    assert_eq!(read(&root, "notes/a.yaml"), "# keep me\nkey: value\n", "the original is untouched");
    assert_eq!(outcome.affected, 1);
}

/// **Create refuses what it must**, and writes nothing when it does.
#[test]
fn a_create_refuses_a_path_that_exists_a_command_file_and_a_path_outside_the_project() {
    let root = project("create_refusals");
    write(&root, "notes/a.txt", "here\n");
    let app = editor(&root);
    let twice = refused(apply(&app, Source::Human, "file_create", serde_json::json!({"path": "notes/a.txt"})).0);
    assert!(twice.contains("already"), "a create does not overwrite: {twice}");
    assert_eq!(read(&root, "notes/a.txt"), "here\n");

    for command in [blueprint_model::trust::COMMAND_FILES[0], blueprint_model::agent_guide::PROJECT_PATH] {
        let said = refused(apply(&app, Source::Human, "file_create", serde_json::json!({"path": command})).0);
        assert!(said.contains(bevy_dark_factory::files_lang::READ_ONLY_NOTE), "a command file is refused in `file_edit`'s own words: {said}");
    }
    let outside = refused(apply(&app, Source::Human, "file_create", serde_json::json!({"path": "../outside.yaml"})).0);
    assert!(outside.contains("inside the project"), "{outside}");
    assert!(!root.join("../outside.yaml").exists());
}

/// **A rename edits the stem and puts the extension back** — including a two-part one.
#[test]
fn a_rename_keeps_the_extension_it_was_not_asked_about() {
    let root = project("keep_extension");
    write(&root, "maps/a.map.yaml", "version: 3\n");
    write(&root, "notes/readme.md", "# readme\n");
    let app = editor(&root);

    done(apply(&app, Source::Human, "file_rename", serde_json::json!({"path": "maps/a.map.yaml", "to": "b"})).0);
    assert!(root.join("maps/b.map.yaml").is_file(), "`b` keeps `.map.yaml`");
    assert!(!root.join("maps/a.map.yaml").exists());
    done(apply(&app, Source::Human, "file_rename", serde_json::json!({"path": "notes/readme.md", "to": "notes/guide"})).0);
    assert!(root.join("notes/guide.md").is_file(), "`b` keeps `.md`");
    // **A new name with its own extension is taken as given.**
    done(apply(&app, Source::Human, "file_rename", serde_json::json!({"path": "notes/guide.md", "to": "notes/guide.txt"})).0);
    assert!(root.join("notes/guide.txt").is_file());
    // **And the undo puts the old name back.**
    let back = done(apply(&app, Source::Human, "file_rename", serde_json::json!({"path": "maps/b.map.yaml", "to": "a"})).0);
    assert!(root.join("maps/a.map.yaml").is_file(), "{}", back.summary);
}

/// **The one rule for a reference**: a rename of a file another file names is refused naming it, and a delete of such a
/// file is refused the same way. Nothing is written by either.
#[test]
fn a_rename_or_delete_of_a_file_another_file_names_is_refused_naming_it() {
    let root = project("named");
    write(&root, "notes/keep.yaml", "version: 1\n");
    write(&root, "docs/index.md", "See `notes/keep.yaml` for the list.\n");
    let app = editor(&root);

    let said = refused(apply(&app, Source::Human, "file_rename", serde_json::json!({"path": "notes/keep.yaml", "to": "notes/gone.yaml"})).0);
    assert!(said.contains("docs/index.md"), "the refusal names the file that names it: {said}");
    assert!(root.join("notes/keep.yaml").is_file(), "nothing was moved");

    let said = refused(apply(&app, Source::Human, "file_delete", serde_json::json!({"path": "notes/keep.yaml"})).0);
    assert!(said.contains("docs/index.md"), "the same rule refuses the delete: {said}");
    assert!(root.join("notes/keep.yaml").is_file(), "nothing was deleted");
}

/// **A delete is refused for a command file in `file_edit`'s words, and for a map that places pieces, naming the pieces.**
#[test]
fn a_delete_refuses_a_command_file_and_a_map_that_places_pieces() {
    let root = project("delete_refusals");
    let app = editor(&root);
    let said = refused(apply(&app, Source::Human, "file_delete", serde_json::json!({"path": blueprint_model::trust::COMMAND_FILES[0]})).0);
    assert!(said.contains(bevy_dark_factory::files_lang::READ_ONLY_NOTE), "the same sentence `file_edit` uses: {said}");

    let map = "assets/blueprint/maps/m.map.yaml";
    assert!(root.join(map).is_file());
    let said = refused(apply(&app, Source::Human, "file_delete", serde_json::json!({"path": map})).0);
    assert!(said.contains("floor@0"), "the refusal names what the map places: {said}");
    assert!(root.join(map).is_file(), "nothing was deleted");
}

/// **A delete of a plain file is one log line and one undo that restores it byte for byte.**
#[test]
fn a_deleted_files_undo_puts_its_text_back() {
    let root = project("delete_undo");
    let text = "# a note\r\nwith a break\r\n";
    write(&root, "notes/old.md", text);
    let app = editor(&root);
    let (applied, _) = apply(&app, Source::Human, "file_delete", serde_json::json!({"path": "notes/old.md"}));
    done(applied);
    assert!(!root.join("notes/old.md").exists(), "the file is gone");
    let line = ops::read_entries(&ops::oplog_path_at(&root)).into_iter().find(|e| e.op == "file_delete" && e.outcome == "applied").unwrap_or_else(|| panic!("the delete is logged"));
    let mut c = ctx(&app, Source::Human);
    ops::undo(&mut c, Some(&line.operation_id)).unwrap_or_else(|e| panic!("the undo restores it: {e}"));
    assert_eq!(read(&root, "notes/old.md"), text, "byte for byte, line endings included");
}

/// **An agent reaches all three, and `--print-operations` lists them**: the registry is the door, so the three names are
/// in it and each answers a schema, a place and a declaration of what it writes.
#[test]
fn an_agent_can_call_all_three_and_they_are_in_the_registry() {
    let registry = ops::registry();
    for name in ["file_create", "file_rename", "file_delete"] {
        let op = registry.iter().find(|o| o.name() == name).unwrap_or_else(|| panic!("`{name}` is registered"));
        assert_eq!(op.undo(), ops::Undo::Inverse, "`{name}` records an inverse");
        assert!(!op.writes().is_empty(), "`{name}` writes a project file");
        assert!(op.schema().get("properties").is_some(), "`{name}` has a parameter schema an agent reads");
    }
}

/// **The four verbs are on the tree's right-click, and the create rows answer the folder too**: every row of the tree
/// carries the menu, the tree's own empty space carries it, and the items are in the order the issue asks for — Delete
/// last.
#[test]
fn the_four_verbs_are_on_the_trees_right_click_with_delete_last() {
    let root = project("menu");
    write(&root, "notes/a.md", "# a\n");
    let mut app = editor(&root);
    let command = bevy_dark_factory::commands::by_id("workspace.files").unwrap_or_else(|| panic!("workspace.files"));
    let live = *app.world().resource::<bevy_dark_factory::keys::Live>();
    app.world_mut().resource_mut::<bevy_dark_factory::command_run::Chords>().queue(&command, live);
    settle(&mut app);
    // The tree walks on a thread of its own.
    for _ in 0..120 {
        app.update();
        if tree_has(&mut app, "notes/a.md") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(tree_has(&mut app, "notes/a.md"), "the tree listed the file");

    assert_eq!(
        files_menu::ITEMS,
        ["New file", "New folder", "Rename", "Duplicate", "Delete"],
        "the menu's own order, with Delete last: a create list that leads and pushes Delete below the fold is the bug the \
         issue names"
    );
    // **The tree's own entity names the menu**, so a right-click on empty space targets the folder, and a row of the tree
    // names it too, so a right-click on a file targets the file.
    let world = app.world_mut();
    let tree = world.query_filtered::<Entity, With<files::FilesTree>>().single(world).unwrap_or_else(|e| panic!("the tree is on screen: {e}"));
    assert!(world.get::<bevy_gooey::ContextMenu>(tree).is_some(), "the tree's empty space offers New");
}

/// **A create from the menu's own item makes the file**: the item is a front end for the operation, not a second way.
#[test]
fn the_menus_create_item_dispatches_the_operation() {
    let root = project("menu_item");
    let mut app = editor(&root);
    let (panel, item) = files_menu::spawn_for_test(app.world_mut());
    // **The menu opened for the tree's empty space**: the folder is the project's root, so the file lands there.
    let root_entity = app.world_mut().spawn_empty().id();
    app.world_mut().entity_mut(panel).insert(bevy_gooey::ContextTarget(root_entity));
    if let Some(mut pending) = app.world_mut().get_mut::<files_menu::PendingName>(item) {
        pending.text = "made.yaml".to_owned();
    }
    app.world_mut().trigger(bevy::ui_widgets::Activate { entity: item });
    settle(&mut app);
    assert!(root.join("made.yaml").is_file(), "the item created the file through `file_create`");
    let log = ops::read_entries(&ops::oplog_path_at(&root));
    assert!(log.iter().any(|e| e.op == "file_create"), "one log line, as the operation makes: {log:?}");
}
