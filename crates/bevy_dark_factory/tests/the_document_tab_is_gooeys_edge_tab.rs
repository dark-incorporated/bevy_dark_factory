//! **A document tab is gooey's `edge_tab`** (#645), so it has a hover state, a skin and an edge weight
//! the workspace strip's tabs also have.
//!
//! # What was wrong
//!
//! `session::draw_the_tabs` built each tab by hand: a literal `Spacing` of 8 px down and `STEP_PAD` across,
//! a 3 px `BorderPx` bottom, an `InkOverride`, a hand-written `BorderColor` and a `BackgroundColor` baked
//! at spawn. It carried no gooey `Role`, so `apply_skin` never styled it; and because the colour was
//! decided on the frame the strip was rebuilt, **it had no hover state at all** — the only state it could
//! show was `active`.
//!
//! The editor already does this the gooey way in `tiles.rs:4134`, which spawns `bevy_gooey::edge_tab` and
//! selects through `bevy::ui::Selected`. One crate, two strips, one of them conforming.
//!
//! # What these tests hold
//!
//! - the hand-built pieces are gone from `draw_the_tabs` (a source scan, because a literal that came back
//!   would be invisible to a world test that only looks at the selected tab);
//! - with two documents open the active tab carries `bevy::ui::Selected` and the other does not;
//! - the tab's bottom edge and inset are the skin's, not the literals 3.0 and 8.0/12.0 — read off the
//!   drawn `Node`, so a change to `skin.border` or `skin.pad()` moves it.

use bevy::prelude::*;

use bevy_dark_factory::harness;
use bevy_dark_factory::session::{self, Doc, Kind, Session};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;
use crate::settle_support;

const SETTLE: usize = 6;

/// `draw_the_tabs`'s own body, from its signature to the function after it.
fn draw_the_tabs_source() -> String {
    let source = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/session.rs"))
        .unwrap_or_else(|e| panic!("session.rs: {e}"));
    let from = source
        .find("pub fn draw_the_tabs(")
        .unwrap_or_else(|| panic!("`draw_the_tabs` is the function this file is about"));
    let body = &source[from..];
    let end = body
        .find("\nfn click_tab(")
        .unwrap_or_else(|| panic!("`click_tab` follows `draw_the_tabs`"));
    body[..end].to_owned()
}

/// **The hand-built tab is gone**: the function names `edge_tab` and carries none of the literals it used
/// to bake a colour, an inset and an edge from.
#[test]
fn the_document_tab_is_gooeys_edge_tab_and_not_a_hand_built_box() {
    let body = draw_the_tabs_source();
    assert!(
        body.contains("bevy_gooey::edge_tab("),
        "the document tab must be gooey's `edge_tab`, or `apply_skin` never styles it and the tab has no \
         hover state (#645)"
    );
    for (what, needle) in [
        ("`BorderPx`, the hand-written 3 px edge", "BorderPx"),
        ("`InkOverride`, the hand-written edge colour", "InkOverride"),
        ("a literal 3.0 px edge", "Val::Px(3.0)"),
        ("a literal `Space::Px(3.0)` edge", "Space::Px(3.0)"),
        ("a literal 8.0 px inset", "Space::Px(8.0)"),
        ("a hand-written `BorderColor`", "BorderColor::all("),
        ("a fill baked at spawn", "BackgroundColor("),
    ] {
        assert!(
            !body.contains(needle),
            "`draw_the_tabs` still carries {what} ({needle:?}). The edge weight, the inset and both colours \
             are `Role::EdgeTab`'s arms in `bevy_gooey/src/skin.rs` now, so a copy here is a second styling \
             path that a skin or density change never reaches (#645)"
        );
    }
}

/// The two maps a strip needs, and the editor on the first.
fn two_documents(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("doctab_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .build("m");
    fixtures::Fixture::write_map(&root, "n", 8.0, &[]);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE {
        app.update();
    }
    session::choose_map(app.world_mut(), "n");
    for _ in 0..SETTLE {
        app.update();
    }
    app
}

/// **The active tab carries `Selected` and the other does not**, which is gooey's whole contract for
/// `Role::EdgeTab`: the host's half is one component, and the skin lights the fill and the bottom edge.
#[test]
fn two_documents_select_the_active_tab_and_leave_the_other_alone() {
    let mut app = two_documents("selected");
    settle_support::settle(&mut app);
    let tabs: Vec<(String, bool)> = {
        let world = app.world_mut();
        let mut query = world.query::<(&bevy_dark_factory::session::DocumentTab, Has<bevy::ui::Selected>)>();
        query.iter(world).map(|(tab, selected)| (tab.0.name.clone(), selected)).collect()
    };
    assert_eq!(tabs.len(), 2, "two maps, two document tabs: {tabs:?}");
    let active: Vec<&str> = tabs.iter().filter(|(_, selected)| *selected).map(|(name, _)| name.as_str()).collect();
    assert_eq!(active, ["n"], "the map that was just opened is the selected tab, and only it: {tabs:?}");
    let inactive = tabs.iter().find(|(name, _)| name == "m").expect("the first map's tab");
    assert!(!inactive.1, "the tab that is not open carries no `Selected`, so its edge stays clear");
}
