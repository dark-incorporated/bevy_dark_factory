//! **The turn ledger's files** (DL-01, #460): what a line is, how it is written and how it is read back.
//!
//! The ledger is `<data_dir>/Grimoire/labels/turns.jsonl`, **append-only**, one JSON object per line, and the pictures it names beside it,
//! **content-addressed by SHA-256** so a frame that repeats is stored once. Nothing here knows about Bevy: the editor's side is
//! [`crate::turn_ledger`], which decides *when* a turn is recorded; this file is only *what* is recorded and *where*.
//!
//! # The rules the issue sets, and where each is kept
//!
//! - **Each line carries a format version** ([`Line::v`]). A line of another version does not read, and says which version this build reads.
//! - **A line that does not parse is reported by line number, never skipped silently** ([`Read::damaged`]): a ledger a reader cannot trust is a
//!   training set nobody can trust, so a damaged line is a finding.
//! - **A disk budget, and nothing is pruned** ([`Store::budget`]): when the next write would pass it, the write is refused with the sentence the
//!   person reads, and the ledger stops collecting. Nothing already written is removed.
//! - **Records are never edited in place.** [`Store::append`] is the only writer of the ledger, and it appends; a later fact about a turn (a
//!   person's verdict, DL-02) is a new line that names the turn.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// **The version of a ledger line's shape.** A line of another version does not read.
pub const FORMAT: u32 = 1;

/// The ledger's file, inside the labels folder.
pub const LEDGER_FILE: &str = "turns.jsonl";

/// The folder of pictures, inside the labels folder. A picture is `images/<first two hex>/<sha256>.png`.
pub const IMAGES_DIR: &str = "images";

/// **The most the ledger and its pictures may take on disk** (#460): two gigabytes. It is a named default shown in Settings (the row says
/// "Collection stops by itself at 2 GB"), so a change here is a change there, and `tests/a_turn_is_recorded_for_learning.rs` reads both.
pub const DEFAULT_BUDGET_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// **One picture, by what it is**: the SHA-256 of the PNG's bytes, which is also its file's name, so two records of one frame name one file.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ImageRef {
    pub sha256: String,
    pub width: u32,
    pub height: u32,
    /// The PNG's size on disk.
    pub bytes: u64,
    /// **Whether the picture is one flat colour** (DL-03, #462): the engine's own answer to `blank_frame`, worked out from the pixels when the picture
    /// landed. A line from before this existed reads as not flat.
    #[serde(default)]
    pub flat: bool,
}

/// **A capture the record has, or says it has not and why.** Exactly one of the two is set: a record never carries a picture field that is
/// silently empty.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageRef>,
    /// **Why there is no picture**: the capture did not land, the view did not settle, no write was seen. A sentence, for a reader of the ledger.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing: Option<String>,
}

impl Shot {
    /// A picture that was taken and stored.
    pub fn taken(image: ImageRef) -> Shot {
        Shot { image: Some(image), missing: None }
    }

    /// A picture that is not there, and the reason.
    pub fn missing(why: impl Into<String>) -> Shot {
        Shot { image: None, missing: Some(why.into()) }
    }
}

/// **Which hand-back ended the turn**: a `grimoire_present` or a resolved note.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HandBackRef {
    /// `presentation` or `note`.
    pub kind: String,
    /// `present-3`, or a note's `n3`.
    pub id: String,
}

/// **What was asked, as the agent handed it back** (#460): the hand-back's own words, which the editor copies and never rewrites.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub hand_back: HandBackRef,
    pub title: String,
    pub summary: String,
    /// What the person asked for, as the agent quoted it back.
    pub specified: Vec<String>,
    /// What the agent decided on its own.
    pub inferred: Vec<String>,
    /// The places the agent said were worth a look.
    pub check: Vec<String>,
    /// **A note's text**, for a turn a note ended; `None` for a presentation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// **One operation of the turn**, with the operation log's own line for it, verbatim.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OperationRecord {
    pub operation_id: String,
    pub op: String,
    /// The log line's `seq`, so the order is the log's.
    pub seq: u64,
    /// **Who made it** — `agent` or `human` — as the log says.
    pub source: String,
    /// The line as `grimoire/oplog_read` returns it. `null` when the hand-back names an id the log no longer holds.
    pub line: serde_json::Value,
}

/// **Who the agent was**, as its MCP handshake said.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AgentRef {
    pub name: String,
    pub version: String,
    /// The model, when something told the editor. Nothing does today, so it is `None` rather than a guess.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// **Which build wrote the record, and how it drew** (#460): a label made by an editor that has since changed is a label about different
/// behaviour, so the build is on every line.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Provenance {
    pub cauldron_version: String,
    /// The commit the build was made from, `unknown` when it could not be found (`build.rs`).
    pub build_commit: String,
    /// The surface's size and format, the window's scale and whether a question stood on the picture.
    pub render: serde_json::Value,
    pub agent: Option<AgentRef>,
    /// **A hash of the project's path, never the path**: enough to tell two projects apart, and nothing a shared ledger would give away.
    pub project_hash: String,
}

/// **One agent turn**, from its first write to its hand-back (#460).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Turn {
    /// `turn-<unix ms>-<n>`: unique across sittings, and what a label (DL-02) names.
    pub id: String,
    /// **A benchmark run's turn** (BM-02), kept out of every split by DL-06.
    pub bench: bool,
    /// When the turn's first write arrived.
    pub started_unix_ms: u64,
    /// When the agent handed it back.
    pub handed_back_unix_ms: u64,
    pub request: Request,
    /// The picture taken when the first write arrived, before it was applied.
    pub before: Shot,
    /// The picture taken at the hand-back, once the view had settled.
    pub after: Shot,
    pub operations: Vec<OperationRecord>,
    /// The pieces each operation touched, the camera and the engine's standing findings, as they were at the after picture.
    pub facts: serde_json::Value,
    /// **Every `grimoire_decide` call of the turn** (DM-05): as its server reported it to the editor.
    pub decisions: Vec<serde_json::Value>,
    pub provenance: Provenance,
    /// **The typed questions a student will be asked about this turn** (DL-03, #462), worded once and stored here: reading a record never rewrites
    /// one. A line from before questions existed has none.
    #[serde(default)]
    pub questions: crate::turn_questions::Questions,
}

/// **A thing the person said about a turn, by pointing at its pieces** (a snip, a developer report): the person's own words and where they were
/// pointing, on the turn that made the pieces (DL-02, #461).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    /// `snip` or `report`.
    pub source: String,
    /// The snip's or report's own name (`snip-<ms>`, `feedback-<ms>`), which finds its files.
    pub name: String,
    /// What the person typed. May be empty.
    pub note: String,
    /// Where they pointed, as the snip or the report states it: rectangles in window and picture pixels.
    pub region: serde_json::Value,
    /// The turn's pieces the region or the picked nodes overlap, by placement id.
    pub pieces: Vec<String>,
}

/// **A label**: what is known about a turn, written as a new line that names it, never as an edit of the turn's own (DL-02, #461).
///
/// `tier` says who the knowing is from, and **only a person's own action is `gold`**: an agent's call is never recorded as the person's verdict,
/// not even one that answers a hand-back on their behalf over BRP. `silver` is a teacher's: a later agent turn that took this one back.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Label {
    /// The turn it is about: [`Turn::id`].
    pub turn: String,
    /// `kept`, `undone`, `revised`, `superseded` or `item`.
    pub label: String,
    /// `gold` or `silver`.
    pub tier: String,
    /// When it happened, so a late label (a revision half an hour after a Keep) can be weighed differently from a prompt one.
    pub at_unix_ms: u64,
    /// **`revised`: the operation that did it**, the person's undo or edit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// **`superseded`: the later turn that took this one back**, or fixed it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub by_turn: Option<String>,
    /// **`item`: what the person said, and where.**
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<Item>,
}

/// **A hand-back the person closed without judging it.** Not a label: a dismissal kept the result unread, which proves nothing about it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Dismissed {
    pub turn: String,
    pub at_unix_ms: u64,
}

/// **What a ledger line holds.** Tagged by `kind`, so a label is a new line of its own and never an edit of a turn's.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Body {
    Turn(Box<Turn>),
    Label(Box<Label>),
    Dismissed(Dismissed),
}

/// **One line of the ledger.**
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Line {
    /// [`FORMAT`].
    pub v: u32,
    #[serde(flatten)]
/// **Which model graded a turn, and where it ran** (DL-04, #463).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AssessorRef {
    /// The assessor's version: its prompt and wordings. A turn is assessed once per version.
    pub version: u32,
    /// The endpoint asked, with no key in it.
    pub endpoint: String,
    /// `local` or `remote`: whether the pictures left the machine.
    pub site: String,
}

/// **One vote**: the model asked one question in one of three wordings.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Vote {
    /// Which wording (0, 1, 2) this vote is of.
    pub wording: u32,
    /// What the model answered, as the decision model's wire shape. `None` for a refused vote.
    pub answer: Option<serde_json::Value>,
    /// A yes/no answer's probability of yes.
    pub p_yes: Option<f64>,
    /// **Why the vote is a refusal**: the provider declined, the answer was of another type, or it was an option that was not offered. Never coerced.
    pub refused: Option<String>,
    /// The model that answered, as the provider named it.
    #[serde(default)]
    pub model: Option<String>,
}

/// **What the votes on one question came to** (DL-04, #463).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AssessedQuestion {
    pub id: String,
    pub votes: Vec<Vote>,
    /// For a yes/no: the fraction of the valid votes that said yes, the soft label. `None` when no vote was valid.
    pub yes_fraction: Option<f64>,
    /// For a choice: the votes each option got.
    pub tally: std::collections::BTreeMap<String, u32>,
    /// How many of the votes were refusals.
    pub refused: u32,
}

/// **A turn graded by the assessor** (DL-04, #463): silver, a teacher's labels, to be used where the votes agree (DL-06) and graded against the
/// person's own answers (DL-07, DL-08).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Assessment {
    pub turn: String,
    pub assessor: AssessorRef,
    pub at_unix_ms: u64,
    pub answers: Vec<AssessedQuestion>,
    /// Why nothing was asked, when a turn had no picture to look at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
}

    pub body: Body,
}

impl Line {
    /// A turn, as the ledger holds it.
    pub fn turn(turn: Turn) -> Line {
        Line { v: FORMAT, body: Body::Turn(Box::new(turn)) }
    Assessment(Box<Assessment>),
    }

    /// A label, as the ledger holds it.
    pub fn label(label: Label) -> Line {
        Line { v: FORMAT, body: Body::Label(Box::new(label)) }
    }

    /// A dismissal, as the ledger holds it.
    pub fn dismissed(dismissed: Dismissed) -> Line {
        Line { v: FORMAT, body: Body::Dismissed(dismissed) }
    }
}

/// **A line that did not read**, by its number in the file (from 1) and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Damaged {
    pub line: usize,
    pub why: String,
}

/// **What a ledger holds**: its lines in order, each with its number, and the ones that did not read.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Read {
    pub lines: Vec<(usize, Line)>,
    pub damaged: Vec<Damaged>,
}

    /// An assessment, as the ledger holds it.
    pub fn assessment(assessment: Assessment) -> Line {
        Line { v: FORMAT, body: Body::Assessment(Box::new(assessment)) }
    }

impl Read {
    /// The turns, in the order they were written.
    pub fn turns(&self) -> impl Iterator<Item = &Turn> {
        self.lines.iter().filter_map(|(_, line)| match &line.body {
            Body::Turn(turn) => Some(turn.as_ref()),
            _ => None,
        })
    }

    /// The labels, in the order they were written.
    pub fn labels(&self) -> impl Iterator<Item = &Label> {
        self.lines.iter().filter_map(|(_, line)| match &line.body {
            Body::Label(label) => Some(label.as_ref()),
            _ => None,
        })
    }

    /// The dismissals, in the order they were written.
    pub fn dismissals(&self) -> impl Iterator<Item = &Dismissed> {
        self.lines.iter().filter_map(|(_, line)| match &line.body {
            Body::Dismissed(dismissed) => Some(dismissed),
            _ => None,
        })
    }
}

/// **Read a ledger's text.** A blank line is nothing; any other line that is not a line of this format is [`Damaged`], with its number.
pub fn read_text(text: &str) -> Read {
    let mut read = Read::default();
    for (at, raw) in text.lines().enumerate() {
        let number = at + 1;
        if raw.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Line>(raw) {
            Ok(line) if line.v == FORMAT => read.lines.push((number, line)),
            Ok(line) => read.damaged.push(Damaged { line: number, why: format!("version {} is not the {FORMAT} this build reads", line.v) }),
            Err(e) => read.damaged.push(Damaged { line: number, why: e.to_string() }),
        }

    /// The assessments, in the order they were written.
    pub fn assessments(&self) -> impl Iterator<Item = &Assessment> {
        self.lines.iter().filter_map(|(_, line)| match &line.body {
            Body::Assessment(assessment) => Some(assessment.as_ref()),
            _ => None,
        })
    }

    /// **The person's answers to single questions** (DL-07, #466), in the order they were given: the labels named `audit`.
    pub fn audits(&self) -> impl Iterator<Item = &Label> {
        self.labels().filter(|label| label.label == AUDIT)
    }

    /// **The human holdout** (DL-07, #466): every label marked `holdout`. Never trained on; the one place that says which lines they are.
    pub fn holdout(&self) -> impl Iterator<Item = &Label> {
        self.labels().filter(|label| label.holdout)
    }
    }
    read
}

/// **Read the ledger at `path`.** A file that is not there is an empty ledger; one that cannot be read is an error naming it.
pub fn read_file(path: &Path) -> Result<Read, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(read_text(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Read::default()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

/// **A picture's name**: the SHA-256 of its bytes, in lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// **What the ledger takes on disk now.**
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// The turns written.
    pub turns: usize,
    /// The ledger and its pictures, in bytes.
    pub bytes: u64,
}

/// **The labels folder**, and the one writer of what is in it.
pub struct Store {
    dir: PathBuf,
    budget: u64,
    /// The bytes on disk, walked once and then kept by the writes this store makes.
    used: Option<u64>,
}

impl Store {
    /// A store in `dir`, with the default budget. The folder is made by the first write.
    pub fn new(dir: PathBuf) -> Store {
        Store { dir, budget: DEFAULT_BUDGET_BYTES, used: None }
    }

    /// The same store with another budget.
    pub fn with_budget(mut self, bytes: u64) -> Store {
        self.budget = bytes;
        self
    }

    /// The folder.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The ledger's file.
    pub fn ledger_path(&self) -> PathBuf {
        self.dir.join(LEDGER_FILE)
    }

    /// The most this store will take, in bytes.
    pub fn budget(&self) -> u64 {
        self.budget
    }

    /// Where a picture with this hash is kept.
    pub fn image_path(&self, sha256: &str) -> PathBuf {
        self.dir.join(IMAGES_DIR).join(&sha256[..2.min(sha256.len())]).join(format!("{sha256}.png"))
    }

    /// **The bytes the folder holds**, walked on the first ask.
    pub fn used(&mut self) -> u64 {
        *self.used.get_or_insert_with(|| bytes_under(&self.dir))
    }

    /// **What the ledger holds now**: its turn lines and every byte of the folder. Reads the file, so it is for a Settings row and a test,
    /// not for a frame.
    pub fn usage(&self) -> Usage {
        let turns = read_file(&self.ledger_path()).map(|read| read.turns().count()).unwrap_or(0);
        Usage { turns, bytes: bytes_under(&self.dir) }
    }

    /// **The sentence a refused write says.** The same words every time, because the person reads it once on the status line and again in the
    /// Settings row.
    fn full(&mut self) -> String {
        let used = self.used();
        format!(
            "the turn ledger in {} holds {used} bytes and its budget is {}, so it stopped collecting; nothing was removed. \
             To resume, make room in that folder (delete what you do not want kept), then turn \"Learn from my use\" off and on again in Settings",
            self.dir.display(),
            self.budget
        )
    }

    fn room_for(&mut self, bytes: u64) -> Result<(), String> {
        if self.used().saturating_add(bytes) > self.budget { Err(self.full()) } else { Ok(()) }
    }

    /// **Keep a picture**, once: a PNG whose hash is already on disk is not written again and costs no budget.
    pub fn put_image(&mut self, png: &[u8], width: u32, height: u32) -> Result<ImageRef, String> {
        let sha256 = sha256_hex(png);
        let image = ImageRef { sha256: sha256.clone(), width, height, bytes: png.len() as u64, flat: false };
        let path = self.image_path(&sha256);
        if path.is_file() {
            return Ok(image);
        }
        self.room_for(png.len() as u64)?;
        bevy_grimoire_core::atomic_file::write(&path, png).map_err(|e| format!("{}: {e}", path.display()))?;
        self.used = Some(self.used().saturating_add(png.len() as u64));
        Ok(image)
    }

    /// **Append one line.** The line is written in one `write`, so another process's append cannot land inside it.
    pub fn append(&mut self, line: &Line) -> Result<(), String> {
        let mut text = serde_json::to_string(line).map_err(|e| format!("the ledger line did not serialise: {e}"))?;
        text.push('\n');
        self.room_for(text.len() as u64)?;
        let path = self.ledger_path();
        std::fs::create_dir_all(&self.dir).map_err(|e| format!("{}: {e}", self.dir.display()))?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        file.write_all(text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        self.used = Some(self.used().saturating_add(text.len() as u64));
        Ok(())
    }
}

/// The bytes of every file under `dir`; a folder that is not there holds none.
fn bytes_under(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(meta) if meta.is_dir() => bytes_under(&entry.path()),
            Ok(meta) => meta.len(),
            Err(_) => 0,
        })
        .sum()
}
