//! The machine plate: every system this crate adds is in [`BlueprintSystems`], and every public component
//! is reflected as a component and registered by [`BlueprintPlugin`].
//!
//! `bevy_blueprint` has no settings resource of its own: it reads the family's (`live::reload_settings`).
//! System names are the full path only with Bevy's `debug` feature, which the dev-dependency turns on;
//! the test asserts it found crate systems so a build without the feature cannot pass vacuously.

use std::any::TypeId;

use bevy::ecs::schedule::Schedules;
use bevy::prelude::*;
use bevy_blueprint::live::BlueprintLivePlugin;
use bevy_blueprint::*;

const CRATE: &str = "bevy_blueprint::";

fn app() -> App {
    let dir = std::env::temp_dir().join(format!("blueprint_machine_plate_{}", std::process::id()));
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<StandardMaterial>()
        .add_plugins((BlueprintPlugin, BlueprintLivePlugin { dir: dir.clone(), maps: Vec::new(), root: dir }));
    app.update();
    app
}

#[test]
fn machine_plate_every_system_is_in_the_root_set() {
    let app = app();
    let set = BlueprintSystems.intern();
    let schedules = app.world().resource::<Schedules>();
    let mut seen = 0;
    let mut stray = Vec::new();
    for (_label, schedule) in schedules.iter() {
        let Ok(systems) = schedule.systems() else { continue };
        let members = schedule.graph().systems_in_set(set).ok();
        for (key, system) in systems {
            let name = system.name().to_string();
            if !name.starts_with(CRATE) {
                continue;
            }
            seen += 1;
            if !members.is_some_and(|m| m.contains(&key)) {
                stray.push(name);
            }
        }
    }
    assert!(seen > 0, "no system named `{CRATE}*` found; is Bevy's `debug` feature on?");
    assert!(stray.is_empty(), "systems outside BlueprintSystems: {stray:?}");
}

#[test]
fn machine_plate_components_are_reflected() {
    let app = app();
    let registry = app.world().resource::<AppTypeRegistry>().read();
    let components = [
        TypeId::of::<Placement>(),
        TypeId::of::<OfDescriptor>(),
        TypeId::of::<Tags>(),
        TypeId::of::<MountedOn>(),
        TypeId::of::<Supporting>(),
        TypeId::of::<Paint>(),
    ];
    for id in components {
        let name = registry.get(id).map_or("<unregistered>", |r| r.type_info().type_path());
        assert!(registry.get_type_data::<ReflectComponent>(id).is_some(), "{name} has no ReflectComponent");
    }
}
