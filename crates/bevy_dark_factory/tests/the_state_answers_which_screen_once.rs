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
    // **The home's own plugins**, because going home is a screen the editor draws (`LauncherPlugin`), and the
    // loader that takes a created project back to its map (`LoadingPlugin`) is its other half. This is the pair
    // `a_new_project_opens_something_to_edit::on_the_home` uses, and it is the difference between a launcher
    // this app can stand on and one whose screen has no systems while the editor's teardown is half done.
    app.add_plugins(bevy_dark_factory::launcher::LauncherPlugin);
    app.add_plugins(bevy_dark_factory::loading::LoadingPlugin);
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    // **Sealed last**: `finish()`/`cleanup()` is what registers the glTF loader a launcher's cards need, and
    // nothing may be added after it.
    app.finish();
    app.cleanup();
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

/// **Open a workspace the way the strip does**: its command id queued as a chord, which is the one path the
/// keys, the palette, the activity bar and an agent all take (`commands::by_id`).
///
/// **Not `insert_resource`**: the editor keeps `Workspace` and `Mode` as two axes and syncs the first from
/// the second (`Workspace::showing`), so a workspace inserted without its mode is put back by that system —
/// measured: `ui.workspace` read `"MAP"` after `insert_resource(Workspace::Files)`.
fn switch_to(app: &mut App, workspace: Workspace) {
    use bevy_dark_factory::command_run::Chords;
    use bevy_dark_factory::commands;
    use bevy_dark_factory::keys::Live;
    let id = format!("workspace.{}", workspace.id());
    let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    for _ in 0..24 {
        app.update();
    }
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "the `{id}` command did not open {}", workspace.title());
}

fn go_to(app: &mut App, screen: Screen) {
    app.world_mut().resource_mut::<NextState<Screen>>().set(screen);
    for _ in 0..6 {
        app.update();
    }
}

/// **The launcher, a workspace, and the project page.** Three screens, each read where it is up.
///
/// **One app per screen, and not one app walked between them.** `Screen::Editor`'s exit is a full teardown
/// that drops `Project`, `Mode` and every panel resource, and the editor's systems are gated on
/// `in_state(Screen::Editor)` — so a walk *out* of the editor and back is a transition the harness does not
/// support (measured: `Res<Project>` and `Res<Mode>` fail validation in ~18 systems on the way out). The
/// launcher's own tests stand on the home from a fresh app for the same reason
/// (`a_new_project_opens_something_to_edit::on_the_home`).
///
/// Each screen is asserted where it is genuinely up, which is what makes the three fields' meanings
/// distinguishable: they disagree on the launcher, they agree on nothing, and each says its own thing.
#[test]
fn the_screen_the_workspace_and_the_panel_are_three_different_answers() {
    // **The launcher.** A leave is a full teardown — `close_the_door` drops `Opened`, `Workspace` with it —
    // so no workspace is open and `ui.workspace` is null. That is the one path per fact: `screen` says
    // launcher, `ui.workspace` says none open, `mode` says nothing showing, and no field answers either of
    // the others' questions.
    let mut home = editor("state_launcher");
    go_to(&mut home, Screen::Launcher);
    let at_home = state(&mut home);
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
        at_home["ui"]["workspace"].is_null(),
        "no workspace is open on the launcher, and `ui.workspace` says that rather than naming the panel's job: {at_home}",
    );
    assert_eq!(at_home["ui"]["panels"], serde_json::json!([]), "no panel is on screen while the launcher is: {at_home}");
    assert!(at_home["mode"].is_null(), "and `mode` names no panel, because none is showing: {at_home}");

    // **A workspace.** `screen` says the workbench is up, `ui.workspace` says which job, `mode` says which
    // panel of that job.
    let mut work = editor("state_workbench");
    switch_to(&mut work, Workspace::Files);
    let at_work = state(&mut work);
    assert_eq!(at_work["screen"], serde_json::json!("editor"), "the workbench is on screen: {at_work}");
    assert!(at_work["ui"].get("screen").is_none(), "and `screen` is still the only field that says so: {at_work}");
    assert_eq!(at_work["ui"]["workspace"], serde_json::json!("FILES"), "the Files workspace is the one open: {at_work}");
    assert_eq!(at_work["ui"]["panels"], serde_json::json!(["FILES"]), "and it shows its own panel: {at_work}");
    assert_eq!(at_work["mode"], serde_json::json!("FILES"), "`mode` names the panel that is showing: {at_work}");

    // **The project page, from a fresh app stood on it by the harness.** Not a walk out of the editor: that
    // exit is the teardown the harness does not support (see this file's note), and `stand_on_the_project_page`
    // is the one way a test arrives on the page.
    let root = fixtures::Fixture::new("state_page").sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let mut page = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    page.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    page.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    bevy_dark_factory::harness::stand_on_the_project_page(&mut page, &root);
    let at_page = state(&mut page);
    assert_eq!(at_page["screen"], serde_json::json!("project"), "the project page is on screen: {at_page}");
    assert!(at_page["mode"].is_null(), "and no panel of the workbench is showing: {at_page}");
    assert!(
        at_page["ui"]["workspace"].is_null() || at_page["ui"]["workspace"] == serde_json::json!("FILES"),
        "`ui.workspace` names a workspace only while one is open, and never a screen: {at_page}",
    );
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
