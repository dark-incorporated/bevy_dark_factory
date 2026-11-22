//! **One watcher walks the project, and the index that follows it is what the panels read** (#501, stage 1).
//!
//! # The gap this closes
//!
//! #501 lists twenty-two poll constants in seventeen modules, each re-statting the project on its own clock, so the same file was
//! read by several panels at several rhythms, and two lists that should agree ([`crate::files::listed_files`] and what the finder's
//! scan walked) were two walks. This module is the answer to "what files exist, and which moved": **one background thread is the
//! only code that walks the project's tree after its first listing**, and the panels it has been migrated to subscribe.
//!
//! **That is not "the only code that looks at the project".** Stage 1 moves two consumers (the Files tree and `listed_files`, and the
//! finder's scan in `crosslinks`); twenty poll constants in the other modules still `stat` their own files on their own clocks, and
//! `docs/guide/reload-contract.md` has the checklist that retires them.
//!
//! # What is published
//!
//! [`ProjectFileChanged`], a Bevy [`Message`], **once per changed entry** (not once per subscriber: each subscriber has its own
//! `MessageReader` cursor over the one message). It carries `(rel, kind, stamp)` — [`watch::Due`](crate::watch::Due)'s shape with the
//! kind said — and `dir`, because a folder appearing is a change a tree wants too.
//!
//! **Debounced by [`DEBOUNCE`]** (#501 cites Renzora's `renzora_project_watch` for the number and the reason). One Ctrl+S arrives as a
//! create, several writes and a close (or, for an editor that saves atomically, a remove and a rename). A change is held until its
//! stamp has been steady for [`DEBOUNCE`] (or for [`MAX_HOLD`], so a file written more often than that still publishes), then diffed
//! against what the index holds: a save is one event, and a file that ends where it began is none. **An operation that has just
//! finished writing** (`file_create`, `file_rename`, `file_delete`) says so with [`note_written`] and is published at once: it knows
//! the write is done, and the row it made must be on screen in the next frame.
//!
//! # How the thread finds changes, with no new dependency
//!
//! The crate has no `notify` (it is behind the off-by-default `asset_spike` feature), so the thread **sweeps**, every [`SWEEP`] by
//! default: it stats each folder it knows (a folder's mtime moves when an entry is added, removed or renamed) and re-reads only the
//! folders that moved, and it stats each file it knows. The first listing is one walk, whose `symlink_metadata` calls already give
//! every stamp; after it the index is the fold of the events. Two things walk again, and both are named: a `.gitignore` change (it
//! changes what is listed), and a folder written in the last [`RACY`] is re-read on the next sweep as well (git's racy-clean rule), so
//! two changes inside one clock tick are not lost.
//!
//! # No second path, and no silent failure
//!
//! The index is the only listing there is. [`crate::files::listed_files`] asks the index that follows the root and **returns an error**
//! when none does, when it cannot settle, or when the project root cannot be read: there is no fallback walk on the caller's thread.
//! An unreadable folder is listed empty, **warned about once**, and read again on every sweep until it can be read (its stamp is not
//! advanced meanwhile, so what is inside is not lost). A name that is not UTF-8 is skipped with a warning. **A root that goes after it was
//! listed refuses too**, even though an older listing is held, and the tree says so; the refusal ends when the root can be looked at again.
//!
//! **`listed_files` from an operation blocks its caller** while the thread settles (one `stat` per folder, and what was held flushed):
//! it can wait for the first listing (0.26 s at 10,000 files and 1.6 s at 100,000, measured below with a stepped editor's own build) or
//! for a full sweep already running (266 ms at 100,000), and for at most [`SETTLE_WAIT`] (10 s) before it is refused.
//!
//! # How a subscriber reads it
//!
//! `Res<ProjectIndex>`: [`ProjectIndex::generation`] says whether anything moved (an atomic load), [`ProjectIndex::snapshot`] is an
//! `Arc` clone of the listing, and a `MessageReader<ProjectFileChanged>` says which path moved. A plugin that reads the index adds
//! [`ProjectIndexPlugin`] itself with [`ensure`].
//!
//! # What it costs
//!
//! Measured on `big` (a throwaway test, the `test` profile, one project of 10,000 and one of 100,000 files in 100 folders): building a
//! stepped editor, which waits for the first listing, takes 0.26 s and 1.6 s; **a sweep that looks at every file takes 36 ms at 10,000
//! files and 266 ms at 100,000**, so the thread leaves a sweep alone for [`SWEEP_DUTY`] times its own length (at 100,000 files a sweep
//! every 2.7 s rather than every second); publishing a change rebuilds the listing in 7-17 ms and 34-45 ms, on the thread; a `Snapshot`
//! clone is under a microsecond. On the UI thread the finder's scan with a previous scan to reuse costs 4 ms at 10,000 files and 43 ms at
//! 100,000, and it runs only when a text file came, went or changed ([`Snapshot::text_generation`]); that 43 ms is **not** reduced here.
//! **The worst case for a change going unseen is about [`MAX_SWEEP_EVERY`] plus [`MAX_HOLD`]**, when a sweep took long enough to reach
//! the ceiling; [`ProjectIndex::failure`] and the tree's notice can lag by up to one sweep time, while `listed` is not affected because it
//! settles first.
//!
//! # What this does not touch
//!
//! `watch.rs` keeps deciding what the editor adopts: its `adopt_map` refusal is a correctness rule about unsaved work, not a poll, and
//! this module only says that a path moved.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex, MutexGuard, Weak, mpsc};
use std::time::{Duration, Instant, SystemTime};

use bevy::prelude::*;

use crate::files::{Ignore, SKIPPED_DIRS, TreeBuild};
use crate::project::Project;

/// **How often the thread looks at every file it knows** unless [`ProjectIndex::set_sweep`] says otherwise: the one poll clock that
/// stays when the per-module ones retire into it.
pub const SWEEP: Duration = Duration::from_secs(1);

/// **How long a change must be steady before it is published**, so one save (a create, several writes and a close) is one event.
pub const DEBOUNCE: Duration = Duration::from_millis(100);

/// **The longest a change is held**, however often it moves: a log appended more often than [`DEBOUNCE`] still publishes.
pub const MAX_HOLD: Duration = Duration::from_secs(2);

/// How often a held change is looked at again while it is settling.
const TICK: Duration = Duration::from_millis(25);

/// **The shortest time [`ProjectIndex::set_sweep`] accepts**: a sweep time of nothing would make the thread sweep without rest.
pub const MIN_SWEEP: Duration = TICK;

/// **The longest the back-off leaves a sweep alone** (unless a caller set a longer time): a sweep that took 266 ms at 100,000 files
/// waits 2.7 s, and one slow sweep (a stall of seconds) cannot park the thread for ten times that.
pub const MAX_SWEEP_EVERY: Duration = Duration::from_secs(30);

/// **How long the thread waits between sweeps that look at every file**: the time set, or [`SWEEP_DUTY`] times the last sweep's length
/// when that is longer, at most [`MAX_SWEEP_EVERY`] (the time set wins when it is longer than that).
pub fn sweep_interval(set: Duration, last_took: Duration) -> Duration {
    set.max((last_took * SWEEP_DUTY).min(MAX_SWEEP_EVERY))
}

/// **A folder written this recently is read again on the next sweep**: a second change in the same clock tick would leave its mtime
/// where the first read saw it.
const RACY: Duration = Duration::from_secs(2);

/// How long a caller with no world waits for the thread to settle before it is refused.
const SETTLE_WAIT: Duration = Duration::from_secs(10);

/// How long a stepped editor waits for the first listing when it starts following a project.
const FIRST_LISTING_WAIT: Duration = Duration::from_secs(60);

/// How often [`ProjectIndex::wait_ready`] looks.
const READY_POLL: Duration = Duration::from_millis(1);

/// **How many times its own length a sweep is left alone for**: a sweep that took 266 ms (measured at 100,000 files on big) is followed by
/// at least 2.66 s of other work, so up to a sweep of 3 s the thread spends at most a tenth of itself sweeping. Above that
/// [`MAX_SWEEP_EVERY`] caps the gap, and a sweep that long takes more than a tenth (a 5 s sweep is 5 s of every 30 s).
const SWEEP_DUTY: u32 = 10;

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

/// What the index holds of one entry. A folder's `settled` says its mtime is old enough to be trusted (see [`RACY`]), and a folder
/// that could not be read is never settled, so it is read again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    dir: bool,
    stamp: FileStamp,
    settled: bool,
}

/// What one look at one path found: a folder or a file with its stamp, or `None` for a path that is gone (or a link, which is never
/// followed and never listed).
type Seen = Option<(bool, FileStamp)>;

/// **The listing as of one generation**, immutable and shared: readers clone the `Arc`, never the listing.
#[derive(Debug)]
pub struct Snapshot {
    /// Unique across every index in the process, and larger after every applied change.
    pub generation: u64,
    /// **The generation of the last listing in which a text file ([`crate::crosslinks::is_text_file`]) was created, removed or changed.**
    /// A reader of the text files (the finder's scan) has nothing to do when only this stays the same.
    pub text_generation: u64,
    /// Every file (not folder), sorted by path, with its stamp: no cap.
    pub files: Vec<(String, FileStamp)>,
    /// The tree the Files workspace draws, with its caps; rebuilt only when the structure moved.
    pub tree: Arc<TreeBuild>,
}

// ── The seam every lookup goes through ───────────────────────────────────────

thread_local! {
    static LOOKUPS: Cell<u64> = const { Cell::new(0) };
}

/// **How many `stat`, `read_dir` and `.gitignore` reads this module has made on this thread.** A test reads it across a frame to say
/// the index's own seam made none on the UI thread. It counts what goes through this module and nothing else: another module's
/// `std::fs` call is invisible to it.
pub fn lookups_on_this_thread() -> u64 {
    LOOKUPS.with(Cell::get)
}

fn counted() {
    LOOKUPS.with(|c| c.set(c.get() + 1));
}

fn look_stat(path: &Path) -> io::Result<std::fs::Metadata> {
    counted();
    std::fs::symlink_metadata(path)
}

fn look_dir(path: &Path) -> io::Result<std::fs::ReadDir> {
    counted();
    std::fs::read_dir(path)
}

// ── Reading the disk ─────────────────────────────────────────────────────────

fn is_gone(e: &io::Error) -> bool {
    matches!(e.kind(), io::ErrorKind::NotFound | io::ErrorKind::NotADirectory)
}

/// Say something once per `key`, so a folder that stays unreadable is one line and not one a second.
fn warn_once(warned: &mut HashSet<String>, key: String, message: impl FnOnce() -> String) {
    if warned.insert(key) {
        warn!("project index: {}", message());
    }
}

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

/// **The root's `.gitignore`**: none is no rule, and a file that cannot be read is an error (a wrong listing is worse than none).
fn ignore_of(root: &Path) -> Result<Ignore, String> {
    counted();
    match std::fs::read_to_string(root.join(".gitignore")) {
        Ok(text) => Ok(Ignore::parse(&text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Ignore::default()),
        Err(e) => Err(format!("{} cannot be read: {e}", root.join(".gitignore").display())),
    }
}

/// **One look at one path.** `Ok(None)` for a path that is not there or is a link; an error for anything else (a permission, say),
/// which the caller says and does not take for a removal.
fn observe(root: &Path, rel: &str) -> io::Result<Seen> {
    let path = full_path(root, rel);
    let meta = if rel.is_empty() {
        counted();
        std::fs::metadata(&path)
    } else {
        look_stat(&path)
    };
    match meta {
        Ok(meta) if meta.file_type().is_symlink() => Ok(None),
        Ok(meta) => Ok(Some((meta.is_dir(), stamp_of(&meta)))),
        Err(e) if is_gone(&e) => Ok(None),
        Err(e) => Err(e),
    }
}

/// **The entries directly inside one folder**, with their stamps, filtered the way the tree filters. An error when the folder (or an
/// entry in it that is still there) cannot be read: a half-read folder is not returned as a whole one.
fn read_children(root: &Path, dir: &str, ignore: &Ignore, warned: &mut HashSet<String>) -> io::Result<Vec<(String, Entry)>> {
    let read = look_dir(&full_path(root, dir))?;
    let read_at = SystemTime::now();
    let mut out = Vec::new();
    for entry in read {
        let entry = entry?;
        let meta = match look_stat(&entry.path()) {
            Ok(meta) => meta,
            // Gone between the listing and the look: it is not there.
            Err(e) if is_gone(&e) => continue,
            Err(e) => return Err(e),
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            warn_once(warned, format!("name:{dir}/{file_name:?}"), || format!("`{}` has a name that is not UTF-8, so it is not listed", entry.path().display()));
            continue;
        };
        let is_dir = meta.is_dir();
        let rel = if dir.is_empty() { name.to_owned() } else { format!("{dir}/{name}") };
        if !admitted(&rel, name, is_dir, ignore) {
            continue;
        }
        let stamp = stamp_of(&meta);
        out.push((rel, Entry { dir: is_dir, stamp, settled: is_dir && settled(stamp.modified, read_at) }));
    }
    Ok(out)
}

/// **Walk below `dir`** into `out`. An error only for `dir` itself; a folder below it that cannot be read is warned about once and
/// left in `out` unsettled and empty, so every sweep reads it again.
fn walk_into(
    root: &Path,
    dir: &str,
    ignore: &Ignore,
    stop: &AtomicBool,
    warned: &mut HashSet<String>,
    out: &mut BTreeMap<String, Entry>,
) -> io::Result<()> {
    if stop.load(Ordering::Relaxed) {
        return Ok(());
    }
    for (rel, entry) in read_children(root, dir, ignore, warned)? {
        let is_dir = entry.dir;
        out.insert(rel.clone(), entry);
        if is_dir && let Err(e) = walk_into(root, &rel, ignore, stop, warned, out) {
            warn_once(warned, format!("dir:{rel}"), || format!("`{rel}` cannot be read ({e}): it is listed empty and read again on every sweep"));
            if let Some(held) = out.get_mut(&rel) {
                held.settled = false;
            }
        }
    }
    Ok(())
}

/// **The one walk**: every entry under the root, uncapped, with the root itself kept under `""` so its own mtime is followed. An
/// error when the root is not a readable folder.
fn walk(root: &Path, ignore: &Ignore, stop: &AtomicBool, warned: &mut HashSet<String>) -> io::Result<BTreeMap<String, Entry>> {
    let mut out = BTreeMap::new();
    match observe(root, "")? {
        Some((true, stamp)) => {
            out.insert(String::new(), Entry { dir: true, stamp, settled: settled(stamp.modified, SystemTime::now()) });
        }
        Some((false, _)) => return Err(io::Error::new(io::ErrorKind::InvalidInput, "it is not a folder")),
        None => return Err(io::Error::new(io::ErrorKind::NotFound, "it is not there")),
    }
    walk_into(root, "", ignore, stop, warned, &mut out)?;
    Ok(out)
}

fn parent_of(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(parent, _)| parent)
}

// ── The shared state ─────────────────────────────────────────────────────────

/// Handed out to every applied change, from every index in the process, so a generation never repeats across projects.
static GENERATION: AtomicU64 = AtomicU64::new(0);
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// What the thread hands the rest of the editor, replaced under the lock: the snapshot and the failure by assignment, the outbox by
/// appending a batch and by being taken whole. A panic elsewhere cannot tear it; the index's own entries live on the thread and are
/// never behind this lock.
#[derive(Default)]
struct State {
    snapshot: Option<Arc<Snapshot>>,
    outbox: Vec<ProjectFileChanged>,
    /// Paths an operation has finished writing.
    noted: Vec<String>,
    /// Callers waiting for the thread to settle.
    syncs: Vec<mpsc::Sender<()>>,
    /// Why there is no listing, when there is none (the root cannot be read).
    failure: Option<String>,
}

struct Shared {
    id: u64,
    root: PathBuf,
    state: Mutex<State>,
    wake: Condvar,
    stop: AtomicBool,
    /// Set (under the lock) when the outbox has something, so a quiet frame costs one atomic load.
    news: AtomicBool,
    generation: AtomicU64,
    walks: AtomicU64,
    sweep_millis: AtomicU64,
    last_sweep_micros: AtomicU64,
    /// Whether `State::failure` is set, so a quiet frame asking for it costs an atomic load.
    failed: AtomicBool,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn sweep(&self) -> Duration {
        Duration::from_millis(self.sweep_millis.load(Ordering::Relaxed))
    }
}

/// One running index: its id, the root it follows, and a weak handle to it.
type Running = (u64, PathBuf, Weak<Shared>);

/// Every running index by `(id, root)`, so a later app on the same root does not hide an earlier one in the registry. [`registered`] (so
/// [`note_written`] and [`listed`]) answers with the **newest** that still runs: with two apps on one root the older one is not
/// reached by them and falls back on its own sweep, which is acceptable for tests and is not a case the editor has.
static REGISTRY: LazyLock<Mutex<Vec<Running>>> = LazyLock::new(Default::default);

fn registry() -> MutexGuard<'static, Vec<Running>> {
    REGISTRY.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn registered(root: &Path) -> Option<Arc<Shared>> {
    registry().iter().rev().filter(|(_, r, _)| r == root).find_map(|(_, _, weak)| weak.upgrade().filter(|s| !s.stop.load(Ordering::Relaxed)))
}

/// **An operation has finished writing `rel`** (made, renamed away, deleted): the index looks at that path at once and publishes what
/// it finds, with no debounce, because the writer knows the write is done. A hint, not an answer: with no index following `root` there
/// is no one to tell, and the call does nothing.
pub fn note_written(root: &Path, rel: &str) {
    let Some(shared) = registered(root) else { return };
    shared.lock().noted.push(rel.to_owned());
    shared.wake.notify_all();
}

/// **The listing as of now, for a caller with no world**: the index that follows `root` settles (a sweep of its folders and what
/// was held, on its own thread) and answers. An error, said, when no index follows `root`, when it does not settle in time, or when it
/// has no listing because the root cannot be read.
fn settled_snapshot(root: &Path) -> Result<Arc<Snapshot>, String> {
    let shared = registered(root).ok_or_else(|| {
        format!(
            "no project index follows {}; the Files workspace and the finder start one for the open project, so there is no listing to read",
            root.display()
        )
    })?;
    let (tx, rx) = mpsc::channel();
    shared.lock().syncs.push(tx);
    shared.wake.notify_all();
    if let Err(e) = rx.recv_timeout(SETTLE_WAIT) {
        let why = format!("the project index of {} did not settle within {SETTLE_WAIT:?} ({e})", root.display());
        warn!("{why}");
        return Err(why);
    }
    let state = shared.lock();
    // **A failure refuses even when an older listing is held**: a root that has gone is not the project the old list was of.
    if let Some(why) = &state.failure {
        return Err(why.clone());
    }
    state.snapshot.clone().ok_or_else(|| format!("the project index of {} has no listing yet", root.display()))
}

/// **Every file of the project, sorted, with no cap**: the index's answer, or the reason there is none. [`crate::files::listed_files`]
/// is this.
pub fn listed(root: &Path) -> Result<Vec<String>, String> {
    Ok(settled_snapshot(root)?.files.iter().map(|(rel, _)| rel.clone()).collect())
}

// ── The thread ───────────────────────────────────────────────────────────────

/// A change seen and not yet published: what it looked like last, since when it has looked like that, and since when it has been held.
struct Pending {
    seen: Seen,
    since: Instant,
    held_since: Instant,
    immediate: bool,
}

struct Sweeper {
    shared: Arc<Shared>,
    ignore: Ignore,
    /// **Everything the index holds**, on this thread and nowhere else: a reader sees only what [`Sweeper::publish`] hands over.
    entries: BTreeMap<String, Entry>,
    pending: HashMap<String, Pending>,
    warned: HashSet<String>,
    tree: Option<Arc<TreeBuild>>,
    /// The generation of the last listing in which a text file came, went or changed.
    text_generation: u64,
}

impl Sweeper {
    fn new(shared: Arc<Shared>) -> Sweeper {
        Sweeper { shared, ignore: Ignore::default(), entries: BTreeMap::new(), pending: HashMap::new(), warned: HashSet::new(), tree: None, text_generation: 0 }
    }

    fn root(&self) -> &Path {
        &self.shared.root
    }

    fn stopped(&self) -> bool {
        self.shared.stop.load(Ordering::Relaxed)
    }

    /// There is no listing, and this says why: warned once, and kept for the callers who ask (a listing that was there is refused too).
    fn fail(&mut self, why: String) {
        warn_once(&mut self.warned, format!("failure:{why}"), || why.clone());
        self.shared.lock().failure = Some(why);
        self.shared.failed.store(true, Ordering::Release);
    }

    /// The root can be looked at again: the failure is over, and may be said again if it comes back.
    fn clear_failure(&mut self) {
        // The reverse of `fail`, so a reader that sees `failed` unset never finds the failure still held.
        if self.shared.failed.load(Ordering::Acquire) {
            self.shared.lock().failure = None;
            self.shared.failed.store(false, Ordering::Release);
            self.warned.retain(|key| !key.starts_with("failure:"));
        }
    }

    /// **The first listing**: the one walk. It publishes no events (a first sighting is not a change). `false` when the root cannot be
    /// read, which is a failure said and tried again on the next sweep.
    fn first_listing(&mut self) -> bool {
        let ignore = match ignore_of(self.root()) {
            Ok(ignore) => ignore,
            Err(why) => {
                self.fail(why);
                return false;
            }
        };
        let entries = match walk(&self.shared.root, &ignore, &self.shared.stop, &mut self.warned) {
            Ok(entries) => entries,
            Err(e) => {
                self.fail(format!("the project root {} cannot be listed: {e}", self.root().display()));
                return false;
            }
        };
        if self.stopped() {
            return false;
        }
        self.shared.walks.fetch_add(1, Ordering::Relaxed);
        self.ignore = ignore;
        self.entries = entries;
        self.publish(true, &[]);
        self.clear_failure();
        true
    }

    /// **Hand the listing to the rest of the editor**: the snapshot is built here, off the lock, and swapped in with the events.
    fn publish(&mut self, structural: bool, events: &[ProjectFileChanged]) {
        let generation = GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        let files = self.entries.iter().filter(|(rel, e)| !e.dir && !rel.is_empty()).map(|(rel, e)| (rel.clone(), e.stamp)).collect();
        let tree = match &self.tree {
            Some(tree) if !structural => tree.clone(),
            _ => {
                let built = Arc::new(crate::files::build_tree(self.entries.iter().filter(|(rel, _)| !rel.is_empty()).map(|(rel, e)| (rel.as_str(), e.dir))));
                self.tree = Some(built.clone());
                built
            }
        };
        let text_generation = if self.text_generation == 0 || events.iter().any(|e| !e.dir && crate::crosslinks::is_text_file(&e.rel)) {
            generation
        } else {
            self.text_generation
        };
        self.text_generation = text_generation;
        let snapshot = Arc::new(Snapshot { generation, text_generation, files, tree });
        let mut st = self.shared.lock();
        st.snapshot = Some(snapshot);
        // **The failure is not cleared here.** A publish can follow a root that has gone (a held change ripens against it and folds to a
        // removal); only a look that found the root, or a listing that read it, ends the failure (`clear_failure`).
        if !events.is_empty() {
            st.outbox.extend_from_slice(events);
            self.shared.news.store(true, Ordering::Release);
        }
        self.shared.generation.store(generation, Ordering::Release);
    }

    /// An operation's finished write: looked at now and published at once.
    fn note(&mut self, rel: &str) {
        if rel.is_empty() || !self.listed(rel) {
            return;
        }
        match observe(self.root(), rel) {
            Ok(seen) => {
                let now = Instant::now();
                self.pending.insert(rel.to_owned(), Pending { seen, since: now, held_since: now, immediate: true });
            }
            Err(e) => warn_once(&mut self.warned, format!("stat:{rel}"), || format!("`{rel}` cannot be looked at ({e})")),
        }
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
            if !admitted(&at, part, n + 1 < parts.len(), &self.ignore) {
                return false;
            }
        }
        true
    }

    /// **Look at what the index knows.** Folders first (their mtime says an entry came or went, and only a folder that moved is read),
    /// then, when `with_files`, each file's own stamp. Whatever differs is held as a pending change. A caller that only wants the
    /// listing fresh passes `false` and costs one `stat` per folder, not per file.
    fn sweep(&mut self, with_files: bool) {
        match observe(&self.shared.root, "") {
            Ok(Some((true, _))) => self.clear_failure(),
            Ok(_) => {
                let why = format!("the project root {} is not there", self.root().display());
                self.fail(why);
                return;
            }
            Err(e) => {
                let why = format!("the project root {} cannot be looked at: {e}", self.root().display());
                self.fail(why);
                return;
            }
        }
        let Sweeper { shared, ignore, entries, pending, warned, .. } = self;
        let root = shared.root.as_path();
        let mut children_of: HashMap<&str, Vec<&str>> = HashMap::new();
        for rel in entries.keys().filter(|rel| !rel.is_empty()) {
            children_of.entry(parent_of(rel)).or_default().push(rel.as_str());
        }
        let mut folder_updates: Vec<(String, Entry)> = Vec::new();
        let now = SystemTime::now();
        for (rel, entry) in entries.iter() {
            if shared.stop.load(Ordering::Relaxed) {
                return;
            }
            if !with_files && !entry.dir {
                continue;
            }
            let (is_dir, stamp) = match observe(root, rel) {
                Ok(Some(seen)) => seen,
                Ok(None) => {
                    suspect(pending, rel, None);
                    continue;
                }
                Err(e) => {
                    warn_once(warned, format!("stat:{rel}"), || format!("`{rel}` cannot be looked at ({e}); it is left as it was"));
                    continue;
                }
            };
            if is_dir != entry.dir {
                suspect(pending, rel, Some((is_dir, stamp)));
                continue;
            }
            if !entry.dir {
                if stamp != entry.stamp {
                    suspect(pending, rel, Some((false, stamp)));
                }
                continue;
            }
            if stamp.modified == entry.stamp.modified && entry.settled {
                continue;
            }
            // **A folder that moved (or is too fresh to trust, or could not be read) is read, and only that folder.** Its stamp moves
            // on only when the read worked: a failed read leaves it as it was, so the next sweep reads it again.
            let children = match read_children(root, rel, ignore, warned) {
                Ok(children) => {
                    // A folder that can be read again is warned about again if it fails again.
                    if !warned.is_empty() {
                        warned.remove(&format!("dir:{rel}"));
                    }
                    children
                }
                Err(e) => {
                    warn_once(warned, format!("dir:{rel}"), || format!("`{rel}` cannot be read ({e}): it is listed empty and read again on every sweep"));
                    continue;
                }
            };
            let listed: HashSet<&str> = children.iter().map(|(c, _)| c.as_str()).collect();
            for (child, e) in &children {
                if entries.contains_key(child) {
                    continue;
                }
                suspect(pending, child, Some((e.dir, e.stamp)));
                if e.dir {
                    // A folder that arrived with things in it: they are new too.
                    let mut inside = BTreeMap::new();
                    if let Err(err) = walk_into(root, child, ignore, &shared.stop, warned, &mut inside) {
                        warn_once(warned, format!("dir:{child}"), || format!("`{child}` cannot be read ({err}): it is listed empty and read again on every sweep"));
                    }
                    for (r, en) in inside {
                        suspect(pending, &r, Some((en.dir, en.stamp)));
                    }
                }
            }
            for child in children_of.get(rel.as_str()).into_iter().flatten() {
                if !listed.contains(child) {
                    suspect(pending, child, None);
                }
            }
            folder_updates.push((rel.clone(), Entry { dir: true, stamp, settled: settled(stamp.modified, now) }));
        }
        for (rel, entry) in folder_updates {
            if let Some(held) = entries.get_mut(&rel) {
                *held = entry;
            }
        }
    }

    /// **Re-look at what is held, and take out what has been steady long enough** (all of it when `force`).
    fn ripen(&mut self, force: bool) -> Vec<(String, Seen)> {
        let now = Instant::now();
        let Sweeper { shared, pending, warned, .. } = self;
        let mut ripe = Vec::new();
        pending.retain(|rel, held| {
            let seen = match observe(&shared.root, rel) {
                Ok(seen) => seen,
                Err(e) => {
                    warn_once(warned, format!("stat:{rel}"), || format!("`{rel}` cannot be looked at ({e}); it is left as it was"));
                    return false;
                }
            };
            if seen != held.seen {
                held.seen = seen;
                held.since = now;
            }
            if force || held.immediate || now.duration_since(held.since) >= DEBOUNCE || now.duration_since(held.held_since) >= MAX_HOLD {
                ripe.push((rel.clone(), held.seen));
                false
            } else {
                true
            }
        });
        ripe.sort_by(|a, b| a.0.cmp(&b.0));
        ripe
    }

    /// **Fold what settled into the index**, queue one event for each entry that really changed, and publish once.
    fn apply(&mut self, ripe: Vec<(String, Seen)>) {
        if ripe.is_empty() {
            return;
        }
        // **The folders above a new path are looked at before anything is folded**, so folding is memory only.
        let mut above: HashMap<String, Seen> = HashMap::new();
        for (rel, seen) in &ripe {
            if seen.is_none() {
                continue;
            }
            let mut at = parent_of(rel);
            while !at.is_empty() && !self.entries.contains_key(at) && !above.contains_key(at) {
                let looked = observe(self.root(), at).unwrap_or_else(|e| {
                    warn_once(&mut self.warned, format!("stat:{at}"), || format!("`{at}` cannot be looked at ({e})"));
                    None
                });
                above.insert(at.to_owned(), looked);
                at = parent_of(at);
            }
        }
        let mut events = Vec::new();
        let mut structural = false;
        let mut gitignore = false;
        for (rel, seen) in ripe {
            if rel.is_empty() {
                continue;
            }
            gitignore |= rel == ".gitignore";
            fold(&mut self.entries, &above, &rel, seen, &mut events, &mut structural);
        }
        if !events.is_empty() {
            self.publish(structural, &events);
        }
        if gitignore {
            self.rewalk();
        }
    }

    /// **`.gitignore` changed, so what is listed changed**: the one event that walks again.
    fn rewalk(&mut self) {
        let ignore = match ignore_of(self.root()) {
            Ok(ignore) => ignore,
            Err(why) => {
                warn_once(&mut self.warned, format!("gitignore:{why}"), || format!("{why}; the listing keeps the rules it had"));
                return;
            }
        };
        let fresh = match walk(&self.shared.root, &ignore, &self.shared.stop, &mut self.warned) {
            Ok(fresh) => fresh,
            Err(e) => {
                warn_once(&mut self.warned, format!("rewalk:{e}"), || format!("the project cannot be listed again after `.gitignore` changed ({e}); the listing keeps the rules it had"));
                return;
            }
        };
        if self.stopped() {
            return;
        }
        self.shared.walks.fetch_add(1, Ordering::Relaxed);
        self.clear_failure();
        self.ignore = ignore;
        let mut diff: Vec<(String, Seen)> = Vec::new();
        for (rel, e) in &fresh {
            if !rel.is_empty() && !self.entries.contains_key(rel) {
                diff.push((rel.clone(), Some((e.dir, e.stamp))));
            }
        }
        for rel in self.entries.keys() {
            if !rel.is_empty() && !fresh.contains_key(rel) {
                diff.push((rel.clone(), None));
            }
        }
        diff.sort_by(|a, b| a.0.cmp(&b.0));
        let mut events = Vec::new();
        let mut structural = false;
        let none = HashMap::new();
        for (rel, seen) in diff {
            fold(&mut self.entries, &none, &rel, seen, &mut events, &mut structural);
        }
        if !events.is_empty() {
            self.publish(structural, &events);
        }
    }
}

/// Hold `rel` as a change that has been seen.
fn suspect(pending: &mut HashMap<String, Pending>, rel: &str, seen: Seen) {
    let now = Instant::now();
    pending.entry(rel.to_owned()).or_insert_with(|| Pending { seen, since: now, held_since: now, immediate: false });
}

/// **One settled observation against what the index holds**, in memory only. A path that appears gets its missing folders first (their
/// looks are in `above`); a folder that goes takes everything under it; a file whose stamp moved is `Modified`; anything that ends
/// where it began is nothing.
fn fold(
    entries: &mut BTreeMap<String, Entry>,
    above: &HashMap<String, Seen>,
    rel: &str,
    seen: Seen,
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
        let mut missing = Vec::new();
        let mut at = parent_of(rel);
        while !at.is_empty() && !entries.contains_key(at) {
            missing.push(at.to_owned());
            at = parent_of(at);
        }
        for folder in missing.into_iter().rev() {
            if let Some(Some((true, folder_stamp))) = above.get(&folder) {
                entries.insert(folder.clone(), Entry { dir: true, stamp: *folder_stamp, settled: false });
                events.push(ProjectFileChanged { rel: folder, kind: ChangeKind::Created, stamp: Some(*folder_stamp), dir: true });
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
    let mut sweeper = Sweeper::new(shared.clone());
    let mut ready = sweeper.first_listing();
    // The last sweep that looked at files too. A caller's settle looks at folders only, so it does not move this clock.
    let mut last_full = Instant::now();
    let mut last_full_took = Duration::ZERO;
    loop {
        // **A sweep is left alone for `SWEEP_DUTY` times its own length, at most `MAX_SWEEP_EVERY`** (see `sweep_interval`): above a sweep of
        // 3 s the ceiling wins, and the thread spends more than a tenth of itself sweeping (a 5 s sweep is 5 s of every 30 s).
        let sweep_every = sweep_interval(shared.sweep(), last_full_took);
        let (noted, syncs) = {
            let mut st = shared.lock();
            if shared.stop.load(Ordering::Relaxed) {
                return;
            }
            if st.noted.is_empty() && st.syncs.is_empty() && last_full.elapsed() < sweep_every {
                let wait = if sweeper.pending.is_empty() { sweep_every.saturating_sub(last_full.elapsed()) } else { TICK };
                st = shared.wake.wait_timeout(st, wait).unwrap_or_else(std::sync::PoisonError::into_inner).0;
                if shared.stop.load(Ordering::Relaxed) {
                    return;
                }
            }
            (std::mem::take(&mut st.noted), std::mem::take(&mut st.syncs))
        };
        let sync = !syncs.is_empty();
        let due = last_full.elapsed() >= sweep_every;
        if !ready {
            if sync || due {
                ready = sweeper.first_listing();
                last_full = Instant::now();
            }
        } else {
            if sync || due {
                let began = Instant::now();
                sweeper.sweep(due);
                if due {
                    // **Only a sweep that looked at files says how long a sweep takes**, and it is what the back-off reads.
                    last_full_took = began.elapsed();
                    shared.last_sweep_micros.store(u64::try_from(last_full_took.as_micros()).unwrap_or(u64::MAX), Ordering::Relaxed);
                    last_full = Instant::now();
                }
            }
            for rel in noted {
                sweeper.note(&rel);
            }
            let ripe = sweeper.ripen(sync);
            sweeper.apply(ripe);
        }
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
        // The flag is set and the thread woken under the lock, so a thread between its check and its wait cannot miss the wakeup.
        {
            let _held = self.shared.lock();
            self.shared.stop.store(true, Ordering::Relaxed);
            self.shared.wake.notify_all();
        }
        registry().retain(|(id, _, _)| *id != self.shared.id);
    }
}

/// **What the open project holds, following the changes** — the one listing the editor's panels read.
///
/// Started on the first frame a [`Project`] is present (or when the plugin is built, if one already is) and restarted when its root
/// changes. See the module note for how to read it.
#[derive(Resource, Default)]
pub struct ProjectIndex {
    handle: Option<Handle>,
    published: u64,
    /// A root whose thread could not be started: said once, and not tried again every frame.
    failed_root: Option<PathBuf>,
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

    /// Why there is no listing, when the project root cannot be read.
    pub fn failure(&self) -> Option<String> {
        let handle = self.handle.as_ref().filter(|h| h.shared.failed.load(Ordering::Acquire))?;
        handle.shared.lock().failure.clone()
    }

    /// **How many full walks the project has had**: one, and one more for each `.gitignore` change.
    pub fn walks(&self) -> u64 {
        self.handle.as_ref().map_or(0, |h| h.shared.walks.load(Ordering::Relaxed))
    }

    /// How many [`ProjectFileChanged`] messages have been published, one per changed entry.
    pub fn published(&self) -> u64 {
        self.published
    }

    /// **How often the thread looks at every file it knows** (default [`SWEEP`]), **never less than [`MIN_SWEEP`]**: a shorter time is
    /// raised to it, so no caller can make the thread spin. A test that wants only an operation's own note to reveal a change sets this
    /// long; one that wants to see a debounce sets it short.
    pub fn set_sweep(&self, every: Duration) {
        if let Some(handle) = &self.handle {
            let every = every.max(MIN_SWEEP);
            handle.shared.sweep_millis.store(u64::try_from(every.as_millis()).unwrap_or(u64::MAX), Ordering::Relaxed);
            handle.shared.wake.notify_all();
        }
    }

    /// The sweep time set now (default [`SWEEP`]); the thread may wait longer after a slow sweep (see [`sweep_interval`]).
    pub fn sweep(&self) -> Duration {
        self.handle.as_ref().map_or(SWEEP, |h| h.shared.sweep())
    }

    /// How long the thread's last sweep took.
    pub fn last_sweep(&self) -> Duration {
        self.handle.as_ref().map_or(Duration::ZERO, |h| Duration::from_micros(h.shared.last_sweep_micros.load(Ordering::Relaxed)))
    }

    /// Wait for the first listing. `false` when `timeout` passes first or the root cannot be read (see [`ProjectIndex::failure`]).
    pub fn wait_ready(&self, timeout: Duration) -> bool {
        let start = Instant::now();
        while self.generation() == 0 {
            if self.failure().is_some() || start.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(READY_POLL);
        }
        true
    }

    fn start(&mut self, root: &Path) {
        let shared = Arc::new(Shared {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            root: root.to_path_buf(),
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            stop: AtomicBool::new(false),
            news: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            walks: AtomicU64::new(0),
            sweep_millis: AtomicU64::new(u64::try_from(SWEEP.as_millis()).unwrap_or(u64::MAX)),
            last_sweep_micros: AtomicU64::new(0),
            failed: AtomicBool::new(false),
        });
        registry().push((shared.id, root.to_path_buf(), Arc::downgrade(&shared)));
        let handle = Handle { shared: shared.clone() };
        match std::thread::Builder::new().name("project-index".into()).spawn(move || run(shared)) {
            Ok(_) => self.handle = Some(handle),
            // The handle's drop takes the entry back out of the registry.
            Err(e) => {
                error!("the project index of {} could not start its thread: {e}", root.display());
                self.failed_root = Some(root.to_path_buf());
            }
        }
    }

    /// Follow `want` (or nothing). `wait` blocks for the first listing, which a stepped editor does so its first frame reads one.
    fn retarget(&mut self, want: Option<&Path>, wait: bool) {
        if self.root() == want {
            return;
        }
        self.handle = None;
        let Some(root) = want else {
            self.failed_root = None;
            return;
        };
        if self.failed_root.as_deref() == Some(root) {
            return;
        }
        self.failed_root = None;
        self.start(root);
        if wait && self.handle.is_some() && !self.wait_ready(FIRST_LISTING_WAIT) {
            warn!(
                "the project index of {} had no listing after {FIRST_LISTING_WAIT:?}: {}",
                root.display(),
                self.failure().unwrap_or_else(|| "it was still walking".to_owned())
            );
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
    let tried = want.is_some() && index.failed_root.as_deref() == want;
    if index.root() != want && !tried {
        index.retarget(want, waits.is_some());
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
        // **A project that is already open is followed from here**, so an operation that runs before the first frame has an index to ask.
        let root = app.world().get_resource::<Project>().map(|p| p.root.clone());
        if let Some(root) = root {
            let wait = app.world().contains_resource::<WaitsForFirstListing>();
            app.world_mut().resource_mut::<ProjectIndex>().retarget(Some(&root), wait);
        }
    }
}

/// **Add [`ProjectIndexPlugin`] unless it is there**: what a plugin that reads the index calls, so the dependency belongs to the
/// plugin that has it and an app cannot be built with a reader and no index.
pub fn ensure(app: &mut App) {
    if !app.is_plugin_added::<ProjectIndexPlugin>() {
        app.add_plugins(ProjectIndexPlugin);
    }
}
