//! **Two modules read the project index instead of a clock** (#501, checklist items 3 and 4): an open file in the Files workspace reloads
//! when the index says it moved (`files::POLL_S`, 0.25 s, is gone), and the inspector's `Used by` line reads `maps/` again when a map file
//! moves (`crosslinks::USED_BY_POLL_S`, 1.0 s, is gone).
//!
//! # Why these can fail
//!
//! Each test makes the index sweep as often as it may ([`MIN_SWEEP`]), so the sweep is not what is slow, changes a file with plain
//! `std::fs` calls, and waits (a bounded number of steps) for the panel to show what an **independent `std::fs` read** says the disk holds. The
//! only road from the disk to the panel is the [`ProjectFileChanged`] message: take the subscription out and nothing else looks at the file,
//! so the wait runs out and the test fails.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::crosslinks::UsedByLine;
use bevy_dark_factory::editor::lock_on_placement;
use bevy_dark_factory::files::{self, Files};
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::project::OpenMap;
use bevy_dark_factory::project_index::{MIN_SWEEP, ProjectIndex};
use bevy_dark_factory::workspace::Workspace;
use bevy_dark_factory::{commands, harness};

use crate::fixtures;
use crate::settle_support::Frames;

/// How many frames one step runs, so a message published this frame has been read by every subscriber.
const FRAMES_PER_STEP: usize = 3;

/// How long the test sleeps between steps, in milliseconds.
const STEP_SLEEP_MS: u64 = 5;

/// How many steps the panel may take to show a change: at least ten seconds at [`STEP_SLEEP_MS`], counted in steps so a starved host is
/// not read as a change never heard (#644). The index's sweep is [`MIN_SWEEP`] here and its debounce 100 ms; this is far more than both
/// together, so running out means the change was never heard, not that it was slow. The steps taken are printed.
const SEEN_WITHIN: Frames = Frames::covering(Duration::from_secs(10), Duration::from_millis(STEP_SLEEP_MS));

fn project(tag: &str) -> PathBuf {
    fixtures::Fixture::new(&format!("index_subscribers_{tag}_{}", std::process::id()))
        .descriptor("alpha/bed", "alpha")
        .place_as("bed@0", "alpha/bed", (0.0, 0.0))
        .build("m")
}

fn editor(root: &Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world().resource::<ProjectIndex>().set_sweep(MIN_SWEEP);
    step(&mut app);
    app
}

fn step(app: &mut App) {
    for _ in 0..FRAMES_PER_STEP {
        app.update();
    }
}

/// Step until `done`, for at most [`SEEN_WITHIN`] steps; says how many it took, or panics saying what was not seen.
fn wait_for(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) -> Frames {
    for stepped in 1..=SEEN_WITHIN.0 {
        step(app);
        if done(app) {
            let took = Frames(stepped);
            println!("{what}: seen after {took:?}");
            return took;
        }
        std::thread::sleep(Duration::from_millis(STEP_SLEEP_MS));
    }
    panic!("{what}: not seen within {SEEN_WITHIN:?}");
}

fn in_the_files_workspace(app: &mut App) {
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    step(app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "the Files workspace is reachable by its command");
}

fn open(app: &mut App, rel: &str) {
    files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    step(app);
}

fn buffer(app: &App, rel: &str) -> files::OpenFile {
    app.world().resource::<Files>().get(rel).cloned().unwrap_or_else(|| panic!("{rel} is open"))
}

fn on_disk(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// **An open file nobody has typed in reloads when its own `Modified` event arrives**, and what it shows is what `std::fs` reads.
#[test]
fn an_open_file_reloads_when_the_index_says_it_moved() {
    let root = project("reload");
    std::fs::write(root.join("notes.md"), "# first\n").unwrap_or_else(|e| panic!("{e}"));
    let mut app = editor(&root);
    in_the_files_workspace(&mut app);
    open(&mut app, "notes.md");
    assert_eq!(buffer(&app, "notes.md").text, "# first\n", "it opened with what the disk held");

    std::fs::write(root.join("notes.md"), "# second, and longer\n").unwrap_or_else(|e| panic!("{e}"));
    wait_for(&mut app, "the open file shows the disk's text", |app| buffer(app, "notes.md").text == on_disk(&root, "notes.md"));
    assert_eq!(buffer(&app, "notes.md").text, "# second, and longer\n");
    assert!(buffer(&app, "notes.md").external.is_none(), "nothing was typed, so nothing is asked");
}

/// **A file with unsaved edits still keeps them**: the change from outside waits as a question, the person's text is untouched and the
/// file on disk is not written over.
#[test]
fn a_file_with_unsaved_edits_is_not_overwritten_by_the_index_event() {
    let root = project("unsaved");
    std::fs::write(root.join("notes.md"), "# first\n").unwrap_or_else(|e| panic!("{e}"));
    let mut app = editor(&root);
    in_the_files_workspace(&mut app);
    open(&mut app, "notes.md");
    let mine = "# first\nmy unsaved line\n";
    app.world_mut().resource_mut::<Files>().get_mut("notes.md").unwrap_or_else(|| panic!("open")).text = mine.to_owned();

    let outside = "# first, changed from outside\n";
    std::fs::write(root.join("notes.md"), outside).unwrap_or_else(|e| panic!("{e}"));
    wait_for(&mut app, "the outside change waits as a question", |app| buffer(app, "notes.md").external.is_some());
    let file = buffer(&app, "notes.md");
    assert_eq!(file.external.as_deref(), Some(on_disk(&root, "notes.md").as_str()), "the question holds what the disk holds");
    assert_eq!(file.text, mine, "the person's text is untouched");
    assert_eq!(on_disk(&root, "notes.md"), outside, "and the file is not written over");
}

/// The line the inspector's `Used by` shows, when the references block is up.
fn used_by_text(app: &mut App) -> Option<String> {
    let world = app.world_mut();
    world.query_filtered::<&Text, With<UsedByLine>>().iter(world).next().map(|t| t.0.clone())
}

/// **The `Used by` line is read again when a map file appears in `maps/`**, and says what an independent read of the folder finds.
#[test]
fn the_used_by_line_reads_maps_again_when_a_map_file_moves() {
    let root = project("used_by");
    let mut app = editor(&root);
    let (id, piece) = {
        let open = app.world().resource::<OpenMap>();
        (open.map.placements[0].id.clone(), open.map.placements[0].descriptor.clone())
    };
    assert!(lock_on_placement(app.world_mut(), &id));
    wait_for(&mut app, "the line is up", |app| used_by_text(app).is_some());
    assert_eq!(used_by_text(&mut app).as_deref(), Some("placed in m"), "one map places it");

    // **A second map that places the piece**, written with plain `std::fs`.
    let maps = {
        let project = app.world().resource::<bevy_dark_factory::project::Project>();
        project.maps_dir.clone()
    };
    let text = std::fs::read_to_string(maps.join("m.map.yaml")).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("\nname: m\n") || text.starts_with("name: m\n"), "the fixture's map names itself `m`: {text}");
    std::fs::write(maps.join("second.map.yaml"), text.replacen("name: m\n", "name: second\n", 1)).unwrap_or_else(|e| panic!("{e}"));

    // The independent ground truth: the `name:` of every `*.map.yaml` of the folder whose text names the piece (a map is called by its own name).
    let mut names: Vec<String> = std::fs::read_dir(&maps)
        .unwrap_or_else(|e| panic!("{e}"))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| std::fs::read_to_string(path).is_ok_and(|text| text.contains(&piece)))
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .filter_map(|text| text.lines().find_map(|line| line.strip_prefix("name: ").map(|name| name.trim().to_owned())))
        .collect();
    names.sort();
    let want = format!("placed in {} maps: {}", names.len(), names.join(", "));
    assert_eq!(names, ["m", "second"], "the ground truth is two maps");
    wait_for(&mut app, "the line names both maps", |app| used_by_text(app).as_deref() == Some(want.as_str()));
}

/// Step until the project index lists `rel` (so its event has been published), then a few frames more so every subscriber has read it.
fn wait_until_listed(app: &mut App, rel: &str) {
    wait_for(app, &format!("the index lists {rel}"), |app| {
        app.world().resource::<ProjectIndex>().snapshot().is_some_and(|s| s.files.iter().any(|(f, _)| f == rel))
    });
    step(app);
    step(app);
}

/// **A change to a file nobody has open, and to another open file, reads only what moved**: the counter of event-driven reads moves by
/// the one file that changed and by nothing for the unrelated one.
#[test]
fn an_unrelated_change_causes_no_read_of_an_open_file() {
    let root = project("unrelated_files");
    std::fs::write(root.join("a.md"), "# a\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("b.md"), "# b\n").unwrap_or_else(|e| panic!("{e}"));
    let mut app = editor(&root);
    in_the_files_workspace(&mut app);
    open(&mut app, "a.md");
    open(&mut app, "b.md");
    wait_until_listed(&mut app, "b.md");
    let before = files::reads_after_an_event();

    // **A file that is not open**: heard by the index, and no open file is read.
    std::fs::write(root.join("unrelated.txt"), "nothing opened this\n").unwrap_or_else(|e| panic!("{e}"));
    wait_until_listed(&mut app, "unrelated.txt");
    assert_eq!(files::reads_after_an_event(), before, "a file nobody opened was read");
    assert_eq!(buffer(&app, "a.md").text, "# a\n");

    // **Another open file**: that one is read once, and `a.md` is not.
    std::fs::write(root.join("b.md"), "# b, changed\n").unwrap_or_else(|e| panic!("{e}"));
    wait_for(&mut app, "b.md reloads", |app| buffer(app, "b.md").text == on_disk(&root, "b.md"));
    step(&mut app);
    step(&mut app);
    assert_eq!(files::reads_after_an_event(), before + 1, "exactly the one file that moved was read");
    assert_eq!(buffer(&app, "a.md").text, "# a\n", "and the other open file is as it was");
}

/// **The `Used by` line reads `maps/` for a map file and for nothing else**: a text file in `maps/`, a map file in a folder below it and
/// a file elsewhere cause no read; a second map does (the control that shows the counter moves).
#[test]
fn an_unrelated_change_causes_no_read_of_maps() {
    let root = project("unrelated_maps");
    let mut app = editor(&root);
    let id = app.world().resource::<OpenMap>().map.placements[0].id.clone();
    assert!(lock_on_placement(app.world_mut(), &id));
    wait_for(&mut app, "the line is up", |app| used_by_text(app).is_some());
    step(&mut app);
    let (maps, project_root) = {
        let project = app.world().resource::<bevy_dark_factory::project::Project>();
        (project.maps_dir.clone(), project.root.clone())
    };
    let rel = |path: &Path| path.strip_prefix(&project_root).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or_else(|e| panic!("{e}"));
    let before = bevy_dark_factory::crosslinks::used_by_reads();
    let line = used_by_text(&mut app);

    std::fs::create_dir_all(maps.join("sub")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(maps.join("readme.txt"), "not a map\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(maps.join("notes.yaml"), "a: 1\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(maps.join("sub/deeper.map.yaml"), "name: deeper\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("elsewhere.yaml"), "a: 1\n").unwrap_or_else(|e| panic!("{e}"));
    for path in [maps.join("readme.txt"), maps.join("notes.yaml"), maps.join("sub/deeper.map.yaml"), root.join("elsewhere.yaml")] {
        wait_until_listed(&mut app, &rel(&path));
    }
    assert_eq!(bevy_dark_factory::crosslinks::used_by_reads(), before, "an unrelated change made the line read `maps/`");
    assert_eq!(used_by_text(&mut app), line, "and the line is as it was");

    // **The control**: a map file in the maps folder is a read.
    let text = std::fs::read_to_string(maps.join("m.map.yaml")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(maps.join("second.map.yaml"), text.replacen("name: m\n", "name: second\n", 1)).unwrap_or_else(|e| panic!("{e}"));
    wait_for(&mut app, "a map file is a read", |_| bevy_dark_factory::crosslinks::used_by_reads() > before);
}
