//! **Every turn carries the typed questions a student will later be asked about it** (DL-03, #462; series labels from use, #459).
//!
//! They are written in the decision model's own shapes, so a label and a prediction are about the same question: one declarative `noul` per
//! `specified` line, the named checks with the engine's answer beside each where the engine can compute one, the agent's own `blueprint_decide`
//! questions verbatim, and the turn question whose gold label is the person's verdict.
//!
//! # What is proved
//!
//! 1. **Three specified lines and a floating piece** give three declarative `noul` questions, a `floating` check whose engine answer is `yes`, and
//!    the turn question. The questions are declarative: laya's `noul` head answered 0.0000 to every interrogative instruction on 2026-09-21.
//! 2. **An empty or over-long `specified` line produces no question, and the record says so**, with the line whole.
//! 3. **Changing the template version keeps old records' questions as they were**: the wording is stored, never regenerated on read.
//! 4. **The agent's own decision questions are kept verbatim**, and the engine answers the checks it can (a flat picture is `blank_frame`).

use bevy::prelude::*;

use bevy_dark_factory::turn_ledger::{ShotImage, TurnLedger};
use bevy_dark_factory::turn_questions::{Engine, Input, NAMED_CHECKS, SPECIFIED_MAX_CHARS, TEMPLATE_VERSION, TURN_QUESTION_ID, Templates, questions_for};
use bevy_dark_factory::turn_store::Turn;

use crate::a_turn_is_recorded_for_learning::{FRAMES_AT_MOST, call, editor, ledger_of, place, present, until_written};

/// **One agent turn, handed back with these specified lines**, and the record it leaves.
fn a_recorded_turn(tag: &str, lift: Option<f32>, specified: &[&str]) -> (Turn, App) {
    let (mut app, labels) = editor(tag);
    let piece_op = place(&mut app, [20.0, 18.0]);
    let mut ids = vec![piece_op];
    if let Some(lift) = lift {
        let id = app.world().resource::<bevy_dark_factory::project::OpenMap>().map.placements.last().expect("the piece just placed").id.clone();
        let lifted = call(
            &mut app,
            "blueprint/operation_apply",
            serde_json::json!({"operation": "placement_set_lift", "parameters": {"id": id, "lift": lift}, "dry_run": false}),
        );
        assert_eq!(lifted["outcome"], "applied", "{lifted}");
        ids.push(lifted["operation_id"].as_str().expect("an id").to_owned());
    }
    crate::a_turn_is_recorded_for_learning::render_what_is_asked(&mut app);
    present(&mut app, &ids, specified);
    until_written(&mut app);
    let read = ledger_of(&labels);
    let turn = read.turns().next().expect("a turn").clone();
    (turn, app)
}

fn instructions(question: &serde_json::Value) -> &str {
    question["instructions"].as_str().unwrap_or_else(|| panic!("a noul question carries its instructions: {question}"))
}

/// **1. Three specified lines and a floating piece.**
#[test]
fn three_specified_lines_and_a_floating_piece_give_the_questions_and_the_engines_answer() {
    let (turn, _app) = a_recorded_turn("questions_floating", Some(0.4), &["a bed to the east", "another beside the door", "both facing north"]);
    let q = &turn.questions;
    assert_eq!(q.template_version, TEMPLATE_VERSION);
    assert!(q.skipped.is_empty(), "{:?}", q.skipped);

    let specified: Vec<_> = q.items.iter().filter(|item| item.source == "specified").collect();
    assert_eq!(specified.len(), 3, "one question per specified line: {:?}", q.items);
    for (item, line) in specified.iter().zip(["a bed to the east", "another beside the door", "both facing north"]) {
        assert_eq!(item.question["type"], "noul", "{item:?}");
        assert_eq!(instructions(&item.question), format!("The result satisfies: {line}"));
        // **Declarative, never a question**: the one form known to work.
        let said = instructions(&item.question);
        assert!(!said.ends_with('?'), "{said}");
        assert!(!["Does ", "Is ", "Are ", "Do ", "Did ", "Can "].iter().any(|word| said.starts_with(word)), "{said}");
    }

    // **The floating check, with the engine's answer**: the piece was lifted off where it would rest.
    let floating = q.items.iter().find(|item| item.id == "check-floating").expect("the floating check");
    assert_eq!(floating.source, "check");
    assert_eq!(floating.question["type"], "noul");
    let engine = floating.engine.as_ref().expect("the engine answers floating");
    assert_eq!((engine.answer.as_str(), engine.p_yes), ("yes", 1.0), "{engine:?}");
    assert!(engine.basis.contains("lifted"), "{}", engine.basis);

    // **The checks the engine cannot answer carry none**, and the ones it can say what they measured.
    let clipping = q.items.iter().find(|item| item.id == "check-clipping").expect("the clipping check");
    assert_eq!(clipping.engine.as_ref().expect("the engine answers clipping").answer, "no");
    assert!(q.items.iter().find(|item| item.id == "check-out_of_area").expect("the out-of-area check").engine.is_none());
    let blank = q.items.iter().find(|item| item.id == "check-blank_frame").expect("the blank-frame check");
    assert_eq!(blank.engine.as_ref().expect("the engine reads the picture").answer, "no", "the test's picture has more than one colour");
    assert_eq!(q.items.iter().filter(|item| item.source == "check").count(), NAMED_CHECKS.len());

    // **The turn question is last**, declarative, with no engine answer: its label is the person's.
    let last = q.items.last().expect("questions");
    assert_eq!(last.id, TURN_QUESTION_ID);
    assert_eq!(instructions(&last.question), "The person keeps this result");
    assert!(last.engine.is_none());
}

/// **1. A piece nobody lifted is not floating**, and the engine says so.
#[test]
fn a_piece_standing_where_it_should_is_not_floating() {
    let (turn, _app) = a_recorded_turn("questions_standing", None, &["a bed"]);
    let floating = turn.questions.items.iter().find(|item| item.id == "check-floating").expect("the floating check");
    assert_eq!(floating.engine.as_ref().expect("an engine answer").answer, "no");
}

/// **2. An empty or over-long specified line produces no question, and the record says why.**
#[test]
fn an_empty_or_over_long_line_is_skipped_whole_and_never_trimmed() {
    let long = "a long wall ".repeat(40);
    assert!(long.chars().count() > SPECIFIED_MAX_CHARS);
    let (turn, _app) = a_recorded_turn("questions_skipped", None, &["", &long, "a bed"]);
    let q = &turn.questions;
    let asked: Vec<&str> = q.items.iter().filter(|item| item.source == "specified").map(|item| item.id.as_str()).collect();
    assert_eq!(asked, ["specified-3"], "only the line that can be asked about is asked about");
    assert_eq!(q.skipped.len(), 2, "{:?}", q.skipped);
    assert_eq!(q.skipped[0].text, "");
    assert_eq!(q.skipped[1].text, long, "a line that is too long is named whole, never trimmed");
    assert!(q.skipped[1].why.contains("never trimmed"), "{}", q.skipped[1].why);
    assert!(!q.items.iter().any(|item| instructions(&item.question).contains("a long wall a long wall")), "the long line was made into a question");
}

/// **3. Changing the template version keeps old records' questions as they were.**
#[test]
fn a_new_template_changes_the_next_record_and_leaves_the_old_one() {
    let (old, _app) = a_recorded_turn("questions_versions", None, &["a bed"]);
    assert_eq!(old.questions.template_version, 1);
    let was = instructions(&old.questions.items.iter().find(|i| i.id == "specified-1").expect("asked").question).to_owned();
    assert_eq!(was, "The result satisfies: a bed");

    // A later build words it differently, under a new version.
    let later = Templates { version: 2, specified: "It holds that: {}", turn: "The person is happy with this result" };
    let specified = vec!["a bed".to_owned()];
    let new = questions_for(&Input { specified: &specified, checks: Err("not read in this test"), engine: &Engine::default(), decisions: &[] }, &later);
    assert_eq!(new.template_version, 2);
    assert_eq!(instructions(&new.items[0].question), "It holds that: a bed");

    // **The old record, read again, says what it said**: reading never rewrites a question.
    let labels_again = turn_text_of(&old);
    let reread: Turn = serde_json::from_str(&labels_again).expect("the record reads");
    assert_eq!(reread.questions.template_version, 1);
    assert_eq!(instructions(&reread.questions.items.iter().find(|i| i.id == "specified-1").expect("asked").question), was);
    assert_ne!(instructions(&new.items[0].question), was);
}

fn turn_text_of(turn: &Turn) -> String {
    serde_json::to_string(turn).expect("a turn serialises")
}

/// **4. The agent's own decision questions are kept verbatim.**
#[test]
fn the_agents_own_questions_are_kept_verbatim() {
    let (mut app, labels) = editor("questions_decided");
    let op = place(&mut app, [20.0, 18.0]);
    crate::a_turn_is_recorded_for_learning::render_what_is_asked(&mut app);
    let asked = serde_json::json!({"type": "noul", "instructions": "The east exit is clear"});
    let said = call(
        &mut app,
        "blueprint/decide_report",
        serde_json::json!({"params": {"questions": {"exit": asked}}, "answer": {"answers": {"exit": {"noul": 0.9}}}, "model": "clef-flash"}),
    );
    assert_eq!(said["recorded"], true);
    present(&mut app, &[op], &["a bed"]);
    until_written(&mut app);
    let read = ledger_of(&labels);
    let turn = read.turns().next().expect("a turn");
    let kept = turn.questions.items.iter().find(|item| item.source == "decision").expect("the agent's question");
    assert_eq!(kept.id, "decision-1-exit");
    assert_eq!(kept.question, asked, "verbatim");
}

/// **4. A flat picture is the engine's `blank_frame`.**
#[test]
fn a_flat_picture_is_a_blank_frame() {
    let (mut app, labels) = editor("questions_blank");
    let op = place(&mut app, [20.0, 18.0]);
    present(&mut app, &[op], &["a bed"]);
    for _ in 0..FRAMES_AT_MOST {
        app.update();
        // A renderer that draws nothing: one colour for every picture.
        let tickets = app.world().resource::<TurnLedger>().pending_shots();
        for ticket in tickets {
            let flat = ShotImage { width: 4, height: 4, rgba: vec![7; 4 * 4 * 4] };
            app.world_mut().resource_mut::<TurnLedger>().deliver_shot(ticket, Ok(flat));
        }
        if !app.world().resource::<TurnLedger>().written().is_empty() {
            break;
        }
    }
    let read = ledger_of(&labels);
    let turn = read.turns().next().expect("a turn");
    let blank = turn.questions.items.iter().find(|item| item.id == "check-blank_frame").expect("the blank-frame check");
    assert_eq!(blank.engine.as_ref().expect("an engine answer").answer, "yes");
    assert!(turn.after.image.as_ref().is_some_and(|image| image.flat));
}
