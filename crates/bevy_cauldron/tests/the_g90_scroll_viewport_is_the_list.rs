//! **gooey's scroll viewport is chrome's scroll list** (#260, UI-99, round 11: bevy_gooey G-90).
//!
//! `chrome::scroll_list` is `bevy_gooey::scroll_viewport` with the editor's gutter and gap; the rail gooey spawns beside it is tagged `ScrollTrack` so the wheel-over-the-bar forwarding and the
//! tests that find the bar keep working. This file asks a running editor what a list is and whether the end of it can be reached.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_cauldron::chrome::ScrollTrack;
use bevy_cauldron::harness;
use bevy_cauldron::tiles::Mode;
use bevy_gooey::{Role, ScrollRailOf};

use crate::fixtures;

fn editor(tag: &str, mode: Mode) -> App {
    let root = fixtures::Fixture::new(&format!("g90_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..12 {
        app.update();
    }
    app
}

/// **Every list is a scroll viewport with a rail beside it that is a scroll track**, its rows `GAP_TIGHT` px apart and `SCROLL_GUTTER` clear on its right.
#[test]
fn every_list_is_a_scroll_viewport_with_a_tagged_rail() {
    for (mode, name) in [(Mode::Map, "map"), (Mode::Tiles, "tiles"), (Mode::Compose, "compose")] {
        let mut app = editor(name, mode);
        let world = app.world_mut();
        let viewports: Vec<(Entity, f32)> = world
            .query::<(Entity, &Role, &Node)>()
            .iter(world)
            .filter(|(_, role, _)| **role == Role::ScrollViewport)
            .map(|(e, _, n)| {
                assert_eq!(n.row_gap, Val::Px(3.0), "{name}: a list's rows are 3 px apart");
                (e, match n.padding.right { Val::Px(p) => p, other => panic!("{other:?}") })
            })
            .collect();
        assert!(!viewports.is_empty(), "{name}: the door has at least one list");
        for (view, gutter) in &viewports {
            assert!(*gutter >= 5.0, "{name}: the rail's lane is kept clear: {gutter}");
            let rails = world.query::<(&ScrollRailOf, Has<ScrollTrack>)>().iter(world).filter(|(of, _)| of.0 == *view).collect::<Vec<_>>();
            assert_eq!(rails.len(), 1, "{name}: {view:?} has one rail");
            assert!(rails[0].1, "{name}: and it is a scroll track");
        }
    }
}

/// **The end of a list can be reached**: a list taller than its box scrolls to `content - visible` and stays there, so its last row is on screen.
#[test]
fn a_list_taller_than_its_box_scrolls_to_its_end() {
    let mut app = editor("end", Mode::Map);
    let world = app.world_mut();
    let tall: Vec<(Entity, f32)> = world
        .query::<(Entity, &Role, &ComputedNode)>()
        .iter(world)
        .filter(|(_, role, _)| **role == Role::ScrollViewport)
        .filter_map(|(e, _, c)| {
            let range = (c.content_size().y - (c.size().y - c.scrollbar_size.y)) * c.inverse_scale_factor();
            (range > 1.0).then_some((e, range))
        })
        .collect();
    assert!(!tall.is_empty(), "the Map door's inspector is taller than its box");
    for (view, range) in &tall {
        app.world_mut().get_mut::<ScrollPosition>(*view).expect("a ScrollPosition").y = *range;
    }
    for _ in 0..4 {
        app.update();
    }
    for (view, range) in tall {
        let at = app.world().get::<ScrollPosition>(view).map(|p| p.y).unwrap_or(0.0);
        assert!(at >= range - 1.0, "{view:?} stays at its end {range}, not {at}");
    }
}
