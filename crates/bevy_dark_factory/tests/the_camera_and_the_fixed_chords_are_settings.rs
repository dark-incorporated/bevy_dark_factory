//! **The camera's tunables and the fixed chords are settings** (`settings.yaml`'s `view:` and `keymap:`).
//!
//! The Map camera's nearest and furthest, its wheel step, its pan and a piece's turn were constants in `view.rs` and
//! `editor.rs`; the chords read outside the key census (the console's, the terminal's, the palette's) were constants in
//! a dozen modules. A person sets each in their settings file now, the editor takes it while running, and a value no
//! camera can use is refused naming its field.

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::chords::{Fixed, chord, default_of};
use bevy_dark_factory::settings::SettingsLayer;
use bevy_gooey::{Modifiers, Shortcut};

use crate::fixtures;

/// More than the settings watch's own look interval, so every update looks.
const STEP: Duration = Duration::from_millis(300);

fn settle(app: &mut App) {
    for _ in 0..6 {
        app.update();
    }
}

/// An editor reading its user settings from `settings.yaml` in a temp folder, which the test writes.
fn editor(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let config = root.join("user_config");
    std::fs::create_dir_all(&config).expect("a config folder");
    let settings = config.join("settings.yaml");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
    app.insert_resource(bevy_dark_factory::settings::UserSettingsFile(settings.clone()));
    settle(&mut app);
    (app, settings)
}

#[test]
fn a_settings_file_sets_the_cameras_range_while_the_editor_runs() {
    let (mut app, settings) = editor("view_settings");
    assert_eq!(bevy_dark_factory::view::tunables().zoom_max_m, 80.0, "the shipped furthest");
    std::fs::write(&settings, "version: 1\nview: {zoom_max: 10.0, yaw_step_deg: 45.0}\n").expect("a settings file");
    settle(&mut app);
    let view = bevy_dark_factory::view::tunables();
    assert_eq!(view.zoom_max_m, 10.0, "the file's furthest is the camera's");
    assert_eq!(view.yaw_step_deg, 45.0, "and its turn of a piece");
    assert_eq!(view.zoom_min_m, 0.25, "a key the file leaves out keeps its default");
}

#[test]
fn a_view_no_camera_can_use_is_refused_naming_its_field() {
    for (text, field) in [
        ("version: 1\nview: {zoom_min: 0.0}\n", "view.zoom_min"),
        ("version: 1\nview: {zoom_max: 0.1}\n", "view.zoom_max"),
        ("version: 1\nview: {zoom_step: 1.0}\n", "view.zoom_step"),
        ("version: 1\nview: {pan_speed: -1.0}\n", "view.pan_speed"),
        ("version: 1\nview: {yaw_step_deg: 120.0}\n", "view.yaw_step_deg"),
    ] {
        let refused = SettingsLayer::read("settings.yaml", text).expect_err(text);
        assert_eq!(refused.field.as_deref(), Some(field), "{text}: {}", refused.sentence());
    }
    assert!(SettingsLayer::read("settings.yaml", "version: 1\nview: {zoom_maxx: 9.0}\n").is_err(), "a key the camera has not got is refused");
}

#[test]
fn a_fixed_chord_is_rebound_by_its_id_while_the_editor_runs() {
    let (mut app, settings) = editor("fixed_chord");
    let moved = Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::KeyL);
    assert_eq!(chord(Fixed::ConsoleToggle), default_of(Fixed::ConsoleToggle), "the console answers to its default");
    std::fs::write(&settings, format!("version: 1\nkeymap:\n  console.open: {}\n", serde_json::to_string(&moved).expect("a chord"))).expect("a settings file");
    settle(&mut app);
    assert_eq!(chord(Fixed::ConsoleToggle), moved, "the console answers to the chord the file gives it");
    assert!(SettingsLayer::read("settings.yaml", "version: 1\nkeymap:\n  console.opne: {key: KeyL}\n").is_err(), "an id nothing has is refused");
}
