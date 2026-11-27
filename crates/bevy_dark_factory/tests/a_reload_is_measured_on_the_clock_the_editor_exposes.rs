//! **A settings reload no longer depends on any clock, so a test cannot drive it with one** (#279, then #538).
//!
//! #279 made the reload measurable on `Time<Real>`, the clock the family watch polled and settled on, so a test could step it instead of
//! sleeping. #538 retired that clock: the host follows the family folder through the project index and asks the watch to read when the
//! index says a file moved, so the reload lands after the index's sweep and debounce on the wall clock, whatever `Time` does.
//!
//! **Owner exception (2026-11-14)** for both tests here (the file keeps their place, and re-points what each one pins):
//!
//! | test | before (#279) | now (#538) |
//! |---|---|---|
//! | `a_clock_that_does_not_advance_reloads_nothing` | a frozen clock never crossed the poll and settle, so nothing reloaded | **renamed** `a_clock_that_does_not_advance_still_reloads_the_file`: a frozen clock changes nothing, the edit lands once the index has seen it |
//! | `stepping_the_clock_past_the_poll_and_the_settle_reloads_the_file` | three steps of 0.2 s crossed both intervals and the edit landed | **renamed** `stepping_the_clock_does_not_reload_the_file_before_the_index_has_seen_it`: three steps are no time for the index, so nothing has landed, and then it does |
//!
//! Each fails if the reload is measured on a clock again: the first would then wait for time that never passes, the second would land at once.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::editor::EditorState;
use bevy_dark_factory::family_host::{reload_settings, FamilySettings};
use bevy_dark_factory::project_index::{MAX_SWEEP_EVERY, MIN_SWEEP, ProjectIndex};

use crate::fixtures;
use crate::settle_support::Frames;

const SETTINGS: &str = include_str!("fixtures/bevy_elements.yaml");
const SETTINGS_LATER: &str = include_str!("fixtures/bevy_elements_18.5.yaml");

/// One step of the test's clock, in seconds. It crosses the old poll and settle intervals (0.1 s each) in a single update.
const STEP: f32 = 0.2;

/// The pause between two frames of [`until_landed`].
const PAUSE: Duration = Duration::from_millis(10);

/// **How many frames a test steps for the index to show the host a change** (#644): a count, not a wall-clock window, so a starved
/// host does not cut it short; at [`PAUSE`] apart they cover at least 30 s.
const WAIT: Frames = Frames::covering(Duration::from_secs(30), PAUSE);

/// **The editor the host runs in, on a clock this test owns**, and the index's sweep at its floor.
fn editor(root: &std::path::Path, step: f32) -> App {
    let mut app = bevy_dark_factory::harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<EditorState>().init_resource::<FamilySettings>().add_systems(Update, reload_settings);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(step)));
    app.update();
    app.world().resource::<ProjectIndex>().set_sweep(MIN_SWEEP);
    app
}

/// The settings the host holds right now, as text.
fn held(app: &App) -> String {
    app.world().resource::<FamilySettings>().text.get("bevy_elements").cloned().unwrap_or_default()
}

fn project(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let file = root.join("family").join("bevy_elements.yaml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, SETTINGS).unwrap();
    (root, file)
}

fn until_landed(app: &mut App) {
    for _ in 0..WAIT.0 {
        if held(app) == SETTINGS_LATER {
            break;
        }
        app.update();
        std::thread::sleep(PAUSE);
    }
    assert_eq!(held(app), SETTINGS_LATER, "the edit must be the settings the host holds, verbatim, within {WAIT:?}");
}

/// **A clock that does not advance still reloads the file**: the reload is the index's, not time's.
#[test]
fn a_clock_that_does_not_advance_still_reloads_the_file() {
    let (root, file) = project("reload_frozen_clock");
    let mut app = editor(&root, 0.0);
    std::fs::write(&file, SETTINGS_LATER).unwrap();
    until_landed(&mut app);
    assert!(app.world().resource::<EditorState>().status.problems().is_empty(), "and a reload is not a problem");
}

/// **Stepping the clock does not reload the file before the index has seen it**, and the file lands afterwards.
#[test]
fn stepping_the_clock_does_not_reload_the_file_before_the_index_has_seen_it() {
    let (root, file) = project("reload_stepped_clock");
    let mut app = editor(&root, STEP);
    // **The index is held still while the clock steps**, so "nothing has landed" does not rest on three frames taking under the
    // index's 100 ms debounce of wall clock: a family_all frame is slower than that on a loaded host, and the index then landed the
    // edit honestly inside the three steps. A long sweep is the index's own test lever (`ProjectIndex::set_sweep`); the pause
    // lets a sweep already under way finish before the write, so the only thing that could land the file is the clock.
    app.world().resource::<ProjectIndex>().set_sweep(MAX_SWEEP_EVERY);
    std::thread::sleep(MIN_SWEEP * 4);
    std::fs::write(&file, SETTINGS_LATER).unwrap();
    // Three steps are 0.6 s of the editor's clock and no sweep at all for the index.
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(held(&app), SETTINGS, "the clock moved past the old poll and settle, and the index has not had its debounce, so nothing has landed");
    app.world().resource::<ProjectIndex>().set_sweep(MIN_SWEEP);
    until_landed(&mut app);
}
