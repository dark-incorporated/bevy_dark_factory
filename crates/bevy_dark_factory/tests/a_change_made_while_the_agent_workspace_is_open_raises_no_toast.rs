//! **A placement made while the Agent workspace is open raises no toast** (#439). Found while recording: `placement_add` (an agent operation) with the Agent
//! workspace open raised "the Map camera has no viewport yet, so the Night Shift's change was not framed", an error-sounding toast for a normal, successful
//! change. The follow brings the Map back to look at the change; the Map camera has no viewport for the first frames of that, and the follow must wait for
//! one, silently, and then frame the change.
//!
//! - **no problem is raised**: neither the toast nor the journal hears about a viewport;
//! - **the view follows when the Map is shown**: the camera ends at the placement the agent made.

use std::time::Duration;

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;

use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::editor::EditorState;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::view::Rig;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// One frame of the test clock, seconds: 60 Hz, as the follow's own tests step it.
const FRAME: f32 = 1.0 / 60.0;

/// Frames the editor runs after a step, seconds of the test clock: far past the follow's ease and dwell.
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
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).side(32.0).descriptor("wall", "alpha").place("wall", (1.5, 1.5)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
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

fn apply(app: &mut App, operation: &str, parameters: serde_json::Value) -> serde_json::Value {
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("blueprint/operation_apply").cloned() else {
        panic!("no `blueprint/operation_apply` is registered, so an agent cannot reach it");
    };
    app.world_mut()
        .run_system_with(id, Some(serde_json::json!({"operation": operation, "parameters": parameters, "dry_run": false})))
        .unwrap_or_else(|e| panic!("{operation}: {e:?}"))
        .unwrap_or_else(|e| panic!("{operation}: {e:?}"))
}

/// Every sentence the status line and the journal have been given as a problem, oldest first.
fn problems(app: &App) -> Vec<String> {
    app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect()
}

/// **An operation applied while the Agent workspace is open says nothing about a viewport, and the view goes to the change once the Map is shown.**
#[test]
fn a_placement_made_while_the_agent_workspace_is_open_raises_no_toast_and_is_followed_when_the_map_shows() {
    let mut app = editor("agent_open_follow");
    switch_to(&mut app, Workspace::Agent);
    // The camera is pointed away from where the change will be, so "it followed" cannot pass on a camera that never moved.
    let origin = app.world().resource::<bevy_dark_factory::project::OpenMap>().map.origin;
    app.world_mut().resource_mut::<Rig>().focus = Vec3::new(origin.0 + 12.0, origin.1, origin.2 + 12.0);
    updates(&mut app, 3);
    let before = problems(&app);

    let answer = apply(&mut app, "placement_add", serde_json::json!({"piece": "wall", "at": AT}));
    assert_ne!(answer["outcome"], "previewed", "the placement is applied, not previewed: {answer}");
    updates(&mut app, frames_of(FOLLOW_SECONDS));

    let said: Vec<String> = problems(&app).into_iter().filter(|p| !before.contains(p)).collect();
    assert!(said.iter().all(|p| !p.contains("viewport")), "a normal, successful change raised a problem about the viewport: {said:?}");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Map, "the follow brought the Map back to show the change");
    let focus = app.world().resource::<Rig>().focus;
    let target = Vec3::new(origin.0 + AT[0], origin.1, origin.2 + AT[1]);
    assert!(
        focus.distance(target) <= FRAMED_WITHIN_M,
        "the view followed the change to {target:?}, but the camera's focus is {focus:?} ({:.1} m away)",
        focus.distance(target)
    );
}

/// Where the second change is made, in map metres: far from the first and from the camera.
const SECOND_AT: [f32; 2] = [9.5, 9.5];

/// **A second change made while the first still waits for a viewport does not replace it**: both are followed, in order, so the view ends at the second.
#[test]
fn two_changes_made_while_the_agent_workspace_is_open_are_both_held_and_the_view_ends_at_the_second() {
    let mut app = editor("agent_open_follow_two");
    switch_to(&mut app, Workspace::Agent);
    let origin = app.world().resource::<bevy_dark_factory::project::OpenMap>().map.origin;
    let before = problems(&app);
    for at in [AT, SECOND_AT] {
        let answer = apply(&mut app, "placement_add", serde_json::json!({"piece": "wall", "at": at}));
        assert_ne!(answer["outcome"], "previewed", "the placement is applied: {answer}");
    }
    updates(&mut app, frames_of(2.0 * FOLLOW_SECONDS));
    let said: Vec<String> = problems(&app).into_iter().filter(|p| !before.contains(p)).collect();
    assert!(said.iter().all(|p| !p.contains("viewport")), "no problem about the viewport: {said:?}");
    let focus = app.world().resource::<Rig>().focus;
    let target = Vec3::new(origin.0 + SECOND_AT[0], origin.1, origin.2 + SECOND_AT[1]);
    assert!(focus.distance(target) <= FRAMED_WITHIN_M, "the view ends at the second change {target:?}, focus {focus:?}");
}
