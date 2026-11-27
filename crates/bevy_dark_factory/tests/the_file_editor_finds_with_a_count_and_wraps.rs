//! **The file editor finds a word, says how many there are, and wraps round the page** (#612, the owner's pick of
//! 2026-11-27 from the polish candidates: *"a find/replace bar that shows match count and wraps"*).
//!
//! # Where the bar comes from, and what was wrong with it here
//!
//! The Files editor is gooey's `code_editor`, and every one carries gooey's find bar (`CodeFind`): `Cmd+F` (or `Ctrl+F`)
//! opens it with the keyboard in its query, `Cmd+Alt+F` with the replace field. `Cmd+F` is also a census chord here,
//! `FocusFilter`, which is `Context::Global`; on the Files workspace it takes no box (`tiles::focus_filter`), so with the
//! editor focused the chord is the editor's alone.
//!
//! The bar's fields are children of the code editor, gooey's text field lets `Enter` bubble, and the editor answers every
//! key that reaches it: `Enter` in the query stepped to a match and then put a line break in the page over it (read from
//! gooey's source at the pin, `code_editor.rs:964` and `text.rs:143`; dark-incorporated/bevy_gooey#128). The editor now
//! stops the bar's keys at the bar (`files::keep_the_find_bars_keys`), and this measures the bar as a person reaches it:
//! the Files workspace, a page open from the project, the editor focused.
//!
//! # What this measures
//!
//! 1. `Cmd+F` opens the bar inside the editor's box and puts the keyboard in its query; no filter box takes it.
//! 2. A word the page holds three times reads `1 of 3` as it is typed.
//! 3. `Enter` walks the matches, `1 of 3`, `2 of 3`, `3 of 3`, and the next wraps to `1 of 3` with the first match
//!    selected again.
//! 4. `Previous` from the first match wraps to the last: `3 of 3`, the last match selected.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files;
use bevy_dark_factory::filter::Filters;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_gooey::{CodeEditor, CodeEditorValue, CodeFind, CodePos, Role, testing};

use crate::fixtures;
use crate::settle_support;

const PAGE_REL: &str = "design/notes.md";

/// **A page that holds `frost` three times**, at the start of lines 0, 2 and 4.
const PAGE: &str = "frost on the glass\nnothing here\nfrost on the field\nmore words\nfrost on the road\n";

/// The three matches of `frost`, as the places they start and end.
const MATCHES: [(CodePos, CodePos); 3] = [
    (CodePos { line: 0, col: 0 }, CodePos { line: 0, col: 5 }),
    (CodePos { line: 2, col: 0 }, CodePos { line: 2, col: 5 }),
    (CodePos { line: 4, col: 0 }, CodePos { line: 4, col: 5 }),
];

/// How far a laid-out box may stand past another, physical px (layout rounds to whole pixels).
const SLACK_PX: f32 = 1.0;

fn page(tag: &str) -> (App, Entity) {
    let root = fixtures::Fixture::new(&format!("find_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let full = root.join(PAGE_REL);
    std::fs::create_dir_all(full.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, PAGE).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    harness::resize_surface(&mut app, 1400, 900).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(&mut app);
    files::open_file(app.world_mut(), PAGE_REL).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let editor = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<CodeEditor>>().single(world).unwrap_or_else(|e| panic!("one code editor is open: {e}"))
    };
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    if let Some(mut widget) = app.world_mut().get_mut::<CodeEditor>(editor) {
        widget.select(CodePos::new(0, 0), CodePos::new(0, 0));
    }
    settle_support::settle(&mut app);
    (app, editor)
}

/// **The host's primary modifier key** (#638): `Command` on macOS, `Control` elsewhere.
fn primary_key() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

fn key(app: &mut App, code: KeyCode, logical: Key, state: ButtonState) {
    let window = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PrimaryWindow>>().single(world).unwrap_or(Entity::PLACEHOLDER)
    };
    app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: logical, state, text: None, repeat: false, window });
    app.update();
}

/// **`Cmd+F` as a person presses it**: the modifier down, `F` down and up, the modifier up.
fn press_find(app: &mut App) {
    let modifier = if cfg!(target_os = "macos") { Key::Super } else { Key::Control };
    key(app, primary_key(), modifier.clone(), ButtonState::Pressed);
    key(app, KeyCode::KeyF, Key::Character("f".into()), ButtonState::Pressed);
    key(app, KeyCode::KeyF, Key::Character("f".into()), ButtonState::Released);
    key(app, primary_key(), modifier, ButtonState::Released);
    settle_support::settle(app);
}

fn descendants(world: &World, root: Entity) -> Vec<Entity> {
    let mut found = vec![root];
    let mut next = 0;
    while next < found.len() {
        if let Some(kids) = world.get::<Children>(found[next]) {
            found.extend(kids.iter());
        }
        next += 1;
    }
    found
}

/// A node's border box, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **The find bar under the editor**, when one is drawn.
fn bar(app: &App, editor: Entity) -> Option<Entity> {
    let world = app.world();
    descendants(world, editor).into_iter().find(|e| world.get::<Role>(*e) == Some(&Role::CodeFind))
}

/// **What the bar's count says**, read from the words it draws: `n of m`, or `No results`.
fn count_words(app: &App, editor: Entity) -> String {
    let world = app.world();
    let Some(bar) = bar(app, editor) else { panic!("the find bar is drawn") };
    let words: Vec<String> = descendants(world, bar)
        .into_iter()
        .filter_map(|e| world.get::<Text>(e).map(|t| t.0.clone()))
        .filter(|t| t.contains(" of ") || t == "No results")
        .collect();
    assert_eq!(words.len(), 1, "the bar draws one count: {words:?}");
    words[0].clone()
}

fn selection(app: &App, editor: Entity) -> (CodePos, CodePos) {
    let Some(widget) = app.world().get::<CodeEditor>(editor) else { panic!("the editor is open") };
    let selection = widget.buffer().selection();
    (selection.start(), selection.end())
}

/// **The page's text as the editor holds it.**
fn page_text(app: &App, editor: Entity) -> String {
    app.world().get::<CodeEditorValue>(editor).map(|value| value.text().to_owned()).unwrap_or_else(|| panic!("the editor is open"))
}

/// **`Cmd+F` opens the bar inside the editor with the keyboard in its query**, and no filter box takes the chord.
#[test]
fn the_find_chord_opens_the_bar_inside_the_editor_with_the_keyboard_in_its_query() {
    let (mut app, editor) = page("open");
    assert!(bar(&app, editor).is_none(), "no find bar is drawn before the chord");
    press_find(&mut app);
    assert!(app.world().get::<CodeFind>(editor).is_some_and(|find| find.open), "Cmd+F opened the editor's find bar");
    assert_eq!(testing::focused_label(&app).as_deref(), Some("Find"), "the keyboard is in the bar's query");
    assert!(!app.world().resource::<Filters>().typing(), "no filter box took the find chord");
    let Some(bar) = bar(&app, editor) else { panic!("the find bar is drawn") };
    let world = app.world();
    let bar_box = border_box(world, bar).unwrap_or_else(|| panic!("the bar is laid out"));
    let editor_box = border_box(world, editor).unwrap_or_else(|| panic!("the editor is laid out"));
    assert!(bar_box.width() > 0.0 && bar_box.height() > 0.0, "the bar has a size: {bar_box:?}");
    assert!(
        bar_box.min.x >= editor_box.min.x - SLACK_PX
            && bar_box.max.x <= editor_box.max.x + SLACK_PX
            && bar_box.min.y >= editor_box.min.y - SLACK_PX
            && bar_box.max.y <= editor_box.max.y + SLACK_PX,
        "the bar {bar_box:?} is drawn inside the editor {editor_box:?}"
    );
}

/// **The count says how many there are, and `Enter` walks them and wraps** from the last match to the first.
#[test]
fn the_count_reads_n_of_three_and_enter_wraps_from_the_last_match_to_the_first() {
    let (mut app, editor) = page("wrap");
    press_find(&mut app);
    testing::type_text(&mut app, "frost");
    assert_eq!(count_words(&app, editor).as_str(), "1 of 3", "the word is on the page three times");
    assert_eq!(page_text(&app, editor).as_str(), PAGE, "typing the query put nothing in the page");
    // The first Enter selects the match the count is on; each next one steps, and the fourth wraps.
    for (presses, want, at) in [(1, "1 of 3", 0), (2, "2 of 3", 1), (3, "3 of 3", 2), (4, "1 of 3", 0)] {
        testing::press(&mut app, KeyCode::Enter);
        assert_eq!(page_text(&app, editor).as_str(), PAGE, "Enter {presses} in the bar stepped and put no line break in the page");
        assert_eq!(count_words(&app, editor).as_str(), want, "after {presses} Enter");
        assert_eq!(selection(&app, editor), MATCHES[at], "after {presses} Enter the selection is match {at}");
    }
}

/// **`Previous` from the first match wraps to the last.**
#[test]
fn previous_from_the_first_match_wraps_to_the_last() {
    let (mut app, editor) = page("back");
    press_find(&mut app);
    testing::type_text(&mut app, "frost");
    testing::press(&mut app, KeyCode::Enter);
    assert_eq!(count_words(&app, editor).as_str(), "1 of 3");
    let previous = testing::find_by_label(&mut app, "Previous").unwrap_or_else(|e| panic!("the bar has a Previous button: {e}"));
    testing::click(&mut app, previous);
    settle_support::settle(&mut app);
    assert_eq!(count_words(&app, editor).as_str(), "3 of 3", "Previous from the first match is the last");
    assert_eq!(selection(&app, editor), MATCHES[2], "the last match is selected");
}
