//! **A Rust file's compiler errors are marked where they stand, by the project's rust-analyzer** (#611), and **an editor
//! without rust-analyzer says so once and keeps its own colours**.
//!
//! # What this measures
//!
//! A fixture project is given a `Cargo.toml` and a `src/lib.rs` whose second line assigns a string to a `u32`. The file
//! is opened in the Files workspace and frames run, at most [`FRAMES`] of them, until the open editor's `Diagnostics`
//! hold an error. The error must be on line 2 and cover the string, both read back from its byte range against the
//! editor's own text — not from anything the editor computed about lines.
//!
//! The second test names a program that is not installed, opens the file and a second Rust file, and reads the status:
//! exactly one problem names the missing program and the command that installs it, said once. The editor still holds the
//! file and colours it: its highlights and its own findings are for the text on screen, and it takes typing.
//!
//! # rust-analyzer is required, not skipped
//!
//! The first test runs the real `rust-analyzer` from `PATH`. A machine without it fails here, and the failure carries the
//! editor's own status line, which names the install command (`rustup component add rust-analyzer`).

use std::path::PathBuf;
use std::time::Duration;

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::ui::InteractionDisabled;
use bevy::window::PrimaryWindow;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::editor::EditorState;
use bevy_dark_factory::files::Files;
use bevy_dark_factory::files_lsp::RustAnalyzer;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_gooey::{CodeEditor, CodeEditorValue, CodeSeverity, Diagnostics, Highlights};

use crate::fixtures;
use crate::settle_support;

/// The fixture's manifest: a package of its own, and its own workspace so no folder above it claims it.
const MANIFEST: &str = "[package]\nname = \"rust_errors_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n";

/// The file opened, with a type error on its second line: a string where a `u32` is declared.
const LIB: &str = "pub fn answer() -> u32 {\n    let wrong: u32 = \"forty-two\";\n    wrong\n}\n";
const REL: &str = "src/lib.rs";

/// **How many frames the error may take to be marked**, [`FRAME_GAP`] apart: the server's start, its reading of the
/// package and its `cargo check` of it, with room for a loaded machine.
const FRAMES: usize = 6_000;
const FRAME_GAP: Duration = Duration::from_millis(20);

/// A program name nothing installs.
const ABSENT: &str = "rust-analyzer-not-installed-611";

/// The editor on a fixture Cargo package, the Files workspace up and `src/lib.rs` open; `program` replaces the server's.
fn rig(tag: &str, program: Option<&str>) -> (App, PathBuf) {
    let name = format!("rust_errors_{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&name).descriptor("floor", "alpha").build("m");
    std::fs::write(root.join("Cargo.toml"), MANIFEST).unwrap_or_else(|e| panic!("{e}"));
    std::fs::create_dir_all(root.join("src")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join(REL), LIB).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    if let Some(program) = program {
        app.insert_resource(RustAnalyzer::with_program(program));
    }
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    harness::resize_surface(&mut app, 1400, 900).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(&mut app);
    bevy_dark_factory::files::open_file(app.world_mut(), REL).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    (app, root)
}

/// One finding as the open editor marks it: the text it covers, its line (1-based), its severity and its note.
#[derive(Debug)]
struct Marked {
    covers: String,
    line: usize,
    severity: CodeSeverity,
    message: String,
}

/// **What the open editor marks on the text it shows**, read from its `Diagnostics` for the revision on screen.
fn marked(app: &mut App) -> Vec<Marked> {
    let mut editors = app.world_mut().query_filtered::<(&CodeEditorValue, Option<&Diagnostics>), With<CodeEditor>>();
    let Some((value, Some(diagnostics))) = editors.iter(app.world()).next() else { return Vec::new() };
    if diagnostics.revision() != value.revision() {
        return Vec::new();
    }
    let text = value.text();
    diagnostics
        .items()
        .iter()
        .filter_map(|d| {
            let covers = text.get(d.range.clone())?.to_owned();
            let line = text.get(..d.range.start)?.matches('\n').count() + 1;
            Some(Marked { covers, line, severity: d.severity, message: d.message.clone() })
        })
        .collect()
}

/// The status's problems, as they read.
fn problems(app: &App) -> Vec<String> {
    app.world().resource::<EditorState>().status.problems().iter().map(|p| p.line()).collect()
}

#[test]
fn a_type_error_is_marked_on_its_line_by_rust_analyzer() {
    let (mut app, _root) = rig("marked", None);
    let mut seen = Vec::new();
    for _ in 0..FRAMES {
        app.update();
        seen = marked(&mut app);
        if seen.iter().any(|m| m.severity == CodeSeverity::Error) {
            break;
        }
        std::thread::sleep(FRAME_GAP);
    }
    let error = seen.iter().find(|m| m.severity == CodeSeverity::Error).unwrap_or_else(|| {
        panic!("no error was marked within {FRAMES} frames: the editor marked {seen:?}; the status says {:?}", problems(&app))
    });
    assert_eq!(error.line, 2, "the error is on the line that assigns the string: {error:?}");
    assert!(error.covers.contains("forty-two"), "the error covers the string: {error:?}");
    assert!(error.message.contains("mismatched types"), "the note is the compiler's: {error:?}");
    assert!(app.world().resource::<RustAnalyzer>().is_running(), "the server runs while its project is open");
}

#[test]
fn without_rust_analyzer_the_editor_says_so_once_and_keeps_its_own_colours() {
    let (mut app, root) = rig("absent", Some(ABSENT));
    // A second Rust file of the same project: the absence is not said again.
    std::fs::write(root.join("src/other.rs"), "pub fn other() -> u32 {\n    7\n}\n").unwrap_or_else(|e| panic!("{e}"));
    bevy_dark_factory::files::open_file(app.world_mut(), "src/other.rs").unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    for _ in 0..30 {
        app.update();
    }
    let said: Vec<_> = app.world().resource::<EditorState>().status.problems().iter().filter(|p| p.text.contains(ABSENT)).cloned().collect();
    assert_eq!(said.len(), 1, "one problem names the missing server: {said:?}; every problem: {:?}", problems(&app));
    assert_eq!(said[0].count, 1, "it is said once, not repeated: {said:?}");
    assert!(said[0].text.contains("not installed") && said[0].text.contains("rustup component add rust-analyzer"), "it says what is missing and how to install it: {said:?}");
    assert!(!app.world().resource::<RustAnalyzer>().is_running(), "no server runs");

    // **The tokenizer keeps working**: the file on screen is coloured and analysed by the editor's own rules, at the revision shown.
    let mut editors = app.world_mut().query_filtered::<(Entity, &CodeEditorValue, Option<&Highlights>, Option<&Diagnostics>, Has<InteractionDisabled>), With<CodeEditor>>();
    let (editor, text, highlights, findings, disabled) = editors
        .iter(app.world())
        .next()
        .map(|(e, v, h, d, disabled)| (e, v.text().to_owned(), h.map(|h| h.revision() == v.revision()), d.map(|d| d.revision() == v.revision()), disabled))
        .unwrap_or_else(|| panic!("an editor is open"));
    assert!(text.contains("pub fn other"), "the editor holds the file: {text:?}");
    assert_eq!(highlights, Some(true), "the tokenizer coloured the text on screen");
    assert_eq!(findings, Some(true), "the editor's own findings are for the text on screen");
    assert!(!disabled, "the file can be edited");

    // **And it takes typing**: a key reaches the buffer, with no server to tell.
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    settle_support::settle(&mut app);
    bevy_gooey::testing::type_text(&mut app, "typed");
    settle_support::settle(&mut app);
    let held = app.world().resource::<Files>().get("src/other.rs").map(|f| f.text.clone());
    assert!(held.as_deref().is_some_and(|t| t.contains("typed")), "the typed text reached the buffer: {held:?}");
}
