//! **The open map's notes, as the editor holds them** (#200, H3).
//!
//! The sign over every standing note and the Notes panel both draw from the map's `.notes.yaml`. Each
//! used to read the file itself and take an empty list when it did not parse, so one bad edit (a hand
//! edit, a truncated write, an agent's mistake) made every sign and every row vanish until the file
//! was fixed. Now one system reads it, decides each edit once, and holds the last good [`Notes`]: a
//! refused file leaves what is drawn as it was and is named in `blueprint_state.reloads` (kind `notes`,
//! the file and the field) and on the status line.
//!
//! **The file is read again when the project index says it moved** ([`crate::project_index`], #501): the stamp the
//! index holds for the path is not the one the last look was made at. A frame in which the index's generation did not
//! move makes no filesystem call (one atomic load, after the path is built), and a change to another file reads nothing. A read that finds the same text as the last
//! one decides nothing, so one edit is one verdict however many frames pass. A notes file the index cannot follow
//! (outside the project, under a skipped folder, named by `.gitignore`) is refused on the status line and not read;
//! see `crate::followed_files`.

use std::path::PathBuf;

use bevy::prelude::*;
use blueprint_model::live::KindVerdict;
use blueprint_model::notes::{self, Notes};
use blueprint_model::reload::kind_by_id;
use blueprint_model::reload_check::{self, Check};

use crate::editor::EditorState;
use crate::followed_files::{self, Asked, Looked};
use crate::live::EditorReloads;
use crate::project::{OpenMap, Project};
use crate::project_index::ProjectIndex;

/// **The last good notes of the open map**, and what the view last saw of their file.
#[derive(Resource, Default)]
pub struct NotesView {
    /// The notes file this holds the notes of; another map's file starts the view afresh.
    path: PathBuf,
    /// What the last look was made at: the index's stamp of the notes file.
    followed: Option<Looked>,
    /// Why the index could not be asked, said once.
    unanswered: Option<String>,
    /// Why the index cannot follow the notes file, said once.
    unfollowed: Option<String>,
    /// **How many times the view has looked at the filesystem**: the file read, and the `.gitignore` read and symbolic-link check a
    /// path the index does not list needs. A test says a change to another file did not make the view look.
    reads: u64,
    /// **How many times the notes have been replaced**, which is the only thing about this view other code
    /// may watch: `is_changed` says the view was written to, which a look that found the same text also does,
    /// so it says nothing about whether the notes moved.
    generation: u64,
    /// The text last read, good or refused, so the same bytes are decided once.
    text: Option<String>,
    notes: Notes,
}

impl NotesView {
    /// The notes the editor draws: the file's, as of the last edit that read, or the earlier good
    /// ones when the newest edit was refused. Empty for a map with no notes file.
    pub fn notes(&self) -> &Notes {
        &self.notes
    }

    /// **The notes file this has looked at**, or nothing before the first read.
    ///
    /// A reader that has to tell "this map has no notes" from "this map's file has not been looked
    /// at yet" cannot do it from [`Self::notes`] alone: both are empty. This is the difference, and
    /// it is what `present::record_handed_back_notes` reads to take its baseline.
    /// **How many times the notes held here have been replaced.** A reader that must rebuild when the notes
    /// move (the outliner's NOTES group) compares this and nothing else.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// **How many filesystem looks the view has made** (see the field).
    pub fn reads(&self) -> u64 {
        self.reads
    }

    pub fn looked_at(&self) -> Option<&std::path::Path> {
        (!self.path.as_os_str().is_empty()).then_some(self.path.as_path())
    }
}

/// **Read the open map's notes file when it changed, and decide it once.**
///
/// A file that reads replaces the held notes; one that is refused ([`Notes::parse`]: not YAML, the
/// wrong type, a field missing, a version this build does not read, an id twice) leaves them; a file
/// that is gone empties them and clears its verdict. A refusal names the file as the project's
/// directory sees it and the field it is wrong at.
pub fn sync_notes_view(
    index: Res<ProjectIndex>,
    open: Option<Res<OpenMap>>,
    project: Option<Res<Project>>,
    mut view: ResMut<NotesView>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let (Some(open), Some(project)) = (open, project) else { return };
    let path = notes::path_for_map(&open.map_path);
    let same_file = view.path == path;
    if same_file && view.followed.as_ref().is_some_and(|f| f.generation == index.generation()) {
        return;
    }
    let snapshot = match followed_files::ask(&index, &project.root) {
        Asked::Wait => return,
        Asked::Refuse(why) => {
            followed_files::say_once(&mut view.unanswered, "the notes file", Some(why), state.as_deref_mut());
            return;
        }
        Asked::Ready(snapshot) => snapshot,
    };
    followed_files::say_once(&mut view.unanswered, "the notes file", None, None);
    if same_file && let Some(followed) = view.followed.as_mut() && !followed.moved(&project.root, &snapshot) {
        followed.generation = snapshot.generation;
        return;
    }
    let mut looks = 0u64;
    let reach = followed_files::reach(&project.root, &snapshot, &path, &mut looks);
    view.reads += looks;
    let followed = Looked::new(&snapshot, &project.root, vec![(path.clone(), reach.clone().unwrap_or(None))]);
    if !same_file {
        // Another map has no earlier value of its own: nothing carries over.
        view.notes = Notes::default();
        view.text = None;
        view.generation += 1;
        view.path = path.clone();
    }
    if let Err(why) = reach {
        // **No event will ever say this file moved**, so it is not read: what is held stays, and the status line says why.
        followed_files::say_once(&mut view.unfollowed, "the notes file", Some(why), state.as_deref_mut());
        view.followed = Some(followed);
        return;
    }
    followed_files::say_once(&mut view.unfollowed, "the notes file", None, None);
    let file = path.strip_prefix(&project.project_dir).unwrap_or(&path).to_string_lossy().replace('\\', "/");
    view.reads += 1;
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            // A file that is there and cannot be read is not a file that is gone: it is refused, and the notes held stay. The text
            // is forgotten, so the file reading as the text it had before is a change again and clears the refusal.
            view.followed = Some(followed);
            view.text = None;
            let refused = KindVerdict::refused("notes", file, None, e.to_string());
            warn!("{}", refused.sentence());
            if let Some(state) = state.as_deref_mut() {
                state.status.raise("", refused.problem("the notes already shown stay"));
            }
            if let Some(reloads) = reloads.as_deref_mut() {
                reloads.forget(&["notes"]);
                reloads.decide(vec![refused]);
            }
            return;
        }
    };
    view.followed = Some(followed);
    if same_file && view.text == text {
        return;
    }
    view.text = text.clone();
    let Some(text) = text else {
        view.notes = Notes::default();
        view.generation += 1;
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&["notes"]);
        }
        return;
    };
    let verdict = match Notes::parse(&text) {
        Ok(read) => {
            view.notes = read;
            view.generation += 1;
            KindVerdict::applied("notes", file)
        }
        Err(reason) => {
            let field = kind_by_id("notes").and_then(|k| match reload_check::check(k, &file, &text) {
                Check::Refused(r) => r.field,
                _ => None,
            });
            let refused = KindVerdict::refused("notes", file, field, reason);
            if let Some(state) = state.as_deref_mut() {
                let kept = if same_file { "the notes already shown stay" } else { "no notes are shown from it" };
                state.status.raise("", refused.problem(kept));
            }
            refused
        }
    };
    if let Some(reloads) = reloads.as_deref_mut() {
        // One decision lists the file read, so another map's verdict is not left standing beside it.
        reloads.forget(&["notes"]);
        reloads.decide(vec![verdict]);
    }
}
