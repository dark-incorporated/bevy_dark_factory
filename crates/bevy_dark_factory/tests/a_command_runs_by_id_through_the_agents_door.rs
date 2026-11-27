//! **A command runs by id for an agent exactly as its key runs for a person** (UI-90, #256).
//!
//! `blueprint/run_command { id }` is the agent's door onto the one command registry
//! (`commands.rs`): the id names an `Action`, the editor presses that action's chord, and the
//! same dispatcher, with the same gates, answers. So the agent cannot do what the key would refuse,
//! and a verb needs no second front end. `blueprint/state.ui` says where the person is looking, so an
//! agent knows which commands are live before it asks.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy_dark_factory::commands;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::project::OpenMap;
use serde_json::json;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .sized_descriptor("fence", "alpha", 2.064, 0.12)
        .mounted_descriptor("market_stall_awning_with_lanterns", "alpha", "worktop")
        .surface_descriptor("worktop_table", "alpha", "worktop")
        .build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("{e}"));
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

fn map(app: &App) -> blueprint_model::map::Map {
    app.world().resource::<OpenMap>().map.clone()
}

/// **Lay a run of fence the way the editor's own tests do**, so there is one thing to take back.
fn lay_a_run(app: &mut App) {
    bevy_dark_factory::editor::run_for_test(app.world_mut(), "fence", (0.0, 0.0), (8.0, 0.0));
    app.update();
}

/// **Cmd+Z as a person presses it.**
fn cmd_z(app: &mut App) {
    let m = bevy_dark_factory::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(m);
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyZ,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(m);
    app.update();
}

/// **The call, then the frames the key needs to land and let go.**
fn run(app: &mut App, id: &str) -> serde_json::Value {
    let answer = call(app, "blueprint/run_command", json!({"id": id}));
    for _ in 0..4 {
        app.update();
    }
    answer
}

/// **`edit.undo` over the door undoes exactly as Cmd+Z does**, and the state says the person is in
/// the workbench. The twin app takes the key; the two maps must be the same.
#[test]
fn an_agent_runs_edit_undo_exactly_as_cmd_z_does() {
    let mut by_key = editor("cmdkey");
    let mut by_id = editor("cmdid");
    let start = map(&by_id);
    for app in [&mut by_key, &mut by_id] {
        lay_a_run(app);
    }
    let laid = map(&by_id);
    assert_ne!(laid.placements.len(), start.placements.len(), "the run laid nothing; nothing to undo");

    cmd_z(&mut by_key);
    let answer = run(&mut by_id, "edit.undo");

    assert_eq!(answer["outcome"], json!("ran"), "answer: {answer}");
    assert_eq!(answer["id"], json!("edit.undo"));
    assert_eq!(
        map(&by_id).placements.len(),
        map(&by_key).placements.len(),
        "`edit.undo` must leave the map as Cmd+Z leaves it"
    );
    assert_eq!(map(&by_id).placements.len(), start.placements.len(), "the run is taken back");

    let state = call(&mut by_id, "blueprint/state", json!({}));
    assert_eq!(state["screen"], json!("editor"), "`screen` is the field that says the workbench is up (#601): {state}");
    assert_eq!(state["ui"]["workspace"], json!("MAP"), "and `ui.workspace` names which one: {state}");
    assert_eq!(state["editor"]["redo_depth"], json!(1), "the undone run is on the redo stack");

    let again = run(&mut by_id, "edit.redo");
    assert_eq!(again["outcome"], json!("ran"), "answer: {again}");
    assert_eq!(map(&by_id).placements.len(), laid.placements.len(), "`edit.redo` puts the run back");
}

/// **A command the key would refuse is refused, with the registry's own reason, and nothing runs.**
#[test]
fn a_command_that_is_not_enabled_is_refused_with_its_reason() {
    let mut app = editor("cmdoff");
    lay_a_run(&mut app);
    let live = *app.world().resource::<Live>();
    let (command, why) = commands::all()
        .into_iter()
        .find_map(|c| c.enabled(live).err().map(|why| (c, why)))
        .expect("some command is disabled in the map's idle state");
    let before = map(&app);

    let answer = run(&mut app, command.id);

    assert_eq!(answer["outcome"], json!("refused"), "answer: {answer}");
    assert_eq!(answer["why"], json!(why), "the reason is the registry's: {answer}");
    assert_eq!(map(&app), before, "a refused command changes nothing");
}

/// **An id nobody registered is refused and the answer helps**: it names the ids that contain the text.
#[test]
fn an_unknown_id_is_refused_and_names_the_nearest_ids() {
    let mut app = editor("cmdunknown");
    let answer = run(&mut app, "edit.undoo");
    assert_eq!(answer["outcome"], json!("refused"), "answer: {answer}");
    let near: Vec<&str> = answer["did_you_mean"]
        .as_array()
        .unwrap_or_else(|| panic!("no did_you_mean: {answer}"))
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(near.contains(&"edit.undo"), "`edit.undo` is the nearest id: {near:?}");
}

/// **`blueprint/state.ui` says where the person is looking**: the screen, the workspace, the panels it
/// has and the region that has the keys.
#[test]
fn the_state_says_the_screen_the_workspace_the_panels_and_who_has_the_keys() {
    let mut app = editor("cmdstate");
    let state = call(&mut app, "blueprint/state", json!({}));
    let ui = &state["ui"];
    assert_eq!(state["screen"], json!("editor"), "the workbench is up, and `screen` is the one field that says so (#601): {state}");
    assert!(ui.get("screen").is_none(), "`ui` does not spell the screen a second time: {ui}");
    assert_eq!(ui["workspace"], json!("MAP"), "ui: {ui}");
    assert_eq!(ui["panels"], json!(["MAP"]), "the Map door shows one panel: {ui}");
    assert_eq!(ui["focus"], json!("viewport"), "nothing holds the keys but the viewport: {ui}");
}

/// **Every chord is reachable by id, and the one door presses the same keys the key does** (#668).
///
/// Twenty-seven controls live in `chords::CHORDS` and are not census rows — the terminal's own, Zen
/// mode, the left panel, the palette, settings, the console. A person reached them; an agent reached
/// none, because `commands::all()` and `chords::CHORDS` shared no id at all. Now one resolver
/// (`commands::registered`) answers for both, `blueprint/run_command` presses the chord's own keys,
/// and there is no second dispatch: the answer comes from the same systems a key press reaches.
#[test]
fn terminal_collapse_by_id_moves_the_placement_one_step_round_the_cycle() {
    use bevy_dark_factory::terminal::Terminal;
    let mut app = editor("chord_terminal_collapse");
    let before = app.world().resource::<Terminal>().placement();
    let expected = before.next();

    let answer = run(&mut app, "terminal.collapse");

    assert_eq!(answer["outcome"], json!("ran"), "`terminal.collapse` is reachable by id (#668): {answer}");
    assert_eq!(answer["id"], json!("terminal.collapse"));
    assert_eq!(
        app.world().resource::<Terminal>().placement(),
        expected,
        "the door moved the pane one step round the cycle, from {before:?}"
    );
    // **The chord's own keys, spelled the way the host does**: the receipt names the same key the keymap
    // screen shows, because it is the chord that was pressed.
    assert!(answer["shortcut"].as_str().is_some_and(|key| key.contains('J')), "the receipt names the chord: {answer}");
}

/// **`shell.zen` by id begins Zen mode**, so an agent can stage the state a capture needs, and the
/// second call ends it — the same two presses a person makes.
#[test]
fn shell_zen_by_id_begins_and_ends_zen_mode() {
    use bevy_dark_factory::shell_view::ShellView;
    let mut app = editor("chord_shell_zen");
    assert!(!app.world().resource::<ShellView>().zen, "the editor opens out of Zen mode");

    let on = run(&mut app, "shell.zen");
    assert_eq!(on["outcome"], json!("ran"), "`shell.zen` is reachable by id (#668): {on}");
    assert!(app.world().resource::<ShellView>().zen, "and the door began Zen mode");

    let off = run(&mut app, "shell.zen");
    assert_eq!(off["outcome"], json!("ran"), "{off}");
    assert!(!app.world().resource::<ShellView>().zen, "and the second press ended it");
}

/// **The palette opens by id**, which is the other control a staged capture needs: `palette.open` is a
/// `Fixed` chord and was unreachable, so a capture of the palette could only be made by pressing keys.
#[test]
fn palette_open_by_id_opens_the_palette() {
    let mut app = editor("chord_palette_open");
    let before = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_gooey::CommandPalette>>().iter(world).count()
    };
    assert_eq!(before, 0, "the palette is shut to begin with");

    let answer = run(&mut app, "palette.open");

    assert_eq!(answer["outcome"], json!("ran"), "`palette.open` is reachable by id (#668): {answer}");
    let shown = {
        let world = app.world_mut();
        world
            .query_filtered::<&Node, With<bevy_gooey::CommandPalette>>()
            .iter(world)
            .any(|node| node.display != Display::None)
    };
    assert!(shown, "and the door opened it");
}

/// **The two registries are one list, and no chord is unreachable** — the assertion that stops the
/// two drifting 27 apart again.
///
/// It is over the registries themselves, not over a sample: every `chords::Fixed` id must resolve
/// through `commands::registered`, and every command id must too. A chord added without a door, or a
/// command renamed out of the resolver, fails here rather than in a capture that cannot be staged.
#[test]
fn every_chord_and_every_command_is_reachable_by_id() {
    use bevy_dark_factory::chords;
    use bevy_dark_factory::commands::registered;

    for named in chords::CHORDS {
        let found = registered(named.id).unwrap_or_else(|| panic!("`{}` ({:?}) is a chord nothing can reach (#668)", named.id, named.fixed));
        assert_eq!(found.id(), named.id, "the resolver answers with the id it was asked for");
        assert_eq!(found.title(), named.title, "and the chord's own words");
    }
    for command in commands::all() {
        let found = registered(command.id).unwrap_or_else(|| panic!("`{}` is a command nothing can reach", command.id));
        assert_eq!(found.id(), command.id, "the resolver answers with the id it was asked for");
    }
    // **No id names two controls**: a shared id would make which one runs a matter of lookup order.
    let mut seen = std::collections::HashSet::new();
    for id in commands::registered_ids() {
        assert!(seen.insert(id), "`{id}` is two controls' id");
    }
}

/// **An unknown id is still refused, and the nearest ids now include a chord's own** (#668).
///
/// The old refusal suggested `tiles.close` for `terminal.collapse`, because the chord's id was not in
/// the list it measured against. One list means a typo of a chord finds the chord.
#[test]
fn a_typo_of_a_chord_id_finds_the_chord() {
    let mut app = editor("chord_typo");
    let answer = run(&mut app, "terminal.collaps");

    assert_eq!(answer["outcome"], json!("refused"), "{answer}");
    let near: Vec<&str> = answer["did_you_mean"].as_array().expect("a list to choose from").iter().filter_map(|v| v.as_str()).collect();
    assert!(
        near.contains(&"terminal.collapse"),
        "the nearest id to `terminal.collaps` is the chord (#668): {near:?} in {answer}"
    );
}
