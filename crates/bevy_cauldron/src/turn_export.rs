//! **The training set: the ledger's tiers resolved, written as Valen's JSONL, split by scene** (DL-06, #465; series labels from use, #459).
//!
//! The ledger (DL-01…DL-05) holds what happened and what is known about it, one line at a time, with no opinion about which of it a student may
//! learn from. This file is that opinion, and it is the only place the tiers are resolved: it reads a labels folder and writes the files a
//! trainer reads — `<out>/train.jsonl`, `<out>/eval.jsonl`, `<out>/holdout.jsonl` — with the pictures they name under `<out>/images/`, and
//! `<out>/summary.json` counting what was parked, contradicted and left out.
//!
//! # Which answer counts, per question
//!
//! 1. **Gold, the person's own, always wins**: a hand-back they kept, undid or fixed by hand (DL-02), and their answer to an audit question
//!    (DL-07). Where the person gave two answers to one question the later stands.
//! 2. **Objective, the engine's, answers its own checks** — the answer the engine computed when the turn was recorded (DL-03), and the fact a
//!    perturbation measured on the saved scene (DL-05). Both are hard: the engine is not guessing.
//! 3. **Silver, a teacher's, only where it can be corroborated**: the assessor's three votes all agreeing with each other (DL-04) is soft, the
//!    vote fraction itself; an assess-then-fix pair is one check flagged and then cleared, the assessor agreeing with itself (DL-05); and a lone
//!    silver vote (a later agent turn taking this one back) stands only where a gold or objective answer agrees with it.
//! 4. **Everything else is parked**, and counted with its reason ([`Parked`]). A parked question still goes into its record with
//!    `"probabilities": null`, which is how Valen's own smoke set writes a question with no label, and its reader skips it when training.
//! 5. **A gold answer that contradicts silver keeps the gold, and the disagreement is recorded** ([`Disagreement`]), which DL-08 counts: it is the
//!    teacher's error rate, and the student's ceiling. **Only a teacher's contradiction is a teacher error** ([`Disagreement::kind`]): a person
//!    overruling their own earlier answer, and two of the engine's own answers disagreeing, are written down too and counted apart.
//!
//! # What a record's state shows
//!
//! **Every picture the state has, in the order they were taken**: a question whose `about` is `change` (`check-out_of_area`, the shipped check
//! that asks what changed between the before and the after) is therefore trained on both, and one whose state has only one picture is parked
//! (`one-picture`) rather than answered from half its evidence. Every asset a record names is a picture it shows, and every picture it shows is an
//! asset, with its SHA-256.
//!
//! # A line that does not read stops the export
//!
//! The ledger reports a line it cannot parse with its number ([`Read::damaged`]), and **this refuses to export past one**, naming every line: a
//! torn line is a turn or a label nothing can account for, and a set that quietly left it out would look complete and not be.
//!
//! # The split is by scene, never by record
//!
//! A scene is a project and a map ([`scene_of`]), and a scene's turn, every perturbation of it and every assess-then-fix pair out of it share one
//! id, so none of them lands on the other side of the train / eval line from the rest: **a `group_id` is never in both files**. A scene goes to
//! eval when its own id hashes into that share (one scene in [`EVAL_SHARE`]), which is a property of the scene alone and so needs no record
//! order to repeat. **A benchmark record is eval**, and so is every other record of its scene, because a scene is never split. **The human holdout
//! (DL-07) is a file of its own** and never mixed into either: a question the person answered in the audit is graded on and never trained on.
//!
//! # A build whose pictures are of another interface
//!
//! A record is of an editor build ([`Turn::provenance`]), and a picture of an interface that has since changed teaches the wrong thing. Records
//! written by a build older than [`OLDEST_BUILD`] are left out, and counted ([`LeftOut`]), unless the caller asks for them
//! ([`Options::include_old_builds`]). A version that does not read as one is left out too: it cannot be shown to be new enough.
//!
//! # The export is derived, and byte-identical
//!
//! Nothing is written into the labels folder, and nothing about the machine, the time or the output path is in the files: two exports of one
//! ledger give the same bytes, which is what lets a report (DL-08) cite a set by what it is. The records are ordered by scene and then by the
//! state's words and pictures, the questions by id and the assets by path, so no order rests on the ledger's.

use std::collections::BTreeMap;
use std::path::Path;

use bevy_oracle::Question;
use serde_json::{Map, Value, json};

use crate::turn_questions::TURN_QUESTION_ID;
use crate::turn_store::{AUDIT, AssessFix, Assessment, Body, Label, Perturbed, Read, Shot, Store, TIER_OBJECTIVE, TIER_SILVER, Turn, scene_of, sha256_hex};

/// **The builds whose pictures are of the interface as it is.** A record written before it shows a UI that has changed, so it is left out unless
/// the caller asks for every build. It moves when the interface a record's pictures show changes, and it is named here rather than read off this
/// build, so a release made today still takes the records of the release before it.
pub const OLDEST_BUILD: &str = "0.6.8";

/// **One scene in this many goes to eval**, a property of the scene's own id and nothing else. Five is a fifth held back: enough to measure a
/// student on, and few enough that four fifths still teach.
pub const EVAL_SHARE: u32 = 5;

/// The training set's file, inside the export folder.
pub const TRAIN_FILE: &str = "train.jsonl";
/// The eval set's file, inside the export folder.
pub const EVAL_FILE: &str = "eval.jsonl";
/// The human holdout's file, inside the export folder: a student is graded on it and never trained on it.
pub const HOLDOUT_FILE: &str = "holdout.jsonl";
/// What the export did, in numbers, inside the export folder: the counts DL-08's report cites, with the parked questions indexed by the record they are
/// about (`parked_by_record`) so a report can place each in its week.
pub const SUMMARY_FILE: &str = "summary.json";

/// **Who an answer is from**, in the order that decides which stands. `Silver < Objective < Gold`, so the strongest answer is the greatest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Tier {
    Silver,
    Objective,
    Gold,
}

impl Tier {
    /// The word a record carries, which is `Label::tier`'s own spelling.
    pub fn name(self) -> &'static str {
        match self {
            Tier::Silver => TIER_SILVER,
            Tier::Objective => TIER_OBJECTIVE,
            Tier::Gold => "gold",
        }
    }
}

/// **How much stands behind a silver answer**, which is what decides whether it can be a target on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Evidence {
    /// The assessor's three votes all said the same: it stands.
    Unanimous,
    /// One check flagged a problem and a later answer cleared it: the assessor agreeing with itself (DL-05). It stands.
    Pair,
    /// The assessor's three votes were counted and split: never a target, and parked as `votes-split`.
    Split,
    /// The assessor answered fewer than three times (it declined, or gave an answer of another type): parked as `votes-refused`.
    Refusals,
    /// One silver label, and no more (a later agent turn taking this one back): it stands only where a gold or objective answer agrees.
    Single,
}

impl Evidence {
    /// **Whether it can be a target with nothing to corroborate it**: the three votes agreeing, or a check cleared after it was flagged.
    fn stands_alone(self) -> bool {
        matches!(self, Evidence::Unanimous | Evidence::Pair)
    }
}

/// **One answer to one question**, as a candidate for its target.
#[derive(Clone, Debug, PartialEq)]
enum Answer {
    /// Yes or no, with no doubt in it.
    Hard(bool),
    /// The fraction of the assessor's votes that said yes: a soft label.
    Soft(f64),
    /// How many of the assessor's votes each offered option got.
    Choice(BTreeMap<String, u32>),
}

impl Answer {
    /// Which way it leans, where it leans: what "agrees" is measured on.
    fn direction(&self) -> Option<bool> {
        match self {
            Answer::Hard(yes) => Some(*yes),
            Answer::Soft(fraction) => Some(*fraction >= 0.5),
            Answer::Choice(_) => None,
        }
    }

    /// The answer in words, for a disagreement a person reads.
    fn describe(&self) -> String {
        match self {
            Answer::Hard(true) => "yes".to_owned(),
            Answer::Hard(false) => "no".to_owned(),
            Answer::Soft(fraction) => format!("{fraction:.2} yes"),
            Answer::Choice(tally) => {
                let parts: Vec<String> = tally.iter().filter(|(_, count)| **count > 0).map(|(option, count)| format!("{option} {count}")).collect();
                parts.join(", ")
            }
        }
    }
}

/// **One answer offered for one question**, with where it came from.
#[derive(Clone, Debug, PartialEq)]
struct Candidate {
    tier: Tier,
    /// `person`, `engine`, `perturbation`, `assessor` or `superseded`: the record's `meta.questions` says which.
    source: &'static str,
    answer: Answer,
    /// The ledger record it came from, so a report can trace any number back.
    record: String,
    /// When it was given, and where its line is: of two answers by one person, the later stands.
    at_unix_ms: u64,
    order: usize,
    /// **The person's audit answer** (DL-07): gold, and the holdout, so it is graded on and never trained on.
    holdout: bool,
    /// The assessor's version, where an assessor answered.
    assessor_version: Option<u32>,
    build_commit: String,
    evidence: Evidence,
}

/// **One question a record asks**, with what the record needs to place it.
#[derive(Clone, Debug)]
struct Placed {
    /// The question in Valen's shape (`{"type": "noul", "instructions": ...}`).
    shape: Value,
    /// What the student looks at: `after`, or `change` when the question is about the difference between two pictures.
    about: String,
}

/// **The state a student is asked about**: the words above the pictures and the pictures in the order they are shown, by hash. Two records of
/// one state are one record in the set, as Valen's shared-state form is.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct State {
    text: String,
    pictures: Vec<(String, String)>,
}

impl State {
    /// A state from the words and the named pictures, keeping the ones that were taken.
    fn new(said: &str, pictures: &[(&str, &Shot)]) -> State {
        let taken: Vec<(String, String)> =
            pictures.iter().filter_map(|(name, shot)| shot.image.as_ref().map(|image| ((*name).to_owned(), image.sha256.clone()))).collect();
        let mut text = said.to_owned();
        if !taken.is_empty() {
            let names: Vec<&str> = taken.iter().map(|(name, _)| name.as_str()).collect();
            text.push_str(&format!(" The pictures, in order: {}.", names.join(", ")));
        }
        State { text, pictures: taken }
    }
}

/// **A state and everything known about it**: the questions it asks and every answer offered for them.
struct Gathered {
    /// `turn`, `perturbed` or `assess_fix`: the record kind that first put this state in the set.
    source: &'static str,
    /// The ledger records this state is made of.
    records: Vec<String>,
    cauldron_version: String,
    build_commit: String,
    bench: bool,
    questions: BTreeMap<String, Placed>,
    candidates: BTreeMap<String, Vec<Candidate>>,
}

impl Gathered {
    fn place(&mut self, id: &str, shape: &Value, about: &str) {
        self.questions.entry(id.to_owned()).or_insert_with(|| Placed { shape: shape.clone(), about: about.to_owned() });
    }

    fn offer(&mut self, id: &str, candidate: Candidate) {
        self.candidates.entry(id.to_owned()).or_default().push(candidate);
    }
}

/// **A question no trusted answer reached**, with the reason: parked and counted, never silently dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parked {
    /// The ledger records the state is made of.
    pub record: String,
    pub question: String,
    /// A short word for a count: `no-answer`, `votes-split`, `one-vote`, `not-a-distribution`, `one-picture`.
    pub kind: &'static str,
    /// The sentence a person reads.
    pub why: String,
}

/// **Two answers to one question that disagreed**, with the one that stood: what DL-08 counts, and what it must not count as the teacher's error.
///
/// [`Disagreement::kind`] says which it is. **Only `teacher` is a teacher's error**: an assessor that contradicted the answer that stood. The other
/// kinds are written down and counted apart, because they say nothing about the assessor: `person` is the person's own earlier answer overruled
/// by their later one, and `engine` is two of the engine's own answers (or one engine answer overruling a perturbation's) disagreeing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Disagreement {
    pub record: String,
    pub question: String,
    /// `teacher` (silver against what stood: DL-08's error count), `person` or `engine`.
    pub kind: &'static str,
    /// The tier that stood.
    pub kept: Tier,
    /// What it said.
    pub kept_answer: String,
    /// What the answer that lost said.
    pub silver_answer: String,
}

impl Disagreement {
    /// **Whether this is the assessor being wrong**, which is what the teacher's error rate is made of.
    pub fn is_teacher_error(&self) -> bool {
        self.kind == "teacher"
    }
}

/// **Something the set would have needed that was not there**, with the reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeftOut {
    /// The ledger record, or the state it is of.
    pub record: String,
    /// `old-build`, `no-source-turn`, `no-question`, `unreadable-question`, `unreadable-picture`, `no-target`.
    pub kind: &'static str,
    pub why: String,
}

/// **What an export did.**
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Exported {
    /// The files written, and how many records each holds. A file with no records is not written, because Valen's reader refuses an empty
    /// dataset.
    pub files: Vec<(std::path::PathBuf, usize)>,
    pub train: usize,
    pub eval: usize,
    pub holdout: usize,
    /// The pictures written under `images/`.
    pub images: usize,
    pub parked: Vec<Parked>,
    pub disagreements: Vec<Disagreement>,
    pub left_out: Vec<LeftOut>,
}

impl Exported {
    /// The parked questions, counted by kind.
    pub fn parked_by_kind(&self) -> BTreeMap<&'static str, usize> {
        counts(self.parked.iter().map(|parked| parked.kind))
    }

    /// The records left out, counted by kind.
    pub fn left_out_by_kind(&self) -> BTreeMap<&'static str, usize> {
        counts(self.left_out.iter().map(|left| left.kind))
    }

    /// **The disagreements, counted by kind**: `teacher` is the assessor's error count, which DL-08 divides by the questions it answered.
    pub fn disagreements_by_kind(&self) -> BTreeMap<&'static str, usize> {
        counts(self.disagreements.iter().map(|disagreement| disagreement.kind))
    }

    /// **How often the assessor was wrong**: the `teacher` disagreements alone, never a person's own change of mind.
    pub fn teacher_errors(&self) -> usize {
        self.disagreements.iter().filter(|disagreement| disagreement.is_teacher_error()).count()
    }
}

fn counts(kinds: impl Iterator<Item = &'static str>) -> BTreeMap<&'static str, usize> {
    let mut out = BTreeMap::new();
    for kind in kinds {
        *out.entry(kind).or_insert(0) += 1;
    }
    out
}

/// **What an export is allowed to do**: which builds are new enough, and how a scene lands on one side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub eval_share: u32,
    pub oldest_build: String,
    pub include_old_builds: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options { eval_share: EVAL_SHARE, oldest_build: OLDEST_BUILD.to_owned(), include_old_builds: false }
    }
}

/// **Export the ledger in `labels` into `out`.** Reads `labels` and writes nothing into it.
pub fn export(labels: &Path, out: &Path, options: &Options) -> Result<Exported, String> {
    check(options)?;
    let read = crate::turn_store::read_file(&labels.join(crate::turn_store::LEDGER_FILE))?;
    let store = Store::new(labels.to_path_buf());
    export_read(&read, &store, out, options)
}

/// **Export a ledger already in hand.** `store` is the labels folder the pictures are read from; nothing is written into it.
pub fn export_read(read: &Read, store: &Store, out: &Path, options: &Options) -> Result<Exported, String> {
    check(options)?;
    // **A line that does not read is refused, not skipped** (as `cauldron-negatives` refuses one): a torn line is a label nothing can be trusted
    // to be missing or present, and an export that quietly drops a turn would make a set that looks complete and is not.
    if let Some(bad) = read.damaged.first() {
        let lines: Vec<String> = read.damaged.iter().map(|damaged| format!("{} ({})", damaged.line, damaged.why)).collect();
        return Err(format!("the ledger's line {} does not read, so the set would silently leave out what it holds: {}", bad.line, lines.join("; ")));
    }
    let ledger = Ledger::of(read, options);
    let (gathered, left_out) = ledger.gather();
    emit(&gathered, left_out, store, out, options)
}

/// **The sentences an export refuses on its options**, before it reads anything: a share of zero has no eval side, and an `--oldest` that is not a
/// version cannot be compared with one, so it would admit every build rather than the ones asked for.
fn check(options: &Options) -> Result<(), String> {
    if options.eval_share == 0 {
        return Err("the eval share is one scene in how many, so it cannot be 0".to_owned());
    }
    if !options.include_old_builds && parse_version(&options.oldest_build).is_none() {
        return Err(format!(
            "`{}` is not a version for the oldest build to keep: it reads as three numbers (`0.6.8`), and an oldest build that cannot be read would let every build through",
            options.oldest_build
        ));
    }
    Ok(())
}

/// **The ledger, indexed the ways the tiers need it.**
struct Ledger<'a> {
    turns: BTreeMap<&'a str, &'a Turn>,
    /// Every label on a turn, by turn, with its line number.
    labels: BTreeMap<&'a str, Vec<(usize, &'a Label)>>,
    /// The assessment this export reads, per turn: the newest assessor version, and the line it is on.
    assessments: BTreeMap<&'a str, (usize, &'a Assessment)>,
    perturbed: Vec<(usize, &'a Perturbed)>,
    pairs: Vec<(usize, &'a AssessFix)>,
    options: &'a Options,
}

impl<'a> Ledger<'a> {
    fn of(read: &'a Read, options: &'a Options) -> Ledger<'a> {
        let mut ledger = Ledger { turns: BTreeMap::new(), labels: BTreeMap::new(), assessments: BTreeMap::new(), perturbed: Vec::new(), pairs: Vec::new(), options };
        for (order, line) in &read.lines {
            match &line.body {
                Body::Turn(turn) => {
                    ledger.turns.insert(turn.id.as_str(), turn.as_ref());
                }
                Body::Label(label) => ledger.labels.entry(label.turn.as_str()).or_default().push((*order, label.as_ref())),
                Body::Assessment(assessment) => {
                    let better = match ledger.assessments.get(assessment.turn.as_str()) {
                        Some((_, held)) => assessment.assessor.version >= held.assessor.version,
                        None => true,
                    };
                    if better {
                        ledger.assessments.insert(assessment.turn.as_str(), (*order, assessment.as_ref()));
                    }
                }
                Body::Perturbed(perturbed) => ledger.perturbed.push((*order, perturbed.as_ref())),
                Body::AssessFix(pair) => ledger.pairs.push((*order, pair.as_ref())),
                // **The fast model's own answer is never a label** (DL-08, #467): reading it here would let a student be trained on its own guess, and
                // the escalation decision is the report's business, not the set's.
                Body::FastAnswers(_) | Body::Dismissed(_) => {}
            }
        }
        ledger
    }

    /// **Whether a record's build is new enough to teach from.** A version that is not one cannot be shown to be.
    fn new_enough(&self, version: &str) -> Result<(), LeftOut> {
        if self.options.include_old_builds {
            return Ok(());
        }
        // **The oldest is a version**, refused by [`check`] before any ledger is read, so a record is never admitted by a misread bound.
        let Some(oldest) = parse_version(&self.options.oldest_build) else {
            return Err(LeftOut {
                record: String::new(),
                kind: "bad-oldest",
                why: format!("the oldest build `{}` is not a version, so no build can be compared with it", self.options.oldest_build),
            });
        };
        match parse_version(version) {
            Some(built) if built >= oldest => Ok(()),
            Some(_) => Err(LeftOut {
                record: String::new(),
                kind: "old-build",
                why: format!("its build is {version}, older than the {} whose pictures are of this interface", self.options.oldest_build),
            }),
            None => Err(LeftOut {
                record: String::new(),
                kind: "old-build",
                why: format!("its build version `{version}` is not a version, so it cannot be shown to be new enough"),
            }),
        }
    }

    /// **Every state the ledger offers**, with its questions and the answers offered for them, and the records left out.
    fn gather(&self) -> (BTreeMap<(String, State), Gathered>, Vec<LeftOut>) {
        let mut gathered: BTreeMap<(String, State), Gathered> = BTreeMap::new();
        let mut left_out: Vec<LeftOut> = Vec::new();
        let gated = |id: &str, version: &str, left_out: &mut Vec<LeftOut>| match self.new_enough(version) {
            Ok(()) => true,
            Err(mut why) => {
                why.record = id.to_owned();
                left_out.push(why);
                false
            }
        };
        for turn in self.turns.values() {
            if gated(&turn.id, &turn.provenance.cauldron_version, &mut left_out) {
                self.gather_turn(turn, &mut gathered);
            }
        }
        for (order, record) in &self.perturbed {
            if gated(&record.id, &record.provenance.cauldron_version, &mut left_out) {
                self.gather_perturbed(*order, record, &mut gathered);
            }
        }
        for (order, pair) in &self.pairs {
            let Some(turn) = self.turns.get(pair.source_turn.as_str()) else {
                left_out.push(LeftOut {
                    record: pair.id.clone(),
                    kind: "no-source-turn",
                    why: format!("its source turn {} is not in the ledger, so its build and its questions are unknown", pair.source_turn),
                });
                continue;
            };
            if gated(&pair.id, &turn.provenance.cauldron_version, &mut left_out) {
                self.gather_pair(*order, pair, turn, &mut gathered, &mut left_out);
            }
        }
        (gathered, left_out)
    }

    /// **One turn's result, with its questions**: the after picture the questions are about, and the before it is compared with.
    fn gather_turn(&self, turn: &'a Turn, gathered: &mut BTreeMap<(String, State), Gathered>) {
        let scene = scene_of(turn);
        let state = State::new(&task_text(turn), &[("before", &turn.before), ("after", &turn.after)]);
        let mut one = Gathered {
            source: "turn",
            records: vec![turn.id.clone()],
            cauldron_version: turn.provenance.cauldron_version.clone(),
            build_commit: turn.provenance.build_commit.clone(),
            bench: turn.bench,
            questions: BTreeMap::new(),
            candidates: BTreeMap::new(),
        };
        for item in &turn.questions.items {
            one.place(&item.id, &item.question, &item.about);
            if let Some(engine) = &item.engine {
                one.offer(
                    &item.id,
                    Candidate {
                        tier: Tier::Objective,
                        source: "engine",
                        answer: Answer::Hard(engine.p_yes >= 0.5),
                        record: turn.id.clone(),
                        at_unix_ms: turn.handed_back_unix_ms,
                        order: 0,
                        holdout: false,
                        assessor_version: None,
                        build_commit: turn.provenance.build_commit.clone(),
                        evidence: Evidence::Unanimous,
                    },
                );
            }
        }
        self.offer_person(&mut one, turn);
        self.offer_assessor(&mut one, turn);
        merge(gathered, (scene, state), one);
    }

    /// **What the person said**, which always wins: their verdict on the hand-back (the turn question) and their audit answers, which name the
    /// question they answered.
    fn offer_person(&self, one: &mut Gathered, turn: &'a Turn) {
        let Some(labels) = self.labels.get(turn.id.as_str()) else {
            return;
        };
        for (line, label) in labels {
            let answer = match (label.label.as_str(), label.answer.as_deref()) {
                ("kept", _) => Some(Answer::Hard(true)),
                ("undone", _) => Some(Answer::Hard(false)),
                // Kept, then fixed by hand: the result as handed back was not what they wanted.
                ("revised", _) => Some(Answer::Hard(false)),
                (AUDIT, Some("yes")) => Some(Answer::Hard(true)),
                (AUDIT, Some("no")) => Some(Answer::Hard(false)),
                // A skip is a skip, never a no.
                _ => None,
            };
            let Some(answer) = answer else { continue };
            let audit = label.label == AUDIT;
            let question = if audit { label.question.clone() } else { Some(TURN_QUESTION_ID.to_owned()) };
            let Some(question) = question else { continue };
            one.offer(
                &question,
                Candidate {
                    tier: Tier::Gold,
                    source: "person",
                    answer,
                    record: format!("{}#L{line}", turn.id),
                    at_unix_ms: label.at_unix_ms,
                    order: *line,
                    holdout: audit && label.holdout,
                    assessor_version: None,
                    build_commit: turn.provenance.build_commit.clone(),
                    evidence: Evidence::Unanimous,
                },
            );
        }
        // **The agent corrected itself** (DL-02): silver, and one vote, so it stands only where a kept answer agrees with it.
        for (line, label) in labels.iter().filter(|(_, label)| label.label == "superseded") {
            one.offer(
                TURN_QUESTION_ID,
                Candidate {
                    tier: Tier::Silver,
                    source: "superseded",
                    answer: Answer::Hard(false),
                    record: format!("{}#L{line}", turn.id),
                    at_unix_ms: label.at_unix_ms,
                    order: *line,
                    holdout: false,
                    assessor_version: None,
                    build_commit: turn.provenance.build_commit.clone(),
                    evidence: Evidence::Single,
                },
            );
        }
    }

    /// **What the assessor said**, per question: its vote fraction for a yes/no, its tally for a choice.
    fn offer_assessor(&self, one: &mut Gathered, turn: &'a Turn) {
        let Some((order, assessment)) = self.assessments.get(turn.id.as_str()) else {
            return;
        };
        for answered in &assessment.answers {
            let Some(placed) = one.questions.get(&answered.id) else {
                continue;
            };
            let Ok(question) = serde_json::from_value::<Question>(placed.shape.clone()) else {
                continue;
            };
            let valid = answered.votes.iter().filter(|vote| vote.refused.is_none()).count();
            let (answer, evidence) = match &question {
                Question::Noul { .. } => {
                    let Some(fraction) = answered.yes_fraction else {
                        continue;
                    };
                    let agreeing = answered.votes.iter().filter(|vote| vote.refused.is_none()).all(|vote| vote.p_yes.is_some_and(|p| (p >= 0.5) == (fraction >= 0.5)));
                    let evidence = if valid < crate::turn_assess::VOTES {
                        Evidence::Refusals
                    } else if agreeing {
                        Evidence::Unanimous
                    } else {
                        Evidence::Split
                    };
                    (Answer::Soft(fraction), evidence)
                }
                Question::Choice { .. } => {
                    let counted: u32 = answered.tally.values().sum();
                    let options = answered.tally.values().filter(|count| **count > 0).count();
                    let evidence = if counted < crate::turn_assess::VOTES as u32 {
                        Evidence::Refusals
                    } else if options == 1 {
                        Evidence::Unanimous
                    } else {
                        Evidence::Split
                    };
                    (Answer::Choice(answered.tally.clone()), evidence)
                }
                // A score's votes are not kept as a distribution: `AssessedQuestion` holds no probabilities for one.
                Question::Score { .. } => continue,
            };
            one.offer(
                &answered.id,
                Candidate {
                    tier: Tier::Silver,
                    source: "assessor",
                    answer,
                    record: format!("{}#L{order}", turn.id),
                    at_unix_ms: assessment.at_unix_ms,
                    order: *order,
                    holdout: false,
                    assessor_version: Some(assessment.assessor.version),
                    build_commit: turn.provenance.build_commit.clone(),
                    evidence,
                },
            );
        }
    }

    /// **One engine-made defect**: the scene the person kept as a positive and the defective one as a negative, both objective.
    fn gather_perturbed(&self, order: usize, record: &'a Perturbed, gathered: &mut BTreeMap<(String, State), Gathered>) {
        let turn = self.turns.get(record.source_turn.as_str()).copied();
        let said = turn.map(task_text).unwrap_or_else(|| "The task: the scene the person kept.".to_owned());
        let check = defect_check(&record.defect);
        let base = |state: State| {
            (
                state,
                Gathered {
                    source: "perturbed",
                    records: vec![record.id.clone()],
                    cauldron_version: record.provenance.cauldron_version.clone(),
                    build_commit: record.provenance.build_commit.clone(),
                    bench: record.bench,
                    questions: BTreeMap::new(),
                    candidates: BTreeMap::new(),
                },
            )
        };
        let candidate = |found: bool| Candidate {
            tier: Tier::Objective,
            source: "perturbation",
            answer: Answer::Hard(found),
            record: record.id.clone(),
            at_unix_ms: record.at_unix_ms,
            order,
            holdout: false,
            assessor_version: None,
            build_commit: record.provenance.build_commit.clone(),
            evidence: Evidence::Unanimous,
        };
        // **The scene the person kept**: a result that is right, and a scene with no defect in it.
        let (state, mut positive) = base(State::new(&said, &[("after", &record.before)]));
        if let Some(turn) = turn {
            for item in &turn.questions.items {
                positive.place(&item.id, &item.question, &item.about);
            }
        }
        positive.offer(TURN_QUESTION_ID, candidate(true));
        if let Some(check) = &check {
            positive.offer(check, candidate(false));
        }
        merge(gathered, (record.scene.clone(), state), positive);

        // **The defective scene**: not a result the person would keep, and the check the defect is.
        let (state, mut negative) = base(State::new(&said, &[("before", &record.before), ("after", &record.after)]));
        if let Some(turn) = turn {
            for item in &turn.questions.items {
                negative.place(&item.id, &item.question, &item.about);
            }
        }
        negative.offer(TURN_QUESTION_ID, candidate(false));
        if let Some(check) = &check {
            negative.offer(check, candidate(true));
        }
        merge(gathered, (record.scene.clone(), state), negative);
    }

    /// **A check flagged a problem and a later answer cleared it**: the flagged frame is a negative and the cleared one a positive, both silver.
    fn gather_pair(&self, order: usize, pair: &'a AssessFix, turn: &'a Turn, gathered: &mut BTreeMap<(String, State), Gathered>, left_out: &mut Vec<LeftOut>) {
        let id = format!("check-{}", pair.check);
        let Some(item) = turn.questions.items.iter().find(|item| item.id == id) else {
            left_out.push(LeftOut {
                record: pair.id.clone(),
                kind: "no-question",
                why: format!("its source turn asks no question `{id}`, so there is no question for the pair's frames to answer"),
            });
            return;
        };
        for (shot, found) in [(&pair.flagged.frame, true), (&pair.cleared.frame, false)] {
            let state = State::new(&task_text(turn), &[("after", shot)]);
            let mut one = Gathered {
                source: "assess_fix",
                records: vec![pair.id.clone()],
                cauldron_version: turn.provenance.cauldron_version.clone(),
                build_commit: turn.provenance.build_commit.clone(),
                bench: pair.bench,
                questions: BTreeMap::new(),
                candidates: BTreeMap::new(),
            };
            one.place(&item.id, &item.question, &item.about);
            one.offer(
                &item.id,
                Candidate {
                    tier: Tier::Silver,
                    source: "assessor",
                    answer: Answer::Hard(found),
                    record: pair.id.clone(),
                    at_unix_ms: turn.handed_back_unix_ms,
                    order,
                    holdout: false,
                    assessor_version: None,
                    build_commit: turn.provenance.build_commit.clone(),
                    evidence: Evidence::Pair,
                },
            );
            merge(gathered, (pair.scene.clone(), state), one);
        }
    }
}

/// **Where a defect's own named check is the question it answers**, by the defect's name in the ledger.
fn defect_check(defect: &str) -> Option<String> {
    let check = match defect {
        "float" => "floating",
        "clip" => "clipping",
        "outside_area" => "out_of_area",
        _ => return None,
    };
    Some(format!("check-{check}"))
}

/// **The words above a turn's pictures**: the task as the agent handed it back.
fn task_text(turn: &Turn) -> String {
    let request = &turn.request;
    let mut said = format!("The task: {}. {}", request.title, request.summary);
    if !request.specified.is_empty() {
        said.push_str(&format!(" The person asked for: {}.", request.specified.join("; ")));
    }
    said
}

/// **Add one record's state to the set**, merging with what is already known about the same state: one row per state, as Valen's shared-state
/// form is.
fn merge(gathered: &mut BTreeMap<(String, State), Gathered>, key: (String, State), one: Gathered) {
    match gathered.entry(key) {
        std::collections::btree_map::Entry::Vacant(slot) => {
            slot.insert(one);
        }
        std::collections::btree_map::Entry::Occupied(mut slot) => {
            let held = slot.get_mut();
            for (id, placed) in one.questions {
                held.place(&id, &placed.shape, &placed.about);
            }
            for (id, candidates) in one.candidates {
                for candidate in candidates {
                    held.offer(&id, candidate);
                }
            }
            for record in one.records {
                if !held.records.contains(&record) {
                    held.records.push(record);
                }
            }
        }
    }
}

/// **What one question's target is**, once every answer for it has been weighed.
enum Outcome {
    /// The answer that stood.
    Target(Candidate),
    /// No trusted answer: parked, with its reason.
    Parked(&'static str, String),
}

/// **Resolve one question**, recording every disagreement with an answer that stood.
fn resolve(candidates: &[Candidate], disagreements: &mut Vec<Disagreement>, record: &str, question: &str) -> Outcome {
    let of = |tier: Tier| -> Vec<Candidate> {
        let mut of: Vec<Candidate> = candidates.iter().filter(|c| c.tier == tier).cloned().collect();
        of.sort_by_key(|c| (c.at_unix_ms, c.order));
        of
    };
    let gold = of(Tier::Gold);
    let objective = of(Tier::Objective);
    let silver: Vec<&Candidate> = candidates.iter().filter(|c| c.tier == Tier::Silver).collect();

    // **The strongest answer that stands**: the person's latest, else the engine's where it is of one voice, else a corroborated teacher's.
    let kept: Option<Candidate> = if let (Some(first), Some(last)) = (gold.first(), gold.last()) {
        if first.answer.direction().is_some() && first.answer.direction() != last.answer.direction() {
            // Two of the person's own answers to one question disagree: the later stands, and says so.
            disagreements.push(Disagreement {
                record: record.to_owned(),
                question: question.to_owned(),
                kind: "person",
                kept: Tier::Gold,
                kept_answer: last.answer.describe(),
                silver_answer: first.answer.describe(),
            });
        }
        Some(last.clone())
    } else if !objective.is_empty() {
        let first = &objective[0];
        if let Some(other) = objective.iter().find(|other| other.answer.direction() != first.answer.direction()) {
            // Two of the engine's own answers to one question disagree: neither outranks the other, so neither stands, and both are written down.
            disagreements.push(Disagreement {
                record: record.to_owned(),
                question: question.to_owned(),
                kind: "engine",
                kept: Tier::Objective,
                kept_answer: other.answer.describe(),
                silver_answer: first.answer.describe(),
            });
            return Outcome::Parked("votes-split", "the engine's own answers to this question disagreed, so neither of them stands".to_owned());
        }
        Some(objective[objective.len() - 1].clone())
    } else {
        // **A teacher's answer stands where its own votes agree**, and where every teacher's answer of the same state agrees with it.
        let mut usable: Vec<Candidate> = silver.iter().filter(|c| c.evidence.stands_alone()).map(|c| (*c).clone()).collect();
        usable.sort_by_key(|c| (c.at_unix_ms, c.order));
        let one = usable.iter().find(|c| c.evidence == Evidence::Unanimous).or_else(|| usable.first()).cloned();
        match one {
            Some(one) if usable.iter().all(|other| other.answer.direction() == one.answer.direction()) => Some(one),
            _ => None,
        }
    };

    // **A silver answer is used where it agrees with what stood, and recorded where it does not**: the teacher's error rate, which DL-08 counts.
    if let Some(standing) = &kept {
        for candidate in &silver {
            let (Some(mine), Some(theirs)) = (candidate.answer.direction(), standing.answer.direction()) else {
                continue;
            };
            if mine != theirs {
                disagreements.push(Disagreement {
                    record: record.to_owned(),
                    question: question.to_owned(),
                    kind: "teacher",
                    kept: standing.tier,
                    kept_answer: standing.answer.describe(),
                    silver_answer: candidate.answer.describe(),
                });
            }
        }
        return Outcome::Target(standing.clone());
    }
    // **Nothing stands**: a lone label has nothing to corroborate it, split votes are not an answer, and no answer is no answer.
    let speaking: Vec<&Candidate> = silver.iter().copied().filter(|candidate| candidate.answer.direction().is_some()).collect();
    if speaking.is_empty() {
        return Outcome::Parked("no-answer", "no one answered it: no person's verdict or audit answer, no engine answer, and no assessor's".to_owned());
    }
    if speaking.iter().any(|candidate| candidate.evidence == Evidence::Split) {
        return Outcome::Parked("votes-split", "the assessor's three votes did not agree with each other, and no gold or engine answer agrees with them".to_owned());
    }
    if speaking.iter().any(|candidate| candidate.evidence == Evidence::Refusals) {
        return Outcome::Parked("votes-refused", "the assessor declined some of its three votes, so the share it gave is not an answer".to_owned());
    }
    Outcome::Parked("one-vote", "a silver label is one vote, and no gold or engine answer agrees with it".to_owned())
}

/// **Write the set**: resolve every question, make the records, copy the pictures, and put each record in the file its scene belongs to.
fn emit(gathered: &BTreeMap<(String, State), Gathered>, mut left_out: Vec<LeftOut>, store: &Store, out: &Path, options: &Options) -> Result<Exported, String> {
    let mut exported = Exported::default();
    let mut train: Vec<String> = Vec::new();
    let mut eval: Vec<String> = Vec::new();
    let mut holdout: Vec<String> = Vec::new();
    let mut assets: BTreeMap<String, String> = BTreeMap::new();
    // **A scene is whole**: a benchmark record puts its whole scene in eval, so its other records go with it.
    let mut bench_scenes: BTreeMap<&str, bool> = BTreeMap::new();
    for ((scene, _), one) in gathered {
        *bench_scenes.entry(scene.as_str()).or_insert(false) |= one.bench;
    }

    for ((scene, state), one) in gathered {
        let records = one.records.join(", ");
        let mut questions = Map::new();
        let mut main_targets = Map::new();
        let mut holdout_targets = Map::new();
        let mut provenance = Map::new();
        for (id, placed) in &one.questions {
            let Ok(question) = serde_json::from_value::<Question>(placed.shape.clone()) else {
                left_out.push(LeftOut {
                    record: records.clone(),
                    kind: "unreadable-question",
                    why: format!("the question `{id}` is not one of the three shapes a student is asked"),
                });
                continue;
            };
            if let Err(why) = valen_shape(&question) {
                left_out.push(LeftOut {
                    record: records.clone(),
                    kind: "unreadable-question",
                    why: format!("the question `{id}` is not one Valen's reader takes: {why}"),
                });
                continue;
            }
            questions.insert(id.clone(), placed.shape.clone());
            let offered = one.candidates.get(id).cloned().unwrap_or_default();
            // **A question about the change needs both pictures**: asked of a state that has one, it would be trained on half its evidence.
            if placed.about == "change" && state.pictures.len() < 2 {
                exported.parked.push(Parked {
                    record: records.clone(),
                    question: id.clone(),
                    kind: "one-picture",
                    why: format!("it asks what changed between the before and the after, and this state shows {} picture(s)", state.pictures.len()),
                });
                main_targets.insert(id.clone(), json!({"probabilities": null}));
                continue;
            }
            match resolve(&offered, &mut exported.disagreements, &records, id) {
                Outcome::Target(standing) => {
                    let Some(probabilities) = probabilities(&question, &standing.answer) else {
                        let (kind, why) = parked_why(&offered, &question);
                        exported.parked.push(Parked { record: records.clone(), question: id.clone(), kind, why });
                        main_targets.insert(id.clone(), json!({"probabilities": null}));
                        continue;
                    };
                    let mut said = Map::new();
                    said.insert("tier".to_owned(), json!(standing.tier.name()));
                    said.insert("source".to_owned(), json!(standing.source));
                    said.insert("assessor_version".to_owned(), json!(standing.assessor_version));
                    said.insert("build_commit".to_owned(), json!(standing.build_commit));
                    said.insert("record".to_owned(), json!(standing.record));
                    provenance.insert(id.clone(), Value::Object(said));
                    // **The person's audit answer is the holdout** (DL-07): never in train, and always in its own file.
                    if standing.holdout {
                        holdout_targets.insert(id.clone(), json!({"probabilities": probabilities}));
                        main_targets.insert(id.clone(), json!({"probabilities": null}));
                    } else {
                        main_targets.insert(id.clone(), json!({"probabilities": probabilities}));
                    }
                }
                Outcome::Parked(kind, why) => {
                    exported.parked.push(Parked { record: records.clone(), question: id.clone(), kind, why });
                    main_targets.insert(id.clone(), json!({"probabilities": null}));
                }
            }
        }
        if questions.is_empty() {
            left_out.push(LeftOut {
                record: records,
                kind: "no-question",
                why: "the state asks no question Valen's reader takes, so there is nothing to answer".to_owned(),
            });
            continue;
        }

        // **The pictures are copied once**, by hash, and named relative to the export folder.
        let mut state_assets: Vec<Value> = Vec::new();
        let mut copied = true;
        for (name, sha) in &state.pictures {
            if let Err(why) = copy_image(store, sha, out) {
                left_out.push(LeftOut {
                    record: records.clone(),
                    kind: "unreadable-picture",
                    why: format!("the {name} picture {sha} cannot be read from the labels folder: {why}"),
                });
                copied = false;
                break;
            }
            assets.insert(format!("{}/{sha}.png", crate::turn_store::IMAGES_DIR), sha.clone());
            state_assets.push(json!({"path": format!("{}/{sha}.png", crate::turn_store::IMAGES_DIR), "sha256": sha}));
        }
        if !copied {
            continue;
        }

        // **A record with no target at all is not written**: a line that answers nothing teaches nothing.
        let answered = |targets: &Map<String, Value>| targets.values().any(|target| !target["probabilities"].is_null());
        // **Provenance, per question** (the issue's `meta`): which tier answered it, where that answer came from, the assessor's version and the
        // build commit, so a report can trace any number in the set back to its line.
        let meta = json!({
            "scene": scene,
            "records": one.records,
            "tier": strongest_tier(&provenance),
            "source": one.source,
            "cauldron_version": one.cauldron_version,
            "build_commit": one.build_commit,
            "questions": provenance,
        });
        let record = |targets: &Map<String, Value>| -> String {
            let line = json!({
                "group_id": scene,
                "request": {"state": state_message(state), "questions": questions},
                "targets": targets,
                "assets": state_assets,
                "meta": meta,
            });
            serde_json::to_string(&line).unwrap_or_default()
        };
        let to_eval = bench_scenes.get(scene.as_str()).copied().unwrap_or(false) || in_eval(scene, options.eval_share);
        if answered(&main_targets) {
            if to_eval {
                eval.push(record(&main_targets));
            } else {
                train.push(record(&main_targets));
            }
        } else if holdout_targets.is_empty() {
            left_out.push(LeftOut {
                record: records.clone(),
                kind: "no-target",
                why: "no answer of a trusted tier reached any of its questions, so the line is parked whole".to_owned(),
            });
        }
        if answered(&holdout_targets) {
            holdout.push(record(&holdout_targets));
        }
    }

    // **Every record of a scene is on one side**, which only a bug in the bucket could break: it is checked, never assumed.
    let train_scenes: std::collections::BTreeSet<&str> = train.iter().filter_map(|line| group_of(line)).collect();
    let eval_scenes: std::collections::BTreeSet<&str> = eval.iter().filter_map(|line| group_of(line)).collect();
    if let Some(shared) = train_scenes.intersection(&eval_scenes).next() {
        return Err(format!("the scene `{shared}` landed in both train and eval, which the split exists to prevent"));
    }

    exported.left_out = left_out;
    exported.images = assets.len();
    exported.train = train.len();
    exported.eval = eval.len();
    exported.holdout = holdout.len();
    for (name, lines) in [(TRAIN_FILE, &train), (EVAL_FILE, &eval), (HOLDOUT_FILE, &holdout)] {
        if lines.is_empty() {
            // **A side this export wrote nothing for is not left standing from an earlier one**: a stale `eval.jsonl` beside a fresh
            // `train.jsonl` would be a set whose split is not the ledger's. Only a file this export could have written is removed.
            let stale = out.join(name);
            if stale.is_file() {
                std::fs::remove_file(&stale).map_err(|e| format!("{}: {e}", stale.display()))?;
            }
            continue;
        }
        let mut text = String::with_capacity(lines.iter().map(|line| line.len() + 1).sum());
        for line in lines.iter() {
            text.push_str(line);
            text.push('\n');
        }
        let path = out.join(name);
        bevy_grimoire_core::atomic_file::write(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        exported.files.push((path, lines.len()));
    }
    // **The parked questions by the record they are about** (DL-08, #467), because a count alone cannot be placed in a week: a report reads the
    // record, the record names its turn, and the turn names its week. Every other line here is a count of the set, and this is the one index into it.
    let mut parked_by_record: BTreeMap<String, usize> = BTreeMap::new();
    for parked in &exported.parked {
        *parked_by_record.entry(parked.record.clone()).or_insert(0) += 1;
    }
    let summary = json!({
        "train_records": exported.train,
        "eval_records": exported.eval,
        "holdout_records": exported.holdout,
        "images": exported.images,
        "parked": exported.parked_by_kind(),
        "parked_by_record": parked_by_record,
        "disagreements": exported.disagreements_by_kind(),
        "teacher_errors": exported.teacher_errors(),
        "left_out": exported.left_out_by_kind(),
    });
    let mut text = serde_json::to_string_pretty(&summary).unwrap_or_default();
    text.push('\n');
    let path = out.join(SUMMARY_FILE);
    bevy_grimoire_core::atomic_file::write(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(exported)
}

/// The `group_id` of a written record, without re-reading the whole line.
fn group_of(line: &str) -> Option<&str> {
    let at = line.find("\"group_id\":\"")? + "\"group_id\":\"".len();
    let rest = &line[at..];
    rest.find('"').map(|end| &rest[..end])
}

/// **The strongest tier a record carries**, for its `meta`: gold where the person answered anything of it.
fn strongest_tier(provenance: &Map<String, Value>) -> &'static str {
    let mut strongest = Tier::Silver;
    for entry in provenance.values() {
        let tier = match entry["tier"].as_str() {
            Some("gold") => Tier::Gold,
            Some(TIER_OBJECTIVE) => Tier::Objective,
            _ => Tier::Silver,
        };
        strongest = strongest.max(tier);
    }
    strongest.name()
}

/// **Why a question's answer is not a distribution**, in the words a person reads. The one case left after `resolve` is a score question whose
/// votes were kept as a tally this format does not carry.
fn parked_why(_candidates: &[Candidate], question: &Question) -> (&'static str, String) {
    match question {
        Question::Score { .. } => ("not-a-distribution", "a score question's votes are not kept as probabilities, so there is nothing to train on".to_owned()),
        Question::Choice { .. } => ("votes-split", "none of the votes named an option the question offers".to_owned()),
        Question::Noul { .. } => ("votes-split", "the answer that stood is not a yes or a no".to_owned()),
    }
}

/// **Whether a scene goes to eval**: its own id's hash, so a scene lands on one side however the ledger is ordered, and a report can say which
/// side a scene is on without running an export.
pub fn in_eval(scene: &str, share: u32) -> bool {
    bucket(scene, share) == 0
}

/// **Which scene's share an id falls in**: its own hash, so a scene lands on one side however the ledger is ordered.
fn bucket(scene: &str, share: u32) -> u32 {
    let digest = sha256_hex(scene.as_bytes());
    let mut bytes = [0u8; 4];
    for (at, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&digest[at * 2..at * 2 + 2], 16).unwrap_or(0);
    }
    u32::from_be_bytes(bytes) % share
}

/// **The state a student sees**: the words, then each picture as an `image_url` part relative to the export folder, as Valen's own smoke set
/// writes one.
///
/// **Every picture the state has**, in the order they were taken (the before, then the after), each as an `image_url` part relative to the export
/// folder, as Valen's own smoke set writes one.
///
/// **All of them, never only the last**: a question whose `about` is `change` (`check-out_of_area` is the one in the shipped checks) asks what
/// changed between the before and the after, so a state that showed only the result would train it on half its evidence. Every asset the record
/// names is a picture shown here, and every picture shown is an asset.
fn state_message(state: &State) -> Value {
    let mut content = vec![json!({"type": "text", "text": state.text})];
    for (_, sha) in &state.pictures {
        content.push(json!({"type": "image_url", "image_url": {"url": format!("{}/{sha}.png", crate::turn_store::IMAGES_DIR)}}));
    }
    json!({"messages": [{"role": "user", "content": content}]})
}

/// **The probabilities a target is**: hard 1 or 0 for the person and the engine, the vote fraction for a teacher's.
fn probabilities(question: &Question, answer: &Answer) -> Option<BTreeMap<String, f64>> {
    let mut out = BTreeMap::new();
    match (question, answer) {
        (Question::Noul { .. }, Answer::Hard(yes)) => {
            out.insert("true".to_owned(), if *yes { 1.0 } else { 0.0 });
            out.insert("false".to_owned(), if *yes { 0.0 } else { 1.0 });
        }
        (Question::Noul { .. }, Answer::Soft(fraction)) => {
            let yes = fraction.clamp(0.0, 1.0);
            out.insert("true".to_owned(), yes);
            out.insert("false".to_owned(), 1.0 - yes);
        }
        (Question::Choice { criteria, .. }, Answer::Choice(tally)) => {
            let counted: u32 = criteria.keys().filter_map(|option| tally.get(option)).sum();
            if counted == 0 {
                return None;
            }
            for option in criteria.keys() {
                out.insert(option.clone(), f64::from(tally.get(option).copied().unwrap_or(0)) / f64::from(counted));
            }
        }
        _ => return None,
    }
    Some(out)
}

/// **Whether Valen's reader takes this question**: a candidate list where the type needs one, and words in it.
fn valen_shape(question: &Question) -> Result<(), String> {
    match question {
        Question::Noul { instructions } => {
            if instructions.trim().is_empty() {
                return Err("its instructions are empty".to_owned());
            }
        }
        Question::Choice { instructions, criteria } => {
            if instructions.trim().is_empty() {
                return Err("its instructions are empty".to_owned());
            }
            if !(1..=255).contains(&criteria.len()) {
                return Err(format!("a choice takes 1 to 255 named options, and this has {}", criteria.len()));
            }
            if criteria.keys().any(|option| option.trim().is_empty()) || criteria.values().any(|meaning| meaning.trim().is_empty()) {
                return Err("an option has an empty name or meaning".to_owned());
            }
        }
        Question::Score { instructions, criteria } => {
            if instructions.trim().is_empty() {
                return Err("its instructions are empty".to_owned());
            }
            if !(2..=255).contains(&criteria.len()) {
                return Err(format!("a score takes 2 to 255 levels, and this has {}", criteria.len()));
            }
            if criteria.iter().any(|level| level.trim().is_empty()) {
                return Err("a level is empty".to_owned());
            }
        }
    }
    Ok(())
}

/// **Copy one picture into the export, by its hash**, and refuse one whose bytes do not hash to the name it is filed under: the name is what the
/// ledger means by it.
fn copy_image(store: &Store, sha: &str, out: &Path) -> Result<(), String> {
    let from = store.image_path(sha);
    let bytes = std::fs::read(&from).map_err(|e| format!("{}: {e}", from.display()))?;
    if sha256_hex(&bytes) != sha {
        return Err(format!("{} holds bytes that do not hash to its name", from.display()));
    }
    let to = out.join(crate::turn_store::IMAGES_DIR).join(format!("{sha}.png"));
    if std::fs::read(&to).is_ok_and(|held| held == bytes) {
        return Ok(());
    }
    bevy_grimoire_core::atomic_file::write(&to, &bytes).map_err(|e| format!("{}: {e}", to.display()))
}

/// **A version as three numbers**, or `None` where it is not one. `0.6.9` is older than `0.7.0`, which the digits alone say.
pub fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    let mut parts = text.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_reads_as_three_numbers_or_not_at_all() {
        assert_eq!(parse_version("0.6.8"), Some((0, 6, 8)));
        assert_eq!(parse_version("1.0.0"), Some((1, 0, 0)));
        assert_eq!(parse_version("0.6"), Some((0, 6, 0)));
        assert_eq!(parse_version("test"), None);
        assert_eq!(parse_version("0.6.8.1"), None);
        assert!(parse_version("0.7.0") > parse_version("0.6.9"), "digits, not strings");
    }

    #[test]
    fn a_scenes_bucket_is_its_own_and_inside_the_share() {
        assert_eq!(bucket("scene-a", EVAL_SHARE), bucket("scene-a", EVAL_SHARE));
        assert!(bucket("scene-a", EVAL_SHARE) < EVAL_SHARE);
        assert!(bucket("scene-b", EVAL_SHARE) < EVAL_SHARE);
    }

    #[test]
    fn a_hard_answer_is_one_or_zero_and_a_soft_one_is_the_fraction() {
        let noul = Question::noul("The result satisfies: a floor");
        assert_eq!(probabilities(&noul, &Answer::Hard(true)).unwrap().get("true"), Some(&1.0));
        assert_eq!(probabilities(&noul, &Answer::Hard(false)).unwrap().get("false"), Some(&1.0));
        let soft = probabilities(&noul, &Answer::Soft(2.0 / 3.0)).unwrap();
        assert!((soft["true"] + soft["false"] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_choice_counts_every_option_and_a_score_is_never_a_distribution() {
        let choice = Question::choice("Which side is the door on?", [("east", "the east wall"), ("west", "the west wall")]);
        let tally = BTreeMap::from([("east".to_owned(), 2)]);
        let by_option = probabilities(&choice, &Answer::Choice(tally)).unwrap();
        assert_eq!(by_option.keys().collect::<Vec<_>>(), ["east", "west"]);
        assert_eq!(by_option["west"], 0.0);
        let score = Question::score("How good is it?", ["bad", "fair", "good"]);
        assert!(probabilities(&score, &Answer::Hard(true)).is_none());
    }
}
