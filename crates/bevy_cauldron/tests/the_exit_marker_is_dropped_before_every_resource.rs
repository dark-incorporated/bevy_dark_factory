//! **The exit marker is dropped before every resource** (#427, review of `TheWorldEnds`).
//!
//! The watchdog begins the exit when the world is dropped (the system's quit writes no `AppExit`), through a marker entity. What has to
//! hold is that it is dropped before the `Terminal`, whose drop waits on its shells. In Bevy 0.20 `World::clear_all` clears every
//! table, then every sparse set, and a resource is a sparse-set component: so the marker, a table component, goes before every resource
//! **whatever order they were made in**. The review of the first version feared the order of creation mattered (tables are cleared in
//! the order they were first made); it does not for resources, and this test shows it: a resource made before the plugin and one made
//! after both see the exit begun when they are dropped. A table component of an entity made before the plugin is dropped first, and
//! nothing covers it; that is pinned too, so a Bevy upgrade that changes either fails here.

use std::sync::Mutex;
use std::time::Duration;

use bevy::prelude::*;
use bevy_cauldron::exit_watchdog::{ExitState, ExitWatchdog, ExitWatchdogPlugin};

static SEEN: Mutex<Vec<(&'static str, bool)>> = Mutex::new(Vec::new());

/// Records, as it is dropped, whether the exit had begun.
struct Probe(&'static str, ExitState);

impl Drop for Probe {
    fn drop(&mut self) {
        SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((self.0, self.1.exit_has_begun()));
    }
}

#[derive(Resource)]
struct MadeBefore(#[allow(dead_code)] Probe);

#[derive(Resource)]
struct MadeAfter(#[allow(dead_code)] Probe);

/// A plain component, `Table` storage, on an entity made before the plugin.
#[derive(Component)]
struct EarlierEntity(#[allow(dead_code)] Probe);

fn nothing() {}

#[test]
fn every_resource_is_dropped_with_the_exit_begun_and_an_earlier_entitys_component_is_not() {
    let state = ExitState::new();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).insert_resource(state.clone()).insert_resource(MadeBefore(Probe("resource before", state.clone())));
    app.world_mut().spawn(EarlierEntity(Probe("entity before", state.clone())));
    app.add_plugins(ExitWatchdogPlugin { watchdog: ExitWatchdog { grace: Duration::from_secs(60), then: nothing } });
    app.insert_resource(MadeAfter(Probe("resource after", state.clone())));
    // The start-up a real run goes through; the marker must not depend on it.
    app.update();
    assert!(!state.exit_has_begun(), "nothing has begun to exit while the app runs");

    app.world_mut().clear_all();

    let seen = SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    assert!(seen.contains(&("resource after", true)), "a resource made after the plugin is dropped once the exit has begun: {seen:?}");
    assert!(seen.contains(&("resource before", true)), "so is one made before it, a resource being a sparse-set component: {seen:?}");
    assert!(seen.contains(&("entity before", false)), "a table component of an entity made before the plugin is dropped before the marker: {seen:?}");
}
