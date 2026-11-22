//! **The pictures on the project's boards, as the editor looks at them** (#200, H3; SG-03, #402).
//!
//! A picture on a board (`board_set`) is a project-relative picture an agent is told to look at. Nothing in
//! this repository decodes it, so a file that a full disk cut off, that a text saved over or that was
//! renamed from another format cost the agent a picture it could not open, and the author a sentence
//! nobody said. One system looks at every board's `board.yaml` and every picture they list when the project
//! index says one of them moved, and decides into `blueprint_state.reloads`, kind `reference_images`, per
//! file:
//!
//! - `refused` with the file and the header field (`signature`, `format`, `header`, `width`, `height`;
//!   [`blueprint_model::image_header`]) and why, when it is no longer the picture its name says;
//! - `applied` when a refused picture is a picture again, or an edit left one that reads as other
//!   dimensions.
//!
//! **A picture first seen reading is no news**: opening a map, or adding a picture, is not an edit.
//! **A listed picture that is not there is not decided here**: `blueprint_state.board` already says
//! `missing: true` for it, and a project may name a file it is about to add. **One edit is one verdict**:
//! a look decides only what differs from the look before, and everything one look finds is one `seq`.
//!
//! The pixels are never decoded and the file is never written: a picture is read a few kilobytes at a
//! time (a JPEG's frame header can sit behind a large profile, so a header that is not reached is read
//! again from the first megabyte).
//!
//! # How it knows a picture moved (#501)
//!
//! It subscribes to the project index ([`crate::project_index`]) and has no clock of its own. The index's listing says every file's
//! size and modification time, so a look compares the stamp the listing holds with the one the picture was read at, and **reads only a
//! board or a picture whose stamp differs**; a project change that touches neither reads nothing. The boards are found in the
//! listing (the design folder's `board/board.yaml` files, [`board_files_in`]), not by walking the folders. **A picture replaced in
//! place, under the same name, with other bytes, moves its modification time and is read again**; one replaced by bytes of the same
//! length inside the same filesystem timestamp tick is indistinguishable by a size and a time, which the index never goes beyond (it reads
//! no file's bytes), and so is not seen. The poll this replaces re-read every picture once a second and caught that case; whether any
//! filesystem the editor runs on can produce it with a real save was not measured. [INFERENCE]
//!
//! **What the index does not list is said, not read**: a board or picture under `.git`, `target`, `.live`, `node_modules` or a folder the
//! project's `.gitignore` names, or outside the project, is not followed, so it is warned about once and decides nothing, rather than
//! read from disk behind the index's back.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use bevy::prelude::*;
use blueprint_model::board::{self, Board};
use blueprint_model::image_header::{self, Header};
use blueprint_model::live::{KindVerdict, Outcome};

use crate::editor::EditorState;
use crate::live::EditorReloads;
use crate::project::{OpenMap, Project};
use crate::project_index::{FileStamp, ProjectIndex, Snapshot};
use crate::world_lists::stamp_of;

/// The kind this view decides.
const KIND: &str = "reference_images";

/// What a first read of a picture takes: every header but a JPEG's is inside it.
const FIRST_READ: usize = 4096;

/// What a look at one picture found.
#[derive(Clone, PartialEq)]
enum Look {
    /// It is the picture its name says.
    Reads(Header),
    /// It is not, or it could not be read.
    Refused { field: Option<String>, reason: String },
}

/// What the index said of a file when it was read: its stamp, `None` when it is not there, or why the index cannot answer for it.
type Stamp = Result<Option<FileStamp>, String>;

/// **What the editor last made of the pictures on the boards.**
#[derive(Resource, Default)]
pub struct ReferenceView {
    /// The map this describes; another map (or project) starts the view afresh.
    map_path: PathBuf,
    /// The project's root the pictures are under.
    root: PathBuf,
    /// The index generation the files were last looked at.
    generation: u64,
    looked: bool,
    /// Every board file looked at, by its path under the project: its stamp when it was read, and the pictures it listed.
    boards: BTreeMap<String, (Stamp, Vec<String>)>,
    /// Every picture looked at, by the path the reference names it: its stamp when it was read.
    stamps: BTreeMap<String, Stamp>,
    /// By the picture's path as the reference names it: what the last look found.
    last: BTreeMap<String, Look>,
    /// The files the index does not follow that were warned about, so each is said once.
    unfollowed: BTreeSet<String>,
    /// Why the index had no listing, when it did not, so it is said once.
    failure: Option<String>,
    /// How many pictures have been opened to read a header.
    pictures_read: u64,
}

impl ReferenceView {
    /// **How many pictures this view has opened to read a header** since the view was last cleared (a door with no map open): a test reads it across frames to say that a
    /// picture whose stamp did not move was not read again, and that a look at a picture that did move happened. A board file is not counted.
    pub fn pictures_read(&self) -> u64 {
        self.pictures_read
    }

    /// **The header the last look read from a picture**, by the path its board names it: `None` when the picture has not been looked at,
    /// is not there, or is refused.
    pub fn header_of(&self, picture: &str) -> Option<Header> {
        match self.last.get(picture) {
            Some(Look::Reads(header)) => Some(*header),
            _ => None,
        }
    }
}

/// The start of a file, `limit` bytes of it at most.
fn head_of(path: &Path, limit: usize) -> std::io::Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take(limit as u64).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// **Look at one picture**, or `None` when it is not there. The first few kilobytes are read; a header that
/// ends before its width or height there is read again from as much as [`image_header::HEAD`] of the file,
/// because that is how a JPEG with a large profile in front of its frame header looks.
fn look_at(path: &Path, name: &str) -> Option<Look> {
    let mut bytes = match head_of(path, FIRST_READ) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => return Some(Look::Refused { field: None, reason: e.to_string() }),
    };
    let mut cut = bytes.len() == FIRST_READ;
    let mut read = image_header::read(name, &bytes);
    if cut && matches!(&read, Err(r) if matches!(r.field.as_deref(), Some("width" | "height"))) {
        match head_of(path, image_header::HEAD) {
            Ok(more) => {
                cut = more.len() == image_header::HEAD;
                bytes = more;
                read = image_header::read(name, &bytes);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => return Some(Look::Refused { field: None, reason: e.to_string() }),
        }
    }
    Some(match read {
        Ok(header) => Look::Reads(header),
        Err(refusal) => {
            let note = if cut && matches!(refusal.field.as_deref(), Some("width" | "height")) { " (only the first megabyte was read)" } else { "" };
            Look::Refused { field: refusal.field, reason: format!("{}{note}", refusal.reason) }
        }
    })
}

/// A reference path that stays inside the project: `normalise` makes every path the editor writes one, and
/// a hand edit of the file may not be.
fn inside(path: &str) -> bool {
    !path.is_empty() && Path::new(path).components().all(|c| matches!(c, Component::Normal(_)))
}

/// **The places that have a board file, from the index's listing**: what [`board::places_with_a_board`] finds by walking the design
/// folder, found without a `stat`. A place is a folder under `design/` (at most [`board::WALK_DEPTH_MAX`] folders deep, none hidden and
/// none named `board`) with `board/board.yaml` in it. In the listing's order, which is by path.
fn board_places_in(snapshot: &Snapshot) -> Vec<&str> {
    let design = format!("{}/", blueprint_model::design::DESIGN_DIR);
    let from = snapshot.files.partition_point(|(rel, _)| rel.as_str() < design.as_str());
    let tail = format!("/{}/{}", board::BOARD_DIR, board::BOARD_FILE);
    snapshot.files[from..]
        .iter()
        .map_while(|(rel, _)| rel.starts_with(&design).then_some(rel.as_str()))
        .filter_map(|rel| rel.strip_suffix(tail.as_str()))
        .filter(|place| {
            let Some(below) = place.strip_prefix(blueprint_model::design::DESIGN_DIR) else { return false };
            let below = below.trim_start_matches('/');
            let folders: Vec<&str> = below.split('/').filter(|f| !f.is_empty()).collect();
            folders.len() <= board::WALK_DEPTH_MAX && folders.iter().all(|f| !f.starts_with('.') && *f != board::BOARD_DIR)
        })
        .collect()
}

/// **Every board file the view looks at**, by path under the project: the open level's first (**whether it exists yet or not**: a board made
/// now is seen as soon as the index has it), then every other place's in the listing.
fn board_files_in(snapshot: &Snapshot, level_file: &str) -> Vec<String> {
    let mut files = vec![level_file.to_owned()];
    files.extend(board_places_in(snapshot).into_iter().map(board::file_of).filter(|file| file != level_file));
    files
}

/// **Say once that the index does not follow a file**, and that nothing is decided about it.
fn say_unfollowed(unfollowed: &mut BTreeSet<String>, file: &str, why: &str) {
    if unfollowed.insert(file.to_owned()) {
        warn!("reference images: {file} is not looked at: {why}");
    }
}

/// **The pictures a board lists** (the paths as it names them): none for a board that is not there, and a board that does not read is
/// logged here (the `board` kind says it to the person) and lists none.
fn pictures_of(path: &Path) -> Vec<String> {
    match Board::load(path) {
        Ok(board) => board.pictures.into_iter().map(|picture| picture.path).collect(),
        Err(why) => {
            warn!("reference images: {why}");
            Vec::new()
        }
    }
}

/// **Look at the pictures on every board, and decide what differs from the look before.**
///
/// Looked at when the project index says anything in the project moved, and then only the boards and pictures whose stamp differs
/// from the one they were read at are read again. A map that is no longer open, or another one, clears every verdict this view made.
pub fn sync_reference_view(
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    index: Res<ProjectIndex>,
    mut view: ResMut<ReferenceView>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
) {
    let (Some(project), Some(open)) = (project, open) else {
        // A door the map is not open in: what was said about its pictures is not news any more.
        if view.looked {
            *view = ReferenceView::default();
            if let Some(reloads) = reloads.as_deref_mut() {
                reloads.forget(&[KIND]);
            }
        }
        return;
    };
    // **The index is retargeted before the frame's systems run**, so a root it does not follow is a project it has not started on: wait.
    if index.root() != Some(project.root.as_path()) {
        return;
    }
    if let Some(why) = index.failure() {
        // **The index cannot answer, so nothing is decided**, and the failure is said once; the next listing looks again.
        if view.failure.as_ref() != Some(&why) {
            warn!("reference images: the project index cannot list {}: {why}", project.root.display());
            view.failure = Some(why);
        }
        view.generation = 0;
        return;
    }
    if view.failure.is_some() {
        view.failure = None;
    }
    let generation = index.generation();
    let same = view.looked && view.map_path == open.map_path && view.root == project.root;
    if generation == 0 || (same && view.generation == generation) {
        return;
    }
    let Some(snapshot) = index.snapshot() else { return };
    let view = &mut *view;
    view.generation = generation;
    let root = project.root.clone();
    if !same {
        view.map_path = open.map_path.clone();
        view.root = root.clone();
        view.last.clear();
        view.boards.clear();
        view.stamps.clear();
        view.unfollowed.clear();
        view.looked = false;
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.forget(&[KIND]);
        }
    }

    // A board file that does not read is the `board` kind's to say; nothing is listed from it.
    let level_file = board::file_of(&board::level_place(&root, &open.map.name));
    let mut boards: BTreeMap<String, (Stamp, Vec<String>)> = BTreeMap::new();
    let mut stamps: BTreeMap<String, Stamp> = BTreeMap::new();
    let mut now: BTreeMap<String, Look> = BTreeMap::new();
    for file in board_files_in(&snapshot, &level_file) {
        let path = root.join(&file);
        // **The stamp is the listing's, taken before the read**: a write that lands during it moves the stamp again.
        let stamp = stamp_of(&snapshot, &root, &path);
        let pictures = match (view.boards.get(&file), &stamp) {
            (Some((seen, pictures)), _) if *seen == stamp => pictures.clone(),
            (_, Ok(Some(_))) => pictures_of(&path),
            (_, Ok(None)) => Vec::new(),
            (_, Err(why)) => {
                say_unfollowed(&mut view.unfollowed, &file, why);
                Vec::new()
            }
        };
        for picture in &pictures {
            if !inside(picture) {
                continue;
            }
            let path = root.join(picture);
            let stamp = stamp_of(&snapshot, &root, &path);
            let look = match (view.stamps.get(picture), &stamp) {
                (Some(seen), _) if *seen == stamp => view.last.get(picture).cloned(),
                (_, Ok(Some(_))) => {
                    view.pictures_read += 1;
                    look_at(&path, picture)
                }
                (_, Ok(None)) => None,
                (_, Err(why)) => {
                    say_unfollowed(&mut view.unfollowed, picture, why);
                    None
                }
            };
            if let Some(look) = look {
                now.insert(picture.clone(), look);
            }
            stamps.insert(picture.clone(), stamp);
        }
        boards.insert(file, (stamp, pictures));
    }
    view.boards = boards;
    view.stamps = stamps;
    view.looked = true;

    let mut verdicts = Vec::new();
    for (file, look) in &now {
        let before = view.last.get(file);
        if before == Some(look) {
            continue;
        }
        let verdict = match look {
            // A picture seen reading for the first time is a baseline.
            Look::Reads(_) if before.is_none() => continue,
            Look::Reads(_) => KindVerdict::applied(KIND, file.clone()),
            Look::Refused { field, reason } => KindVerdict::refused(KIND, file.clone(), field.clone(), reason.clone()),
        };
        if verdict.outcome == Outcome::Refused
            && let Some(state) = state.as_deref_mut()
        {
            state.status.problem(format!("{}: refused", verdict.sentence()));
        }
        verdicts.push(verdict);
    }
    if let Some(reloads) = reloads.as_deref_mut() {
        // A picture that is gone, or no longer listed, is no longer refused.
        for file in view.last.keys().filter(|f| !now.contains_key(*f)) {
            reloads.forget_file(KIND, file);
        }
        if !verdicts.is_empty() {
            reloads.decide(verdicts);
        }
    }
    view.last = now;
}
