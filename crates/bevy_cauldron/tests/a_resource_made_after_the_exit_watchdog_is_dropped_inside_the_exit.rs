//! **What the exit watchdog covers is what is made after it** (#427, review of `TheWorldEnds`).
//!
//! The watchdog begins the exit when the world is dropped (the system's quit writes no `AppExit`), through a marker entity. Bevy 0.20
//! stores resources as components and `World::clear_all` clears the tables in the order they were first made, so **the marker is
//! dropped before every resource made after the plugin and after every one made before it**. The `Terminal` is the resource whose
//! drop waits on a child, so the binary adds the plugin before it (`main.rs`). This pins both halves: a resource made after the plugin
//! is dropped with the exit begun, and one made before it is not (which is why the plugin is added first, not last).
//!
//! If a Bevy upgrade changes the order, the second half fails and the arrangement in `main.rs` has to be reconsidered.

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

fn nothing() {}

#[test]
fn a_resource_made_after_the_plugin_is_dropped_with_the_exit_begun_and_one_made_before_it_is_not() {
    let state = ExitState::new();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).insert_resource(state.clone()).insert_resource(MadeBefore(Probe("before", state.clone())));
    app.add_plugins(ExitWatchdogPlugin { watchdog: ExitWatchdog { grace: Duration::from_secs(60), then: nothing } });
    app.insert_resource(MadeAfter(Probe("after", state.clone())));
    // The start-up a real run goes through; the marker must not depend on it.
    app.update();
    assert!(!state.exit_has_begun(), "nothing has begun to exit while the app runs");

    app.world_mut().clear_all();

    let seen = SEEN.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
    assert!(seen.contains(&("after", true)), "a resource made after the plugin is dropped once the exit has begun: {seen:?}");
    assert!(seen.contains(&("before", false)), "a resource made before the plugin is dropped before the marker, which is why the binary adds the plugin first: {seen:?}");
}
