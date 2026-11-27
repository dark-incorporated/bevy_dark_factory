//! **The person's audit** (DL-07, #466; series labels from use, #459).
//!
//! The Agent workspace offers an Audit row: one turn's pictures and one of its yes/no questions, answered Y, N or S. The answers are gold,
//! per question, and marked `holdout`. What is measured:
//!
//! 1. **The row offers a record's questions, and Y, N and S answer, the next one following.** Each answer is a gold label on the turn, naming the
//!    question and marked `holdout`; a skip is recorded as `skip`, never as a no; an answered question is never offered again.
//! 2. **The sample is stratified**: what the assessor was unsure of comes first, the rest is random, and a question is offered once.
//! 3. **The holdout is one place**: `Read::holdout` names the audit's lines and not the hand-back's own verdicts.
//! 4. **Off is off**: with collection off there is nothing to audit and no row.
//! 5. **A line another process appends reaches the row**, with the clock stopped: the project index's second root follows the ledger, and no timer does.
//!
//! (DL-06's export of the holdout into its own file is not built yet; `Read::holdout` is the one place it will ask.)

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;

use bevy_dark_factory::audit_view::{Audit, AuditButton, AuditRow, Why, pick};
use bevy_dark_factory::project_index::{MIN_SWEEP, OutsideFiles};
use bevy_dark_factory::turn_store::{AssessedQuestion, Assessment, AssessorRef, Line, Store};
use bevy_dark_factory::workspace::Workspace;

use crate::a_turn_is_recorded_for_learning::{editor, ledger_of, place, present, until_written};
use crate::settle_support::Frames;

/// The wall-clock pause between frames while waiting.
const PAUSE: Duration = Duration::from_millis(10);
/// How many frames a test steps for the row to follow a write (#644): a count, not a wall-clock window, so a starved host does not
/// cut it short; at [`PAUSE`] apart they cover at least 30 s.
const WAIT: Frames = Frames::covering(Duration::from_secs(30), PAUSE);

/// An editor with one turn recorded, pictures and all, and the Agent workspace open.
fn recorded(tag: &str) -> (App, std::path::PathBuf) {
    let (mut app, labels) = editor(tag);
    // The camera has a viewport of its own, so the pictures are asked for (the headless window has none).
    for mut camera in app.world_mut().query_filtered::<&mut Camera, With<bevy_dark_factory::view::MainCamera>>().iter_mut(app.world_mut()) {
        camera.viewport = Some(bevy::camera::Viewport { physical_position: UVec2::new(10, 10), physical_size: UVec2::new(100, 100), ..default() });
    }
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    let first = place(&mut app, [20.0, 18.0]);
    present(&mut app, &[first], &["a bed"]);
    until_written(&mut app);
    // The person keeps the change, which is how a hand-back ends and what lets them go to another workspace.
    press(&mut app, KeyCode::KeyN, "n");
    for _ in 0..5 {
        app.update();
    }
    assert!(!app.world().resource::<bevy_dark_factory::confirm::Confirm>().is_open(), "the hand-back's question is still up");
    (app, labels)
}

fn open_the_agent_workspace(app: &mut App) {
    let command = bevy_dark_factory::commands::by_id("workspace.agent").expect("`workspace.agent` is a command");
    let live = *app.world().resource::<bevy_dark_factory::keys::Live>();
    app.world_mut().resource_mut::<bevy_dark_factory::command_run::Chords>().queue(&command, live);
    for _ in 0..30 {
        app.update();
    }
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Agent, "the Agent workspace did not open");
}

fn press(app: &mut App, code: KeyCode, text: &str) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: Key::Character(text.into()),
            state,
            text: (state == ButtonState::Pressed).then(|| text.into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
}

fn first_offer(app: &App) -> (String, String) {
    let offer = app.world().resource::<Audit>().offers.first().cloned().unwrap_or_else(|| panic!("nothing is offered"));
    (offer.turn, offer.question)
}

/// **1. The row offers the questions; Y, N and S answer; the next follows.**
#[test]
fn y_n_and_s_answer_the_questions_and_the_next_follows() {
    let (mut app, labels) = recorded("audit_keys");
    open_the_agent_workspace(&mut app);
    let waiting = app.world().resource::<Audit>().offers.len();
    assert!(waiting >= 3, "the record's questions were not offered: {waiting}");
    let rows = app.world_mut().query_filtered::<Entity, With<AuditRow>>().iter(app.world()).count();
    assert_eq!(rows, 1, "the Audit row is not in the Agent block");
    let buttons = app.world_mut().query::<&AuditButton>().iter(app.world()).count();
    assert_eq!(buttons, 3, "the row has not got Yes, No and Skip");

    let mut asked = Vec::new();
    for (code, text, word) in [(KeyCode::KeyY, "y", "yes"), (KeyCode::KeyN, "n", "no"), (KeyCode::KeyS, "s", "skip")] {
        let (turn, question) = first_offer(&app);
        assert!(!asked.contains(&question), "`{question}` was offered twice");
        press(&mut app, code, text);
        let read = ledger_of(&labels);
        let last = read.audits().last().unwrap_or_else(|| panic!("no answer was written for {word}"));
        assert_eq!((last.turn.as_str(), last.question.as_deref(), last.answer.as_deref()), (turn.as_str(), Some(question.as_str()), Some(word)));
        assert_eq!(last.tier, "gold");
        assert!(last.holdout, "an audit answer is the holdout");
        assert!(last.asked_because.is_some());
        asked.push(question);
        for _ in 0..10 {
            app.update();
        }
        assert_ne!(first_offer(&app).1, asked[asked.len() - 1], "the question that was answered is still on the row");
    }
    let read = ledger_of(&labels);
    assert_eq!(read.audits().count(), 3);
    assert_eq!(read.holdout().count(), 3, "the holdout is the audit's lines and nothing else");
    assert_eq!(read.audits().filter(|a| a.answer.as_deref() == Some("skip")).count(), 1, "a skip is a skip");
    assert_eq!(app.world().resource::<Audit>().offers.len(), waiting - 3);
}

/// **The keys answer only on the Agent workspace, and not with a modifier held.**
#[test]
fn the_keys_answer_only_on_the_agent_workspace() {
    let (mut app, labels) = recorded("audit_where");
    open_the_agent_workspace(&mut app);
    *app.world_mut().resource_mut::<Workspace>() = Workspace::Map;
    for _ in 0..5 {
        app.update();
    }
    press(&mut app, KeyCode::KeyY, "y");
    assert_eq!(ledger_of(&labels).audits().count(), 0, "Y on the Map answered the audit");
}

/// **The buttons answer as the keys do**, so the row is usable with the mouse.
#[test]
fn the_buttons_answer_as_the_keys_do() {
    let (mut app, labels) = recorded("audit_buttons");
    open_the_agent_workspace(&mut app);
    let (_, question) = first_offer(&app);
    let no = app.world_mut().query::<(Entity, &AuditButton)>().iter(app.world()).find(|(_, b)| **b == AuditButton::No).map(|(e, _)| e).expect("a No button");
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: no });
    for _ in 0..5 {
        app.update();
    }
    let read = ledger_of(&labels);
    let last = read.audits().last().expect("the button wrote nothing");
    assert_eq!((last.question.as_deref(), last.answer.as_deref()), (Some(question.as_str()), Some("no")));
}

/// **2. The sample is stratified**: what the assessor was unsure of comes first, and nothing is offered twice.
#[test]
fn what_the_assessor_was_unsure_of_is_offered_first() {
    let (app, labels) = recorded("audit_pick");
    let read = ledger_of(&labels);
    let turn = read.turns().next().expect("a turn");
    let ids: Vec<String> = turn.questions.items.iter().filter(|i| i.question["type"] == "noul").map(|i| i.id.clone()).collect();
    assert!(ids.len() >= 4, "too few yes/no questions to stratify: {ids:?}");
    // The last question was refused once, the one before it split two to one, the rest were unanimous.
    let graded = |id: &String, yes_fraction: Option<f64>, refused: u32| AssessedQuestion { id: id.clone(), votes: Vec::new(), yes_fraction, tally: Default::default(), refused };
    let n = ids.len();
    let answers: Vec<AssessedQuestion> = ids
        .iter()
        .enumerate()
        .map(|(at, id)| match at {
            a if a == n - 1 => graded(id, Some(1.0), 1),
            a if a == n - 2 => graded(id, Some(2.0 / 3.0), 0),
            _ => graded(id, Some(1.0), 0),
        })
        .collect();
    let mut store = Store::new(labels.clone());
    let assessment = Assessment {
        turn: turn.id.clone(),
        assessor: AssessorRef { version: 1, endpoint: "http://localhost/".to_owned(), site: "local".to_owned() },
        at_unix_ms: 1,
        answers,
        skipped: None,
    };
    store.append(&Line::assessment(assessment)).expect("the assessment is appended");
    drop(app);

    let read = ledger_of(&labels);
    let offers = pick(&read, ids.len(), 7);
    assert_eq!(offers.len(), ids.len());
    assert_eq!((offers[0].question.as_str(), offers[0].why), (ids[n - 1].as_str(), Why::Split), "a refused vote is the least sure");
    assert_eq!((offers[1].question.as_str(), offers[1].why), (ids[n - 2].as_str(), Why::Split), "then the split");
    assert!(offers[2..].iter().all(|o| o.why == Why::Random), "the rest is random");
    let mut names: Vec<&str> = offers.iter().map(|o| o.question.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), offers.len(), "a question was offered twice");
}

/// **3. Off is off**: with collection off there is nothing to audit and no row.
#[test]
fn with_collection_off_there_is_nothing_to_audit() {
    let (mut app, labels) = editor("audit_off");
    let file = std::env::temp_dir().join(format!("blueprint_audit_off_settings_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(file.parent().expect("a folder")).expect("a scratch folder");
    std::fs::write(&file, "{version: 1, collect_labels: false}").expect("a settings file");
    app.insert_resource(bevy_dark_factory::settings::UserSettingsFile(file));
    for _ in 0..400 {
        app.update();
        if !app.world().resource::<bevy_dark_factory::settings::EditorSettings>().collect_labels {
            break;
        }
    }
    open_the_agent_workspace(&mut app);
    assert!(app.world().resource::<Audit>().offers.is_empty());
    assert_eq!(app.world_mut().query_filtered::<Entity, With<AuditRow>>().iter(app.world()).count(), 0);
    assert!(!labels.exists(), "something was written with collection off");
}

/// **5. A line another process appends to the ledger reaches the row, and no timer brings it** (#501; `audit_view::LOOK_EVERY_S` retired).
///
/// The assessor is another process: it appends an assessment to the ledger file. Here that is a `Store` that appends one saying the assessor
/// refused the vote on the third question offered, which makes it the least sure and so the first the sample takes after the one on screen. The
/// clock is frozen (a manual time step of nothing), so a row that still waited for five seconds of `Time` could not pass, and the index's sweep is at
/// its floor. The expectation is what the test chose, not anything the audit computes: the third offer must become the second.
#[test]
fn a_line_another_process_appends_reaches_the_row_with_the_clock_stopped() {
    let (mut app, labels) = recorded("audit_external");
    open_the_agent_workspace(&mut app);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    app.world().resource::<OutsideFiles>().set_sweep(&labels, MIN_SWEEP);
    let offered: Vec<(String, String)> = app.world().resource::<Audit>().offers.iter().map(|o| (o.turn.clone(), o.question.clone())).collect();
    assert!(offered.len() >= 4, "the record's questions were not offered: {offered:?}");
    let (turn, target) = offered[2].clone();

    // The assessor was unsure of that one: its vote was refused.
    let refused = AssessedQuestion { id: target.clone(), votes: Vec::new(), yes_fraction: Some(1.0), tally: Default::default(), refused: 1 };
    let assessment = Assessment {
        turn,
        assessor: AssessorRef { version: 1, endpoint: "http://localhost/".to_owned(), site: "local".to_owned() },
        at_unix_ms: 1,
        answers: vec![refused],
        skipped: None,
    };
    Store::new(labels.clone()).append(&Line::assessment(assessment)).expect("the assessment is appended");

    let mut second = None;
    for _ in 0..WAIT.0 {
        app.update();
        second = app.world().resource::<Audit>().offers.get(1).map(|o| o.question.clone());
        if second.as_deref() == Some(target.as_str()) {
            break;
        }
        std::thread::sleep(PAUSE);
    }
    assert!(second.as_deref() == Some(target.as_str()), "the assessment reached the row within {WAIT:?}: the second offer is {second:?}, not {target}");
    assert_eq!(app.world().resource::<Audit>().offers[0].question, offered[0].1, "the question on screen stays under the person's hand");
}
