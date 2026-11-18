//! **The journal's title says what it holds (#447).**
//!
//! After #442 the panel behind `Cmd+E` is `bevy_gooey`'s notification centre: it lists the agent's steps (notes) and the refusals (alerts). Its heading still read
//! "EVERY ERROR THIS SESSION", which is wrong the moment the panel holds only steps. The heading is now the word for what the centre is, and the key's legend
//! line says the same.

use bevy::prelude::*;
use bevy_dark_factory::chrome::{Journal, NotificationsOverlay};
use bevy_dark_factory::harness;

use crate::fixtures;

/// **Every `Text` under the journal's overlay**, whatever its depth.
fn texts_under(app: &mut App, overlay: Entity) -> Vec<String> {
    let world = app.world_mut();
    let mut out = Vec::new();
    let mut stack = vec![overlay];
    while let Some(e) = stack.pop() {
        if let Some(text) = world.get::<Text>(e) {
            out.push(text.0.clone());
        }
        if let Some(children) = world.get::<Children>(e) {
            stack.extend(children.iter());
        }
    }
    out
}

/// **With only steps in it (nothing has gone wrong), the journal is titled as notifications and does not say "error".**
#[test]
fn the_journal_with_only_steps_is_titled_notifications_and_does_not_say_error() {
    let root = fixtures::Fixture::new("journal_title").side(16.0).descriptor("crate", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    let overlay = app.world_mut().query_filtered::<Entity, With<NotificationsOverlay>>().iter(app.world()).next().expect("the journal's overlay is spawned on the Editor screen");
    app.world_mut().get_mut::<Node>(overlay).expect("a node").display = Display::Flex;
    for _ in 0..4 {
        app.update();
    }
    // Nothing went wrong: no problem is held, and the journal holds no refusal.
    assert!(app.world().resource::<Journal>().steps.is_empty(), "a fresh editor has taken no step");

    let texts = texts_under(&mut app, overlay);
    assert!(texts.iter().any(|t| t.eq_ignore_ascii_case("notifications")), "the journal's title is `Notifications`: {texts:#?}");
    assert!(!texts.iter().any(|t| t.to_lowercase().contains("error")), "a journal holding only steps never says `error`: {texts:#?}");
}

/// **The `Cmd+E` legend line says what the key opens**, not only its errors.
#[test]
fn the_cmd_e_legend_line_names_notifications() {
    let said = bevy_dark_factory::keys::binding(bevy_dark_factory::keys::Action::ShowErrors).does;
    assert!(said.contains("notifications"), "the legend line for Cmd+E: {said}");
    assert!(!said.contains("error"), "the legend line for Cmd+E: {said}");
}
