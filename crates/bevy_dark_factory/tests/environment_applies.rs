//! **A map's environment is saved in `bevy_elements`' own settings types and drives the preview** (#98): one
//! file per type, `family/bevy_elements/<Type>.ron`, each written by `family_set` and applied by the crate.
#![cfg(feature = "bevy_elements")]

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_dark_factory::ops::{self, Applied, Ctx};
use bevy_elements::{Climate, RockSettings, SnowSettings, WeatherOverrideSettings, WeatherPreset, WetnessSettings, WorldClock};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;

use crate::fixtures;

/// A project whose every environment type has a file of the crate's defaults.
fn project(tag: &str) -> (App, Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family/bevy_elements")).unwrap();
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_elements\"])").unwrap();
    fn write<T: Serialize + Default>(root: &std::path::Path, ident: &str) {
        std::fs::write(file(root, ident), ron::to_string(&T::default()).unwrap()).unwrap();
    }
    write::<WorldClock>(&root, "WorldClock");
    write::<Climate>(&root, "Climate");
    write::<WeatherOverrideSettings>(&root, "WeatherOverrideSettings");
    write::<SnowSettings>(&root, "SnowSettings");
    write::<RockSettings>(&root, "RockSettings");
    write::<WetnessSettings>(&root, "WetnessSettings");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_dark_factory::project::Project>().clone();
    (app, Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() }, root)
}

fn file(root: &std::path::Path, ident: &str) -> std::path::PathBuf {
    blueprint_model::family::settings_path(root, "bevy_elements", ident)
}

/// One settings file, read as the crate's own type: what a restarted game reads.
fn read<T: DeserializeOwned>(root: &std::path::Path, ident: &str) -> T {
    ron::from_str(&std::fs::read_to_string(file(root, ident)).unwrap()).unwrap()
}

fn set(ctx: &mut Ctx, ty: &str, field: &str, value: &str) {
    let r = ops::dispatch(ctx, "family_set", &json!({"crate": "bevy_elements", "type": ty, "field": field, "value": value}));
    assert!(matches!(r, Ok(Applied::Done { .. })), "{ty}.{field} = {value}: {r:?}");
}

fn until(app: &mut App, what: &str, done: impl Fn(&App) -> bool) {
    let start = Instant::now();
    while start.elapsed().as_secs_f64() < 8.0 {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("never saw: {what}");
}

#[test]
fn a_chosen_date_and_climate_are_in_the_files_and_read_back_after_a_restart() {
    let (_app, mut ctx, root) = project("restart");
    set(&mut ctx, "WorldClock", "date.month", "12");
    set(&mut ctx, "WorldClock", "hour", "18.5");
    set(&mut ctx, "Climate", "mean_temperature_c", "-4.0");
    // "Restart": nothing but the files is read.
    let clock: WorldClock = read(&root, "WorldClock");
    assert_eq!((clock.date.month, clock.hour), (12, 18.5));
    assert_eq!(read::<Climate>(&root, "Climate").mean_temperature_c, -4.0);
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(read::<Climate>(&root, "Climate").mean_temperature_c, Climate::default().mean_temperature_c, "one undo per change");
    assert_eq!(read::<WorldClock>(&root, "WorldClock").hour, 18.5);
}

/// The panel's Force and Release write `WeatherOverrideSettings`, and the editor applies the file to the
/// running resource; `bevy_elements`' own `apply_weather_override` forces and releases the director from
/// it (held by the crate's own test).
#[test]
fn forcing_rain_holds_until_it_is_released_back_to_the_climate() {
    let (mut app, mut ctx, _root) = project("rain");
    let preset = |a: &App| a.world().get_resource::<WeatherOverrideSettings>().and_then(|s| s.preset);
    set(&mut ctx, "WeatherOverrideSettings", "preset", "Some(Rain)");
    until(&mut app, "the running override forcing rain", |a| preset(a) == Some(WeatherPreset::Rain));
    set(&mut ctx, "WeatherOverrideSettings", "preset", "None");
    until(&mut app, "the override released to the climate", |a| a.world().get_resource::<WeatherOverrideSettings>().is_some() && preset(a).is_none());
}

#[test]
fn scrubbing_the_clock_moves_the_preview_clock_and_writes_nothing() {
    use bevy_dark_factory::environment::{scrub_clock, ClockScrub};
    let (_editor, mut ctx, root) = project("scrub");
    let before = std::fs::read_to_string(file(&root, "WorldClock")).unwrap();
    let mut app = App::new();
    app.init_resource::<ClockScrub>().insert_resource(WorldClock::default()).add_systems(Update, scrub_clock);
    app.update();
    let untouched = app.world().resource::<WorldClock>().hour;

    app.world_mut().resource_mut::<ClockScrub>().set(18.5);
    app.update();
    assert_eq!(app.world().resource::<WorldClock>().hour, 18.5);
    app.world_mut().resource_mut::<ClockScrub>().set(-1.0);
    app.update();
    assert_eq!(app.world().resource::<WorldClock>().hour, 23.0, "wraps into the day");

    assert_eq!(std::fs::read_to_string(file(&root, "WorldClock")).unwrap(), before, "scrubbing writes no file");
    assert!(bevy_dark_factory::ops::undo(&mut ctx, None).is_err(), "scrubbing is not an undo step");

    app.world_mut().resource_mut::<ClockScrub>().release();
    app.world_mut().resource_mut::<WorldClock>().hour = untouched;
    app.update();
    assert_eq!(app.world().resource::<WorldClock>().hour, untouched, "released: the file's clock runs");
}

#[test]
fn the_panels_weather_buttons_write_the_forced_weather_and_the_clock_buttons_write_nothing() {
    use bevy_dark_factory::environment::EnvAction;
    let (_app, mut ctx, root) = project("buttons");
    let dispatch = |ctx: &mut Ctx, a: EnvAction| {
        let r = ops::dispatch(ctx, "family_set", &a.family_params().unwrap());
        assert!(matches!(r, Ok(Applied::Done { .. })), "{a:?}: {r:?}");
    };
    let forced = |root: &std::path::Path| read::<WeatherOverrideSettings>(root, "WeatherOverrideSettings").preset;
    for preset in [WeatherPreset::Rain, WeatherPreset::Fog, WeatherPreset::Clear] {
        dispatch(&mut ctx, EnvAction::Force(preset));
        assert_eq!(forced(&root), Some(preset));
    }
    dispatch(&mut ctx, EnvAction::ReleaseWeather);
    assert_eq!(forced(&root), None);
    assert!(EnvAction::Scrub(6.0).family_params().is_none() && EnvAction::ReleaseClock.family_params().is_none());
}

#[test]
fn the_panels_season_and_climate_buttons_are_one_undo_step_each() {
    use bevy_dark_factory::environment::{ClimatePreset, EnvAction};
    let (_app, mut ctx, root) = project("season_climate");
    let dispatch = |ctx: &mut Ctx, a: EnvAction| {
        let r = ops::dispatch(ctx, "family_set", &a.family_params().unwrap());
        assert!(matches!(r, Ok(Applied::Done { .. })), "{a:?}: {r:?}");
    };
    for (month, _) in EnvAction::SEASONS {
        dispatch(&mut ctx, EnvAction::Month(month));
        assert_eq!(read::<WorldClock>(&root, "WorldClock").date.month, month);
    }
    let climate = |root: &std::path::Path| ron::to_string(&read::<Climate>(root, "Climate")).unwrap();
    for (preset, _) in ClimatePreset::ALL {
        dispatch(&mut ctx, EnvAction::Climate(preset));
        assert_eq!(climate(&root), ron::to_string(&preset.climate()).unwrap());
    }
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(climate(&root), ron::to_string(&ClimatePreset::Tropical.climate()).unwrap(), "one undo steps back one climate");
}

#[test]
fn snow_rocks_and_sand_are_set_in_the_crates_own_types_and_a_short_file_still_loads() {
    let (_app, mut ctx, root) = project("cover");
    set(&mut ctx, "SnowSettings", "drift", "false");
    set(&mut ctx, "RockSettings", "talus_cover", "0.5");
    set(&mut ctx, "WetnessSettings", "wet_per_mm", "0.9");
    assert!(!read::<SnowSettings>(&root, "SnowSettings").drift);
    assert_eq!(read::<RockSettings>(&root, "RockSettings").talus_cover, 0.5);
    assert_eq!(read::<WetnessSettings>(&root, "WetnessSettings").wet_per_mm, 0.9);
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(read::<WetnessSettings>(&root, "WetnessSettings").wet_per_mm, WetnessSettings::default().wet_per_mm, "one undo per change");
    // A file written before a field existed takes the crate's default for it.
    assert_eq!(ron::from_str::<SnowSettings>("()").unwrap(), SnowSettings::default());
}
