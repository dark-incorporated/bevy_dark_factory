//! **A family settings file edited while the editor runs is reloaded within a second** (#96).
//!
//! The host follows the family folder through the project index (#538; `blueprint_model::family::SETTINGS_POLL_SECS` retired), so these tests
//! wait on the wall clock, with the index's sweep at its floor, for the host to hold the text the test wrote. **Owner exception
//! (2026-11-14)**, for the test here (`family_settings_reload.rs::an_edited_settings_file_reaches_the_editor_and_a_bad_one_is_a_problem`): it
//! stepped `Time<Real>` with `TimeUpdateStrategy::ManualDuration` because `FamilyWatch` measured a 0.1 s poll and a 0.1 s settle on that clock
//! (#279), and measured the one-second promise on it. The watch has no clock now, so a step of the clock moves nothing; the 3 s wall-clock
//! bound is unchanged, and the promise is measured on the wall clock, where the index's sweep and debounce actually run. Since
//! #644 the wait itself is bounded by frames covering those 3 s, so a starved host cannot end it early; the promise is still
//! the wall clock's.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_dark_factory::editor::EditorState;
use bevy_dark_factory::family_host::{reload_settings, FamilySettings};
use bevy_dark_factory::project_index::{MIN_SWEEP, ProjectIndex};

use crate::fixtures;
use crate::settle_support::Frames;

/// The editor's promise (#96): an edit is in force within this much wall-clock time.
const PROMISE: Duration = Duration::from_secs(1);
/// The wall-clock pause between frames while waiting.
const PAUSE: Duration = Duration::from_millis(10);
/// The bound the owner set (2026-10-29, #121) on the whole wait: frames covering three seconds at [`PAUSE`], counted rather
/// than timed so a starved host cannot end the wait early (#644).
const BOUND: Frames = Frames::covering(Duration::from_secs(3), PAUSE);

/// **A real `ElementsSettings`, as `bevy_elements` writes one**, and the same file an hour later.
///
/// These tests wrote `(sun: (hour: 6.0))`, which is not an `ElementsSettings`: the host's typed read
/// refuses it (`Unexpected field named sun`), nothing is stored, and the test was asserting on a
/// fixture the crate throws away (#152). The base is `ElementsSettings::defaults().to_yaml()` with
/// `clock.hour` set; **exactly one line differs** between the two, which is what makes the edit an
/// edit rather than a second file.
const SETTINGS: &str = include_str!("fixtures/bevy_elements.yaml");
const SETTINGS_LATER: &str = include_str!("fixtures/bevy_elements_18.5.yaml");

#[test]
fn an_edited_settings_file_reaches_the_editor_and_a_bad_one_is_a_problem() {
    let root = fixtures::Fixture::new("family_reload").descriptor("site/wall", "alpha").build("m");
    let file = root.join("family").join("bevy_elements.yaml");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, SETTINGS).unwrap();
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<EditorState>().init_resource::<FamilySettings>().add_systems(Update, reload_settings);
    app.update();
    app.world().resource::<ProjectIndex>().set_sweep(MIN_SWEEP);
    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(&file, SETTINGS_LATER).unwrap();
    let start = Instant::now();
    let mut stepped = BOUND;
    for frame in 0..BOUND.0 {
        if app.world().resource::<FamilySettings>().text["bevy_elements"] == SETTINGS_LATER {
            stepped = Frames(frame);
            break;
        }
        app.update();
        std::thread::sleep(PAUSE);
    }
    let took = start.elapsed();
    assert!(stepped < BOUND, "took {stepped:?}");
    assert!(took <= PROMISE, "the edit took {took:?} on the wall clock, past the {PROMISE:?} it promises");
    assert_eq!(
        app.world().resource::<FamilySettings>().text["bevy_elements"],
        SETTINGS_LATER,
        "the edit must be the settings the host holds, verbatim"
    );

    std::thread::sleep(Duration::from_millis(50));
    std::fs::write(&file, "{sun: {hour: ").unwrap();
    for _ in 0..BOUND.0 {
        if !app.world().resource::<EditorState>().status.problems().is_empty() {
            break;
        }
        app.update();
        std::thread::sleep(PAUSE);
    }
    let probs = app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect::<Vec<_>>();
    assert!(probs.iter().any(|p| p.contains("bevy_elements") && p.contains("not valid YAML")), "{probs:?}");
    assert_eq!(
        app.world().resource::<FamilySettings>().text["bevy_elements"],
        SETTINGS_LATER,
        "a refused edit keeps the last good settings, verbatim"
    );
}
