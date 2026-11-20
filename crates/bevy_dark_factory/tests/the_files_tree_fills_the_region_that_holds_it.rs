//! **The file tree fills the space it is given** (#524).
//!
//! The owner's note, from `feedback-1795094312811`: *"Why doesn't the file explorer fill this div? It's not
//! using our layout space best."*
//!
//! **What is responsible, measured rather than guessed.** `bevy_gooey::tree` is `table_bundle`, whose root
//! states `flex_direction: Column` and **no height at all**; `Role::Table` virtualizes, which needs a bounded
//! height to lay rows into. `files.rs`'s `spawn_the_files_workspace` puts the tree straight into a
//! `fill_region` and states no size for it, so the tree lays out at its content's height instead of the
//! region's — measured here at **245 px inside a 356 px region**.
//!
//! **The fix is not in this repo.** `sized()` and `tree()` both provide `Node` and `Role`, so a bundle cannot
//! carry both on one entity, and a wrapping `sized()` does not reach the tree (measured: the wrapper at
//! 100 %, the tree still 245 px). The node with no height was the table's own root, so the fix is in
//! `bevy_gooey` — **bevy_gooey#111**: `flex_grow: 1.0` and `min_height: 0` on `table_bundle`'s root. This
//! test is the proof from this repo's side, and by construction it covers the other three call sites of the
//! same shape: `outliner.rs`, `design_view.rs` and `files_panel.rs` each spawn `tree()` bare inside a
//! `fill_region`.

use std::time::Duration;

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;
use bevy::ui::ComputedNode;
use bevy::window::{PrimaryWindow, Window, WindowResolution};

use bevy_dark_factory::files::FilesTree;
use bevy_dark_factory::harness;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// One frame at 60 Hz.
const FRAME: f32 = 1.0 / 60.0;

/// Frames to let the door's first layout settle before a size is read.
const SETTLE: usize = 60;

/// The window's width, and the two heights the tree is measured at. A tree that fills is the region's
/// height at both and grows between them; a tree at its content's height is shorter than the region and
/// reads the same at both.
const WIDTH: f32 = 1280.0;
const SHORT_H: f32 = 640.0;
const TALL_H: f32 = 900.0;

/// Folders that give the tree more rows than a short window shows.
const FILLER_FOLDERS: usize = 12;

const TARGET: &str = "zz/deep/nest/target.yaml";

fn project(tag: &str) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("lamp_tall", "alpha")
        .build("m");
    for n in 0..FILLER_FOLDERS {
        let dir = root.join(format!("docs_{n:02}"));
        std::fs::create_dir_all(&dir).expect("filler folder");
        std::fs::write(dir.join("note.txt"), "filler\n").expect("filler file");
    }
    let target = root.join(TARGET);
    std::fs::create_dir_all(target.parent().expect("parent")).expect("target folder");
    std::fs::write(target, "# a file the agent edits\na: 1\nb: 2\n").expect("target");
    root
}

/// The editor at one window size.
///
/// The headless harness builds no window, so one is spawned with the size the editor is measured at, the
/// same way `no_panel_draws_a_control_past_its_own_content_box` stages its own (#494). `DebuggerPlugin` and
/// `OpsBridgePlugin` are the agent's door, which is how a project file is opened.
fn editor_at(root: &std::path::Path, width: f32, height: f32) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(width as u32, height as u32),
                ..default()
            },
            PrimaryWindow,
        ))
        .id();
    let mut window_ref = app
        .world_mut()
        .get_mut::<Window>(window)
        .expect("the window just spawned");
    window_ref.resolution.set_scale_factor_override(Some(1.0));
    harness::resize_surface(&mut app, width as u32, height as u32).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    app
}

/// Open the Files door and let its layout settle.
fn the_files_workspace(app: &mut App) {
    apply(app, "door_open", serde_json::json!({ "door": "files" }));
    for _ in 0..SETTLE {
        app.update();
    }
    assert_eq!(
        *app.world().resource::<Workspace>(),
        Workspace::Files,
        "the test is on the Files door"
    );
}

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) {
    let Some(RemoteMethodSystemId::Instant(id)) = app
        .world()
        .resource::<RemoteMethods>()
        .get("blueprint/operation_apply")
        .cloned()
    else {
        panic!("blueprint/operation_apply is not registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(
            id,
            Some(serde_json::json!({
                "operation": operation,
                "parameters": parameters,
                "dry_run": false
            })),
        )
        .unwrap_or_else(|e| panic!("{operation}: {e:?}"))
        .unwrap_or_else(|e| panic!("{operation}: {e:?}"));
}

/// The tree's height and the height of the fill region that holds it.
fn tree_and_its_region(app: &mut App) -> Option<(f32, f32)> {
    let tree = {
        let mut query = app
            .world_mut()
            .query_filtered::<Entity, With<FilesTree>>();
        query.iter(app.world()).next()?
    };
    let tree_h = app
        .world()
        .get::<ComputedNode>(tree)
        .map(|n| n.size().y)?;
    let region = app
        .world()
        .get::<ChildOf>(tree)
        .map(|p| p.parent())?;
    let region_h = app
        .world()
        .get::<ComputedNode>(region)
        .map(|n| n.size().y)?;
    Some((tree_h, region_h))
}

/// **The tree's height is the region's height**, not its content's.
#[test]
fn the_files_tree_fills_the_region_that_holds_it() {
    let root = project("files-tree-fills-its-region");

    let mut short = editor_at(&root, WIDTH, SHORT_H);
    the_files_workspace(&mut short);
    let (short_tree, short_region) =
        tree_and_its_region(&mut short).expect("the Files door lays out a tree inside a fill region");

    assert!(
        short_region > 0.0,
        "the region has a height to fill: {short_region}"
    );
    assert!(
        (short_tree - short_region).abs() <= 1.0,
        "**the file tree is {short_tree} px in a region {short_region} px tall** (#524): it lays out at its \
         content's height instead of filling the space. `bevy_gooey::tree` is a virtualized table whose root \
         stated no height; that is bevy_gooey#111, and this repo picks the fix up by moving its pin."
    );

    let mut tall = editor_at(&root, WIDTH, TALL_H);
    the_files_workspace(&mut tall);
    let (tall_tree, tall_region) =
        tree_and_its_region(&mut tall).expect("the Files door lays out a tree inside a fill region");

    assert!(
        tall_region > short_region,
        "a taller window gives the region more height: {short_region} -> {tall_region}"
    );
    assert!(
        (tall_tree - tall_region).abs() <= 1.0,
        "**the file tree is {tall_tree} px in a region {tall_region} px tall at {TALL_H}** (#524): a tree \
         that filled would have grown with the region."
    );
}
