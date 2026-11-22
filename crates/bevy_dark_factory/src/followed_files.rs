//! **What the sibling views that read one file each ask the project index** (#501, items 5 to 8: `data_view`, `notes_view`,
//! `oplog_view`, `checks_view`).
//!
//! Each of them used to `stat` its file every frame and read it again once a second whatever the stat said. They now read the
//! index's [`Snapshot`] instead: **the stamp the index holds for a path is what "it changed" is decided by**, and a view reads its
//! file only when that stamp is not the one its last look was made at. A frame in which the index's generation did not move makes no
//! filesystem call: it builds the path the view follows (an allocation) and does one atomic load.
//!
//! **A look is made at the stamps of one snapshot, taken before the file is read.** A write that lands after the snapshot but before
//! the read is read now and shows as a different stamp in the next snapshot, so it is read once more (the text is the same, which a
//! view decides nothing about); a stamp is never newer than what was read.
//!
//! **When the index cannot answer, the view refuses and says so** ([`Asked::Refuse`], [`reach`]'s error): a path outside the project
//! root, under a folder the index does not follow, named by the root's `.gitignore`, or a symbolic link is a file no index event will
//! ever announce, so reading it once and showing it would be the old poll's silence under another name. There is no fallback read.
//! A link that replaces a file that was not there changes no stamp, so it is noticed at the view's next look, whatever causes one.
//! The root's `.gitignore` is one of the stamps a look is made at, so a rule that arrives later is looked at again. [`ask`] cannot say
//! that the index's thread never started (it keeps no error for that): the view then waits, and the index logs the failure once.
//!
//! **The four views each repeat the same preamble** (build the path, compare the generation, ask, compare the stamps) instead of
//! sharing a function, because each holds a different set of paths and a different state to reset when the project changes.
//!
//! **What this does not catch** [INFERENCE: from how the stamp is made, not measured]: two writes of the same length inside one
//! modification-time tick leave the same stamp, so the index publishes nothing for the second. The retired once-a-second re-read
//! caught that case; nothing does now.

use std::path::Path;
use std::sync::Arc;

use bevy::prelude::*;

use crate::editor::EditorState;
use crate::files::{Ignore, SKIPPED_DIRS};
use crate::project_index::{FileStamp, ProjectIndex, Snapshot};

/// **What asking the index for the project's listing found.**
pub(crate) enum Asked {
    /// The index is not following this project yet, or has not finished its first listing: ask again next frame.
    Wait,
    /// The index cannot list the project (its root cannot be read): said, and nothing is read.
    Refuse(String),
    Ready(Arc<Snapshot>),
}

/// **The index's listing of the open project, or why there is none.** No filesystem call.
pub(crate) fn ask(index: &ProjectIndex, root: &Path) -> Asked {
    if index.root() != Some(root) {
        return Asked::Wait;
    }
    if let Some(why) = index.failure() {
        return Asked::Refuse(why);
    }
    match index.snapshot() {
        Some(snapshot) => Asked::Ready(snapshot),
        None => Asked::Wait,
    }
}

/// **`path` as the index names it**: relative to `root`, with `/`, with `.` and `..` folded away lexically. An error when it is not
/// under `root`, or a part of it is not UTF-8.
fn rel_of(root: &Path, path: &Path) -> Result<String, String> {
    use std::path::Component;
    let outside = || format!("{} is not under the project root {}, so the project index cannot follow it", path.display(), root.display());
    let inside = path.strip_prefix(root).map_err(|_| outside())?;
    let mut parts: Vec<&str> = Vec::new();
    for part in inside.components() {
        match part {
            Component::Normal(name) => parts.push(name.to_str().ok_or_else(|| format!("{} is not UTF-8, so the project index does not list it", path.display()))?),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop().ok_or_else(outside)?;
            }
            Component::RootDir | Component::Prefix(_) => return Err(outside()),
        }
    }
    if parts.is_empty() {
        return Err(outside());
    }
    Ok(parts.join("/"))
}

/// **The stamp the snapshot holds for `rel`**, or none when the index does not list it.
fn listed(snapshot: &Snapshot, rel: &str) -> Option<FileStamp> {
    snapshot.files.binary_search_by(|(listed, _)| listed.as_str().cmp(rel)).ok().map(|i| snapshot.files[i].1)
}

/// **Why the index would not list `rel` however it came to exist**: a part of it is a skipped folder, or the root's `.gitignore` names
/// it or a folder above it. Reads the `.gitignore` (counted in `looks`), so it is asked of a path the snapshot does not list and only
/// when a look is made.
fn kept_out(root: &Path, rel: &str, looks: &mut u64) -> Option<String> {
    let parts: Vec<&str> = rel.split('/').collect();
    if let Some(skipped) = parts[..parts.len() - 1].iter().find(|part| SKIPPED_DIRS.contains(part)) {
        return Some(format!("`{rel}` is under `{skipped}`, a folder the project index does not follow"));
    }
    *looks += 1;
    let text = match std::fs::read_to_string(root.join(".gitignore")) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => return Some(format!("{} cannot be read ({e}), so it cannot be said whether the project index follows `{rel}`", root.join(".gitignore").display())),
    };
    let ignore = Ignore::parse(&text);
    (1..=parts.len()).find_map(|n| {
        let at = parts[..n].join("/");
        ignore.matches(&at, n < parts.len()).then(|| format!("`{at}` is named by the project's .gitignore, so the project index does not follow `{rel}`"))
    })
}

/// **What a look at `path` is made at**: its stamp in the snapshot (`None` when it is not listed, which for a file that is not there
/// is the whole truth), or the reason the index can never announce it: outside the project, under a skipped folder, named by the
/// `.gitignore`, or a symbolic link (the index never lists one). **Every filesystem look this makes is added to `looks`** (the
/// `.gitignore` read and the link check), so a view's `reads` count says them too; a path the snapshot lists makes none.
pub(crate) fn reach(root: &Path, snapshot: &Snapshot, path: &Path, looks: &mut u64) -> Result<Option<FileStamp>, String> {
    let rel = rel_of(root, path)?;
    if let Some(stamp) = listed(snapshot, &rel) {
        return Ok(Some(stamp));
    }
    if let Some(why) = kept_out(root, &rel, looks) {
        return Err(why);
    }
    *looks += 1;
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            Err(format!("{} is a symbolic link, which the project index never lists, so it cannot announce a change to it", path.display()))
        }
        // There and not listed: made since the snapshot, and the next one lists it.
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{} cannot be looked at ({e}), so the project index cannot be said to follow it", path.display())),
    }
}

/// **What a view's last look was made at**: the snapshot's generation, the stamp of each path it looked at, and the root's
/// `.gitignore` (a rule added or removed can make the index stop or start listing a path, which no stamp of the path says).
#[derive(Clone, Debug, Default)]
pub(crate) struct Looked {
    pub generation: u64,
    stamps: Vec<(std::path::PathBuf, Option<FileStamp>)>,
}

impl Looked {
    pub fn new(snapshot: &Snapshot, root: &Path, mut stamps: Vec<(std::path::PathBuf, Option<FileStamp>)>) -> Looked {
        stamps.push((root.join(".gitignore"), listed(snapshot, ".gitignore")));
        Looked { generation: snapshot.generation, stamps }
    }

    /// **Whether any path this look was made at has another stamp in `snapshot`.** No filesystem call: a path the index cannot follow
    /// has no stamp, the same as the look had.
    pub fn moved(&self, root: &Path, snapshot: &Snapshot) -> bool {
        self.stamps.iter().any(|(path, was)| rel_of(root, path).ok().and_then(|rel| listed(snapshot, &rel)) != *was)
    }
}

/// **Say a refusal once**: `what` is the view, `why` the reason. Logged and put on the status line the first time `why` is not the one
/// held in `said`, and not again while it stands; `None` clears it.
pub(crate) fn say_once(said: &mut Option<String>, what: &str, why: Option<String>, state: Option<&mut EditorState>) {
    if *said == why {
        return;
    }
    if let Some(why) = &why {
        warn!("{what}: {why}");
        if let Some(state) = state {
            state.status.problem(format!("{what}: {why}"));
        }
    }
    *said = why;
}
