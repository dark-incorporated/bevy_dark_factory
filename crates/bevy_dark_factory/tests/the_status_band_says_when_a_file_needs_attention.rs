//! **The status band says when a reload refused a file or needs a restart** (UI-34, #228).
//!
//! The band's left end answers what is in hand (`the_status_band_counts_the_set_in_hand`); this is
//! the other half of the same acceptance item — *"a refused reload turns the problems item to the
//! warning kind"*. A person editing beside a running game asks two questions, and only the first
//! had an answer on screen: how many am I carrying, and did my edit take.
//!
//! Three claims, and each can fail:
//!
//! 1. **Every file applied says nothing.** A band that said "0 problems" on every frame is a row a
//!    reader stops seeing, and it is the state the editor is in almost all the time.
//! 2. **A refused file turns the item to the warning kind**, drawn in the running skin's `accent`,
//!    which is the issue's own wording and gooey's rule for [`bevy_gooey::StatusKind::Warning`]. The
//!    editor keeps running and the previous state stays; what did not take is what the mark is for.
//! 3. **A file that reads but needs a restart is the same kind**, and a person fixing one still has
//!    to see the other, so both counts are named.
//!
//! **The readout is gooey's status item** (#647): the kind is on the item, where gooey reads it for
//! the `status_mark()` child it spawns, and the words are a `ControlText` child under it. The colour
//! read here is therefore the one `apply_skin` wrote — the mark's fill — and not a `TextColor` this
//! crate computed from the same kind, which is what it asserted before #647 and what made the band
//! paint a second, hand-written copy of a fact the skin already owns.

use bevy::prelude::*;
use bevy_dark_factory::harness;
use bevy_gooey::{Role, StatusKind};
use blueprint_model::live::KindVerdict;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name)
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    (root, app)
}

/// What the band currently says, the kind it says it as, and the ink gooey drew the mark in, or
/// `None` when the readout is absent.
///
/// The words are the item's `Text` child — gooey's `status_item_quiet` spawns the words under the
/// item, and the item itself carries no `Text` — and the ink is the `status_mark()` child's
/// `BackgroundColor`, which is `apply_skin`'s.
fn said(app: &mut App) -> Option<(String, StatusKind, Color)> {
    let world = app.world_mut();
    let item = world
        .query_filtered::<Entity, With<bevy_dark_factory::chrome::ProblemsCount>>()
        .single(world)
        .ok()?;
    let kids: Vec<Entity> = world.get::<Children>(item).map(|c| c.iter().collect())?;
    let words = kids.iter().find(|k| world.get::<Text>(**k).is_some())?;
    let mark = kids.iter().find(|k| world.get::<Role>(**k) == Some(&Role::StatusMark))?;
    let text = world.get::<Text>(*words)?.0.clone();
    let kind = *world.get::<StatusKind>(item)?;
    let ink = world.get::<BackgroundColor>(*mark)?.0;
    Some((text, kind, ink))
}

/// Whether the item is drawn at all. A readout with nothing to say is `Shown(false)`, gooey's rule
/// for a status item that is not there (the job row and the wish chip are hidden the same way).
fn shown(app: &mut App) -> Option<bool> {
    let world = app.world_mut();
    let item = world
        .query_filtered::<Entity, With<bevy_dark_factory::chrome::ProblemsCount>>()
        .single(world)
        .ok()?;
    world.get::<bevy_gooey::Shown>(item).map(|s| s.0)
}

fn skin(app: &App) -> bevy_gooey::GooeySkin {
    app.world()
        .get_resource::<bevy_gooey::GooeySkin>()
        .expect("the editor cannot run without a GooeySkin")
        .clone()
}

fn record(app: &mut App, verdict: KindVerdict) {
    app.world_mut()
        .resource_mut::<bevy_dark_factory::live::EditorReloads>()
        .decide(vec![verdict]);
    app.update();
}

/// **Every file applied says nothing**, so the band does not carry a permanent "0 problems".
#[test]
fn a_band_with_nothing_wrong_says_nothing() {
    let (_root, mut app) = editor("problems-clean");
    record(&mut app, KindVerdict::applied("kit", "kit.yaml"));
    let (text, kind, ink) = said(&mut app).expect("the band must carry the readout even when it is empty");
    assert_eq!(
        text, "",
        "an applied file must leave the band silent; `{text}` on every edit is a receipt"
    );
    assert_eq!(
        kind,
        StatusKind::Info,
        "nothing wrong is `Info`, not a mark beside a word"
    );
    assert_eq!(
        shown(&mut app),
        Some(false),
        "and an empty readout is not drawn at all: an item with nothing to say would hold a \\
         target's width of the band open on every frame"
    );
    assert_eq!(
        ink,
        Color::NONE,
        "plain state draws no mark: gooey's `StatusKind::Info` is the absence of one, and the skin \\
         asks its `accent` for nothing here"
    );
}

/// **A refused file turns the item to the warning kind, in the skin's `accent`.**
#[test]
fn a_refused_file_turns_the_item_to_the_warning_kind() {
    let (_root, mut app) = editor("problems-refused");
    record(
        &mut app,
        KindVerdict::refused("kit", "kit.yaml", Some("pieces[3].yaw".to_owned()), "not a number"),
    );
    let (text, kind, ink) = said(&mut app).expect("the band must carry the readout");
    assert_eq!(
        text, "1 refused",
        "the band must say what became of the file; the journal has the detail, the band the count"
    );
    assert_eq!(
        kind,
        StatusKind::Warning,
        "a refused reload turns the problems item to the warning kind: the editor keeps running on \\
         the previous state, and a person has to see that their edit did not take"
    );
    assert_eq!(shown(&mut app), Some(true), "and the item is drawn once it has something to say");
    assert_eq!(
        ink,
        skin(&app).accent,
        "the ink must come from the running skin, so a theme edit reaches the band"
    );
}

/// **A file that reads but needs a restart is a warning**, and a refusal beside it outranks it.
#[test]
fn a_file_needing_a_restart_is_a_warning_under_a_refusal() {
    let (_root, mut app) = editor("problems-restart");
    record(
        &mut app,
        KindVerdict::needs_restart("rigs", "rigs.yaml", "restart to load the rigs"),
    );
    let (text, kind, ink) = said(&mut app).expect("the band must carry the readout");
    assert_eq!(text, "1 needs a restart");
    assert_eq!(
        kind,
        StatusKind::Warning,
        "a valid file the program cannot take live is a warning: nothing was lost, something waits"
    );
    assert_eq!(ink, skin(&app).accent, "a warning's ink is the skin's `accent`");

    // A refusal beside it is the graver fact, so the item says the error and names both counts.
    record(
        &mut app,
        KindVerdict::refused("kit", "kit.yaml", None, "does not parse"),
    );
    let (text, kind, ink) = said(&mut app).expect("the band must carry the readout");
    assert_eq!(
        text, "1 refused · 1 needs a restart",
        "both counts, because a person who fixes the refusal still has a restart owed"
    );
    assert_eq!(kind, StatusKind::Warning);
    assert_eq!(ink, skin(&app).accent);
}
