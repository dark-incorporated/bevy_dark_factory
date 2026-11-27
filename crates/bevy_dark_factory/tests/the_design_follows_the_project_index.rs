//! **The Design workspace and the areas overlay follow the design files through the project index, and not on a clock** (#538;
//! `design_view::REFRESH_S` and `design_areas::REFRESH_S` retired).
//!
//! Both used to read the design tree (the page and its board; the level's areas) every half second. They read when the index says a file under
//! `design/` moved. These tests write files the way a person's own editor or an agent does (plain `std::fs`) and wait, in frames that sleep, for
//! the editor to agree with the file read back with `std::fs`. The clock is frozen (a manual time step of nothing) and the index's sweep is at
//! its floor, so a reader that still waited for half a second of `Time` could not pass, and only the index's debounce stands between a write and
//! the screen.

use std::path::Path;
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::design_areas::StyleAreas;
use bevy_dark_factory::design_view::{DesignView, Page};
use bevy_dark_factory::harness;
use bevy_dark_factory::project_index::{MIN_SWEEP, ProjectIndex};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;
use crate::settle_support::Frames;

/// The wall-clock pause between the frames [`until`] steps.
const PAUSE: Duration = Duration::from_millis(10);
/// How long a test waits for the editor to follow a file: frames, not seconds, covering at least 30 s at [`PAUSE`], so a
/// starved host does not run it out early (#644).
const WAIT: Frames = Frames::covering(Duration::from_secs(30), PAUSE);

fn write(root: &Path, rel: &str, text: &str) {
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().unwrap_or_else(|| panic!("{full:?}"))).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, text).unwrap_or_else(|e| panic!("{}: {e}", full.display()));
}

fn editor(tag: &str, before: impl FnOnce(&Path)) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    before(&root);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    for _ in 0..4 {
        app.update();
    }
    app.world().resource::<ProjectIndex>().set_sweep(MIN_SWEEP);
    (root, app)
}

fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    for _ in 0..WAIT.0 {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(PAUSE);
    }
    panic!("never happened within {WAIT:?}: {what}");
}

/// The text of the page the Design workspace shows, when it shows one that read.
fn shown(app: &App) -> Option<String> {
    match &app.world().resource::<DesignView>().snapshot().page {
        Page::Text(text) => Some(text.clone()),
        _ => None,
    }
}

/// **A page edited outside the editor is on the Design workspace's page**, and a page that appears where there was none.
#[test]
fn a_design_page_edited_outside_the_editor_reaches_the_workspace_with_the_clock_stopped() {
    let (root, mut app) = editor("design_page_follows", |root| write(root, "design/style.md", "# First words\n"));
    *app.world_mut().resource_mut::<Workspace>() = Workspace::Design;
    until(&mut app, "the Design workspace shows the game's page", |a| shown(a).as_deref() == Some("# First words\n"));

    write(&root, "design/style.md", "# Second words, written by a hand\n\nMore of them.\n");
    let on_disk = std::fs::read_to_string(root.join("design/style.md")).unwrap_or_else(|e| panic!("{e}"));
    until(&mut app, "the page shows the edited file", |a| shown(a).as_deref() == Some(on_disk.as_str()));
}

/// **An area written by hand is on the overlay's list, and a removal takes it off.**
#[test]
fn an_area_written_outside_the_editor_reaches_the_overlay_with_the_clock_stopped() {
    let (root, mut app) = editor("design_area_follows", |_| ());
    assert!(app.world().resource::<StyleAreas>().areas.is_empty(), "the project has no areas to start with");
    write(&root, "design/levels/m/areas/yard/area.yaml", "version: 1\nrect: [2.0, 2.0, 6.0, 6.0]\n");
    until(&mut app, "the overlay holds the yard", |a| a.world().resource::<StyleAreas>().area_at((3.0, 3.0)).map(|area| area.name.as_str()) == Some("yard"));
    std::fs::remove_dir_all(root.join("design/levels/m/areas/yard")).unwrap_or_else(|e| panic!("{e}"));
    until(&mut app, "the overlay no longer holds the yard", |a| a.world().resource::<StyleAreas>().area_at((3.0, 3.0)).is_none());
}
