//! **A follow held for a viewport survives the person leaving the Map door, and a held request does not spend a pending Look** (#449, from the #439 review).
//!
//! #439 holds a change that is due while the Map camera has no viewport (`Following::deferred`) and drops it, with the one sentence "could not be framed", after
//! `DEFERRED_FRAMES_MAX` frames. Two defects were left:
//!
//! 1. **The count ran off the Map door.** The Map camera has no viewport while another door is open, so a person who left the Map with a request held got the
//!    sentence ten seconds later though nothing had gone wrong. The count now runs only while the Map is the active door, and the request is framed when the Map
//!    is shown again.
//! 2. **A held request spent a pending Look.** The queue reads empty while a request is held, and an empty queue ends a Look ("a Look with nothing to look at is
//!    spent"), so the person's Look was gone by the time the viewport came and the change waited for the toggle.
//!
//! The Map camera's viewport is taken away by a system in `Last` that this file owns (`NoViewport`): the editor writes a viewport every frame it has a slot, so the
//! absence has to be re-made after it, and lifting the flag is what "the viewport is there again" means.

use std::time::Duration;

use bevy::camera::Viewport;
use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;

use bevy_dark_factory::editor::{DoorRequest, EditorState};
use bevy_dark_factory::follow::{DEFERRED_FRAMES_MAX, Following};
use bevy_dark_factory::harness;
use bevy_dark_factory::project::Project;
use bevy_dark_factory::settings::EditorSettings;
use bevy_dark_factory::view::{MainCamera, Rig};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;
use crate::settle_support::Frames;

/// One frame of the test clock, seconds: 60 Hz, as the follow's own tests step it.
const FRAME: f32 = 1.0 / 60.0;

/// Frames the person spends on another door: more than the follow may wait for a viewport.
const AWAY_FRAMES: usize = DEFERRED_FRAMES_MAX as usize + 100;

/// Where the agent places the piece, in map metres, far from where the camera is pointed.
const AT: [f32; 2] = [-10.5, -10.5];

/// How near the camera's focus must end to the placement, in metres: the view is framed on it.
const FRAMED_WITHIN_M: f32 = 4.0;

/// The pause between two frames of [`until_the_editor_has`].
const SETTINGS_PAUSE: Duration = Duration::from_millis(15);

/// **Frames [`until_the_editor_has`] steps before it stops looking**: at [`SETTINGS_PAUSE`] apart they cover the 10 s it used to wait.
const SETTINGS_SEEN_WITHIN: Frames = Frames::covering(Duration::from_secs(10), SETTINGS_PAUSE);

/// While `true`, the Map camera is given a viewport with no area at the end of every frame: the state of a camera that is not ready.
#[derive(Resource, Default)]
struct NoViewport(bool);

fn take_the_viewport_away(flag: Res<NoViewport>, mut camera: Query<&mut Camera, With<MainCamera>>) {
    if flag.0
        && let Ok(mut camera) = camera.single_mut()
    {
        camera.viewport = Some(Viewport { physical_size: UVec2::ZERO, ..default() });
    }
}

fn updates(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// **Step the editor until it has taken a settings edit made by hand** (#501 item 18: the settings files are followed by an
/// index, not polled, so an edit is seen after the index's sweep and debounce, which a fixed few frames do not wait for). The
/// wait is [`SETTINGS_SEEN_WITHIN`] frames, not a wall-clock window, so a starved host does not cut it short (#644). The
/// assertions after it are unchanged.
fn until_the_editor_has(app: &mut App, done: impl Fn(&EditorSettings) -> bool) {
    for _ in 0..SETTINGS_SEEN_WITHIN.0 {
        if done(app.world().resource::<EditorSettings>()) {
            break;
        }
        app.update();
        std::thread::sleep(SETTINGS_PAUSE);
    }
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).side(32.0).descriptor("wall", "alpha").place("wall", (1.5, 1.5)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    app.insert_resource(bevy_gooey::MotionSettings::default());
    app.init_resource::<NoViewport>();
    app.add_systems(Last, take_the_viewport_away);
    updates(&mut app, 8);
    app
}

/// **A door crossed as an agent crosses it** (`door_open`), not by a chord: a chord is the person's own hand on the keys, and a person's key press takes the camera back and
/// drops what the follow holds (#358), which is a different, correct rule.
fn switch_to(app: &mut App, workspace: Workspace) {
    app.world_mut().resource_mut::<EditorState>().door_request = Some(DoorRequest { workspace, tab: None });
    updates(app, 12);
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "the {} workspace did not open", workspace.title());
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

fn problems(app: &App) -> Vec<String> {
    app.world().resource::<EditorState>().status.problems().iter().map(|p| p.text.clone()).collect()
}

fn no_viewport(app: &mut App, on: bool) {
    app.world_mut().resource_mut::<NoViewport>().0 = on;
}

/// The camera pointed away from the change, so "it was framed" cannot pass on a camera that never moved; returns where the change will be.
fn point_away(app: &mut App) -> Vec3 {
    let origin = app.world().resource::<bevy_dark_factory::project::OpenMap>().map.origin;
    app.world_mut().resource_mut::<Rig>().focus = Vec3::new(origin.0 + 12.0, origin.1, origin.2 + 12.0);
    updates(app, 3);
    Vec3::new(origin.0 + AT[0], origin.1, origin.2 + AT[1])
}

fn near(app: &App, target: Vec3) -> bool {
    app.world().resource::<Rig>().focus.distance(target) <= FRAMED_WITHIN_M
}

/// **Leave the Map door for 700 frames with a request held, come back: it is framed, and no problem is raised.**
#[test]
fn a_request_held_for_a_viewport_is_framed_when_the_map_is_shown_again_however_long_the_person_was_away() {
    let mut app = editor("held_away");
    let target = point_away(&mut app);
    let before = problems(&app);

    // The Map camera is not ready: the change is held, not framed.
    no_viewport(&mut app, true);
    updates(&mut app, 3);
    let answer = apply(&mut app, "placement_add", serde_json::json!({"piece": "wall", "at": AT}));
    assert_ne!(answer["outcome"], "previewed", "the placement is applied, not previewed: {answer}");
    updates(&mut app, 10);
    assert!(!near(&app, target), "the view cannot have followed a change the camera had no viewport for");

    // The person leaves the Map door, and stays away longer than the follow waits for a viewport.
    switch_to(&mut app, Workspace::Agent);
    updates(&mut app, AWAY_FRAMES);
    let said: Vec<String> = problems(&app).into_iter().filter(|p| !before.contains(p)).collect();
    assert!(said.iter().all(|p| !p.contains("could not be framed")), "the follow gave up on a request while the person was on another door: {said:?}");

    // They come back; the Map has its viewport again.
    no_viewport(&mut app, false);
    switch_to(&mut app, Workspace::Map);
    updates(&mut app, 400);
    let said: Vec<String> = problems(&app).into_iter().filter(|p| !before.contains(p)).collect();
    assert!(said.iter().all(|p| !p.contains("could not be framed")), "a problem was raised for a change that could be framed: {said:?}");
    let focus = app.world().resource::<Rig>().focus;
    assert!(near(&app, target), "the held change was not framed when the Map came back: focus {focus:?}, change at {target:?}");
}

/// **A pending Look survives a request being held**: with the follow switched off, the Look is what shows the change, and it is still pending when the viewport comes.
#[test]
fn a_pending_look_survives_a_held_request() {
    let mut app = editor("held_look");
    let target = point_away(&mut app);

    // The change is due while the camera has no viewport, so it is held.
    no_viewport(&mut app, true);
    updates(&mut app, 3);
    apply(&mut app, "placement_add", serde_json::json!({"piece": "wall", "at": AT}));
    updates(&mut app, 10);
    assert!(!near(&app, target), "the view cannot have followed a change the camera had no viewport for");

    // The person turns the follow off (the setting, written where the editor reads it) and asks to look. The request is held, so the queue reads empty; the Look is still theirs.
    let settings = app.world().resource::<Project>().project_dir.join("editor.yaml");
    std::fs::write(&settings, "{version: 1, follow_agent: false}").expect("editor.yaml");
    until_the_editor_has(&mut app, |s| !s.follow_agent);
    updates(&mut app, 120);
    assert!(!app.world().resource::<EditorSettings>().follow_agent, "the setting did not reach the running editor, so the test would prove nothing about it");
    app.world_mut().resource_mut::<Following>().look_now();
    updates(&mut app, 10);

    // The viewport comes: the Look shows the change although the toggle is off.
    no_viewport(&mut app, false);
    updates(&mut app, 400);
    let focus = app.world().resource::<Rig>().focus;
    assert!(near(&app, target), "the pending Look was spent while the request was held: focus {focus:?}, change at {target:?}");
}
