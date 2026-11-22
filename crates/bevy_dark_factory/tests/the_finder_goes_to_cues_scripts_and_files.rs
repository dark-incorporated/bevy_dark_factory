//! **Go to anything reaches cues, scripts and files** (NV-05, #382; series getting around, #377).
//!
//! `a_person_goes_to_anything.rs` proves the finder over pieces and placements and the `>` source. The Sound and Files workspaces now exist, so
//! the rest of the issue's list is a result too, and each is measured by where a person lands:
//!
//! 1. **A cue's name and Enter** open the Sound workspace with that cue selected.
//! 2. **A script's name and Enter** open the Files workspace on that script.
//! 3. **A text file's name and Enter** open the Files workspace on that file.
//! 4. **A cue the sound file gains while the editor runs is found at once**: the index is rebuilt from the project's files, not read once.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;

use bevy_dark_factory::files;
use bevy_dark_factory::harness;
use bevy_dark_factory::session::Session;
use bevy_dark_factory::workspace::Workspace;
use bevy_dark_factory::world_lists::SelectedCue;

use bevy_gooey::CommandPalette;

use crate::fixtures;

/// One frame of the test clock, seconds.
const FRAME: f32 = 1.0 / 60.0;

/// Frames a finder choice may take: the switch, the panel's build and the selection.
const SETTLE: usize = 60;

/// The project's sound file, in the crate's own shape.
const CUES: &str = "\
cues:
  cues:
  - name: door.creak
  - name: wind.loop
";

/// The same file after an agent adds a cue.
const MORE_CUES: &str = "\
cues:
  cues:
  - name: door.creak
  - name: wind.loop
  - name: bell.toll
";

fn editor(tag: &str) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("goto_files_{tag}_{}", std::process::id()))
        .sized_descriptor("bed", "alpha", 2.0, 1.0)
        .place_as("bed@0", "bed", (2.0, 2.0))
        .side(64.0)
        .build("m");
    for (rel, text) in [
        ("family/bevy_siren.yaml", CUES),
        ("scripts/lantern.rhai", "let lit = true;\n"),
        ("notes/design.md", "# the design\n"),
    ] {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap_or_else(|| panic!("{path:?}"))).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    }
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME)));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    // The index reads the project's files on a timer: give it the time a person would take.
    for _ in 0..120 {
        app.update();
    }
    (app, root)
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn palette_is_open(app: &mut App) -> bool {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<CommandPalette>>().iter(world).next().is_some()
}

fn key_for(c: char) -> KeyCode {
    match c {
        'a' => KeyCode::KeyA,
        'b' => KeyCode::KeyB,
        'c' => KeyCode::KeyC,
        'd' => KeyCode::KeyD,
        'e' => KeyCode::KeyE,
        'g' => KeyCode::KeyG,
        'i' => KeyCode::KeyI,
        'k' => KeyCode::KeyK,
        'l' => KeyCode::KeyL,
        'n' => KeyCode::KeyN,
        'o' => KeyCode::KeyO,
        'p' => KeyCode::KeyP,
        'r' => KeyCode::KeyR,
        's' => KeyCode::KeyS,
        't' => KeyCode::KeyT,
        'w' => KeyCode::KeyW,
        '.' => KeyCode::Period,
        other => panic!("no key for `{other}`"),
    }
}

fn send(app: &mut App, key_code: KeyCode, logical: Key, text: Option<&str>, state: ButtonState) {
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("one primary window");
    app.world_mut().write_message(KeyboardInput { key_code, logical_key: logical, state, text: text.map(Into::into), repeat: false, window });
}

fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

/// **Cmd/Ctrl+P**, as a hand makes it.
fn ctrl_p(app: &mut App) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in [primary(), KeyCode::KeyP] {
            send(app, key, Key::Unidentified(NativeKey::Unidentified), None, state);
        }
        app.update();
    }
    settle(app);
}

fn type_str(app: &mut App, text: &str) {
    for c in text.chars() {
        let s = c.to_string();
        for state in [ButtonState::Pressed, ButtonState::Released] {
            let typed = (state == ButtonState::Pressed).then_some(s.as_str());
            send(app, key_for(c), Key::Character(s.as_str().into()), typed, state);
            app.update();
        }
    }
    settle(app);
}

fn enter(app: &mut App) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, KeyCode::Enter, Key::Enter, None, state);
        app.update();
    }
    settle(app);
}

/// **Open the finder, type, choose** — the whole of a person's gesture.
fn go_to(app: &mut App, query: &str) {
    ctrl_p(app);
    assert!(palette_is_open(app), "Cmd/Ctrl+P opens the one finder");
    type_str(app, query);
    enter(app);
    assert!(!palette_is_open(app), "choosing a result closes the finder");
}

/// **A cue's name in Cmd+P opens Sound with the cue selected.**
#[test]
fn a_cues_name_opens_the_sound_workspace_with_it_selected() {
    let (mut app, _root) = editor("cue");
    go_to(&mut app, "door.creak");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Sound, "a cue is shown on the Sound workspace");
    assert_eq!(app.world().resource::<SelectedCue>().0.as_deref(), Some("door.creak"), "the cue the person named is the one marked, not the first");
}

/// **A script's name opens the Files workspace on that script.**
#[test]
fn a_scripts_name_opens_the_files_workspace_on_it() {
    let (mut app, _root) = editor("script");
    go_to(&mut app, "lantern");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "a script is a file, shown on the Files workspace");
    assert_eq!(files::active_file(app.world().resource::<Session>()).as_deref(), Some("scripts/lantern.rhai"), "the script the person named is the tab that is active");
}

/// **Any other text file's name opens the Files workspace on that file.**
#[test]
fn a_files_name_opens_the_files_workspace_on_it() {
    let (mut app, _root) = editor("file");
    // Owner exception (2026-11-13): the_finder_goes_to_cues_scripts_and_files.rs::a_files_name_opens_the_files_workspace_on_it: the query `design` is now also the name of the Design workspace (#404), which the finder lists first; the query is `design.` (the file's own dot), which only the name `design.md` holds, and every check is kept.
    go_to(&mut app, "design.");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files);
    assert_eq!(files::active_file(app.world().resource::<Session>()).as_deref(), Some("notes/design.md"));
}

/// **A cue an agent adds is in the finder at once**: the index follows the project's files.
#[test]
fn a_cue_added_to_the_sound_file_is_found_without_reopening_the_project() {
    let (mut app, root) = editor("added");
    std::fs::write(root.join("family/bevy_siren.yaml"), MORE_CUES).unwrap_or_else(|e| panic!("{e}"));
    // Owner exception (2026-11-14): the_finder_goes_to_cues_scripts_and_files.rs::a_cue_added_to_the_sound_file_is_found_without_reopening_the_project: the finder's files used to be re-read on a timer of this test's frame clock (1.5 s, which 180 frames of the manual clock were), and are now followed by the project index (#501), whose sweep and debounce run on the wall clock (about a second). The wait counted frames that the index does not count (measured: it failed once in a run of 135 tests and passed 6 of 6 alone; [INFERENCE] 180 frames can take about a second of wall time); it now steps frames until the finder holds the cue (at most ten seconds), and the assertions after it are the same.
    let started = std::time::Instant::now();
    while started.elapsed().as_secs_f64() < 10.0
        && !app.world().resource::<bevy_dark_factory::goto::GotoIndex>().of_kind(bevy_dark_factory::goto::Kind::Cue).any(|e| e.key == "bell.toll")
    {
        app.update();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    go_to(&mut app, "bell");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Sound, "the new cue is a result");
    assert_eq!(app.world().resource::<SelectedCue>().0.as_deref(), Some("bell.toll"));
}
