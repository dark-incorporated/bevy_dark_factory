//! **The Plugins door reads the family crates through reflection**: a crate's settings types, its
//! components and its systems are listed with no code for that crate in the door, and switching a crate
//! on writes a file for each of its settings types from the running resource.
#![cfg(feature = "family_all")]

use bevy::prelude::*;
use bevy_dark_factory::plugins_tab::{self, PluginsDoor, SettingsInspector};
use bevy_dark_factory::tiles::Mode;

use crate::fixtures;

fn open(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    app.world_mut().insert_resource(Mode::Plugins);
    (root, app)
}

#[test]
fn a_crate_shows_its_settings_components_and_systems_by_reflection() {
    let (_root, mut app) = open("plugins_lights");
    app.world_mut().resource_mut::<PluginsDoor>().selected = Some("bevy_lights".to_owned());
    for _ in 0..3 {
        app.update();
    }
    let settings: Vec<String> = {
        let registry = app.world().resource::<AppTypeRegistry>().read();
        bevy_blueprint::settings::settings_types(&registry, "bevy_lights").unwrap().into_iter().map(|r| bevy_blueprint::settings::ident_of(r).to_owned()).collect()
    };
    let inspectors: Vec<String> = app.world_mut().query::<&SettingsInspector>().iter(app.world()).map(|i| i.ident.clone()).collect();
    assert_eq!(inspectors.len(), settings.len(), "one section per settings type: {inspectors:?} vs {settings:?}");

    let components: Vec<String> = plugins_tab::components_of(app.world(), "bevy_lights").into_iter().map(|c| c.short).collect();
    assert!(components.iter().any(|c| c == "Flicker"), "{components:?}");
    assert!(components.iter().any(|c| c == "Fire"), "{components:?}");

    let systems = plugins_tab::systems_of(app.world(), "bevy_lights");
    assert!(!systems.is_empty(), "the crate's systems are listed");
    assert!(systems.iter().all(|s| !s.sets.is_empty()), "each is in one of the crate's sets (the machine plate): {systems:?}");
}

#[test]
fn switching_gooey_on_writes_its_skin_file_from_the_running_resource() {
    let (root, mut app) = open("plugins_switch");
    let file = root.join("family/bevy_gooey/GooeySkin.ron");
    assert!(!file.exists());
    app.world_mut().commands().queue(|world: &mut World| plugins_tab::switch(world, "bevy_gooey", true));
    for _ in 0..2 {
        app.update();
    }
    let text = std::fs::read_to_string(&file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let read: bevy_gooey::GooeySkin = ron::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
    assert_eq!(read, *app.world().resource::<bevy_gooey::GooeySkin>(), "the file reads back as the running skin");
    let listed = blueprint_model::family::Family::load(&root).unwrap().crates;
    assert!(listed.iter().any(|c| c == "bevy_gooey"), "{listed:?}");
}
