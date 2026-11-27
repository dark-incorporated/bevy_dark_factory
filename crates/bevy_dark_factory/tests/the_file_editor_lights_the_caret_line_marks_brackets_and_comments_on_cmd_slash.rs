//! **The file editor lights the caret's line, marks the bracket at the caret and its match, and comments lines on `Cmd+/`** (#612:
//! the owner's three picks of 2026-11-27 among the VSCode-like behaviours this file measures).
//!
//! # What the editor did before
//!
//! - **The caret's line and the brackets were already gooey's.** `bevy_gooey`'s code editor (v0.62.0) draws a band across the caret's
//!   row (`Role::CodeLineLit`, the skin's `surface_raised` over the editor's own ground) and a framed mark on the bracket at the caret
//!   and on the one that matches it (`Role::CodeBracket`, the accent at a quarter), in every editor, with no switch to turn on. What
//!   nothing measured was that they reach the Files workspace's editor, so a theme, a pin or a page change that lost them would have
//!   passed every test. These two measure them there.
//! - **`Cmd+/` did nothing.** Gooey's editor lets a primary chord it does not know go by, and no chord in `chords.rs` or row in
//!   `keys.rs` named `/` with a modifier, so the text was what it was before the press.
//!
//! # What this measures
//!
//! The band's own rect against the caret's row, both marks' rects against the two brackets' places, and the file's text after
//! `Cmd+/` and after a second one. The comment's spelling for each language is measured on `files_lang::toggle_comment` itself.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files::{self, Files};
use bevy_dark_factory::files_lang::{self, Lang};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_gooey::{CodeEditor, CodeEditorValue, CodePos, Role};

use crate::fixtures;
use crate::settle_support;

/// A game's own Rust source: brackets of three kinds, an indented body and a blank line.
const SOURCE: &str = "fn main() {\n    let x = (1 + 2);\n\n    let y = [x];\n}\n";

/// Where it sits in the project.
const REL: &str = "src/lib.rs";

/// **An editor in the Files workspace with `rel` open and the keyboard in it.** The window is spawned because the code editor lays
/// its rows out against the interface size the window gives it.
fn editor_on(tag: &str, rel: &str, text: &str) -> (App, Entity) {
    let root = fixtures::Fixture::new(&format!("polish612_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    harness::resize_surface(&mut app, 1400, 900).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, text).unwrap_or_else(|e| panic!("{e}"));
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(&mut app);
    files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let editor = app.world_mut().query_filtered::<Entity, With<CodeEditor>>().single(app.world_mut()).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    settle_support::settle(&mut app);
    (app, editor)
}

fn stand(app: &mut App, editor: Entity, line: usize, col: usize) {
    select(app, editor, CodePos::new(line, col), CodePos::new(line, col));
}

fn select(app: &mut App, editor: Entity, anchor: CodePos, head: CodePos) {
    let Some(mut widget) = app.world_mut().get_mut::<CodeEditor>(editor) else { panic!("the editor is open") };
    widget.select(anchor, head);
    settle_support::settle(app);
}

/// Every entity at or under `root`.
fn under(world: &World, root: Entity) -> Vec<Entity> {
    let mut found = vec![root];
    let mut next = 0;
    while next < found.len() {
        if let Some(children) = world.get::<Children>(found[next]) {
            found.extend(children.iter());
        }
        next += 1;
    }
    found
}

/// A px value the editor wrote, or a panic naming the one it did not.
fn px(v: Val, what: &str) -> f32 {
    match v {
        Val::Px(p) => p,
        other => panic!("{what} carries {other:?}, which no layout wrote"),
    }
}

/// **The drawn parts of one role under the editor**: each one's `(left, top, width, height)` as its `Node` says, and its fill.
fn drawn(app: &App, editor: Entity, wanted: fn(Role) -> bool) -> Vec<(Node, Option<Color>)> {
    let world = app.world();
    under(world, editor)
        .into_iter()
        .filter(|&e| world.get::<Role>(e).copied().is_some_and(wanted))
        .filter_map(|e| world.get::<Node>(e).map(|node| (node.clone(), world.get::<BackgroundColor>(e).map(|b| b.0))))
        .filter(|(node, _)| node.display != Display::None)
        .collect()
}

/// **The editor's own ground**, the fill a band must differ from to be seen.
fn ground(app: &App, editor: Entity) -> Option<Color> {
    let world = app.world();
    under(world, editor)
        .into_iter()
        .find(|&e| matches!(world.get::<Role>(e).copied(), Some(Role::CodeEditor)))
        .and_then(|e| world.get::<BackgroundColor>(e).map(|b| b.0))
}

/// **The band over the caret's row**: its top and height. One band, drawn.
fn band(app: &App, editor: Entity) -> (f32, f32, Option<Color>) {
    let bands = drawn(app, editor, |r| matches!(r, Role::CodeLineLit));
    assert_eq!(bands.len(), 1, "one band lights the caret's line, drawn: {bands:?}");
    let (node, fill) = &bands[0];
    (px(node.top, "the band's top"), px(node.height, "the band's height"), *fill)
}

/// **The caret's line is lit across its whole row** (#612 pick 1). The band's top is the caret's row times a line's height (the
/// file is short, so the view is not scrolled), it follows the caret to another line, and its fill is not the editor's ground.
#[test]
fn the_caret_line_is_lit_on_the_caret_row() {
    let (mut app, editor) = editor_on("lit", REL, SOURCE);
    stand(&mut app, editor, 3, 4);
    let (top, line_px, fill) = band(&app, editor);
    assert!(line_px > 0.0, "the band is a line high: {line_px}");
    assert_eq!(top, 3.0 * line_px, "the band stands on the caret's row, line 3: top {top} with lines {line_px} px high");
    stand(&mut app, editor, 1, 0);
    let (moved, ..) = band(&app, editor);
    assert_eq!(moved, line_px, "the band followed the caret to line 1: top {moved}");
    let ground = ground(&app, editor).unwrap_or_else(|| panic!("the editor has a ground"));
    let fill = fill.unwrap_or_else(|| panic!("the band has a fill"));
    assert_ne!(fill, ground, "the band is a colour the editor's ground is not, so a person sees it");
}

/// **The bracket at the caret and the one that matches it are marked** (#612 pick 2). The caret stands just before line 0's `{`;
/// its match is line 4's `}`. Two marks are drawn, one a character wide on each bracket's row, and they stand as far apart across
/// as the two brackets' columns are (10 and 0).
#[test]
fn the_bracket_at_the_caret_and_its_match_are_marked() {
    let (mut app, editor) = editor_on("brackets", REL, SOURCE);
    stand(&mut app, editor, 0, 10);
    let line_px = band(&app, editor).1;
    let mut marks: Vec<(f32, f32, f32)> = drawn(&app, editor, |r| matches!(r, Role::CodeBracket))
        .into_iter()
        .map(|(node, _)| (px(node.top, "a mark's top"), px(node.left, "a mark's left"), px(node.width, "a mark's width")))
        .collect();
    marks.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(marks.len(), 2, "the open bracket and its match are each marked: {marks:?}");
    let ((open_top, open_left, advance), (close_top, close_left, _)) = (marks[0], marks[1]);
    assert_eq!(open_top, 0.0, "the `{{` mark stands on line 0");
    assert_eq!(close_top, 4.0 * line_px, "the `}}` mark stands on line 4: {close_top} with lines {line_px} px high");
    assert!(advance > 0.0, "a mark is one character wide: {advance}");
    assert!(
        (open_left - close_left - 10.0 * advance).abs() < 0.01,
        "the `{{` at column 10 is marked ten characters right of the `}}` at column 0: {open_left} and {close_left}, {advance} px a character"
    );
    // Away from any bracket, nothing is marked.
    stand(&mut app, editor, 1, 6);
    let none = drawn(&app, editor, |r| matches!(r, Role::CodeBracket));
    assert!(none.is_empty(), "a caret on a word marks no bracket: {none:?}");
}

fn send(app: &mut App, key_code: KeyCode, logical_key: Key, state: ButtonState) {
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("one primary window");
    app.world_mut().write_message(KeyboardInput {
        key_code,
        text: match (&logical_key, state) {
            (Key::Character(c), ButtonState::Pressed) => Some(c.clone()),
            _ => None,
        },
        logical_key,
        state,
        repeat: false,
        window,
    });
}

/// The platform's primary modifier, which `primary(Slash)` names: Command on a mac, Control elsewhere.
fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

/// **One `Cmd+/`**, as the keyboard sends it: the modifier down, `/` down and up, the modifier up.
fn cmd_slash(app: &mut App) {
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    send(app, primary(), unidentified(), ButtonState::Pressed);
    app.update();
    send(app, KeyCode::Slash, Key::Character("/".into()), ButtonState::Pressed);
    app.update();
    send(app, KeyCode::Slash, Key::Character("/".into()), ButtonState::Released);
    send(app, primary(), unidentified(), ButtonState::Released);
    settle_support::settle(app);
}

/// The file's text as the editor holds it, after checking the widget holds the same.
fn held(app: &mut App, editor: Entity) -> String {
    let text = app.world().resource::<Files>().get(REL).map(|f| f.text.clone()).unwrap_or_else(|| panic!("{REL} is open"));
    let shown = app.world().get::<CodeEditorValue>(editor).map(|v| v.text().to_owned()).unwrap_or_else(|| panic!("the editor is open"));
    assert_eq!(shown, text, "the widget and the file's buffer hold the same text");
    text
}

/// **`Cmd+/` on a `.rs` line puts `// ` at its indent, and a second `Cmd+/` takes it off** (#612 pick 3). The caret stays on the
/// text it was on, and the file is dirty after the first press and clean after the second.
#[test]
fn cmd_slash_comments_a_rust_line_and_a_second_press_takes_it_off() {
    let (mut app, editor) = editor_on("comment", REL, SOURCE);
    stand(&mut app, editor, 1, 8);
    cmd_slash(&mut app);
    let once = held(&mut app, editor);
    assert_eq!(once, SOURCE.replace("    let x", "    // let x"), "the caret's line is commented at its indent, in Rust's `//`");
    assert!(app.world().resource::<Files>().is_dirty(REL), "a commented line is an unsaved edit");
    let caret = app.world().get::<CodeEditor>(editor).map(|w| w.buffer().caret()).unwrap_or_else(|| panic!("the editor is open"));
    assert_eq!(caret, CodePos::new(1, 11), "the caret stayed on the `x` it was on, three bytes along");
    cmd_slash(&mut app);
    assert_eq!(held(&mut app, editor), SOURCE, "a second Cmd+/ takes the comment off, and the text is as it was");
    assert!(!app.world().resource::<Files>().is_dirty(REL), "back to what the disk holds");
}

/// **A selection over several lines comments them all at their shallowest indent, leaving the blank line alone**, and `Cmd+/` again
/// takes every mark off, because every line with words on it carries one.
#[test]
fn cmd_slash_over_a_selection_comments_every_line_with_words() {
    let (mut app, editor) = editor_on("block", REL, SOURCE);
    // From line 1 to the start of line 4: the drag stopped at line 4's start, so line 4 is not taken.
    select(&mut app, editor, CodePos::new(1, 2), CodePos::new(4, 0));
    cmd_slash(&mut app);
    assert_eq!(held(&mut app, editor), "fn main() {\n    // let x = (1 + 2);\n\n    // let y = [x];\n}\n");
    cmd_slash(&mut app);
    assert_eq!(held(&mut app, editor), SOURCE);
}

/// **Each language's own comment**: `#` in YAML, `//` in `.ron`, Rhai and Rust, `<!-- -->` round a Markdown line, and nothing for a
/// file in no language the editor knows a comment for.
#[test]
fn every_language_comments_in_its_own_syntax() {
    let apply = |lang: Lang, text: &str, lines: std::ops::RangeInclusive<usize>| -> Option<String> {
        let mut edits = files_lang::toggle_comment(lang, text, lines)?;
        let mut out = text.to_owned();
        edits.sort_by_key(|(range, _)| range.start);
        for (range, with) in edits.into_iter().rev() {
            out.replace_range(range, &with);
        }
        Some(out)
    };
    assert_eq!(apply(Lang::Yaml, "a: 1\nb:\n  c: 2\n", 1..=2).as_deref(), Some("a: 1\n# b:\n#   c: 2\n"));
    assert_eq!(apply(Lang::Ron, "(\n    a: 1,\n)\n", 1..=1).as_deref(), Some("(\n    // a: 1,\n)\n"));
    assert_eq!(apply(Lang::Rhai, "let a = 1;\n", 0..=0).as_deref(), Some("// let a = 1;\n"));
    assert_eq!(apply(Lang::Rust, "    fn f() {}\n", 0..=0).as_deref(), Some("    // fn f() {}\n"));
    assert_eq!(apply(Lang::Markdown, "# Title\n\nSome words.\n", 2..=2).as_deref(), Some("# Title\n\n<!-- Some words. -->\n"));
    assert_eq!(apply(Lang::Markdown, "<!-- Some words. -->\n", 0..=0).as_deref(), Some("Some words.\n"), "Markdown's wrap comes off");
    assert_eq!(apply(Lang::Yaml, "# a: 1\n#b: 2\n", 0..=1).as_deref(), Some("a: 1\nb: 2\n"), "a mark with no space after it comes off alone");
    assert_eq!(
        apply(Lang::Rust, "// a\nb\n", 0..=1).as_deref(),
        Some("// // a\n// b\n"),
        "a block that is not all commented is commented, the way VS Code does it"
    );
    assert_eq!(files_lang::toggle_comment(Lang::Plain, "words\n", 0..=0), None, "a `.txt` file has no comment to put on");
    assert_eq!(Lang::of(REL), Lang::Rust, "a `.rs` file is Rust's");
}
