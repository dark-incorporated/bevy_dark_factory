//! **A Rust file's completions and hovers come from the project's Rust server, and its edits and saves reach it** (#698),
//! measured against a fixed server rather than rust-analyzer's own answers.
//!
//! # The fixed server
//!
//! `dark_factory-lsp-stub` (`src/bin/dark_factory-lsp-stub.rs`) is named in place of rust-analyzer through the one
//! setting the editor starts its server from, [`RustAnalyzer::with_program`]: the same spawn, the same threads and the
//! same frames as rust-analyzer gets, so nothing here depends on rust-analyzer being installed and no test skips without
//! it. The stub answers completion with four items, of which the editor's list can write two, and hover with
//! `stub hover on `<word>``; after every open, change and save it publishes one finding saying its version, the saves it
//! was told of and the whole text it holds, which is how a test reads what the server was told.
//!
//! # What this measures
//!
//! 1. **Completion at the caret**: an `s` typed after `an` asks (gooey's own typing trigger), the server's items are the
//!    editor's list at the start of the word, in the server's order, without the two it cannot apply, and Enter writes
//!    the server's edit over the word. The server then holds exactly the editor's text, sent as edits.
//! 2. **Hover**: the note on `answer` is the server's text, about the bytes the server named.
//! 3. **Save**: typing and a deletion reach the server as edits, and a save is told: the server says `1 saves` and holds
//!    what the disk now holds.

use std::path::PathBuf;

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files::Files;
use bevy_dark_factory::files_lsp::RustAnalyzer;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_gooey::{CodeCompletions, CodeEditor, CodeEditorValue, CodeHover, CodeHoverRequested, CodePos, Diagnostics};

use crate::fixtures;
use crate::settle_support;

/// The fixed server, built with the editor.
const STUB: &str = env!("CARGO_BIN_EXE_dark_factory-lsp-stub");

/// The fixture's manifest: a package of its own, and its own workspace so no folder above it claims it.
const MANIFEST: &str = "[package]\nname = \"rust_server_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n";

/// The file opened: `an` on line 6 is the word completed.
const LIB: &str = "pub fn answer() -> u32 {\n    42\n}\n\npub fn ask() -> u32 {\n    an\n}\n";
const REL: &str = "src/lib.rs";

/// The editor on a fixture Cargo package with the stub as its Rust server, the Files workspace up and `src/lib.rs` open,
/// once the stub has published what it holds: the server answered `initialize` and holds the file.
fn rig(tag: &str) -> (App, PathBuf) {
    let name = format!("rust_server_{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&name).descriptor("floor", "alpha").build("m");
    std::fs::write(root.join("Cargo.toml"), MANIFEST).unwrap_or_else(|e| panic!("{e}"));
    std::fs::create_dir_all(root.join("src")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join(REL), LIB).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(RustAnalyzer::with_program(STUB));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    harness::resize_surface(&mut app, 1400, 900).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(&mut app);
    bevy_dark_factory::files::open_file(app.world_mut(), REL).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    settle_support::until(&mut app, "the stub's publication on the open file", |app| held(app).is_some_and(|said| said.starts_with("stub v1,")));
    (app, root)
}

fn editor_of(app: &mut App) -> Entity {
    let mut editors = app.world_mut().query_filtered::<Entity, With<CodeEditor>>();
    editors.iter(app.world()).next().unwrap_or_else(|| panic!("an editor is open"))
}

fn text_of(app: &mut App) -> String {
    let editor = editor_of(app);
    app.world().get::<CodeEditorValue>(editor).map(|v| v.text().to_owned()).unwrap_or_default()
}

/// **What the stub says it holds**, from the one finding it publishes, as the editor marks it on the text on screen.
fn held(app: &mut App) -> Option<String> {
    let editor = editor_of(app);
    let value = app.world().get::<CodeEditorValue>(editor)?;
    let diagnostics = app.world().get::<Diagnostics>(editor).filter(|d| d.revision() == value.revision())?;
    // The editor's note names who found it after the message: `(stub)`, the stub's `source`.
    diagnostics.items().iter().find_map(|d| d.message.strip_suffix(" (stub)").filter(|said| said.starts_with("stub v")).map(str::to_owned))
}

/// The keyboard in the editor, with the caret at `at`.
fn caret_at(app: &mut App, at: CodePos) {
    let editor = editor_of(app);
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    app.world_mut().get_mut::<CodeEditor>(editor).map(|mut e| e.select(at, at)).unwrap_or_else(|| panic!("no editor"));
    settle_support::settle(app);
}

#[test]
fn the_servers_completions_open_at_the_caret_and_its_hover_is_the_note() {
    let (mut app, _root) = rig("complete");
    let editor = editor_of(&mut app);

    // **Completion**: the caret after `an`, and an `s` typed: gooey asks, the editor asks the server, the server answers.
    caret_at(&mut app, CodePos::new(5, 6));
    bevy_gooey::testing::type_text(&mut app, "s");
    let typed = text_of(&mut app);
    let anchor = typed.find("    ans\n").map_or(0, |at| at + 4);
    settle_support::until(&mut app, "the server's completions on the editor", |app| app.world().get::<CodeCompletions>(editor).is_some_and(|c| !c.items.is_empty()));
    let offered = app.world().get::<CodeCompletions>(editor).cloned().unwrap_or_default();
    assert_eq!(offered.anchor, anchor, "the list narrows from the start of the word at the caret");
    let read: Vec<(&str, &str, &str)> = offered.items.iter().map(|i| (i.label.as_str(), i.detail.as_str(), i.text())).collect();
    assert_eq!(
        read,
        [("answer_stub", "fn() -> u32", "answer_stub()"), ("another_stub", "const u32", "another_stub")],
        "the server's items, in its order, without the import and the rewrite the list cannot apply"
    );

    // **The list is open at the caret**: Enter writes the lit item, the server's edit, over the word typed.
    bevy_gooey::testing::press(&mut app, KeyCode::Enter);
    let chosen = text_of(&mut app);
    assert!(chosen.contains("pub fn ask() -> u32 {\n    answer_stub()\n}"), "Enter wrote the server's edit over `ans`: {chosen:?}");
    settle_support::until(&mut app, "the server holding the chosen completion", |app| held(app).is_some_and(|said| said.ends_with(&format!("holds {chosen:?}"))));

    // **Hover**: the note on `answer` is the server's text, about the bytes it named.
    let at = chosen.find("answer()").map_or(0, |at| at + 2);
    let revision = app.world().get::<CodeEditorValue>(editor).map(CodeEditorValue::revision).unwrap_or(0);
    app.world_mut().trigger(CodeHoverRequested { entity: editor, offset: at, revision });
    settle_support::until(&mut app, "the server's hover on the editor", |app| app.world().get::<CodeHover>(editor).is_some());
    let said = app.world().get::<CodeHover>(editor).cloned().unwrap_or_else(|| panic!("the hover is on the editor"));
    assert_eq!(said.text, "stub hover on `answer`", "the note is the server's");
    assert_eq!(&chosen[said.range.clone()], "answer", "about the word the server named: {said:?}");
    assert_eq!(said.revision, revision, "for the text it was asked on");
}

#[test]
fn edits_reach_the_server_as_edits_and_a_save_is_told() {
    let (mut app, root) = rig("save");

    // **Typing and a deletion**: `10` typed before `42`, and the `0` taken back.
    caret_at(&mut app, CodePos::new(1, 4));
    bevy_gooey::testing::type_text(&mut app, "10");
    bevy_gooey::testing::press(&mut app, KeyCode::Backspace);
    let edited = text_of(&mut app);
    assert!(edited.contains("\n    142\n"), "the editor holds the edit: {edited:?}");
    settle_support::until(&mut app, "the server holding the edited text", |app| held(app).is_some_and(|said| said.ends_with(&format!("0 saves, holds {edited:?}"))));

    // **A save**: the call every save in the editor makes. The server is told, and holds what the disk now holds.
    app.world_mut()
        .resource_scope(|_, mut files: Mut<Files>| bevy_dark_factory::files::save_file(&mut files, &root, REL))
        .unwrap_or_else(|e| panic!("the save was refused: {e}"));
    assert_eq!(std::fs::read_to_string(root.join(REL)).unwrap_or_else(|e| panic!("{e}")), edited, "the file holds the edit");
    settle_support::until(&mut app, "the server told of the save", |app| held(app).is_some_and(|said| said.contains(", 1 saves, ")));
    let said = held(&mut app).unwrap_or_default();
    assert!(said.ends_with(&format!(", 1 saves, holds {edited:?}")), "one save told, of the text saved: {said}");
}
