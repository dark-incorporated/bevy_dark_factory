//! **A quit that does not finish is finished for it** (#337).
//!
//! On 2026-11-07 an update quit the app, the window and the audio went, and the process stayed alive with
//! nothing drawn: a frozen app that had to be killed by hand, and a swap that never ran. The binary now adds a
//! watchdog that, once `AppExit` is written, ends the process if it is still there after a short grace. The
//! binary's own action kills the process, so these tests give it one that counts.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_dark_factory::exit_watchdog::{ExitWatchdog, ExitWatchdogPlugin, arm_after};

use crate::settle_support::Frames;

/// The grace the tests use: long enough to tell "not yet" from "done", short enough to wait for.
const GRACE: Duration = Duration::from_millis(300);

/// How long one look for the watchdog sleeps.
const PAUSE: Duration = Duration::from_millis(10);

/// The most looks a test gives the watchdog to act: at least ten seconds at [`PAUSE`], counted in looks so a starved host
/// is not read as a watchdog that never acted (#644).
const PATIENCE: Frames = Frames::covering(Duration::from_secs(10), PAUSE);

static ENDED_BY_ARM_AFTER: AtomicUsize = AtomicUsize::new(0);
static ENDED_BY_THE_PLUGIN: AtomicUsize = AtomicUsize::new(0);

fn ended_by_arm_after() {
    ENDED_BY_ARM_AFTER.fetch_add(1, Ordering::SeqCst);
}

fn ended_by_the_plugin() {
    ENDED_BY_THE_PLUGIN.fetch_add(1, Ordering::SeqCst);
}

/// Waits for the watchdog to have acted `at_least` times and says how long that took on the wall clock, which is what the
/// grace is measured in; the wait itself is bounded by [`PATIENCE`] looks.
fn wait_for(count: &AtomicUsize, at_least: usize) -> Duration {
    let started = Instant::now();
    for _ in 0..PATIENCE.0 {
        if count.load(Ordering::SeqCst) >= at_least {
            return started.elapsed();
        }
        std::thread::sleep(PAUSE);
    }
    panic!("the watchdog never acted within {PATIENCE:?}");
}

#[test]
fn the_action_runs_after_the_grace_and_not_before() {
    ENDED_BY_ARM_AFTER.store(0, Ordering::SeqCst);
    arm_after(GRACE, ended_by_arm_after).unwrap_or_else(|e| panic!("{e}"));

    std::thread::sleep(GRACE / 3);
    assert_eq!(ENDED_BY_ARM_AFTER.load(Ordering::SeqCst), 0, "a third of the way through the grace it has not acted");
    let took = wait_for(&ENDED_BY_ARM_AFTER, 1) + GRACE / 3;
    assert!(took >= GRACE * 9 / 10, "it acted after {took:?}, before the grace of {GRACE:?}");
}

#[test]
fn nothing_is_armed_until_the_app_exits_and_a_second_exit_arms_nothing_more() {
    ENDED_BY_THE_PLUGIN.store(0, Ordering::SeqCst);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(ExitWatchdogPlugin { watchdog: ExitWatchdog { grace: GRACE, then: ended_by_the_plugin } });

    // A running editor never starts it.
    for _ in 0..5 {
        app.update();
    }
    std::thread::sleep(GRACE * 2);
    assert_eq!(ENDED_BY_THE_PLUGIN.load(Ordering::SeqCst), 0, "an editor that is not quitting is left alone");

    // The first exit arms it; a second, while it waits, does not arm another.
    app.world_mut().write_message(AppExit::Success);
    app.update();
    app.world_mut().write_message(AppExit::Success);
    app.update();
    wait_for(&ENDED_BY_THE_PLUGIN, 1);
    std::thread::sleep(GRACE * 2);
    assert_eq!(ENDED_BY_THE_PLUGIN.load(Ordering::SeqCst), 1, "it acted once, for the first exit");
}
