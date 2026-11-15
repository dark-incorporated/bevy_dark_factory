//! **With "Answer hand-backs here" on, the Agent workspace stays open when the agent edits the map, and the hand-back is answered in it** (#458).
//!
//! The setting was unreachable exactly when it mattered: an agent's `placement_add` made the follow carry the person to the Map, which closed the AGENT block, so
//! the `grimoire/present` that came next fell back to the modal Keep/Undo. (#439 stopped only the toast for a change made while the Agent workspace is open; the
//! view still moved.) Now, with the setting on and the Agent workspace open:
//!
//! - **the workspace is still the Agent workspace**, the card is in the AGENT block, and Accept and Undo there answer it, with no modal over the screen;
//! - **the view does not move**: the follow waits, held, for the person to go back to the Map, and is carried out then;
//! - **a press in the Agent workspace is not the person working the Map's view**, so answering the card does not drop the follow that waits.
//!
//! With the setting off, `a_change_made_while_the_agent_workspace_is_open_raises_no_toast` still holds: the follow brings the Map back.

use std::time::Duration;

use bevy::input::mouse::MouseButton;
use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;

use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::confirm::Confirm;
use bevy_cauldron::editor::{DoorRequest, EditorState};
use bevy_cauldron::familiar::{Familiar, HandBackAnswer, HandBackCard, HandBackHere};
use bevy_cauldron::follow::Following;
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::present::Reply;
use bevy_cauldron::project::OpenMap;
use bevy_cauldron::view::Rig;
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::Activate;

use crate::fixtures;

/// One frame of the test clock, seconds: 60 Hz, as the follow's own tests step it.
const FRAME: f32 = 1.0 / 60.0;

/// Seconds the editor runs after a step: far past the follow's ease and dwell.
const FOLLOW_SECONDS: f32 = 6.0;

/// Where the agent places the piece, in map metres, far from where the camera is pointed.
const AT: [f32; 2] = [-10.5, -10.5];

/// How near the camera's focus must end to the placement, in metres: the view is framed on it.
const FRAMED_WITHIN_M: f32 = 4.0;

fn frames_of(seconds: f32) -> usize {
    (seconds / FRAME).ceil() as usize + 2
}

fn updates(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .side(32.0)
        .descriptor("wall", "alpha")
        .place_as("wall@0", "wall", (1.5, 1.5))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    app.insert_resource(bevy_gooey::MotionSettings::default());
    updates(&mut app, 8);
    app
}

fn switch_to(app: &mut App, workspace: Workspace) {
    let id = format!("workspace.{}", workspace.id());
    let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    updates(app, 12);
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "`{id}` did not open the {} workspace", workspace.title());
}

/// **A door crossed as the person crosses it back to the Map**, without a key press: a chord is a hand on the keys, which is a different rule (#358).
fn go_back_to_the_map(app: &mut App) {
    app.world_mut().resource_mut::<EditorState>().door_request = Some(DoorRequest { workspace: Workspace::Map, tab: None });
    updates(app, 12);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Map, "the Map did not open");
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("`{method}` is not registered, so an agent cannot reach it");
    };
    let answer = app.world_mut().run_system_with(id, Some(params)).unwrap_or_else(|e| panic!("{method}: {e:?}")).unwrap_or_else(|e| panic!("{method}: {e:?}"));
    updates(app, 4);
    answer
}

fn problems(app: &App) -> Vec<String> {
    app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect()
}

fn cards(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<HandBackCard>>().iter(world).count()
}

fn blocks(app: &mut App) -> usize {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<Familiar>>().iter(world).count()
}

fn answer_of(app: &mut App) -> serde_json::Value {
    call(app, "grimoire/state", serde_json::json!({}))["presentation"]["answer"].clone()
}

fn press(app: &mut App, reply: Reply) {
    let button = {
        let world = app.world_mut();
        world.query::<(Entity, &HandBackAnswer)>().iter(world).find(|(_, answer)| answer.0 == reply).map(|(entity, _)| entity).expect("the card has that button")
    };
    app.world_mut().commands().trigger(Activate { entity: button });
    updates(app, 6);
}

/// Where the agent's piece stands, in map metres, on the open map.
fn target(app: &App) -> Vec3 {
    let origin = app.world().resource::<OpenMap>().map.origin;
    Vec3::new(origin.0 + AT[0], origin.1, origin.2 + AT[1])
}

/// **The setting on, the Agent workspace open, the camera pointed away**: the state in which the agent then edits the map and hands it back.
fn watching_the_agent(tag: &str) -> (App, Vec3) {
    let mut app = editor(tag);
    switch_to(&mut app, Workspace::Agent);
    app.world_mut().resource_mut::<HandBackHere>().0 = true;
    let origin = app.world().resource::<OpenMap>().map.origin;
    let away = Vec3::new(origin.0 + 12.0, origin.1, origin.2 + 12.0);
    app.world_mut().resource_mut::<Rig>().focus = away;
    updates(&mut app, 3);
    (app, away)
}

/// The agent places a piece and hands the turn back.
fn the_agent_places_and_hands_back(app: &mut App) {
    let done = call(app, "grimoire/operation_apply", serde_json::json!({"operation": "placement_add", "parameters": {"piece": "wall", "at": AT}, "dry_run": false}));
    assert_ne!(done["outcome"], "previewed", "the placement is applied, not previewed: {done}");
    // **The hand-back asks to look at the piece the agent made**, so its own follow is held with the change's.
    let placed = app.world().resource::<OpenMap>().map.placements.last().map(|p| p.id.clone()).unwrap_or_else(|| panic!("the map holds no placement"));
    call(
        app,
        "grimoire/present",
        serde_json::json!({
            "title": "the new wall",
            "summary": "Placed a wall in the south-west corner.",
            "specified": ["a wall in the corner"],
            "check": [{"what": "the wall stands in the corner", "placements": [placed]}],
            "operation_ids": [done["operation_id"]],
        }),
    );
    updates(app, frames_of(FOLLOW_SECONDS));
}

/// **The acceptance case**: the Agent workspace is open, the setting is on, the agent edits and hands back. The workspace is still the Agent workspace, the card is in the
/// AGENT block, no modal covers it, and Accept there answers it.
#[test]
fn a_hand_back_after_an_agents_change_is_answered_in_the_agent_workspace_with_no_modal() {
    let (mut app, away) = watching_the_agent("handback_stays_accept");
    the_agent_places_and_hands_back(&mut app);

    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Agent, "the agent's change moved the person off the Agent workspace");
    assert_eq!(blocks(&mut app), 1, "the AGENT block is still there, once");
    assert_eq!(cards(&mut app), 1, "the hand-back card is in the AGENT block");
    assert!(!app.world().resource::<Confirm>().is_open(), "no modal covers the screen: the question is asked in the block");
    assert_eq!(app.world().resource::<Rig>().focus, away, "the view did not move for a change made while the Agent workspace is open");

    press(&mut app, Reply::Accept);
    assert_eq!(answer_of(&mut app), "kept", "Accept in the block answers the hand-back");
    assert_eq!(cards(&mut app), 0, "and the card is gone with the answer");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Agent, "answering does not move the person either");
}

/// **Undo in the block takes the agent's change back**, still on the Agent workspace.
#[test]
fn undo_on_the_card_takes_the_agents_change_back_without_leaving_the_agent_workspace() {
    let (mut app, _) = watching_the_agent("handback_stays_undo");
    let placed_before = app.world().resource::<OpenMap>().map.placements.len();
    the_agent_places_and_hands_back(&mut app);
    assert_eq!(app.world().resource::<OpenMap>().map.placements.len(), placed_before + 1, "the agent's piece is placed");
    assert_eq!(cards(&mut app), 1, "the card is in the AGENT block");

    press(&mut app, Reply::Undo);
    assert_eq!(answer_of(&mut app), "undone", "Undo in the block answers the hand-back");
    assert_eq!(app.world().resource::<OpenMap>().map.placements.len(), placed_before, "and takes the piece back");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Agent, "on the Agent workspace");
    assert!(!app.world().resource::<Confirm>().is_open(), "with no modal");
}

/// **The follow waits for the person to return to the Map, and is carried out then**, though the person pressed in the Agent workspace to answer the card: that press is not
/// the person working the Map's view, so it does not drop what the follow holds.
#[test]
fn the_held_follow_is_carried_out_when_the_person_returns_to_the_map_after_answering_the_card() {
    let (mut app, away) = watching_the_agent("handback_stays_follow");
    let before = problems(&app);
    the_agent_places_and_hands_back(&mut app);
    assert!(app.world().resource::<Following>().pending() > 0, "the follow waits, held, rather than being dropped or carried out");
    assert_eq!(app.world().resource::<Rig>().focus, away, "and the view has not moved");

    // The person presses on the card: a mouse press in the Agent workspace.
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    updates(&mut app, 3);
    press(&mut app, Reply::Accept);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left);
    updates(&mut app, 3);
    assert!(app.world().resource::<Following>().pending() > 0, "a press on the card dropped the follow that waits for the Map");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Agent);

    go_back_to_the_map(&mut app);
    updates(&mut app, frames_of(FOLLOW_SECONDS));
    let focus = app.world().resource::<Rig>().focus;
    let at = target(&app);
    assert!(focus.distance(at) <= FRAMED_WITHIN_M, "back on the Map the held change was not followed: focus {focus:?}, the change is at {at:?}");
    let said: Vec<String> = problems(&app).into_iter().filter(|p| !before.contains(p)).collect();
    assert!(said.is_empty(), "the held follow raised a problem: {said:?}");
}
