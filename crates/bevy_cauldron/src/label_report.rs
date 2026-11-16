//! **The label report: whether the set is worth training on, and what the teacher costs** (DL-08, #467; series labels from use, #459).
//!
//! The ledger records what happened and DL-06's export resolves which of it a student may learn from. Neither says whether the set is worth
//! training on, and the one number that decides it is the teacher's: **a student cannot beat the assessor that labelled it**.
//!
//! # What it shows
//!
//! - **Per week**: turns recorded, turns the person answered, and the labels per tier — gold (the person's), objective (the engine's), silver (a
//!   teacher's) and parked (no trusted answer reached the question).
//! - **The class balance per question source**: how many of the person's and the engine's answers are yes against no, so a set that teaches only one
//!   answer is visible rather than inferred.
//! - **The assessor against the person**: how many turns the person undid, and how often the assessor had said the result was fine anyway (it said
//!   "the person keeps this" through the Undo, which is the label a student would learn); and its agreement on the human holdout (DL-07), the same
//!   measurement taken on questions nobody trains on.
//! - **The fast model against both**: its agreement with the person's gold answers and with the assessor, which is what a student starting from that
//!   model already knows.
//! - **The disagreements by kind** (DL-06's own count), the size of the set, the holdout's own size, and the per-model calibrations DM-04 recorded.
//!
//! # Escalation: the fast model answers first, and the assessor is asked where it is unsure
//!
//! The assessor costs an expensive call per question, three wordings over. The fast decision model (DM-01, calibrated by DM-04) answers a whole
//! turn's questions in one request, and **the assessor is asked only the questions the fast model cannot settle**:
//!
//! - **unsure**: its probability is between the named bounds ([`UNSURE_LOW`], [`UNSURE_HIGH`]), so it is not deciding;
//! - **outvoted**: it disagrees with the turn's objective answer where the engine has one (a named check's measurement);
//! - **sampled**: the named random share ([`ESCALATE_SHARE`], one question in that many), so the assessor stays measured on the questions the fast
//!   model was sure of too. Without it the teacher's agreement would only ever be measured on the cases it is weakest at, and the ceiling would be
//!   wrong in the one direction that matters.
//!
//! **Never the other way round.** Every question goes to the fast model first, and nothing the assessor says sends a question back to it. **A fast
//! answer is never a label**: it is held in [`crate::turn_store::FastAnswers`], a line of its own that DL-06's export does not read, so a student
//! can never be trained on its own guess. Every question it saves is counted ([`Escalation::saved`]).
//!
//! # The gate: off until the teacher is worth distilling from
//!
//! Escalation is off until the assessor's holdout agreement reaches [`TEACHER_BAR`] over at least [`HOLDOUT_MIN`] answers. Below either, **every**
//! question goes to the assessor (nothing is saved), and the report says the teacher is not good enough to distil from, with the numbers that
//! decided it.
//!
//! # Derived, and byte-identical
//!
//! The report reads the ledger and what DL-06 wrote beside it, and writes `report.md` and `report.json` into the export folder. It writes nothing into
//! the labels folder and holds nothing of the machine, the clock or the output path, so **two reports of one ledger are byte-identical**.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_oracle::{Checks, LOCAL_MODEL_NAME, OPENROUTER_MODEL};
use serde::{Deserialize, Serialize};

use crate::turn_export::{Options, SUMMARY_FILE};
use crate::turn_questions::TURN_QUESTION_ID;
use crate::turn_store::{AUDIT, Assessment, FastAnswers, Label, Read, Turn, read_file};
use bevy_grimoire_core::rig_check::civil_date_utc;

/// **The report a person reads**, inside the export folder.
pub const REPORT_FILE: &str = "report.md";

/// **The same report as data**, beside it: what a test, a gate or a later pass reads.
pub const REPORT_JSON: &str = "report.json";

/// **The bar the holdout agreement must reach before escalation is turned on** (DL-08, #467): the assessor is right four times in five.
///
/// Below it, a student trained on its answers learns its one mistake for every four right ones, and its ceiling is that noise. The shape is the one
/// `decide_check`'s `--bar` already has, where a calibration that does not reach the bar may not block anything: a calibration too weak to block a
/// build is too weak to teach one.
pub const TEACHER_BAR: f64 = 0.80;

/// **The fewest holdout answers that can show the bar is met.** The audit's weekly target (`audit_view::WEEKLY_TARGET`): an agreement reached on
/// three questions is not a measurement, and opening escalation on one is exactly what this gate exists to prevent.
pub const HOLDOUT_MIN: usize = 20;

/// **The lower bound of "the fast model is guessing"**, on its P(yes): below it the answer is a no it believes.
pub const UNSURE_LOW: f64 = 0.35;

/// **The upper bound of "the fast model is guessing"**, on its P(yes): above it the answer is a yes it believes.
pub const UNSURE_HIGH: f64 = 0.65;

/// **One question in this many goes to the assessor whatever the fast model said**, chosen by the question's own hash so two runs agree: a fifth of
/// the questions it was sure of, which is enough to measure the assessor there every week and saves the other four fifths.
pub const ESCALATE_SHARE: u32 = 5;

// ---------------------------------------------------------------------------------------------------------------
// The report's types.
// ---------------------------------------------------------------------------------------------------------------

/// **One week of the report.** `week` is the Monday's date, UTC, so two machines name a week the same way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Week {
    /// `YYYY-MM-DD`: the Monday of the week, UTC.
    pub week: String,
    /// The turns whose hand-back fell in the week.
    pub turns: usize,
    /// Of those, the turns the person answered: a Keep, an Undo, a revision or an audit answer.
    pub answered: usize,
    /// Labels from the person.
    pub gold: usize,
    /// Labels from the engine: an answer it computed, and an engine-made defect.
    pub objective: usize,
    /// Labels from a teacher.
    pub silver: usize,
    /// Questions of the week's records that DL-06 parked: no trusted answer reached them.
    pub parked: usize,
}

/// **One question the ledger asks, and what is known about its answers.**
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestionRow {
    pub id: String,
    /// `specified`, `check`, `decision` or `turn`.
    pub source: String,
    /// How many turns ask it.
    pub turns: usize,
    pub gold: usize,
    pub objective: usize,
    pub silver: usize,
    /// The person's answers to it that are the human holdout.
    pub holdout: usize,
}

/// **One question source's class balance**: how many of the answers that stand say yes against no. A source whose answers are all one way teaches
/// a student to give that answer whatever it is shown.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRow {
    pub source: String,
    pub gold_yes: usize,
    pub gold_no: usize,
    pub objective_yes: usize,
    pub objective_no: usize,
    pub silver_yes: usize,
    pub silver_no: usize,
}

/// **How many questions two answers were compared on, and how many agreed.**
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agreement {
    pub questions: usize,
    pub agreed: usize,
}

impl Agreement {
    /// The share agreed, or `None` where nothing was compared: never a zero that looks like a measurement.
    pub fn share(&self) -> Option<f64> {
        (self.questions > 0).then(|| self.agreed as f64 / self.questions as f64)
    }
}

/// **The assessor's accuracy against the person**: the student's ceiling.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assessor {
    /// The questions of the ledger it answered.
    pub answered: usize,
    /// The turns the person undid, and how often the assessor had said the result was fine anyway.
    pub undone: usize,
    pub said_fine_on_an_undo: usize,
    /// The turns the person kept, and how often the assessor agreed.
    pub kept: usize,
    pub said_fine_on_a_keep: usize,
    /// **Its agreement on the human holdout**: the questions nobody trains on, so the one measurement with no leakage in it.
    pub holdout: Agreement,
    pub bar: f64,
    pub holdout_min: usize,
    /// What DL-06 counted: the assessor contradicting the answer that stood.
    pub teacher_errors: usize,
}

impl Assessor {
    /// **Whether the holdout shows the assessor is good enough to distil from**: the bar, over the fewest answers that can show it.
    pub fn meets_the_bar(&self) -> bool {
        self.holdout.questions >= self.holdout_min && self.holdout.share().is_some_and(|share| share >= self.bar)
    }
}

/// **The fast model against the person and against the assessor.**
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Fast {
    /// The turns whose questions it answered.
    pub turns: usize,
    pub questions: usize,
    /// Its agreement with the person's gold answers, where both answered one question.
    pub with_gold: Agreement,
    /// Its agreement with the assessor.
    pub with_assessor: Agreement,
}

/// **One model's calibrations**, as DM-04 recorded them, or that it is still waiting for one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CalibrationRow {
    pub model: String,
    /// `measured` or `waiting`.
    pub state: String,
    /// The checks the records are for, with what each measured. Empty while waiting.
    pub checks: Vec<MeasuredCheck>,
    /// Why the row says what it says, in the words a person reads. **An absent remote calibration is `waiting` and never an error**: DM-04's
    /// remote run waits on the owner's consent and a quota that fits 122 requests.
    pub why: String,
}

/// **One check's calibration for one model** (DM-04, #455).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MeasuredCheck {
    pub check: String,
    pub set_size: usize,
    pub agreement: f64,
    pub threshold: f64,
    pub date: String,
}

/// **What escalation did, and why it was on.**
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Escalation {
    /// Whether the assessor was asked only where the fast model was unsure. Closed while the holdout has not shown the teacher is worth it.
    pub open: bool,
    /// The questions of every turn the fast model answered.
    pub questions: usize,
    /// The ones that went on to the assessor.
    pub escalated: usize,
    /// The ones it saved: an expensive call nobody paid.
    pub saved: usize,
    /// [`ESCALATE_SHARE`], the named random share.
    pub share: u32,
    /// [`UNSURE_LOW`] and [`UNSURE_HIGH`], the named bounds on the fast model's probability.
    pub bounds: (f64, f64),
    /// What the gate read: the holdout's agreement, and the bar it was compared with.
    pub gate: Agreement,
    pub bar: f64,
    pub holdout_min: usize,
}

/// **The set DL-06 wrote**, as its own `summary.json` says it: the report cites the set and never invents a number it does not carry.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Set {
    /// Whether an export is beside the ledger at all.
    pub found: bool,
    pub train: usize,
    pub eval: usize,
    pub holdout_records: usize,
    pub images: usize,
    /// Parked questions, by the export's own kind.
    pub parked: BTreeMap<String, usize>,
    /// The records the export parked, and how many of each one's questions: what places a parked question in its week.
    pub parked_by_record: BTreeMap<String, usize>,
    /// Records the set could not take, by kind.
    pub left_out: BTreeMap<String, usize>,
    /// DL-06's disagreements, by kind: `teacher` is the assessor's error count.
    pub disagreements: BTreeMap<String, usize>,
    pub teacher_errors: usize,
}

/// **Whether the set is worth training on**, as one word and its reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Distil {
    /// `on` or `off`.
    pub state: String,
    pub why: String,
}

/// **The report.**
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// Every week with a record in it, oldest first.
    pub weeks: Vec<Week>,
    pub summary: Totals,
    pub questions: Vec<QuestionRow>,
    pub sources: Vec<SourceRow>,
    pub assessor: Assessor,
    pub fast: Fast,
    /// DL-06's disagreements, by kind.
    pub disagreements: BTreeMap<String, usize>,
    pub set: Set,
    pub escalation: Escalation,
    pub calibrations: Vec<CalibrationRow>,
    pub distil: Distil,
}

/// **The totals over every week.**
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Totals {
    pub turns: usize,
    /// Turns the person answered.
    pub answered: usize,
    /// Turns the person closed without judging: not a label, and not an answer.
    pub dismissed: usize,
    pub gold: usize,
    pub objective: usize,
    pub silver: usize,
    /// The person's answers that are the human holdout.
    pub holdout: usize,
    /// The questions the ledger's turns ask.
    pub questions: usize,
}

// ---------------------------------------------------------------------------------------------------------------
// The report, from the ledger and the export beside it.
// ---------------------------------------------------------------------------------------------------------------

/// **Write `report.md` and `report.json` into `out`**, from the ledger in `labels` and the set DL-06 wrote into `out`.
///
/// `checks` is where DM-04's per-model calibrations are read from (the editor's own checks with the project's over them); a model with no record is
/// `waiting`, never an error. Nothing is written into `labels`.
pub fn report(labels: &Path, out: &Path, options: &Options, checks: &Checks) -> Result<Report, String> {
    let read = read_file(&labels.join(crate::turn_store::LEDGER_FILE))?;
    // **A torn line stops the report**, as it stops the export: a count made past a line nobody can read is a count of something else.
    if let Some(bad) = read.damaged.first() {
        return Err(format!("the ledger's line {} does not read, so no number here would be the ledger's: {}", bad.line, bad.why));
    }
    let set = Set::read(out, options)?;
    let report = of(&read, Some(&set), checks);
    write(&report, out)?;
    Ok(report)
}

impl Set {
    /// **What the export beside `out` says**, or nothing where there is none: a report of a ledger nobody exported is still a report.
    pub fn read(out: &Path, options: &Options) -> Result<Set, String> {
        let path = out.join(SUMMARY_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Set::default()),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        let said: serde_json::Value =
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let count = |at: &serde_json::Value| at.as_u64().and_then(|n| usize::try_from(n).ok()).unwrap_or(0);
        let counts = |at: &serde_json::Value| -> BTreeMap<String, usize> {
            at.as_object()
                .map(|map| map.iter().map(|(kind, n)| (kind.clone(), count(n))).collect())
                .unwrap_or_default()
        };
        // **An export from before `parked_by_record` existed** has none, which reads as no parked questions placed by record: the count is still
        // there under `parked`, and the report says so rather than inventing a week for each.
        let _ = options;
        Ok(Set {
            found: true,
            train: count(&said["train_records"]),
            eval: count(&said["eval_records"]),
            holdout_records: count(&said["holdout_records"]),
            images: count(&said["images"]),
            parked: counts(&said["parked"]),
            parked_by_record: counts(&said["parked_by_record"]),
            left_out: counts(&said["left_out"]),
            disagreements: counts(&said["disagreements"]),
            teacher_errors: count(&said["teacher_errors"]),
        })
    }
}

/// **The report's numbers**, from a ledger in hand and, where there is one, the set beside it. Writes nothing.
pub fn of(read: &Read, set: Option<&Set>, checks: &Checks) -> Report {
    let turns: Vec<&Turn> = read.turns().collect();
    let mut weeks: BTreeMap<String, Week> = BTreeMap::new();
    let mut totals = Totals::default();
    let mut questions: BTreeMap<String, QuestionRow> = BTreeMap::new();
    let mut sources: BTreeMap<String, SourceRow> = BTreeMap::new();

    // **Every label, by the turn it is about**, so a week and a question are each one lookup.
    let mut labels_of: BTreeMap<&str, Vec<&Label>> = BTreeMap::new();
    for label in read.labels() {
        labels_of.entry(label.turn.as_str()).or_default().push(label);
    }
    let mut assessments: BTreeMap<&str, &Assessment> = BTreeMap::new();
    for assessment in read.assessments() {
        let better = assessments.get(assessment.turn.as_str()).is_none_or(|held| assessment.assessor.version >= held.assessor.version);
        if better {
            assessments.insert(assessment.turn.as_str(), assessment);
        }
    }
    // **The fast model's newest line per turn stands**: a re-run after the question set changed is the newer answer.
    let mut fast_of: BTreeMap<&str, &FastAnswers> = BTreeMap::new();
    for fast in read.fast_answers() {
        fast_of.insert(fast.turn.as_str(), fast);
    }

    for turn in &turns {
        let week = monday_of(turn.handed_back_unix_ms / 1_000);
        let row = week_row(&mut weeks, &week);
        row.turns += 1;
        totals.turns += 1;

        // **The questions it asks**, each counted once, with the engine's answer where it has one.
        for item in &turn.questions.items {
            let entry = questions.entry(item.id.clone()).or_insert_with(|| QuestionRow {
                id: item.id.clone(),
                source: item.source.clone(),
                turns: 0,
                gold: 0,
                objective: 0,
                silver: 0,
                holdout: 0,
            });
            entry.turns += 1;
            totals.questions += 1;
            if let Some(engine) = &item.engine {
                entry.objective += 1;
                row.objective += 1;
                totals.objective += 1;
                let source = sources.entry(item.source.clone()).or_insert_with(|| SourceRow { source: item.source.clone(), ..SourceRow::default() });
                if engine.p_yes >= 0.5 {
                    source.objective_yes += 1;
                } else {
                    source.objective_no += 1;
                }
            }
        }

        // **The person's and the teacher's labels on it**, each in its tier, and the direction where the label is a yes or a no.
        let mut answered = false;
        for label in labels_of.get(turn.id.as_str()).map(Vec::as_slice).unwrap_or(&[]) {
            let direction = direction_of(label);
            let question = label.question.clone().unwrap_or_else(|| TURN_QUESTION_ID.to_owned());
            let source = questions.get(&question).map(|row| row.source.clone()).unwrap_or_else(|| "turn".to_owned());
            match label.tier.as_str() {
                "gold" => {
                    row.gold += 1;
                    totals.gold += 1;
                    let entry = questions.entry(question.clone()).or_insert_with(|| QuestionRow {
                        id: question.clone(),
                        source: source.clone(),
                        turns: 0,
                        gold: 0,
                        objective: 0,
                        silver: 0,
                        holdout: 0,
                    });
                    entry.gold += 1;
                    if label.holdout {
                        entry.holdout += 1;
                        totals.holdout += 1;
                    }
                    let name = source.clone();
                    let source = sources.entry(source).or_insert_with(|| SourceRow { source: name, ..SourceRow::default() });
                    match direction {
                        Some(true) => source.gold_yes += 1,
                        Some(false) => source.gold_no += 1,
                        None => {}
                    }
                }
                "silver" => {
                    row.silver += 1;
                    totals.silver += 1;
                    let entry = questions.entry(question.clone()).or_insert_with(|| QuestionRow {
                        id: question.clone(),
                        source: source.clone(),
                        turns: 0,
                        gold: 0,
                        objective: 0,
                        silver: 0,
                        holdout: 0,
                    });
                    entry.silver += 1;
                    let name = source.clone();
                    let source = sources.entry(source).or_insert_with(|| SourceRow { source: name, ..SourceRow::default() });
                    match direction {
                        Some(true) => source.silver_yes += 1,
                        Some(false) => source.silver_no += 1,
                        None => {}
                    }
                }
                _ => {}
            }
            answered |= direction.is_some();
        }
        if answered {
            row.answered += 1;
            totals.answered += 1;
        }
    }
    totals.dismissed = read.dismissals().count();

    // **The engine-made defects**: one objective label each, in the week the defect was made.
    for defect in read.perturbed() {
        let week = monday_of(defect.at_unix_ms / 1_000);
        let row = week_row(&mut weeks, &week);
        row.objective += 1;
        totals.objective += 1;
        let id = defect_question(&defect.defect);
        let entry = questions.entry(id.clone()).or_insert_with(|| QuestionRow {
            id: id.clone(),
            source: "check".to_owned(),
            turns: 0,
            gold: 0,
            objective: 0,
            silver: 0,
            holdout: 0,
        });
        // **The defect's own named check is the question it answers**, so a defect is one more objective answer to `check-floating` (or `clipping`, or
        // `out_of_area`) and the table's numbers add up to the totals'. It is not in the class balance: that is of the questions a turn asks, and a
        // defect answers a scene the engine made.
        entry.objective += 1;
    }

    // **The assessor against the person.**
    let mut assessor = Assessor {
        answered: assessments.values().flat_map(|assessment| assessment.answers.iter()).filter(|answer| answer.yes_fraction.is_some()).count(),
        undone: 0,
        said_fine_on_an_undo: 0,
        kept: 0,
        said_fine_on_a_keep: 0,
        holdout: Agreement::default(),
        bar: TEACHER_BAR,
        holdout_min: HOLDOUT_MIN,
        teacher_errors: set.map(|set| set.teacher_errors).unwrap_or(0),
    };
    for turn in &turns {
        let labels = labels_of.get(turn.id.as_str()).map(Vec::as_slice).unwrap_or(&[]);
        // **The turn's own verdict**: the person's latest Keep, Undo or revision of the hand-back.
        let verdict = labels
            .iter()
            .filter(|label| matches!(label.label.as_str(), "kept" | "undone" | "revised"))
            .max_by_key(|label| label.at_unix_ms);
        let Some(verdict) = verdict else { continue };
        // **Did the assessor say the result was fine**: the turn question, answered yes.
        let said_fine = assessments
            .get(turn.id.as_str())
            .and_then(|assessment| assessment.answers.iter().find(|answer| answer.id == TURN_QUESTION_ID))
            .and_then(|answer| answer.yes_fraction)
            .is_some_and(|fraction| fraction >= 0.5);
        match verdict.label.as_str() {
            "undone" => {
                assessor.undone += 1;
                if said_fine {
                    assessor.said_fine_on_an_undo += 1;
                }
            }
            _ => {
                assessor.kept += 1;
                if said_fine {
                    assessor.said_fine_on_a_keep += 1;
                }
            }
        }
    }
    // **The holdout**: each audit answer against the assessor's answer to that same question. A skip is no answer, and compared with nothing.
    for label in read.holdout() {
        let (Some(question), Some(person)) = (label.question.as_deref(), label.answer.as_deref().and_then(reading)) else { continue };
        let Some(teacher) = assessments
            .get(label.turn.as_str())
            .and_then(|assessment| assessment.answers.iter().find(|answer| answer.id == question))
            .and_then(|answer| answer.yes_fraction)
        else {
            continue;
        };
        assessor.holdout.questions += 1;
        if (teacher >= 0.5) == person {
            assessor.holdout.agreed += 1;
        }
    }

    // **The fast model against the person and the assessor.**
    let mut fast = Fast::default();
    for (turn, answers) in &fast_of {
        fast.turns += 1;
        fast.questions += answers.answers.len();
        let person = gold_of(labels_of.get(turn).map(Vec::as_slice).unwrap_or(&[]));
        let teacher = assessments.get(turn).copied();
        for answer in &answers.answers {
            let Some(mine) = answer.p_yes.map(|p| p >= 0.5) else { continue };
            if let Some(theirs) = person.get(&answer.id) {
                fast.with_gold.questions += 1;
                if *theirs == mine {
                    fast.with_gold.agreed += 1;
                }
            }
            if let Some(fraction) = teacher.and_then(|teacher| teacher.answers.iter().find(|one| one.id == answer.id)).and_then(|one| one.yes_fraction) {
                fast.with_assessor.questions += 1;
                if (fraction >= 0.5) == mine {
                    fast.with_assessor.agreed += 1;
                }
            }
        }
    }

    // **Escalation**, as the ledger's own lines say it was decided.
    let mut escalation = Escalation {
        open: gate_open(read),
        questions: 0,
        escalated: 0,
        saved: 0,
        share: ESCALATE_SHARE,
        bounds: (UNSURE_LOW, UNSURE_HIGH),
        gate: assessor.holdout,
        bar: TEACHER_BAR,
        holdout_min: HOLDOUT_MIN,
    };
    for answers in fast_of.values() {
        escalation.questions += answers.escalation.len();
        let asked = answers.escalation.iter().filter(|escalated| escalated.asked).count();
        escalation.escalated += asked;
        escalation.saved += answers.escalation.len() - asked;
    }

    // **The parked questions, in the week of the record they are about**: DL-06 names the record, and a record's turn names its week.
    let set = set.cloned().unwrap_or_default();
    for (record, count) in set.parked_by_record.clone() {
        let week = record
            .split(',')
            .map(str::trim)
            .find_map(|id| turns.iter().find(|turn| turn.id == id))
            .map(|turn| monday_of(turn.handed_back_unix_ms / 1_000));
        let Some(week) = week else { continue };
        week_row(&mut weeks, &week).parked += count;
    }

    let distil = distil(&assessor, &escalation, &totals);

    Report {
        weeks: weeks.into_values().collect(),
        summary: totals,
        questions: questions.into_values().collect(),
        sources: sources.into_values().collect(),
        assessor,
        fast,
        disagreements: set.disagreements.clone(),
        set,
        escalation,
        calibrations: calibrations(checks),
        distil,
    }
}

/// **A week's row**, made when it first has a record.
fn week_row<'a>(weeks: &'a mut BTreeMap<String, Week>, week: &str) -> &'a mut Week {
    weeks.entry(week.to_owned()).or_insert_with(|| Week {
        week: week.to_owned(),
        turns: 0,
        answered: 0,
        gold: 0,
        objective: 0,
        silver: 0,
        parked: 0,
    })
}

/// **Which way a label points**, where it is a yes or a no: a Keep is yes, an Undo or a revision is no, an audit answer is what they said, and a
/// skip is neither.
fn direction_of(label: &Label) -> Option<bool> {
    match label.label.as_str() {
        "kept" => Some(true),
        "undone" | "revised" => Some(false),
        AUDIT => label.answer.as_deref().and_then(reading),
        _ => None,
    }
}

fn reading(word: &str) -> Option<bool> {
    match word {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// **The person's answers to one turn's questions**, by question: what the fast model is measured against.
fn gold_of(labels: &[&Label]) -> BTreeMap<String, bool> {
    let mut out: BTreeMap<String, (u64, bool)> = BTreeMap::new();
    for label in labels.iter().filter(|label| label.tier == "gold") {
        let Some(direction) = direction_of(label) else { continue };
        let question = label.question.clone().unwrap_or_else(|| TURN_QUESTION_ID.to_owned());
        // The later answer to one question stands.
        let entry = out.entry(question).or_insert((label.at_unix_ms, direction));
        if label.at_unix_ms >= entry.0 {
            *entry = (label.at_unix_ms, direction);
        }
    }
    out.into_iter().map(|(question, (_, direction))| (question, direction)).collect()
}

/// **The question an engine-made defect's own named check answers**, by the defect's name in the ledger.
fn defect_question(defect: &str) -> String {
    let check = match defect {
        "float" => "floating",
        "clip" => "clipping",
        "outside_area" => "out_of_area",
        other => other,
    };
    format!("check-{check}")
}

/// **Whether the set is worth training on**, and in one sentence why.
fn distil(assessor: &Assessor, escalation: &Escalation, totals: &Totals) -> Distil {
    if totals.turns == 0 {
        return Distil { state: "off".to_owned(), why: "the ledger holds no turn yet, so there is nothing to distil from".to_owned() };
    }
    if assessor.holdout.questions < assessor.holdout_min {
        return Distil {
            state: "off".to_owned(),
            why: format!(
                "the assessor is not good enough to distil from yet: the holdout has {} answer(s) of the {} a bar can be measured on, so the teacher's ceiling is unknown and escalation stays off",
                assessor.holdout.questions, assessor.holdout_min
            ),
        };
    }
    if !assessor.meets_the_bar() {
        let share = assessor.holdout.share().unwrap_or(0.0);
        return Distil {
            state: "off".to_owned(),
            why: format!(
                "the assessor is not good enough to distil from: it agreed with the person on {share:.0}% of {} holdout questions, under the {:.0}% bar, so a student trained on it would learn its errors",
                assessor.holdout.questions,
                assessor.bar * 100.0
            ),
        };
    }
    let share = assessor.holdout.share().unwrap_or(0.0);
    let saved = if escalation.saved == 0 {
        "escalation is on, and nothing has been saved yet".to_owned()
    } else {
        format!("escalation saved {} of {} questions ({:.0}%)", escalation.saved, escalation.questions, escalation.saved as f64 / escalation.questions as f64 * 100.0)
    };
    Distil {
        state: "on".to_owned(),
        why: format!("the assessor agreed with the person on {share:.0}% of {} holdout questions, over the {:.0}% bar: {}", assessor.holdout.questions, assessor.bar * 100.0, saved),
    }
}

/// **One row per model DM-04 recorded a calibration for**, and a `waiting` row for each model that has none.
///
/// The models are the two the person may choose between (DM-03): the local one and the remote one. **A model with no record is `waiting`, never an
/// error**: DM-04's remote run waits on the owner's consent and a quota, and a report that failed on it would fail every day until then.
pub fn calibrations(checks: &Checks) -> Vec<CalibrationRow> {
    let mut rows: BTreeMap<String, CalibrationRow> = BTreeMap::new();
    for check in &checks.checks {
        for record in &check.calibrations {
            let row = rows.entry(record.model.clone()).or_insert_with(|| CalibrationRow {
                model: record.model.clone(),
                state: "measured".to_owned(),
                checks: Vec::new(),
                why: String::new(),
            });
            row.checks.push(MeasuredCheck {
                check: record.check.clone(),
                set_size: record.set_size,
                agreement: record.agreement,
                threshold: record.threshold,
                date: record.date.clone(),
            });
        }
    }
    // **The person may choose either model**, so both are always shown: one measured, one waiting.
    for (model, why) in [
        (LOCAL_MODEL_NAME, "the local model has no calibration on this project's checks yet"),
        (remote_name(), "the remote model has no calibration record yet: DM-04's remote run waits on the owner's consent that these editor clips may leave the LAN, and a quota that fits the set"),
    ] {
        rows.entry(model.to_owned()).or_insert_with(|| CalibrationRow {
            model: model.to_owned(),
            state: "waiting".to_owned(),
            checks: Vec::new(),
            why: format!("no calibration names it: {why}"),
        });
    }
    let mut rows: Vec<CalibrationRow> = rows.into_values().collect();
    // **A measured row says what it measured**, so the reason is filled once its checks are known.
    for row in rows.iter_mut().filter(|row| row.state == "measured") {
        let agreements: Vec<String> = row.checks.iter().map(|check| format!("{} {:.2} on {}", check.check, check.agreement, check.set_size)).collect();
        row.why = format!("measured: {}", agreements.join("; "));
    }
    rows
}

/// **The remote model's name as a person reads it**, without the provider prefix.
fn remote_name() -> &'static str {
    OPENROUTER_MODEL.split_once('/').map_or(OPENROUTER_MODEL, |(_, name)| name)
}

/// **The Monday of the week a moment falls in**, `YYYY-MM-DD`, UTC: the same week on every machine, whatever its clock's zone.
///
/// **Signed, and floored**, because the Monday of the first week of 1970 is three days *before* the epoch: a record stamped a second after the epoch
/// is a timestamp a fixture writes, and a subtraction that wrapped would name a week in the year 584 billion.
fn monday_of(secs: u64) -> String {
    let days = i64::try_from(secs / 86_400).unwrap_or(i64::MAX / 86_400);
    // 1970-01-01 was a Thursday, so the Monday of that week was three days before the epoch: `rem_euclid` floors, which is what a week needs.
    let since_monday = (days + 3).rem_euclid(7);
    let monday = (days - since_monday).max(0);
    civil_date_utc(u64::try_from(monday * 86_400).unwrap_or(0))
}

// ---------------------------------------------------------------------------------------------------------------
// The files.
// ---------------------------------------------------------------------------------------------------------------

/// **Write the report's two files into `out`**: the words a person reads, and the same numbers as data.
fn write(report: &Report, out: &Path) -> Result<(), String> {
    let json = serde_json::to_string_pretty(report).map_err(|e| format!("the report as JSON: {e}"))?;
    let path = out.join(REPORT_JSON);
    bevy_grimoire_core::atomic_file::write(&path, format!("{json}\n").as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
    let markdown = markdown(report);
    let path = out.join(REPORT_FILE);
    bevy_grimoire_core::atomic_file::write(&path, markdown.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))
}

/// The share as a person reads it, or that there is nothing to measure yet.
fn percent(agreement: &Agreement) -> String {
    match agreement.share() {
        Some(share) => format!("{:.0}% of {}", share * 100.0, agreement.questions),
        None => "nothing compared".to_owned(),
    }
}

/// **The report as words**: the same numbers in the order they decide anything.
pub fn markdown(report: &Report) -> String {
    let totals = &report.summary;
    let mut out = String::new();
    out.push_str("# The label report\n\n");
    out.push_str(&format!(
        "{} turn(s) recorded, {} answered. {} gold, {} objective, {} silver label(s), {} in the holdout.\n\n",
        totals.turns, totals.answered, totals.gold, totals.objective, totals.silver, totals.holdout
    ));

    // **The one verdict first.**
    out.push_str(&format!("## Distilling: {}\n\n{}\n\n", report.distil.state, report.distil.why));

    out.push_str("## Per week\n\n| Week | Turns | Answered | Gold | Objective | Silver | Parked |\n|---|---|---|---|---|---|---|\n");
    for week in &report.weeks {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            week.week, week.turns, week.answered, week.gold, week.objective, week.silver, week.parked
        ));
    }
    out.push('\n');

    let assessor = &report.assessor;
    out.push_str("## The assessor against the person\n\n");
    out.push_str(&format!("- {} question(s) answered.\n", assessor.answered));
    out.push_str(&format!(
        "- of {} turn(s) the person undid, it said the result was fine {} time(s).\n",
        assessor.undone, assessor.said_fine_on_an_undo
    ));
    out.push_str(&format!("- of {} turn(s) the person kept, it agreed {} time(s).\n", assessor.kept, assessor.said_fine_on_a_keep));
    out.push_str(&format!(
        "- on the human holdout: {} agreed ({}), against the {:.0}% bar over at least {} answers.\n",
        assessor.holdout.agreed,
        percent(&assessor.holdout),
        assessor.bar * 100.0,
        assessor.holdout_min
    ));
    out.push_str(&format!("- {} teacher error(s): answers that contradicted the one that stood.\n\n", assessor.teacher_errors));

    let fast = &report.fast;
    out.push_str("## The fast model\n\n");
    out.push_str(&format!("- {} turn(s), {} question(s) answered.\n", fast.turns, fast.questions));
    out.push_str(&format!("- with the person's gold answers: {} agreed.\n", percent(&fast.with_gold)));
    out.push_str(&format!("- with the assessor: {} agreed.\n\n", percent(&fast.with_assessor)));

    let escalation = &report.escalation;
    out.push_str("## Escalation\n\n");
    out.push_str(&format!(
        "{}, on {} of {} question(s): {} saved. Unsure means P(yes) between {:.2} and {:.2}, or a disagreement with the engine's own answer where it \
         has one; one question in {} goes anyway, so the assessor stays measured where the fast model was sure.\n\n",
        if escalation.open { "On" } else { "Off: every question goes to the assessor" },
        escalation.escalated,
        escalation.questions,
        escalation.saved,
        escalation.bounds.0,
        escalation.bounds.1,
        escalation.share
    ));

    out.push_str("## The class balance, per question source\n\n| Source | Gold yes | Gold no | Objective yes | Objective no | Silver yes | Silver no |\n|---|---|---|---|---|---|---|\n");
    for source in &report.sources {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            source.source, source.gold_yes, source.gold_no, source.objective_yes, source.objective_no, source.silver_yes, source.silver_no
        ));
    }
    out.push('\n');

    out.push_str("## Per question\n\n| Question | Source | Turns | Gold | Objective | Silver | Holdout |\n|---|---|---|---|---|---|---|\n");
    for question in &report.questions {
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            question.id, question.source, question.turns, question.gold, question.objective, question.silver, question.holdout
        ));
    }
    out.push('\n');

    let set = &report.set;
    out.push_str("## The set\n\n");
    if set.found {
        out.push_str(&format!("- {} train, {} eval, {} holdout record(s), {} picture(s).\n", set.train, set.eval, set.holdout_records, set.images));
        let parked: Vec<String> = set.parked.iter().map(|(kind, count)| format!("{count} {kind}")).collect();
        out.push_str(&format!("- parked: {}.\n", if parked.is_empty() { "nothing".to_owned() } else { parked.join(", ") }));
        let left: Vec<String> = set.left_out.iter().map(|(kind, count)| format!("{count} {kind}")).collect();
        out.push_str(&format!("- left out: {}.\n", if left.is_empty() { "nothing".to_owned() } else { left.join(", ") }));
        let disagreements: Vec<String> = set.disagreements.iter().map(|(kind, count)| format!("{count} {kind}")).collect();
        out.push_str(&format!("- disagreements: {}.\n\n", if disagreements.is_empty() { "none".to_owned() } else { disagreements.join(", ") }));
    } else {
        out.push_str("- no export beside this report: run `cauldron-labels export` first, and the set's numbers appear here.\n\n");
    }

    out.push_str("## The models' calibrations\n\n| Model | State | What |\n|---|---|---|\n");
    for row in &report.calibrations {
        out.push_str(&format!("| {} | {} | {} |\n", row.model, row.state, row.why));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// The line the Agent workspace shows.
// ---------------------------------------------------------------------------------------------------------------

/// **What the ledger holds, in one line's worth of numbers**: the workspace shows it, and it is the same count the report makes.
#[derive(Clone, Debug, PartialEq)]
pub struct LedgerLine {
    pub turns: usize,
    pub answered: usize,
    pub gold: usize,
    pub objective: usize,
    pub silver: usize,
    pub holdout: usize,
    /// The assessor's agreement on the holdout, where anything was compared.
    pub holdout_agreement: Option<f64>,
    pub bar: f64,
}

/// **The counts a line needs**, from a ledger in hand and nothing else: no export, no model, no file.
pub fn summary(read: &Read) -> LedgerLine {
    let labels: Vec<&Label> = read.labels().collect();
    let answered_turns: std::collections::BTreeSet<&str> =
        labels.iter().filter(|label| direction_of(label).is_some()).map(|label| label.turn.as_str()).collect();
    let mut holdout = Agreement::default();
    let mut assessments: BTreeMap<&str, &Assessment> = BTreeMap::new();
    for assessment in read.assessments() {
        assessments.insert(assessment.turn.as_str(), assessment);
    }
    for label in read.holdout() {
        let (Some(question), Some(person)) = (label.question.as_deref(), label.answer.as_deref().and_then(reading)) else { continue };
        let Some(teacher) = assessments
            .get(label.turn.as_str())
            .and_then(|assessment| assessment.answers.iter().find(|answer| answer.id == question))
            .and_then(|answer| answer.yes_fraction)
        else {
            continue;
        };
        holdout.questions += 1;
        if (teacher >= 0.5) == person {
            holdout.agreed += 1;
        }
    }
    LedgerLine {
        turns: read.turns().count(),
        answered: answered_turns.len(),
        gold: read.labels().filter(|label| label.tier == "gold").count(),
        objective: read.turns().flat_map(|turn| turn.questions.items.iter()).filter(|item| item.engine.is_some()).count(),
        silver: read.labels().filter(|label| label.tier == "silver").count(),
        holdout: read.holdout().count(),
        holdout_agreement: holdout.share(),
        bar: TEACHER_BAR,
    }
}

/// **The line the Agent workspace shows**, and the same words a person reads in `report.md`'s first paragraph.
pub fn line(summary: &LedgerLine) -> String {
    if summary.turns == 0 {
        return "no turn recorded yet, so there is nothing to report".to_owned();
    }
    let agreement = match summary.holdout_agreement {
        Some(share) => format!("{:.0}%", share * 100.0),
        None => "unmeasured".to_owned(),
    };
    format!(
        "{} turns ({} answered), gold {}, objective {}, silver {}, holdout {}; assessor {agreement} on the holdout, bar {:.2}",
        summary.turns, summary.answered, summary.gold, summary.objective, summary.silver, summary.holdout, summary.bar
    )
}

/// **Whether the assessor is worth distilling from yet**, from the ledger alone: the holdout's agreement reaches the bar over the fewest answers that
/// can show it. The one place the gate is decided, so the batch and the report cannot disagree about whether escalation is open.
pub fn gate_open(read: &Read) -> bool {
    let line = summary(read);
    read.holdout().filter(|label| label.question.is_some()).count() >= HOLDOUT_MIN
        && line.holdout_agreement.is_some_and(|share| share >= TEACHER_BAR)
}

/// **Whether a question goes to the slow assessor**, and why: the one place escalation is decided, so the ledger's line and the report's count can
/// never say different things.
///
/// `p_yes` is the fast model's probability, `against` the turn's gold or objective answer where it has one, and `seed` the question's own hash, so the
/// sampled share is a property of the question alone and two runs escalate the same questions.
pub fn escalate(p_yes: Option<f64>, against: Option<bool>, seed: u64) -> (bool, &'static str) {
    // **A question the fast model could not answer** is one the assessor must: a refusal is not an answer, and nothing may be saved on nothing.
    let Some(p) = p_yes else { return (true, "unsure") };
    if (UNSURE_LOW..=UNSURE_HIGH).contains(&p) {
        return (true, "unsure");
    }
    if against.is_some_and(|said| said != (p >= 0.5)) {
        return (true, "outvoted");
    }
    if seed.is_multiple_of(u64::from(ESCALATE_SHARE)) {
        return (true, "sampled");
    }
    (false, "sure")
}

/// **One question's seed for the sampled share**: its turn and question's own hash, so the share needs no run order.
pub fn seed_of(turn: &str, question: &str) -> u64 {
    let digest = crate::turn_store::sha256_hex(format!("{turn}\0{question}").as_bytes());
    u64::from_str_radix(&digest[..16], 16).unwrap_or(0)
}

/// **The turn's own answer to a question, where the engine computed one**: a named check's, from the map. What "outvoted" means.
///
/// **A person's audit answer is not here**: it is given in the editor's own keystrokes, after the turn's record was written, and the fast pass runs in
/// this batch where the person's answers are not yet part of the record. Their labels are the ceiling the report measures against, and never an input a
/// student's call may be steered by.
pub fn against(turn: &Turn, question: &str) -> Option<bool> {
    turn.questions.items.iter().find(|item| item.id == question).and_then(|item| item.engine.as_ref()).map(|engine| engine.p_yes >= 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The week a moment falls in is its Monday**, whatever the day.
    #[test]
    fn a_week_is_the_monday_it_starts_on() {
        // 2026-11-09 is a Monday; 2026-11-15 the Sunday after it, and 2026-11-16 the next Monday.
        assert_eq!(monday_of(1_794_182_400), "2026-11-09", "the Monday itself");
        assert_eq!(monday_of(1_794_700_800), "2026-11-09", "the Sunday of the same week");
        assert_eq!(monday_of(1_794_787_200), "2026-11-16", "the next Monday");
    }

    /// **Escalation is decided one way**: unsure, then outvoted, then the share, then not.
    #[test]
    fn escalation_is_decided_in_one_place() {
        assert_eq!(escalate(Some(0.5), None, 0), (true, "unsure"));
        assert_eq!(escalate(Some(0.35), None, 0), (true, "unsure"), "the bound is inside");
        assert_eq!(escalate(Some(0.65), None, 0), (true, "unsure"), "the bound is inside");
        assert_eq!(escalate(Some(0.9), Some(false), 1), (true, "outvoted"));
        assert_eq!(escalate(Some(0.9), Some(true), 1), (false, "sure"));
        assert_eq!(escalate(Some(0.9), Some(true), 5), (true, "sampled"), "one seed in {} is sampled", ESCALATE_SHARE);
        assert_eq!(escalate(None, None, 3), (true, "unsure"), "a refusal is not an answer, so nothing is saved on it");
    }
}
