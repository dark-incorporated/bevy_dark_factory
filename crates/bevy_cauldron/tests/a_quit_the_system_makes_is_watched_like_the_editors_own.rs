//! **A quit the system makes is watched like the editor's own** (#427).
//!
//! Cmd+Q on a Mac is the system menu's `terminate:`, not the editor's key handler: winit's `applicationWillTerminate` runs the app's
//! exit and drops the `World`, with no `AppExit` written and no `QuitRequested` or close request read. The exit watchdog was armed by
//! those, so on this path it never was: measured on a real macOS build (bmb, 0.6.8, 2026-11-15), a quit stuck in the teardown (a
//! terminal's `Child::wait`) was alive after 70 s, with the watchdog thread asleep and no line in the log. Dropping the world is
//! itself the start of an exit.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_cauldron::exit_watchdog::{ExitWatchdog, ExitWatchdogPlugin, SecondPress};

/// The grace the test uses: long enough to tell "not yet" from "done", short enough to wait for.
const GRACE: Duration = Duration::from_millis(300);

/// The most the test waits for the watchdog to act.
const PATIENCE: Duration = Duration::from_secs(5);

static ENDED: AtomicUsize = AtomicUsize::new(0);

fn ended() {
    ENDED.fetch_add(1, Ordering::SeqCst);
}

fn the_chord_is_never_down() -> bool {
    false
}

#[test]
fn dropping_the_world_with_no_app_exit_starts_the_deadline() {
    ENDED.store(0, Ordering::SeqCst);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(ExitWatchdogPlugin { watchdog: ExitWatchdog { grace: GRACE, then: ended } })
        .insert_resource(SecondPress { debounce: Duration::from_millis(100), poll: Duration::from_millis(10), down: the_chord_is_never_down });
    // The startup that starts the watch.
    app.update();

    // A running editor is left alone.
    std::thread::sleep(GRACE * 2);
    assert_eq!(ENDED.load(Ordering::SeqCst), 0, "an editor that is not quitting is left alone");

    // winit's `exiting` is exactly this: no `AppExit`, the world is cleared.
    app.world_mut().clear_all();

    let started = Instant::now();
    while ENDED.load(Ordering::SeqCst) == 0 {
        assert!(started.elapsed() < PATIENCE, "the world was dropped with no AppExit and the watchdog never began its deadline");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(started.elapsed() >= GRACE * 8 / 10, "it acted after {:?}, before the grace of {GRACE:?}", started.elapsed());
}
