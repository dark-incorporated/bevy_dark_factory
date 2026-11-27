//! **The inspector's history is its own section, closed by default, so the rows below it are reachable without
//! scrolling** (#691).
//!
//! # What the capture measured
//!
//! At 1360×850 the Map inspector held a **168 px band with no text** between the `now:` bar and EDGES: the
//! history list reserved a screenful (`HISTORY_LIST_ROWS`, 6.5 rows) for rows the session did not have. Z stood
//! at the panel's bottom edge and FAMILY SETTINGS was below the window, while a fifth of the panel sat empty.
//!
//! # The owner's call: collapsible
//!
//! The block is gooey's own `section` — a header with a chevron that Enter, Space, South or a click toggles —
//! and it starts **closed**. Closed, it is its header and nothing else: its body is not laid out and focus
//! never stops in it, so the rows under it move up by exactly the height the open list held. **Opened, E-018's
//! rule stands unchanged**: the list is the same fixed six and a half rows however many entries the session
//! holds, so the rows below it do not move as edits arrive.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::WindowResolution;

use bevy_dark_factory::editor::{HistoryTree, MapControlsPane};
use bevy_gooey::{SectionHeader, SectionOpen};
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::Mode;

use crate::fixtures;
use crate::settle_support;

/// The window the capture used.
const WINDOW: (u32, u32) = (1360, 850);
/// The interface scale the editor ships at.
const UI_SCALE: f32 = 1.2;
/// Frames a layout needs to settle after the surface is resized.
const SETTLE_FRAMES: usize = 120;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(&format!("history_section_{name}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .place("floor", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy::ui::UiScale(UI_SCALE));
    // **The window is spawned at the size captured, not at `Window::default`** — `surface::fit_surface_to_window`
    // sizes the surface from the primary window every frame, so a default 1280 x 720 window overrides a later
    // `resize_surface` and the panel measured is half the one a person saw (the trap `RUNNING-A-LOOP.md` records).
    app.world_mut().spawn((
        Window { resolution: WindowResolution::new(WINDOW.0, WINDOW.1), ..default() },
        bevy::window::PrimaryWindow,
    ));
    harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    app
}

/// The history tree, its section, and the section's body.
///
/// **The section is the nearest ancestor that carries `SectionOpen`**, and the body is its `SectionBody`
/// child: `section()` spawns `[header, body, note]` and `children![..]` puts the tree inside the body, so the
/// chain is tree -> body -> section.
fn parts(app: &mut App) -> (Entity, Entity, Entity) {
    let world = app.world_mut();
    let tree = world
        .query_filtered::<Entity, With<HistoryTree>>()
        .iter(world)
        .next()
        .unwrap_or_else(|| panic!("the inspector draws no history tree"));
    let mut at = tree;
    let mut chain = Vec::new();
    for _ in 0..5 {
        let Some(parent) = world.get::<ChildOf>(at).map(ChildOf::parent) else { break };
        chain.push(parent);
        at = parent;
    }
    let section = chain
        .iter()
        .copied()
        .find(|e| world.get::<SectionOpen>(*e).is_some())
        .unwrap_or_else(|| panic!("no `SectionOpen` above the history tree; chain: {chain:?}"));
    let kids: Vec<Entity> = world
        .get::<Children>(section)
        .map(|kids| kids.iter().collect())
        .unwrap_or_default();
    let body = kids
        .into_iter()
        .find(|kid| world.get::<bevy_gooey::Role>(*kid) == Some(&bevy_gooey::Role::SectionBody))
        .unwrap_or_else(|| panic!("no `SectionBody` under the section"));
    (tree, section, body)
}

/// **The history starts closed**, and its body is not laid out.
#[test]
fn the_history_section_starts_closed() {
    let mut app = editor("closed");
    let (_tree, section, body) = parts(&mut app);
    let open = app.world().get::<SectionOpen>(section).expect("a gooey section carries `SectionOpen`").0;
    assert!(!open, "the history is closed when the inspector is built (#691: the empty band was a screenful it did not have)");
    let node = app.world().get::<Node>(body).expect("a section body has a `Node`");
    assert_eq!(node.display, Display::None, "a closed section's body is not laid out");
    // **The header is drawn**: closed is a header, not a disappearance.
    let world = app.world_mut();
    let heading = world
        .get::<Children>(section)
        .map(|kids| kids.to_vec())
        .unwrap_or_default();
    assert!(
        heading.iter().any(|kid| world.get::<SectionHeader>(*kid).is_some()),
        "a closed section is still its header: {heading:?}"
    );
}

/// **The rows below the closed section are on screen at 1360×850 without scrolling.**
///
/// The row is found by its own words, and "on screen" is the same rule the inspector's other reach tests use:
/// its top is at or above the scroll viewport's bottom.
#[test]
fn the_rows_below_the_closed_history_are_on_screen() {
    let mut app = editor("reach");
    let (_, _, body) = parts(&mut app);

    // **The body takes no room**, which is the height the rows below it gain.
    let body_height = box_of(&app, body).map(|r| r.height()).unwrap_or(0.0);
    assert!(
        body_height < 1.0,
        "a closed section's body is laid out to no height; it measured {body_height} px"
    );

    // **The furthest row the issue names is inside the panel**: FAMILY SETTINGS' heading and the MAP SIZE
    // field above it both stand above the inspector's own bottom edge.
    let pane = app
        .world_mut()
        .query_filtered::<Entity, With<MapControlsPane>>()
        .iter(app.world())
        .next()
        .expect("the inspector's scroll");
    let pane = box_of(&app, pane).expect("the inspector is laid out");
    for words in ["MAP SIZE  (m)", "FAMILY SETTINGS"] {
        let at = row_top(&mut app, words)
            .unwrap_or_else(|| panic!("the inspector draws no `{words}` row"));
        assert!(
            at <= pane.max.y + 0.5,
            "`{words}` stands at y {at:.1} and the inspector's panel ends at {:.1} — the closed history did not \
             give its room back (#691)",
            pane.max.y
        );
        assert!(
            at >= pane.min.y - 0.5,
            "`{words}` stands above the inspector's own top: {at:.1} over {:.1}",
            pane.min.y
        );
    }
}

/// **Opening the section shows the list**, and the section's state survives the inspector being rebuilt.
#[test]
fn opening_the_history_shows_the_list() {
    let mut app = editor("opened");
    let (_, section, body) = parts(&mut app);
    app.world_mut().entity_mut(section).insert(SectionOpen(true));
    settle_support::settle(&mut app);
    let node = app.world().get::<Node>(body).expect("a section body has a `Node`");
    assert_eq!(node.display, Display::Flex, "an open section lays its body out");
    let height = app
        .world()
        .get::<ComputedNode>(body)
        .map(|c| c.size().y * c.inverse_scale_factor())
        .unwrap_or(0.0);
    assert!(height > 0.0, "the open body has the list's height, not none: {height}");
}

/// A node's drawn box, logical px.
fn box_of(app: &App, entity: Entity) -> Option<Rect> {
    let computed = app.world().get::<ComputedNode>(entity)?;
    let at = app.world().get::<UiGlobalTransform>(entity)?.translation * computed.inverse_scale_factor();
    Some(Rect::from_center_size(at, computed.size() * computed.inverse_scale_factor()))
}

/// **The top of the drawn text that reads `words`**, in logical px from the window's own top.
fn row_top(app: &mut App, words: &str) -> Option<f32> {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &Text)>();
    let found: Vec<Entity> = query
        .iter(world)
        .filter(|(_, text)| text.0 == words)
        .map(|(entity, _)| entity)
        .collect();
    found.into_iter().filter_map(|entity| box_of(app, entity).map(|r| r.min.y)).fold(None, |held, at| {
        Some(held.map_or(at, |held: f32| held.min(at)))
    })
}
