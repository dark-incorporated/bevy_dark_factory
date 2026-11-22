//! **The editor's margins and paddings are gooey's `Spacing`** (#260, UI-99, round 6: bevy_gooey G-85).
//!
//! The Kit panels' spacing was `Node` literals and `entry::<Node>()` patches. It is a [`bevy_gooey::Spacing`] now, in steps of the editor skin's
//! `spacing` scale, and this file pins the two things that could have moved: the scale is the px the editor always used, and a text that stands
//! off its neighbours (a section heading, a panel title) keeps its ink and its font while it does.

use bevy::prelude::*;
use bevy_dark_factory::chrome;
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::Mode;
use bevy_gooey::{GooeySkin, Role};

use crate::fixtures;

fn editor(tag: &str, mode: Mode) -> App {
    let root = fixtures::Fixture::new(&format!("spacing_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **The steps are the px**: `STEP_TIGHT`, `STEP_ROW`, `STEP_PAD` and `STEP_GROUP` read the editor skin's `spacing` scale, and the scale holds the px
/// the editor always used (3, 5, 12, 16). A skin edit that moved a step would move every margin that names it, so this is where it fails first.
#[test]
fn the_editor_spacing_steps_are_its_gap_constants() {
    let app = editor("steps", Mode::Map);
    let skin = app.world().resource::<GooeySkin>();
    for (step, px, name) in [(chrome::STEP_TIGHT, 3.0, "STEP_TIGHT"), (chrome::STEP_ROW, 5.0, "STEP_ROW"), (chrome::STEP_PAD, 12.0, "STEP_PAD"), (chrome::STEP_GROUP, 16.0, "STEP_GROUP")] {
        assert_eq!(step.px(skin), px, "{name} is {px} px in the editor's skin");
        assert_eq!(chrome::space(step), px, "{name} is {px} px in the published scale arithmetic reads");
    }
}

/// **A section heading keeps its ink and its font and stands the margins it stood**: 16 px above, 3 px below, as `chrome::section` always drew it.
#[test]
fn the_section_headings_keep_their_ink_and_their_margins() {
    let mut app = editor("section", Mode::Map);
    let (entity, _) = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &Text)>();
        q.iter(world).find(|(_, text)| text.0 == "PLAN").map(|(e, t)| (e, t.0.clone())).expect("the PLAN section is in the Map's inspector")
    };
    assert_eq!(app.world().get::<Role>(entity), Some(&Role::Sized), "the heading carries gooey's layout role, so `Spacing` is written on it");
    let node = app.world().get::<Node>(entity).expect("a heading has a Node");
    assert_eq!(node.margin.top, Val::Px(chrome::space(chrome::STEP_GROUP)), "16 px above the heading");
    assert_eq!(node.margin.bottom, Val::Px(chrome::space(chrome::STEP_TIGHT)), "3 px below it");
    assert_eq!(app.world().get::<TextColor>(entity).map(|c| c.0), Some(chrome::ink().label), "the heading keeps the label ink the host gave it");
    let size = app.world().get::<TextFont>(entity).map(|f| f.font_size).expect("a heading has a font");
    assert_eq!(size, chrome::font(chrome::text::HEADING).font_size, "and the heading type size");
}

/// **A setting's row stands its 16 px margin below it**, now a `Spacing` on a gooey column.
#[test]
fn a_settings_rows_have_their_group_margin() {
    let mut app = editor("settings", Mode::Map);
    app.world_mut().write_message(bevy_dark_factory::settings_screen::OpenSettings);
    for _ in 0..6 {
        app.update();
    }
    let world = app.world_mut();
    let mut q = world.query::<(&Role, &Node)>();
    let group = chrome::space(chrome::STEP_GROUP);
    let rows = q.iter(world).filter(|(role, node)| **role == Role::Column && node.margin.bottom == Val::Px(group)).count();
    assert!(rows >= 3, "Settings lists its settings as columns with a group margin below each; found {rows}");
}
