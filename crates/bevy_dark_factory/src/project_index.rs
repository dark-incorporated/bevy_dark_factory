//! **The one watcher, and the index that follows it** (#501, stage 1).
//!
//! # The gap this closes
//!
//! Twenty-two poll constants in seventeen modules each re-stat the project on their own clock, so the same file is read by
//! several panels at several rhythms, and two lists that should agree ([`crate::files::listed_files`] and what the finder's
//! scan walked) were two walks. **Nothing in the editor owned the answer to "did a file change?"** This module does: one
//! background thread is the only code that looks at the project's directory tree, and everything else subscribes.
//!
//! # What is published
//!
//! [`ProjectFileChanged`], a Bevy [`Message`], **once per changed entry** (not once per subscriber: each subscriber has its own
//! `MessageReader` cursor over the one message). It carries `(rel, kind, stamp)` — [`watch::Due`](crate::watch::Due)'s shape
//! with the kind said — and `dir`, because a folder appearing is a change a tree wants too.
//!
//! **Debounced by [`DEBOUNCE`].** One Ctrl+S arrives as a create, several writes and a close (or, for an editor that saves
//! atomically, a remove and a rename). A change is held until its stamp has been steady for [`DEBOUNCE`], then diffed against
//! what the index holds, so a save is one event, and a file that ends where it began is none. **An operation that has just
//! finished writing** (`file_create`, `file_rename`, `file_delete`) says so with [`note_written`] and is published at once: it
//! knows the write is done, and the row it made must be on screen in the next frame.
//!
//! # How the thread finds changes, with no new dependency
//!
//! The crate has no `notify` (it is behind the off-by-default `asset_spike` feature), so the thread **sweeps**, on the one clock
//! that stays ([`SWEEP`]): it stats each folder it knows (a folder's mtime moves when an entry is added, removed or renamed) and
//! re-reads only the folders that moved, and it stats each file it knows. **It never walks the project again**: the first listing
//! is one walk, whose `symlink_metadata` calls already give every stamp, and from then on the index is the fold of the events. The
//! two exceptions are named: a `.gitignore` change re-walks (it changes what is listed), and a folder written in the last
//! [`RACY`] is re-read on the next sweep as well (git's racy-clean rule), so two changes inside one clock tick are not lost.
//! A sweep runs on the thread, so the UI thread's frame makes no lookup at all.
//!
//! # How a subscriber reads it
//!
//! `Res<ProjectIndex>`: [`ProjectIndex::generation`] says whether anything moved (an atomic load), [`ProjectIndex::snapshot`]
//! is an `Arc` clone of the listing (no filesystem), and a `MessageReader<ProjectFileChanged>` says which path moved. A caller
//! with no world (`files_search`, `ops_files_verbs`) calls [`crate::files::listed_files`], which asks the registered index for its
//! root to settle and answers from it, and walks cold only when no index exists for that root.
//!
//! # What this does not touch
//!
//! `watch.rs` keeps deciding what the editor adopts: its `adopt_map` refusal is a correctness rule about unsaved work, not a poll,
//! and this module only says that a path moved.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard, Weak, mpsc};
use std::time::{Duration, Instant, SystemTime};

use bevy::prelude::*;

use crate::files::{Ignore, SKIPPED_DIRS, TreeBuild};
use crate::project::Project;

/// **How often the thread looks at what it knows**: the one poll clock that stays when the per-module ones retire into it.
pub const SWEEP: Duration = Duration::from_secs(1);

/// **How long a change must be steady before it is published**: Renzora's number, for the same reason (one save is a create,
/// several writes and a close).
pub const DEBOUNCE: Duration = Duration::from_millis(100);

/// How often a held change is looked at again while it is settling.
const TICK: Duration = Duration::from_millis(25);

/// **A folder written this recently is read again on the next sweep**: a second change in the same clock tick would leave its
/// mtime where the first read saw it.
const RACY: Duration = Duration::from_secs(2);

/// How long a caller with no world waits for the thread to settle before it walks for itself.
const SETTLE_WAIT: Duration = Duration::from_secs(10);

/// **A file's size and modification time**: what "it changed" is decided by, never a hash (nothing here reads a file's bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileStamp {
    pub len: u64,
    pub modified: Option<SystemTime>,
}

/// **What happened to an entry.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Created,
    Modified,
    Removed,
}

/// **A path under the project changed**, published once for the whole editor.
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub struct ProjectFileChanged {
    /// The path under the project, with `/` separators.
    pub rel: String,
    pub kind: ChangeKind,
    /// The stamp after the change; `None` for a removal.
    pub stamp: Option<FileStamp>,
    /// Whether the entry is a folder. A folder is only ever created or removed, never modified.
    pub dir: bool,
}

/// What the index holds of one entry. A folder's `settled` says its mtime is old enough to be trusted (see [`RACY`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    dir: bool,
    stamp: FileStamp,
    settled: bool,
}

/// **The listing as of one generation**, immutable and shared: readers clone the `Arc`, never the listing.
#[derive(Debug)]
pub struct Snapshot {
    /// Unique across every index in the process, and larger after every applied change.
    pub generation: u64,
    /// Every file (not folder), sorted by path, with its stamp: no cap.
    pub files: Vec<(String, FileStamp)>,
    /// The tree the Files workspace draws, with its caps; rebuilt only when the structure moved.
    pub tree: Arc<TreeBuild>,
}

// ── The seam every lookup goes through ───────────────────────────────────────

thread_local! {
    static LOOKUPS: Cell<u64> = const { Cell::new(0) };
}

/// **How many `stat` and `read_dir` calls the index's seam has made on this thread.** A test reads it across a frame to say the
/// UI thread looked at nothing.
pub fn lookups_on_this_thread() -> u64 {
    LOOKUPS.with(Cell::get)
}

fn counted() {
    LOOKUPS.with(|c| c.set(c.get() + 1));
}

fn look_stat(path: &Path) -> std::io::Result<std::fs::Metadata> {
    counted();
    std::fs::symlink_metadata(path)
}

fn look_dir(path: &Path) -> std::io::Result<std::fs::ReadDir> {
    counted();
    std::fs::read_dir(path)
}

// ── Reading the disk ─────────────────────────────────────────────────────────

fn stamp_of(meta: &std::fs::Metadata) -> FileStamp {
    FileStamp { len: if meta.is_dir() { 0 } else { meta.len() }, modified: meta.modified().ok() }
}

fn settled(modified: Option<SystemTime>, read_at: SystemTime) -> bool {
    modified.is_some_and(|m| read_at.duration_since(m).is_ok_and(|age| age >= RACY))
}

fn full_path(root: &Path, rel: &str) -> PathBuf {
    if rel.is_empty() { root.to_path_buf() } else { root.join(rel) }
}

/// **What the tree lists and the index keeps**: not a skipped folder, not `.DS_Store`, not named by `.gitignore`.
fn admitted(rel: &str, name: &str, is_dir: bool, ignore: &Ignore) -> bool {
    !((is_dir && SKIPPED_DIRS.contains(&name)) || name == ".DS_Store" || ignore.matches(rel, is_dir))
}

fn ignore_of(root: &Path) -> Ignore {
    std::fs::read_to_string(root.join(".gitignore")).map(|t| Ignore::parse(&t)).unwrap_or_default()
}

/// **One look at one path**: `None` for a path that is not there or is a link (a link is never followed and never listed).
fn observe(root: &Path, rel: &str) -> Option<(bool, FileStamp)> {
    let path = full_path(root, rel);
    let meta = if rel.is_empty() {
        counted();
        std::fs::metadata(&path).ok()?
    } else {
        look_stat(&path).ok()?
    };
    if meta.file_type().is_symlink() {
        return None;
    }
    Some((meta.is_dir(), stamp_of(&meta)))
}

/// **The entries directly inside one folder**, with their stamps, filtered the way the tree filters.
fn read_children(root: &Path, dir: &str, ignore: &Ignore) -> Option<Vec<(String, Entry)>> {
    let read = look_dir(&full_path(root, dir)).ok()?;
    let read_at = SystemTime::now();
    let mut out = Vec::new();
    for entry in read.filter_map(Result::ok) {
        let Ok(meta) = look_stat(&entry.path()) else { continue };
        if meta.file_type().is_symlink() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = meta.is_dir();
        let rel = if dir.is_empty() { name.clone() } else { format!("{dir}/{name}") };
        if !admitted(&rel, &name, is_dir, ignore) {
            continue;
        }
        let stamp = stamp_of(&meta);
        out.push((rel, Entry { dir: is_dir, stamp, settled: is_dir && settled(stamp.modified, read_at) }));
    }
    Some(out)
}

fn walk_into(root: &Path, dir: &str, ignore: &Ignore, stop: &AtomicBool, out: &mut BTreeMap<String, Entry>) {
    if stop.load(Ordering::Relaxed) {
        return;
    }
    for (rel, entry) in read_children(root, dir, ignore).unwrap_or_default() {
        let is_dir = entry.dir;
        out.insert(rel.clone(), entry);
        if is_dir {
            walk_into(root, &rel, ignore, stop, out);
        }
    }
}

/// **The one walk**: every entry under the root, uncapped, with the root itself kept under `""` so its own mtime is followed.
fn walk(root: &Path, ignore: &Ignore, stop: &AtomicBool) -> BTreeMap<String, Entry> {
    let mut out = BTreeMap::new();
    if let Some((_, stamp)) = observe(root, "") {
        out.insert(String::new(), Entry { dir: true, stamp, settled: settled(stamp.modified, SystemTime::now()) });
        walk_into(root, "", ignore, stop, &mut out);
    }
    out
}

/// **The files the project lists when no index is following it**: one walk, on the caller's thread.
fn cold_listing(root: &Path) -> Vec<String> {
    let stop = AtomicBool::new(false);
    walk(root, &ignore_of(root), &stop).into_iter().filter(|(rel, e)| !e.dir && !rel.is_empty()).map(|(rel, _)| rel).collect()
}

fn parent_of(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(parent, _)| parent)
}

// ── The shared state ─────────────────────────────────────────────────────────

/// Handed out to every applied change, from every index in the process, so a generation never repeats across projects.
static GENERATION: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
struct State {
    entries: BTreeMap<String, Entry>,
    snapshot: Option<Arc<Snapshot>>,
    outbox: Vec<ProjectFileChanged>,
    /// Paths an operation has finished writing.
    noted: Vec<String>,
    /// Callers waiting for the thread to settle.
    syncs: Vec<mpsc::Sender<()>>,
}

struct Shared {
    root: PathBuf,
    state: Mutex<State>,
    wake: Condvar,
    stop: AtomicBool,
    /// Set (under the lock) when the outbox has something, so a quiet frame costs one atomic load.
    news: AtomicBool,
    generation: AtomicU64,
    walks: AtomicU64,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic with the lock held cannot leave the state half-applied: an apply is built first and swapped in whole.
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

static REGISTRY: LazyLock<Mutex<HashMap<PathBuf, Weak<Shared>>>> = LazyLock::new(Default::default);

fn registry() -> MutexGuard<'static, HashMap<PathBuf, Weak<Shared>>> {
    REGISTRY.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn registered(root: &Path) -> Option<Arc<Shared>> {
    registry().get(root).and_then(Weak::upgrade).filter(|s| !s.stop.load(Ordering::Relaxed))
}

/// **An operation has finished writing `rel`** (made, renamed away, deleted): the index looks at that path at once and publishes
/// what it finds, with no debounce, because the writer knows the write is done. A no-op when no index follows `root`.
pub fn note_written(root: &Path, rel: &str) {
    let Some(shared) = registered(root) else { return };
    shared.lock().noted.push(rel.to_owned());
    shared.wake.notify_all();
}

/// **The listing as of now, for a caller with no world**: the registered index for `root` settles (one sweep, on its thread) and
/// answers; `None` when no index follows `root`.
fn settled_snapshot(root: &Path) -> Option<Arc<Snapshot>> {
    let shared = registered(root)?;
    let (tx, rx) = mpsc::channel();
    shared.lock().syncs.push(tx);
    shared.wake.notify_all();
    rx.recv_timeout(SETTLE_WAIT).ok()?;
    shared.lock().snapshot.clone()
}

/// **Every file of the project, sorted, with no cap**: from the index that follows `root` when there is one, and from one cold
/// walk when there is not. [`crate::files::listed_files`] is this.
pub fn listed(root: &Path) -> Vec<String> {
    match settled_snapshot(root) {
        Some(snapshot) => snapshot.files.iter().map(|(rel, _)| rel.clone()).collect(),
        None => {
            let mut files = cold_listing(root);
            files.sort();
            files
        }
    }
}

// ── The thread ───────────────────────────────────────────────────────────────

/// A change seen and not yet published: what it looked like last, and since when it has looked like that.
struct Pending {
    seen: Option<(bool, FileStamp)>,
    since: Instant,
    immediate: bool,
}

struct Sweeper {
    shared: Arc<Shared>,
    ignore: Ignore,
    pending: HashMap<String, Pending>,
}

impl Sweeper {
    fn root(&self) -> &Path {
        &self.shared.root
    }

    fn stopped(&self) -> bool {
        self.shared.stop.load(Ordering::Relaxed)
    }

    /// **The first listing**: the one walk. It publishes no events (a first sighting is not a change).
    fn first_listing(&mut self) {
        self.ignore = ignore_of(self.root());
        let entries = walk(self.root(), &self.ignore, &self.shared.stop);
        self.shared.walks.fetch_add(1, Ordering::Relaxed);
        let mut st = self.shared.lock();
        st.entries = entries;
        self.publish(&mut st, true, &[]);
    }

    /// Rebuild the snapshot and queue `events`, under the lock.
    fn publish(&self, st: &mut State, structural: bool, events: &[ProjectFileChanged]) {
        let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        let files = st.entries.iter().filter(|(rel, e)| !e.dir && !rel.is_empty()).map(|(rel, e)| (rel.clone(), e.stamp)).collect();
        let previous = st.snapshot.as_ref().map(|s| s.tree.clone());
        let tree = match previous {
            Some(tree) if !structural => tree,
            _ => Arc::new(crate::files::build_tree(st.entries.iter().filter(|(rel, _)| !rel.is_empty()).map(|(rel, e)| (rel.as_str(), e.dir)))),
        };
        st.snapshot = Some(Arc::new(Snapshot { generation, files, tree }));
        self.shared.generation.store(generation, Ordering::Release);
        if !events.is_empty() {
            st.outbox.extend_from_slice(events);
            self.shared.news.store(true, Ordering::Release);
        }
    }

    /// Hold `rel` as a change that has been seen.
    fn suspect(&mut self, rel: &str, seen: Option<(bool, FileStamp)>) {
        self.pending.entry(rel.to_owned()).or_insert_with(|| Pending { seen, since: Instant::now(), immediate: false });
    }

    /// **An operation's finished write**: looked at now and published at once.
    fn note(&mut self, rel: &str) {
        if rel.is_empty() || !self.listed(rel) {
            return;
        }
        let seen = observe(self.root(), rel);
        self.pending.insert(rel.to_owned(), Pending { seen, since: Instant::now(), immediate: true });
    }

    /// Whether the tree would list `rel`: no part of it is skipped or ignored.
    fn listed(&self, rel: &str) -> bool {
        let parts: Vec<&str> = rel.split('/').collect();
        let mut at = String::new();
        for (n, part) in parts.iter().enumerate() {
            if !at.is_empty() {
                at.push('/');
            }
            at.push_str(part);
            let is_dir = n + 1 < parts.len();
            if !admitted(&at, part, is_dir, &self.ignore) {
                return false;
            }
        }
        true
    }

    /// **Look at everything the index knows.** Folders first (their mtime says an entry came or went, and only a folder that
    /// moved is read), then each file's own stamp. Whatever differs is held as a pending change.
    fn sweep(&mut self) {
        let known: Vec<(String, Entry)> = self.shared.lock().entries.iter().map(|(rel, e)| (rel.clone(), *e)).collect();
        let mut children_of: HashMap<&str, Vec<&str>> = HashMap::new();
        let mut is_known: HashSet<&str> = HashSet::with_capacity(known.len());
        for (rel, _) in &known {
            is_known.insert(rel.as_str());
            if !rel.is_empty() {
                children_of.entry(parent_of(rel)).or_default().push(rel.as_str());
            }
        }
        let mut folder_updates: Vec<(String, Entry)> = Vec::new();
        let now = SystemTime::now();
        for (rel, entry) in &known {
            if self.stopped() {
                return;
            }
            let Some((is_dir, stamp)) = observe(self.root(), rel) else {
                self.suspect(rel, None);
                continue;
            };
            if is_dir != entry.dir {
                self.suspect(rel, Some((is_dir, stamp)));
                continue;
            }
            if !entry.dir {
                if stamp != entry.stamp {
                    self.suspect(rel, Some((false, stamp)));
                }
                continue;
            }
            if stamp.modified == entry.stamp.modified && entry.settled {
                continue;
            }
            // **A folder that moved (or is too fresh to trust) is read, and only that folder.**
            if let Some(children) = read_children(self.root(), rel, &self.ignore) {
                let listed: HashSet<&str> = children.iter().map(|(c, _)| c.as_str()).collect();
                for (child, e) in &children {
                    if is_known.contains(child.as_str()) {
                        continue;
                    }
                    self.suspect(child, Some((e.dir, e.stamp)));
                    if e.dir {
                        // A folder that arrived with things in it: they are new too.
                        let mut inside = BTreeMap::new();
                        walk_into(self.root(), child, &self.ignore, &self.shared.stop, &mut inside);
                        for (r, en) in inside {
                            self.suspect(&r, Some((en.dir, en.stamp)));
                        }
                    }
                }
                for child in children_of.get(rel.as_str()).into_iter().flatten() {
                    if !listed.contains(child) {
                        self.suspect(child, None);
                    }
                }
            }
            folder_updates.push((rel.clone(), Entry { dir: true, stamp, settled: settled(stamp.modified, now) }));
        }
        let mut st = self.shared.lock();
        for (rel, entry) in folder_updates {
            if let Some(held) = st.entries.get_mut(&rel) {
                *held = entry;
            }
        }
    }

    /// **Re-look at what is held, and take out what has been steady long enough** (all of it when `force`).
    fn ripen(&mut self, force: bool) -> Vec<(String, Option<(bool, FileStamp)>)> {
        let now = Instant::now();
        let root = self.shared.root.clone();
        let mut ripe = Vec::new();
        self.pending.retain(|rel, held| {
            let seen = observe(&root, rel);
            if seen != held.seen {
                held.seen = seen;
                held.since = now;
            }
            if force || held.immediate || now.duration_since(held.since) >= DEBOUNCE {
                ripe.push((rel.clone(), held.seen));
                false
            } else {
                true
            }
        });
        ripe.sort_by(|a, b| a.0.cmp(&b.0));
        ripe
    }

    /// **Fold what settled into the index**, queue one event for each entry that really changed, and rebuild the snapshot once.
    fn apply(&mut self, ripe: Vec<(String, Option<(bool, FileStamp)>)>) {
        let mut events = Vec::new();
        let mut structural = false;
        let mut gitignore = false;
        {
            let mut st = self.shared.lock();
            for (rel, seen) in ripe {
                if rel.is_empty() {
                    continue;
                }
                gitignore |= rel == ".gitignore";
                fold(&mut st.entries, self.shared.root.as_path(), &rel, seen, &mut events, &mut structural);
            }
            if !events.is_empty() {
                self.publish(&mut st, structural, &events);
            }
        }
        if gitignore {
            self.rewalk();
        }
    }

    /// **`.gitignore` changed, so what is listed changed**: the one event that walks again.
    fn rewalk(&mut self) {
        self.ignore = ignore_of(self.root());
        let fresh = walk(self.root(), &self.ignore, &self.shared.stop);
        self.shared.walks.fetch_add(1, Ordering::Relaxed);
        let mut diff: Vec<(String, Option<(bool, FileStamp)>)> = Vec::new();
        {
            let st = self.shared.lock();
            for (rel, e) in &fresh {
                if !rel.is_empty() && !st.entries.contains_key(rel) {
                    diff.push((rel.clone(), Some((e.dir, e.stamp))));
                }
            }
            for rel in st.entries.keys() {
                if !rel.is_empty() && !fresh.contains_key(rel) {
                    diff.push((rel.clone(), None));
                }
            }
        }
        diff.sort_by(|a, b| a.0.cmp(&b.0));
        let mut events = Vec::new();
        let mut structural = false;
        let mut st = self.shared.lock();
        for (rel, seen) in diff {
            fold(&mut st.entries, self.shared.root.as_path(), &rel, seen, &mut events, &mut structural);
        }
        if !events.is_empty() {
            self.publish(&mut st, structural, &events);
        }
    }
}

/// **One settled observation against what the index holds.** A path that appears gets its missing folders first; a folder that
/// goes takes everything under it; a file whose stamp moved is `Modified`; anything that ends where it began is nothing.
fn fold(
    entries: &mut BTreeMap<String, Entry>,
    root: &Path,
    rel: &str,
    seen: Option<(bool, FileStamp)>,
    events: &mut Vec<ProjectFileChanged>,
    structural: &mut bool,
) {
    let held = entries.get(rel).copied();
    let remove = |entries: &mut BTreeMap<String, Entry>, events: &mut Vec<ProjectFileChanged>, held: Entry| {
        if held.dir {
            let prefix = format!("{rel}/");
            let below: Vec<(String, Entry)> =
                entries.range(prefix.clone()..).take_while(|(k, _)| k.starts_with(&prefix)).map(|(k, e)| (k.clone(), *e)).collect();
            for (k, e) in below {
                entries.remove(&k);
                events.push(ProjectFileChanged { rel: k, kind: ChangeKind::Removed, stamp: None, dir: e.dir });
            }
        }
        entries.remove(rel);
        events.push(ProjectFileChanged { rel: rel.to_owned(), kind: ChangeKind::Removed, stamp: None, dir: held.dir });
    };
    let create = |entries: &mut BTreeMap<String, Entry>, events: &mut Vec<ProjectFileChanged>, dir: bool, stamp: FileStamp| {
        // **The folders above it come first**, so a subscriber never hears of a file in a folder it has not heard of.
        let mut above = Vec::new();
        let mut at = parent_of(rel);
        while !at.is_empty() && !entries.contains_key(at) {
            above.push(at.to_owned());
            at = parent_of(at);
        }
        for folder in above.into_iter().rev() {
            if let Some((true, folder_stamp)) = observe(root, &folder) {
                entries.insert(folder.clone(), Entry { dir: true, stamp: folder_stamp, settled: false });
                events.push(ProjectFileChanged { rel: folder, kind: ChangeKind::Created, stamp: Some(folder_stamp), dir: true });
            }
        }
        entries.insert(rel.to_owned(), Entry { dir, stamp, settled: false });
        events.push(ProjectFileChanged { rel: rel.to_owned(), kind: ChangeKind::Created, stamp: Some(stamp), dir });
    };
    match (held, seen) {
        (None, Some((dir, stamp))) => {
            create(entries, events, dir, stamp);
            *structural = true;
        }
        (Some(held), None) => {
            remove(entries, events, held);
            *structural = true;
        }
        (Some(held), Some((dir, stamp))) if held.dir != dir => {
            remove(entries, events, held);
            create(entries, events, dir, stamp);
            *structural = true;
        }
        (Some(held), Some((false, stamp))) if held.stamp != stamp => {
            entries.insert(rel.to_owned(), Entry { dir: false, stamp, settled: false });
            events.push(ProjectFileChanged { rel: rel.to_owned(), kind: ChangeKind::Modified, stamp: Some(stamp), dir: false });
        }
        _ => {}
    }
}

/// **The thread's life**: the first listing, then wait, sweep, settle, apply.
fn run(shared: Arc<Shared>) {
    let mut sweeper = Sweeper { shared: shared.clone(), ignore: Ignore::default(), pending: HashMap::new() };
    sweeper.first_listing();
    let mut last_sweep = Instant::now();
    loop {
        let (noted, syncs) = {
            let mut st = shared.lock();
            if shared.stop.load(Ordering::Relaxed) {
                return;
            }
            if st.noted.is_empty() && st.syncs.is_empty() && last_sweep.elapsed() < SWEEP {
                let wait = if sweeper.pending.is_empty() { SWEEP.saturating_sub(last_sweep.elapsed()) } else { TICK };
                st = shared.wake.wait_timeout(st, wait).unwrap_or_else(std::sync::PoisonError::into_inner).0;
                if shared.stop.load(Ordering::Relaxed) {
                    return;
                }
            }
            (std::mem::take(&mut st.noted), std::mem::take(&mut st.syncs))
        };
        let sync = !syncs.is_empty();
        if sync || last_sweep.elapsed() >= SWEEP {
            sweeper.sweep();
            last_sweep = Instant::now();
        }
        for rel in noted {
            sweeper.note(&rel);
        }
        let ripe = sweeper.ripen(sync);
        sweeper.apply(ripe);
        for waiting in syncs {
            let _ = waiting.send(());
        }
    }
}

// ── The resource ─────────────────────────────────────────────────────────────

/// Stops the thread and takes the index out of the registry when the resource lets go of it.
struct Handle {
    shared: Arc<Shared>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        self.shared.wake.notify_all();
        let mut registry = registry();
        if registry.get(&self.shared.root).is_some_and(|w| w.ptr_eq(&Arc::downgrade(&self.shared))) {
            registry.remove(&self.shared.root);
        }
    }
}

/// **What the open project holds, following the changes** — the one listing the editor's panels read.
///
/// Started on the first frame a [`Project`] is present and restarted when its root changes; there is no second walk after the
/// first listing. See the module note for how to read it.
#[derive(Resource, Default)]
pub struct ProjectIndex {
    handle: Option<Handle>,
    published: u64,
}

impl ProjectIndex {
    /// The root being followed.
    pub fn root(&self) -> Option<&Path> {
        self.handle.as_ref().map(|h| h.shared.root.as_path())
    }

    /// **Whether anything moved since a reader last looked**: `0` until the first listing is in, then a number larger after every
    /// applied change and never repeated across projects. An atomic load.
    pub fn generation(&self) -> u64 {
        self.handle.as_ref().map_or(0, |h| h.shared.generation.load(Ordering::Acquire))
    }

    /// The listing as of the latest generation, or `None` before the first listing is in.
    pub fn snapshot(&self) -> Option<Arc<Snapshot>> {
        self.handle.as_ref().and_then(|h| h.shared.lock().snapshot.clone())
    }

    /// **How many full walks the project has had**: one, and one more for each `.gitignore` change.
    pub fn walks(&self) -> u64 {
        self.handle.as_ref().map_or(0, |h| h.shared.walks.load(Ordering::Relaxed))
    }

    /// How many [`ProjectFileChanged`] messages have been published, one per changed entry.
    pub fn published(&self) -> u64 {
        self.published
    }

    /// Wait for the first listing (a test's, and a stepped editor's: its first frame reads a listing, as it always did).
    pub fn wait_ready(&self, timeout: Duration) -> bool {
        let start = Instant::now();
        while self.generation() == 0 {
            if start.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        true
    }

    fn start(&mut self, root: &Path) {
        let shared = Arc::new(Shared {
            root: root.to_path_buf(),
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            stop: AtomicBool::new(false),
            news: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            walks: AtomicU64::new(0),
        });
        registry().insert(root.to_path_buf(), Arc::downgrade(&shared));
        let thread_shared = shared.clone();
        match std::thread::Builder::new().name("project-index".into()).spawn(move || run(thread_shared)) {
            Ok(_) => self.handle = Some(Handle { shared }),
            Err(e) => {
                registry().remove(root);
                error!("the project index could not start its thread: {e}");
                self.handle = None;
            }
        }
    }

    fn has_news(&self) -> bool {
        self.handle.as_ref().is_some_and(|h| h.shared.news.load(Ordering::Acquire))
    }

    fn deliver(&mut self, out: &mut MessageWriter<ProjectFileChanged>) {
        let Some(handle) = &self.handle else { return };
        let events = {
            let mut st = handle.shared.lock();
            handle.shared.news.store(false, Ordering::Release);
            std::mem::take(&mut st.outbox)
        };
        self.published += events.len() as u64;
        out.write_batch(events);
    }
}

/// **Marks an editor that waits for the first listing when it starts following a project**, so its first frame reads a finished
/// listing as it did when the listing was a walk on the frame. Inserted by the stepped harness; the binary does not wait.
#[derive(Resource, Default)]
pub struct WaitsForFirstListing;

/// **Follow the open project, and publish what the thread found.** In `PreUpdate`, so every system of the frame sees the
/// messages.
fn follow_the_project(
    project: Option<Res<Project>>,
    waits: Option<Res<WaitsForFirstListing>>,
    mut index: ResMut<ProjectIndex>,
    mut changed: MessageWriter<ProjectFileChanged>,
) {
    let want = project.as_deref().map(|p| p.root.as_path());
    if index.root() != want {
        match want {
            Some(root) => {
                index.start(root);
                if waits.is_some() {
                    index.wait_ready(Duration::from_secs(60));
                }
            }
            None => index.handle = None,
        }
    }
    if index.has_news() {
        index.deliver(&mut changed);
    }
}

/// The plugin: the message, the resource, and the system that follows the project.
pub struct ProjectIndexPlugin;

impl Plugin for ProjectIndexPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ProjectIndex>().add_message::<ProjectFileChanged>().add_systems(PreUpdate, follow_the_project);
    }
}
