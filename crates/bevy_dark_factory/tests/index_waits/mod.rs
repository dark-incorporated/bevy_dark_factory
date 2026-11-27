//! **What the tests of the panels that follow the project index share** (#501): a stepped editor whose frames run on the test's thread,
//! a sweep set so long that nothing but a writer's note reveals a change, and waits bounded in frames (#644).
//!
//! Every panel test has the same two halves, because a test that only saw the change arrive could not tell a subscription from a poll:
//!
//! 1. **With the index's sweep set to ten minutes ([`SWEEP_NEVER`]), a file is rewritten in place and nothing says so.** The panel must
//!    go on saying what it said, for longer than the longest of the retired clocks ([`LONGER_THAN_THE_OLD_CLOCKS`]). A restored poll would
//!    show the new file in this window, so this half fails on a poll; and the sweep that would reveal the change by chance never comes.
//! 2. **Then the writer says it has finished** ([`announce`], what an operation's own write does) **and the panel shows the new file**
//!    within [`SHOWN_WITHIN`]. This half fails when the subscription is removed.

use std::path::Path;
use std::time::Duration;

use bevy::prelude::*;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::project_index::{self, ProjectIndex};
use bevy_dark_factory::workspace::Workspace;

use crate::settle_support::Frames;

/// A sweep so long that the periodic look at every file never comes during a test: only a writer's own note reveals a change.
pub const SWEEP_NEVER: Duration = Duration::from_secs(600);

/// **How long a panel is watched not changing**: longer than the longest of the four retired clocks (`reference_view::RECHECK_SECS`,
/// one second). 320 frames that each pause [`FRAME_PAUSE`] cover at least 1.6 s of the wall clock, written here from a literal so it
/// does not move with anything the modules say.
pub const LONGER_THAN_THE_OLD_CLOCKS: Frames = Frames::covering(Duration::from_millis(1600), FRAME_PAUSE);

/// How many frames a panel may take to show a change the writer has announced: a debounce, a frame and a margin for a loaded machine,
/// three seconds at [`FRAME_PAUSE`].
pub const SHOWN_WITHIN: Frames = Frames::covering(Duration::from_secs(3), FRAME_PAUSE);

/// How many frames a change that only the index's own (fast) sweep reveals may take: the sweep, the 100 ms debounce, and a margin,
/// one second at [`FRAME_PAUSE`].
pub const SEEN_BY_A_FAST_SWEEP_WITHIN: Frames = Frames::covering(Duration::from_secs(1), FRAME_PAUSE);

/// **How many frames a panel is stepped after the index published a change it does not follow**, so a panel that read on any
/// announcement has had frames to do it: 300 ms at [`FRAME_PAUSE`].
pub const AFTER_AN_UNRELATED_CHANGE: Frames = Frames::covering(Duration::from_millis(300), FRAME_PAUSE);

/// The pause after each frame of a wait, so the index's own thread has the wall clock it needs between frames.
pub const FRAME_PAUSE: Duration = Duration::from_millis(5);

pub fn stepped(root: &Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // **The frame's systems run on this thread**, so the per-thread read counters see everything a frame does.
    app.edit_schedule(PreUpdate, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    app.edit_schedule(Update, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    for _ in 0..8 {
        app.update();
    }
    app
}

pub fn sweep(app: &App, every: Duration) {
    app.world().resource::<ProjectIndex>().set_sweep(every);
}

/// **Step frames until `done`, for at most `within` frames** (#644): the index runs on its own thread, so each frame pauses
/// [`FRAME_PAUSE`]; the bound is a count of frames, so a starved host steps the same frames more slowly instead of running out of
/// seconds. How many frames it took.
pub fn pump_until(app: &mut App, within: Frames, mut done: impl FnMut(&mut App) -> bool) -> Option<Frames> {
    for frame in 1..=within.0 {
        app.update();
        if done(app) {
            return Some(Frames(frame));
        }
        std::thread::sleep(FRAME_PAUSE);
    }
    None
}

pub fn pump_for(app: &mut App, how_long: Frames) {
    pump_until(app, how_long, |_| false);
}

pub fn rel_of(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or_else(|e| panic!("{path:?} is not under {root:?}: {e}")).to_string_lossy().replace('\\', "/")
}

/// **A writer says it has finished**, as `file_create` and the others do.
pub fn announce(root: &Path, path: &Path) {
    project_index::note_written(root, &rel_of(root, path));
}

pub fn write_in_place(path: &Path, text: &str) {
    std::fs::write(path, text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
}

/// Every text under the entities carrying `C`, in the order of the tree.
pub fn texts_under<C: Component>(app: &mut App) -> Vec<String> {
    fn walk(world: &World, entity: Entity, out: &mut Vec<String>) {
        if let Some(text) = world.get::<Text>(entity) {
            out.push(text.0.clone());
        }
        if let Some(children) = world.get::<Children>(entity) {
            for child in children.iter() {
                walk(world, child, out);
            }
        }
    }
    let world = app.world_mut();
    let roots: Vec<Entity> = world.query_filtered::<Entity, With<C>>().iter(world).collect();
    let mut out = Vec::new();
    for root in roots {
        walk(world, root, &mut out);
    }
    out
}

pub fn open_workspace(app: &mut App, id: &str, expected: Workspace) {
    let command = commands::by_id(id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    pump_for(app, Duration::from_millis(300));
    assert_eq!(app.world().resource::<Workspace>(), &expected, "`{id}` did not open the workspace");
}

pub fn published(app: &App) -> u64 {
    app.world().resource::<ProjectIndex>().published()
}
