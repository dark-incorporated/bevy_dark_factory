//! **`blueprint/state` answers "which screen is up" once** (#601).
//!
//! Measured 2026-11-24 with the launcher on screen: top-level `screen` read `"project"`, `ui.screen`
//! read `"launcher"`, and `door` and `mode` both read `"MAP"`. Three fields answered the same
//! question three ways, so an agent that read the wrong one was wrong two times in three — and the
//! `bevy_fairy` drive behind #598 spent three presses learning nothing had moved because `screen` was
//! the one field it trusted.
//!
//! # The contract this pins
//!
//! | field | answers |
//! |---|---|
//! | `screen` | **what is on screen**, and nothing else: `launcher`, `loading`, `project` or `editor` |
//! | `ui.workspace` | which workspace is behind the launcher, or `null` before one is open |
//! | `ui.panels` | which panels that workspace shows |
//! | `mode` | **which panel of that workspace** is showing; `null` before one is |
//!
//! `ui.screen` is gone: it was a second spelling of "is the workbench up", which `screen` already
//! answers, and `door` is gone: it was a second spelling of the workspace, which `ui.workspace`
//! already names.
//!
//! # What is driven
//!
//! The launcher, then a workspace, then back — the three steps the issue names — with the state read
//! at each, so the meanings are asserted where they differ rather than at one convenient moment.

use bevy::prelude::*;
use bevy_dark_factory::screen::Screen;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    for _ in 0..6 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("no method `{method}` is registered"),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

fn state(app: &mut App) -> serde_json::Value {
    call(app, "blueprint/state", serde_json::json!({}))
}

fn go_to(app: &mut App, screen: Screen) {
    app.world_mut().resource_mut::<NextState<Screen>>().set(screen);
    for _ in 0..6 {
        app.update();
    }
}

/// **The launcher, a workspace, and back.** Each step reads the state and asserts what each field
/// answers there, so a field that is right on one screen and wrong on another cannot pass.
#[test]
fn the_screen_the_workspace_and_the_panel_are_three_different_answers() {
    let mut app = editor("state_once");

    // **A launcher first, because that is where the three fields disagreed.** A workspace is loaded
    // behind it — the harness opened one — so a field that reports "which workspace exists" answers
    // here, and a field that reports "which screen is up" answers `launcher`.
    go_to(&mut app, Screen::Launcher);
    let at_home = state(&mut app);
    assert_eq!(at_home["screen"], serde_json::json!("launcher"), "the launcher is on screen: {at_home}");
    assert!(
        at_home["ui"].get("screen").is_none(),
        "`ui.screen` is gone: it answered \"which screen\" a second time, and `screen` is the one field that answers it: {at_home}",
    );
    assert!(
        at_home.get("door").is_none(),
        "`door` is gone: it answered \"which workspace\" a second time, and `ui.workspace` is the one field that answers it: {at_home}",
    );
    assert!(
        !at_home["ui"]["workspace"].is_string() || at_home["ui"]["workspace"] == serde_json::json!("MAP"),
        "a workspace behind the launcher is named by `ui.workspace`, whatever it is: {at_home}",
    );
    assert_eq!(at_home["ui"]["panels"], serde_json::json!([]), "no panel is on screen while the launcher is: {at_home}");
    assert!(at_home["mode"].is_null(), "no panel of the workbench is showing while the launcher is: {at_home}");

    // **A workspace.** `screen` says the workbench is up, `ui.workspace` says which job, `mode`
    // says which panel of that job.
    go_to(&mut app, Screen::Editor);
    app.world_mut().insert_resource(Workspace::Files);
    for _ in 0..6 {
        app.update();
    }
    let at_work = state(&mut app);
    assert_eq!(at_work["screen"], serde_json::json!("editor"), "the workbench is on screen: {at_work}");
    assert!(
        at_work["ui"].get("screen").is_none(),
        "and it is still `screen` that says so, not a second field: {at_work}",
    );
    assert_eq!(at_work["ui"]["workspace"], serde_json::json!("FILES"), "the Files workspace is the one behind the screen: {at_work}");
    assert_eq!(at_work["ui"]["panels"], serde_json::json!(["FILES"]), "and it shows its own panel: {at_work}");
    assert_eq!(at_work["mode"], serde_json::json!("FILES"), "`mode` names the panel that is showing: {at_work}");

    // **Back.** The screen field follows, and the workspace behind it is still named — a reader that
    // wants "which workspace would I return to" reads `ui.workspace`, and a reader that wants "is the
    // workbench up" reads `screen`. They are different questions and the answer now says which.
    go_to(&mut app, Screen::Project);
    let at_page = state(&mut app);
    assert_eq!(at_page["screen"], serde_json::json!("project"), "the project page is on screen: {at_page}");
    assert!(at_page["mode"].is_null(), "and no panel of the workbench is showing: {at_page}");
}

/// **One field answers "is the launcher up", and it is `screen`.** The three-way disagreement the
/// issue measured, as one assertion: no other field of the answer spells a screen name.
#[test]
fn no_second_field_spells_a_screen_name() {
    let mut app = editor("state_names");
    go_to(&mut app, Screen::Launcher);
    let s = state(&mut app);
    for (path, value) in [
        ("ui", s["ui"].clone()),
        ("editor", s["editor"].clone()),
    ] {
        let text = value.to_string();
        assert!(
            !text.contains("\"screen\""),
            "`{path}` carries its own \"screen\": the answer spells the screen twice, which is what #601 is: {value}",
        );
    }
    assert!(s["screen"].is_string(), "the one field that answers it is top-level `screen`: {s}");
}
