//! The machine plate: every system this crate adds is in [`BlueprintAnimSystems`], and every public
//! component is reflected as a component and registered by [`BlueprintAnimPlugin`].
//!
//! The crate has no settings resource. System names are the full path only with Bevy's `debug` feature,
//! which the dev-dependency turns on; the test asserts it found crate systems so a build without the
//! feature cannot pass vacuously.

use std::any::TypeId;

use bevy::ecs::schedule::Schedules;
use bevy::prelude::*;
use bevy_blueprint_anim::*;

const CRATE: &str = "bevy_blueprint_anim::";

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(BlueprintAnimPlugin);
    app.update();
    app
}

#[test]
fn machine_plate_every_system_is_in_the_root_set() {
    let app = app();
    let sets = [BlueprintAnimSystems::Attach, BlueprintAnimSystems::Blend].map(|set| set.intern());
    let schedules = app.world().resource::<Schedules>();
    let mut seen = 0;
    let mut stray = Vec::new();
    for (_label, schedule) in schedules.iter() {
        let Ok(systems) = schedule.systems() else { continue };
        let members: Vec<_> = sets.iter().filter_map(|set| schedule.graph().systems_in_set(*set).ok()).collect();
        for (key, system) in systems {
            let name = system.name().to_string();
            if !name.starts_with(CRATE) {
                continue;
            }
            seen += 1;
            if !members.iter().any(|m| m.contains(&key)) {
                stray.push(name);
            }
        }
    }
    assert!(seen > 0, "no system named `{CRATE}*` found; is Bevy's `debug` feature on?");
    assert!(stray.is_empty(), "systems outside BlueprintAnimSystems: {stray:?}");
}

#[test]
fn machine_plate_components_are_reflected() {
    let app = app();
    let registry = app.world().resource::<AppTypeRegistry>().read();
    for id in [TypeId::of::<PoseBlender>(), TypeId::of::<BlendSource>()] {
        let name = registry.get(id).map_or("<unregistered>", |r| r.type_info().type_path());
        assert!(registry.get_type_data::<ReflectComponent>(id).is_some(), "{name} has no ReflectComponent");
    }
}
