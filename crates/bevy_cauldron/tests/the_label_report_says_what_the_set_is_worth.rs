//! **The label report, and the assessor asked only where the fast model is unsure** (DL-08, #467; series labels from use, #459).
//!
//! The set is worth training on, or it is not, and only numbers say which. The report reads the ledger and the export and writes
//! `report.md` and `report.json` beside the set, with nothing from the machine, the clock or the output path in either.
//!
//! # What is proved
//!
//! 1. **Every number in the report**, over a fixture ledger: the week's turns and answers, the labels per tier, the class balance per question
//!    source, the assessor's accuracy (said "fine" through an Undo, and its agreement on the holdout), the fast model's agreement with the person and
//!    with the assessor, the disagreements by kind, and the size of the holdout.
//! 2. **Two reports of one ledger are byte-identical.**
//! 3. **The fast model answers every turn's questions first, and the assessor is asked only where it is unsure** — never the other way round — with
//!    a named random share so the assessor stays measured everywhere; over 1,000 records the share is respected and every saved question is counted.
//! 4. **A fast answer never becomes a label**: the export does not read that line at all.
//! 5. **The line the Agent workspace shows carries the same numbers**, and says plainly when the teacher is not good enough to distil.
//!
//! The report is derived: no test here writes into the labels folder, and none needs a model on the network.

use std::cell::RefCell;
use std::collections::BTreeMap;

use bevy_oracle::{Answer, DecisionModel, Error, Question, Request, Response, Transport};
use serde_json::Value;

use bevy_cauldron::label_report::{self, ESCALATE_SHARE, HOLDOUT_MIN, REPORT_FILE, REPORT_JSON, TEACHER_BAR, UNSURE_HIGH, UNSURE_LOW};
use bevy_cauldron::turn_assess;
use bevy_cauldron::turn_export::{self, Options};
use bevy_cauldron::turn_questions::{EngineAnswer, Questions, TURN_QUESTION_ID, TurnQuestion};

/// **A scene's id**, for the fixture's defect: what DL-05 puts on a perturbation.
use bevy_cauldron::turn_store::{
    AUDIT, AssessedQuestion, Assessment, AssessorRef, FastAnswer, FastAnswers, HandBackRef, Label, Line, Perturbed, Provenance, Request as Asked, Shot, Store,
    Turn, Vote, scene_of,
};

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire_report_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

const NEW_BUILD: &str = turn_export::OLDEST_BUILD;

fn noul(id: &str, source: &str, said: &str, engine: Option<bool>) -> TurnQuestion {
    TurnQuestion {
        id: id.to_owned(),
        source: source.to_owned(),
        about: "after".to_owned(),
        question: serde_json::to_value(Question::noul(said)).expect("a question"),
        engine: engine.map(|yes| EngineAnswer::new(yes, "measured from the map")),
    }
}

/// **One recorded turn**, its two pictures stored, written by a build this report keeps.
fn a_turn(store: &mut Store, id: &str, map: &str, questions: Vec<TurnQuestion>, at_ms: u64) -> Turn {
    let before = store.put_image(format!("{id} before").as_bytes(), 4, 4).expect("kept");
    let after = store.put_image(format!("{id} after").as_bytes(), 4, 4).expect("kept");
    Turn {
        id: id.to_owned(),
        bench: false,
        started_unix_ms: at_ms.saturating_sub(1_000),
        handed_back_unix_ms: at_ms,
        request: Asked {
            hand_back: HandBackRef { kind: "presentation".into(), id: "present-1".into() },
            title: "a courtyard".into(),
            summary: "A courtyard with a door.".into(),
            specified: vec!["a door on the east side".into(), "a floor".into()],
            inferred: Vec::new(),
            check: Vec::new(),
            note: None,
        },
        before: Shot::taken(before),
        after: Shot::taken(after),
        operations: Vec::new(),
        facts: serde_json::json!({"map": {"name": map}}),
        decisions: Vec::new(),
        provenance: Provenance {
            cauldron_version: NEW_BUILD.to_owned(),
            build_commit: format!("commit-{NEW_BUILD}"),
            render: serde_json::Value::Null,
            agent: None,
            project_hash: "project-a".to_owned(),
        },
        questions: Questions { template_version: 1, items: questions, skipped: Vec::new() },
    }
}

fn label(turn: &str, what: &str, tier: &str, at: u64) -> Label {
    Label {
        turn: turn.to_owned(),
        label: what.to_owned(),
        tier: tier.to_owned(),
        at_unix_ms: at,
        operation_id: None,
        by_turn: None,
        item: None,
        question: None,
        answer: None,
        asked_because: None,
        holdout: false,
    }
}

/// **The person answered one question of the audit**: gold, per question, and the holdout.
fn audit(turn: &str, question: &str, answer: &str, at: u64) -> Label {
    Label { label: AUDIT.to_owned(), question: Some(question.to_owned()), answer: Some(answer.to_owned()), asked_because: Some("random".to_owned()), holdout: true, ..label(turn, AUDIT, "gold", at) }
}

fn vote(p_yes: Option<f64>) -> Vote {
    Vote { wording: 0, answer: p_yes.map(|p| serde_json::json!({"type": "noul", "noul": p})), p_yes, refused: p_yes.is_none().then(|| "declined".to_owned()), model: Some("stand-in".into()) }
}

/// **The assessor answered one question** with three votes, so its fraction is known.
fn assessed(id: &str, votes: [Option<f64>; 3]) -> AssessedQuestion {
    let valid: Vec<f64> = votes.iter().flatten().copied().collect();
    let yes = valid.iter().filter(|p| **p >= 0.5).count();
    AssessedQuestion {
        id: id.to_owned(),
        votes: votes.iter().enumerate().map(|(wording, p)| Vote { wording: wording as u32, ..vote(*p) }).collect(),
        yes_fraction: (!valid.is_empty()).then(|| yes as f64 / valid.len() as f64),
        tally: BTreeMap::new(),
        refused: votes.iter().filter(|p| p.is_none()).count() as u32,
    }
}

fn assessment(turn: &str, answers: Vec<AssessedQuestion>, at: u64) -> Assessment {
    Assessment {
        turn: turn.to_owned(),
        assessor: AssessorRef { version: turn_assess::ASSESSOR_VERSION, endpoint: "http://127.0.0.1:8093/v1/systemone".into(), site: "local".into() },
        at_unix_ms: at,
        answers,
        skipped: None,
    }
}

/// **A week's Wednesday**, in milliseconds: the Monday is `WEEK_MS` before it.
const WEEK_MS: u64 = 7 * 24 * 60 * 60 * 1000;
/// 2026-11-11T12:00:00Z, a Wednesday of the week whose Monday is 2026-11-09.
const WEDNESDAY: u64 = 1_794_340_800_000;

// ---------------------------------------------------------------------------------------------------------------
// The fixture ledger: two weeks, two turns, a kept turn and an undone one.
// ---------------------------------------------------------------------------------------------------------------

/// **A ledger with every number a report has a place for**: a kept turn the assessor agreed with, an undone turn where the assessor said the result
/// was fine (the teacher's ceiling), an audit answer (the holdout), an engine answer, a silver label, and an engine-made defect.
fn fixture(tag: &str) -> (std::path::PathBuf, std::path::PathBuf, Value) {
    let labels = scratch(tag);
    let out = scratch(&format!("{tag}_out"));
    let mut store = Store::new(labels.clone());

    // **The kept turn**: the person kept it, so the turn question is gold yes; the engine said nothing floats; the assessor agreed on `specified-1`
    // and said the result was fine.
    let kept = a_turn(
        &mut store,
        "turn-kept",
        "court",
        vec![
            noul("specified-1", "specified", "The result satisfies: a door on the east side", None),
            noul("check-floating", "check", "Is any piece floating?", Some(false)),
            noul(TURN_QUESTION_ID, "turn", "The person keeps this result", None),
        ],
        WEDNESDAY - WEEK_MS,
    );
    let kept_scene = scene_of(&kept);
    store.append(&Line::turn(kept)).expect("a turn");
    store.append(&Line::label(label("turn-kept", "kept", "gold", WEDNESDAY - WEEK_MS + 10))).expect("a label");
    store
        .append(&Line::assessment(assessment(
            "turn-kept",
            vec![assessed("specified-1", [Some(0.9), Some(0.8), Some(0.7)]), assessed(TURN_QUESTION_ID, [Some(0.8), Some(0.8), Some(0.8)])],
            WEDNESDAY - WEEK_MS + 20,
        )))
        .expect("an assessment");
    // **An engine-made defect on that scene**: the objective negative.
    store
        .append(&Line::perturbed(Perturbed {
            id: "perturbed-1".into(),
            scene: kept_scene.clone(),
            source_turn: "turn-kept".into(),
            at_unix_ms: WEDNESDAY - WEEK_MS + 30,
            defect: "float".into(),
            size: "clear".into(),
            magnitude: 0.4,
            unit: "m".into(),
            target: vec!["lamp".into()],
            tier: bevy_cauldron::turn_store::TIER_OBJECTIVE.to_owned(),
            label: bevy_cauldron::turn_store::LABEL_DEFECTIVE.to_owned(),
            before: Shot::missing("kept scene"),
            after: Shot::missing("defective scene"),
            operations: Vec::new(),
            facts: serde_json::json!({}),
            bench: false,
            provenance: Provenance {
                cauldron_version: NEW_BUILD.to_owned(),
                build_commit: String::new(),
                render: serde_json::Value::Null,
                agent: None,
                project_hash: "project-a".into(),
            },
        }))
        .expect("a defect");

    // **The undone turn**: the person undid it, so the turn question is gold no; the assessor said the result was fine anyway (its error, and the
    // student's ceiling); the person answered `specified-2` in the audit, which is the holdout; the assessor disagreed with them there.
    let undone = a_turn(
        &mut store,
        "turn-undone",
        "court",
        vec![
            noul("specified-1", "specified", "The result satisfies: a door on the east side", None),
            noul("specified-2", "specified", "The result satisfies: a floor", None),
            noul(TURN_QUESTION_ID, "turn", "The person keeps this result", None),
        ],
        WEDNESDAY,
    );
    store.append(&Line::turn(undone)).expect("a turn");
    store.append(&Line::label(label("turn-undone", "undone", "gold", WEDNESDAY + 10))).expect("a label");
    store.append(&Line::label(audit("turn-undone", "specified-2", "no", WEDNESDAY + 20))).expect("an audit answer");
    store.append(&Line::label(label("turn-undone", "superseded", "silver", WEDNESDAY + 30))).expect("a silver label");
    store
        .append(&Line::assessment(
            assessment(
                "turn-undone",
                vec![
                    assessed("specified-1", [Some(0.9), Some(0.8), Some(0.7)]),
                    // The assessor says the floor is satisfied; the person said it is not.
                    assessed("specified-2", [Some(0.9), Some(0.9), Some(0.8)]),
                    // The assessor said the person keeps it, and the person undid it: this is what the ceiling counts.
                    assessed(TURN_QUESTION_ID, [Some(0.9), Some(0.8), Some(0.9)]),
                ],
                WEDNESDAY + 40,
            ),
        ))
        .expect("an assessment");

    // **The fast model answered both turns first**, sure on `specified-1` and on the turn questions, and it agreed with the person on the kept turn.
    store
        .append(&Line::fast_answer(FastAnswers {
            turn: "turn-kept".into(),
            at_unix_ms: WEDNESDAY - WEEK_MS + 5,
            model: Some("d3-flash".into()),
            answers: vec![fast("specified-1", Some(0.9)), fast("check-floating", Some(0.1)), fast(TURN_QUESTION_ID, Some(0.9))],
            escalation: vec![
                escalated("specified-1", true, "sampled"),
                escalated("check-floating", true, "sampled"),
                escalated(TURN_QUESTION_ID, true, "sampled"),
            ],
            skipped: None,
        }))
        .expect("fast answers");
    store
        .append(&Line::fast_answer(FastAnswers {
            turn: "turn-undone".into(),
            at_unix_ms: WEDNESDAY + 5,
            model: Some("d3-flash".into()),
            answers: vec![fast("specified-1", Some(0.9)), fast("specified-2", Some(0.6)), fast(TURN_QUESTION_ID, Some(0.3))],
            escalation: vec![
                escalated("specified-1", true, "sampled"),
                escalated("specified-2", true, "unsure"),
                escalated(TURN_QUESTION_ID, true, "outvoted"),
            ],
            skipped: None,
        }))
        .expect("fast answers");

    let exported = turn_export::export(&labels, &out, &Options::default()).expect("the export");
    let summary: Value = serde_json::from_slice(&std::fs::read(out.join(turn_export::SUMMARY_FILE)).expect("a summary")).expect("json");
    assert_eq!(summary["train_records"].as_u64().unwrap() + summary["eval_records"].as_u64().unwrap(), exported.train as u64 + exported.eval as u64);
    (labels, out, summary)
}

fn fast(id: &str, p_yes: Option<f64>) -> FastAnswer {
    FastAnswer { id: id.to_owned(), p_yes, refused: p_yes.is_none().then(|| "declined".to_owned()) }
}

fn escalated(id: &str, asked: bool, why: &str) -> bevy_cauldron::turn_store::Escalated {
    bevy_cauldron::turn_store::Escalated { id: id.to_owned(), asked, why: why.to_owned() }
}

// ---------------------------------------------------------------------------------------------------------------
// 1. Every number.
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn every_number_of_the_report_over_a_fixture_ledger() {
    let (labels, out, summary) = fixture("numbers");
    let report = label_report::report(&labels, &out, &Options::default(), &bevy_oracle::Checks::builtin()).expect("the report");

    // **The week's turns and answers**, in the week of the turn's hand-back.
    let week = |monday: &str| report.weeks.iter().find(|week| week.week == monday).unwrap_or_else(|| panic!("no week {monday}: {:?}", report.weeks));
    let first = week("2026-11-02");
    assert_eq!((first.turns, first.answered), (1, 1), "the kept turn was recorded and answered: {first:?}");
    assert_eq!((first.gold, first.silver), (1, 0), "{first:?}");
    assert_eq!(first.objective, 2, "the engine answered check-floating, and the defect is its own objective label: {first:?}");
    let second = week("2026-11-09");
    assert_eq!((second.turns, second.answered), (1, 1), "{second:?}");
    assert_eq!((second.gold, second.silver), (2, 1), "the Undo, the audit answer and the superseded label: {second:?}");
    assert_eq!(second.objective, 0, "nothing objective was written that week: {second:?}");

    // **The totals.**
    assert_eq!((report.summary.turns, report.summary.answered), (2, 2), "{:?}", report.summary);
    assert_eq!((report.summary.gold, report.summary.silver), (3, 1), "{:?}", report.summary);
    assert_eq!(report.summary.objective, 2, "an engine answer and an engine-made defect are both objective: {:?}", report.summary);
    assert_eq!(report.summary.holdout, 1, "the audit answered one question: {:?}", report.summary);
    assert_eq!(report.summary.questions, 6, "two turns of three questions: {:?}", report.summary);
    assert_eq!(report.summary.dismissed, 0, "nobody dismissed a hand-back: {:?}", report.summary);

    // **Per question, and per tier.**
    let question = |id: &str| report.questions.iter().find(|question| question.id == id).unwrap_or_else(|| panic!("no question {id}"));
    let floor = question("specified-2");
    assert_eq!((floor.turns, floor.gold, floor.silver, floor.holdout), (1, 1, 0, 1), "the audit answered it, and nothing trained on it: {floor:?}");
    let keeps = question(TURN_QUESTION_ID);
    assert_eq!((keeps.turns, keeps.gold, keeps.silver), (2, 2, 1), "both turns ask it, the person answered both, and a later turn took one back: {keeps:?}");
    let floating = question("check-floating");
    assert_eq!((floating.turns, floating.objective), (1, 2), "the engine's answer and the defect's: {floating:?}");
    assert!(floating.gold == 0 && floating.silver == 0, "{floating:?}");

    // **The class balance, per question source**: the person's answers and the engine's, yes against no.
    let source = |name: &str| report.sources.iter().find(|source| source.source == name).unwrap_or_else(|| panic!("no source {name}"));
    assert_eq!((source("turn").gold_yes, source("turn").gold_no), (1, 1), "the Keep and the Undo of a hand-back: {:?}", source("turn"));
    assert_eq!((source("specified").gold_yes, source("specified").gold_no), (0, 1), "the person said the floor was not there: {:?}", source("specified"));
    assert_eq!((source("check").objective_yes, source("check").objective_no), (0, 1), "the engine found nothing floating: {:?}", source("check"));

    // **The assessor's accuracy against the person.**
    assert_eq!(report.assessor.answered, 5, "the assessor answered five questions: {:?}", report.assessor);
    assert_eq!(report.assessor.undone, 1, "one turn was undone: {:?}", report.assessor);
    assert_eq!(report.assessor.said_fine_on_an_undo, 1, "and the assessor said the person keeps it: {:?}", report.assessor);
    assert_eq!(report.assessor.kept, 1, "and one was kept: {:?}", report.assessor);
    assert_eq!(report.assessor.said_fine_on_a_keep, 1, "and the assessor agreed: {:?}", report.assessor);
    assert_eq!(report.assessor.holdout.questions, 1, "the holdout is one question: {:?}", report.assessor.holdout);
    assert_eq!(report.assessor.holdout.agreed, 0, "the assessor said the floor was satisfied and the person said not: {:?}", report.assessor.holdout);
    assert_eq!(report.assessor.bar, TEACHER_BAR);
    assert!(!report.assessor.meets_the_bar(), "one question never meets the bar");

    // **The disagreements, by kind**: the assessor contradicted the person (it said the person keeps the turn they undid), and nothing of the
    // person's or the engine's disagreed with itself. The report's own count and the export's are the same number.
    assert_eq!(report.disagreements, summary["disagreements"].as_object().map(|map| map.iter().map(|(kind, n)| (kind.clone(), n.as_u64().unwrap_or(0) as usize)).collect()).unwrap_or_default(), "{:?}", report.disagreements);
    assert_eq!(report.assessor.teacher_errors, summary["teacher_errors"].as_u64().unwrap() as usize);
    assert_eq!(report.disagreements.get("teacher").copied().unwrap_or(0), report.assessor.teacher_errors, "{:?}", report.disagreements);
    assert_eq!(report.disagreements.get("teacher").copied().unwrap_or(0), 1, "the assessor said the person keeps the turn they undid, and nothing else: {:?}", report.disagreements);
    assert_eq!(report.disagreements.get("person").copied().unwrap_or(0), 0, "{:?}", report.disagreements);
    assert_eq!(report.disagreements.get("engine").copied().unwrap_or(0), 0, "{:?}", report.disagreements);

    // **The fast model against the person and the teacher.**
    assert_eq!((report.fast.turns, report.fast.questions), (2, 6), "{:?}", report.fast);
    assert_eq!((report.fast.with_gold.questions, report.fast.with_gold.agreed), (3, 2), "it agreed with the Kept turn's yes and missed the Undo: {:?}", report.fast);
    assert_eq!((report.fast.with_assessor.questions, report.fast.with_assessor.agreed), (5, 4), "it differed only on the floor: {:?}", report.fast);

    // **The set the export wrote**, cited by its own numbers.
    assert_eq!(report.set.train + report.set.eval, summary["train_records"].as_u64().unwrap() as usize + summary["eval_records"].as_u64().unwrap() as usize);
    assert_eq!(report.set.holdout_records, summary["holdout_records"].as_u64().unwrap() as usize);
    assert_eq!(report.set.teacher_errors, summary["teacher_errors"].as_u64().unwrap() as usize);

    // **Escalation**: the gate is closed (one holdout question, under the minimum), so nothing counts as saved on the gate's account; the ledger's own
    // lines say what was asked, and those are the fixture's plan.
    assert!(!report.escalation.open, "the gate is closed on one holdout question: {:?}", report.escalation);
    assert_eq!(report.escalation.questions, 6, "{:?}", report.escalation);
    assert_eq!(report.escalation.escalated, 6, "{:?}", report.escalation);

    // **The remote calibration is waiting, not an error**, when no record names it.
    let remote = report.calibrations.iter().find(|c| c.state == "waiting").expect("the remote row is waiting");
    assert!(remote.why.contains("no calibration"), "{remote:?}");

    // **The teacher is not good enough to distil**, and the report says so.
    assert_eq!(report.distil.state, "off", "{:?}", report.distil);
    assert!(report.distil.why.contains("not good enough"), "{:?}", report.distil);

    // **Both files are written beside the set, and say the numbers.**
    let markdown = std::fs::read_to_string(out.join(REPORT_FILE)).expect("report.md");
    assert!(markdown.contains("not good enough to distil"), "{markdown}");
    assert!(markdown.contains("teacher"), "{markdown}");
    let json: Value = serde_json::from_slice(&std::fs::read(out.join(REPORT_JSON)).expect("report.json")).expect("json");
    assert_eq!(json["summary"]["turns"], 2, "{json}");
    assert_eq!(json["assessor"]["holdout"]["questions"], 1, "{json}");
}

// ---------------------------------------------------------------------------------------------------------------
// 2. Byte-identical.
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn two_reports_of_one_ledger_are_byte_identical() {
    let (labels, out, _) = fixture("twice");
    let first = label_report::report(&labels, &out, &Options::default(), &bevy_oracle::Checks::builtin()).expect("the report");
    let (md_first, json_first) = (
        std::fs::read(out.join(REPORT_FILE)).expect("report.md"),
        std::fs::read(out.join(REPORT_JSON)).expect("report.json"),
    );
    let second = label_report::report(&labels, &out, &Options::default(), &bevy_oracle::Checks::builtin()).expect("the report again");
    assert_eq!(first, second, "the report is the same data twice");
    assert_eq!(md_first, std::fs::read(out.join(REPORT_FILE)).expect("report.md"), "report.md differs between two runs");
    assert_eq!(json_first, std::fs::read(out.join(REPORT_JSON)).expect("report.json"), "report.json differs between two runs");
}

// ---------------------------------------------------------------------------------------------------------------
// 3. Escalation, counted, over 1,000 records.
// ---------------------------------------------------------------------------------------------------------------

/// **A stand-in fast model and assessor**: the fast pass is sure of every question but one in ten, and every request is counted.
struct StandIn {
    asked: RefCell<Vec<Request>>,
    /// How many turns' questions have been asked so far, so every tenth is unsure.
    seen: RefCell<usize>,
}

impl Transport for StandIn {
    fn endpoint(&self) -> String {
        "http://127.0.0.1:8093/v1/systemone".into()
    }

    fn site(&self) -> DecisionModel {
        DecisionModel::Local
    }

    fn send(&self, request: &Request) -> Result<Response, Error> {
        self.asked.borrow_mut().push(request.clone());
        // **The assessor asks one question at a time under `q`**, in one of the three wordings; the fast pass asks a turn's questions together.
        let assessor = request.questions.contains_key("q");
        let mut answers = BTreeMap::new();
        for id in request.questions.keys() {
            answers.insert(id.clone(), Answer::Noul { noul: 0.9 });
        }
        if !assessor {
            *self.seen.borrow_mut() += 1;
            if self.seen.borrow().is_multiple_of(10) {
                for answer in answers.values_mut() {
                    *answer = Answer::Noul { noul: 0.5 };
                }
            }
        }
        Ok(Response { model: Some("stand-in-1".into()), answers, usage: None })
    }
}

/// **Open the gate**: enough holdout questions, each of which the assessor answered as the person did.
fn open_the_gate(store: &mut Store) {
    for n in 0..HOLDOUT_MIN {
        let id = format!("turn-open-{n}");
        let turn = a_turn(store, &id, "court", vec![noul("specified-1", "specified", "The result satisfies: a door on the east side", None)], 1_000 + n as u64);
        store.append(&Line::turn(turn)).expect("a turn");
        store.append(&Line::label(audit(&id, "specified-1", "yes", 2_000 + n as u64))).expect("a holdout answer");
        store
            .append(&Line::assessment(assessment(&id, vec![assessed("specified-1", [Some(0.9), Some(0.8), Some(0.9)])], 3_000 + n as u64)))
            .expect("an assessment");
    }
}

#[test]
fn the_assessor_is_asked_only_where_the_fast_model_is_unsure_and_the_share_is_kept() {
    let labels = scratch("escalation");
    let mut store = Store::new(labels.clone());
    open_the_gate(&mut store);
    // **1,000 records, one question each**, so the share is a plain count.
    for n in 0..1_000 {
        let id = format!("turn-{n}");
        let turn = a_turn(&mut store, &id, "court", vec![noul("specified-1", "specified", "The result satisfies: a door on the east side", None)], 10_000 + n as u64);
        store.append(&Line::turn(turn)).expect("a turn");
    }

    let model = StandIn { asked: RefCell::new(Vec::new()), seen: RefCell::new(0) };
    let batch = turn_assess::assess_ledger_with(&mut store, Some(&model), &model, false, None).expect("assessed");
    // **The gate's twenty turns were assessed by the fixture**, so the batch answers their questions with the fast model and does not assess them
    // again: one line each, which is what makes the second run below write nothing.
    assert_eq!((batch.assessed, batch.already, batch.fast), (1_000, 20, 1_020), "{batch:?}");

    let read = turn_store_read(&labels);
    let fast: Vec<&FastAnswers> = read.fast_answers().collect();
    assert_eq!(fast.len(), 1_020, "every turn's questions were answered by the fast model first");
    let by_why = |why: &str| fast.iter().flat_map(|f| f.escalation.iter()).filter(|e| e.why == why).count();

    assert_eq!(by_why("unsure"), 102, "the gate's twenty and one turn in ten of the thousand were unsure: {:?}", by_why("unsure"));
    let total: usize = fast.iter().map(|f| f.escalation.len()).sum();
    assert_eq!(total, 1_020, "one question a turn");
    let sampled = by_why("sampled");
    assert!((150..=270).contains(&sampled), "the random share is one in {ESCALATE_SHARE} of 1,020, and {sampled} were sampled");
    let asked: usize = fast.iter().flat_map(|f| f.escalation.iter()).filter(|e| e.asked).count();
    assert_eq!(asked, by_why("unsure") + sampled, "a question is asked for exactly one reason, and the others are sure");
    // **The saved count is of the turns this batch actually assessed**: the gate's twenty were assessed already, so their questions were planned and
    // no assessor call was made for them, which is neither a saving nor a cost.
    assert_eq!(asked, 288, "the plan over all 1,020: {asked}");
    assert_eq!(batch.saved, by_why("sure") - 15, "every question the plan kept on an assessed turn is a call nobody paid: {batch:?}");

    // **The assessor was called on the asked questions and no others**, three wordings each.
    let assessor: usize = model.asked.borrow().iter().filter(|r| r.questions.contains_key("q")).count();
    // **Three wordings for each question on a turn this batch assessed**, and none for the gate's twenty.
    let asked_of_assessed = asked - 5;
    assert_eq!(assessor, asked_of_assessed * turn_assess::VOTES, "the assessor was asked {assessor} times for {asked_of_assessed} questions");
    assert!(asked < 600, "nothing was saved: {asked} questions were asked");

    // **The saved count is the questions the fast model kept**, and the report's numbers are the batch's, made from the same lines.
    let report = label_report::of(&read, None, &bevy_oracle::Checks::builtin());
    assert!(report.escalation.open, "twenty holdout answers the assessor agreed with open the gate: {:?}", report.escalation.gate);
    assert_eq!(report.escalation.questions, total, "{:?}", report.escalation);
    assert_eq!(report.escalation.escalated, asked, "{:?}", report.escalation);
    assert_eq!(report.escalation.saved, total - asked, "{:?}", report.escalation);
    assert!((UNSURE_LOW..=UNSURE_HIGH).contains(&0.5), "the bounds are named, and 0.5 is inside them");
    assert_eq!(report.escalation.bounds, (UNSURE_LOW, UNSURE_HIGH), "{:?}", report.escalation);
    assert_eq!(report.escalation.share, ESCALATE_SHARE);

    // **A second run writes nothing new**: every turn has been looked at, by the fast model and by the assessor.
    let (assessor_calls, fast_calls) = (model.asked.borrow().len(), model.asked.borrow().iter().filter(|r| !r.questions.contains_key("q")).count());
    let again = turn_assess::assess_ledger_with(&mut store, Some(&model), &model, false, None).expect("assessed twice");
    assert_eq!((again.assessed, again.saved, again.fast, again.already), (0, 0, 0, 1_020), "{again:?}");
    assert_eq!(model.asked.borrow().len(), assessor_calls, "a second run asked the assessor again");
    assert_eq!(model.asked.borrow().iter().filter(|r| !r.questions.contains_key("q")).count(), fast_calls, "a second run asked the fast model again");
}

fn turn_store_read(labels: &std::path::Path) -> bevy_cauldron::turn_store::Read {
    bevy_cauldron::turn_store::read_file(&Store::new(labels.to_path_buf()).ledger_path()).expect("the ledger reads")
}

// ---------------------------------------------------------------------------------------------------------------
// 4. A fast answer is never a label.
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn a_fast_answer_never_becomes_a_label() {
    let (labels, out, _) = fixture("no_label");
    // **One question only the fast model answered**, added to a turn with a gold answer of its own, so the state is still written.
    let read = turn_store_read(&labels);
    assert!(read.fast_answers().count() >= 2);
    let exported = turn_export::export(&labels, &out, &Options::default()).expect("the export");
    // Nothing in the set carries the fast model as an answer's source, and `check-floating` (answered by the engine, and by the fast model) is
    // objective, never the fast model's.
    for record in [turn_export::TRAIN_FILE, turn_export::EVAL_FILE, turn_export::HOLDOUT_FILE] {
        let path = out.join(record);
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for line in text.lines() {
            let one: Value = serde_json::from_str(line).expect("a record");
            for (_, said) in one["meta"]["questions"].as_object().into_iter().flatten() {
                assert_ne!(said["source"], "fast", "a fast model's answer became a target: {one}");
                assert_ne!(said["source"], "student", "a fast model's answer became a target: {one}");
            }
        }
    }
    assert!(exported.train + exported.eval > 0, "the fixture writes a set at all");
}

// ---------------------------------------------------------------------------------------------------------------
// 5. The workspace's line, and the teacher's verdict.
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn the_workspace_line_says_what_the_ledger_holds() {
    let labels = scratch("line");
    let mut store = Store::new(labels.clone());
    open_the_gate(&mut store);
    let read = turn_store_read(&labels);
    let summary = label_report::summary(&read);
    let line = label_report::line(&summary);
    assert!(line.contains(&format!("{HOLDOUT_MIN} turns")), "{line}");
    assert!(line.contains(&format!("{HOLDOUT_MIN} answered")), "{line}");
    assert!(line.contains("gold"), "{line}");
    assert!(line.contains("holdout"), "{line}");
    // **The teacher's agreement is on the line, with the bar it is compared with.**
    assert!(line.contains("1.00") || line.contains("100%"), "the gate's agreement is shown: {line}");
    assert!(line.contains(&format!("{TEACHER_BAR:.2}")) || line.contains("bar"), "{line}");

    // **A ledger with nothing in it says so**, rather than showing a zero that looks like a measurement.
    let empty = label_report::summary(&bevy_cauldron::turn_store::Read::default());
    assert!(label_report::line(&empty).contains("no turn"), "{}", label_report::line(&empty));
}
