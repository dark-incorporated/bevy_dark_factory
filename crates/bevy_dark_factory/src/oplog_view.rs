//! **The operation log, as the editor looks at it** (#200, H3).
//!
//! `assets/blueprint/oplog.jsonl` is append-only text a person can edit and a crash can cut short. The
//! readers that walk it (the steps feed, `blueprint/oplog_read`, the after-edit queue) skip a line that is
//! not an entry, so a damaged line cost the author what that line recorded and said nothing about it:
//! the journal counted "line(s) of the log could not be read" with no line and no field. The undo walk
//! refused at one, but without naming the field.
//!
//! Nothing holds the log, so there is no last good value to keep; what the editor owes is the verdict.
//! One system looks at the file when the project index ([`crate::project_index`], #501) says it moved and decides
//! it into `blueprint_state.reloads` as kind `oplog`: `applied` when every line reads as an [`Entry`], or
//! `refused` with the file, the line and the field of the first line that does not (the same reader an
//! undo uses, [`reload_check::read_lines`]), and the reason, which counts the lines after it.
//!
//! **One edit is one verdict.** The editor appends a line for every operation, and a log that still
//! reads is not news, so a look decides only when what it found differs from the look before: a log
//! that went from clean to damaged, from one damage to another, or back. Frames in between decide
//! nothing.
//!
//! **This view is not on the undo path.** An undo walks the file itself (`ops`), at the moment it is asked; nothing it
//! needs is held here, so the index's debounce cannot delay it. The view only says, after the index has noticed, whether
//! the log still reads.
//!
//! **An append is a `Modified` event for the whole file, and the view reads the whole file again, once** (the stamp says the
//! length and the modification time moved, not where): a rewrite that keeps the prefix cannot be told from an append without reading
//! the prefix, so the verdict needs every line. Measured on `big` by `tests/the_oplog_view_follows_the_project_index.rs`: one
//! dispatched operation appended 544 bytes to a log of 5,000 lines (2.5 MB) and was one `Modified` event and one read, and the frame
//! that read took 36.7 ms; at 50,000 lines (24.8 MB) 552 bytes, one event, one read, 426.9 ms. The frame time is the whole frame, on the
//! UI thread; the retired view read the whole file on every change of its stamp as well, which is read from the old code and was not
//! measured.

use std::path::PathBuf;

use bevy::prelude::*;
use blueprint_model::live::KindVerdict;
use blueprint_model::reload_check;

use crate::editor::EditorState;
use crate::followed_files::{self, Asked, Looked};
use crate::live::EditorReloads;
use crate::ops::Entry;
use crate::project::Project;
use crate::project_index::ProjectIndex;

/// **What a look at the log found.**
#[derive(Clone, PartialEq)]
enum Seen {
    /// No log yet: a project nobody has made a change in.
    Missing,
    /// Every line reads.
    Clean,
    /// A line does not: the first one, as `line 3.seq` (or `line 3`), and why.
    Damaged { field: String, reason: String },
    /// The file is there and cannot be read at all (not UTF-8, no permission), or the project index cannot follow it.
    Unreadable(String),
}

/// **What the editor last made of the open project's operation log.**
#[derive(Resource, Default)]
pub struct OplogView {
    /// The log this describes; another project's log starts the view afresh.
    path: PathBuf,
    /// What the last look was made at: the index's stamp of the log.
    followed: Option<Looked>,
    /// Why the index could not be asked, said once.
    unanswered: Option<String>,
    /// **How many times the view has looked at the filesystem**: the log read, and the `.gitignore` read and symbolic-link check a
    /// path the index does not list needs. A test says a change to another file did not make the view look.
    reads: u64,
    /// A hash of the text last parsed, so the same text is not parsed again.
    text: Option<u64>,
    last: Option<Seen>,
}

impl OplogView {
    /// **How many filesystem looks the view has made** (see the field).
    pub fn reads(&self) -> u64 {
        self.reads
    }
}

fn hash_of(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// **Look at the project's `oplog.jsonl` when the index says it moved, and decide what it found once.**
///
/// A frame in which the index's generation did not move makes no filesystem call (one atomic load, after the path is built); when it did, the log is read only
/// when the stamp the index holds for it is not the one the last look was made at. A read that finds the text it
/// found last time is not parsed again.
///
/// A log that is gone clears the verdict, as a removed file ends any refusal of it.
pub fn sync_oplog_view(
    index: Res<ProjectIndex>,
    project: Option<Res<Project>>,
    mut view: ResMut<OplogView>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Some(project) = project else { return };
    let path = crate::ops::oplog_path(&project);
    let same_file = view.path == path;
    if same_file && view.followed.as_ref().is_some_and(|f| f.generation == index.generation()) {
        return;
    }
    let snapshot = match followed_files::ask(&index, &project.root) {
        Asked::Wait => return,
        Asked::Refuse(why) => {
            followed_files::say_once(&mut view.unanswered, "the operation log", Some(why), state.as_deref_mut());
            return;
        }
        Asked::Ready(snapshot) => snapshot,
    };
    followed_files::say_once(&mut view.unanswered, "the operation log", None, None);
    if same_file && let Some(followed) = view.followed.as_mut() && !followed.moved(&project.root, &snapshot) {
        followed.generation = snapshot.generation;
        return;
    }
    if !same_file {
        view.last = None;
        view.text = None;
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&["oplog"]);
        }
    }
    view.path = path.clone();
    let file = crate::map_files::file_name(&project, &path);
    let mut looks = 0u64;
    let reach = followed_files::reach(&project.root, &snapshot, &path, &mut looks);
    view.reads += looks;
    view.followed = Some(Looked::new(&snapshot, &project.root, vec![(path.clone(), reach.clone().unwrap_or(None))]));
    let seen = match reach {
        // **No event will ever say this file moved**, so it is not read: refused, with the reason.
        Err(why) => {
            view.text = None;
            Seen::Unreadable(why)
        }
        Ok(_) => {
            view.reads += 1;
            match std::fs::read_to_string(&path) {
                Ok(text) => {
                    let hash = hash_of(&text);
                    let unchanged = if view.text == Some(hash) { view.last.clone() } else { None };
                    match unchanged {
                        Some(last) => last,
                        None => {
                            view.text = Some(hash);
                            match reload_check::read_lines::<Entry>("oplog", &file, &text) {
                                Ok(_) => Seen::Clean,
                                Err(refusal) => Seen::Damaged { field: refusal.field.unwrap_or_default(), reason: refusal.reason },
                            }
                        }
                    }
                }
                Err(e) => {
                    view.text = None;
                    if e.kind() == std::io::ErrorKind::NotFound { Seen::Missing } else { Seen::Unreadable(e.to_string()) }
                }
            }
        }
    };
    if view.last.as_ref() == Some(&seen) {
        return;
    }
    view.last = Some(seen.clone());
    let verdict = match seen {
        Seen::Missing => {
            if let Some(reloads) = reloads.as_deref_mut() {
                reloads.forget(&["oplog"]);
            }
            return;
        }
        Seen::Clean => KindVerdict::applied("oplog", file),
        Seen::Damaged { field, reason } => {
            let reason = format!("{reason}; the log's readers skip a line like it and an undo refuses until it is repaired");
            KindVerdict::refused("oplog", file, Some(field), reason)
        }
        Seen::Unreadable(why) => KindVerdict::refused("oplog", file, None, why),
    };
    if verdict.outcome != blueprint_model::live::Outcome::Applied
        && let Some(state) = state.as_deref_mut()
    {
        state.status.problem(format!("{}: refused", verdict.sentence()));
    }
    if let Some(reloads) = reloads.as_deref_mut() {
        reloads.decide(vec![verdict]);
    }
}
