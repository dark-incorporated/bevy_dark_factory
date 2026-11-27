//! **`blueprint/state` carries the editor's own frame clock (E-011).**
//!
//! Frame time read by counting ticks from outside swung 20 to 67 ms with machine load, so an agent
//! could not tell a slow editor from a busy machine. The state now says `frame.frame_ms` (wall time
//! between frames) and `frame.busy_ms` (work done inside one frame).

use bevy_dark_factory::harness;

use crate::fixtures;

fn call(app: &mut bevy::prelude::App, method: &str) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("no method `{method}` is registered"),
    };
    app.world_mut()
        .run_system_with(id, Some(serde_json::json!({})))
        .expect("system runs")
        .expect("method answers")
}

#[test]
fn the_state_reports_frame_time_and_busy_time_once_frames_have_run() {
    let root = fixtures::Fixture::new("frame-clock")
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    for _ in 0..5 {
        std::thread::sleep(std::time::Duration::from_millis(3));
        app.update();
    }
    let state = call(&mut app, "blueprint/state");
    let frame = &state["frame"];
    assert!(frame["frame_ms"].as_f64().is_some_and(|m| m > 0.0), "{frame}");
    assert!(frame["busy_ms"].as_f64().is_some_and(|m| m > 0.0), "{frame}");
    assert!(frame["frame_count"].as_f64().is_some_and(|c| c >= 3.0), "{frame}");
}

/// **#677: `frame_ms` and `busy_ms` are two clocks, so a wait between frames is in one and not the other.**
///
/// Until #677 both were one frame's head to its foot — `frame_ms` the profile's ring, `busy_ms` a second clock with the
/// same `First` to `Last` bounds — so they agreed to 0.05 ms on 46 of 50 frames by construction, and "is a slow frame
/// the editor or the machine" had no answer. The wait here is a sleep between two `app.update()`s, which is where a
/// running editor's present and event-loop wait fall. Fails on the tree before the fix: `frame_ms - busy_ms` was ~0.
#[test]
fn a_wait_between_frames_is_in_frame_ms_and_not_in_busy_ms() {
    const WAIT_MS: u64 = 40;
    let root = fixtures::Fixture::new(&format!("frame-split-{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    for _ in 0..5 {
        app.update();
    }
    std::thread::sleep(std::time::Duration::from_millis(WAIT_MS));
    // **This frame's head makes the one before it whole**, and that one's wall time now holds the sleep.
    app.update();
    let state = call(&mut app, "blueprint/state");
    let frame = &state["frame"];
    let frame_ms = frame["frame_ms"].as_f64().unwrap_or_else(|| panic!("no frame_ms: {frame}"));
    let busy_ms = frame["busy_ms"].as_f64().unwrap_or_else(|| panic!("no busy_ms: {frame}"));
    assert!(
        frame_ms - busy_ms >= WAIT_MS as f64,
        "the {WAIT_MS} ms between two frames is in frame_ms ({frame_ms:.3}) and not in busy_ms ({busy_ms:.3}): {frame}"
    );
    // **And the busy time is accounted for, schedule by schedule** (#678): the shares are one clock with it.
    let shares = frame["schedules"].as_array().unwrap_or_else(|| panic!("no schedules: {frame}"));
    let names: Vec<&str> = shares.iter().filter_map(|s| s["schedule"].as_str()).collect();
    for main in ["First", "PreUpdate", "StateTransition", "Update", "PostUpdate", "Last"] {
        assert!(names.contains(&main), "the split names {main}: {names:?}");
    }
    let total: f64 = shares.iter().filter_map(|s| s["ms"].as_f64()).sum();
    assert!((total - busy_ms).abs() < 0.001, "the shares add up to busy_ms: {total:.4} against {busy_ms:.4}: {frame}");
}
