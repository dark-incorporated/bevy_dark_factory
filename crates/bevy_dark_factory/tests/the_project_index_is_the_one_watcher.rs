//! **One watcher answers "did a file change?", and the panels it has been migrated to subscribe** (#501, stage 1;
//! `src/project_index.rs`).
//!
//! The issue's acceptance, as tests:
//!
//! - one project file written is **one** [`ProjectFileChanged`] published (not one per subscriber), even when the save takes longer
//!   than the debounce; the index's own seam makes no `stat` or `read_dir` on the UI thread, and the project is walked once;
//! - **the modules that disagreed agree**: the finder's scan ([`crosslinks::scan`]), [`files::listed_files`] and the Files tree list
//!   the files an independent walk written in this file lists, and keep agreeing after files come and go and `.gitignore` changes;
//! - there is no second path: with no index, the listing is an error and not a walk;
//! - a folder that cannot be read is read again, and what is in it is not lost.
//!
//! The settled-frame budget at 10,000 outliner nodes and 5,000 history lines is
//! `the_frame_a_change_lands_on_stays_inside_two_milliseconds.rs`, unchanged.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_dark_factory::goto::{GotoIndex, Kind};
use bevy_dark_factory::project_index::{ChangeKind, ProjectFileChanged, ProjectIndex, lookups_on_this_thread};
use bevy_dark_factory::{crosslinks, files, harness};

use crate::fixtures;

/// What one subscriber heard: each has its own cursor over the one message.
#[derive(Resource, Default)]
struct HeardByTheTree(Vec<ProjectFileChanged>);

#[derive(Resource, Default)]
struct HeardByTheFinder(Vec<ProjectFileChanged>);

fn hear_as_the_tree(mut read: MessageReader<ProjectFileChanged>, mut heard: ResMut<HeardByTheTree>) {
    heard.0.extend(read.read().cloned());
}

fn hear_as_the_finder(mut read: MessageReader<ProjectFileChanged>, mut heard: ResMut<HeardByTheFinder>) {
    heard.0.extend(read.read().cloned());
}

fn project(tag: &str) -> PathBuf {
    fixtures::Fixture::new(&format!("project_index_{tag}_{}", std::process::id())).side(400.0).descriptor("site/wall", "alpha").build("m")
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{parent:?}: {e}"));
    }
    std::fs::write(&path, text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
}

fn stepped(root: &Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<HeardByTheTree>()
        .init_resource::<HeardByTheFinder>()
        .add_systems(Update, (hear_as_the_tree, hear_as_the_finder));
    // **The frame's systems run on this thread**, so the lookup counter (which counts per thread) sees everything a frame does.
    app.edit_schedule(PreUpdate, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    app.edit_schedule(Update, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    for _ in 0..4 {
        app.update();
    }
    app
}

/// Step frames until `done`, for at most `seconds`: the thread runs on its own clock, so the wait is on the wall.
fn pump(app: &mut App, seconds: f64, mut done: impl FnMut(&mut App) -> bool) -> bool {
    let started = Instant::now();
    while started.elapsed().as_secs_f64() < seconds {
        app.update();
        if done(app) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

fn pump_for(app: &mut App, seconds: f64) {
    pump(app, seconds, |_| false);
}

fn heard_of(app: &App, rel: &str) -> Vec<ProjectFileChanged> {
    app.world().resource::<HeardByTheTree>().0.iter().filter(|e| e.rel == rel).cloned().collect()
}

fn listed(root: &Path) -> Vec<String> {
    files::listed_files(root).unwrap_or_else(|e| panic!("{e}"))
}

/// **One saved file is one event, whoever is listening, and the frame after it looks at nothing.**
///
/// The save is what an editor does: create, several writes, close. Two subscribers read the message, so a watcher that published
/// per subscriber would show as twice the writes.
#[test]
fn one_saved_file_is_one_event_for_every_subscriber_and_the_ui_thread_looks_at_nothing() {
    let root = project("one_event");
    std::fs::create_dir_all(root.join("data")).unwrap_or_else(|e| panic!("{e}"));
    let looked_at_start = lookups_on_this_thread();
    let mut app = stepped(&root);
    {
        let index = app.world().resource::<ProjectIndex>();
        assert_ne!(index.generation(), 0, "the first listing is in before the first frame reads it");
        assert_eq!(index.walks(), 1, "the project has been walked once");
    }

    // **A save**: a create, several writes and a close.
    let path = root.join("data/door.yaml");
    let mut written = 0u64;
    {
        let mut file = std::fs::File::create(&path).unwrap_or_else(|e| panic!("{e}"));
        for n in 0..4 {
            let line = format!("open: {n}\n");
            file.write_all(line.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
            written += line.len() as u64;
        }
    }
    assert!(pump(&mut app, 10.0, |app| !heard_of(app, "data/door.yaml").is_empty()), "the write was published within the sweep");

    // **The frame after it makes no lookup**: nothing that reads the index touches the disk.
    let before = lookups_on_this_thread();
    app.update();
    assert_eq!(lookups_on_this_thread(), before, "the frame after the event made a stat or a read_dir on the UI thread");

    // **Step past a sweep and the debounce**: the same save publishes nothing more.
    pump_for(&mut app, 1.6);
    let tree = heard_of(&app, "data/door.yaml");
    let finder: Vec<_> = app.world().resource::<HeardByTheFinder>().0.iter().filter(|e| e.rel == "data/door.yaml").cloned().collect();
    assert_eq!(tree.len(), 1, "one save is one event: {tree:?}");
    assert_eq!(finder, tree, "the second subscriber read the same one message");
    assert_eq!((tree[0].kind, tree[0].dir), (ChangeKind::Created, false));
    assert_eq!(tree[0].stamp.map(|s| s.len), Some(written), "the event carries the stamp the file ended with");
    let index = app.world().resource::<ProjectIndex>();
    assert_eq!(
        index.published() as usize,
        app.world().resource::<HeardByTheTree>().0.len(),
        "every message was written once and read once by each subscriber: nothing was published per subscriber"
    );
    assert_eq!(index.walks(), 1, "nobody walked the project again after the first listing");
    assert_eq!(
        lookups_on_this_thread(),
        looked_at_start,
        "the index's own seam made a stat, a read_dir or a .gitignore read on the UI thread (another module's `std::fs` call is not counted)"
    );

    // **And the panel that subscribed has it**, with no poll of its own.
    let finder_rows = app.world().resource::<GotoIndex>().of_kind(Kind::File).any(|e| e.key == "data/door.yaml");
    assert!(finder_rows, "the finder lists the new file from the index");
}

/// **A save that takes longer than the debounce is still one event.**
///
/// The sweeps are set 20 ms apart and the file is written five times, 50 ms apart (under the 100 ms debounce, as a literal), so the
/// file is seen at five different sizes across 250 ms. Without the debounce the first sweep would publish a half-written file and the
/// later ones more.
#[test]
fn a_save_that_takes_longer_than_the_debounce_is_still_one_event() {
    let root = project("debounce");
    std::fs::create_dir_all(root.join("data")).unwrap_or_else(|e| panic!("{e}"));
    let mut app = stepped(&root);
    app.world().resource::<ProjectIndex>().set_sweep(Duration::from_millis(20));

    // **A literal, not derived from `DEBOUNCE`**: a test whose gap moved with the constant could not tell the constant was gone.
    let gap = Duration::from_millis(50);
    let path = root.join("data/long.yaml");
    let mut file = std::fs::File::create(&path).unwrap_or_else(|e| panic!("{e}"));
    let mut written = 0u64;
    for n in 0..5 {
        let line = format!("part: {n}\n");
        file.write_all(line.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
        file.flush().unwrap_or_else(|e| panic!("{e}"));
        written += line.len() as u64;
        let until = Instant::now() + gap;
        while Instant::now() < until {
            app.update();
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    drop(file);
    assert!(pump(&mut app, 10.0, |app| !heard_of(app, "data/long.yaml").is_empty()), "the finished save was published");
    pump_for(&mut app, 1.0);
    let heard = heard_of(&app, "data/long.yaml");
    assert_eq!(heard.len(), 1, "five writes over {:?} are one event: {heard:?}", gap * 5);
    assert_eq!((heard[0].kind, heard[0].stamp.map(|s| s.len)), (ChangeKind::Created, Some(written)), "and it is the file as it ended");
}

/// **An atomic save (a new file renamed over the old one) is one `Modified`, and a removal is one `Removed`.**
#[test]
fn an_atomic_save_is_one_modified_and_a_delete_is_one_removed() {
    let root = project("atomic");
    write(&root, "data/door.yaml", "open: 0\n");
    let mut app = stepped(&root);

    write(&root, "data/door.yaml.tmp", "open: 1\nand: more\n");
    std::fs::rename(root.join("data/door.yaml.tmp"), root.join("data/door.yaml")).unwrap_or_else(|e| panic!("{e}"));
    assert!(pump(&mut app, 10.0, |app| !heard_of(app, "data/door.yaml").is_empty()));
    pump_for(&mut app, 1.6);
    let saved = heard_of(&app, "data/door.yaml");
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_eq!(saved[0].kind, ChangeKind::Modified);
    assert_eq!(saved[0].stamp.map(|s| s.len), Some("open: 1\nand: more\n".len() as u64));
    assert!(heard_of(&app, "data/door.yaml.tmp").is_empty(), "the temporary name was never a listed file: {:?}", heard_of(&app, "data/door.yaml.tmp"));

    std::fs::remove_file(root.join("data/door.yaml")).unwrap_or_else(|e| panic!("{e}"));
    assert!(pump(&mut app, 10.0, |app| heard_of(app, "data/door.yaml").len() == 2));
    pump_for(&mut app, 1.2);
    let all = heard_of(&app, "data/door.yaml");
    assert_eq!(all.len(), 2, "{all:?}");
    assert_eq!((all[1].kind, all[1].stamp), (ChangeKind::Removed, None));
    assert!(!listed(&root).contains(&"data/door.yaml".to_owned()));
}

/// **A file made inside a new folder is heard of after the folder**, so a subscriber never meets a file in a folder it has not heard of.
#[test]
fn a_new_folder_is_heard_before_the_file_in_it() {
    let root = project("folder");
    let mut app = stepped(&root);
    write(&root, "fresh/inner/z.txt", "z\n");
    assert!(pump(&mut app, 10.0, |app| !heard_of(app, "fresh/inner/z.txt").is_empty()));
    pump_for(&mut app, 1.2);
    let heard: Vec<(String, bool)> = app
        .world()
        .resource::<HeardByTheTree>()
        .0
        .iter()
        .filter(|e| e.rel.starts_with("fresh"))
        .map(|e| (e.rel.clone(), e.dir))
        .collect();
    assert_eq!(
        heard,
        vec![("fresh".to_owned(), true), ("fresh/inner".to_owned(), true), ("fresh/inner/z.txt".to_owned(), false)],
        "each entry once, folders first"
    );
}

/// **There is one path, and it fails loudly**: a root no index follows has no listing, and the answer says so; nothing walks for it.
#[test]
fn a_root_no_index_follows_is_an_error_and_not_a_walk() {
    let root = project("nobody_follows");
    let before = lookups_on_this_thread();
    let refused = files::listed_files(&root).expect_err("no index follows this root");
    assert!(refused.contains("no project index follows"), "{refused}");
    assert_eq!(lookups_on_this_thread(), before, "and the refusal did not walk the project to find out");
}

/// The rules a project's listing follows, **written out here and not taken from the crate**: a folder named `.git`, `target`, `.live`
/// or `node_modules` is not listed; nor is a file named `.DS_Store`, nor a link; and the root's `.gitignore` hides, for the two
/// shapes these tests use, a file whose name ends with a suffix (`*.log`, `*.txt`) and a folder at the root (`/cache/`).
fn reference_walk(root: &Path, hidden_suffixes: &[&str], hidden_root_folders: &[&str]) -> BTreeSet<String> {
    fn go(root: &Path, dir: &Path, suffixes: &[&str], folders: &[&str], out: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{dir:?}: {e}")) {
            let entry = entry.unwrap_or_else(|e| panic!("{e}"));
            let meta = std::fs::symlink_metadata(entry.path()).unwrap_or_else(|e| panic!("{e}"));
            let name = entry.file_name().to_string_lossy().into_owned();
            let rel = entry.path().strip_prefix(root).unwrap_or_else(|e| panic!("{e}")).to_string_lossy().into_owned();
            if meta.file_type().is_symlink() || name == ".DS_Store" {
                continue;
            }
            if meta.is_dir() {
                if [".git", "target", ".live", "node_modules"].contains(&name.as_str()) || (dir == root && folders.contains(&name.as_str())) {
                    continue;
                }
                go(root, &entry.path(), suffixes, folders, out);
            } else if !suffixes.iter().any(|s| name.ends_with(s)) {
                out.insert(rel);
            }
        }
    }
    let mut out = BTreeSet::new();
    go(root, root, hidden_suffixes, hidden_root_folders, &mut out);
    out
}

/// **The three answers and the independent walk, until they are the same** (the editor writes a file or two of its own, which the
/// index publishes after the debounce): the finder's scan, `listed_files` and the Files tree against `reference_walk`.
fn agree_with_the_walk(app: &mut App, root: &Path, suffixes: &[&str], folders: &[&str], why: &str) -> BTreeSet<String> {
    let mut last = String::new();
    for _ in 0..60 {
        let index = app.world().resource::<ProjectIndex>();
        let from_the_index: BTreeSet<String> = listed(root).into_iter().collect();
        let snapshot = index.snapshot().unwrap_or_else(|| panic!("the index has listed the project"));
        let scan = crosslinks::scan(&snapshot, root, None);
        let tree: BTreeSet<String> = snapshot.tree.names.values().filter(|(_, dir)| !dir).map(|(rel, _)| rel.clone()).collect();
        let walked = reference_walk(root, suffixes, folders);
        let text: BTreeSet<String> = walked.iter().filter(|rel| crosslinks::is_text_file(rel)).cloned().collect();
        let scanned: BTreeSet<String> = scan.files.iter().cloned().collect();
        if from_the_index == walked && tree == walked && scanned == text && snapshot.generation == index.generation() {
            return walked;
        }
        last = format!(
            "listed_files minus walk: {:?}, walk minus listed_files: {:?}; tree minus walk: {:?}; scan minus walk's text: {:?}, walk's text minus scan: {:?}",
            from_the_index.difference(&walked).collect::<Vec<_>>(),
            walked.difference(&from_the_index).collect::<Vec<_>>(),
            tree.difference(&walked).collect::<Vec<_>>(),
            scanned.difference(&text).collect::<Vec<_>>(),
            text.difference(&scanned).collect::<Vec<_>>(),
        );
        std::thread::sleep(Duration::from_millis(100));
        app.update();
    }
    panic!("{why}: the index and an independent walk of the project never agreed: {last}");
}

/// **The modules that disagreed agree**: the finder's scan, `listed_files` and the Files tree return what an independent walk of the
/// project lists, before and after files come and go and `.gitignore` changes, with a link, a `.DS_Store` and the folders the tree
/// never lists all in the project.
#[test]
fn the_finders_scan_the_listing_and_the_tree_return_what_an_independent_walk_lists() {
    let root = project("agree");
    write(&root, "scripts/door.rhai", "let open = true;\n");
    write(&root, "notes.md", "# notes\n");
    write(&root, "data/a.yaml", "a: 1\n");
    write(&root, "data/blob.bin", "x");
    write(&root, "deep/er/est/x.toml", "x = 1\n");
    write(&root, "secret.log", "x");
    write(&root, "cache/blob.txt", "x");
    write(&root, ".DS_Store", "x");
    write(&root, "data/.DS_Store", "x");
    write(&root, "node_modules/pkg/index.json", "{}");
    write(&root, "target/debug/junk.txt", "x");
    write(&root, ".git/config", "x");
    write(&root, ".live/game.yaml", "x: 1\n");
    write(&root, ".gitignore", "# build\n*.log\n/cache/\n");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("notes.md"), root.join("linked.md")).unwrap_or_else(|e| panic!("{e}"));
        std::os::unix::fs::symlink(root.join("data"), root.join("linked_folder")).unwrap_or_else(|e| panic!("{e}"));
    }

    let mut app = stepped(&root);
    let walked = agree_with_the_walk(&mut app, &root, &[".log"], &["cache"], "at the first listing");
    for want in ["scripts/door.rhai", "notes.md", "data/a.yaml", "data/blob.bin", "deep/er/est/x.toml", ".gitignore"] {
        assert!(walked.contains(want), "{want} is listed: {walked:?}");
    }
    for never in ["secret.log", ".DS_Store", "data/.DS_Store", "linked.md", "linked_folder/a.yaml", "node_modules/pkg/index.json", "target/debug/junk.txt", ".git/config", ".live/game.yaml", "cache/blob.txt"] {
        assert!(!walked.contains(never), "{never} is not listed: {walked:?}");
    }

    // **Files come and go from outside**: the index follows, and the answers still agree with the walk.
    write(&root, "scripts/new.rhai", "let n = 1;\n");
    write(&root, "fresh/inner/z.txt", "z\n");
    write(&root, "fresh/pic.png", "png");
    write(&root, "fresh/.DS_Store", "x");
    std::fs::remove_file(root.join("notes.md")).unwrap_or_else(|e| panic!("{e}"));
    assert!(pump(&mut app, 10.0, |app| !heard_of(app, "fresh/pic.png").is_empty() && !heard_of(app, "notes.md").is_empty() && !heard_of(app, "scripts/new.rhai").is_empty()));
    let walked = agree_with_the_walk(&mut app, &root, &[".log"], &["cache"], "after files came and went");
    assert!(walked.contains("scripts/new.rhai") && walked.contains("fresh/inner/z.txt") && !walked.contains("notes.md"));
    assert_eq!(app.world().resource::<ProjectIndex>().walks(), 1, "none of that walked the project again");

    // **`.gitignore` changes what is listed**, which is the one change that walks again.
    write(&root, ".gitignore", "*.log\n/cache/\n*.txt\n");
    assert!(pump(&mut app, 10.0, |app| heard_of(app, "fresh/inner/z.txt").iter().any(|e| e.kind == ChangeKind::Removed)));
    let walked = agree_with_the_walk(&mut app, &root, &[".log", ".txt"], &["cache"], "after `.gitignore` changed");
    assert!(!walked.contains("fresh/inner/z.txt"), "a new `*.txt` rule hides it: {walked:?}");
    assert_eq!(app.world().resource::<ProjectIndex>().walks(), 2, "the `.gitignore` change is the one reason to walk again");
}

/// **The tree's caps are the tree's**: a folder past `TREE_DIR_LIMIT` lists its first entries and says it stopped, and a project
/// past `TREE_LIMIT` entries stops there; under both caps the tree lists everything, folders first.
#[test]
fn the_tree_stops_at_its_caps_and_says_so() {
    let build = |entries: &[(String, bool)]| files::build_tree(entries.iter().map(|(rel, dir)| (rel.as_str(), *dir)));

    let mut one_folder = vec![("big".to_owned(), true)];
    one_folder.extend((0..files::TREE_DIR_LIMIT + 100).map(|n| (format!("big/f{n:05}.txt"), false)));
    let built = build(&one_folder);
    assert!(built.truncated, "a folder past TREE_DIR_LIMIT says the tree stopped");
    assert_eq!(built.names.len(), files::TREE_DIR_LIMIT + 1, "the folder and its first TREE_DIR_LIMIT entries");

    let mut many = Vec::new();
    for folder in 0..6 {
        many.push((format!("d{folder}"), true));
        many.extend((0..files::TREE_DIR_LIMIT - 500).map(|n| (format!("d{folder}/f{n:05}.txt"), false)));
    }
    let built = build(&many);
    assert!(built.truncated, "a project past TREE_LIMIT says the tree stopped");
    assert_eq!(built.names.len(), files::TREE_LIMIT, "and lists exactly TREE_LIMIT entries");

    let small = [("b.txt".to_owned(), false), ("A.txt".to_owned(), false), ("dir".to_owned(), true), ("dir/z.txt".to_owned(), false)];
    let built = build(&small);
    assert!(!built.truncated, "under the caps nothing is cut");
    assert_eq!(built.names.len(), 4);
    assert_eq!(built.nodes.len(), 3, "three at the top: the folder with its child inside it");
    let mut more = small.to_vec();
    more.push(("c.txt".to_owned(), false));
    assert_ne!(build(&more).fingerprint, built.fingerprint, "an entry added moves the fingerprint");
}

/// Puts a folder's permissions back, whatever the test did.
#[cfg(unix)]
struct Restore(PathBuf);

#[cfg(unix)]
impl Drop for Restore {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&self.0, std::fs::Permissions::from_mode(0o755));
    }
}

/// **A folder that cannot be read is read again, and what is in it is not lost.** The index lists it empty and says so (once), and
/// does not move the folder on: when it can be read, what it holds is published.
#[cfg(unix)]
#[test]
fn an_unreadable_folder_is_read_again_and_what_is_in_it_is_not_lost() {
    use std::os::unix::fs::PermissionsExt;
    let root = project("locked");
    write(&root, "locked/inside.txt", "x");
    let locked = root.join("locked");
    let _restore = Restore(locked.clone());
    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap_or_else(|e| panic!("{e}"));
    if std::fs::read_dir(&locked).is_ok() {
        eprintln!("a_folder_with_no_permissions_can_be_read_by_this_user (root?): the unreadable-folder case cannot be made here");
        return;
    }

    let mut app = stepped(&root);
    app.world().resource::<ProjectIndex>().set_sweep(Duration::from_millis(50));
    assert!(!listed(&root).contains(&"locked/inside.txt".to_owned()), "what is behind a folder that cannot be read is not listed");
    assert!(listed(&root).iter().all(|f| !f.starts_with("locked/")));
    pump_for(&mut app, 0.5);
    assert!(heard_of(&app, "locked/inside.txt").is_empty());

    std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap_or_else(|e| panic!("{e}"));
    assert!(pump(&mut app, 10.0, |app| !heard_of(app, "locked/inside.txt").is_empty()), "once the folder can be read, what it holds is published");
    assert_eq!(heard_of(&app, "locked/inside.txt")[0].kind, ChangeKind::Created);
    assert!(listed(&root).contains(&"locked/inside.txt".to_owned()));
}
