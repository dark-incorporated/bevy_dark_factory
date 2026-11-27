//! **A source file the finder lists opens when it is chosen** (#630), and its path is named in `blueprint_state.file_outline`.
//!
//! **The measured failure this pins.** The 2026-11-25 review's project held `assets/big_source.rs` — 2,200 lines, the file five of its
//! six timed interactions were blocked on — and `Cmd`/`Ctrl`+`P`, `big_source`, Enter left `file_outline` at
//! `{"file": null, "headings": []}` on three consecutive attempts. The palette was not answering as commands (that is
//! `the_first_finder_query_searches_everything.rs`'s subject); there was simply **no row to choose**, because the finder's index builds
//! its rows from a text-file test that knew only the extensions the editor colours (`crosslinks::is_text_file`). A `.rs` file was not
//! text to the index, so a query naming it matched nothing and Enter ran nothing.
//!
//! Measured the way a person did it — the chord, the file's name, Enter — and then **the state an agent reads**, which is where the
//! review saw the null. A path that is a row but that the editor cannot open is the same null, so both halves are one assertion here:
//! the row exists, and choosing it puts the path in `file_outline.file` and takes the tab.

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::window::PrimaryWindow;

use bevy_dark_factory::file_outline;
use bevy_dark_factory::files::{self, Files};
use bevy_dark_factory::goto::GotoIndex;
use bevy_dark_factory::harness;
use bevy_dark_factory::session::Session;
use bevy_dark_factory::workspace::Workspace;
use bevy_gooey::{CommandList, CommandPalette};
use serde_json::json;

use crate::fixtures;
use crate::settle_support;

/// One frame of the test clock, seconds.
const FRAME: f32 = 1.0 / 60.0;

/// **The file the review could not open**: a source file of the project, under the folder the project's assets live in.
const SOURCE: &str = "assets/big_source.rs";

/// What it holds, so the text the editor lands on is recognisably this file's.
const SOURCE_TEXT: &str = "// line 1 of a long file\nfn f_0(x: u32) -> u32 { x + 0 }\n";

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("source_row_{tag}_{}", std::process::id()))
        .sized_descriptor("lamp_tall", "alpha", 0.4, 0.4)
        .place_as("lamp_tall@0", "lamp_tall", (2.0, 2.0))
        .side(64.0)
        .build("m");
    let path = root.join(SOURCE);
    std::fs::create_dir_all(path.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&path, SOURCE_TEXT).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME)));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **`blueprint_state.file_outline`**, read from the resources it is built from — the same function the agent's door calls, so this is
/// the state the review read and not a second answer standing in for it.
fn outline(app: &mut App) -> serde_json::Value {
    let world = app.world();
    file_outline::state_json(world.get_resource::<Files>(), world.get_resource::<Session>())
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
    match c {
        '.' => KeyCode::Period,
        '_' => KeyCode::Minus,
        'a'..='z' => match c {
            'b' => KeyCode::KeyB, 'c' => KeyCode::KeyC, 'e' => KeyCode::KeyE, 'g' => KeyCode::KeyG,
            'i' => KeyCode::KeyI, 'o' => KeyCode::KeyO, 'r' => KeyCode::KeyR, 's' => KeyCode::KeyS,
            'u' => KeyCode::KeyU,
            other => panic!("no key for `{other}`"),
        },
        other => panic!("no key for `{other}`"),
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

/// **The rows the open palette would list**, as the person reads them.
fn matched_rows(app: &mut App) -> Vec<String> {
    let list = app.world().resource::<CommandList>().0.clone();
    let world = app.world_mut();
    let Some(palette) = world.query::<&CommandPalette>().iter(world).next() else { return Vec::new() };
    palette.results().iter().filter_map(|&at| list.get(at)).map(|row| row.label()).collect()
}

#[test]
fn a_source_file_the_finder_lists_opens_and_its_path_is_in_the_outline_state() {
    let mut app = editor("opens");

    // ── The row exists, and it is the file's own page ───────────────────────────────────────────────────────────────────────
    let entry = settle_support::until(&mut app, "the finder's index holds a row for the source file", |app| {
        app.world().resource::<GotoIndex>().entries().iter().any(|e| e.key == SOURCE)
    });
    assert!(entry >= settle_support::Frames(0), "the index is the thing being measured");
    let row = app
        .world()
        .resource::<GotoIndex>()
        .entries()
        .iter()
        .find(|e| e.key == SOURCE)
        .cloned()
        .unwrap_or_else(|| panic!("`{SOURCE}` is a row"));
    assert_eq!(row.title, "big_source.rs", "the row shows the file's own name");
    assert!(
        matches!(row.shown, bevy_dark_factory::ops::Shown::File { ref path, .. } if path == SOURCE),
        "the row's page is the file itself, at `{SOURCE}`: {:?}",
        row.shown
    );
    assert_eq!(outline(&mut app), json!({"file": null, "headings": []}), "nothing is open before the chord");

    // ── The person's gesture: the chord, the file's name, Enter ─────────────────────────────────────────────────────────────
    ctrl_p(&mut app);
    assert!(palette_is_open(&mut app), "Cmd/Ctrl+P opens the one finder");
    // **The file's own stem, with its extension**: `big_source` is what the review typed, and `big_source.rs` is the row's name.
    type_str(&mut app, "big_source.rs");
    let labels = matched_rows(&mut app);
    assert!(
        labels.iter().any(|label| label.contains("big_source.rs")),
        "the query narrows to the file's row: {labels:?}"
    );

    enter(&mut app);
    assert!(!palette_is_open(&mut app), "choosing a result closes the finder");

    // ── What the review saw stay null ───────────────────────────────────────────────────────────────────────────────────────
    settle_support::until(&mut app, "the chosen source file is open", |app| {
        outline(app)["file"].as_str() == Some(SOURCE)
    });
    assert_eq!(outline(&mut app)["file"], json!(SOURCE), "`file_outline.file` names the file the person chose");
    assert_eq!(
        *app.world().resource::<Workspace>(),
        Workspace::Files,
        "a project file's page is the Files workspace"
    );
    assert_eq!(
        files::active_file(app.world().resource::<Session>()).as_deref(),
        Some(SOURCE),
        "and the file is the tab that is active, not merely a path the reveal wrote down"
    );
}
