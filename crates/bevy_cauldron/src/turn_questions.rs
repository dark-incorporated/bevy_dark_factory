//! **The typed questions each turn carries** (DL-03, #462; series labels from use, #459).
//!
//! A turn's record (DL-01) holds the pictures; this holds **what a student will later be asked about them**, written in the decision model's own
//! shapes (`noul`, `choice`, `score`; [`bevy_oracle::Question`]), so a label and a prediction are about the same question.
//!
//! # Where the questions come from
//!
//! - **One `noul` per `specified` line**, stated **declaratively**: "The result satisfies: a mountain with a twin peak", never "Does ...?". laya's
//!   `noul` head answered 0.0000 to every interrogative instruction when it was measured on 2026-09-21, and nobody has checked d3, so the one
//!   wording that is known to work is the only one written.
//! - **The named checks** that apply ([`NAMED_CHECKS`]), in the checks' own words, with the engine's answer beside each where the engine can
//!   compute one ([`Engine`]). An engine answer is an **objective** label, with no teacher in it.
//! - **The agent's own `grimoire_decide` questions** from the turn (DM-05), copied **verbatim**.
//! - **The turn question**, "The person keeps this result". Its gold label is DL-02's verdict, and it is the one target with no teacher noise.
//!
//! # Wording is stored, never regenerated
//!
//! **One function writes every question** ([`questions_for`]), one template per source ([`Templates`]), and **the text it wrote is stored in the
//! record beside the version of the templates** ([`Questions::template_version`]). Reading a record never writes a question, so changing the
//! wording later (a new [`Templates`] with a new version) changes the next record and leaves every old one as it was: an old label is never
//! silently about a new sentence.
//!
//! # What is refused rather than shaped
//!
//! A `specified` line that is **empty** or **longer than [`SPECIFIED_MAX_CHARS`]** produces **no question**, and the record says which line and
//! why ([`Skipped`]). It is never trimmed: a question about half of what the person said is a question about something they did not ask.

use bevy_oracle::{Checks, Question};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// **The version of the wording below.** A change to any template is a new number, so a record always says which words it was written with.
pub const TEMPLATE_VERSION: u32 = 1;

/// **The longest `specified` line that becomes a question**, in characters. A line past it is a paragraph, not a requirement a yes/no can be
/// asked of, and is skipped with its reason rather than cut.
pub const SPECIFIED_MAX_CHARS: usize = 240;

/// **The id of the turn question**, whose gold label is the person's verdict (DL-02).
pub const TURN_QUESTION_ID: &str = "turn-kept";

/// **The checks a turn is asked, by the names the checks file gives them.** The issue says `outside_area`; the shipped check
/// (`bevy_oracle/fixtures/checks.yaml`) is `out_of_area`, and a question is read from the check, so the file's name is the one used.
pub const NAMED_CHECKS: [&str; 4] = ["floating", "clipping", "out_of_area", "blank_frame"];

/// **How far a piece may be lifted off where it would rest before the engine calls it floating**, in metres. The editor's lift keys step by a
/// subgrid unit; anything under a centimetre is rounding, not a hover.
pub const FLOATING_LIFT_M: f32 = 0.01;

/// **The words of every question, by source.** `{}` is replaced by the line it is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Templates {
    pub version: u32,
    /// What a `specified` line becomes. Declarative: it states, and the model says whether the statement holds.
    pub specified: &'static str,
    /// The turn question.
    pub turn: &'static str,
}

impl Templates {
    /// The wording this build writes.
    pub const CURRENT: Templates = Templates {
        version: TEMPLATE_VERSION,
        specified: "The result satisfies: {}",
        turn: "The person keeps this result",
    };
}

/// **Why a line produced no question.**
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    /// Where the line came from: `specified`, or `check` for a checks file that did not read.
    pub source: String,
    /// The line, whole. Never trimmed.
    pub text: String,
    pub why: String,
}

/// **What the engine says the answer is**, where it can compute one: an objective label.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EngineAnswer {
    /// `yes` or `no`, in the check's own sense: for a check, yes means something is wrong.
    pub answer: String,
    /// The probability the engine gives yes: 1 or 0, because the engine is not guessing.
    pub p_yes: f64,
    /// A sentence saying what was measured.
    pub basis: String,
}

impl EngineAnswer {
    pub fn new(yes: bool, basis: impl Into<String>) -> EngineAnswer {
        EngineAnswer { answer: if yes { "yes" } else { "no" }.to_owned(), p_yes: if yes { 1.0 } else { 0.0 }, basis: basis.into() }
    }
}

/// **One question a turn carries.**
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TurnQuestion {
    /// Unique in the record: `specified-1`, `check-floating`, `decision-2-mood`, `turn-kept`.
    pub id: String,
    /// `specified`, `check`, `decision` or `turn`.
    pub source: String,
    /// What the student looks at: `after`, or `change` when the question is about the difference between the before and the after.
    pub about: String,
    /// The question in the decision model's wire shape (`{"type": "noul", "instructions": ...}`).
    pub question: Value,
    /// The engine's answer, when it can compute one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineAnswer>,
}

/// **Every question a turn carries**, with the version of the wording that wrote them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Questions {
    /// [`Templates::version`] when the record was written. `0` is a record from before questions existed.
    pub template_version: u32,
    pub items: Vec<TurnQuestion>,
    pub skipped: Vec<Skipped>,
}

/// **What the engine could compute from the map at the after picture.** A field is `None` where the engine has no answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Engine {
    pub floating: Option<EngineAnswer>,
    pub clipping: Option<EngineAnswer>,
    pub out_of_area: Option<EngineAnswer>,
    pub blank_frame: Option<EngineAnswer>,
}

impl Engine {
    fn for_check(&self, name: &str) -> Option<EngineAnswer> {
        match name {
            "floating" => self.floating.clone(),
            "clipping" => self.clipping.clone(),
            "out_of_area" => self.out_of_area.clone(),
            "blank_frame" => self.blank_frame.clone(),
            _ => None,
        }
    }
}

/// **What a turn's questions are made from.**
pub struct Input<'a> {
    /// What the person asked for, as the agent quoted it.
    pub specified: &'a [String],
    /// The project's checks (the editor's own with the project's over them), or why they could not be read.
    pub checks: Result<&'a Checks, &'a str>,
    pub engine: &'a Engine,
    /// The turn's `grimoire_decide` reports, as the ledger kept them.
    pub decisions: &'a [Value],
}

/// **Write the questions of one turn.** The only place a question is worded.
pub fn questions_for(input: &Input<'_>, templates: &Templates) -> Questions {
    let mut out = Questions { template_version: templates.version, items: Vec::new(), skipped: Vec::new() };

    // 1. One declarative noul per specified line.
    for (at, line) in input.specified.iter().enumerate() {
        let n = at + 1;
        if line.trim().is_empty() {
            out.skipped.push(Skipped { source: "specified".to_owned(), text: line.clone(), why: format!("specified line {n} is empty, so there is nothing to ask") });
            continue;
        }
        let length = line.chars().count();
        if length > SPECIFIED_MAX_CHARS {
            out.skipped.push(Skipped {
                source: "specified".to_owned(),
                text: line.clone(),
                why: format!("specified line {n} is {length} characters, over the {SPECIFIED_MAX_CHARS} a question is made from, and a line is never trimmed"),
            });
            continue;
        }
        out.items.push(TurnQuestion {
            id: format!("specified-{n}"),
            source: "specified".to_owned(),
            about: "after".to_owned(),
            question: noul(&templates.specified.replace("{}", line)),
            engine: None,
        });
    }

    // 2. The named checks that apply, in the checks' own words, with the engine's answer beside each.
    match input.checks {
        Ok(checks) => {
            for name in NAMED_CHECKS {
                let Some(check) = checks.get(name) else { continue };
                out.items.push(TurnQuestion {
                    id: format!("check-{name}"),
                    source: "check".to_owned(),
                    // The out-of-area check compares the before and the after; the others look at the result.
                    about: if check.images.iter().any(|image| image == "before") { "change" } else { "after" }.to_owned(),
                    question: noul(&check.question),
                    engine: input.engine.for_check(name),
                });
            }
        }
        Err(why) => out.skipped.push(Skipped { source: "check".to_owned(), text: String::new(), why: format!("the checks did not read: {why}") }),
    }

    // 3. The agent's own questions, verbatim: the same JSON it sent, under the id it gave.
    for (at, decision) in input.decisions.iter().enumerate() {
        let Some(asked) = decision["params"]["questions"].as_object() else { continue };
        for (id, question) in asked {
            out.items.push(TurnQuestion {
                id: format!("decision-{}-{id}", at + 1),
                source: "decision".to_owned(),
                about: "after".to_owned(),
                question: question.clone(),
                engine: None,
            });
        }
    }

    // 4. The turn question, whose gold label is the person's verdict.
    out.items.push(TurnQuestion {
        id: TURN_QUESTION_ID.to_owned(),
        source: "turn".to_owned(),
        about: "after".to_owned(),
        question: noul(templates.turn),
        engine: None,
    });
    out
}

/// A `noul` question, in the shape the decision model reads.
fn noul(instructions: &str) -> Value {
    serde_json::to_value(Question::noul(instructions.to_owned())).unwrap_or_else(|_| json!({"type": "noul", "instructions": instructions}))
}
