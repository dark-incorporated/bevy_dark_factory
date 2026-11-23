//! **A game's data files, as the editor looks at them** (#200, H3).
//!
//! `<blueprint dir>/data.yaml` registers the tuning files a game owns (`data_files`), and the Data panel
//! and `data_set` read them. A game follows them with
//! `bevy_blueprint::live::BlueprintLiveApp::reload_data_file` (#263), and the registry classes the kind
//! `Live`. So the editor owes the author two things, and neither used to be said: that a hand edit which
//! no longer reads (a cut-off save, a bounded leaf that became a word) left the panel with no rows and
//! nothing to say why, and whether an edit which does read is one the game can take.
//!
//! One system looks at the registry and every file it lists when **the project index** ([`crate::project_index`],
//! #501) says one of them moved (the stamp it holds for the path is not the one the last look was made at), and
//! decides into `blueprint_state.reloads`:
//!
//! - kind `game_data`, per file: `refused` with the file, the leaf and why when it does not pass
//!   [`data_files::check`] (or is registered and gone), and `applied` when an edit leaves a file that
//!   reads (the game's own reader says in its own verdict whether it took it);
//! - kind `data_registry`: `applied` when an edit leaves a registry that reads, `refused` with the field
//!   when it does not (the same reader `data_set` uses).
//!
//! **A file seen reading for the first time is no news**: opening a project, or registering a file, is
//! not an edit, so nothing is decided until a file changes or fails. **One edit is one verdict**: a look
//! decides only what differs from the look before, and everything one look finds is one `seq`.
//!
//! **A file the project index cannot follow is refused, not read**: a registered path outside the project, under
//! a skipped folder or named by `.gitignore` is `refused` with that reason, because no event would ever say it moved
//! (see `crate::followed_files`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use bevy::prelude::*;
use blueprint_model::data_files::{self, DataFiles};
use blueprint_model::live::{KindVerdict, Outcome};
use blueprint_model::{reload, reload_check};

use crate::editor::EditorState;
use crate::followed_files::{self, Asked, Looked};
use crate::live::EditorReloads;
use crate::project::Project;
use crate::project_index::ProjectIndex;

/// What a look at one file found.
#[derive(Clone, PartialEq)]
enum Look {
    /// It reads, and this is the hash of its text.
    Reads(u64),
    /// It does not, or the registry names it and it is not there.
    Refused { field: Option<String>, reason: String },
}

/// **What the editor last made of the open project's data registry and the files it lists.**
#[derive(Resource, Default)]
pub struct DataView {
    /// The blueprint directory this describes; another project starts the view afresh.
    dir: PathBuf,
    /// What the last look was made at: the index's stamp of the registry first, then of every file it listed.
    followed: Option<Looked>,
    /// Why the index could not be asked, said once.
    unanswered: Option<String>,
    /// **How many times this view has looked at the filesystem**: each file read (the registry, and each file it lists), the
    /// `.gitignore` read and the symbolic-link check a path the index does not list needs. A test says a change to another file
    /// did not make it look.
    reads: u64,
    /// By the file as a verdict names it: its kind and what the last look found.
    last: BTreeMap<String, (&'static str, Look)>,
    /// By the file: the hash of the text first seen reading, kept while the project stays open even when a
    /// refused registry takes the file out of `last`.
    baseline: BTreeMap<String, u64>,
}

impl DataView {
    /// **How many filesystem looks this view has made** (see the field).
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

/// **Look at the registry and the files it lists, and decide what differs from the look before.**
///
/// A frame in which the project index's generation did not move makes no filesystem call (one atomic load, after the path is built). When it did, the
/// registry and the files the last look listed are compared by the stamp the index holds for each, and they
/// are read only when one differs. A registry that is gone clears every verdict this view made.
pub fn sync_data_view(
    index: Res<ProjectIndex>,
    project: Option<Res<Project>>,
    mut view: ResMut<DataView>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let Some(project) = project else { return };
    let dir = project.blueprint_dir.clone();
    let same_dir = view.dir == dir;
    if same_dir && view.followed.as_ref().is_some_and(|f| f.generation == index.generation()) {
        return;
    }
    let snapshot = match followed_files::ask(&index, &project.root) {
        Asked::Wait => return,
        Asked::Refuse(why) => {
            followed_files::say_once(&mut view.unanswered, "the data files", Some(why), state.as_deref_mut());
            return;
        }
        Asked::Ready(snapshot) => snapshot,
    };
    followed_files::say_once(&mut view.unanswered, "the data files", None, None);
    if same_dir && let Some(followed) = view.followed.as_mut() && !followed.moved(&project.root, &snapshot) {
        followed.generation = snapshot.generation;
        return;
    }
    if !same_dir {
        view.dir = dir.clone();
        view.last.clear();
        view.baseline.clear();
        view.followed = None;
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&["data_registry", "game_data"]);
        }
    }

    let registry = dir.join(data_files::REGISTRY_FILE);
    let mut stamps = Vec::new();
    // Every file read and every filesystem look `reach` made.
    let mut reads = 0u64;
    let mut now: BTreeMap<String, (&'static str, Look)> = BTreeMap::new();
    let reg_name = crate::map_files::file_name(&project, &registry);
    let registry_reach = followed_files::reach(&project.root, &snapshot, &registry, &mut reads);
    stamps.push((registry.clone(), registry_reach.clone().unwrap_or(None)));
    let registry_text = match registry_reach {
        Err(why) => Err(why),
        Ok(_) => {
            reads += 1;
            match std::fs::read_to_string(&registry) {
                Ok(text) => Ok(Some(text)),
                // No registry is no data files to say anything about.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.to_string()),
            }
        }
    };
    match registry_text {
        Err(reason) => {
            now.insert(reg_name.clone(), ("data_registry", Look::Refused { field: None, reason }));
        }
        Ok(None) => {}
        Ok(Some(text)) => match reload::kind_by_id("data_registry") {
            None => {
                now.insert(reg_name.clone(), ("data_registry", Look::Refused { field: None, reason: "the editor has no `data_registry` kind of reload".to_owned() }));
            }
            Some(kind) => match reload_check::check(kind, &reg_name, &text) {
                reload_check::Check::Refused(r) => {
                    now.insert(reg_name.clone(), ("data_registry", Look::Refused { field: r.field, reason: r.reason }));
                }
                _ => {
                    now.insert(reg_name.clone(), ("data_registry", Look::Reads(hash_of(&text))));
                    if let Ok(registered) = DataFiles::parse(&text) {
                        for f in &registered.files {
                            let path = dir.join(&f.path);
                            let name = crate::map_files::file_name(&project, &path);
                            let file_reach = followed_files::reach(&project.root, &snapshot, &path, &mut reads);
                            stamps.push((path.clone(), file_reach.clone().unwrap_or(None)));
                            let look = match file_reach {
                                Err(why) => Look::Refused { field: None, reason: why },
                                Ok(_) => {
                                    reads += 1;
                                    match std::fs::read_to_string(&path) {
                                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                                            Look::Refused { field: None, reason: format!("`{}` is registered in data.yaml and is not there", f.path) }
                                        }
                                        Err(e) => Look::Refused { field: None, reason: e.to_string() },
                                        Ok(text) => match data_files::check(&name, f, &text) {
                                            Ok(()) => Look::Reads(hash_of(&text)),
                                            Err(r) => Look::Refused { field: r.field, reason: r.reason },
                                        },
                                    }
                                }
                            };
                            now.insert(name, ("game_data", look));
                        }
                    }
                }
            },
        },
    }
    view.followed = Some(Looked::new(&snapshot, &project.root, stamps));
    view.reads += reads;

    let view = &mut *view;
    let mut verdicts = Vec::new();
    for (file, (kind, look)) in &now {
        let before = view.last.get(file).map(|(_, l)| l);
        if before == Some(look) {
            continue;
        }
        let verdict = match look {
            Look::Reads(hash) if *kind == "game_data" => {
                // The text the editor first saw reading is what a game started with, as far as it can
                // tell. A file first seen refused has no baseline, so what mends it is news.
                match (view.baseline.get(file).copied(), before) {
                    (None, None) => {
                        view.baseline.insert(file.clone(), *hash);
                        continue;
                    }
                    // A file registered again unchanged: nothing is waiting.
                    (Some(b), None) if b == *hash => continue,
                    // Any other change that reads is an edit the game can take, whoever made it: the
                    // game's own reader (`reload_data_file`) says in its verdict whether it did.
                    _ => KindVerdict::applied(*kind, file.clone()),
                }
            }
            // A registry seen reading for the first time is a baseline too.
            Look::Reads(_) if before.is_none() => continue,
            Look::Reads(_) => KindVerdict::applied(*kind, file.clone()),
            Look::Refused { field, reason } => KindVerdict::refused(*kind, file.clone(), field.clone(), reason.clone()),
        };
        if verdict.outcome == Outcome::Refused
            && let Some(state) = state.as_deref_mut()
        {
            state.status.problem(format!("{}: refused", verdict.sentence()));
        }
        verdicts.push(verdict);
    }
    if let Some(reloads) = reloads.as_deref_mut() {
        for (file, (kind, _)) in &view.last {
            if !now.contains_key(file) {
                reloads.forget_file(kind, file);
            }
        }
        if !verdicts.is_empty() {
            reloads.decide(verdicts);
        }
    }
    view.last = now;
}
