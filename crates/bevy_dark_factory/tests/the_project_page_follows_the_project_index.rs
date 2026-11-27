//! **The Project page notices a map made or removed elsewhere through the index, not on a clock** (#538;
//! `project_page::WATCH_SECS` retired).
//!
//! The page used to list the blueprint folder and the maps folder every second. It reads the index's listing now: the open project's index
//! when it follows the page's project, and, when no project is open (the page is shown before one is), the blueprint folder followed as a
//! second root. Both tests change the maps folder with plain `std::fs` and wait, real time a counted number of frames apart, for the
//! page's catalog to agree with a listing the test reads itself. The clock is frozen (a manual time step of nothing) and the index's
//! sweep is at its floor, so a page that still waited for a second of `Time` could not pass.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::harness;
use bevy_dark_factory::launcher::LauncherPlugin;
use bevy_dark_factory::loading::LoadingPlugin;
use bevy_dark_factory::project::Project;
use bevy_dark_factory::project_index::{MIN_SWEEP, OutsideFiles, ProjectIndex};
use bevy_dark_factory::project_page::ProjectPage;

use crate::fixtures::Fixture;
use crate::settle_support::Frames;

/// The pause between frames while the page is waited for.
const PAUSE: Duration = Duration::from_millis(10);
/// How long a test waits for the page to follow a file: frames covering 30 s at [`PAUSE`], counted rather than timed so a starved
/// host cannot run it out (#644).
const WAIT: Frames = Frames::covering(Duration::from_secs(30), PAUSE);

fn on_the_page(tag: &str) -> (PathBuf, App) {
    let root = Fixture::new(&format!("page_follows_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LoadingPlugin);
    app.add_plugins(LauncherPlugin);
    harness::stand_on_the_project_page(&mut app, &root);
    (root, app)
}

fn maps_dir(root: &Path) -> PathBuf {
    root.join("assets/blueprint/maps")
}

/// **The maps on disk, by a plain read of the folder**: the stem of each `*.map.yaml`.
fn maps_on_disk(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(maps_dir(root))
        .unwrap_or_else(|e| panic!("{e}"))
        .flatten()
        .filter_map(|e| e.file_name().to_str()?.strip_suffix(".map.yaml").map(str::to_owned))
        .collect();
    names.sort();
    names
}

fn maps_on_the_page(app: &App) -> Vec<String> {
    let mut names: Vec<String> = app.world().resource::<ProjectPage>().catalog.maps.iter().map(|m| m.name.clone()).collect();
    names.sort();
    names
}

fn until_the_page_agrees(app: &mut App, root: &Path, what: &str) {
    for _ in 0..WAIT.0 {
        app.update();
        if maps_on_the_page(app) == maps_on_disk(root) {
            return;
        }
        std::thread::sleep(PAUSE);
    }
    panic!("never happened within {WAIT:?}: {what}: the page lists {:?}, the folder holds {:?}", maps_on_the_page(app), maps_on_disk(root));
}

fn made_and_removed(app: &mut App, root: &Path) {
    assert_eq!(maps_on_the_page(app), maps_on_disk(root), "the page starts in agreement with the folder");
    let maps = maps_dir(root);
    std::fs::copy(maps.join("m.map.yaml"), maps.join("n.map.yaml")).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(maps_on_disk(root), ["m", "n"], "the test made a second map");
    until_the_page_agrees(app, root, "a map made elsewhere is on the page");
    std::fs::remove_file(maps.join("n.map.yaml")).unwrap_or_else(|e| panic!("{e}"));
    until_the_page_agrees(app, root, "a map removed elsewhere is off the page");
}

/// **With the project open, the page follows the open project's index.**
#[test]
fn a_map_made_or_removed_elsewhere_reaches_the_page_through_the_open_projects_index() {
    let (root, mut app) = on_the_page("open");
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    app.world().resource::<ProjectIndex>().set_sweep(MIN_SWEEP);
    made_and_removed(&mut app, &root);
}

/// **With no project open, the page follows the blueprint folder as a second root.**
#[test]
fn a_map_made_or_removed_elsewhere_reaches_the_page_before_a_project_is_open() {
    let (root, mut app) = on_the_page("closed");
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    // No project is open, so no project index follows the page's root.
    app.world_mut().remove_resource::<Project>();
    for _ in 0..4 {
        app.update();
    }
    assert!(app.world().resource::<ProjectIndex>().root().is_none(), "no project index follows a root now");
    let blueprint = root.join("assets/blueprint");
    app.world().resource::<OutsideFiles>().set_sweep(&blueprint, MIN_SWEEP);
    made_and_removed(&mut app, &root);
}
