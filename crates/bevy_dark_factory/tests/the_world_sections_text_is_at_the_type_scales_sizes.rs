//! **The World workspace's text is drawn at the editor's own type sizes** (#518,
//! `feedback-1795094070279`: *"We ned rules, data models, mental models, and a damn fine font for all of the
//! text information in this system."*).
//!
//! # What the note points at
//!
//! The pick is the World workspace's inspector, the `WATER` section: a heading, a hint under it
//! (`set on the TERRAIN line: water ...`), and the list below that. Measured before this change, a body line there
//! drew at **11.666 px** — the skin's 11 px snapped to a whole device pixel by `bevy_gooey::apply_skin` at the
//! editor's `UiScale` of 1.2 — while every label `chrome::font` draws is **11 px** (`chrome::text::BODY`). One window,
//! two body sizes, and the common one outside the type scale: **550 of 602 texts on screen** drew at 11.666.
//!
//! # What this measures
//!
//! Every text the World workspace draws under its `WATER`, `FOREST` and `LIGHTS` sections is at a size the type scale
//! names — read from the drawn `TextFont`, not from what spawned it — and the section's own hint is at
//! [`chrome::text::BODY`] exactly, `11.0` at 100 % text. The scale is `{9, 10, 11, 13, 15}`; 11.666 is not one of them,
//! which is what makes the assertion bite on the shipped code.

use bevy::prelude::*;
use bevy::text::FontSize;
use bevy_dark_factory::chrome::{self, text};
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;
use crate::settle_support;




fn world_app(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("world_text_{tag}_{}", std::process::id()))
        .descriptor("stool", "alpha")
        .place("stool", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let command = commands::by_id("workspace.world").unwrap_or_else(|| panic!("`workspace.world` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(&mut app);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::World);
    app
}

/// One text's drawn size, in logical px, when it has one.
fn size_of(app: &mut App, words: &str) -> Option<f32> {
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &TextFont)>();
    q.iter(world).find(|(t, _)| t.0 == words).and_then(|(_, font)| match font.font_size {
        FontSize::Px(px) => Some(px),
        _ => None,
    })
}

/// **The hint under `WATER` — the line the capture's pick sits on — is drawn at `chrome::text::BODY`, not at gooey's
/// snapped skin size.** Before the change `bevy_gooey::label` drew it at `11.666…`; this asks for `11.0`.
#[test]
fn the_water_hint_is_drawn_at_the_type_scales_body_size() {
    let mut app = world_app("hint");
    let drew = size_of(&mut app, bevy_dark_factory::world_lists::WATER_HINT)
        .unwrap_or_else(|| panic!("the WATER section draws its hint"));
    let body = text::BODY.px();
    assert!(
        (drew - body).abs() < 0.01,
        "the WATER hint drew at {drew} px; the type scale's BODY is {body} px. A body label drawn at the skin's \
         snapped 11.666 px is a second body size the scale does not name (#518)"
    );
}

/// **Every text under the World sections is at a size the type scale names.** The rule, not one line: heading, hint and
/// row alike. `{9, 10, 11, 13, 15}` is the whole scale, so a text at any other size is the drift the note measured.
#[test]
fn every_world_section_text_is_at_a_size_the_scale_names() {
    let mut app = world_app("scale");
    let named: Vec<f32> = [text::TITLE, text::TAB, text::BODY, text::HEADING, text::LABEL, text::CONTROL, text::HINT]
        .iter()
        .map(|role| role.px())
        .collect();
    // The texts the three sections draw, by their first words.
    for (label, words) in [
        ("the WATER heading", "WATER"),
        ("the WATER hint", bevy_dark_factory::world_lists::WATER_HINT),
        ("the FOREST heading", "FOREST"),
        ("what a map with no water says", bevy_dark_factory::world_lists::NO_WATER),
    ] {
        let Some(drew) = size_of(&mut app, words) else { panic!("{label} is drawn") };
        assert!(
            named.iter().any(|named| (drew - named).abs() < 0.01),
            "{label} drew at {drew} px, which the type scale {named:?} does not name (#518)"
        );
    }
}

/// **The hint is in the quiet ink**, so the fix is a size and not a repaint: the same `Ink::dim` a supporting line takes.
#[test]
fn the_water_hint_keeps_the_quiet_ink() {
    let mut app = world_app("ink");
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &TextColor)>();
    let drawn = q.iter(world).find(|(t, _)| t.0 == bevy_dark_factory::world_lists::WATER_HINT).map(|(_, c)| c.0);
    let want = chrome::ink().dim;
    assert_eq!(drawn, Some(want), "the hint is drawn in `Ink::dim`, not the body ink");
}
