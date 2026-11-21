//! **A Files-workspace buffer that was never saved has a draft on disk** (#508).
//!
//! Between two `Cmd+S` presses the editor's unsaved text lives only in memory, and a crash takes it back — and a buffer that
//! was never saved to disk at all has no copy anywhere. The map is written when it is saved or when an operation commits; a
//! file's buffer had nothing.
//!
//! # Where a draft goes, and why it is not the project
//!
//! [`blueprint_model::user_dirs::drafts_dir`], beside `recent.yaml` and `dark_factory.log`, for the argument
//! `log_file.rs` makes: *neither is inside a project (a project is a folder a person owns and shares)*. **A draft is
//! deliberately not project data.** A half-typed YAML file that an agent or a person's game could read is a file that says
//! something nobody meant; a draft under the person's own data folder is read by nothing but this editor, on this machine.
//!
//! # The shape on disk
//!
//! One JSON file per project, named by a hash of the project's absolute root, so two checkouts of one repository do not share
//! drafts. It holds each unsaved file's path, its text, the text the file held when the buffer was read (so a draft whose
//! file has moved on since can still be *offered* rather than silently applied), and when it was taken.
//!
//! # What is offered, and when
//!
//! [`write`] runs on the same interval the unsaved buffers change — a person's typing, and an agent's write into a clean
//! buffer — so the file on disk is never more than a few seconds behind. [`offers`] is read when a project opens, and
//! [`take`] removes one draft after the person has decided. **Declining deletes it**: a draft nobody wants is a draft that
//! asks again every launch, which is a question a person learns to dismiss without reading.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// **How long after the last change the drafts are written**, in seconds. A draft is for a crash, and a write per keystroke
/// would be a disk write per keystroke; a second is under the time a person takes to notice a crash and read the offer.
pub const DRAFT_WRITE_S: f64 = 1.0;

/// **One unsaved buffer, as the draft holds it.**
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Drafted {
    /// The file, relative to the project's root, the way every other name in this crate is.
    pub path: String,
    /// What the person had typed, or what an agent wrote into a buffer nobody had typed in.
    pub text: String,
    /// **What the file said when the buffer was read or last written**, so an offer can say whether the file has moved on
    /// since — which is a conflict the person deserves to know about before taking the draft.
    pub disk: String,
    /// **When it was taken**, RFC 3339, UTC: the offer says this, because "half an hour ago" and "just now" are different
    /// decisions.
    pub taken_at: String,
}

/// **A project's drafts**, as the file holds them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Drafts {
    pub version: u32,
    /// The project's root, absolute, so a file found by a hash can say which project it is about.
    pub root: String,
    #[serde(default)]
    pub files: Vec<Drafted>,
}

/// The draft format's version.
pub const DRAFTS_VERSION: u32 = 1;

/// **The drafts file for a project**, under the person's data folder: `<data_dir>/Blueprint/drafts/<hash>.json`.
///
/// The name is a hash of the project's absolute root so two checkouts of one repository — which is a real case, a person's
/// worktree beside a clone — do not overwrite each other's drafts. `None` on a platform with no data directory, which is a
/// machine with nowhere to put a draft; the editor then keeps its buffers in memory and says nothing, because there is
/// nowhere to say it.
pub fn file_for(root: &Path) -> Option<PathBuf> {
    let dir = blueprint_model::user_dirs::drafts_dir()?;
    Some(dir.join(format!("{:016x}.json", hash_of(&root.to_string_lossy()))))
}

/// **FNV-1a of a string**, the hash this crate's row keys use, so the drafts folder and the tree name things the same way.
fn hash_of(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// **Write the drafts for a project**, or remove the file when there is nothing unsaved. An empty drafts folder is the state
/// the editor is in almost all the time, and a file per project asking a question about nothing every launch is noise.
///
/// **Not atomic-required, but atomic anyway**: a crashed writer's half file would be a draft nobody can read, which is
/// strictly worse than no draft, so the write goes through the same `atomic_file` every other writer in this crate uses.
pub fn write(root: &Path, files: Vec<Drafted>) -> Result<(), String> {
    let Some(path) = file_for(root) else { return Ok(()) };
    if files.is_empty() {
        match std::fs::remove_file(&path) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        }
    }
    let drafts = Drafts { version: DRAFTS_VERSION, root: root.to_string_lossy().replace('\\', "/"), files };
    let text = serde_json::to_string(&drafts).map_err(|e| format!("the drafts did not write: {e}"))?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    blueprint_model::atomic_file::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// **The drafts a project has waiting**, or an empty list when there is none. A file that does not read is treated as no
/// draft rather than as a problem: a corrupt draft is not a reason to refuse to open a project, and the person's real files
/// are all still where they are.
pub fn offers(root: &Path) -> Vec<Drafted> {
    let Some(path) = file_for(root) else { return Vec::new() };
    let Ok(text) = std::fs::read_to_string(&path) else { return Vec::new() };
    let Ok(drafts) = serde_json::from_str::<Drafts>(&text) else { return Vec::new() };
    if drafts.version != DRAFTS_VERSION {
        return Vec::new();
    }
    drafts.files
}

/// **Remove one file's draft**, after the person has decided about it: taken, or declined. The rest of the project's drafts
/// stay, and the file goes when the last one is taken (as [`write`] with an empty list does).
pub fn take(root: &Path, rel: &str) -> Result<(), String> {
    let mut files = offers(root);
    let before = files.len();
    files.retain(|d| d.path != rel);
    if files.len() == before {
        return Ok(());
    }
    write(root, files)
}

/// **What an offer says**, in one sentence a person reads: the file, when the draft was taken, and whether the file has
/// moved on since — the conflict they deserve to know about before taking it.
pub fn offer_sentence(draft: &Drafted, disk_now: Option<&str>) -> String {
    let moved = match disk_now {
        // The file is gone: taking the draft brings it back, which is worth saying.
        None => " The file is gone from the project; taking this brings it back.".to_owned(),
        Some(now) if now != draft.disk => " The file has changed on disk since this was taken.".to_owned(),
        Some(_) => String::new(),
    };
    format!("{} was never saved: a draft taken {}{moved}", draft.path, draft.taken_at)
}

/// **Now, as RFC 3339 in UTC**, the way an operation log line is stamped: the same reader can show both.
pub fn now_rfc3339() -> String {
    crate::ops::now_rfc3339_public()
}

/// **The drafts a project has, written from its buffers**, for the system that keeps the file true. Kept here rather than in
/// [`crate::files`] because a draft's shape is this module's, and `files` should not have to know a JSON file exists.
pub fn harvest(map: &BTreeMap<String, crate::files::OpenFile>) -> Vec<Drafted> {
    let taken_at = now_rfc3339();
    map.iter()
        .filter(|(_, file)| file.dirty() && file.problem.is_none())
        .map(|(rel, file)| Drafted { path: rel.clone(), text: file.text.clone(), disk: file.disk.clone(), taken_at: taken_at.clone() })
        .collect()
}

// ── The system that keeps the file true ─────────────────────────────────────

/// **When the drafts were last written**, so a keystroke does not become a disk write. A resource, because the editor runs
/// the same system in two workspaces and both must share one clock.
#[derive(Resource, Default)]
pub struct DraftClock {
    last: f64,
    /// The drafts as last written, so an unchanged set is not rewritten every [`DRAFT_WRITE_S`].
    written: Vec<Drafted>,
}

/// **Write the drafts when the unsaved set has changed**, once per [`DRAFT_WRITE_S`] and not once per keystroke.
///
/// **A write failure is a problem on the status line, once**, not a refusal of anything a person was doing: the buffer is
/// still theirs and the editor still runs. [`DraftClock::written`] is not updated on a failure, so the next interval tries
/// again rather than believing a write that did not land.
pub fn keep_the_drafts(
    // **The editor's own clock**, as the periodic save reads it: a draft is for a crash, and the editor's frame steps are what
    // "a second after the typing stopped" means to the loop that writes it.
    time: Res<bevy::time::Time>,
    files: Option<Res<crate::files::Files>>,
    mut clock: ResMut<DraftClock>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    let now = time.elapsed_secs_f64();
    if now - clock.last < DRAFT_WRITE_S {
        return;
    }
    clock.last = now;
    let Some(files) = files else { return };
    let Some(root) = files.root().map(std::path::Path::to_path_buf) else { return };
    let drafts = harvest(files.every_open());
    if drafts == clock.written {
        return;
    }
    match write(&root, drafts.clone()) {
        Ok(()) => clock.written = drafts,
        Err(why) => {
            if let Some(state) = state.as_deref_mut() {
                state.status.problem(format!("DRAFTS NOT KEPT: {why}"));
            }
        }
    }
}

/// **The drafts a project has waiting**, so a screen can offer them: a resource rather than a read at each frame, because a
/// crash is a rare thing and a disk read per frame for it is not.
#[derive(Resource, Default)]
pub struct DraftOffers {
    /// The project the offers are about, so a project change re-reads them.
    pub root: Option<PathBuf>,
    /// The drafts waiting for the person's answer.
    pub waiting: Vec<Drafted>,
}

impl DraftOffers {
    /// **Take a draft into the buffer it belongs to**: the text is restored, unsaved, and the draft is removed so the next
    /// launch does not ask again.
    pub fn take(&mut self, world: &mut World, rel: &str) -> Result<(), String> {
        let Some(draft) = self.waiting.iter().find(|d| d.path == rel).cloned() else {
            return Err(format!("{rel} has no draft waiting"));
        };
        // **The file's buffer is opened first**, so a draft of a file nobody has open has somewhere to go.
        crate::files::hold_for_editing(world, rel).map_err(|e| format!("{rel}: {e}"))?;
        if let Some(mut files) = world.get_resource_mut::<crate::files::Files>()
            && let Some(file) = files.get_mut(rel)
        {
            file.text = draft.text.clone();
            file.pushed += 1;
        }
        if let Some(root) = self.root.clone() {
            take(&root, rel)?;
        }
        self.waiting.retain(|d| d.path != rel);
        Ok(())
    }

    /// **Decline a draft**: it is deleted, and nothing is applied. A draft nobody wants that asked again every launch is a
    /// question a person learns to dismiss without reading.
    pub fn decline(&mut self, rel: &str) -> Result<(), String> {
        if let Some(root) = self.root.clone() {
            take(&root, rel)?;
        }
        self.waiting.retain(|d| d.path != rel);
        Ok(())
    }
}

/// **Read a project's drafts when it opens**, and say that they are waiting. An offer is not applied: a draft is what might
/// have been meant, and a launch that silently restored one would be a launch that guessed.
pub fn offer_on_open(project: Option<Res<crate::project::Project>>, mut offers: ResMut<DraftOffers>, mut state: Option<ResMut<crate::editor::EditorState>>) {
    let Some(project) = project else { return };
    if offers.root.as_deref() == Some(project.root.as_path()) {
        return;
    }
    let found = self::offers(&project.root);
    if !found.is_empty()
        && let Some(state) = state.as_deref_mut()
    {
        state.status.note(match found.len() {
            1 => format!("a draft of {} was never saved; take it or decline it in the Files workspace", found[0].path),
            n => format!("{n} buffers had drafts that were never saved; take them or decline them in the Files workspace"),
        });
    }
    offers.root = Some(project.root.clone());
    offers.waiting = found;
}

pub struct DraftPlugin;

impl Plugin for DraftPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DraftClock>()
            .init_resource::<DraftOffers>()
            .add_systems(OnEnter(crate::screen::Screen::Editor), offer_on_open)
            .add_systems(Update, keep_the_drafts.run_if(in_state(crate::screen::Screen::Editor)));
    }
}
