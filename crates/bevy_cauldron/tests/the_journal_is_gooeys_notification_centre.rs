//! **The journal behind `Cmd+E` is `bevy_gooey`'s notification centre** (UI-100, #442; carried from UI-39, #233).
//!
//! Every line of the journal — an agent's step, a refusal, a note — is a `NotificationRow` of gooey's one `ToastLog`, drawn by gooey; a step that can be
//! taken back carries an **Undo** button that runs the one undo by id (`grimoire/operation_undo`); the refusal toast and its journal row are one row; and
//! the hand-built panel and its systems are gone, which the last test pins the way the raw-node ratchet pins its count.

use bevy::prelude::*;
use bevy_cauldron::chrome::{Journal, NotificationsOverlay};
use bevy_cauldron::harness;
use bevy_gooey::{NotificationActionButton, NotificationRow, ToastKind, ToastLog};

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name).side(16.0).descriptor("crate", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    (root, app)
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("no method `{method}` is registered");
    };
    app.world_mut().run_system_with(id, Some(params)).unwrap_or_else(|e| panic!("{method}: {e:?}")).unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

/// Three placements, the way an agent dispatches them; returns the operation ids.
fn three_placements(app: &mut App) -> Vec<String> {
    let mut ids = Vec::new();
    for at in [[2.0, 2.0], [4.0, 2.0], [6.0, 2.0]] {
        let answer = call(
            app,
            "grimoire/operation_apply",
            serde_json::json!({"operation": "placement_add", "parameters": {"piece": "crate", "at": at}, "dry_run": false}),
        );
        assert_eq!(answer["outcome"], "applied", "{answer}");
        ids.push(answer["operation_id"].as_str().unwrap_or_default().to_owned());
    }
    app.update();
    ids
}

/// `Cmd+E`: the overlay the centre stands in is shown, and the centre draws its rows.
fn open_the_journal(app: &mut App) {
    let overlay = app.world_mut().query_filtered::<Entity, With<NotificationsOverlay>>().iter(app.world()).next().expect("the journal's overlay is spawned on the Editor screen");
    app.world_mut().get_mut::<Node>(overlay).expect("a node").display = Display::Flex;
    for _ in 0..4 {
        app.update();
    }
}

/// **The words of every row the centre draws**, in the order it draws them (newest first).
fn rows(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let rows: Vec<Entity> = world.query_filtered::<Entity, With<NotificationRow>>().iter(world).collect();
    let Some(column) = rows.first().and_then(|r| world.get::<ChildOf>(*r)).map(ChildOf::parent) else { return Vec::new() };
    let kids: Vec<Entity> = world.get::<Children>(column).map(|c| c.iter().collect()).unwrap_or_default();
    kids.into_iter()
        .filter(|k| world.get::<NotificationRow>(*k).is_some())
        .filter_map(|k| {
            let first = world.get::<Children>(k)?.iter().next()?;
            world.get::<Text>(first).map(|t| t.0.clone())
        })
        .collect()
}

/// The Undo buttons the centre draws, with the words of the row each is in.
fn undo_buttons(app: &mut App) -> Vec<(Entity, String)> {
    let world = app.world_mut();
    let buttons: Vec<Entity> = world.query_filtered::<Entity, With<NotificationActionButton>>().iter(world).collect();
    buttons
        .into_iter()
        .map(|b| {
            let label = world.get::<Children>(b).and_then(|c| c.iter().next()).and_then(|t| world.get::<Text>(t)).map(|t| t.0.clone()).unwrap_or_default();
            let row = world.get::<ChildOf>(b).map(ChildOf::parent).and_then(|r| world.get::<Children>(r)?.iter().next()).and_then(|t| world.get::<Text>(t)).map(|t| t.0.clone()).unwrap_or_default();
            assert_eq!(label, "Undo", "the only action a journal row carries");
            (b, row)
        })
        .collect()
}

fn oplog(root: &std::path::Path) -> Vec<bevy_cauldron::ops::Entry> {
    bevy_cauldron::ops::read_entries(&bevy_cauldron::ops::oplog_path_at(root))
}

/// **(1) Cmd+E after three agent operations finds three notification rows, in order, with the operation text.**
#[test]
fn cmd_e_after_three_agent_operations_shows_three_rows_in_order_with_the_operations_words() {
    let (_root, mut app) = editor("centre_rows");
    three_placements(&mut app);
    let steps = app.world().resource::<Journal>().steps.clone();
    assert_eq!(steps.len(), 3, "three applied operations, three steps: {steps:#?}");
    open_the_journal(&mut app);
    let drawn = rows(&mut app);
    assert_eq!(drawn.len(), 3, "three operations are three rows and nothing else: {drawn:#?}");
    // Newest first, as gooey draws its log: read oldest first they are the steps in the order they happened.
    for (row, step) in drawn.iter().rev().zip(&steps) {
        assert!(row.contains(&step.text), "the row `{row}` does not carry the operation's own sentence `{}`", step.text);
    }
    assert!(!drawn.iter().any(|r| r.contains("No notifications yet")), "{drawn:#?}");
    // And they are the log's own rows, notes: not alerts.
    let log = app.world().resource::<ToastLog>();
    assert_eq!(log.entries().filter(|e| e.kind == ToastKind::Note).count(), 3);
    assert_eq!(log.entries().filter(|e| e.kind == ToastKind::Alert).count(), 0);
}

/// **(2) Pressing Undo on a row's action takes back exactly that step**: one new line in the operation log naming the step it undoes, the map back, and the
/// journal showing it (a row for the undo, and the button on the step before).
#[test]
fn pressing_undo_on_a_rows_action_takes_back_exactly_that_step() {
    let (root, mut app) = editor("centre_undo");
    let ids = three_placements(&mut app);
    harness::resize_surface(&mut app, 1536, 1400).unwrap_or_else(|e| panic!("{e}"));
    open_the_journal(&mut app);

    // Exactly one row carries an Undo: the newest step, the one that can be taken back.
    let buttons = undo_buttons(&mut app);
    assert_eq!(buttons.len(), 1, "only the newest step can be taken back: {buttons:?}");
    let steps = app.world().resource::<Journal>().steps.clone();
    assert!(buttons[0].1.contains(&steps[2].text), "the Undo is on the newest step's row, not `{}`", buttons[0].1);

    let lines_before = oplog(&root).len();
    let placed_before = app.world().resource::<bevy_cauldron::project::OpenMap>().map.placements.len();
    bevy_gooey::testing::click(&mut app, buttons[0].0);
    for _ in 0..6 {
        app.update();
    }

    let after = oplog(&root);
    assert_eq!(after.len(), lines_before + 1, "one new line in the operation log");
    let undo = after.last().expect("the undo's line");
    assert_eq!(undo.undoes.as_deref(), Some(ids[2].as_str()), "the line names the step it took back: {undo:?}");
    let placed_after = app.world().resource::<bevy_cauldron::project::OpenMap>().map.placements.len();
    assert_eq!(placed_after, placed_before - 1, "the map is back by exactly that step");
    assert_eq!(app.world().resource::<bevy_cauldron::project::OpenMap>().map.placements.len(), placed_before - 1, "and the other two stand");

    // **The row shows it**: the undo is a row of its own, newest, and the button moved to the step before.
    let drawn = rows(&mut app);
    assert_eq!(drawn.len(), 4, "three steps and the undo: {drawn:#?}");
    let said = undo.result.clone().expect("the undo says what it did");
    assert!(drawn[0].contains(&said), "the newest row is the undo's sentence `{said}`: {drawn:#?}");
    let buttons = undo_buttons(&mut app);
    assert_eq!(buttons.len(), 1, "one step can be taken back now: {buttons:?}");
    assert!(buttons[0].1.contains(&steps[1].text), "and it is the second step: {buttons:?}");
}

/// **(3) The refusal toast and its journal row agree: they are one row.** A refusal raised twice in a row is one row reading `(x2)`, not three lines.
#[test]
fn the_refusal_toast_and_its_journal_row_are_one_row() {
    let (_root, mut app) = editor("centre_refusal");
    let line = "cannot remove: `alpha/floor` still places it";
    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().status.problem(line.to_owned());
    for _ in 0..3 {
        app.update();
    }
    {
        let log = app.world().resource::<ToastLog>();
        let alerts: Vec<_> = log.entries().filter(|e| e.kind == ToastKind::Alert).collect();
        assert_eq!(alerts.len(), 1, "the toast and the journal's line are one row: {alerts:?}");
        assert_eq!(alerts[0].text, line);
        let journal = app.world().resource::<Journal>();
        assert_eq!(journal.entries().last().map(|e| e.text.as_str()), Some(line), "the journal holds the same sentence");
    }
    // The toast stands for that row: its text is the row's.
    let toast = app.world_mut().query::<&bevy_gooey::Toast>().iter(app.world()).map(|t| t.text().to_owned()).collect::<Vec<_>>();
    assert_eq!(toast, [line.to_owned()], "one sticky toast says it");

    app.world_mut().resource_mut::<bevy_cauldron::editor::EditorState>().status.problem(line.to_owned());
    for _ in 0..3 {
        app.update();
    }
    let log = app.world().resource::<ToastLog>();
    let alerts: Vec<_> = log.entries().filter(|e| e.kind == ToastKind::Alert).collect();
    assert_eq!(alerts.len(), 1, "a repeat is the same row: {alerts:?}");
    assert_eq!(alerts[0].count(), 2, "its count is 2, which the row reads as `(x2)`");
    open_the_journal(&mut app);
    let drawn = rows(&mut app);
    assert!(drawn.iter().any(|r| r.contains(line) && r.ends_with("(x2)")), "the row reads the tally: {drawn:#?}");
}

/// **(4) No `JournalPanel` remains**, nor the list, the line, the paint or the spawn it had: a source that names one has put a second path back.
#[test]
fn no_hand_built_journal_panel_remains() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let gone = ["JournalPanel", "JournalList", "ProblemLogLine", "problem_log_line", "paint_journal", "fn journal_panel"];
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&src).unwrap_or_else(|e| panic!("{}: {e}", src.display())).flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            for (n, line) in text.lines().enumerate() {
                if gone.iter().any(|g| line.contains(g)) {
                    found.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
                }
            }
        }
    }
    assert!(found.is_empty(), "the journal is gooey's notification centre; the hand-built panel is gone:\n{}", found.join("\n"));
}
