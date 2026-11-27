//! **A switch between the workspaces that show the map keeps the map's scene** (#695).
//!
//! `docs/ui/vision.md` §4, `workspace::shows_the_map` and `screen::cross`'s own comment all say one thing: Map,
//! World, Kit and Animation share **one viewport**, so a switch among them changes which *panels* are drawn and
//! never the map, the selection or the picture.
//!
//! Measured on `big` under Xvfb on 2026-11-27 (`a0782796`, `bedroom` open), the editor did the opposite: after a
//! switch to World, Kit or Animation the map's `Mesh3d` entities were **1, 6 and 13** of 45, and the viewport was
//! black but for the axis gizmo. The cause was `screen::cross` → `rebuild_the_door` → `NextState(Screen::Editor)`,
//! whose same-state transition runs `close_the_door`, whose first line is `despawn_scene` — so the map's own
//! meshes went with the door on every switch. The resources crossed (`tests/a_workspace_switch_keeps_the_map.rs`
//! proves those) and the picture did not.
//!
//! **What a switch keeps is `screen::TheMapsScene` — the camera, its key light and what the map draws — and
//! nothing else.** The frame and its panels are still swept and spawned again. A first cut of this fix kept the
//! frame and swept only the panels, and every `OnEnter(Editor)` spawner that was not a panel then drew a second
//! copy beside the first: two main cameras, two entities on every placement id, a frame resource that outlived a
//! leave (`integrate/1127`, 2026-11-27). So these tests say both halves: the room survives, and nothing is drawn
//! twice.
//!
//! `[INFERENCE]`-free, because a headless editor draws no pixels: the claim is about entities and the components
//! that make one drawable, which is what `Mesh3d` is — see `crate::thumbs` on why the component alone is not a
//! drawn mesh, and `devshot` on the query a still uses.

use std::collections::{HashMap, HashSet};

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;

use bevy_dark_factory::editor::Placement;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::{binding, Action};
use bevy_dark_factory::project::OpenMap;
use bevy_dark_factory::view::MainCamera;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// Frames the editor is given to settle after a switch: the transition runs in the frame after the key, and the
/// panels are spawned on the frame after that.
const SETTLE_FRAMES: usize = 10;

/// **Two pieces drawn as footprint markers**, so the room has meshes in a headless editor: a `.glb` piece's meshes
/// come from a scene the asset server instantiates, and this harness had none under a `.glb` placement after 300
/// frames (measured on `big`, 2026-11-27), while a marker's slab and notch are `Mesh3d` the frame the piece spawns.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("keep_{tag}_{}", std::process::id()))
        .flat_descriptor("alpha/floor", 1.0, 1.0)
        .flat_descriptor("alpha/wall", 1.0, 1.0)
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .place_as("wall@0", "alpha/wall", (2.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
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

/// **The map's meshes**: every `Mesh3d` that is one of the map's pieces or inside one — what a person means by
/// "the room". The Map's own tool tiles (the removal and marquee outlines, the bounds floor) are the Map's tools
/// rather than the map, and are swept and spawned again with the rest of the Map's furniture.
fn map_meshes(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    let mut pieces = world.query_filtered::<Entity, With<Placement>>();
    let pieces: HashSet<Entity> = pieces.iter(world).collect();
    let mut links = world.query::<(Entity, &ChildOf)>();
    let parent: HashMap<Entity, Entity> = links.iter(world).map(|(e, p)| (e, p.parent())).collect();
    let in_a_piece = |mut at: Entity| -> bool {
        loop {
            if pieces.contains(&at) {
                return true;
            }
            match parent.get(&at) {
                Some(up) => at = *up,
                None => return false,
            }
        }
    };
    let mut meshes = world.query_filtered::<Entity, With<Mesh3d>>();
    meshes.iter(world).filter(|e| in_a_piece(*e)).collect()
}

/// **The room as it stands**: the map's meshes, which must exist for a test about keeping them to measure anything.
fn the_room(app: &mut App) -> Vec<Entity> {
    let meshes = map_meshes(app);
    assert!(!meshes.is_empty(), "the fixture's pieces drew no mesh, so there is no room to measure");
    meshes
}

/// **The placement ids that stand as entities**, sorted: one per row of the map when the map is drawn once.
fn standing_ids(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let mut q = world.query::<&Placement>();
    let mut ids: Vec<String> = q.iter(world).map(|p| p.0.clone()).collect();
    ids.sort();
    ids
}

/// The placement ids the open map holds, sorted.
fn map_ids(app: &App) -> Vec<String> {
    let mut ids: Vec<String> = app.world().resource::<OpenMap>().map.placements.iter().map(|p| p.id.clone()).collect();
    ids.sort();
    ids
}

fn cameras(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<MainCamera>>();
    q.iter(world).collect()
}

/// **The map's meshes survive Map → World → Kit → Animation → Map**, which is what the three places say a switch
/// between the four does.
///
/// Fails on the tree before #695: `cross` went through `rebuild_the_door`, whose same-state
/// `NextState(Screen::Editor)` runs `close_the_door` and so `despawn_scene`, so every one of the map's meshes was
/// gone by the time the next workspace was drawn.
#[test]
fn the_maps_meshes_survive_a_round_trip_through_the_workspaces_that_share_its_viewport() {
    let mut app = editor("meshes");
    let before = the_room(&mut app);

    for workspace in [Workspace::World, Workspace::Kit, Workspace::Animation, Workspace::Map] {
        chord_to(&mut app, workspace);
        let now = map_meshes(&mut app);
        let kept = before.iter().filter(|e| now.contains(e)).count();
        assert_eq!(
            kept,
            before.len(),
            "the {} workspace has {kept} of the map's {} meshes: a switch despawned the scene, where \
             `docs/ui/vision.md` §4 says Map, World, Kit and Animation share one viewport",
            workspace.title(),
            before.len()
        );
        assert_eq!(app.world().resource::<OpenMap>().map.name, "m", "the switch to {} reloaded the map", workspace.title());
    }
}

/// **A kept scene is drawn once**: after every switch there is one entity per placement and one main camera.
///
/// Fails on the first cut of #695 (`38f385c8`): it kept the scene by skipping the sweep, and `OnEnter(Editor)` drew
/// the map and the camera again beside what stood — every placement id twice on the way back to Map, and a second
/// `MainCamera` that made every `single()` on it fail (`the_editor_follows_an_agents_change`).
#[test]
fn a_switch_draws_the_scene_once() {
    let mut app = editor("once");
    the_room(&mut app);
    let rows = map_ids(&app);
    assert_eq!(rows.len(), 2, "the fixture places two pieces");

    for workspace in [Workspace::World, Workspace::Map, Workspace::Kit, Workspace::Animation, Workspace::Map] {
        chord_to(&mut app, workspace);
        assert_eq!(standing_ids(&mut app), rows, "after the switch to {}, each placement stands once", workspace.title());
        assert_eq!(cameras(&mut app).len(), 1, "after the switch to {}, the editor has one main camera", workspace.title());
    }
}

/// **A change made while the Map was away is drawn when it is back**: the kept pieces are reconciled with the map,
/// not trusted. A row taken out is taken off the screen, and a row moved is drawn where it now is.
///
/// The map's own redraws run only while the Map is open, so a change from elsewhere (an agent's, an edit on disk)
/// reaches the screen through `editor::spawn_existing` on the way back — which must neither keep a piece whose row
/// is gone nor leave a moved one where it was.
#[test]
fn a_change_made_off_the_map_is_drawn_when_the_map_is_back() {
    let mut app = editor("reconcile");
    the_room(&mut app);
    let floor_at = |app: &mut App| -> Vec3 {
        let world = app.world_mut();
        let mut q = world.query::<(&Placement, &Transform)>();
        q.iter(world)
            .find(|(p, _)| p.0 == "floor@0")
            .map(|(_, t)| t.translation)
            .unwrap_or_else(|| panic!("`floor@0` stands"))
    };
    let was = floor_at(&mut app);

    chord_to(&mut app, Workspace::World);
    {
        let mut open = app.world_mut().resource_mut::<OpenMap>();
        open.map.placements.retain(|p| p.id != "wall@0");
        let floor = open.map.placements.iter_mut().find(|p| p.id == "floor@0").unwrap_or_else(|| panic!("the floor row"));
        floor.at.0 += 4.0;
    }
    // **Saved first**: a switch with unsaved edits asks "Leave this map?" (`editor::apply_door_request`), and this
    // test is about what is drawn on the way back, not about that question.
    bevy_dark_factory::app_verbs::save_the_open_map(app.world_mut()).unwrap_or_else(|e| panic!("{e}"));
    app.update();

    chord_to(&mut app, Workspace::Map);
    assert_eq!(standing_ids(&mut app), ["floor@0"], "the row taken out while the Map was away is off the screen, and the other stands once");
    let now = floor_at(&mut app);
    assert!(
        (now.distance(was) - 4.0).abs() < 1e-3,
        "the moved row is drawn where it now is, 4 m on: it was at {was:?} and stands at {now:?}"
    );
}

/// **A switch to a workspace that does not show the map takes the scene with it**, as every door change did: no
/// scene stands behind a page that draws none, and the way back draws the map again, once.
#[test]
fn a_switch_to_a_workspace_that_does_not_show_the_map_takes_the_scene_with_it() {
    let mut app = editor("agent");
    the_room(&mut app);
    let camera = cameras(&mut app);
    let rows = map_ids(&app);

    chord_to(&mut app, Workspace::Agent);
    assert!(standing_ids(&mut app).is_empty(), "the Agent workspace draws no map, so no piece stands behind it");
    let there = cameras(&mut app);
    assert_eq!(there.len(), 1, "the editor has one main camera on the Agent workspace");
    assert_ne!(there, camera, "the Map's camera went with the scene and a new one was spawned");

    chord_to(&mut app, Workspace::Map);
    assert_eq!(standing_ids(&mut app), rows, "back on the Map, each placement stands once");
    assert_eq!(cameras(&mut app).len(), 1, "and there is one main camera");
}

/// **A switch builds the panels the new workspace shows, and takes down the ones it does not.**
///
/// Regression (2026-11-27, `integrate/1127`): the first version of #695 returned before the state transition, so
/// `OnEnter(Editor)` — the one place a panel is spawned — never ran, and World's WATER heading was never drawn.
/// Keeping the scene must not mean keeping the old panels.
///
/// Every destination is named by a marker its own module exports, so this is the panel itself rather than a count
/// of nodes that could be anything.
#[test]
fn a_switch_builds_the_panels_the_new_workspace_shows() {
    use bevy_dark_factory::tiles::{AnimRoot, MapRoot};
    use bevy_dark_factory::workspace_panels::WorldRoot;

    fn count<M: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut q = world.query_filtered::<Entity, With<M>>();
        q.iter(world).count()
    }

    let mut app = editor("panels");
    assert!(count::<MapRoot>(&mut app) > 0, "the Map workspace draws its panel");
    assert_eq!(count::<WorldRoot>(&mut app), 0, "and Map does not draw World's");

    chord_to(&mut app, Workspace::World);
    assert!(count::<WorldRoot>(&mut app) > 0, "the World workspace's panel is built on arrival");

    chord_to(&mut app, Workspace::Animation);
    assert!(count::<AnimRoot>(&mut app) > 0, "the Animation workspace's bench is built on arrival");
    assert_eq!(count::<WorldRoot>(&mut app), 0, "and World's panel is taken down, or two would stand in one dock");

    chord_to(&mut app, Workspace::Map);
    assert!(count::<MapRoot>(&mut app) > 0, "and Map's is built again on the way back");
}

/// **The frame profile names where a switch spends its frame** (#678, #541). The three instrumented systems are not
/// where a switch goes, so the profile's answer used to be "no system named". Every frame now keeps one share per main
/// schedule, and a switch's frame names `StateTransition` — where `OnExit(Editor)` sweeps the frame and `OnEnter(Editor)`
/// builds it again — with the switch's time in it. `--no-capture` prints the split.
#[test]
fn a_switch_frame_names_the_schedule_it_spent_its_time_in() {
    use bevy_dark_factory::profile::{Frame, Profiler};

    fn transition_ms(frame: &Frame) -> f64 {
        frame.schedules.iter().filter(|s| s.name() == "StateTransition").map(|s| s.ms).sum()
    }

    let mut app = editor("profile");
    let settled = app.world().resource::<Profiler>().last().cloned().expect("a settled frame is in the ring");
    chord_to(&mut app, Workspace::Kit);
    let profiler = app.world().resource::<Profiler>();
    let switch = profiler
        .frames()
        .filter(|frame| frame.number > settled.number)
        .max_by(|a, b| transition_ms(a).total_cmp(&transition_ms(b)))
        .cloned()
        .expect("the switch's frames are in the ring");
    let split: Vec<String> = switch.schedules.iter().map(|s| format!("{} {:.3} ms", s.name(), s.ms)).collect();
    println!("switch frame {}: busy {:.3} ms: {}", switch.number, switch.busy_ms, split.join(", "));
    let settled_split: Vec<String> = settled.schedules.iter().map(|s| format!("{} {:.3} ms", s.name(), s.ms)).collect();
    println!("settled frame {}: busy {:.3} ms: {}", settled.number, settled.busy_ms, settled_split.join(", "));
    let shares: f64 = switch.schedules.iter().map(|s| s.ms).sum();
    assert!((shares - switch.busy_ms).abs() < 1e-6, "the shares are the busy time, split: {shares} against {switch:?}");
    assert!(
        transition_ms(&switch) >= 1.0 && transition_ms(&switch) > 10.0 * transition_ms(&settled),
        "the switch's frame names StateTransition with the switch's time in it ({:.3} ms; a settled frame's is {:.3} ms): {split:?}",
        transition_ms(&switch),
        transition_ms(&settled)
    );
}
