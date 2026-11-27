//! **What the tests of the four views that follow the project index share** (#501, items 5 to 8): an editor whose index sweeps fast,
//! a wait bounded in frames that says how many frames a change took to be seen (#644), and the proof that a file the view does not
//! follow makes it read nothing. There are no tests here.

use std::path::Path;
use std::time::Duration;

use bevy::prelude::*;
use bevy_dark_factory::ops::Ctx;
use bevy_dark_factory::project::{OpenMap, Project};
use bevy_dark_factory::project_index::ProjectIndex;
use serde_json::{Value, json};

use crate::fixtures;
use crate::settle_support::Frames;

/// **How many frames a change may take to be seen**, from a literal: the index's sweep is set to 25 ms below, its debounce is 100 ms,
/// and each frame pauses [`STEP`], so one second of frames. The number is not derived from any constant of the code under test, so a
/// view that waited on a clock of its own (or never looked again) cannot pass by moving a constant.
pub const SEEN_WITHIN: Frames = Frames::covering(Duration::from_millis(1000), STEP);

/// How many frames a test steps waiting for something that must happen before it says it never did: ten seconds at [`STEP`].
const GIVE_UP: Frames = Frames::covering(Duration::from_secs(10), STEP);

/// How many frames the editor is stepped, with nothing written, to say a frame had nothing left to do: 400 ms at [`STEP`].
const QUIET: Frames = Frames::covering(Duration::from_millis(400), STEP);

/// **How many frames the editor is stepped after each write to a neighbour**, from a literal: 1.3 s at [`STEP`], so twice this covers
/// at least 2.6 s and a view that still re-read once a second (the retired timer) would read at least twice, and a count that held
/// steady cannot be the timer's quiet.
const NEIGHBOUR_WATCH: Frames = Frames::covering(Duration::from_millis(1300), STEP);

/// How long one frame of a wait pauses, so the index's thread has the wall clock it needs between frames.
pub const STEP: Duration = Duration::from_millis(5);

/// **An editor open on a one-map project, whose index sweeps every 25 ms** (the shortest it accepts), with the operations bridge so
/// `blueprint/state` can be asked.
pub fn editor(tag: &str) -> (App, Ctx) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    sweep_every(&app, Duration::from_millis(25));
    let ctx = Ctx {
        project: Some(app.world().resource::<Project>().clone()),
        open: app.world().get_resource::<OpenMap>().cloned(),
        session: "test".to_owned(),
        ..Ctx::default()
    };
    (app, ctx)
}

pub fn sweep_every(app: &App, every: Duration) {
    app.world().resource::<ProjectIndex>().set_sweep(every);
}

pub fn published(app: &App) -> u64 {
    app.world().resource::<ProjectIndex>().published()
}

/// **Step the editor `how_long` frames**, each pausing [`STEP`] (#644).
pub fn pump(app: &mut App, how_long: Frames) {
    for _ in 0..how_long.0 {
        app.update();
        std::thread::sleep(STEP);
    }
}

/// **Step until `done`, and say how many frames it took**; panics after [`GIVE_UP`].
pub fn until_seen(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) -> Frames {
    for frame in 1..=GIVE_UP.0 {
        app.update();
        if done(app) {
            return Frames(frame);
        }
        std::thread::sleep(STEP);
    }
    panic!("never saw: {what} (stepped {GIVE_UP:?}); reloads {}", reloads(app));
}

/// **Step until the index has published nothing for [`QUIET`]**, so what a test counts next is what the test did.
///
/// **Named for the index, not `settle`** (#576). `settle_support::settle` steps until the *UI shape* stops
/// changing; this one steps until the *index* publishes nothing, and panics `the index never went quiet` when it
/// does not. Two helpers with one name and two meanings is a trap: fifteen files write
/// `use crate::index_view_support as support;` and then `support::settle_until_the_index_is_quiet(app)`, which reads exactly like the shared
/// one and waits for something else.
pub fn settle_until_the_index_is_quiet(app: &mut App) {
    for _ in 0..25 {
        let before = published(app);
        pump(app, QUIET);
        if published(app) == before {
            return;
        }
    }
    panic!("the index never went quiet");
}

pub fn reloads(app: &mut App) -> Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("blueprint/state").cloned() else {
        panic!("blueprint/state is not registered");
    };
    let state = app
        .world_mut()
        .run_system_with(id, Some(json!({})))
        .unwrap_or_else(|e| panic!("{e:?}"))
        .unwrap_or_else(|e| panic!("{e:?}"));
    state["reloads"].clone()
}

pub fn verdict(app: &mut App, kind: &str) -> Value {
    reloads(app)["kinds"].as_array().and_then(|k| k.iter().find(|v| v["kind"] == kind).cloned()).unwrap_or(Value::Null)
}

/// **A file beside the one a view follows, written and then changed, makes that view look at nothing.**
///
/// The index is waited on, not assumed: the test steps until it has published the neighbour's creation and again its change (its own
/// count, `ProjectIndex::published`, is the independent evidence that the index announced them), and then steps on for a while so a view
/// that read on any announcement has had frames to do it. `reads` is the view's own count of its filesystem looks (files read, `.gitignore` reads, link checks). The followed file may be there or not.
pub fn a_neighbour_is_not_read(app: &mut App, beside: &Path, reads: impl Fn(&App) -> u64) {
    settle_until_the_index_is_quiet(app);
    let before = reads(app);
    let neighbour = beside.with_file_name("neighbour-of-the-followed-file.txt");
    for (n, text) in ["first", "a second, longer text"].into_iter().enumerate() {
        let announced = published(app);
        std::fs::write(&neighbour, text).unwrap_or_else(|e| panic!("{neighbour:?}: {e}"));
        until_seen(app, &format!("the index announced write {n} of the neighbour"), |a| published(a) > announced);
        pump(app, NEIGHBOUR_WATCH);
    }
    assert_eq!(reads(app), before, "the view looked at the filesystem again when only a file beside it changed");
}
