//! **The everything-finder searches everything on its first query, and `>` still means commands** (#630).
//!
//! The 2026-11-25 review typed `big_source` in `Cmd`/`Ctrl`+`P` and read *"No command matches"*, and from that and the code it
//! inferred the first query was being judged against the command registry. **That inference is refuted here**, and this test says so
//! in the way the issue asks for: it measures what the list holds on the frame the palette opens, and a `File:` row of the project is
//! on it. The real cause is in `a_listed_source_file_opens_from_the_finder.rs` — the row for a `.rs` file was never built.
//!
//! What this test pins, end to end through the chord a person presses:
//!
//! 1. **The list written when the palette opens is the project's rows**, not `commands::all()`: a `goto:<kind>:<key>` row of a file
//!    the project holds is on it, and the registry's own `workspace.files` is not.
//! 2. **The first query is matched against those rows**, and the query the review typed lands the person on the file.
//! 3. **`>` still means commands** on the same chord: it lists the registry under `>` headings, and Enter runs the command.

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;

use bevy_dark_factory::editor::EditorState;
use bevy_dark_factory::files;
use bevy_dark_factory::goto::GotoIndex;
use bevy_dark_factory::harness;
use bevy_dark_factory::session::Session;
use bevy_dark_factory::workspace::Workspace;

use bevy_gooey::{CommandList, CommandPalette};

use crate::fixtures;
use crate::settle_support;

/// One frame of the test clock, seconds.
const FRAME: f32 = 1.0 / 60.0;

/// **The file the review typed the name of**: a source file of the project, under the folder its assets live in.
const SOURCE: &str = "assets/big_source.rs";

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("first_query_{tag}_{}", std::process::id()))
        .sized_descriptor("lamp_tall", "alpha", 0.4, 0.4)
        .place_as("lamp_tall@0", "lamp_tall", (2.0, 2.0))
        .side(64.0)
        .build("m");
    // **A name no command holds**, so "the list is the project's" cannot be answered from the registry by accident.
    let page = root.join(SOURCE);
    std::fs::create_dir_all(page.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&page, "// line 1 of a long file\nfn f_0(x: u32) -> u32 { x + 0 }\n").unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    // **The index is built by its own system**, so the test waits for the row rather than for a frame count.
    settle_support::until(&mut app, "the finder's index holds a row for the project's source file", |app| {
        app.world().resource::<GotoIndex>().entries().iter().any(|e| e.key == SOURCE)
    });
    app
}

fn palette_is_open(app: &mut App) -> bool {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<CommandPalette>>().iter(world).next().is_some()
}

fn send(app: &mut App, key_code: KeyCode, logical: Key, text: Option<&str>, state: ButtonState) {
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("one primary window");
    app.world_mut().write_message(KeyboardInput { key_code, logical_key: logical, state, text: text.map(Into::into), repeat: false, window });
}

fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

fn ctrl_p(app: &mut App) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in [primary(), KeyCode::KeyP] {
            send(app, key, Key::Unidentified(NativeKey::Unidentified), None, state);
        }
        app.update();
    }
    settle_support::settle(app);
}

fn key_for(c: char) -> KeyCode {
    // **Total over the alphabet**: a query this test types cannot fail for a letter the table forgot, which is how the first version
    // of this test broke — `big_source` needs a `c`.
    const LETTERS: [(char, KeyCode); 26] = [
        ('a', KeyCode::KeyA), ('b', KeyCode::KeyB), ('c', KeyCode::KeyC), ('d', KeyCode::KeyD),
        ('e', KeyCode::KeyE), ('f', KeyCode::KeyF), ('g', KeyCode::KeyG), ('h', KeyCode::KeyH),
        ('i', KeyCode::KeyI), ('j', KeyCode::KeyJ), ('k', KeyCode::KeyK), ('l', KeyCode::KeyL),
        ('m', KeyCode::KeyM), ('n', KeyCode::KeyN), ('o', KeyCode::KeyO), ('p', KeyCode::KeyP),
        ('q', KeyCode::KeyQ), ('r', KeyCode::KeyR), ('s', KeyCode::KeyS), ('t', KeyCode::KeyT),
        ('u', KeyCode::KeyU), ('v', KeyCode::KeyV), ('w', KeyCode::KeyW), ('x', KeyCode::KeyX),
        ('y', KeyCode::KeyY), ('z', KeyCode::KeyZ),
    ];
    match c {
        '.' | '>' => KeyCode::Period,
        '_' => KeyCode::Minus,
        _ => LETTERS
            .iter()
            .find(|(letter, _)| *letter == c)
            .map(|(_, key)| *key)
            .unwrap_or_else(|| panic!("no key for `{c}`")),
    }
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
    settle_support::settle(app);
}

fn enter(app: &mut App) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, KeyCode::Enter, Key::Enter, None, state);
        app.update();
    }
    settle_support::settle(app);
}

/// **The rows the open palette would list**, in its own order: what the person reads.
fn matched_rows(app: &mut App) -> Vec<String> {
    let list = app.world().resource::<CommandList>().0.clone();
    let world = app.world_mut();
    let Some(palette) = world.query::<&CommandPalette>().iter(world).next() else { return Vec::new() };
    palette.results().iter().filter_map(|&at| list.get(at)).map(|row| row.label()).collect()
}

#[test]
fn the_first_query_searches_the_project_and_a_greater_than_sign_means_commands() {
    let mut app = editor("first_query");

    // ── 1: `>` means commands, on the chord and in the context it is pressed in ─────────────────────────────────────────────
    // **Measured first, while the editor is on the page `edit.undo` is bound to.** `edit.undo` is `Context::Map`, and a command
    // list is judged by the context the palette opened from (`Finder::judged_in`), so pressing this half after the file half's
    // reveal would list it greyed "Not on this tab" — a real rule, and not the subject here.
    bevy_dark_factory::editor::run_for_test(app.world_mut(), "lamp_tall", (3.0, 3.0), (6.0, 3.0));
    settle_support::settle(&mut app);
    assert!(app.world().resource::<EditorState>().undo_depth() > 0, "a run is one undo step");

    ctrl_p(&mut app);
    assert!(palette_is_open(&mut app), "Cmd/Ctrl+P opens the one finder");
    type_str(&mut app, ">undo");
    let labels = matched_rows(&mut app);
    assert!(!labels.is_empty(), "`>` lists the command registry");
    assert!(
        labels.iter().all(|label| label.starts_with('>')),
        "every row of a `>` query is a command, under its own `>` heading: {labels:?}"
    );

    enter(&mut app);
    assert!(!palette_is_open(&mut app), "choosing a command closes the finder");
    settle_support::settle(&mut app);
    assert_eq!(
        app.world().resource::<EditorState>().redo_depth(),
        1,
        "`>` then the command's name ran `edit.undo` once: the `>` itself was not part of the query"
    );

    // ── 2: the first query is a file query, and the list it is judged against is the project's ──────────────────────────────
    assert_ne!(*app.world().resource::<Workspace>(), Workspace::Files, "the test does not start on the page the file is on");
    ctrl_p(&mut app);
    assert!(palette_is_open(&mut app), "the chord opens the finder again");

    // **The frame the palette opens**, before one character is typed: the list is the project's rows, not the registry's.
    let ids: Vec<String> = app.world().resource::<CommandList>().0.iter().map(|row| row.id.clone()).collect();
    assert!(
        ids.iter().any(|id| id.starts_with(bevy_dark_factory::goto::ID_PREFIX)),
        "the list is `goto`'s own rows (`goto:<kind>:<key>`), not the command registry's: {ids:?}"
    );
    assert!(
        !ids.iter().any(|id| id == "workspace.files"),
        "and it does not hold the registry's `workspace.files`, which is what a command list would: {ids:?}"
    );
    let labels = matched_rows(&mut app);
    assert!(
        labels.iter().any(|label| label.contains("big_source.rs")),
        "the project's own file is already a row the palette would show: {labels:?}"
    );

    // **The query the review typed**, as the first thing after the chord: `big_source` answered "No command matches" there.
    type_str(&mut app, "big_source");
    let labels = matched_rows(&mut app);
    assert!(
        labels.iter().any(|label| label.contains("big_source.rs")),
        "the first query narrows to the project's file rather than answering \"No command matches\": {labels:?}"
    );

    enter(&mut app);
    assert!(!palette_is_open(&mut app), "choosing a result closes the finder");
    settle_support::until(&mut app, "the file the query named is open in the Files workspace", |app| {
        *app.world().resource::<Workspace>() == Workspace::Files
            && files::active_file(app.world().resource::<Session>()).as_deref() == Some(SOURCE)
    });
}
