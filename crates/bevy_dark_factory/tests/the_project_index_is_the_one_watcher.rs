//! **One watcher answers "did a file change?", and the panels subscribe** (#501, stage 1; `src/project_index.rs`).
//!
//! The issue's acceptance, as tests:
//!
//! - one project file written is **one** [`ProjectFileChanged`] published (not one per subscriber), and the UI thread makes no
//!   `stat` or `read_dir` in the frame after it, and the project is walked once;
//! - **the two modules that disagreed agree**: the finder's scan ([`crosslinks::scan`]), [`files::listed_files`] and the Files
//!   tree list the same files, and keep agreeing after a file is made, removed, and after `.gitignore` changes.
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
    assert_eq!(lookups_on_this_thread(), looked_at_start, "the UI thread never looked at the disk, in any frame");

    // **And the panel that subscribed has it**, with no poll of its own.
    let finder_rows = app.world().resource::<GotoIndex>().of_kind(Kind::File).any(|e| e.key == "data/door.yaml");
    assert!(finder_rows, "the finder lists the new file from the index");
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
    assert!(!files::listed_files(&root).contains(&"data/door.yaml".to_owned()));
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

/// The files `listed_files`, the finder's scan and the Files tree each name, from one moment of the index.
fn three_answers(app: &App, root: &Path) -> (Vec<String>, Vec<String>, BTreeSet<String>) {
    for _ in 0..5 {
        let index = app.world().resource::<ProjectIndex>();
        let listed = files::listed_files(root);
        let snapshot = index.snapshot().unwrap_or_else(|| panic!("the index has listed the project"));
        if index.snapshot().is_some_and(|again| again.generation != snapshot.generation) {
            continue;
        }
        let scan = crosslinks::scan(&snapshot, root, None);
        let tree: BTreeSet<String> = snapshot.tree.names.values().filter(|(_, dir)| !dir).map(|(rel, _)| rel.clone()).collect();
        return (listed, scan.files, tree);
    }
    panic!("the project kept changing under the test");
}

fn agree(app: &App, root: &Path, why: &str) -> Vec<String> {
    let (listed, scanned, tree) = three_answers(app, root);
    let text: Vec<String> = listed.iter().filter(|rel| crosslinks::is_text_file(rel)).cloned().collect();
    assert_eq!(scanned, text, "{why}: the finder's scan names the text files `listed_files` does");
    assert_eq!(tree, listed.iter().cloned().collect::<BTreeSet<_>>(), "{why}: the Files tree lists the files `listed_files` does");
    listed
}

/// **The modules that disagreed agree**: the finder's scan (which walked the tree's capped list on its own clock), `listed_files`
/// (which walked uncapped) and the Files tree return the same set, before and after files come and go and `.gitignore` changes.
#[test]
fn the_finders_scan_the_listing_and_the_tree_return_the_same_files() {
    let root = project("agree");
    write(&root, "scripts/door.rhai", "let open = true;\n");
    write(&root, "notes.md", "# notes\n");
    write(&root, "data/a.yaml", "a: 1\n");
    write(&root, "data/blob.bin", "x");
    write(&root, "deep/er/est/x.toml", "x = 1\n");
    write(&root, "secret.log", "x");
    write(&root, "cache/blob.txt", "x");
    write(&root, ".gitignore", "# build\n*.log\n/cache/\n");

    // **The counter measures**: a cold walk on this thread is seen by it, and it answers the same rules.
    let before = lookups_on_this_thread();
    let cold = files::listed_files(&root);
    assert!(lookups_on_this_thread() > before, "a walk on the UI thread shows in the counter");
    assert!(cold.contains(&"scripts/door.rhai".to_owned()) && !cold.contains(&"secret.log".to_owned()) && !cold.iter().any(|f| f.starts_with("cache/")));

    let mut app = stepped(&root);
    let listed = agree(&app, &root, "at the first listing");
    for want in ["scripts/door.rhai", "notes.md", "data/a.yaml", "data/blob.bin", "deep/er/est/x.toml", ".gitignore"] {
        assert!(listed.contains(&want.to_owned()), "{want} is listed: {listed:?}");
    }
    assert!(!listed.contains(&"secret.log".to_owned()) && !listed.iter().any(|f| f.starts_with("cache/")), "`.gitignore` hides what it names");
    assert!(cold.iter().all(|f| listed.contains(f)), "what the walk listed, the index lists");

    // **Files come and go from outside**: the index follows, and the three still agree.
    write(&root, "scripts/new.rhai", "let n = 1;\n");
    write(&root, "fresh/inner/z.txt", "z\n");
    write(&root, "fresh/pic.png", "png");
    std::fs::remove_file(root.join("notes.md")).unwrap_or_else(|e| panic!("{e}"));
    assert!(pump(&mut app, 10.0, |app| !heard_of(app, "fresh/pic.png").is_empty() && !heard_of(app, "notes.md").is_empty() && !heard_of(app, "scripts/new.rhai").is_empty()));
    let listed = agree(&app, &root, "after files came and went");
    assert!(listed.contains(&"scripts/new.rhai".to_owned()) && listed.contains(&"fresh/inner/z.txt".to_owned()) && !listed.contains(&"notes.md".to_owned()));
    assert_eq!(app.world().resource::<ProjectIndex>().walks(), 1, "none of that walked the project again");

    // **`.gitignore` changes what is listed**, which is the one change that walks again.
    write(&root, ".gitignore", "*.log\n/cache/\n*.txt\n");
    assert!(pump(&mut app, 10.0, |app| !heard_of(app, "fresh/inner/z.txt").iter().all(|e| e.kind != ChangeKind::Removed)));
    let listed = agree(&app, &root, "after `.gitignore` changed");
    assert!(!listed.contains(&"fresh/inner/z.txt".to_owned()), "a new `*.txt` rule hides it: {listed:?}");
    assert_eq!(app.world().resource::<ProjectIndex>().walks(), 2, "the `.gitignore` change is the one reason to walk again");
}
