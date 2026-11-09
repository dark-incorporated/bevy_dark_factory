//! The machine plate: every system this crate adds is in [`DevshotSystems`]. The crate has no settings
//! resource and no component. System names are the full path only with Bevy's `debug` feature, which the
//! dev-dependency turns on; the test asserts it found crate systems so a build without the feature cannot
//! pass vacuously.

use bevy::ecs::schedule::Schedules;
use bevy::prelude::*;
use bevy_devshot::*;

const CRATE: &str = "bevy_devshot::";

#[test]
fn machine_plate_every_system_is_in_the_root_set() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(DevshotPlugin);
    app.update();
    let set = DevshotSystems.intern();
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
    assert!(stray.is_empty(), "systems outside DevshotSystems: {stray:?}");
}
