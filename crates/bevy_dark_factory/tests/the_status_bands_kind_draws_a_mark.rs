//! **The status band's kind draws a mark, and the skin draws its ink** (#647).
//!
//! The band's problems readout carried `bevy_gooey::StatusKind` on a bare `Text`, and nothing in gooey
//! reads a kind off a bare `Text`: the kind drew nothing at all, and the colour a person actually saw
//! was a `TextColor` this crate computed from the same kind by hand — one fact stated twice, one copy
//! dead, and the band painting its own colour beside a mark that was never built.
//!
//! gooey reads a `StatusKind` in exactly two places, both **on the parent of a `Role::StatusMark`**
//! (`bevy_gooey` at `8647648`, `skin.rs:3438` and `:4464`): the mark is the skin's `accent` for a
//! warning, its `danger` for an error, and not drawn at all for plain state. So the readout is gooey's
//! own `status_item_quiet` — the node gooey's readers look at — with the `status_mark()` child that
//! widget spawns, and the words are a child of the item in the skin's `text`.
//!
//! Three claims, and each can fail:
//!
//! 1. **The readout is the widget gooey's readers look at**: a `StatusItem` carrying the kind, with a
//!    `StatusMark` child. Before this, the band carried a `StatusKind` no gooey role read.
//! 2. **The kind draws**: a refusal makes the mark a visible bar in the running skin's `accent`, and a
//!    clean band draws none. Measured on the mark's own `BackgroundColor`, which is `apply_skin`'s.
//! 3. **The ink is the skin's, not this crate's**: move the skin's `accent` and its `text` and the mark
//!    and the words follow in the same frame. The words are the skin's `text` in every kind — the kind
//!    is the mark's, never a second colour the band writes into the words itself.

use bevy::prelude::*;
use bevy_dark_factory::chrome::ProblemsCount;
use bevy_dark_factory::harness;
use bevy_gooey::{Role, StatusItem, StatusKind};
use blueprint_model::live::KindVerdict;

use crate::fixtures;

/// The accent of a skin a person could write: moved somewhere no preset puts it, so a passing
/// assertion cannot be the editor's own constant coinciding with the field.
fn an_accent_of_its_own() -> Color {
    Color::srgb(0.13, 0.87, 0.31)
}

/// And the words' ink in that skin, moved too and to a colour that is not the accent.
fn an_ink_of_its_own() -> Color {
    Color::srgb(0.91, 0.31, 0.77)
}

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    for _ in 0..4 {
        app.update();
    }
    (root, app)
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

/// The band's readout as a gooey reader finds it: the item, the words and the mark.
struct Readout {
    item: Entity,
    words: Entity,
    mark: Entity,
}

fn readout(app: &mut App) -> Readout {
    let item = item(app);
    assert_eq!(
        app.world().get::<Role>(item),
        Some(&Role::StatusItem),
        "the readout must be gooey's status item, the node `apply_skin` reads a `StatusKind` off for \
         the mark: a bare `Text` wearing a kind is one nothing draws"
    );
    let world = app.world_mut();
    let kids: Vec<Entity> = world.get::<Children>(item).map(|c| c.iter().collect()).unwrap_or_default();
    let words = *kids
        .iter()
        .find(|k| world.get::<Text>(**k).is_some())
        .unwrap_or_else(|| panic!("the readout's words are a child of the item: {kids:?}"));
    let mark = *kids
        .iter()
        .find(|k| world.get::<Role>(**k) == Some(&Role::StatusMark))
        .unwrap_or_else(|| panic!("the readout carries gooey's `status_mark()`: {kids:?}"));
    Readout { item, words, mark }
}

/// The band's problems readout, whatever it is made of.
fn item(app: &mut App) -> Entity {
    let world = app.world_mut();
    world
        .query_filtered::<Entity, With<ProblemsCount>>()
        .single(world)
        .unwrap_or_else(|e| panic!("the band carries the readout: {e}"))
}

fn said(app: &App, found: &Readout) -> String {
    app.world().get::<Text>(found.words).expect("the words carry their text").0.clone()
}

fn kind(app: &App, found: &Readout) -> StatusKind {
    *app.world().get::<StatusKind>(found.item).expect("the item carries the kind")
}

/// The ink the mark draws in: `apply_skin`'s `BackgroundColor` on the `StatusMark`.
fn mark_ink(app: &App, found: &Readout) -> Color {
    app.world().get::<BackgroundColor>(found.mark).expect("the mark is painted").0
}

/// The ink the words draw in: `apply_skin`'s `TextColor` on the item's words.
fn words_ink(app: &App, found: &Readout) -> Color {
    app.world().get::<TextColor>(found.words).expect("the words are painted").0
}

fn mark_display(app: &App, found: &Readout) -> Display {
    app.world().get::<Node>(found.mark).expect("the mark is laid out").display
}

/// **The readout is the widget gooey reads a kind off**, not a `Text` wearing one.
#[test]
fn the_readout_is_the_status_item_gooey_styles() {
    let (_root, mut app) = editor("status-mark-widget");
    record(
        &mut app,
        KindVerdict::refused("kit", "kit.yaml", Some("pieces[3].yaw".to_owned()), "not a number"),
    );
    let item = item(&mut app);
    assert_eq!(
        app.world().get::<Role>(item),
        Some(&Role::StatusItem),
        "the readout must be gooey's status item: it is the node `apply_skin` reads a `StatusKind` off \
         for the mark, and before this the band carried a kind no gooey role read"
    );
    assert!(
        app.world().get::<StatusItem>(item).is_some(),
        "and the item's own marker, the one `status_item_quiet` puts on it"
    );
    let found = readout(&mut app);
    assert_eq!(kind(&app, &found), StatusKind::Warning, "a refused reload is the warning kind");
    assert!(
        app.world().get::<Text>(found.item).is_none(),
        "the words are the item's child, not the item: gooey inks a `ControlText` child and has no ink \
         for a `StatusItem`'s own text"
    );
    assert_eq!(said(&app, &found), "1 refused", "and the child says what became of the file");
}

/// **The kind draws a mark in the skin's accent, and plain state draws none.**
#[test]
fn a_warning_draws_the_skins_accent_and_a_clean_band_draws_no_mark() {
    let (_root, mut app) = editor("status-mark-kinds");
    record(&mut app, KindVerdict::applied("kit", "kit.yaml"));
    let clean = readout(&mut app);
    assert_eq!(said(&app, &clean), "", "an applied file leaves the band silent");
    assert_eq!(kind(&app, &clean), StatusKind::Info, "nothing wrong is `Info`");
    assert_eq!(
        mark_display(&app, &clean),
        Display::None,
        "plain state draws no mark: gooey's `StatusKind::Info` is the absence of one"
    );
    assert_eq!(mark_ink(&app, &clean), Color::NONE, "and the bar it would draw is clear");

    record(
        &mut app,
        KindVerdict::refused("kit", "kit.yaml", Some("pieces[3].yaw".to_owned()), "not a number"),
    );
    let warned = readout(&mut app);
    assert_eq!(said(&app, &warned), "1 refused");
    assert_eq!(kind(&app, &warned), StatusKind::Warning, "a refusal is the warning kind");
    assert_eq!(
        mark_display(&app, &warned),
        Display::Flex,
        "a warning draws the mark: a visible bar beside the words, which is what the band only ever \
         claimed in a doc comment before this"
    );
    assert_eq!(
        mark_ink(&app, &warned),
        skin(&app).accent,
        "the mark's ink is the running skin's `accent`, so a theme edit moves it"
    );
    assert_ne!(
        mark_ink(&app, &clean),
        mark_ink(&app, &warned),
        "two kinds must not draw the same mark"
    );
    assert_eq!(
        words_ink(&app, &clean),
        words_ink(&app, &warned),
        "the words do not change colour with the kind: gooey reads the kind for the mark, and a \
         second colour on the words would be the hand-written copy this replaced"
    );
    assert_eq!(
        words_ink(&app, &warned),
        skin(&app).text,
        "the words are the skin's `text`, in every kind"
    );
}

/// **The mark and the words take the skin's ink, and the kind is the mark's alone.**
#[test]
fn the_mark_and_the_words_take_the_skins_ink() {
    let (_root, mut app) = editor("status-mark-ink");
    record(
        &mut app,
        KindVerdict::refused("kit", "kit.yaml", Some("pieces[3].yaw".to_owned()), "not a number"),
    );
    let found = readout(&mut app);
    assert_eq!(
        words_ink(&app, &found),
        skin(&app).text,
        "the words are the skin's `text`, as gooey inks a `ControlText` — not an accent this crate \
         computes from the kind"
    );

    let mut moved = skin(&app);
    moved.accent = an_accent_of_its_own();
    moved.text = an_ink_of_its_own();
    app.insert_resource(moved);
    for _ in 0..2 {
        app.update();
    }

    assert_eq!(
        mark_ink(&app, &found),
        an_accent_of_its_own(),
        "a theme edit reaches the mark in the same frame: its fill is `apply_skin`'s, not a colour \
         `show_problems_count` wrote"
    );
    assert_eq!(words_ink(&app, &found), an_ink_of_its_own(), "and so do the words");
    assert_ne!(
        mark_ink(&app, &found),
        words_ink(&app, &found),
        "one kind, one colour: the mark says the kind, and the words stay readable in the skin's text \
         rather than repeating the kind in a second hand-written colour"
    );
}
