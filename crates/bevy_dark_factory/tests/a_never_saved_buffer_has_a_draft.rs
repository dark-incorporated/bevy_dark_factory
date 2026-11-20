//! **A buffer that was never saved has a draft on disk, and an interval saves the ordinary way** (#508).
//!
//! Between two `Cmd+S` presses an unsaved buffer lives only in memory, so a crash takes the last minutes back — and a buffer
//! that was never saved to disk at all has no copy anywhere. Two things answer it, and only the second is unconditional:
//!
//! 1. **A draft**: every unsaved buffer reaches a file under the person's own data folder
//!    ([`blueprint_model::user_dirs::drafts_dir`], never inside a project), and a fresh editor on the same project offers it
//!    with the file's name and when it was taken; declining deletes it.
//! 2. **The interval**: the map and the buffers are saved on a timer, through the **manual save path** — `map_save` and
//!    `files::save_all` — so there is one line in the log and no second, silent write path. The timer is parked while a
//!    question is up, while no project is open, and while the game plays, so it never fires at a moment that would be wrong.
//!
//! And the status band says *Auto save in Ns* **only in the last five seconds**, because a countdown drawn all the time is a
//! row a person stops seeing.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::draft::{self, DraftOffers, Drafted};
use bevy_dark_factory::files::{self, Files};
use bevy_dark_factory::harness;
use bevy_dark_factory::project::OpenMap;
use bevy_dark_factory::settings::EditorSettings;

use crate::fixtures;

const FRAME: f32 = 1.0 / 60.0;

fn project(tag: &str) -> PathBuf {
    fixtures::Fixture::new(&format!("drafts_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m")
}

fn editor(root: &Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME)));
    for _ in 0..6 {
        app.update();
    }
    app
}

/// **Turn the interval on through the project's settings layer**, not by inserting `EditorSettings`: `sync_settings`
/// re-resolves the resource from the layer files every frame, so an inserted value is replaced by the default in the next one.
fn with_interval(root: &Path, seconds: f64) -> App {
    let path = root.join("assets/blueprint/editor.yaml");
    std::fs::write(&path, format!("version: 1\nauto_save_interval: {seconds}\n")).unwrap_or_else(|e| panic!("{e}"));
    let mut app = editor(root);
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(app.world().resource::<EditorSettings>().auto_save_interval_s, seconds, "the project layer armed the interval");
    app
}

/// **A draft is written for a buffer nobody saved**, and it is not inside the project.
#[test]
fn an_unsaved_buffer_reaches_a_draft_outside_the_project() {
    let root = project("reaches");
    std::fs::write(root.join("notes.md"), "# on disk\n").unwrap_or_else(|e| panic!("{e}"));
    let mut app = editor(&root);
    files::open_file(app.world_mut(), "notes.md").unwrap_or_else(|e| panic!("{e}"));
    {
        let mut files = app.world_mut().resource_mut::<Files>();
        let file = files.get_mut("notes.md").expect("open");
        file.text = "# typed and never saved\n".to_owned();
    }
    // **The write is on a one-second clock**, so step past it rather than reading inside the same frame.
    step_seconds(&mut app, 2.0);
    let drafts = draft::offers(&root);
    assert_eq!(drafts.len(), 1, "one unsaved buffer, one draft: {drafts:?}");
    assert_eq!(drafts[0].path, "notes.md");
    assert_eq!(drafts[0].text, "# typed and never saved\n", "the person's text, byte for byte");
    assert!(!drafts[0].taken_at.is_empty(), "the draft says when it was taken, so the offer can");
    // **Never inside the project**: a draft is not data a game or an agent reads.
    let file = draft::file_for(&root).expect("this machine has a data folder");
    assert!(!file.starts_with(&root), "the draft is {} — inside the project at {}", file.display(), root.display());
}

/// **A fresh editor offers the draft with its file's name and when it was taken, and declining deletes it.**
#[test]
fn a_fresh_editor_offers_the_draft_and_declining_deletes_it() {
    let root = project("offers");
    std::fs::write(root.join("notes.md"), "# on disk\n").unwrap_or_else(|e| panic!("{e}"));
    let taken = "2026-11-20T09:30:00Z".to_owned();
    draft::write(
        &root,
        vec![Drafted { path: "notes.md".to_owned(), text: "# typed\n".to_owned(), disk: "# on disk\n".to_owned(), taken_at: taken.clone() }],
    )
    .unwrap_or_else(|e| panic!("{e}"));

    // **A fresh editor on the same project**, as a launch after a crash is.
    let mut app = editor(&root);
    step_seconds(&mut app, 0.5);
    let waiting = app.world().resource::<DraftOffers>().waiting.clone();
    assert_eq!(waiting.len(), 1, "the draft is offered: {waiting:?}");
    assert_eq!(waiting[0].path, "notes.md");

    // **Taking it puts the person's text into the file's buffer, unsaved.**
    let mut offers = app.world_mut().remove_resource::<DraftOffers>().expect("offers");
    offers.take(app.world_mut(), "notes.md").unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().insert_resource(offers);
    let files = app.world().resource::<Files>();
    assert_eq!(files.get("notes.md").map(|f| f.text.as_str()), Some("# typed\n"), "the draft's text is the buffer's");
    assert!(files.is_dirty("notes.md"), "it is unsaved, which is what taking a draft means");
    assert!(draft::offers(&root).is_empty(), "a taken draft is gone, so the next launch does not ask again");

    // **And declining deletes it.**
    draft::write(&root, vec![Drafted { path: "notes.md".to_owned(), text: "# typed again\n".to_owned(), disk: "# on disk\n".to_owned(), taken_at: taken.clone() }])
        .unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<DraftOffers>().waiting = draft::offers(&root);
    let mut offers = app.world_mut().remove_resource::<DraftOffers>().expect("offers");
    offers.decline("notes.md").unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().insert_resource(offers);
    assert!(draft::offers(&root).is_empty(), "a declined draft is deleted rather than offered every launch");
}

/// **The offer says the file's name and when it was taken**, and whether the file has moved on since.
#[test]
fn the_offer_names_the_file_the_time_and_whether_the_file_moved_on() {
    let draft_of = Drafted { path: "notes.md".to_owned(), text: "# typed\n".to_owned(), disk: "# on disk\n".to_owned(), taken_at: "2026-11-20T09:30:00Z".to_owned() };
    let said = draft::offer_sentence(&draft_of, Some("# on disk\n"));
    assert!(said.contains("notes.md") && said.contains("2026-11-20T09:30:00Z"), "{said}");
    assert!(!said.contains("changed on disk"), "the file is as the draft left it: {said}");
    let moved = draft::offer_sentence(&draft_of, Some("# somebody else's\n"));
    assert!(moved.contains("changed on disk"), "{moved}");
    let gone = draft::offer_sentence(&draft_of, None);
    assert!(gone.contains("gone"), "{gone}");
}

/// **Everything with unsaved edits reaches the draft**, and a file with none is not in it.
#[test]
fn the_draft_holds_every_dirty_buffer_and_no_clean_one() {
    let root = project("every");
    for rel in ["a.md", "b.md"] {
        std::fs::write(root.join(rel), "# clean\n").unwrap_or_else(|e| panic!("{e}"));
    }
    let mut app = editor(&root);
    for rel in ["a.md", "b.md"] {
        files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    }
    {
        let mut files = app.world_mut().resource_mut::<Files>();
        if let Some(file) = files.get_mut("a.md") {
            file.text = "# typed in a\n".to_owned();
        }
    }
    step_seconds(&mut app, 2.0);
    let paths: Vec<String> = draft::offers(&root).into_iter().map(|d| d.path).collect();
    assert_eq!(paths, vec!["a.md".to_owned()], "only the dirty buffer is drafted; `b.md` is as the disk has it");
}

/// **A clean project has no drafts file at all**, so the offer is silent on every ordinary launch.
#[test]
fn a_project_with_nothing_unsaved_leaves_no_draft() {
    let root = project("clean");
    std::fs::write(root.join("notes.md"), "# on disk\n").unwrap_or_else(|e| panic!("{e}"));
    let mut app = editor(&root);
    files::open_file(app.world_mut(), "notes.md").unwrap_or_else(|e| panic!("{e}"));
    step_seconds(&mut app, 2.0);
    assert!(draft::offers(&root).is_empty(), "a clean buffer has nothing to draft");
    let file = draft::file_for(&root).expect("a data folder");
    assert!(!file.exists(), "with nothing unsaved there is no drafts file");
}

/// **Step `app` past `seconds` of its own clock**, the interval the draft is written on.
fn step_seconds(app: &mut App, seconds: f32) {
    // **`TimeUpdateStrategy::ManualDuration` moves `Time<Real>` too**, which is the clock the draft's interval reads, so a
    // test does not wait real seconds for a one-second timer.
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME)));
    let frames = (seconds / FRAME).ceil() as usize;
    for _ in 0..frames {
        app.update();
    }
}

// ── The interval save (#508, second half) ────────────────────────────────────

use bevy_dark_factory::autosave::{AutoSave, WARN_S};

/// **One interval past a placed piece writes the map, and the log records `map_save`** — one line, as `Cmd+S` does, not a
/// second write path.
#[test]
fn the_interval_saves_the_map_through_map_save_and_the_log_says_so() {
    let root = project("interval");
    let mut app = with_interval(&root, 2.0);
    make_the_map_dirty(&mut app);
    step_seconds(&mut app, 2.5);
    let open = app.world().resource::<OpenMap>();
    assert!(!open.dirty(), "the map was saved, so it is what the file holds");
    let log = bevy_dark_factory::ops::read_entries(&bevy_dark_factory::ops::oplog_path_at(&root));
    let saves = log.iter().filter(|e| e.op == "map_save").count();
    assert_eq!(saves, 1, "one `map_save` line for one interval save, as a Cmd+S makes: {log:?}");
    // **And the file on disk holds the piece**, not only the editor's memory of it.
    let text = std::fs::read_to_string(root.join("assets/blueprint/maps/m.map.yaml")).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains("an unsaved edit of the person's"), "the file holds the work: {text}");
}

/// **An unsaved buffer reaches disk on the same interval**, and a read-only buffer is skipped rather than refused.
#[test]
fn the_interval_saves_a_buffer_and_skips_a_read_only_one() {
    let root = project("interval_buffer");
    std::fs::write(root.join("notes.md"), "# on disk\n").unwrap_or_else(|e| panic!("{e}"));
    let mut app = with_interval(&root, 2.0);
    files::open_file(app.world_mut(), "notes.md").unwrap_or_else(|e| panic!("{e}"));
    {
        let mut files = app.world_mut().resource_mut::<Files>();
        let file = files.get_mut("notes.md").expect("open");
        file.text = "# typed in the buffer\n".to_owned();
        // **A read-only buffer is the case the issue names**: a refusal every interval is a toast nobody can dismiss.
        let agent = files.get_mut("agent.yaml");
        if let Some(agent) = agent {
            agent.text = "# typed where it must not save\n".to_owned();
        }
    }
    step_seconds(&mut app, 2.5);
    assert_eq!(std::fs::read_to_string(root.join("notes.md")).unwrap(), "# typed in the buffer\n", "the buffer reached disk on the interval");
    assert!(!app.world().resource::<Files>().is_dirty("notes.md"));
}

/// **The countdown does not fire while a question is up, while no project is open, or while the game plays.**
#[test]
fn the_timer_is_parked_while_a_question_is_up_and_when_no_project_is_open() {
    let root = project("parked");
    let mut app = with_interval(&root, 2.0);
    make_the_map_dirty(&mut app);
    // **A question is up**, so the countdown stands still and nothing is written.
    app.world_mut().resource_mut::<bevy_dark_factory::confirm::Confirm>().ask(
        bevy_dark_factory::confirm::Asked::LeaveMap,
        "Leave?",
        "",
        "Save and leave",
        "Stay",
    );
    step_seconds(&mut app, 3.0);
    assert!(app.world().resource::<OpenMap>().dirty(), "a question is up, so the timer did not save over the answer");
    let before = app.world().resource::<AutoSave>().remaining;
    step_seconds(&mut app, 1.0);
    assert_eq!(app.world().resource::<AutoSave>().remaining, before, "and the countdown is parked, not restarted");


    // **The question is answered, and the countdown runs again.** `Confirm::answer` only takes an answer a key or a button
    // settled, so the test settles it the way the dialog's own button does: the resource stays (the editor's systems require
    // it), and `N` would settle it; `answer` reads what a settle recorded.
    settle_the_question(&mut app);
    step_seconds(&mut app, 3.0);
    assert!(!app.world().resource::<OpenMap>().dirty(), "with nothing in the way the save lands");
}

/// **Settle whatever question is up the way its own answer key does**, without the key: a `Confirm` in a test has no dialog,
/// and `answer` only returns what a key or a button already settled.
fn settle_the_question(app: &mut App) {
    // **`N` refuses, and refusing is one of the three answers**: the question closes and `is_open` is false, which is the
    // state the countdown needs. What was answered does not matter to the timer, only that nothing is up.
    app.world_mut()
        .resource_mut::<bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>>()
        .press(bevy::input::keyboard::KeyCode::KeyN);
    app.update();
    app.world_mut()
        .resource_mut::<bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>>()
        .release(bevy::input::keyboard::KeyCode::KeyN);
    for _ in 0..3 {
        app.update();
    }
}

/// **The band says a number only in the last five seconds**, before and inside the window.
#[test]
fn the_band_says_auto_save_only_inside_the_last_five_seconds() {
    assert_eq!(bevy_dark_factory::autosave::band_text(30.0, true), "", "half a minute out is quiet");
    assert_eq!(bevy_dark_factory::autosave::band_text(WARN_S + 0.5, true), "", "just outside the window is quiet");
    assert_eq!(bevy_dark_factory::autosave::band_text(WARN_S, true), "Auto save in 5s", "the window's own edge speaks");
    assert_eq!(bevy_dark_factory::autosave::band_text(2.4, true), "Auto save in 3s", "rounded up, so the last second reads 1s not 0s");
    assert_eq!(bevy_dark_factory::autosave::band_text(2.4, false), "", "off says nothing at all");
}

/// **A dirty map, for a test**: a note the file does not hold, which is what an unsaved edit is.
fn make_the_map_dirty(app: &mut App) {
    let mut open = app.world_mut().resource_mut::<OpenMap>();
    open.map.note = Some("an unsaved edit of the person's".to_owned());
    assert!(open.dirty());
}
