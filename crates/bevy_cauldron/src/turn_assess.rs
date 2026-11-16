//! **The assessor: a fresh-context model grades each turn's questions** (DL-04, #463; series labels from use, #459).
//!
//! A turn's record (DL-01) holds typed questions (DL-03). The person's Keep and Undo label the whole turn; this answers the questions, per question,
//! from the before and after pictures and the request, so a student can later be graded and trained question by question. Its answers are
//! **silver**: a teacher's, to be used where its votes agree (DL-06), and graded against the person's own answers (DL-07, DL-08).
//!
//! # What it is, and what it is not
//!
//! - **Never the agent that made the edit, and never in its conversation.** Each question goes out as a request of its own, with the request's words
//!   and the pictures as the only context. The model is the person's *decision-model choice* ([`crate::decision_model::Source`]): Local or Remote, as
//!   Settings chooses, **with no fallback to the other** (a choice that cannot be asked is refused with its reason). Nothing here picks a model.
//! - **It runs out of the frame loop**, in its own process (`cauldron-labels assess`), by hand. It never blocks the editor, which it does not touch:
//!   it reads the ledger and appends to it, and the ledger's one-write appends make that safe beside a running editor.
//! - **It looks for defects, not success.** The prompt asks what is wrong with the result against the request, then the question.
//! - **Each question is asked three times, in three wordings**, and the vote fraction is kept as a soft label: a model's stated confidence is poorly
//!   calibrated, its agreement with itself is a better signal. The wordings and the prompt are versioned ([`ASSESSOR_VERSION`]) and every
//!   record names the version.
//! - **An answer is typed or it is a refusal.** A provider's refusal, an answer of another type and a choice that is not an option offered are each
//!   recorded as a refused vote with the reason, never coerced into a yes or a no.
//! - **A turn is assessed once per assessor version.**
//!
//! # The fast model answers first, and the assessor only where it is unsure (DL-08, #467)
//!
//! A whole turn's questions go to the fast decision model in **one** request, three pictures' worth of the same evidence, and that answer is
//! recorded as [`crate::turn_store::FastAnswers`] — **never a label**, so no student is trained on its own guess. The assessor is then asked only the
//! questions the fast model cannot settle, by [`crate::label_report::escalate`]: unsure (between the named bounds), outvoted (against the person's or
//! the engine's answer), and the named random share. **Never the other way round**: nothing the assessor says sends a question back to the fast model.
//!
//! **The gate.** Until the holdout (DL-07) shows the assessor reaching [`crate::label_report::TEACHER_BAR`] over
//! [`crate::label_report::HOLDOUT_MIN`] answers, every question is escalated and nothing is saved: the teacher is not worth distilling from yet, and
//! the report says so. A batch counts what it saved ([`Batch::saved`]).
//!
//! # Privacy
//!
//! The pictures go to the chosen model. With **Remote** chosen they leave the machine, so [`assess_ledger`] refuses unless it was told it may
//! (`--allow-remote`), and it says where they go. A model that is unreachable stops the batch with its error: nothing is guessed.

use std::collections::BTreeMap;

use bevy_oracle::{Answer, DecisionModel, Error, Question, Request, Transport, ask, data_url};
use serde_json::json;


use crate::turn_store::{
    AssessedQuestion, Assessment, AssessorRef, Escalated, FastAnswer, FastAnswers, Line, Read, Store, Turn, Vote,
};

/// **The version of the prompt and the wordings below.** A change to either is a new number: every record says which it was assessed with, and a
/// turn assessed by an older version is assessed again.
pub const ASSESSOR_VERSION: u32 = 1;

/// **How many times each question is asked**, once per wording.
pub const VOTES: usize = 3;

/// **The three wordings**, `{}` being the question as the record stores it. The stored words are never changed; these frame them.
pub const WORDINGS: [&str; VOTES] = [
    "{}",
    "Look for what is wrong first, then judge this. {}",
    "Judge strictly, as a reviewer who expects to find a defect. {}",
];

/// **The prompt**: the model is asked for the defects of the result against the request, with the request's own words.
fn prompt(turn: &Turn, pictures: &[&str]) -> String {
    let request = &turn.request;
    let mut said = String::from(
        "You are a strict reviewer grading the result of an editing task in a 3D map editor. Look for what is WRONG with the result against what was \
         asked, not for what is right with it. ",
    );
    said.push_str(&format!("The pictures, in order, are: {}. ", pictures.join(", ")));
    said.push_str(&format!("The task: {}. {}", request.title, request.summary));
    if !request.specified.is_empty() {
        said.push_str(&format!(" The person asked for: {}.", request.specified.join("; ")));
    }
    said.push_str(" Answer the question from the pictures alone.");
    said
}

/// **A question in one of the wordings.**
fn worded(question: &Question, wording: usize) -> Question {
    let frame = |text: &str| WORDINGS[wording].replace("{}", text);
    match question {
        Question::Noul { instructions } => Question::Noul { instructions: frame(instructions) },
        Question::Choice { instructions, criteria } => Question::Choice { instructions: frame(instructions), criteria: criteria.clone() },
        Question::Score { instructions, criteria } => Question::Score { instructions: frame(instructions), criteria: criteria.clone() },
    }
}

/// **One vote**: the model asked this question in this wording, alone.
///
/// `Err` is a failure of the model or the way to it (unreachable, a refused request, no key): it ends the batch. A provider's refusal, an answer of
/// the wrong type and an option that is not offered are the vote's own refusal.
fn vote(transport: &dyn Transport, state: &str, pictures: &[String], question: &Question, wording: usize) -> Result<Vote, Error> {
    let mut request = Request::new(state).with_question("q", worded(question, wording));
    for picture in pictures {
        request = request.with_image(picture.clone());
    }
    let refused = |reason: String| Vote { wording: wording as u32, answer: None, p_yes: None, refused: Some(reason), model: None };
    match ask(transport, &request) {
        Ok(response) => {
            let model = response.model.clone();
            Ok(match response.answers.get("q") {
                Some(Answer::Noul { noul }) if (0.0..=1.0).contains(noul) => {
                    Vote { wording: wording as u32, answer: Some(json!({"type": "noul", "noul": noul})), p_yes: Some(*noul), refused: None, model }
                }
                Some(Answer::Noul { noul }) => refused(format!("the probability {noul} is outside 0..1")),
                Some(other) => Vote { wording: wording as u32, answer: serde_json::to_value(other).ok(), p_yes: None, refused: None, model },
                None => refused("the model answered no question".to_owned()),
            })
        }
        Err(e @ (Error::Refusal { .. } | Error::BadResponse { .. })) => Ok(refused(scrub(&e.to_string(), &transport.endpoint()))),
        Err(e) => Err(e),
    }
}

/// **What the votes on one question come to**: for a yes/no, the fraction of the valid votes that said yes; for a choice, how many votes each option got.
fn tally(question: &Question, votes: &[Vote]) -> (Option<f64>, BTreeMap<String, u32>, u32) {
    let refused = votes.iter().filter(|v| v.refused.is_some()).count() as u32;
    match question {
        Question::Noul { .. } => {
            let valid: Vec<f64> = votes.iter().filter_map(|v| v.p_yes).collect();
            let fraction = (!valid.is_empty()).then(|| valid.iter().filter(|p| **p >= 0.5).count() as f64 / valid.len() as f64);
            (fraction, BTreeMap::new(), refused)
        }
        Question::Choice { .. } => {
            let mut counts = BTreeMap::new();
            for chosen in votes.iter().filter_map(|v| v.answer.as_ref()).filter_map(|a| a["choice"].as_str()) {
                *counts.entry(chosen.to_owned()).or_insert(0) += 1;
            }
            (None, counts, refused)
        }
        Question::Score { .. } => (None, BTreeMap::new(), refused),
    }
}

fn pictures_of(turn: &Turn, store: &Store) -> (Vec<String>, Vec<&'static str>) {
    let mut pictures = Vec::new();
    let mut named = Vec::new();
    for (name, shot) in [("before", &turn.before), ("after", &turn.after)] {
        if let Some(image) = &shot.image
            && let Ok(bytes) = std::fs::read(store.image_path(&image.sha256))
        {
            pictures.push(data_url(&bytes));
            named.push(name);
        }
    }
    (pictures, named)
}

/// **Where the answers come from**, as the record says it.
fn assessor_of(transport: &dyn Transport) -> AssessorRef {
    AssessorRef {
        version: ASSESSOR_VERSION,
        // The address with no key in it: the ledger is kept for good.
        endpoint: public_endpoint(&transport.endpoint()),
        site: transport.site().where_ran().to_owned(),
    }
}

/// **An endpoint without a key in it**: no `user:password@`, no query, no fragment.
fn public_endpoint(url: &str) -> String {
    let (scheme, rest) = match url.split_once("://") {
        Some((scheme, rest)) => (format!("{scheme}://"), rest),
        None => (String::new(), url),
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    format!("{scheme}{host}{}", tail.split(['?', '#']).next().unwrap_or(""))
}

/// **`text` with every address in it made safe to keep**: `bevy_oracle::Error`'s own words name the endpoint as it was asked, key and all, and a
/// refused vote's reason is kept for good. (The same scrub the MCP server's decision report makes, which cannot be shared across the two crates.)
fn scrub(text: &str, endpoint: &str) -> String {
    let text = if endpoint.is_empty() { text.to_owned() } else { text.replace(endpoint, &public_endpoint(endpoint)) };
    text.split_inclusive(char::is_whitespace)
        .map(|piece| {
            let word = piece.trim_end();
            let spaces = &piece[word.len()..];
            let Some(at) = word.find("://") else { return piece.to_owned() };
            let core = word.trim_end_matches(['"', '\'', ')', ',', '.', ';']);
            let tail = &word[core.len()..];
            let start = word[..at].rfind(|c: char| !c.is_ascii_alphabetic()).map_or(0, |p| p + 1);
            format!("{}{}{tail}{spaces}", &word[..start], public_endpoint(&core[start..]))
        })
        .collect()
}

/// **Assess one turn**: every question of its record, [`VOTES`] times each. A turn with no picture is recorded as skipped, with the reason.
pub fn assess_turn(turn: &Turn, store: &Store, transport: &dyn Transport) -> Result<Assessment, Error> {
    let mut assessment = Assessment {
        turn: turn.id.clone(),
        assessor: assessor_of(transport),
        at_unix_ms: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
        answers: Vec::new(),
        skipped: None,
    };
    let (pictures, named) = pictures_of(turn, store);
    if pictures.is_empty() {
        assessment.skipped = Some("the record has no picture that can be read, so there is nothing to look at".to_owned());
        return Ok(assessment);
    }
    let state = prompt(turn, &named);
    for item in &turn.questions.items {
        let Ok(question) = serde_json::from_value::<Question>(item.question.clone()) else {
            assessment.answers.push(AssessedQuestion {
                id: item.id.clone(),
                votes: Vec::new(),
                yes_fraction: None,
                tally: BTreeMap::new(),
                refused: VOTES as u32,
            });
            continue;
        };
        let mut votes = Vec::with_capacity(VOTES);
        for wording in 0..VOTES {
            votes.push(vote(transport, &state, &pictures, &question, wording)?);
        }
        let (yes_fraction, tally, refused) = tally(&question, &votes);
        assessment.answers.push(AssessedQuestion { id: item.id.clone(), votes, yes_fraction, tally, refused });
    }
    Ok(assessment)
}

/// **Assess one turn's questions, but only the ones `ask` says go to the assessor**: a question it answers no to is skipped, and its id is left out of
/// the record's answers.
fn assess_turn_where(turn: &Turn, store: &Store, transport: &dyn Transport, ask: &dyn Fn(&str) -> bool) -> Result<Assessment, Error> {
    let mut assessment = Assessment {
        turn: turn.id.clone(),
        assessor: assessor_of(transport),
        at_unix_ms: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0),
        answers: Vec::new(),
        skipped: None,
    };
    let (pictures, named) = pictures_of(turn, store);
    if pictures.is_empty() {
        assessment.skipped = Some("the record has no picture that can be read, so there is nothing to look at".to_owned());
        return Ok(assessment);
    }
    let state = prompt(turn, &named);
    for item in turn.questions.items.iter().filter(|item| ask(&item.id)) {
        let Ok(question) = serde_json::from_value::<Question>(item.question.clone()) else {
            assessment.answers.push(AssessedQuestion {
                id: item.id.clone(),
                votes: Vec::new(),
                yes_fraction: None,
                tally: BTreeMap::new(),
                refused: VOTES as u32,
            });
            continue;
        };
        let mut votes = Vec::with_capacity(VOTES);
        for wording in 0..VOTES {
            votes.push(vote(transport, &state, &pictures, &question, wording)?);
        }
        let (yes_fraction, tally, refused) = tally(&question, &votes);
        assessment.answers.push(AssessedQuestion { id: item.id.clone(), votes, yes_fraction, tally, refused });
    }
    Ok(assessment)
}

/// **The fast model's answers to one turn's questions, in one request per [`MAX_QUESTIONS`]** (DL-08, #467): the whole turn at once, from the same
/// pictures and the request's own words, so the escalation decision is made on evidence the assessor would see too. Never a label.
///
/// A turn with no picture that can be read produces no line: there is nothing to look at, and the assessor skips it for the same reason.
fn fast_pass(turn: &Turn, store: &Store, transport: &dyn Transport) -> Result<FastAnswers, Error> {
    let (pictures, named) = pictures_of(turn, store);
    if pictures.is_empty() {
        // **A line all the same**, with the reason: a turn nobody can look at must not be asked again on every batch.
        return Ok(FastAnswers {
            turn: turn.id.clone(),
            at_unix_ms: now_ms(),
            model: None,
            answers: Vec::new(),
            escalation: turn
                .questions
                .items
                .iter()
                .map(|item| Escalated { id: item.id.clone(), asked: true, why: "no-picture".to_owned() })
                .collect(),
            skipped: Some("the record has no picture that can be read, so there is nothing to look at".to_owned()),
        });
    }
    let state = prompt(turn, &named);
    let mut answers: Vec<FastAnswer> = Vec::new();
    let mut model: Option<String> = None;
    for chunk in turn.questions.items.chunks(bevy_oracle::MAX_QUESTIONS) {
        let mut request = Request::new(state.clone());
        for picture in &pictures {
            request = request.with_image(picture.clone());
        }
        let mut asked = 0;
        for item in chunk {
            let Ok(question) = serde_json::from_value::<Question>(item.question.clone()) else { continue };
            request.questions.insert(item.id.clone(), question);
            asked += 1;
        }
        if asked == 0 {
            continue;
        }
        // **A model that cannot be reached stops the batch**, as it does for the assessor: nothing is guessed, and the ledger keeps no half answer.
        let response = ask(transport, &request)?;
        if let Some(name) = response.model {
            model.get_or_insert(name);
        }
        for item in chunk {
            let answer = response.answers.get(&item.id);
            answers.push(match answer {
                // **Only a yes/no has a probability the escalation can read**; a choice or a score is answered and not escalated on, so it counts as
                // a refusal here and the assessor decides it.
                Some(Answer::Noul { noul }) if (0.0..=1.0).contains(noul) => FastAnswer { id: item.id.clone(), p_yes: Some(*noul), refused: None },
                Some(Answer::Noul { noul }) => FastAnswer { id: item.id.clone(), p_yes: None, refused: Some(format!("the probability {noul} is outside 0..1")) },
                Some(_) => FastAnswer { id: item.id.clone(), p_yes: None, refused: Some("it is not a yes/no answer, so the assessor decides it".to_owned()) },
                None => FastAnswer { id: item.id.clone(), p_yes: None, refused: Some("the model answered no question".to_owned()) },
            });
        }
    }
    // **The escalation is decided once, by the one place that decides it** ([`crate::label_report::escalate`]), and kept with the answers, so the
    // report's count is what the batch did and never a second opinion.
    let escalation: Vec<Escalated> = answers
        .iter()
        .map(|answer| {
            let against = crate::label_report::against(turn, &answer.id);
            let (asked, why) = crate::label_report::escalate(answer.p_yes, against, crate::label_report::seed_of(&turn.id, &answer.id));
            Escalated { id: answer.id.clone(), asked, why: why.to_owned() }
        })
        .collect();
    Ok(FastAnswers { turn: turn.id.clone(), at_unix_ms: now_ms(), model, answers, escalation, skipped: None })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// **What a batch did.**
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Batch {
    /// The turns assessed now.
    pub assessed: usize,
    /// The turns skipped because this version had assessed them already.
    pub already: usize,
    /// **The turns the fast model answered first** (DL-08, #467): `0` for a batch with no fast model in hand.
    pub fast: usize,
    /// **The questions it saved**: escalated to nobody, so nobody paid a call for them.
    pub saved: usize,
}

/// **Assess every turn in the ledger that this assessor version has not assessed**, appending one line each.
///
/// `allow_remote` is the person's say that pictures may leave the machine: without it a Remote choice is refused, naming where they would go.
/// `limit` stops after that many turns, for a batch that is run a little at a time.
///
/// **This is the batch with no fast model in hand** ([`assess_ledger_with`] with `None`): every question goes to the assessor, which is what the gate
/// does until the holdout has shown the teacher is worth distilling from, and what a caller that has no calibrated fast model has.
pub fn assess_ledger(store: &mut Store, transport: &dyn Transport, allow_remote: bool, limit: Option<usize>) -> Result<Batch, String> {
    assess_ledger_with(store, None, transport, allow_remote, limit)
}

/// **The batch the editor runs**: the fast model answers every turn's questions first, its answers are kept (and are never labels), and the assessor
/// is asked only where it is unsure (DL-08, #467).
///
/// **The gate decides whether anything is saved at all**: while the holdout has not shown the assessor reaching [`TEACHER_BAR`] over
/// [`HOLDOUT_MIN`] answers, every question is escalated (`why: "gate"`), so the ledger collects the fast model's answers without spending less.
pub fn assess_ledger_with(store: &mut Store, fast: Option<&dyn Transport>, transport: &dyn Transport, allow_remote: bool, limit: Option<usize>) -> Result<Batch, String> {
    if transport.site() == DecisionModel::Remote && !allow_remote {
        return Err(format!(
            "the decision model chosen is Remote ({}), so assessing would send the turns' pictures off this machine; run it again with --allow-remote to say that is fine, or choose Local in Settings",
            public_endpoint(&transport.endpoint())
        ));
    }
    let read: Read = crate::turn_store::read_file(&store.ledger_path())?;
    let done: std::collections::BTreeSet<&str> =
        read.assessments().filter(|a| a.assessor.version == ASSESSOR_VERSION).map(|a| a.turn.as_str()).collect();
    let already_fast: std::collections::BTreeSet<&str> = read.fast_answers().map(|fast| fast.turn.as_str()).collect();
    // **The gate**, read once: the holdout's agreement against the bar, as the report reads it.
    let gate_open = crate::label_report::gate_open(&read);
    let mut batch = Batch::default();
    for turn in read.turns() {
        let assessed = done.contains(turn.id.as_str());
        let fast_done = fast.is_none() || already_fast.contains(turn.id.as_str());
        if assessed && fast_done {
            batch.already += 1;
            continue;
        }
        if limit.is_some_and(|limit| batch.assessed >= limit) {
            break;
        }
        // **The fast model first**, and its answers are kept before anything is asked of the assessor.
        let mut plan: Option<FastAnswers> = None;
        if let Some(model) = fast
            && !already_fast.contains(turn.id.as_str())
        {
            match fast_pass(turn, store, model) {
                Ok(mut answers) => {
                    if !gate_open {
                        // **The gate is closed: nothing may be saved**, so every question goes on to the assessor, and the line says why.
                        for escalated in &mut answers.escalation {
                            escalated.asked = true;
                            escalated.why = "gate".to_owned();
                        }
                    }
                    store.append(&Line::fast_answer(answers.clone()))?;
                    batch.fast += 1;
                    plan = Some(answers);
                }
                Err(e) => return Err(format!("{} ({} turn(s) assessed before it)", scrub(&e.to_string(), &model.endpoint()), batch.assessed)),
            }
        }
        // **A turn this version has assessed already** is not assessed twice: only the fast pass it was missing was run.
        if assessed {
            batch.already += 1;
            continue;
        }
        let ask: Box<dyn Fn(&str) -> bool + '_> = match &plan {
            Some(answers) => Box::new(move |id: &str| answers.asked(id)),
            None => Box::new(|_: &str| true),
        };
        let assessment = assess_turn_where(turn, store, transport, ask.as_ref())
            .map_err(|e| format!("{} ({} assessed before it)", scrub(&e.to_string(), &transport.endpoint()), batch.assessed))?;
        if let Some(answers) = &plan {
            batch.saved += answers.escalation.iter().filter(|escalated| !escalated.asked).count();
        }
        store.append(&Line::assessment(assessment))?;
        batch.assessed += 1;
    }
    Ok(batch)
}
