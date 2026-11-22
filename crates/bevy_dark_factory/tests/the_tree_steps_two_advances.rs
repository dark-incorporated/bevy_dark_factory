//! **A tree row and a file row step the same distance** (#523, `feedback-1795094264493`).
//!
//! > The collapasable items seem to be 2 space indented, but the files seem to be 4 space. Please be
//! > consistent.
//!
//! # What the capture measured
//!
//! The Files tree at `ui_scale` 2.4 (1088 x 703 logical, 2176 x 1406 physical) drew the same tree two
//! ways. A level in the tree stepped **24 logical px** — 3.64 advances of the 11 px FiraMono body —
//! while the file editor beside it indents YAML by `files::YAML_INDENT_SPACES`, two advances, 13.2 px.
//! The eye read a 2-space step at the top of the tree and a 4-space step under it, in one list.
//!
//! # Why there was no constant
//!
//! #489 asked for exactly this and closed citing a commit that changed nothing here. The editor had
//! no tree step of its own: `bevy_gooey` sizes a level at `tree_indent(text_px)` = `1.5 x the text
//! size`, and writes that onto every `TreeTwisty`. `chrome::TREE_STEP` is the step the editor states,
//! and `chrome::the_tree_steps_two_advances` writes it after gooey has had its say.
//!
//! # What this measures
//!
//! The **twisty's own left margin** — the depth gooey placed the row at, rewritten to the editor's
//! step. Reading the placement rather than the drawn text keeps this free of pooled-row reuse and of
//! the cell's own inset. One level deeper must stand exactly `TREE_STEP` further in, at the interface
//! scale the capture was taken at and at the design's own.

use std::time::Duration;

use bevy::prelude::*;
use bevy::ui::UiScale;
use bevy::window::{PrimaryWindow, Window, WindowResolution};

use bevy_dark_factory::chrome;
use bevy_dark_factory::files::{self, FilesTree};
use bevy_dark_factory::harness;

use crate::fixtures;

/// The window the owner's capture was taken in, physical px.
const CAPTURE_WINDOW: (u32, u32) = (2176, 1406);

/// The interface scales this holds at: the design's own, and the capture's.
const SCALES: [f32; 2] = [1.0, 1.5];

/// Frames a layout needs to settle after the scale changes and the tree walks.
const SETTLE: usize = 40;

/// How far a drawn edge may stand from where the arithmetic puts it, logical px: layout rounds.
const SLACK_PX: f32 = 0.6;

/// **Two rows at the SAME depth, of the two kinds the capture names**: a folder that opens, and a
/// plain file. Comparing rows at different depths would measure the step itself, not the defect.
const FOLDER: &str = "notes/deep";
const FILE: &str = "notes/one.md";
/// One level deeper than either, so a step right at one depth and wrong at the next is caught.
const DEEPER: &str = "notes/deep/two.md";

fn project(tag: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    for (rel, text) in [(FILE, "# one\n"), ("notes/two.md", "# two\n"), (DEEPER, "# deep\n")] {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap_or_else(|| panic!("{rel} has a parent"))).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(path, text).unwrap_or_else(|e| panic!("{e}"));
    }
    root
}

fn editor(root: &std::path::Path, scale: f32) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window { resolution: WindowResolution::new(CAPTURE_WINDOW.0, CAPTURE_WINDOW.1), ..default() }, PrimaryWindow));
    harness::resize_surface(&mut app, CAPTURE_WINDOW.0, CAPTURE_WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    for _ in 0..SETTLE {
        app.update();
    }
    app
}

/// The Files door, open, with the rows this test reads drawn.
fn files_tree(app: &mut App) -> Entity {
    let command = bevy_dark_factory::commands::by_id("workspace.files").unwrap_or_else(|| panic!("the Files command"));
    let live = *app.world().resource::<bevy_dark_factory::keys::Live>();
    app.world_mut().resource_mut::<bevy_dark_factory::command_run::Chords>().queue(&command, live);
    for _ in 0..SETTLE {
        app.update();
    }
    let tree = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<FilesTree>>().single(world).unwrap_or_else(|e| panic!("the Files door is open: {e}"))
    };
    // The walk is a thread on a timer: step until the rows this test reads are there.
    for _ in 0..300 {
        let there = {
            let table = app.world().get::<bevy_gooey::Table>(tree);
            let keys = [files::key_of(FOLDER), files::key_of(FILE), files::key_of(DEEPER)];
            table.and_then(|t| t.source::<bevy_gooey::Tree>().map(|t| keys.iter().all(|k| t.contains(*k)))).unwrap_or(false)
        };
        if there {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
        app.update();
    }
    // Open the folders above the rows this test reads, through the tree's own state.
    {
        let world = app.world_mut();
        if let Some(mut table) = world.get_mut::<bevy_gooey::Table>(tree)
            && let Some(t) = table.source_mut::<bevy_gooey::Tree>()
        {
            for key in [files::key_of("notes"), files::key_of(FOLDER)] {
                t.set_open(key, true);
            }
        }
    }
    for _ in 0..SETTLE {
        app.update();
    }
    tree
}

/// **How far in the tree placed `key`'s row, logical px**: the left margin of its own `TreeTwisty`.
fn depth_offset(app: &mut App, tree: Entity, key: u64) -> f32 {
    let world = app.world_mut();
    let row = {
        let mut q = world.query::<(Entity, &bevy_gooey::TableRow)>();
        q.iter(world).find(|(_, r)| r.table == tree && r.key == key).map(|(e, _)| e)
    };
    let row = row.unwrap_or_else(|| panic!("the tree has a row for key {key}"));
    let mut q = world.query::<(Entity, &bevy_gooey::Role, &Node)>();
    let mut found = None;
    for (e, role, node) in q.iter(world) {
        if *role != bevy_gooey::Role::TreeTwisty {
            continue;
        }
        let mut at = e;
        loop {
            if at == row {
                found = Some(node.margin.left);
                break;
            }
            match world.get::<ChildOf>(at) {
                Some(p) => at = p.parent(),
                None => break,
            }
        }
        if found.is_some() {
            break;
        }
    }
    // **`Val::Px` is already logical px**: `UiScale` multiplies it at layout, not in the component.
    match found.unwrap_or_else(|| panic!("the row for key {key} has a twisty")) {
        Val::Px(px) => px,
        other => panic!("the twisty's margin is a px value, not {other:?}"),
    }
}

/// **One level deeper is one `TREE_STEP` further in** (#523), for a folder's row and a file's alike.
#[test]
fn the_tree_steps_two_advances() {
    let root = project("tree523");
    for scale in SCALES {
        let mut app = editor(&root, scale);
        let tree = files_tree(&mut app);
        let folder = depth_offset(&mut app, tree, files::key_of(FOLDER));
        let file = depth_offset(&mut app, tree, files::key_of(FILE));
        let deeper = depth_offset(&mut app, tree, files::key_of(DEEPER));
        // **The two kinds stand at the same depth**, which is what makes them comparable: this is
        // the capture's folder and its file, one level in, and the defect is that they did not line
        // up before the fix.
        assert!(
            (folder - file).abs() <= SLACK_PX,
            "at scale {scale}: the folder `{FOLDER}` sits {folder:.2} in and the file `{FILE}` sits {file:.2} — one depth, two places"
        );
        // **And the step is the editor's own**: two advances of the body face, the file editor's
        // YAML indent.
        let want = bevy_dark_factory::chrome::tree_step();
        assert!(
            ((deeper - folder) - want).abs() <= SLACK_PX,
            "at scale {scale}: folder {folder:.2}, file {file:.2}, deeper {deeper:.2}; one level deeper stands {:.2} further in, and the editor's step is {want:.2}",
            deeper - folder
        );
    }
}

/// **The step is the file editor's own, named once** (#523): `tree_step()` is `YAML_INDENT_SPACES`
/// advances of the body face, so a tree and the YAML in the editor beside it indent by one rule.
#[test]
fn the_tree_step_is_the_file_editors_two_spaces() {
    assert_eq!(chrome::tree_step(), files::YAML_INDENT_SPACES as f32 * chrome::body_char_w());
    assert_eq!(files::YAML_INDENT_SPACES, 2, "the file editor indents YAML and Markdown by two spaces");
}
