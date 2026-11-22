//! **The forest is redrawn when the project index says a file of it moved, and not on a clock** (#501, `forest_view::POLL_S` retired).
//!
//! The view used to `stat` the forest file and every tree file twice a second. It reads the project index's stamps now, so these tests
//! change files the way an editor, an agent or a hand does (a plain `std::fs` write) and wait, on the wall clock, for the drawing to
//! agree with what the test wrote:
//!
//! - the **forest file** changes its tree count (more, fewer, none because the file is gone), and the view follows each;
//! - a **tree file** changes its species' trunk and the placed tree regrows.
//!
//! **A test that a leftover timer could pass is not one**: the clock is frozen (a manual time step of nothing), so a view that still
//! waited for half a second of `Time` would never look, and the index's sweep is set to its floor so only the index's own debounce stands
//! between a write and the drawing. The ground truth is what the test wrote (a count it makes itself, and the file read back with
//! `std::fs`), never anything the view derives.

#![cfg(feature = "bevy_flora")]

use std::path::Path;
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::forest_view::ForestView;
use bevy_dark_factory::ops::{self, Ctx};
use bevy_dark_factory::project::{OpenMap, Project};
use bevy_dark_factory::project_index::{MIN_SWEEP, ProjectIndex};
use bevy_flora::ProceduralTree;
use serde_json::json;

use crate::fixtures;

/// How long a test waits for the drawing to follow a write: the index's sweep and debounce are a fraction of it.
const WAIT_SECONDS: f64 = 20.0;
/// The wall-clock pause between frames while waiting.
const PAUSE_MILLIS: u64 = 10;

fn open(tag: &str) -> (App, Project) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // **The clock does not move**: whatever the view does, it does not do because time passed.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    for _ in 0..4 {
        app.update();
    }
    // The shortest sweep there is, so the index's own periodic sweep is never what a change waits for.
    app.world().resource::<ProjectIndex>().set_sweep(MIN_SWEEP);
    let project = app.world().resource::<Project>().clone();
    (app, project)
}

/// Step frames until `done`, on the wall clock.
fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    let started = Instant::now();
    while started.elapsed().as_secs_f64() < WAIT_SECONDS {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(PAUSE_MILLIS));
    }
    panic!("never happened within {WAIT_SECONDS} s: {what}");
}

/// **A forest file of `trees` placed oaks**, each a hand-grown tree (a seed), so no pool has to be built.
fn forest_text(trees: usize) -> String {
    let mut text = String::from("version: 1\ntrees:\n");
    for id in 0..trees {
        text.push_str(&format!(
            "- {{id: {id}, species: oak, at: [{}.0, 2.0], ground_m: 0.0, yaw: 0.0, scale: 1.0, variant: 0, seed: {id}, age_years: 10.0}}\n",
            id * 3
        ));
    }
    text
}

/// **The trees the file says, counted by a plain read of it**: one line starting `- {` for each.
fn trees_in(path: &Path) -> usize {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())).lines().filter(|l| l.starts_with("- {")).count()
}

fn drawn(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query::<&ForestView>().iter(world).count()
}

fn placed_radius(app: &mut App) -> Option<f32> {
    let world = app.world_mut();
    world.query_filtered::<&ProceduralTree, With<ForestView>>().iter(world).next().map(|t| t.0.trunk.radius)
}

/// **A change to the forest file is drawn, whether it adds trees, takes some away, or removes the file.**
#[test]
fn a_forest_file_changed_on_disk_is_redrawn_with_the_clock_stopped() {
    let (mut app, project) = open("forest_follows_the_index");
    let path = project.blueprint_dir.join("forest").join("m.yaml");
    std::fs::create_dir_all(path.parent().unwrap_or_else(|| panic!("{path:?}"))).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(drawn(&mut app), 0, "there is no forest file yet");

    for (written, why) in [(3, "a stroke planted three"), (5, "two more were planted"), (1, "an undo took four away")] {
        std::fs::write(&path, forest_text(written)).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(trees_in(&path), written, "the file the test wrote says {written} trees");
        until(&mut app, &format!("{why}: {written} trees drawn"), |app| drawn(app) == trees_in(&path));
    }

    std::fs::remove_file(&path).unwrap_or_else(|e| panic!("{e}"));
    until(&mut app, "the forest file is gone, so no trees are drawn", |app| drawn(app) == 0);

    // It comes back: a file that appears again is drawn again.
    std::fs::write(&path, forest_text(2)).unwrap_or_else(|e| panic!("{e}"));
    until(&mut app, "a forest file that appears again is drawn", |app| drawn(app) == trees_in(&path));
}

/// **A tree file edited on disk regrows the tree that uses it.**
#[test]
fn a_tree_file_edited_on_disk_regrows_the_tree_with_the_clock_stopped() {
    let (mut app, project) = open("forest_tree_file_follows_the_index");
    let open_map = app.world().resource::<OpenMap>().clone();
    let mut ctx = Ctx { project: Some(project.clone()), open: Some(open_map), session: "test".to_owned(), ..Ctx::default() };
    ops::dispatch(&mut ctx, "tree_edit", &json!({"name": "tall", "preset": "birch", "set": {"trunk.radius": 0.3}})).unwrap_or_else(|e| panic!("{e:?}"));
    let forest = project.blueprint_dir.join("forest").join("m.yaml");
    std::fs::create_dir_all(forest.parent().unwrap_or_else(|| panic!("{forest:?}"))).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(
        &forest,
        "version: 1\ntrees:\n- {id: 0, species: trees/tall.tree.yaml, at: [4.0, 2.0], ground_m: 0.0, yaw: 0.0, scale: 1.0, variant: 0, seed: 7, age_years: 12.0}\n",
    )
    .unwrap_or_else(|e| panic!("{e}"));
    until(&mut app, "the placed tree is grown from its species file", |app| placed_radius(app) == Some(0.3));

    // The species file is edited on its own: the forest file does not move, so only the tree file's stamp can say it.
    let before = std::fs::read_to_string(&forest).unwrap_or_else(|e| panic!("{e}"));
    ops::dispatch(&mut ctx, "tree_edit", &json!({"name": "tall", "set": {"trunk.radius": 0.9}})).unwrap_or_else(|e| panic!("{e:?}"));
    assert_eq!(std::fs::read_to_string(&forest).unwrap_or_else(|e| panic!("{e}")), before, "the forest file was not touched");
    until(&mut app, "the placed tree regrows from the edited species file", |app| placed_radius(app) == Some(0.9));
}

/// **A forest file the index does not list (the project's `.gitignore` names it) is still drawn**: the view reads it when it looks and says so,
/// and does not take an unlisted file for a missing one. The index's own listing is the ground truth that it is not listed.
#[test]
fn a_forest_file_the_project_ignores_is_still_drawn() {
    let root = fixtures::Fixture::new(&format!("forest_unlisted_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    std::fs::write(root.join(".gitignore"), "forest/\n").unwrap_or_else(|e| panic!("{e}"));
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    // The file is written after the index's first listing and before the view's first frame; the project says where the forest lives.
    let forest = app.world().resource::<Project>().blueprint_dir.join("forest").join("m.yaml");
    std::fs::create_dir_all(forest.parent().unwrap_or_else(|| panic!("{forest:?}"))).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&forest, forest_text(2)).unwrap_or_else(|e| panic!("{e}"));
    // (That the index does not list the file is asserted after the drawing, below: before its first sweep that would hold of any new file.)
    until(&mut app, "the unlisted forest file is drawn", |app| drawn(app) == trees_in(&forest));
    let listed = app.world().resource::<ProjectIndex>().snapshot().map(|s| s.files.iter().any(|(rel, _)| rel.ends_with("forest/m.yaml")));
    assert_eq!(listed, Some(false), "the index does not list a file the project's .gitignore names, so the view read it without the index");
}

/// **A forest file that cannot be read keeps the drawing, and is said**, where it used to be taken for an empty forest. Frames run on this thread
/// so the warning can be counted by a subscriber set here. (Skipped as root, which can read anything.)
#[cfg(unix)]
#[test]
fn a_forest_file_that_cannot_be_read_keeps_the_trees_and_is_said() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Mutex};

    use bevy::log::tracing::{self, Event, Subscriber, field::{Field, Visit}};
    use bevy::log::tracing_subscriber::{Layer, Registry, layer::{Context, SubscriberExt}};

    struct Messages(Arc<Mutex<Vec<String>>>);
    struct Text(String);
    impl Visit for Text {
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            if field.name() == "message" {
                self.0 = format!("{value:?}");
            }
        }
    }
    impl<S: Subscriber> Layer<S> for Messages {
        fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
            let mut text = Text(String::new());
            event.record(&mut text);
            self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(text.0);
        }
    }

    let (mut app, project) = open("forest_unreadable");
    app.edit_schedule(Update, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    let path = project.blueprint_dir.join("forest").join("m.yaml");
    std::fs::create_dir_all(path.parent().unwrap_or_else(|| panic!("{path:?}"))).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&path, forest_text(3)).unwrap_or_else(|e| panic!("{e}"));
    until(&mut app, "three trees drawn", |app| drawn(app) == 3);

    let seen: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let said = || seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().filter(|m| m.contains("m.yaml") && m.contains("cannot be read")).count();
    tracing::subscriber::with_default(Registry::default().with(Messages(seen.clone())), || {
        std::fs::write(&path, forest_text(5)).unwrap_or_else(|e| panic!("{e}"));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).unwrap_or_else(|e| panic!("{e}"));
        if std::fs::read_to_string(&path).is_ok() {
            eprintln!("skipped: this user can read a file with no permissions");
            return;
        }
        until(&mut app, "the view said it cannot read the forest file", |_| said() >= 1);
        assert_eq!(drawn(&mut app), 3, "a file that cannot be read leaves the drawing as it was, not an empty forest");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap_or_else(|e| panic!("{e}"));
    });
}
