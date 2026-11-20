//! **The Files tree's right-click menu**: New file, New folder, Rename, Duplicate, Delete (#507).
//!
//! The tree had no right-click at all: a person who wanted a `notes/` folder or a second copy of a YAML file did it in their
//! file manager and the tree redrew a poll later. This is the menu, and **every item is a front end for an operation**
//! ([`crate::ops_files_verbs`]), so a menu choice is one log line and one `Cmd+Z`, the same as an agent's call:
//!
//! | item | dispatches |
//! |---|---|
//! | New file | `file_create` with the typed name |
//! | New folder | `file_create` of `<folder>/.gitkeep`, which is how a folder is made (a folder has no operation of its own) |
//! | Rename | `file_rename` with the typed name; the extension is kept |
//! | Duplicate | `file_create` with `copy_from` the file's own path |
//! | Delete | `file_delete` |
//!
//! # Delete is last
//!
//! The issue's own argument: leading with the creates pushes Delete below a fixed menu's height, so the thing a person
//! right-clicked a file to do ends up behind a scroll. One row each, Delete last.
//!
//! # The folder is the target of a right-click on empty space
//!
//! gooey opens a menu at the entity the click reached, and the tree's own node carries this menu, so a right-click that hits
//! no row reaches the tree — the folder. A row of the tree names the same menu, so a right-click on a file reaches that row,
//! and [`row_of`] reads which file it was from [`crate::files::TreeNames`] — the tree's own record of what each row key
//! names, so the menu cannot disagree with the row it is drawn on.
//!
//! # A name is typed on the menu row, not in a second dialog
//!
//! `bevy_gooey` has no prompt widget, and a modal for one word is heavier than the row that asks for it. Each naming item is
//! a row carrying a [`text_input`]: typing keeps the name on the row ([`PendingName`]) and the row's own click runs it. A
//! menu that closes loses the draft, which is what closing a menu means.

use bevy::prelude::*;
use bevy::ui_widgets::{Activate, ValueChange};
use bevy_gooey::{ContextMenu, ContextTarget, context_menu, menu_item, observe, separator, text_input};

use crate::editor::EditorState;
use crate::files::{FilesTree, TreeNames};
use crate::project::Project;

/// **The menu's items, top to bottom**, as the words a person reads. One table, so the menu and its test read the same list.
pub const ITEMS: [&str; 5] = ["New file", "New folder", "Rename", "Duplicate", "Delete"];

/// What a naming row is for: the operation it runs and the thing it is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Naming {
    /// Make this file. `dir` makes a folder (by the `.gitkeep` inside it); `dir_of` is the folder it goes in, empty for the
    /// project's root.
    Create { dir: bool, dir_of: String },
    /// Rename the file at this path.
    Rename(String),
}

/// **The name typed on a naming row**, and what the row is for. A component and not a resource: it is one row's own draft,
/// and the menu closing takes it with it.
#[derive(Component, Clone, Debug)]
pub struct PendingName {
    pub what: Naming,
    pub text: String,
}

/// The menu's panel.
#[derive(Component)]
pub struct FilesMenu;

/// **A duplicate row** and **a delete row**, so an observer can tell them apart by component.
#[derive(Component)]
pub struct DuplicateItem;
/// The Delete row.
#[derive(Component)]
pub struct DeleteItem;

/// The menu's entity, so the tree and its rows can name it.
#[derive(Resource, Default, Clone, Copy)]
pub struct FilesMenuHandle(pub Option<Entity>);

pub struct FilesMenuPlugin;

impl Plugin for FilesMenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FilesMenuHandle>()
            .add_systems(OnEnter(crate::screen::Screen::Editor), spawn_menu)
            .add_systems(
                Update,
                (name_the_menu_on_the_tree, name_the_menu_on_rows).run_if(in_state(crate::screen::Screen::Editor)),
            );
    }
}

/// **Build the one menu** the tree and every row of it name, on entering the editor: the screen's sweep takes it when the
/// door closes, so the next door needs its own.
fn spawn_menu(mut commands: Commands, mut handle: ResMut<FilesMenuHandle>) {
    let menu = commands
        .spawn((
            FilesMenu,
            context_menu(children![
                (text_input("New file", ""), PendingName { what: Naming::Create { dir: false, dir_of: String::new() }, text: String::new() }, observe(on_name_typed), observe(run_the_naming)),
                (text_input("New folder", ""), PendingName { what: Naming::Create { dir: true, dir_of: String::new() }, text: String::new() }, observe(on_name_typed), observe(run_the_naming)),
                (text_input("Rename", ""), PendingName { what: Naming::Rename(String::new()), text: String::new() }, observe(on_name_typed), observe(run_the_naming)),
                separator(),
                (menu_item(ITEMS[3]), DuplicateItem, observe(run_the_duplicate)),
                (menu_item(ITEMS[4]), DeleteItem, observe(run_the_delete)),
            ]),
        ))
        .id();
    handle.0 = Some(menu);
}

/// **The tree's own node names the menu**, so a right-click on the tree's empty space targets the folder.
fn name_the_menu_on_the_tree(handle: Res<FilesMenuHandle>, trees: Query<(Entity, Option<&ContextMenu>), With<FilesTree>>, mut commands: Commands) {
    let Some(menu) = handle.0 else { return };
    for (tree, named) in &trees {
        if named.is_some_and(|c| c.0 == menu) {
            continue;
        }
        commands.entity(tree).try_insert(ContextMenu(menu));
    }
}

/// **Every row of the tree names the same menu.** The rows are a pool — the entity showing a file now may show a folder the
/// next frame — so the menu is put on each row and never taken off: what an item does is decided from the row's own key at
/// the click, not from which row it was when the menu was named.
fn name_the_menu_on_rows(handle: Res<FilesMenuHandle>, trees: Query<Entity, With<FilesTree>>, rows: Query<(Entity, &bevy_gooey::TableRow, Option<&ContextMenu>)>, mut commands: Commands) {
    let Some(menu) = handle.0 else { return };
    let Ok(tree) = trees.single() else { return };
    for (row, table_row, named) in &rows {
        if table_row.table != tree || named.is_some_and(|c| c.0 == menu) {
            continue;
        }
        commands.entity(row).try_insert(ContextMenu(menu));
    }
}

/// **What the menu was opened for**: the path under the project and whether it is a folder. `None` is the tree's empty space,
/// which is the project's root folder.
fn row_of(entity: Entity, parents: &Query<&ChildOf>, targets: &Query<&ContextTarget>, rows: Query<&bevy_gooey::TableRow>, trees: Query<&TreeNames>) -> Option<(String, bool)> {
    let target = ContextTarget::of(entity, parents, targets)?;
    // **Only a `TableRow` names a file**: the tree's own node, and anything above a row, is the folder.
    let row = rows.get(target).ok()?;
    trees.iter().next()?.names.get(&row.key).cloned()
}

/// **The person typed a name into a naming row.** Kept on the row, so the item that runs knows what to make.
fn on_name_typed(change: On<ValueChange<String>>, mut pending: Query<&mut PendingName>) {
    if let Ok(mut pending) = pending.get_mut(change.source) {
        pending.text = change.value.clone();
    }
}

/// **Run a naming row**: New file, New folder or Rename, through its operation.
fn run_the_naming(activate: On<Activate>, mut names: Query<&mut PendingName>, parents: Query<&ChildOf>, targets: Query<&ContextTarget>, rows: Query<&bevy_gooey::TableRow>, trees: Query<&TreeNames>, mut commands: Commands) {
    let Ok(pending) = names.get_mut(activate.entity) else { return };
    let typed = pending.text.trim().to_owned();
    if typed.is_empty() {
        return;
    }
    let what = pending.what.clone();
    // **The folder the new file goes in**: the right-clicked row when it is a folder, its parent when it is a file, and the
    // project's root when the click reached no row at all.
    let target = row_of(activate.entity, &parents, &targets, rows, trees);
    let (dir_of, on_a_folder) = match &target {
        Some((rel, true)) => (rel.clone(), true),
        Some((rel, false)) => (folder_of(rel), false),
        None => (String::new(), true),
    };
    commands.queue(move |world: &mut World| {
        let dispatched = match &what {
            Naming::Create { dir, .. } => {
                let rel = join(&dir_of, &typed);
                // **A folder is made by the file inside it**: `file_create` makes one file, and a `.gitkeep` is the file that
                // keeps an otherwise empty folder in a project under version control.
                let path = if *dir { join(&rel, GITKEEP) } else { rel };
                ("file_create", serde_json::json!({"path": path, "text": ""}))
            }
            // **Rename acts on a file row**, and its own operation says so when the row is not one.
            Naming::Rename(_) => match &target {
                Some((rel, false)) => ("file_rename", serde_json::json!({"path": rel, "to": typed})),
                _ => ("file_rename", serde_json::json!({"path": typed, "to": typed})),
            },
        };
        let _ = on_a_folder;
        dispatch_and_say(world, dispatched.0, dispatched.1);
    });
}

/// **Run Duplicate**: the file under the right-click, made again beside itself.
fn run_the_duplicate(activate: On<Activate>, parents: Query<&ChildOf>, targets: Query<&ContextTarget>, rows: Query<&bevy_gooey::TableRow>, trees: Query<&TreeNames>, mut commands: Commands) {
    let Some((rel, is_dir)) = row_of(activate.entity, &parents, &targets, rows, trees) else { return };
    if is_dir {
        return;
    }
    // **Beside itself, with ` copy` in the name.** `file_create` refuses a path already there rather than overwriting, so a
    // second Duplicate is refused naming the copy: that refusal is the honest answer, where a number in the name would be the
    // editor guessing how many copies a person wants.
    let copy = copy_name(&rel);
    commands.queue(move |world: &mut World| {
        dispatch_and_say(world, "file_create", serde_json::json!({"path": copy, "copy_from": rel}));
    });
}

/// **Run Delete**: the file under the right-click, through its operation, which is where the guards are.
fn run_the_delete(activate: On<Activate>, parents: Query<&ChildOf>, targets: Query<&ContextTarget>, rows: Query<&bevy_gooey::TableRow>, trees: Query<&TreeNames>, mut commands: Commands) {
    let Some((rel, is_dir)) = row_of(activate.entity, &parents, &targets, rows, trees) else { return };
    if is_dir {
        return;
    }
    commands.queue(move |world: &mut World| {
        dispatch_and_say(world, "file_delete", serde_json::json!({"path": rel}));
    });
}

/// The file that makes an otherwise empty folder a folder a project keeps.
pub const GITKEEP: &str = ".gitkeep";

/// **`path copy`**, in the same folder and with the extension kept: `notes/a.md` becomes `notes/a copy.md`.
pub fn copy_name(rel: &str) -> String {
    let (folder, name) = match rel.rsplit_once('/') {
        Some((folder, name)) => (Some(folder), name),
        None => (None, rel),
    };
    let made = match name.split_once('.') {
        Some((stem, extension)) => format!("{stem} copy.{extension}"),
        None => format!("{name} copy"),
    };
    match folder {
        Some(folder) => format!("{folder}/{made}"),
        None => made,
    }
}

/// **The folder a path is in**, or empty for the project's root.
fn folder_of(rel: &str) -> String {
    rel.rsplit_once('/').map(|(folder, _)| folder.to_owned()).unwrap_or_default()
}

/// **`folder/name`**, or `name` at the root: one join, so a folder never becomes `//name`.
fn join(folder: &str, name: &str) -> String {
    let folder = folder.trim_end_matches('/');
    if folder.is_empty() { name.to_owned() } else { format!("{folder}/{name}") }
}

/// **Dispatch an operation from the menu and say what it said**, on the status line — the same two lines every other menu of
/// the editor ends with, so a refusal is read where a refusal is always read.
fn dispatch_and_say(world: &mut World, operation: &str, params: serde_json::Value) {
    let Some(project) = world.get_resource::<Project>().cloned() else { return };
    let mut ctx = crate::ops::Ctx { project: Some(project), source: crate::ops::Source::Human, session: crate::ops::session_id(), ..crate::ops::Ctx::default() };
    let said = match crate::ops::dispatch(&mut ctx, operation, &params) {
        Ok(crate::ops::Applied::Done { outcome, operation_id, .. }) => Ok((outcome.summary.clone(), operation_id)),
        Ok(crate::ops::Applied::Refused(e)) | Err(e) => Err(e.0),
    };
    if let Some(mut state) = world.get_resource_mut::<EditorState>() {
        match said {
            Ok((summary, id)) => {
                state.status.note(summary.clone());
                state.record_logged(id, summary, false);
            }
            Err(why) => state.status.problem(why),
        }
    }
}

/// **A menu and its New-file row, for a test.** `(panel, new_file_row)`.
#[doc(hidden)]
pub fn spawn_for_test(world: &mut World) -> (Entity, Entity) {
    let panel = world.spawn((FilesMenu, context_menu(()))).id();
    let item = world
        .spawn((
            ChildOf(panel),
            text_input("New file", ""),
            PendingName { what: Naming::Create { dir: false, dir_of: String::new() }, text: String::new() },
            observe(on_name_typed),
            observe(run_the_naming),
        ))
        .id();
    (panel, item)
}
