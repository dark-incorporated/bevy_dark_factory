//! **The journal behind `Cmd+E` is `bevy_gooey`'s notification centre** (UI-100, #442; carried from UI-39, #233).
//!
//! Every line of the journal — an agent's step, a refusal, a note — is a `NotificationRow` of gooey's one `ToastLog`, drawn by gooey; a step that can be
//! taken back carries an **Undo** button that runs the one undo by id (`blueprint/operation_undo`); the refusal toast and its journal row are one row; and
//! the hand-built panel and its systems are gone, which the last test pins the way the raw-node ratchet pins its count.

use bevy::prelude::*;
use bevy_dark_factory::chrome::{Journal, NotificationsOverlay};
use bevy_dark_factory::harness;
use bevy_gooey::{NotificationActionButton, NotificationRow, ToastKind, ToastLog};

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name).side(16.0).descriptor("crate", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
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
            "blueprint/operation_apply",
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

fn oplog(root: &std::path::Path) -> Vec<bevy_dark_factory::ops::Entry> {
    bevy_dark_factory::ops::read_entries(&bevy_dark_factory::ops::oplog_path_at(root))
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
    let placed_before = app.world().resource::<bevy_dark_factory::project::OpenMap>().map.placements.len();
    bevy_gooey::testing::click(&mut app, buttons[0].0);
    for _ in 0..6 {
        app.update();
    }

    let after = oplog(&root);
    assert_eq!(after.len(), lines_before + 1, "one new line in the operation log");
    let undo = after.last().expect("the undo's line");
    assert_eq!(undo.undoes.as_deref(), Some(ids[2].as_str()), "the line names the step it took back: {undo:?}");
    let placed_after = app.world().resource::<bevy_dark_factory::project::OpenMap>().map.placements.len();
    assert_eq!(placed_after, placed_before - 1, "the map is back by exactly that step");
    assert_eq!(app.world().resource::<bevy_dark_factory::project::OpenMap>().map.placements.len(), placed_before - 1, "and the other two stand");

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
    app.world_mut().resource_mut::<bevy_dark_factory::editor::EditorState>().status.problem(line.to_owned());
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

    app.world_mut().resource_mut::<bevy_dark_factory::editor::EditorState>().status.problem(line.to_owned());
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

/// **A person's Undo is the person's** (review of #442): with the agent paused (UI-60) the press still takes the step back, the oplog line says `human` and
/// not `agent`, the agent is not told it was heard (the AGENT block's "last heard" is the agent's), and the receipt is not the agent's.
#[test]
fn a_persons_undo_press_works_while_the_agent_is_paused_and_is_written_human() {
    use bevy_dark_factory::night_shift::{AgentActivity, AgentPause};
    let (root, mut app) = editor("centre_person");
    let ids = three_placements(&mut app);
    harness::resize_surface(&mut app, 1536, 1400).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(AgentPause(true));
    open_the_journal(&mut app);
    let heard = app.world().get_resource::<AgentActivity>().map(AgentActivity::heard);
    let buttons = undo_buttons(&mut app);
    assert_eq!(buttons.len(), 1, "{buttons:?}");
    let placed_before = app.world().resource::<bevy_dark_factory::project::OpenMap>().map.placements.len();
    let lines_before = oplog(&root).len();

    bevy_gooey::testing::click(&mut app, buttons[0].0);
    for _ in 0..6 {
        app.update();
    }

    let after = oplog(&root);
    assert_eq!(after.len(), lines_before + 1, "the press undid the step although the agent is paused");
    let undo = after.last().expect("the undo's line");
    assert_eq!(undo.undoes.as_deref(), Some(ids[2].as_str()));
    assert_eq!(serde_json::to_value(undo.source).unwrap_or_default(), "human", "a person's press is written human: {undo:?}");
    assert_eq!(app.world().resource::<bevy_dark_factory::project::OpenMap>().map.placements.len(), placed_before - 1, "the map is back by that step");
    assert_eq!(app.world().get_resource::<AgentActivity>().map(AgentActivity::heard), heard, "the agent was not called, so the AGENT block does not say it was heard");
    let status = &app.world().resource::<bevy_dark_factory::editor::EditorState>().status;
    assert!(!status.has_problem(), "the agent-pause refusal was not raised: {:?}", status.problems());
    assert!(!status.note_text().starts_with("agent:"), "the receipt is the person's: {}", status.note_text());
}

/// **A stale press refuses and undoes nothing else**: the Undo of a step a newer change sits on is refused with the log's own reason, as a problem the
/// toast and the journal carry; the log gains no line and the map is as it was.
#[test]
fn a_stale_undo_press_is_refused_and_undoes_nothing_else() {
    use bevy_gooey::NotificationAction;
    let (root, mut app) = editor("centre_stale");
    let ids = three_placements(&mut app);
    // A fourth change lands after the row was drawn: the third step is no longer the newest.
    let answer = call(
        &mut app,
        "blueprint/operation_apply",
        serde_json::json!({"operation": "placement_add", "parameters": {"piece": "crate", "at": [8.0, 2.0]}, "dry_run": false}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    app.update();
    let lines_before = oplog(&root).len();
    let placed_before = app.world().resource::<bevy_dark_factory::project::OpenMap>().map.placements.len();
    let problems_before = app.world().resource::<bevy_dark_factory::editor::EditorState>().status.problems().len();

    // The press of the old row's Undo, as the event the centre sends for it.
    app.world_mut().trigger(NotificationAction { entry: 0, id: format!("undo:{}", ids[2]) });
    for _ in 0..4 {
        app.update();
    }

    assert_eq!(oplog(&root).len(), lines_before, "a refused undo writes nothing");
    assert_eq!(app.world().resource::<bevy_dark_factory::project::OpenMap>().map.placements.len(), placed_before, "and nothing else was undone");
    let status = &app.world().resource::<bevy_dark_factory::editor::EditorState>().status;
    assert_eq!(status.problems().len(), problems_before + 1, "the refusal is a problem, which the toast and the journal carry: {:?}", status.problems());
}

/// **The log holds what it can and says what it left out** (review of #442): the journal can hold more lines than gooey's log does, and the log's own push
/// drops its oldest in silence. The sync keeps the log within its capacity, writes a note counting what is left out, keeps no id for a row that is gone, and
/// writes a repeat of a refusal that was left out again.
#[test]
fn a_journal_longer_than_the_log_says_what_it_left_out_and_keeps_no_stale_id() {
    let (_root, mut app) = editor("centre_capacity");
    let capacity = ToastLog::CAPACITY;
    let step = |n: usize| bevy_dark_factory::steps::Step {
        seq: n as u64,
        operation_id: format!("op-{n}"),
        op: "placement_add".to_owned(),
        text: format!("placed piece {n}"),
        capture: None,
    };
    // 200 refusals (the journal's cap) and 130 steps: 330 lines for a log of 200.
    {
        let mut journal = app.world_mut().resource_mut::<Journal>();
        for i in 0..bevy_dark_factory::chrome::JOURNAL_CAP {
            journal.record(&format!("refusal {i}"), 1);
        }
        journal.steps = (0..130).map(step).collect();
    }
    for _ in 0..3 {
        app.update();
    }
    let (held, notes, left_note) = {
        let log = app.world().resource::<ToastLog>();
        let left: Vec<String> = log.entries().filter(|e| e.text.contains("left out")).map(|e| e.text.clone()).collect();
        let ours = log.entries().filter(|e| !e.text.contains("left out")).count();
        (log.len(), ours, left)
    };
    assert!(held <= capacity, "the log holds {held} rows, past its capacity {capacity}");
    assert_eq!(left_note.len(), 1, "one note says what was left out: {left_note:?}");
    let left = 330 - notes;
    assert!(left > 0, "the journal is longer than the log, so something is left out");
    assert!(left_note[0].starts_with(&format!("{left} older line(s) are left out")), "the note counts what is left out ({left}): {left_note:?}");
    assert!(left_note[0].contains("operation log"), "and says where the whole record is: {left_note:?}");
    assert!(left_note[0].contains(&format!("holds the newest {notes} ")), "and the number it keeps is the rows the journal holds ({notes}), not the log's capacity: {left_note:?}");
    {
        // **No stale id**: a refusal the log holds is found by its id, one it left out is found by none.
        let journal = app.world().resource::<Journal>();
        let log = app.world().resource::<ToastLog>();
        let mut some = 0;
        for entry in journal.entries() {
            if let Some(id) = journal.log_id_of(&entry.text) {
                some += 1;
                assert!(log.get(id).is_some(), "`{}` keeps the id {id} of a row the log no longer holds", entry.text);
            }
        }
        assert!(some > 0 && some < bevy_dark_factory::chrome::JOURNAL_CAP, "some refusals are rows and the oldest are left out: {some}");
        assert!(journal.log_id_of("refusal 0").is_none(), "the oldest refusal was left out");
        assert!(journal.log_id_of(&format!("refusal {}", bevy_dark_factory::chrome::JOURNAL_CAP - 1)).is_some(), "the newest refusal is a row");
    }

    // **A repeat of a refusal that was left out is written again**, once. The last refusal is the one that folds, so it is made the oldest row first.
    {
        let mut journal = app.world_mut().resource_mut::<Journal>();
        journal.steps.clear();
        journal.record("the oldest", 1);
    }
    for _ in 0..3 {
        app.update();
    }
    {
        let mut journal = app.world_mut().resource_mut::<Journal>();
        journal.steps = (1000..1250).map(step).collect();
    }
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().resource::<Journal>().log_id_of("the oldest").is_none(), "the refusal was left out by the steps that came after it");
    app.world_mut().resource_mut::<Journal>().record("the oldest", 1);
    for _ in 0..3 {
        app.update();
    }
    let journal = app.world().resource::<Journal>();
    let log = app.world().resource::<ToastLog>();
    let id = journal.log_id_of("the oldest").expect("the repeat of a refusal that was left out is a row again");
    assert_eq!(log.get(id).map(|row| row.count()), Some(2), "with its tally");
    assert_eq!(log.entries().filter(|e| e.text == "the oldest").count(), 1, "and only once");
    assert!(log.len() <= capacity, "still within the capacity: {}", log.len());
}

/// **(4) No `JournalPanel` remains**, nor the list, the line, the paint or the spawn it had: a source that names one has put a second path back. The scan
/// walks `src/` and every folder under it.
#[test]
fn no_hand_built_journal_panel_remains() {
    fn walk(dir: &std::path::Path, found: &mut Vec<String>) {
        let gone = ["JournalPanel", "JournalList", "ProblemLogLine", "problem_log_line", "paint_journal", "fn journal_panel"];
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                for (n, line) in text.lines().enumerate() {
                    if gone.iter().any(|g| line.contains(g)) {
                        found.push(format!("{}:{}: {}", path.display(), n + 1, line.trim()));
                    }
                }
            }
        }
    }
    let mut found = Vec::new();
    walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut found);
    assert!(found.is_empty(), "the journal is gooey's notification centre; the hand-built panel is gone:\n{}", found.join("\n"));
}
