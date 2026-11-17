//! **A fresh-context model grades each turn's questions, three times each, and a vote that is not an option is a refusal** (DL-04, #463; series labels
//! from use, #459).
//!
//! The assessor reads the ledger and appends `assessment` lines. Here it is a stand-in model that replays fixed answers, so the votes are known.
//!
//! # What is proved
//!
//! 1. **Each question is asked three times, in three wordings, each as a request of its own**; the fraction of yes votes is stored; an answer that is
//!    not one of the options offered, and a provider's refusal, are refused votes and are never counted as a yes or a no.
//! 2. **A turn is assessed once per assessor version**: a second run skips it, an older version's assessment does not stop a new one.
//! 3. **Remote needs the person's say**: the pictures would leave the machine, so nothing is asked without it; a model that cannot be reached stops the
//!    batch and writes nothing.
//! 4. **The record says where the answers came from, without a key in it**, and a turn with no picture is recorded as skipped.
//! 5. **The context is the request and the pictures only**: the agent's own conversation is not in the request.

use std::cell::RefCell;
use std::collections::BTreeMap;

use bevy_oracle::{Answer, DecisionModel, Error, Question, Request, Response, Transport};

use bevy_dark_factory::turn_assess::{ASSESSOR_VERSION, VOTES, assess_ledger};
use bevy_dark_factory::turn_questions::{Questions, TurnQuestion};
use bevy_dark_factory::turn_store::{self, Assessment, AssessorRef, HandBackRef, Line, Provenance, Request as Asked, Shot, Store, Turn};

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("blueprint_assess_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn noul(question: &str, text: &str) -> TurnQuestion {
    TurnQuestion {
        id: question.to_owned(),
        source: "specified".to_owned(),
        about: "after".to_owned(),
        question: serde_json::to_value(Question::noul(text)).expect("a question"),
        engine: None,
    }
}

/// **A turn with two pictures in the store and three questions**: two yes/no and one choice.
fn a_turn(store: &mut Store, id: &str, pictures: bool) -> Turn {
    let (before, after) = if pictures {
        let before = store.put_image(b"before picture", 4, 4).expect("kept");
        let after = store.put_image(b"after picture", 4, 4).expect("kept");
        (Shot::taken(before), Shot::taken(after))
    } else {
        (Shot::missing("none"), Shot::missing("none"))
    };
    let choice = Question::choice("Which side is the door on?", [("east", "the east wall"), ("west", "the west wall")]);
    Turn {
        id: id.to_owned(),
        bench: false,
        started_unix_ms: 1,
        handed_back_unix_ms: 2,
        request: Asked {
            hand_back: HandBackRef { kind: "presentation".into(), id: "present-1".into() },
            title: "a courtyard".into(),
            summary: "A courtyard with a door.".into(),
            specified: vec!["a door on the east side".into(), "a floor".into()],
            inferred: Vec::new(),
            check: Vec::new(),
            note: None,
        },
        before,
        after,
        operations: Vec::new(),
        facts: serde_json::Value::Null,
        decisions: Vec::new(),
        provenance: Provenance { dark_factory_version: "0".into(), build_commit: "unknown".into(), render: serde_json::Value::Null, agent: None, project_hash: String::new() },
        questions: Questions {
            template_version: 1,
            items: vec![
                noul("specified-1", "The result satisfies: a door on the east side"),
                noul("specified-2", "The result satisfies: a floor"),
                TurnQuestion {
                    id: "decision-1-side".into(),
                    source: "decision".into(),
                    about: "after".into(),
                    question: serde_json::to_value(choice).expect("a question"),
                    engine: None,
                },
            ],
            skipped: Vec::new(),
        },
    }
}

/// **A stand-in model that replays fixed answers**, and keeps every request it was sent.
struct Stub {
    site: DecisionModel,
    endpoint: String,
    unreachable: bool,
    asked: RefCell<Vec<Request>>,
}

impl Stub {
    fn local() -> Stub {
        Stub { site: DecisionModel::Local, endpoint: "http://127.0.0.1:8093/v1/systemone".into(), unreachable: false, asked: RefCell::new(Vec::new()) }
    }
}

/// Which of the three wordings a question was asked in.
fn wording_of(instructions: &str) -> usize {
    if instructions.starts_with("Look for what is wrong first") {
        1
    } else if instructions.starts_with("Judge strictly") {
        2
    } else {
        0
    }
}

impl Transport for Stub {
    fn endpoint(&self) -> String {
        self.endpoint.clone()
    }

    fn site(&self) -> DecisionModel {
        self.site
    }

    fn send(&self, request: &Request) -> Result<Response, Error> {
        self.asked.borrow_mut().push(request.clone());
        if self.unreachable {
            return Err(Error::Unreachable { endpoint: self.endpoint.clone(), detail: "no route".into() });
        }
        let answer = match request.questions.get("q").expect("one question") {
            Question::Noul { instructions } if instructions.contains("a door on the east side") => {
                // yes, yes, no: two thirds.
                Answer::Noul { noul: [0.9, 0.8, 0.1][wording_of(instructions)] }
            }
            // The provider declines the second wording of the floor question, and answers 1.7 (not a probability) to the third.
            Question::Noul { instructions } => match wording_of(instructions) {
                0 => Answer::Noul { noul: 0.2 },
                1 => Answer::Refusal,
                _ => Answer::Noul { noul: 1.7 },
            },
            // **An option that was never offered**: the model says `north`.
            Question::Choice { .. } => Answer::Choice { choice: "north".into(), probabilities: BTreeMap::from([("north".to_owned(), 1.0)]), confidence: 1.0 },
            Question::Score { .. } => unreachable!("no score question is asked here"),
        };
        Ok(Response { model: Some("stand-in-1".into()), answers: BTreeMap::from([("q".to_owned(), answer)]), usage: None })
    }
}

fn assessments(store: &Store) -> Vec<Assessment> {
    turn_store::read_file(&store.ledger_path()).expect("reads").assessments().cloned().collect()
}

/// **1. Three votes a question, the fraction kept, an off-option answer refused.**
#[test]
fn each_question_is_asked_three_times_and_an_off_option_answer_is_a_refusal() {
    let mut store = Store::new(scratch("votes"));
    let turn = a_turn(&mut store, "turn-1", true);
    store.append(&Line::turn(turn)).expect("a turn");
    let model = Stub::local();
    let batch = assess_ledger(&mut store, &model, false, None).expect("assessed");
    assert_eq!((batch.assessed, batch.already), (1, 0));

    // **Each question went out three times, each alone, with the request's words and the two pictures, and nothing of the agent's conversation.**
    let asked = model.asked.borrow();
    assert_eq!(asked.len(), 3 * VOTES, "three questions, three wordings each");
    for request in asked.iter() {
        assert_eq!(request.questions.len(), 1, "a question is asked alone");
        assert_eq!(request.images.len(), 2, "the before and the after");
        assert!(request.state.contains("a courtyard") && request.state.contains("a door on the east side"), "{}", request.state);
        assert!(request.state.contains("WRONG"), "the prompt looks for defects: {}", request.state);
    }
    let wordings: std::collections::BTreeSet<usize> = asked
        .iter()
        .map(|r| match r.questions.get("q") {
            Some(Question::Noul { instructions }) | Some(Question::Choice { instructions, .. }) => wording_of(instructions),
            _ => 99,
        })
        .collect();
    assert_eq!(wordings, [0, 1, 2].into(), "all three wordings were used");

    let [assessment] = assessments(&store).try_into().expect("one assessment line");
    assert_eq!(assessment.turn, "turn-1");
    assert_eq!(assessment.assessor.version, ASSESSOR_VERSION);
    let door = &assessment.answers[0];
    assert_eq!(door.votes.len(), VOTES);
    assert!((door.yes_fraction.expect("a fraction") - 2.0 / 3.0).abs() < 1e-9, "{door:?}");
    assert_eq!(door.refused, 0);

    // **A refusal and an impossible probability are refused votes**, and the fraction is of the votes that count.
    let floor = &assessment.answers[1];
    assert_eq!(floor.refused, 2, "{floor:?}");
    assert_eq!(floor.yes_fraction, Some(0.0), "one valid vote, and it said no");
    assert!(floor.votes[1].refused.is_some() && floor.votes[1].p_yes.is_none(), "{:?}", floor.votes[1]);
    assert!(floor.votes[2].refused.as_deref().is_some_and(|why| why.contains("outside 0..1")), "{:?}", floor.votes[2]);

    // **An option nobody offered is a refusal, never one of the options.**
    let side = &assessment.answers[2];
    assert_eq!(side.refused as usize, VOTES, "{side:?}");
    assert!(side.tally.is_empty(), "an off-option answer was counted: {:?}", side.tally);
    assert!(side.votes.iter().all(|v| v.answer.is_none() && v.refused.is_some()));
    assert_eq!(door.votes[0].model.as_deref(), Some("stand-in-1"), "the model that answered is named");
}

/// **2. A turn is assessed once per assessor version.**
#[test]
fn a_turn_is_assessed_once_per_version_and_a_new_version_assesses_it_again() {
    let mut store = Store::new(scratch("versions"));
    let turn = a_turn(&mut store, "turn-1", true);
    store.append(&Line::turn(turn)).expect("a turn");
    // An older assessor's assessment is in the ledger already.
    store
        .append(&Line::assessment(Assessment {
            turn: "turn-1".into(),
            assessor: AssessorRef { version: ASSESSOR_VERSION - 1, endpoint: "old".into(), site: "local".into() },
            at_unix_ms: 1,
            answers: Vec::new(),
            skipped: None,
        }))
        .expect("an older assessment");

    let model = Stub::local();
    let first = assess_ledger(&mut store, &model, false, None).expect("assessed");
    assert_eq!((first.assessed, first.already), (1, 0), "an older version does not stop this one");
    let asked_once = model.asked.borrow().len();
    assert!(asked_once > 0);

    let second = assess_ledger(&mut store, &model, false, None).expect("assessed");
    assert_eq!((second.assessed, second.already), (0, 1), "this version has assessed it");
    assert_eq!(model.asked.borrow().len(), asked_once, "nothing was asked again");
    assert_eq!(assessments(&store).len(), 2, "the older line and this version's, never rewritten");
}

/// **3. Remote needs the person's say, and a model that cannot be reached stops the batch.**
#[test]
fn remote_needs_the_persons_say_and_an_unreachable_model_stops_the_batch() {
    let mut store = Store::new(scratch("remote"));
    let turn = a_turn(&mut store, "turn-1", true);
    store.append(&Line::turn(turn)).expect("a turn");

    let remote = Stub { site: DecisionModel::Remote, endpoint: "https://openrouter.example/api".into(), unreachable: false, asked: RefCell::new(Vec::new()) };
    let why = assess_ledger(&mut store, &remote, false, None).expect_err("Remote without leave");
    assert!(why.contains("--allow-remote") && why.contains("off this machine"), "{why}");
    assert!(remote.asked.borrow().is_empty(), "a picture was sent without the person's say");
    assert!(assessments(&store).is_empty());
    assess_ledger(&mut store, &remote, true, None).expect("with leave it goes");
    assert!(!remote.asked.borrow().is_empty());

    let mut store = Store::new(scratch("unreachable"));
    let turn = a_turn(&mut store, "turn-1", true);
    store.append(&Line::turn(turn)).expect("a turn");
    let down = Stub { unreachable: true, ..Stub::local() };
    let why = assess_ledger(&mut store, &down, false, None).expect_err("unreachable");
    assert!(why.contains("did not answer"), "{why}");
    assert!(assessments(&store).is_empty(), "a guess was written for a model that could not be asked");
}

/// **4. The record names where the answers came from with no key in the address, and a turn with no picture is skipped with the reason.**
#[test]
fn the_record_names_its_source_without_a_key_and_skips_a_turn_with_no_picture() {
    let mut store = Store::new(scratch("source"));
    let with = a_turn(&mut store, "turn-1", true);
    let without = a_turn(&mut store, "turn-2", false);
    store.append(&Line::turn(with)).expect("a turn");
    store.append(&Line::turn(without)).expect("a turn");
    let model = Stub { endpoint: "https://user:secret@api.example.com/v1/decide?key=abc#x".into(), ..Stub::local() };
    let batch = assess_ledger(&mut store, &model, false, None).expect("assessed");
    assert_eq!(batch.assessed, 2);
    let all = assessments(&store);
    assert_eq!(all[0].assessor.endpoint, "https://api.example.com/v1/decide");
    assert_eq!(all[0].assessor.site, "local");
    assert!(all[1].skipped.as_deref().is_some_and(|why| why.contains("no picture")), "{:?}", all[1]);
    assert!(all[1].answers.is_empty());
    let text = std::fs::read_to_string(store.ledger_path()).expect("the ledger");
    assert!(!text.contains("secret") && !text.contains("abc#x"), "a key reached the ledger");
}

/// **A model that takes its time**, and is the same for every thread.
struct Slow;

impl Transport for Slow {
    fn endpoint(&self) -> String {
        "http://127.0.0.1:8093/v1/systemone".into()
    }

    fn site(&self) -> DecisionModel {
        DecisionModel::Local
    }

    fn send(&self, request: &Request) -> Result<Response, Error> {
        std::thread::sleep(std::time::Duration::from_millis(15));
        let answer = match request.questions.get("q").expect("one question") {
            Question::Noul { .. } => Answer::Noul { noul: 0.9 },
            _ => Answer::Refusal,
        };
        Ok(Response { model: Some("slow-1".into()), answers: BTreeMap::from([("q".to_owned(), answer)]), usage: None })
    }
}

/// How long one frame of the editor may take while a batch runs, in milliseconds: the budget `opening_a_project_never_holds_a_frame_past_the_budget` holds.
const FRAME_BUDGET_MS: f64 = 100.0;

/// **6. A batch is outside the editor's frame**: it is a process and a thread of its own, so the editor's frames stay inside the UI budget while it runs.
#[test]
fn the_editors_frames_stay_in_budget_while_a_batch_runs() {
    let mut store = Store::new(scratch("budget"));
    for n in 0..6 {
        let turn = a_turn(&mut store, &format!("turn-{n}"), true);
        store.append(&Line::turn(turn)).expect("a turn");
    }
    let (mut app, _labels) = crate::a_turn_is_recorded_for_learning::editor("assess_budget");
    let batch = std::thread::spawn(move || assess_ledger(&mut store, &Slow, false, None).map(|b| (b.assessed, store)));

    let mut longest = 0.0_f64;
    let mut frames = 0;
    while !batch.is_finished() {
        let started = std::time::Instant::now();
        app.update();
        longest = longest.max(started.elapsed().as_secs_f64() * 1000.0);
        frames += 1;
    }
    let (assessed, _store) = batch.join().expect("the batch thread").expect("the batch");
    assert_eq!(assessed, 6, "the batch did not finish");
    assert!(frames >= 10, "the batch finished before the editor drew {frames} frames: the measurement means nothing");
    assert!(longest <= FRAME_BUDGET_MS, "a frame took {longest:.0} ms while a batch ran, over the {FRAME_BUDGET_MS} ms budget");
}
