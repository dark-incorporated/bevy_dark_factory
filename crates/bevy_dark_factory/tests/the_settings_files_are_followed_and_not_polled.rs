//! **The settings and the keymap are followed, not polled: the two files they live in are watched by an index and a hand edit is seen**
//! (#501, checklist item 18: `settings::RECHECK_SECS`, `settings_screen::LOOK_EVERY`, `keymap_screen::LOOK_EVERY`).
//!
//! The person's `settings.yaml` lives in a folder outside the project, so the project's index cannot follow it: `OutsideFiles` follows
//! a folder for the names it is asked for. The project's own `editor.yaml` is followed by the project's index. Neither reader has a timer
//! or a `stat` of its own any more, so each test below fails if its reader stops listening to the index:
//!
//! - **a hand edit is seen** (the plain `std::fs` write a person's editor makes) by `sync_settings`, the settings screen and the keymap
//!   screen, and what they show is compared with what an independent read of the file says;
//! - **a frame is not a poll**: with the sweep set to a minute and no one told, a hand edit is not seen by a frame that looks;
//! - **the editor's own write is in the next frame**, without a sweep: `edit_layer` tells the indexes the write is done;
//! - **absent is not unreadable**: a person who never saved a setting has no settings folder and is shown nothing wrong; a folder that
//!   cannot be looked at is a refusal that names the file and ends when it can be looked at again;
//! - **an outside folder is followed for its names only**, a link to the file is followed, and `note_written` and `listed` stay the one
//!   path they were.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_dark_factory::live::EditorReloads;
use bevy_dark_factory::project::Project;
use bevy_dark_factory::project_index::{ChangeKind, OutsideFileChanged, OutsideFiles, ProjectIndex, note_path_written, note_written};
use bevy_dark_factory::settings::{EditorSettings, UserSettingsFile, file_reads_on_this_thread};
use bevy_dark_factory::settings_screen::OpenSettings;
use bevy_gooey::testing::find_by_label;
use bevy_gooey::{Modifiers, Shortcut};

use crate::fixtures;

/// The longest a test waits on the wall clock for something an index must publish: a ceiling, never the measurement.
const WAIT_S: f64 = 10.0;

/// **A sweep short enough that a hand edit is found at once**; the debounce still holds the change until it has been steady.
const FAST_SWEEP: Duration = Duration::from_millis(50);

/// **A sweep so long that only a note can reveal a change inside a test.**
const NO_SWEEP: Duration = Duration::from_secs(60);

/// What a hand edit must be seen within, measured from the write: a sweep, the debounce and the thread's tick with a loaded machine's
/// room. It is not derived from those constants; it is the number the owner's "within a second or so" allows.
const HAND_EDIT_SEEN_WITHIN: Duration = Duration::from_millis(1500);

/// How long a frame that looks at nothing is given to see a change that no one told of: longer than a hand edit's budget, so a poll
/// at any rhythm the old code had (a quarter of a second, a second) would have shown it.
const NOT_POLLED_FOR: Duration = Duration::from_millis(1800);

/// The ceiling on a hand edit at the default sweep (one second between sweeps, plus the debounce, with a loaded machine's room).
const DEFAULT_SWEEP_HAND_EDIT_WITHIN: Duration = Duration::from_millis(2500);

struct Rig {
    app: App,
    dir: PathBuf,
    user: PathBuf,
    project: PathBuf,
}

/// **A stepped editor whose person's folder is outside the project** (a sibling of its root, so the project's index cannot see it).
fn rig(tag: &str) -> Rig {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let dir = root.parent().expect("a fixture root has a parent").join(format!("blueprint-outside-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let user = dir.join("settings.yaml");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user.clone()));
    for _ in 0..3 {
        app.update();
    }
    let project = app.world().resource::<Project>().project_dir.join("editor.yaml");
    Rig { app, dir, user, project }
}

impl Rig {
    fn set_sweeps(&self, every: Duration) {
        self.app.world().resource::<OutsideFiles>().set_sweep(&self.dir, every);
        self.app.world().resource::<ProjectIndex>().set_sweep(every);
    }

    /// Step frames until `done`, for at most `WAIT_S` on the wall clock; how long it took.
    fn until(&mut self, what: &str, done: impl FnMut(&mut App) -> bool) -> Duration {
        let began = Instant::now();
        assert!(pump(&mut self.app, WAIT_S, done), "never saw: {what}; settings {:?}", self.settings());
        began.elapsed()
    }

    fn settings(&self) -> EditorSettings {
        self.app.world().resource::<EditorSettings>().clone()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&self.dir, std::fs::Permissions::from_mode(0o755));
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

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

fn text_scale(app: &App) -> f32 {
    app.world().resource::<EditorSettings>().text_scale
}

/// **The independent ground truth**: what a plain read of the file says the text scale is, with the editor's parser nowhere in it.
fn text_scale_on_disk(path: &Path) -> f32 {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let after = text.split("text_scale:").nth(1).unwrap_or_else(|| panic!("no text_scale in {text:?}"));
    after.trim_start().split([',', '}', '\n']).next().unwrap_or("").trim().parse().unwrap_or_else(|e| panic!("{after:?}: {e}"))
}

fn verdicts(app: &App) -> Vec<(String, String, String)> {
    app.world()
        .resource::<EditorReloads>()
        .kinds()
        .iter()
        .filter(|k| k.kind == "editor_settings")
        .map(|k| (k.file.clone(), k.outcome.word().to_owned(), k.reason.clone()))
        .collect()
}

// ── A hand edit ──────────────────────────────────────────────────────────────

/// **A hand edit of the person's file is seen, and it is what the file says.** The sweep is short so the sweep finds it; the old
/// code's once-a-second text read is gone, so the only way the editor learns is through the index.
#[test]
fn a_hand_edit_of_the_users_settings_file_is_seen_by_the_editor_and_matches_the_file() {
    let mut r = rig("outside_hand_edit");
    std::fs::create_dir_all(&r.dir).expect("the person's folder");
    r.set_sweeps(FAST_SWEEP);
    assert!(r.app.world().resource::<OutsideFiles>().wait_ready(&r.dir, Duration::from_secs(10)), "the folder's first listing is in");
    assert_eq!(text_scale(&r.app), 1.0, "no file: the default");

    let wrote = Instant::now();
    std::fs::write(&r.user, "{version: 1, text_scale: 1.75}").expect("a hand edit");
    let seen = r.until("the hand-edited text size", |a| text_scale(a) == 1.75);
    eprintln!("hand edit of the user's file seen after {seen:?} (wall clock from the write {:?})", wrote.elapsed());
    assert!(seen < HAND_EDIT_SEEN_WITHIN, "a hand edit took {seen:?} to be seen");
    assert_eq!(text_scale(&r.app), text_scale_on_disk(&r.user), "the editor shows what the file says");

    // A second edit is a Modified event; a delete is a Removed one that falls back to the default at once.
    std::fs::write(&r.user, "{version: 1, text_scale: 1.25}").expect("a second hand edit");
    r.until("the second edit", |a| text_scale(a) == 1.25);
    assert_eq!(text_scale(&r.app), text_scale_on_disk(&r.user));
    std::fs::remove_file(&r.user).expect("a delete");
    r.until("the file's removal", |a| text_scale(a) == 1.0);
    assert!(verdicts(&r.app).iter().all(|v| v.1 == "applied"), "{:?}", verdicts(&r.app));
}

/// **A frame does not look at the file.** With the sweep a minute long and no one told, an edit is not seen by any number of frames in
/// the time the old poll (a quarter of a second, a second) would have shown it; a note then shows it at once.
#[test]
fn a_frame_that_looks_at_nothing_does_not_see_an_edit_no_one_told_of() {
    let mut r = rig("outside_no_poll");
    std::fs::create_dir_all(&r.dir).expect("the person's folder");
    r.set_sweeps(NO_SWEEP);
    assert!(r.app.world().resource::<OutsideFiles>().wait_ready(&r.dir, Duration::from_secs(10)));
    std::fs::write(&r.user, "{version: 1, text_scale: 2.0}").expect("a hand edit");
    assert!(!pump(&mut r.app, NOT_POLLED_FOR.as_secs_f64(), |a| text_scale(a) == 2.0), "a frame saw an edit that no sweep and no note had revealed: that is a poll");
    note_path_written(&r.user);
    r.app.update();
    assert_eq!(text_scale(&r.app), 2.0, "told, the next frame shows it");
}

/// **The editor's own write is in the next frame**, with the sweep out of the picture: `edit_layer` tells the indexes the write is done
/// and they have published before it returns.
#[test]
fn the_editors_own_write_is_seen_in_the_next_frame_without_a_sweep() {
    let mut r = rig("outside_own_write");
    r.set_sweeps(NO_SWEEP);
    assert!(r.app.world().resource::<OutsideFiles>().wait_ready(&r.dir, Duration::from_secs(10)));
    bevy_dark_factory::setting_catalogue::edit_layer(&r.user, |layer| {
        layer.text_scale = Some(1.5);
        Ok(())
    })
    .unwrap_or_else(|e| panic!("{e}"));
    r.app.update();
    assert_eq!(text_scale(&r.app), 1.5, "the next frame follows the write");
    assert_eq!(text_scale(&r.app), text_scale_on_disk(&r.user));

    // The project's file is told the same way: its index follows the project's root.
    bevy_dark_factory::setting_catalogue::edit_layer(&r.project, |layer| {
        layer.text_scale = Some(2.5);
        Ok(())
    })
    .unwrap_or_else(|e| panic!("{e}"));
    // **The project's tree is told without a wait** (its thread may be sweeping): it publishes within a moment, not in the same frame,
    // and with the sweep a minute away only the note can have shown it.
    r.until("the project's own write, with no sweep", |a| text_scale(a) == 2.5);
}

/// **The project's layer is followed by the project's index**, with the same independent check.
#[test]
fn a_hand_edit_of_the_projects_settings_file_is_seen_by_the_editor() {
    let mut r = rig("outside_project_layer");
    r.set_sweeps(FAST_SWEEP);
    assert!(r.app.world().resource::<ProjectIndex>().wait_ready(Duration::from_secs(10)));
    std::fs::write(&r.project, "{version: 1, text_scale: 2.0}").expect("a hand edit");
    let seen = r.until("the project's hand-edited text size", |a| text_scale(a) == 2.0);
    eprintln!("hand edit of the project's file seen after {seen:?}");
    assert!(seen < HAND_EDIT_SEEN_WITHIN, "a hand edit took {seen:?} to be seen");
    assert_eq!(text_scale(&r.app), text_scale_on_disk(&r.project));
    std::fs::remove_file(&r.project).expect("a delete");
    r.until("the project layer's removal", |a| text_scale(a) == 1.0);
}

// ── Absent and unreadable are two things ─────────────────────────────────────

/// **A person who never saved a setting has no settings folder, and is shown nothing wrong.** Absent is a quiet empty listing; the file
/// is seen when it comes, folder and all.
#[test]
fn a_person_who_never_saved_a_setting_is_shown_no_error_and_the_file_is_seen_when_it_comes() {
    let mut r = rig("outside_absent");
    assert!(!r.dir.exists(), "the person's folder does not exist yet");
    r.set_sweeps(FAST_SWEEP);
    assert!(r.app.world().resource::<OutsideFiles>().wait_ready(&r.dir, Duration::from_secs(10)), "an absent folder still has a first listing");
    pump(&mut r.app, 0.5, |_| false);
    {
        let outside = r.app.world().resource::<OutsideFiles>();
        assert_eq!(outside.failure(&r.dir), None, "an absent folder is not a failure");
        assert!(outside.snapshot(&r.dir).expect("a listing").files.is_empty(), "nothing is listed");
    }
    assert_eq!(r.settings(), EditorSettings::default());
    assert!(verdicts(&r.app).is_empty(), "no file, nothing to refuse: {:?}", verdicts(&r.app));

    std::fs::create_dir_all(&r.dir).expect("the folder comes");
    std::fs::write(&r.user, "{version: 1, text_scale: 1.5}").expect("the file comes");
    r.until("the file of a folder that did not exist", |a| text_scale(a) == 1.5);
    let listed: Vec<String> = r.app.world().resource::<OutsideFiles>().snapshot(&r.dir).expect("a listing").files.iter().map(|(n, _)| n.clone()).collect();
    assert_eq!(listed, vec!["settings.yaml".to_owned()]);
}

/// **A folder that cannot be looked at is a refusal, not an absent file**: the file is named, the last good settings stay, and the
/// refusal ends when the folder can be looked at again.
///
/// **This test needs a process that cannot look inside a folder with no permissions**; run as root it returns after taking the folder
/// away, having checked nothing past that, and says so on stderr.
#[cfg(unix)]
#[test]
fn a_folder_that_cannot_be_looked_at_is_a_refusal_naming_the_file_and_it_ends_when_it_can_be() {
    use std::os::unix::fs::PermissionsExt;
    let mut r = rig("outside_unreadable");
    std::fs::create_dir_all(&r.dir).expect("the person's folder");
    std::fs::write(&r.user, "{version: 1, text_scale: 1.5}").expect("a settings file");
    r.set_sweeps(FAST_SWEEP);
    r.until("the file as written", |a| text_scale(a) == 1.5);

    std::fs::set_permissions(&r.dir, std::fs::Permissions::from_mode(0o000)).expect("take the folder away");
    if std::fs::metadata(&r.user).is_ok() {
        eprintln!("skipped the refusal half: this process can look inside a folder with no permissions (it is root)");
        return;
    }
    let dir = r.dir.clone();
    r.until("the failure", |a| a.world().resource::<OutsideFiles>().failure(&dir).is_some());
    let said = r.app.world().resource::<OutsideFiles>().failure(&r.dir).expect("a failure");
    assert!(said.contains("settings.yaml"), "the failure names the file: {said}");
    r.until("a refusal in the reloads", |a| verdicts(a).iter().any(|v| v.1 == "refused"));
    let refused = verdicts(&r.app).into_iter().find(|v| v.1 == "refused").expect("a refusal");
    assert!(refused.0.contains("settings.yaml"), "the refusal names the file: {refused:?}");
    assert_eq!(text_scale(&r.app), 1.5, "the last good settings stay");

    std::fs::set_permissions(&r.dir, std::fs::Permissions::from_mode(0o755)).expect("give the folder back");
    r.until("the failure to end", |a| a.world().resource::<OutsideFiles>().failure(&dir).is_none());
    r.until("the file to apply again", |a| verdicts(a).last().is_some_and(|v| v.1 == "applied"));
    std::fs::write(&r.user, "{version: 1, text_scale: 1.25}").expect("an edit after");
    r.until("an edit after the failure", |a| text_scale(a) == 1.25);
}

// ── The two screens ──────────────────────────────────────────────────────────

/// **The settings screen shows a hand edit made while it is open.** `show_grid: false` in the file makes the Grid lines row *Modified*,
/// which is a Reset button on screen; the file is read by the test, with no editor code in it, to say the key is there.
#[test]
fn the_settings_screen_shows_a_hand_edit_made_while_it_is_open() {
    let mut r = rig("outside_settings_screen");
    std::fs::create_dir_all(&r.dir).expect("the person's folder");
    r.set_sweeps(FAST_SWEEP);
    assert!(r.app.world().resource::<OutsideFiles>().wait_ready(&r.dir, Duration::from_secs(10)));
    r.app.world_mut().write_message(OpenSettings);
    for _ in 0..4 {
        r.app.update();
    }
    assert!(find_by_label(&mut r.app, "Grid lines").is_ok(), "the screen is open");
    assert!(find_by_label(&mut r.app, "Reset Grid lines").is_err(), "nothing is modified yet");

    std::fs::write(&r.user, "{version: 1, show_grid: false}").expect("a hand edit");
    assert!(std::fs::read_to_string(&r.user).expect("an independent read").contains("show_grid: false"));
    let seen = r.until("the Grid lines row to become modified", |a| find_by_label(a, "Reset Grid lines").is_ok());
    eprintln!("hand edit shown by the settings screen after {seen:?}");
    assert!(seen < HAND_EDIT_SEEN_WITHIN, "the screen took {seen:?}");

    std::fs::write(&r.user, "{version: 1}").expect("the key taken out by hand");
    r.until("the row to be unmodified again", |a| find_by_label(a, "Reset Grid lines").is_err());
}

/// **The keymap screen shows a rebinding made by hand while it is open**, and the chord it shows is the one an independent parse of
/// the file reads.
#[test]
fn the_keymap_screen_shows_a_rebinding_made_by_hand_while_it_is_open() {
    let mut r = rig("outside_keymap_screen");
    std::fs::create_dir_all(&r.dir).expect("the person's folder");
    r.set_sweeps(FAST_SWEEP);
    assert!(r.app.world().resource::<OutsideFiles>().wait_ready(&r.dir, Duration::from_secs(10)));
    r.app.world_mut().write_message(bevy_dark_factory::keymap_screen::OpenKeymap);
    for _ in 0..4 {
        r.app.update();
    }
    let narrow = r.app.world_mut().query_filtered::<Entity, With<bevy_gooey::TextHint>>().iter(r.app.world()).next().expect("the search row");
    r.app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange { source: narrow, value: "redo".to_owned(), is_final: false });
    for _ in 0..3 {
        r.app.update();
    }
    let row = find_by_label(&mut r.app, "Redo").unwrap_or_else(|e| panic!("{e}"));
    let before = bevy_dark_factory::keymap_screen::shown(r.app.world(), row).expect("a chord row");

    // **The ground truth is a plain read of the file and a chord built here**, not the editor's parser: the text says Ctrl+Alt+J.
    let text = "{version: 1, keymap: {\"edit.redo\": {modifiers: {ctrl: true, shift: false, alt: true, meta: false}, key: KeyJ}}}";
    std::fs::write(&r.user, text).expect("a hand edit");
    let on_disk = std::fs::read_to_string(&r.user).expect("an independent read");
    assert!(on_disk.contains("ctrl: true") && on_disk.contains("alt: true") && on_disk.contains("key: KeyJ"), "{on_disk}");
    let truth = Shortcut::new(Modifiers::CTRL | Modifiers::ALT, KeyCode::KeyJ);
    assert_ne!(before, truth, "the hand edit changes the chord");
    let seen = r.until("the rebinding on the keymap screen", |a| {
        find_by_label(a, "Redo").ok().and_then(|row| bevy_dark_factory::keymap_screen::shown(a.world(), row)) == Some(truth)
    });
    eprintln!("hand edit shown by the keymap screen after {seen:?}");
    assert!(seen < HAND_EDIT_SEEN_WITHIN, "the screen took {seen:?}");
}

// ── Whose write it is, and how long a hand edit takes ────────────────────────

/// **Two saves of one length inside one clock tick are two writes.** The second save has the first's size and (put back by the test, as
/// a coarse clock leaves it) the first's modification time, so the stamp did not move; the writer says it wrote, and the editor takes it.
#[test]
fn two_saves_of_one_length_in_one_clock_tick_are_both_taken() {
    let mut r = rig("outside_same_length");
    r.set_sweeps(NO_SWEEP);
    assert!(r.app.world().resource::<OutsideFiles>().wait_ready(&r.dir, Duration::from_secs(10)));
    bevy_dark_factory::setting_catalogue::edit_layer(&r.user, |layer| {
        layer.text_scale = Some(1.25);
        Ok(())
    })
    .unwrap_or_else(|e| panic!("{e}"));
    r.app.update();
    assert_eq!(text_scale(&r.app), 1.25);
    let first = std::fs::metadata(&r.user).expect("the first save");

    let text = std::fs::read_to_string(&r.user).expect("a plain read").replace("1.25", "1.75");
    std::fs::write(&r.user, &text).expect("the second save, the same length");
    std::fs::OpenOptions::new().write(true).open(&r.user).and_then(|f| f.set_modified(first.modified().expect("a modification time"))).expect("the clock tick put back");
    let second = std::fs::metadata(&r.user).expect("the second save");
    assert_eq!((second.len(), second.modified().ok()), (first.len(), first.modified().ok()), "the stamp did not move: that is the case");

    note_path_written(&r.user);
    r.app.update();
    assert_eq!(text_scale(&r.app), 1.75, "a noted write whose size and time did not move is still taken");
    assert_eq!(text_scale(&r.app), text_scale_on_disk(&r.user));
}

/// **A frame that runs on this thread, so the per-thread read counter sees every read.**
fn single_threaded(app: &mut App) {
    app.edit_schedule(PreUpdate, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    app.edit_schedule(Update, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
}

/// **Only the file that moved is read again.** A file in the project and a file beside the person's settings that are not settings
/// files change and nothing is read; the person's file changes and it is read by `sync_settings` and by the open screen once each, and
/// the project's file is not read again by either; the same the other way round. Counted by the readers' own seam
/// (`file_reads_on_this_thread`), so a subscriber that takes any message for its own fails here.
#[test]
fn a_file_that_no_one_changed_is_not_read_and_only_the_file_that_moved_is_read_again() {
    let mut r = rig("outside_own_file_only");
    single_threaded(&mut r.app);
    std::fs::create_dir_all(&r.dir).expect("the person's folder");
    std::fs::write(&r.user, "{version: 1}").expect("the person's file");
    std::fs::write(&r.project, "{version: 1}").expect("the project's file");
    r.set_sweeps(FAST_SWEEP);
    assert!(r.app.world().resource::<OutsideFiles>().wait_ready(&r.dir, Duration::from_secs(10)));
    assert!(r.app.world().resource::<ProjectIndex>().wait_ready(Duration::from_secs(10)));
    r.app.world_mut().write_message(OpenSettings);
    for _ in 0..4 {
        r.app.update();
    }
    pump(&mut r.app, 0.8, |_| false);
    let base = file_reads_on_this_thread();

    // Files that are not settings files: one in the project, one beside the person's file.
    let unrelated_in_project = r.project.with_file_name("unrelated.yaml");
    std::fs::write(&unrelated_in_project, "a: 1\n").expect("an unrelated project file");
    std::fs::write(r.dir.join("unrelated.yaml"), "a: 1\n").expect("an unrelated file beside the person's");
    pump(&mut r.app, 0.8, |_| false);
    assert_eq!(file_reads_on_this_thread(), base, "an unrelated file moved and a settings file was read");

    std::fs::write(&r.user, "{version: 1, text_scale: 1.25}").expect("a hand edit of the person's file");
    r.until("the person's edit", |a| text_scale(a) == 1.25);
    pump(&mut r.app, 0.5, |_| false);
    assert_eq!(file_reads_on_this_thread() - base, 2, "the person's file is read by `sync_settings` and by the open screen, and the project's file by neither");

    let base = file_reads_on_this_thread();
    std::fs::write(&r.project, "{version: 1, text_scale: 2.0}").expect("a hand edit of the project's file");
    r.until("the project's edit", |a| text_scale(a) == 2.0);
    pump(&mut r.app, 0.5, |_| false);
    assert_eq!(file_reads_on_this_thread() - base, 2, "the project's file is read by `sync_settings` and by the open screen, and the person's file by neither");
}

/// **At the default sweep a hand edit is seen within a sweep and the debounce**, not within a frame as it was when the files were
/// `stat`-ed every frame: the number is measured and printed, and the ceiling is what the guide and the changelog say.
#[test]
fn at_the_default_sweep_a_hand_edit_takes_up_to_about_a_second_and_a_tenth() {
    let mut r = rig("outside_default_sweep");
    std::fs::create_dir_all(&r.dir).expect("the person's folder");
    assert!(r.app.world().resource::<OutsideFiles>().wait_ready(&r.dir, Duration::from_secs(10)));
    let mut longest = Duration::ZERO;
    for (n, scale) in ["1.25", "1.75", "2.25"].into_iter().enumerate() {
        pump(&mut r.app, 0.3 * (n as f64 + 1.0), |_| false);
        std::fs::write(&r.user, format!("{{version: 1, text_scale: {scale}}}")).expect("a hand edit");
        let seen = r.until("a hand edit at the default sweep", |a| text_scale(a) == scale.parse::<f32>().expect("a number"));
        longest = longest.max(seen);
        eprintln!("hand edit {scale} at the default sweep seen after {seen:?}");
    }
    assert!(longest < DEFAULT_SWEEP_HAND_EDIT_WITHIN, "a hand edit at the default sweep took {longest:?}");
}

// ── The index, without an editor ─────────────────────────────────────────────

/// A bare app with the outside index only.
fn bare() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    bevy_dark_factory::project_index::ensure_outside(&mut app);
    app.init_resource::<Heard>().add_systems(Update, hear);
    app
}

#[derive(Resource, Default)]
struct Heard(Vec<OutsideFileChanged>);

fn hear(mut read: MessageReader<OutsideFileChanged>, mut heard: ResMut<Heard>) {
    heard.0.extend(read.read().cloned());
}

fn heard_of(app: &App, name: &str) -> Vec<(ChangeKind, bool)> {
    app.world().resource::<Heard>().0.iter().filter(|e| e.rel == name).map(|e| (e.kind, e.dir)).collect()
}

/// **A folder is followed for its names and nothing else in it**: two thousand other files in it are neither listed nor heard, a note
/// reaches the index (the same `note_written` the project's operations call) and a link to the file is followed.
#[test]
fn an_outside_folder_is_followed_for_its_names_only_and_note_written_reaches_it() {
    let dir = std::env::temp_dir().join(format!("blueprint-outside-names-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a folder");
    const OTHERS: usize = 2000;
    for n in 0..OTHERS {
        std::fs::write(dir.join(format!("other_{n}.txt")), "x").expect("an unrelated file");
    }
    std::fs::write(dir.join("turns.jsonl"), "one\n").expect("the named file");
    let mut app = bare();
    app.update();
    app.world_mut().resource_mut::<OutsideFiles>().follow(&dir, &["turns.jsonl", "settings.yaml"]);
    app.world().resource::<OutsideFiles>().set_sweep(&dir, NO_SWEEP);
    assert!(app.world().resource::<OutsideFiles>().wait_ready(&dir, Duration::from_secs(10)));
    let listed: Vec<String> = app.world().resource::<OutsideFiles>().snapshot(&dir).expect("a listing").files.iter().map(|(n, _)| n.clone()).collect();
    assert_eq!(listed, vec!["turns.jsonl".to_owned()], "only the names asked for, and only those that exist, though {OTHERS} others are there");

    // An unrelated file changing is not heard, even when a note names it: the index follows only its names.
    std::fs::write(dir.join("other_7.txt"), "changed").expect("an unrelated write");
    note_written(&dir, "other_7.txt");
    pump(&mut app, 0.4, |_| false);
    assert!(app.world().resource::<Heard>().0.is_empty(), "an unlisted file was heard: {:?}", app.world().resource::<Heard>().0);

    // A note (the sweep is a minute away) shows the append of the named file, with its length.
    std::fs::write(dir.join("turns.jsonl"), "one\ntwo\n").expect("an append");
    note_written(&dir, "turns.jsonl");
    assert!(pump(&mut app, WAIT_S, |a| !heard_of(a, "turns.jsonl").is_empty()), "the note was never published");
    let event = app.world().resource::<Heard>().0.iter().find(|e| e.rel == "turns.jsonl").expect("an event").clone();
    assert_eq!((event.kind, event.dir, event.root.as_path()), (ChangeKind::Modified, false, dir.as_path()));
    assert_eq!(event.stamp.map(|s| s.len), Some(std::fs::metadata(dir.join("turns.jsonl")).expect("an independent stat").len()));

    // A second name appearing by hand is a Created event once a sweep looks (here a note tells it, as the sweep is a minute away).
    std::fs::write(dir.join("settings.yaml"), "{version: 1}").expect("the second name");
    note_path_written(&dir.join("settings.yaml"));
    app.update();
    assert_eq!(heard_of(&app, "settings.yaml"), vec![(ChangeKind::Created, false)], "told of a write, the index published it before the call returned");

    // `listed` is the project tree's answer and stays an error for a folder no project index follows: there is no second listing path.
    assert!(bevy_dark_factory::files::listed_files(&dir).is_err(), "an outside folder is not a project and is not listed as one");

    #[cfg(unix)]
    {
        let target = dir.join("real_settings.yaml");
        std::fs::write(&target, "{version: 1}").expect("a link's target");
        app.world_mut().resource_mut::<OutsideFiles>().unfollow(&dir);
        std::os::unix::fs::symlink(&target, dir.join("linked.yaml")).expect("a link");
        app.world_mut().resource_mut::<OutsideFiles>().follow(&dir, &["linked.yaml"]);
        assert!(app.world().resource::<OutsideFiles>().wait_ready(&dir, Duration::from_secs(10)));
        let linked = app.world().resource::<OutsideFiles>().snapshot(&dir).expect("a listing").files.clone();
        assert_eq!(linked.len(), 1, "a link to a settings file is followed: {linked:?}");
        assert_eq!(linked[0].1.len, std::fs::metadata(&target).expect("the target's own stat").len(), "with the stamp of what it points to");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
