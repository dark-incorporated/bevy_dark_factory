//! **A switch between workspaces keeps the scene** (#695; the other side of #678 and the switch half of #541).
//!
//! `docs/ui/vision.md` §4, `workspace::shows_the_map` and `screen::cross`'s own comment all say one thing: Map,
//! World, Kit and Animation share **one viewport**, so a switch changes which *panels* are drawn and nothing
//! else — never a map, never a selection, never the scene.
//!
//! Measured on `big` under Xvfb on 2026-11-27 (`a0782796`, `bedroom` open), the editor did the opposite: after a
//! switch to World, Kit or Animation the map's `Mesh3d` entities were **1, 6 and 13** of 45, and the viewport was
//! black but for the axis gizmo. The cause was `screen::cross` → `rebuild_the_door` → `NextState(Screen::Editor)`,
//! whose same-state transition runs `close_the_door`, whose first line is `despawn_scene` — so the whole door, the
//! frame and the map's own meshes with it, was torn down and rebuilt per switch. The resources crossed
//! (`tests/a_workspace_switch_keeps_the_map.rs` proves those) and the picture did not.
//!
//! This file is the picture: the map's entities have to survive the round trip, and so does the frame.
//!
//! `[INFERENCE]`-free, because a headless editor draws no pixels: the claim is about entities and the components
//! that make one drawable, which is what `Mesh3d` is — see `crate::thumbs` on why the component alone is not a
//! drawn mesh, and `devshot` on the query a still uses.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;

use bevy_dark_factory::harness;
use bevy_dark_factory::keys::{binding, Action};
use bevy_dark_factory::project::OpenMap;
use bevy_dark_factory::tiles::Mode;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// Frames the editor is given to settle after a switch: the transition runs in the frame after the key, and the
/// panels are spawned on the frame after that.
const SETTLE_FRAMES: usize = 10;

/// **At least this share of the interface's nodes must survive a round trip**, when the frame is kept rather
/// than rebuilt. Nine in ten: a switch may add the panels the other workspace shows and drop the ones it does
/// not, which is a fraction of the frame, and must not renumber the rest.
const LEAST_KEPT_SHARE: f64 = 0.9;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("keep_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .descriptor("alpha/wall", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .place_as("wall@0", "alpha/wall", (2.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{_e}", _e = e));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    app
}

/// Hold a chord down for two frames and let it go, as a person's keyboard does and as `command_run` does for an
/// agent: the modifier and the key together, so `needs_mod` sees one state.
fn press(app: &mut App, action: Action) {
    let row = binding(action);
    let mut keys = vec![row.key];
    if row.needs_mod {
        keys.push(if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft });
    }
    if row.needs_shift == Some(true) {
        keys.push(KeyCode::ShiftLeft);
    }
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in &keys {
            app.world_mut().write_message(KeyboardInput {
                key_code: *code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
        app.update();
    }
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

fn chord_to(app: &mut App, workspace: Workspace) {
    let action = Action::ALL
        .into_iter()
        .find(|a| a.workspace() == Some(workspace))
        .unwrap_or_else(|| panic!("no chord goes to the {} workspace", workspace.title()));
    press(app, action);
    assert_eq!(
        app.world().resource::<Workspace>(),
        &workspace,
        "the key did not switch to the {} workspace",
        workspace.title()
    );
}

/// **The map's meshes**, as entities: what a person is looking at when they say "the room".
fn meshes(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<Mesh3d>>();
    q.iter(world).collect()
}

/// **The frame's nodes**, as entities: every laid-out node that is the shell's rather than a panel's.
///
/// The chrome bar, the docks, the viewport slot, the status band and the strip — **and nothing a panel drew**.
/// A `WorkspacePanel` marks the panel, and its children are the panel's contents, so the panel's whole subtree
/// is excluded. Without that the count would include the rows, fields and labels of the panels, and a switch
/// that legitimately draws a different panel would look like a rebuilt frame.
fn frame_nodes(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    let panels: Vec<Entity> = world
        .query_filtered::<Entity, With<bevy_dark_factory::screen::WorkspacePanel>>()
        .iter(world)
        .collect();
    // Every entity that is a descendant of a panel, worked out once from the parent links.
    let mut under: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    let pairs: Vec<(Entity, Entity)> = world.query::<(Entity, &ChildOf)>().iter(world).map(|(e, p)| (e, p.parent())).collect();
    for panel in panels {
        let mut frontier = vec![panel];
        while let Some(at) = frontier.pop() {
            for (child, parent) in &pairs {
                if *parent == at && under.insert(*child) {
                    frontier.push(*child);
                }
            }
        }
    }
    let mut q = world.query_filtered::<Entity, With<bevy::ui::ComputedNode>>();
    q.iter(world).filter(|e| !under.contains(e)).collect()
}

/// **The map's meshes survive Map → World → Kit → Animation → Map**, which is what the three places say a switch
/// between the four does.
///
/// Fails on the tree before the fix: `cross` went through `rebuild_the_door`, whose same-state
/// `NextState(Screen::Editor)` runs `close_the_door` and so `despawn_scene`, so every one of the map's meshes was
/// gone by the time the next workspace's frame was drawn.
#[test]
fn the_maps_meshes_survive_a_round_trip_through_the_workspaces_that_share_its_viewport() {
    let mut app = editor("meshes");
    // Something to lose: only a map with pieces on it can tell a kept scene from a rebuild.
    let before: Vec<Entity> = meshes(&mut app);
    assert!(!before.is_empty(), "the fixture's map must draw meshes, or this test measures an empty world");

    for workspace in [Workspace::World, Workspace::Kit, Workspace::Animation, Workspace::Map] {
        chord_to(&mut app, workspace);
        for _ in 0..SETTLE_FRAMES {
            app.update();
        }
        let now: Vec<Entity> = meshes(&mut app);
        let kept = before.iter().filter(|e| now.contains(e)).count();
        assert_eq!(
            kept,
            before.len(),
            "the {} workspace has {kept} of the map's {} meshes: a switch despawned the scene, where \
             `docs/ui/vision.md` §4 says Map, World, Kit and Animation share one viewport",
            workspace.title(),
            before.len()
        );
        assert_eq!(
            app.world().resource::<OpenMap>().map.name,
            "m",
            "the switch to {} reloaded the map",
            workspace.title()
        );
    }
}

/// **A round trip keeps the frame**, so a switch is a change of panels rather than a rebuild of the interface.
///
/// The review's numbers (`#678`): a fresh editor kept **2 of 2,624** nodes and cost 205–275 ms a switch. With
/// the frame kept, what a switch replaces is the panels — which are a large share of the nodes, because a
/// panel's contents are most of what is drawn in one. So the claim is about the **frame**: the shell's own
/// nodes, the ones the editor opened with, are still there after a round trip.
///
/// **Not all UI nodes.** A Map → Kit → Map round trip legitimately draws a different inspector and list, so
/// counting every node would measure the panels and call their difference a rebuild — which is the mistake
/// this comment exists to stop the next reader making.
#[test]
fn a_round_trip_keeps_the_frame() {
    let mut app = editor("frame");
    let before: Vec<Entity> = frame_nodes(&mut app);
    assert!(before.len() > 100, "the shell must draw an interface, or this test measures nothing");

    for workspace in [Workspace::Kit, Workspace::Map] {
        chord_to(&mut app, workspace);
        for _ in 0..SETTLE_FRAMES {
            app.update();
        }
    }
    let after: Vec<Entity> = frame_nodes(&mut app);
    let kept = before.iter().filter(|e| after.contains(e)).count();
    let share = kept as f64 / before.len() as f64;
    assert!(
        share >= LEAST_KEPT_SHARE,
        "a map→kit→map round trip kept {kept} of {} UI nodes ({:.1}%, new {}, gone {}): the whole interface was \
         despawned and respawned, which is the 205–275 ms switch #678 measured",
        before.len(),
        100.0 * share,
        after.iter().filter(|e| !before.contains(e)).count(),
        before.iter().filter(|e| !after.contains(e)).count()
    );
}

/// **The panel follows the workspace, and the frame it lives in does not move.**
///
/// The two halves of the switch are separable and both must hold: what changes is *which panel* is on screen
/// (`Mode`), and what does not is the frame around it. Before the fix the panel followed and the frame did not
/// survive, so this test's first half passed and its second could not be written.
#[test]
fn the_panel_moves_with_the_workspace_and_the_frame_does_not() {
    let mut app = editor("panel_and_frame");
    let frame_roots = |app: &mut App| -> Vec<Entity> {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<bevy_dark_factory::chrome::ViewportSlot>>();
        q.iter(world).collect()
    };
    let viewport = frame_roots(&mut app);
    assert_eq!(viewport.len(), 1, "the frame has one viewport slot");

    chord_to(&mut app, Workspace::Kit);
    assert_eq!(app.world().resource::<Mode>().label(), "MESHES", "Kit opens on its first panel");
    assert_eq!(
        frame_roots(&mut app),
        viewport,
        "the viewport slot was respawned: the frame was torn down where only the panels should have changed"
    );

    chord_to(&mut app, Workspace::Animation);
    assert_eq!(app.world().resource::<Mode>().label(), "ANIM", "Animation shows `ANIM`, so the panel follows");
    assert_eq!(frame_roots(&mut app), viewport, "and the frame is still the one the editor opened with");
}

/// **A switch builds the panels the new workspace shows, and takes down the ones it does not.**
///
/// Regression (2026-11-27, `integrate/1127`): the first version of this fix returned before the state
/// transition, so `OnEnter(Editor)` — the one place a panel is spawned — never ran, and World's WATER
/// heading was never drawn. Keeping the frame must not mean keeping the old panels.
///
/// Every destination is named by a marker its own module exports, so this is the panel itself rather than
/// a count of nodes that could be anything.
#[test]
fn a_switch_builds_the_panels_the_new_workspace_shows() {
    use bevy_dark_factory::tiles::{AnimRoot, MapRoot};
    use bevy_dark_factory::workspace_panels::WorldRoot;

    let count = |app: &mut App, name: &str| -> usize {
        let world = app.world_mut();
        match name {
            // **`MapRoot` is on the inspector panel and on the PLACE palette** (the palette is tagged so
            // `tiles::apply_mode` hides it with the tab), so this counts *panels* carrying it: the take-down
            // claim below is made on the World and Animation panels, which have exactly one each.
            "map" => {
                let mut q = world.query_filtered::<Entity, (With<MapRoot>, With<bevy_dark_factory::screen::WorkspacePanel>)>();
                q.iter(world).count()
            }
            "anim" => {
                let mut q = world.query_filtered::<Entity, With<AnimRoot>>();
                q.iter(world).count()
            }
            "world" => {
                let mut q = world.query_filtered::<Entity, With<WorldRoot>>();
                q.iter(world).count()
            }
            _ => unreachable!(),
        }
    };

    let mut app = editor("panels");
    assert!(count(&mut app, "map") > 0, "the Map workspace draws its panel");
    assert_eq!(count(&mut app, "world"), 0, "and Map does not draw World's");

    chord_to(&mut app, Workspace::World);
    assert!(count(&mut app, "world") > 0, "the World workspace's panel is built on arrival");

    chord_to(&mut app, Workspace::Animation);
    assert!(count(&mut app, "anim") > 0, "the Animation workspace's bench is built on arrival");
    assert_eq!(count(&mut app, "world"), 0, "and World's panel is taken down, or two would stand in one dock");

    // **Animation keeps its panel on every workspace**, hidden rather than despawned: its bench holds the rig
    // and the clip, and a switch that dropped it would stop a playback the person left running. So the claim
    // here is that it is still *there*, and that it is not the one on screen.
    chord_to(&mut app, Workspace::Map);
    assert!(count(&mut app, "map") > 0, "and Map's is built again on the way back");
    assert!(count(&mut app, "anim") > 0, "the bench outlives the switch, as its clip does");
}
