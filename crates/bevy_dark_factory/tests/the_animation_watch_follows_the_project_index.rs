//! **The animation tab learns that a rig's file or its manifest changed from the project index, and not from a clock** (#501;
//! `anim_watch::POLL_SECS` and `RIGS_RECHECK_SECS` retired).
//!
//! - **A re-exported GLB is measured again**: the rig's report carries the file's fingerprint, so a changed file is seen as a changed
//!   fingerprint. The new bytes are the ground truth (the test copies a second, different GLB over the first and reads the library's
//!   fingerprint of the file it wrote).
//! - **An exporter mid-write is not measured**: the debounce the old watcher made of two equal polls is the index's. A file touched every
//!   20 ms for most of a second is not measured until the touching stops.
//! - **The manifest's own edits, and the tab's undo and redo, behave exactly as they did**: an edit made by hand is taken (the text it
//!   replaces is one step of the tab's history), `Cmd+Z` and `Cmd+Shift+Z` write the manifest and the watch does not take that write for
//!   an edit of someone else's, and an edit after an undo is a step of its own.
//!
//! The clock is frozen (a manual time step of nothing) and the index's sweep is at its floor, so a view that still waited for a second of
//! `Time` could not pass and only the index's own debounce stands between a write and the tab.

use std::time::{Duration, Instant, SystemTime};

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::{PrimaryWindow, Window};
use bevy_dark_factory::anim_tab::BenchState;
use bevy_dark_factory::anim_watch::BenchReports;
use bevy_dark_factory::project_index::{MIN_SWEEP, ProjectIndex};
use bevy_dark_factory::tiles::Mode;
use blueprint_model::rigs::Rigs;

use crate::fixtures;
use crate::settle_support::Frames;

/// The wall-clock pause between frames while waiting.
const PAUSE: Duration = Duration::from_millis(10);
/// How long a test waits for the tab to follow a write: frames, not seconds, covering at least 60 s at [`PAUSE`], so a
/// starved host does not run it out early (#644).
const WAIT: Frames = Frames::covering(Duration::from_secs(60), PAUSE);
/// How long a file is touched, and how often: the touching stops well inside the index's longest hold (2 s) and well outside its debounce
/// (100 ms), so a measure during it can only be a debounce that does not hold.
const TOUCHING_MILLIS: u64 = 700;
const TOUCH_EVERY_MILLIS: u64 = 20;
/// How long a test lets the watch run, to show that something it must not do is not done: at least 1.5 s at [`PAUSE`].
const QUIET: Frames = Frames::covering(Duration::from_millis(1500), PAUSE);
/// How long the watch runs over an unreadable manifest: at least 2 s at [`PAUSE`].
const UNREADABLE_FOR: Frames = Frames::covering(Duration::from_secs(2), PAUSE);

fn workspace_asset(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets").join(rel)
}

/// An editor on the Anim tab over the shipped rig manifest and two of its GLBs, with the clock frozen. Returns the project's root.
fn editor(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("anim_watch_{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    std::fs::copy(workspace_asset("blueprint/rigs.yaml"), root.join("assets/blueprint/rigs.yaml")).expect("rigs.yaml");
    std::fs::create_dir_all(root.join("assets/characters")).expect("characters dir");
    std::fs::copy(workspace_asset("characters/valkyrie.glb"), root.join("assets/characters/valkyrie.glb")).expect("valkyrie.glb");
    let mut app = bevy_dark_factory::harness::build_headless_at(&root, "m", None, Mode::Anim).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    assert!(app.world().resource::<BenchState>().loaded, "the tab loaded the manifest");
    app.world().resource::<ProjectIndex>().set_sweep(MIN_SWEEP);
    (app, root)
}

fn until(app: &mut App, what: &str, done: impl Fn(&App) -> bool) {
    for _ in 0..WAIT.0 {
        if done(app) {
            return;
        }
        app.update();
        std::thread::sleep(PAUSE);
    }
    panic!("never happened within {WAIT:?}: {what}");
}

fn fingerprint(app: &App, rig: &str) -> Option<u64> {
    app.world().resource::<BenchReports>().by_rig.get(rig).and_then(|r| r.fingerprint)
}

/// **The library's fingerprint of the file at `path`**, which a report carries.
fn fingerprint_of(path: &std::path::Path) -> u64 {
    blueprint_model::glb::Glb::open_fingerprinted(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())).1
}

/// The valkyrie measured, selected so it is the rig on screen.
fn with_the_valkyrie_measured(app: &mut App) -> u64 {
    let at = app.world().resource::<BenchState>().names().iter().position(|n| *n == "valkyrie").expect("valkyrie is a rig");
    app.world_mut().resource_mut::<BenchState>().selected = at;
    until(app, "the valkyrie measured", |a| fingerprint(a, "valkyrie").is_some());
    fingerprint(app, "valkyrie").expect("measured")
}

/// **A GLB re-exported under the tab is measured again**, and the report carries the new file's fingerprint.
#[test]
fn a_re_exported_glb_is_measured_again_with_the_clock_stopped() {
    let (mut app, root) = editor("re_export");
    let was = with_the_valkyrie_measured(&mut app);
    let glb = root.join("assets/characters/valkyrie.glb");
    assert_eq!(was, fingerprint_of(&glb), "the first measure is of the file on disk");

    // The exporter replaces the file with different bytes.
    std::fs::copy(workspace_asset("characters/fieldop.glb"), &glb).expect("a second glb");
    let now = fingerprint_of(&glb);
    assert_ne!(now, was, "the two files are different, so a measure of the new one must show it");
    until(&mut app, "the valkyrie measured again, as the new file", |a| fingerprint(a, "valkyrie") == Some(now));
}

/// **A file still being written is not measured until it settles**: the index's debounce, in place of two equal polls.
#[test]
fn a_glb_touched_every_twenty_milliseconds_is_measured_only_after_the_touching_stops() {
    let (mut app, root) = editor("mid_write");
    let was = with_the_valkyrie_measured(&mut app);
    let glb = root.join("assets/characters/valkyrie.glb");
    std::fs::copy(workspace_asset("characters/fieldop.glb"), &glb).expect("a second glb");
    let now = fingerprint_of(&glb);
    assert_ne!(now, was);

    // The exporter keeps the file open and moves its modification time, as one that is still writing does.
    let touching = Instant::now();
    while touching.elapsed() < Duration::from_millis(TOUCHING_MILLIS) {
        std::fs::File::options()
            .write(true)
            .open(&glb)
            .and_then(|f| f.set_modified(SystemTime::now()))
            .unwrap_or_else(|e| panic!("{}: {e}", glb.display()));
        app.update();
        assert_eq!(fingerprint(&app, "valkyrie"), Some(was), "the file was measured while it was still being touched");
        std::thread::sleep(Duration::from_millis(TOUCH_EVERY_MILLIS));
    }
    until(&mut app, "the file measured once the touching stopped", |a| fingerprint(a, "valkyrie") == Some(now));
}

fn send(app: &mut App, key: KeyCode, state: ButtonState) {
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

fn tap(app: &mut App, held: &[KeyCode], key: KeyCode) {
    for h in held {
        send(app, *h, ButtonState::Pressed);
    }
    send(app, key, ButtonState::Pressed);
    app.update();
    send(app, key, ButtonState::Released);
    for h in held {
        send(app, *h, ButtonState::Released);
    }
    app.update();
    app.update();
}

fn held(app: &App) -> String {
    app.world().resource::<BenchState>().text.clone().expect("the bench holds a manifest")
}

/// Run `frames` frames [`PAUSE`] apart, so the index has had its sweep and its debounce (#644).
fn run_for(app: &mut App, frames: Frames) {
    for _ in 0..frames.0 {
        app.update();
        std::thread::sleep(PAUSE);
    }
}

/// The manifest with one more rig, named `name`, copied from its first.
fn with_a_rig(text: &str, name: &str) -> String {
    let mut rigs = Rigs::parse(text).expect("the shipped manifest reads");
    let copy = rigs.rigs.values().next().expect("a rig").clone();
    rigs.rigs.insert(name.to_owned(), copy);
    rigs.to_yaml().expect("serialises")
}

/// **A hand's edit is one step of the tab's history; undo and redo write the manifest and the watch takes none of those writes for an edit.**
///
/// Every text below is one the test wrote or read back with `std::fs`; the tab must hold, and the file must say, exactly that.
#[test]
fn an_edit_an_undo_and_a_redo_leave_the_tab_and_the_file_agreeing_with_the_clock_stopped() {
    let (mut app, root) = editor("history");
    let path = root.join("assets/blueprint/rigs.yaml");
    let on_disk = || std::fs::read_to_string(&path).expect("rigs.yaml");
    let original = on_disk();
    assert_eq!(held(&app), original, "the tab read the manifest");
    let mods = [bevy_dark_factory::keys::MOD_KEYS[0]];

    // A hand adds a rig: the tab takes the edit, and the manifest it replaces is one step back.
    let first = with_a_rig(&original, "aardvark");
    std::fs::write(&path, &first).expect("write");
    until(&mut app, "the tab holds the first edit", |a| held(a) == first);

    // Cmd+Z puts the original back, on disk and in the tab, and the watch does not take that write for someone else's edit.
    tap(&mut app, &mods, KeyCode::KeyZ);
    assert_eq!((on_disk(), held(&app)), (original.clone(), original.clone()), "Cmd+Z wrote the manifest the edit replaced");
    run_for(&mut app, QUIET);
    assert_eq!((on_disk(), held(&app)), (original.clone(), original.clone()), "the undo's own write was not taken as an edit");

    // Cmd+Shift+Z puts the edit back.
    tap(&mut app, &[mods[0], KeyCode::ShiftLeft], KeyCode::KeyZ);
    assert_eq!((on_disk(), held(&app)), (first.clone(), first.clone()), "Cmd+Shift+Z put the edit back");
    run_for(&mut app, QUIET);
    assert_eq!((on_disk(), held(&app)), (first.clone(), first.clone()), "the redo's own write was not taken as an edit");

    // A second edit by hand is a step of its own: two undos walk back through both.
    let second = with_a_rig(&first, "zebra");
    std::fs::write(&path, &second).expect("write");
    until(&mut app, "the tab holds the second edit", |a| held(a) == second);
    tap(&mut app, &mods, KeyCode::KeyZ);
    assert_eq!((on_disk(), held(&app)), (first.clone(), first.clone()), "one undo back to the first edit");
    run_for(&mut app, QUIET);
    tap(&mut app, &mods, KeyCode::KeyZ);
    assert_eq!((on_disk(), held(&app)), (original.clone(), original.clone()), "a second undo back to the original");
    run_for(&mut app, QUIET);
    tap(&mut app, &[mods[0], KeyCode::ShiftLeft], KeyCode::KeyZ);
    tap(&mut app, &[mods[0], KeyCode::ShiftLeft], KeyCode::KeyZ);
    assert_eq!((on_disk(), held(&app)), (second.clone(), second.clone()), "two redos forward to the second edit");
    run_for(&mut app, QUIET);
    assert_eq!((on_disk(), held(&app)), (second.clone(), second.clone()), "and it stays");
}

/// **A manifest that cannot be read is said at the look the index asked for, and not on every frame; the next move of the index reads it
/// again** (#501). `rigs.yaml` is edited and made unreadable before the index publishes the edit, so the tab's look finds a file it cannot
/// open. Across two seconds of frames (the index's sweep is at its floor, so it looks at the file many times over) the warning is said once;
/// then the file is made readable and edited again, which moves the index's stamp, and the tab takes that edit. The frames run on this thread so
/// the warnings can be counted by a subscriber that is only set here. (Skipped as root, which can read anything.)
#[cfg(unix)]
#[test]
fn a_manifest_that_cannot_be_read_is_said_once_per_look_and_read_again_when_the_index_moves() {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::{Arc, Mutex};

    use bevy::log::tracing::{self, Event, Subscriber, field::{Field, Visit}};
    use bevy::log::tracing_subscriber::{Layer, Registry, layer::{Context, SubscriberExt}};

    /// Every message of every event, as text.
    struct Messages(Arc<Mutex<Vec<String>>>);
    struct Text(String);
    impl Visit for Text {
        fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
            if field.name() == "message" {
                self.0 = format!("{value:?}");
            }
        }
    }
    impl<S: Subscriber> Layer<S> for Messages {
        fn on_event(&self, event: &Event<'_>, _: Context<'_, S>) {
            let mut text = Text(String::new());
            event.record(&mut text);
            self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(text.0);
        }
    }

    let (mut app, root) = editor("unreadable");
    app.edit_schedule(Update, |schedule| {
        schedule.set_executor(bevy::ecs::schedule::SingleThreadedExecutor::new());
    });
    let path = root.join("assets/blueprint/rigs.yaml");
    let original = std::fs::read_to_string(&path).expect("rigs.yaml");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let subscriber = Registry::default().with(Messages(seen.clone()));
    let said = || seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).iter().filter(|m| m.contains("rigs.yaml") && m.contains("cannot be read")).count();
    tracing::subscriber::with_default(subscriber, || {
        let edited = with_a_rig(&original, "aardvark");
        std::fs::write(&path, &edited).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        if std::fs::read_to_string(&path).is_ok() {
            eprintln!("skipped: this user can read a file with no permissions");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
            return;
        }
        run_for(&mut app, UNREADABLE_FOR);
        assert_eq!(said(), 1, "the unreadable manifest is said once at the look, not on every frame: {:?}", seen.lock().map(|m| m.clone()));
        assert_eq!(held(&app), original, "the tab keeps the manifest it holds");

        // Readable again, and edited: the index's stamp moves, so the tab looks again and takes the edit.
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("chmod");
        let again = with_a_rig(&edited, "zebra");
        std::fs::write(&path, &again).expect("write");
        until(&mut app, "the tab takes the edit once the file reads", |a| held(a) == again);
    });
}
