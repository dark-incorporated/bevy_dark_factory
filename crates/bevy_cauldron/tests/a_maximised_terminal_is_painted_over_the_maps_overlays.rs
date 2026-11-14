//! **Nothing the map draws over itself paints over a maximised terminal** (#435). Found recording the terminal clip: with the pane at full window
//! (Shift+Cmd+J, #426) the axis gizmo was still drawn over the terminal's text, because the gizmo is a `GlobalZIndex(COMPASS_Z)` layer and the pane
//! was only a local `ZIndex(1)` among its siblings, so the gizmo's layer stood over it.
//!
//! The paint order is read off [`UiStack`] (back to front), the order the renderer and the picker use, not off the components.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::ui::UiStack;
use bevy::window::PrimaryWindow;

use bevy_cauldron::chrome::COMPASS_Z;
use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_pane::TerminalPaneRoot;

use crate::fixtures;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.world_mut().spawn((Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() }, PrimaryWindow));
    settle(&mut app, 12);
    app
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// Every node of the map's overlay layer ([`COMPASS_Z`]: the axis gizmo, the Look chip) and everything below one, as the stack lists them.
fn overlay_nodes(app: &mut App) -> HashSet<Entity> {
    let world = app.world_mut();
    let layers: Vec<Entity> = world.query::<(Entity, &GlobalZIndex)>().iter(world).filter(|(_, z)| z.0 == COMPASS_Z).map(|(e, _)| e).collect();
    let mut found = HashSet::new();
    let mut walk = layers;
    while let Some(entity) = walk.pop() {
        if found.insert(entity)
            && let Some(children) = world.get::<Children>(entity)
        {
            walk.extend(children.iter());
        }
    }
    found
}

/// The stack position of the pane's root and the highest position of any of the map's overlay nodes that is drawn.
fn paint_order(app: &mut App) -> (usize, Option<usize>, usize) {
    let overlay = overlay_nodes(app);
    let world = app.world_mut();
    let pane = world.query_filtered::<Entity, With<TerminalPaneRoot>>().single(world).expect("the terminal's pane");
    let stack = world.resource::<UiStack>();
    let pane_at = stack.uinodes.iter().position(|e| *e == pane).expect("the pane is in the paint stack");
    let drawn: Vec<usize> = stack.uinodes.iter().enumerate().filter(|(_, e)| overlay.contains(e)).map(|(at, _)| at).collect();
    (pane_at, drawn.iter().copied().max(), drawn.len())
}

/// **While the terminal is maximised the pane is painted after every node of the map's overlay layer**, so the gizmo is under its text; lowered
/// again, the map's overlays are as they were (drawn, and not covered).
#[test]
fn a_maximised_terminal_is_painted_over_the_maps_overlays() {
    let mut app = editor("maximised_over_overlays");
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    settle(&mut app, 8);
    let (_, top_overlay, drawn) = paint_order(&mut app);
    assert!(drawn > 0 && top_overlay.is_some(), "the map's overlay layer (the axis gizmo) is drawn on the map: {drawn} node(s)");

    app.world_mut().resource_mut::<Terminal>().toggle_maximised();
    settle(&mut app, 6);
    assert!(app.world().resource::<Terminal>().is_maximised());
    let (pane_at, top_overlay, drawn) = paint_order(&mut app);
    let top_overlay = top_overlay.expect("the overlay nodes are still in the stack");
    assert!(
        pane_at > top_overlay,
        "maximised, the terminal pane (stack position {pane_at}) is painted over the map's overlay layer, but {drawn} overlay node(s) reach position {top_overlay}"
    );

    app.world_mut().resource_mut::<Terminal>().toggle_maximised();
    settle(&mut app, 6);
    assert!(!app.world().resource::<Terminal>().is_maximised());
    let (_, _, drawn_back) = paint_order(&mut app);
    assert_eq!(drawn_back, drawn, "given back its height, the map's overlays are drawn as before");
}
