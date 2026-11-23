//! **The project's own checks, as the editor holds them** (#200, H3).
//!
//! `Checks::for_project` reads `assets/blueprint/checks.yaml` fresh on every run, so one bad edit (a hand
//! edit, a truncated write, an agent's mistake) made every after-edit check silently not run: the
//! journal said "not run" with the parser's position and nothing said which field was wrong, and the
//! second reader dropped the error altogether. Now one system looks at the file, decides each edit
//! once, and holds the last good [`Checks`]: `blueprint_state.reloads` says `applied`, or `refused`
//! with the file and the field, and the checks the editor asks keep running as the last good ones
//! define them until the file reads again.
//!
//! **The file is read again when the project index says it moved** ([`crate::project_index`], #501): the stamp the
//! index holds for the path is not the one the last look was made at. A frame in which the index's generation did not
//! move makes no filesystem call (one atomic load, after the path is built), and a change to another file reads nothing. A read that finds the same text as the last
//! one decides nothing, so one edit is one verdict however many frames pass. A file the index cannot follow (outside the
//! project, under a skipped folder, named by `.gitignore`) is refused and not read; see `crate::followed_files`.
//!
//! The type is the decision crate's, above core, so [`blueprint_model::reload_check::check`] cannot
//! read it; [`blueprint_model::reload_check::read`] names the field of a file that does not parse,
//! and [`Checks::from_yaml`] says why a file that did parse breaks a rule (two checks with one name, a
//! threshold outside 0..=1), which has no one field.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use blueprint_model::live::KindVerdict;
use blueprint_model::reload_check;
use bevy_oracle::{Checks, Error as DecideError, PROJECT_CHECKS_FILE};

use crate::editor::EditorState;
use crate::followed_files::{self, Asked, Looked};
use crate::live::EditorReloads;
use crate::project::Project;
use crate::project_index::ProjectIndex;

/// **What a look at the file found.**
#[derive(Clone, PartialEq)]
enum Seen {
    /// No file: the editor's own checks alone.
    Missing,
    Text(String),
    /// There, and cannot be read, or the project index cannot follow it; the reason.
    Unreadable(String),
}

/// **The last good checks of the open project**, and what the view last saw of their file.
#[derive(Resource, Default)]
pub struct ChecksView {
    /// The `checks.yaml` this holds the checks of; another project's file starts the view afresh.
    path: PathBuf,
    /// What the last look was made at: the index's stamp of the file.
    followed: Option<Looked>,
    /// Why the index could not be asked, said once.
    unanswered: Option<String>,
    /// **How many times the view has looked at the filesystem**: the file read, and the `.gitignore` read and symbolic-link check a
    /// path the index does not list needs. A test says a change to another file did not make the view look.
    reads: u64,
    /// What the last look found, good or refused, so the same bytes are decided once.
    seen: Option<Seen>,
    /// The editor's checks with the project's over them, as of the last read that was good (no file is
    /// the editor's alone). `None` until one: a file that was broken from the first look has no earlier
    /// good state, so [`checks_for`] falls back to the reader, which says why.
    good: Option<Checks>,
}

impl ChecksView {
    /// **How many filesystem looks the view has made** (see the field).
    pub fn reads(&self) -> u64 {
        self.reads
    }
}

/// `<project root>/assets/blueprint/checks.yaml`, the path `Checks::for_project` reads.
fn path_in(root: &Path) -> PathBuf {
    root.join("assets").join("blueprint").join(PROJECT_CHECKS_FILE)
}

/// **The checks to run for the project at `root`**: the last good ones the view holds for its file, or
/// a fresh read when the view holds none (it has not looked yet, or the file was broken at the first
/// look). A refused edit therefore leaves the checks as they were instead of stopping them.
pub fn checks_for(view: Option<&ChecksView>, root: &Path) -> Result<Checks, String> {
    match view.filter(|v| v.path == path_in(root)).and_then(|v| v.good.clone()) {
        Some(good) => Ok(good),
        None => Checks::for_project(root),
    }
}

/// **A file's text as the checks it defines, or why not**: the field it is wrong at when it did not
/// parse (`None` for a rule it breaks after parsing, which the reason names), and the reason.
fn read(file: &str, text: &str) -> Result<Checks, (Option<String>, String)> {
    let e = match Checks::from_yaml(text) {
        Ok(checks) => return Ok(checks),
        Err(e) => e,
    };
    let reason = match e {
        DecideError::Checks(m) => m,
        other => other.to_string(),
    };
    // The decision crate's reader says where in the text, not which field; the shared one says both for
    // a file that does not parse. A file that parses and still refuses broke a rule: no field.
    match reload_check::read::<Checks>("checks", file, text, |t| Checks::from_yaml(t).map_err(|e| e.to_string())) {
        Err(r) => Err((r.field, reason)),
        Ok(_) => Err((None, reason)),
    }
}

/// **Read the project's `checks.yaml` when the index says it moved, and decide it once.**
///
/// A file that reads replaces the held checks; one that is refused (not YAML, a wrong type, a missing
/// field, two checks of one name, a threshold outside 0..=1) leaves them; a file that is gone is the
/// editor's checks alone and clears its verdict. A refusal names the file as the project's directory
/// sees it and the field it is wrong at.
pub fn sync_checks_view(
    index: Res<ProjectIndex>,
    project: Option<Res<Project>>,
    mut view: ResMut<ChecksView>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Some(project) = project else { return };
    let path = path_in(&project.root);
    let same_file = view.path == path;
    if same_file && view.followed.as_ref().is_some_and(|f| f.generation == index.generation()) {
        return;
    }
    let snapshot = match followed_files::ask(&index, &project.root) {
        Asked::Wait => return,
        Asked::Refuse(why) => {
            followed_files::say_once(&mut view.unanswered, "the checks file", Some(why), state.as_deref_mut());
            return;
        }
        Asked::Ready(snapshot) => snapshot,
    };
    followed_files::say_once(&mut view.unanswered, "the checks file", None, None);
    if same_file && let Some(followed) = view.followed.as_mut() && !followed.moved(&project.root, &snapshot) {
        followed.generation = snapshot.generation;
        return;
    }
    let mut looks = 0u64;
    let reach = followed_files::reach(&project.root, &snapshot, &path, &mut looks);
    view.reads += looks;
    let followed = Looked::new(&snapshot, &project.root, vec![(path.clone(), reach.clone().unwrap_or(None))]);
    let seen = match reach {
        // **No event will ever say this file moved**, so it is not read: refused, with the reason.
        Err(why) => Seen::Unreadable(why),
        Ok(_) => {
            view.reads += 1;
            match std::fs::read_to_string(&path) {
                Ok(text) => Seen::Text(text),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Seen::Missing,
                // A file that is there and cannot be read (not UTF-8, no permission) is not a file that is gone:
                // it is refused, as `Checks::for_project` refused it by naming the path.
                Err(e) => Seen::Unreadable(e.to_string()),
            }
        }
    };
    if same_file && view.seen.as_ref() == Some(&seen) {
        view.followed = Some(followed);
        return;
    }
    if !same_file {
        // Another project has no earlier value of its own: nothing carries over.
        view.good = None;
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&["checks"]);
        }
    }
    view.path = path.clone();
    view.followed = Some(followed);
    view.seen = Some(seen.clone());
    let file = path.strip_prefix(&project.project_dir).unwrap_or(&path).to_string_lossy().replace('\\', "/");
    let decided = match &seen {
        Seen::Text(text) => read(&file, text),
        Seen::Unreadable(why) => Err((None, why.clone())),
        Seen::Missing => {
            // No file is the editor's own checks, which are valid; the edit that deleted a refused file
            // is the one that ends its refusal.
            view.good = Some(Checks::builtin());
            if let Some(reloads) = reloads.as_deref_mut() {
                reloads.forget(&["checks"]);
            }
            return;
        }
    };
    let verdict = match decided {
        Ok(own) => {
            view.good = Some(Checks::builtin().merged(own));
            KindVerdict::applied("checks", file)
        }
        Err((field, reason)) => {
            let refused = KindVerdict::refused("checks", file, field, reason);
            if let Some(state) = state.as_deref_mut() {
                let kept = if view.good.is_some() { "the checks already held stay" } else { "no project checks run until it reads" };
                state.status.raise("", refused.problem(kept));
            }
            refused
        }
    };
    if let Some(reloads) = reloads.as_deref_mut() {
        reloads.forget(&["checks"]);
        reloads.decide(vec![verdict]);
    }
}
