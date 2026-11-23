//! **Nothing the map draws over itself paints over a maximised terminal** (#435). Found recording the terminal clip: with the pane at full window
//! (Shift+Cmd+J, #426) the axis gizmo was still drawn over the terminal's text, because the gizmo is a `GlobalZIndex(COMPASS_Z)` layer and the pane
//! was only a local `ZIndex(1)` among its siblings, so the gizmo's layer stood over it.
//!
//! The paint order is read off [`UiStack`] (back to front), the order the renderer and the picker use, not off the components.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::ui::UiStack;
use bevy::window::PrimaryWindow;

use bevy_dark_factory::keymap_screen::OpenKeymap;
use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_pane::TerminalPaneRoot;

use crate::fixtures;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.world_mut().spawn((Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() }, PrimaryWindow));
    for _ in 0..12 { app.update(); }
    app
}



/// Every node of the map's overlay layer ([`COMPASS_Z`]: the axis gizmo, the Look chip) and everything below one, as the stack lists them.
fn overlay_nodes(app: &mut App) -> HashSet<Entity> {
    let world = app.world_mut();
    let compass = bevy_dark_factory::style::metrics().z.compass;
    let layers: Vec<Entity> = world.query::<(Entity, &GlobalZIndex)>().iter(world).filter(|(_, z)| z.0 == compass).map(|(e, _)| e).collect();
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
    for _ in 0..8 { app.update(); }
    let (_, top_overlay, drawn) = paint_order(&mut app);
    assert!(drawn > 0 && top_overlay.is_some(), "the map's overlay layer (the axis gizmo) is drawn on the map: {drawn} node(s)");

    app.world_mut().resource_mut::<Terminal>().toggle_maximised();
    for _ in 0..6 { app.update(); }
    assert!(app.world().resource::<Terminal>().is_maximised());
    let (pane_at, top_overlay, drawn) = paint_order(&mut app);
    let top_overlay = top_overlay.expect("the overlay nodes are still in the stack");
    assert!(
        pane_at > top_overlay,
        "maximised, the terminal pane (stack position {pane_at}) is painted over the map's overlay layer, but {drawn} overlay node(s) reach position {top_overlay}"
    );

    app.world_mut().resource_mut::<Terminal>().toggle_maximised();
    for _ in 0..6 { app.update(); }
    assert!(!app.world().resource::<Terminal>().is_maximised());
    let (_, _, drawn_back) = paint_order(&mut app);
    assert_eq!(drawn_back, drawn, "given back its height, the map's overlays are drawn as before");
}

/// gooey's own `dialog()` layer, which Settings, Keymap, project config and New project use unraised.
const GOOEY_DIALOG_Z: i32 = 100;

/// **A dialog opened over a maximised terminal is painted over it**: the Keymap's dialog (a bare gooey `dialog()`) and everything in it come after the pane in
/// the paint order, and the map's overlays are still before it.
#[test]
fn a_dialog_opened_over_a_maximised_terminal_is_painted_over_it() {
    let mut app = editor("maximised_under_dialog");
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    app.world_mut().resource_mut::<Terminal>().toggle_maximised();
    for _ in 0..6 { app.update(); }
    app.world_mut().write_message(OpenKeymap);
    for _ in 0..8 { app.update(); }
    let world = app.world_mut();
    let dialog: Vec<Entity> = world.query::<(Entity, &GlobalZIndex)>().iter(world).filter(|(_, z)| z.0 == GOOEY_DIALOG_Z).map(|(e, _)| e).collect();
    assert!(!dialog.is_empty(), "the Keymap's dialog is up");
    let pane = world.query_filtered::<Entity, With<TerminalPaneRoot>>().single(world).expect("the terminal's pane");
    let stack = world.resource::<UiStack>();
    let pane_at = stack.uinodes.iter().position(|e| *e == pane).expect("the pane is in the paint stack");
    let first_dialog = stack.uinodes.iter().position(|e| dialog.contains(e)).expect("the dialog is in the paint stack");
    assert!(first_dialog > pane_at, "the dialog (stack position {first_dialog}) is painted under the maximised terminal (position {pane_at})");
    let (_, top_overlay, _) = paint_order(&mut app);
    assert!(top_overlay.is_none_or(|top| top < pane_at), "and the map's overlays are still under the terminal");
}
