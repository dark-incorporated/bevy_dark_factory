//! **The one way a test waits** (#576): step the editor until the thing the test is about has happened, or say what never
//! happened. There are no tests here.
//!
//! # Why one helper and not ninety-two
//!
//! Every `tests/*.rs` that stepped frames had grown its own `settle`: 92 copies in 16 bodies, six to ninety unconditional
//! frames each, five of which slept the wall clock. A fixed count pays every frame whether or not the work arrived, and it
//! *fails* when the work did not — which is how a view that follows the project index comes to fail one gate and pass the
//! next at the same commit (#576, `RUNNING-A-LOOP.md:368`). A wait on the condition the test is about costs nothing when the
//! editor is quick and is not fooled when the host is loaded.
//!
//! The index-following views keep [`crate::index_view_support::settle`], which waits on the index's own published count rather
//! than on frames; this is the helper for every other test.
//!
//! # What "settled" means here
//!
//! An editor whose frames have stopped creating and destroying entities. Every panel rebuild, every gooey layout pass and
//! every state transition shows up as entities appearing or going, so a step of [`QUIET_FRAMES`] frames that left the entity
//! count alone has laid out what the test last changed. It is not a proof that some background thread has finished — nothing
//! about a fixed frame count is — so a test that waits for a *file* still says so with [`until`].

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use bevy::prelude::*;

/// **Every frame a test steps through [`settle_support`]**, so "the suite got cheaper" is a number a run prints rather than a
/// belief. `cargo test -p bevy_dark_factory --test editor -- --nocapture` prints it from
/// [`frames_stepped`] when [`report_stepped_frames`] is called by the run's last test.
static STEPPED: AtomicU64 = AtomicU64::new(0);

/// How many frames this process has stepped through [`settle`] and [`until`].
pub fn frames_stepped() -> u64 {
    STEPPED.load(Ordering::Relaxed)
}

/// **Print the frames this process stepped**, named, once — for the measurement #576 asks for. A test calls it last; a run without
/// it still counts, and nothing depends on the print.
pub fn report_stepped_frames(who: &str) {
    let stepped = frames_stepped();
    eprintln!("settle_support: {who} stepped {stepped} settling frames in this process");
}

/// Count one stepped frame.
fn step(app: &mut App) {
    app.update();
    STEPPED.fetch_add(1, Ordering::Relaxed);
}

/// **How long a wait may take** before it says what never happened: the same ten seconds `index_view_support` gives, and the
/// number a reader of a failing test is told.
pub const GIVE_UP: Duration = Duration::from_secs(10);

/// **How long one step of a wait sleeps**, so a wait does not spin a core and still sees a change within a frame or two.
const STEP: Duration = Duration::from_millis(2);

/// **How many frames in a row must leave the entity count alone** for the editor to count as laid out. Eight is more than the
/// deepest UI nesting in this crate rebuilds through, and a frame that changes nothing but time still costs 2 ms here.
const QUIET_FRAMES: u32 = 8;

/// **The most frames [`settle`] steps before it gives up**: the editor never stops creating entities while a background
/// thread retries (a refused file, a missing kit), and a test must not hang for a reason that is not its own.
const SETTLE_FRAMES_MAX: u32 = 600;

/// **Step until `done`, and say how long it took**; panics after [`GIVE_UP`] with `what` named. A wait on a file, a thread or a
/// panel is this, and it is the same shape `index_view_support::until_seen` has so a reader meets one idea twice.
pub fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) -> Duration {
    let start = Instant::now();
    while start.elapsed() < GIVE_UP {
        step(app);
        if done(app) {
            return start.elapsed();
        }
        std::thread::sleep(STEP);
    }
    panic!("never saw: {what} (waited {GIVE_UP:?})");
}

/// **Step until the editor's frames have stopped changing what it holds**, so a test reads what it just changed.
///
/// **A test that needs a number of frames says so inline**, `for _ in 0..N { app.update() }`, where a reader can see that the
/// test is about frames rather than about a result.
pub fn settle(app: &mut App) {
    let mut last = app.world().entities().len();
    let mut quiet = 0;
    for _ in 0..SETTLE_FRAMES_MAX {
        step(app);
        let now = app.world().entities().len();
        if now == last {
            quiet += 1;
            if quiet >= QUIET_FRAMES {
                return;
            }
        } else {
            quiet = 0;
            last = now;
        }
        std::thread::sleep(STEP);
    }
    // **Not a failure**: an editor that is still working after six hundred frames has a reason, and it is the test's own
    // assertion about what it holds that should say whether that reason is a defect. Saying so makes the hang visible.
    panic!("the editor did not stop changing what it holds within {SETTLE_FRAMES_MAX} frames");
}

#[cfg(test)]
mod counter {
    use super::*;

    /// **The counters are the measurement #576 asks for**, so a test proves they move and print: one `settle` over a bare app
    /// steps at least one frame, and `frames_stepped` reads it back.
    #[test]
    fn the_stepped_frame_counter_moves_and_reports() {
        let mut app = App::new();
        let before = frames_stepped();
        settle(&mut app);
        let after = frames_stepped();
        assert!(after > before, "a settle stepped {after} frames and the counter said {before}: it is not counting");
        report_stepped_frames("the counter's own test");
    }
}
