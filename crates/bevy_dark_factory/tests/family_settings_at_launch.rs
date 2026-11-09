//! **A settings file already present when the project opens is in force on the first frame** (#98, #96).
//!
//! The watcher reads the files when it is created but reports only *changes*, so settings that
//! were there from the start were never applied until the file was edited once: the
//! environment preview stayed undressed after a launch.

use bevy::prelude::*;
use bevy_dark_factory::editor::EditorState;
use bevy_dark_factory::family_host::{reload_settings, FamilyFiles, PendingSettings};

use crate::fixtures;

/// **A real `WorldClock`, as `bevy_elements` writes one**: the crate's own settings type, one file of its
/// own (`family/bevy_elements/WorldClock.ron`). The type is `#[serde(default)]`, so the hour alone is a
/// whole clock, and the host's typed read takes it.
fn elements_settings() -> &'static str {
    "(hour: 6.0)\n"
}

#[test]
fn a_settings_file_present_at_launch_is_applied_without_an_edit() {
    let root = fixtures::Fixture::new("family_launch").descriptor("site/wall", "alpha").build("m");
    let file = root.join("family").join("bevy_elements").join("WorldClock.ron");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, elements_settings()).unwrap();
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<EditorState>().init_resource::<FamilyFiles>().init_resource::<PendingSettings>().add_systems(Update, reload_settings);
    app.update();
    assert_eq!(
        app.world().resource::<FamilyFiles>().applied[&("bevy_elements".to_owned(), "WorldClock".to_owned())],
        elements_settings(),
        "the file present at launch must be the settings the host holds, verbatim"
    );
}
