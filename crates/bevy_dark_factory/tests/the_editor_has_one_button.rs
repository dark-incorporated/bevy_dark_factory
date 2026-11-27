//! **The editor has one button, and it is gooey's** (#684).
//!
//! `chrome::button` drew a second one by hand: Bevy's `Button`, a `RowRest` fill and a `held_box`, whose `Role` was
//! the layout role `Sized`, so gooey's skin never styled it and a skin edit moved the launcher's buttons and not the
//! panels'. Its 39 call sites spawn `bevy_gooey::button` now. This reads two of them on a running editor: the World
//! workspace's `Pool here` and LIGHT MAP's `Show / hide`.
#![cfg(all(feature = "bevy_lights", feature = "bevy_light_grid"))]

use bevy::prelude::*;
use bevy_dark_factory::chrome::RowRest;
use bevy_dark_factory::light_overlay::LightMapToggle;
use bevy_dark_factory::ops_pool::PoolHere;
use bevy_gooey::Role;

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id()))
        .descriptor("site/torch", "alpha")
        .place("site/torch", (0.0, 0.0))
        .build("m");
    std::fs::write(root.join("family.yaml"), "{version: 1, crates: [\"bevy_lights\"]}").expect("the family file is written");
    // The World workspace, where the pool and light map buttons live (`light_buttons`).
    let mut app = bevy_dark_factory::harness::build_headless_at(&root, "m", None, bevy_dark_factory::tiles::Mode::World).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    app
}

/// The one entity carrying `B`.
fn the<B: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<B>>()
        .single(app.world())
        .unwrap_or_else(|e| panic!("one {}: {e}", std::any::type_name::<B>()))
}

/// **A panel's command button is gooey's `button`**: `Role::Button`, so the skin paints its fill, frame, hover and
/// press; Bevy's `Button`, so a click still reaches its `Activate` observer; its words `Role::ButtonText`, so a screen
/// reader names it by them; and no `RowRest`, so the editor's row repainter does not paint it a second time.
#[test]
fn a_panels_command_button_is_gooeys_button() {
    let mut app = editor("one_button");
    let buttons = [(the::<PoolHere>(&mut app), "Pool here"), (the::<LightMapToggle>(&mut app), "Show / hide")];
    let world = app.world();
    for (entity, words) in buttons {
        assert_eq!(world.get::<Role>(entity), Some(&Role::Button), "`{words}` is a gooey button, which the skin styles");
        assert!(world.get::<bevy::ui_widgets::Button>(entity).is_some(), "`{words}` is on the `Activate` bus its observer listens to");
        assert!(world.get::<RowRest>(entity).is_none(), "`{words}` is painted by the skin alone, not by the row repainter as well");
        let label = world
            .get::<Children>(entity)
            .into_iter()
            .flat_map(|c| c.iter())
            .find(|c| world.get::<Text>(*c).is_some())
            .unwrap_or_else(|| panic!("`{words}` holds its words"));
        assert_eq!(world.get::<Text>(label).map(|t| t.0.as_str()), Some(words), "the button says what it does");
        assert_eq!(world.get::<Role>(label), Some(&Role::ButtonText), "`{words}`'s words are a button's, so a screen reader names it by them");
    }
}
