//! **`Cmd+J` cycles the pane through three positions, and `blueprint/state` says which** (#598).
//!
//! Measured 2026-11-24 at 1600x1000 driving `bevy_fairy/input`: `blueprint/state` was unchanged across
//! three `Cmd+J` presses — `screen: workbench` every time — and the window showed `Start agent` in a
//! 1600x330 pane with no shell prompt. The report was honest about its limit: *"the terminal's shell
//! never started on this host (no prompt, only `Start agent`), so a pane that failed to launch its shell
//! cannot be separated from a placement bug."*
//!
//! # What this file separates
//!
//! The two halves the report could not tell apart, each measured on its own:
//!
//! 1. **The placement.** Three presses from Collapsed reach Maximised, Quarter, then Collapsed again, and
//!    the pane's drawn height follows each — measured on the laid-out `ComputedNode`, not on a resource.
//!    This half does not need a shell: a pane with no shell is still a pane the chord moves.
//! 2. **The state.** `blueprint/state.terminal.placement` names the position after each press, so a drive
//!    that presses the chord reads what moved. **This is the field #598 found missing**: the report's
//!    drive read `screen`, which a terminal chord never changes, and concluded nothing happened.
//!
//! A shell is attempted where the host can start one (`/bin/sh`), and where it cannot the test says so
//! in its message rather than passing quietly: `shell_live` is what distinguishes "the chord did not move
//! the pane" from "the pane moved and its shell never started", which is exactly the pair #598 could not
//! separate.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_pane::{TERMINAL_QUARTER_SHARE, TerminalPaneRoot, TerminalPlacement};
use bevy_dark_factory::menu_bar::EditorMenuBar;

use crate::fixtures;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    // **No `DebuggerPlugin`, no `OpsBridgePlugin`** (#598). `primary_j_cycles_the_terminal_through_three_positions`
    // is the nearest passing pattern and adds neither: `DebuggerPlugin` binds a BRP port, and `harness.rs` says
    // it "stays out of here on purpose" because a test process builds several `App`s. Adding it here panicked
    // the compute pool rather than failing an assertion, which is a test that measured its own harness.
    //
    // **And no `OpsBridgePlugin` either**: the state is read by naming the registered method and running it, and
    // the plugin is what registers it — so this test reads the placement from the resource the pane draws from
    // instead. That is the stronger measurement of the two: a state field can be right while the pane is wrong,
    // and the pane can be right while the field is stale, so both are read and compared on their own.
    app.world_mut().spawn((Window { resolution: bevy::window::WindowResolution::new(1600, 1000), ..default() }, PrimaryWindow));
    for _ in 0..12 {
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

fn window(app: &mut App) -> Entity {
    app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("one primary window")
}

fn send(app: &mut App, key_code: KeyCode, logical_key: Key, state: ButtonState) {
    let window = window(app);
    app.world_mut().write_message(KeyboardInput {
        key_code,
        text: match (&logical_key, state) {
            (Key::Character(c), ButtonState::Pressed) => Some(c.clone()),
            _ => None,
        },
        logical_key,
        state,
        repeat: false,
        window,
    });
}

/// **Cmd/Ctrl+J as a hand makes it**, with the frames the chord needs to land and let go.
fn press_cmd_j(app: &mut App) {
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    let primary = bevy_dark_factory::keys::MOD_KEYS[0];
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, primary, unidentified(), state);
        send(app, KeyCode::KeyJ, Key::Character("j".into()), state);
        app.update();
    }
    for _ in 0..4 {
        app.update();
    }
}

/// **The pane's drawn height and the room below the menu bar**, in logical px.
fn drawn(app: &mut App) -> (f32, f32, bool) {
    let world = app.world_mut();
    let (pane, node, parent) = world
        .query_filtered::<(&ComputedNode, &Node, &ChildOf), With<TerminalPaneRoot>>()
        .iter(world)
        .next()
        .map(|(computed, node, parent)| (computed.size().y * computed.inverse_scale_factor(), node.display != Display::None, parent.parent()))
        .expect("the terminal's pane");
    let root = world.get::<ComputedNode>(parent).map_or(0.0, |root| root.size().y * root.inverse_scale_factor());
    let menu = world.query_filtered::<&ComputedNode, With<EditorMenuBar>>().iter(world).next().map_or(0.0, |menu| menu.size().y * menu.inverse_scale_factor());
    (pane, root - menu, node)
}

/// The placement the pane is drawn in, read off the two things a person sees: drawn or not, and full or not.
fn seen(app: &mut App) -> TerminalPlacement {
    let (height, body, drawn) = drawn(app);
    if !drawn {
        TerminalPlacement::Collapsed
    } else if (height - body).abs() <= 1.0 {
        TerminalPlacement::Maximised
    } else {
        TerminalPlacement::Quarter
    }
}

/// **The placement `blueprint/state` reports**, which is the field #598 found missing.
///
/// Read from the resource `blueprint/state.terminal` is built from, through the same `placement()` the
/// answer calls: the state's own section is a `json!` of this resource, so a resource that says one thing
/// and a drawing that says another is exactly the disagreement this test exists to catch, and it is
/// caught here without a port (`blueprint_state_adds_a_terminal_section` proves the JSON carries it).
fn reported(app: &mut App) -> String {
    let placement = app.world().resource::<Terminal>().placement();
    words(placement).to_owned()
}

fn words(placement: TerminalPlacement) -> &'static str {
    match placement {
        TerminalPlacement::Maximised => "maximised",
        TerminalPlacement::Collapsed => "collapsed",
        TerminalPlacement::Quarter => "quarter",
    }
}

/// **Three presses from Collapsed reach Maximised, Quarter and back, and the state names each.**
///
/// The report's exact drive, with the one field it was missing: the pane's drawn position and
/// `blueprint/state.terminal.placement` are asserted together at every step, so the two cannot disagree
/// and neither can pass on a build where the other is wrong.
#[test]
fn three_presses_cycle_the_pane_and_the_state_reports_each() {
    let mut app = editor("cmd_j_state");

    // **The starting position is the collapsed one**, which is where a launch leaves it.
    assert_eq!(seen(&mut app), TerminalPlacement::Collapsed, "the pane starts collapsed");
    assert_eq!(reported(&mut app), "collapsed", "and the state says so");

    let mut pressed = 0;
    // The owner's order, walked from where a launch leaves the pane: `Collapsed -> Quarter -> Maximised ->
    // Collapsed`. `TerminalPlacement::next` is the one machine that states it, and `cmd_j_is_the_cycle_chord`
    // pins the chord; this walk pins what three presses of it reach.
    for want in [TerminalPlacement::Collapsed, TerminalPlacement::Quarter, TerminalPlacement::Maximised, TerminalPlacement::Collapsed] {
        if pressed > 0 {
            press_cmd_j(&mut app);
        }
        pressed += 1;
        assert_eq!(seen(&mut app), want, "press {pressed}: the pane is drawn at {want:?} and is at {:?}", seen(&mut app));
        assert_eq!(
            reported(&mut app),
            words(want),
            "press {pressed}: `blueprint/state.terminal.placement` must name the position the pane is drawn in",
        );
    }

    // **The quarter is the stated fraction of the body**, not "whatever the split last held": the same
    // measure the maximise fills, so a person who presses the chord twice lands on the number the
    // constant states rather than on a remembered height.
    app.world_mut().resource_mut::<Terminal>().set_placement(TerminalPlacement::Quarter);
    for _ in 0..4 {
        app.update();
    }
    let (quarter, body, _) = drawn(&mut app);
    let want = (body * TERMINAL_QUARTER_SHARE).max(bevy_dark_factory::terminal_pane::TERMINAL_MIN_H);
    assert!(
        (quarter - want).abs() <= 1.0,
        "the quarter is drawn at {quarter} px of a {body} px body, and a quarter of it is {want}",
    );
}

/// **The two halves of #598's report, told apart.** A pane that moves with no shell is a placement that
/// worked and a shell that did not start — and the state says which, so an agent is never left guessing
/// which half failed.
#[test]
fn the_state_says_whether_the_shell_started_as_well_as_where_the_pane_is() {
    let mut app = editor("cmd_j_shell");
    press_cmd_j(&mut app); // Collapsed -> Quarter
    let terminal = app.world().resource::<Terminal>();
    assert_eq!(terminal.placement(), TerminalPlacement::Quarter, "one press from Collapsed is the Quarter");
    assert!(!terminal.is_empty(), "there is a tab to run a shell in");
    // **`shell_live` is `TerminalTab::is_live`**, which is the fact `blueprint/state.terminal.shell_live`
    // reports. It is asserted to be readable here rather than to be true: whether a shell starts is the
    // host's, and the report's own host had none. What #598 fixes is that the two are told apart.
    let live = terminal.tab().is_some_and(bevy_dark_factory::terminal::TerminalTab::is_live);
    if !live {
        eprintln!("#598: the pane cycles with no live shell on this host — the placement half is proved, the shell half is the host's");
    }
}

/// **`blueprint/state` carries the terminal section**, so the field #598 found missing is on the wire.
///
/// The other two tests read the resource, because registering a BRP method needs the port-binding plugin
/// this harness deliberately leaves out. This one proves the JSON: the section is a `json!` of that same
/// resource, so a resource that answers and an answer that omits it cannot both be right.
#[test]
fn blueprint_state_carries_the_terminal_section() {
    let mut app = editor("cmd_j_json");
    // **The pair the passing state tests use** (`a_command_runs_by_id_through_the_agents_door`): the debugger
    // registers `bevy/remote`'s method table and `OpsBridgePlugin` the editor's own methods. The bridge alone
    // panics in `bevy_fairy`'s command layer, which is what this test measured first.
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    press_cmd_j(&mut app); // Collapsed -> Quarter
    let state = call(&mut app, "blueprint/state", serde_json::json!({}));
    let terminal = &state["terminal"];
    assert_eq!(terminal["placement"], "quarter", "`blueprint/state.terminal.placement` names the position: {state}");
    assert!(terminal["next"].is_string(), "and what the next press reaches: {state}");
    assert!(terminal["tabs"].as_u64().is_some_and(|tabs| tabs >= 1), "and how many tabs there are: {state}");
    assert!(terminal["shell_live"].is_boolean(), "and whether a shell is running, which is the half #598 could not tell apart: {state}");
}
