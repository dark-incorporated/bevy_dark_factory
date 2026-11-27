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
use std::time::Duration;

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

/// **A wait's bound, and what a wait took, in frames** (#644). The owner's rule (2026-11-27): a test waits N frames, not N
/// seconds. A bound on the wall clock runs out early when a loaded host starves the editor's frames (or the thread a frame
/// reads), and the miss reads exactly like a product bug; a bound in frames is reached only once the frames were stepped.
/// Every wait still pauses between its frames, so a bound written [`Frames::covering`] a window also gives a thread, a child
/// or a file watcher at least that window of the wall clock.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Frames(pub u32);

impl Frames {
    /// **The frames a wait that pauses `pause` after each one steps to cover `window`** of the wall clock at the least (rounded up).
    pub const fn covering(window: Duration, pause: Duration) -> Self {
        Self(window.as_millis().div_ceil(pause.as_millis()) as u32)
    }
}

impl std::fmt::Debug for Frames {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} frames", self.0)
    }
}

/// **How long one step of a wait sleeps**, so a wait does not spin a core and still sees a change within a frame or two.
pub const STEP: Duration = Duration::from_millis(2);

/// **How many frames a wait steps** before it says what never happened: ten seconds at [`STEP`], the same window
/// `index_view_support` gives, and the number a reader of a failing test is told.
pub const GIVE_UP: Frames = Frames::covering(Duration::from_secs(10), STEP);

/// **How many frames the editor must leave what it holds alone before [`settle`] calls it laid out**: 400 ms at [`STEP`],
/// the window `index_view_support::settle_until_the_index_is_quiet` has always used (`QUIET`), so a reader meets one idea
/// twice. A panel that rewrites a row's text (a selection, a status) changes no entity count, which is why [`shape`] hashes
/// the text too — the failing row of the 2026-11-23 gate, where a Notes row was not yet selected.
///
/// **`settle` is for a frame or two of work, not for a panel's own timer.** A panel that looks every half second
/// (`refresh_reload_list`) can be idle when `settle` returns; a test that needs what such a panel draws says
/// `until(app, "...", condition)` with the thing it wants, which the settle window cannot answer for.
const QUIET: Frames = Frames::covering(SETTLE_TAKES_AT_LEAST, STEP);

/// **The least wall clock one [`settle`] takes**: its quiet frames, each pausing [`STEP`]. A wait that settles on every look
/// counts its looks against this, not against its own pause, or its bound would be hundreds of times the window it names.
pub const SETTLE_TAKES_AT_LEAST: Duration = Duration::from_millis(400);

/// **The most frames [`settle`] steps before it gives up**: the editor never stops creating entities while a background
/// thread retries (a refused file, a missing kit), and a test must not hang for a reason that is not its own.
const SETTLE_FRAMES_MAX: u32 = 4000;

/// **What the editor holds, as one number a wait can compare**: the entity count and the text every `Text` holds.
///
/// The count alone missed the change that matters most to a test — a row whose text is rewritten, which is how a list
/// draws a selection or a status — so a wait could call a frame quiet while the row was about to be redrawn (the
/// 2026-11-23 gate, where a Notes row was not yet selected when `settle` returned).
fn shape(app: &mut App) -> (u32, u64) {
    let world = app.world_mut();
    let entities = world.entities().len();
    let mut texts = world.query::<&Text>();
    let mut hash = 0u64;
    for text in texts.iter(world) {
        for byte in text.0.bytes() {
            hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
        }
    }
    (entities, hash)
}

/// **Step until `done`, and say how many frames it took**; panics after [`GIVE_UP`] with `what` named. A wait on a file, a
/// thread or a panel is this, and it is the same shape `index_view_support::until_seen` has so a reader meets one idea twice.
pub fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) -> Frames {
    for frame in 1..=GIVE_UP.0 {
        step(app);
        if done(app) {
            return Frames(frame);
        }
        std::thread::sleep(STEP);
    }
    panic!("never saw: {what} (stepped {GIVE_UP:?})");
}

/// **Step until the editor's frames have stopped changing what it holds**, so a test reads what it just changed.
///
/// **A test that needs a number of frames says so inline**, `for _ in 0..N { app.update() }`, where a reader can see that the
/// test is about frames rather than about a result.
pub fn settle(app: &mut App) {
    let mut last = shape(app);
    let mut quiet = 0;
    for _ in 0..SETTLE_FRAMES_MAX {
        step(app);
        let now = shape(app);
        if now != last {
            last = now;
            quiet = 0;
        } else {
            quiet += 1;
            if quiet >= QUIET.0 {
                return;
            }
        }
        std::thread::sleep(STEP);
    }
    // **Not a failure**: an editor that is still working after four thousand frames has a reason, and it is the test's own
    // to name. A test that must be sure says `until(app, "...", condition)` instead.
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
