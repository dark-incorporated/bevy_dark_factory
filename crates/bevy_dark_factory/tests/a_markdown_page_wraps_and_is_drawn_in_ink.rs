//! **A Markdown page in the Files workspace wraps at the editor's edge and is drawn in Markdown's inks** (#525,
//! `feedback-1795094391027`: *"This markdown editor is not great"*; the owner approved soft wrap and syntax
//! highlighting, which `bevy_gooey` v0.54.0 ships as the `CodeSoftWrap` and `CodeMarkdown` components, #114).
//!
//! What is measured, as a person reaches it (the Files workspace, a page opened from its tree, the editor focused):
//!
//! 1. **A line far longer than the editor is wide, and a word longer than a row, are drawn inside the editor's content
//!    box and are all there**, at interface sizes 0.5, 1.0, 1.6, 2.4 and 3.0. Measured from the glyphs the rows paint
//!    (`TextLayoutInfo`) against the box of the code area, not from anything the wrap computed: *inside the box* alone
//!    would pass an editor that draws only the columns in view, so the text the rows draw must also be the whole text.
//! 2. **A heading, an emphasis, a code span and a link are four inks, none the plain text's**, read from the colour of
//!    the runs the rows draw.
//! 3. **Editing, selection, undo and Up/Down by drawn row work on a wrapped line.**
//! 4. **The wrap and the ink follow the file shown**: a page has them, a map opened in the same editor has neither, and
//!    the page again has both.
//! 5. **A wrapped list item's later rows start under its text** (#612, `CodeWrapIndent::Same`), read from where the rows'
//!    boxes stand against the editor's own column, and a paragraph with no marker keeps its rows at the edge.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files::{self, Files};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::settings::UserSettingsFile;
use bevy_dark_factory::workspace::Workspace;
use bevy_gooey::{CodeEditor, CodeEditorValue, CodeMarkdown, CodePos, CodeSoftWrap, CodeWrapIndent, Role};

use crate::fixtures;
use crate::settle_support;


/// The interface sizes the acceptance names: the whole range a person can set (`SCALE_RANGE` is 0.5 to 3.0) and the
/// two rungs between.
const INTERFACE_SCALES: [f32; 5] = [0.5, 1.0, 1.6, 2.4, 3.0];

/// The page's file, and a map the same editor can be switched to.
const PAGE: &str = "design/notes.md";
const MAP: &str = "assets/blueprint/maps/m.map.yaml";
const TXT: &str = "design/plain.txt";

/// How far a glyph may stand past the code area it is drawn in, physical px (layout rounds to whole pixels).
const SLACK_PX: f32 = 1.0;

/// **A probe line the editor measures its column width on.** Twenty characters of one face: the editor is opened on it and the width
/// its glyphs paint over twenty is the column the page below is sized against, so the page is as long, in columns, at every interface
/// size: far too long for a row and still short enough for the view to hold every row of it.
const PROBE: &str = "0123456789abcdefghij";

/// **One paragraph of many words on one line, then one word longer than any row, then a short line**, sized in columns: the paragraph is
/// two and a half rows of `columns`, the word one and a quarter rows. The paragraph is the line the owner's capture showed running off the
/// card; the word has no place to break and is cut at the edge.
fn long_page(columns: usize) -> String {
    let mut paragraph = String::new();
    let mut i = 0;
    while paragraph.len() < columns * 5 / 2 {
        paragraph.push_str(&format!("frost{i} took the blossom "));
        i += 1;
    }
    let word = "x".repeat(columns * 5 / 4);
    format!("{}\n{word}\nend\n", paragraph.trim_end())
}



/// An editor in the Files workspace on a project holding `PAGE` (with `text`) and the map, `PAGE` open and the code
/// editor focused, at the person's `interface_scale`.
///
/// **The scale goes through a settings file and the window is spawned**, for the reason
/// `no_panel_draws_a_control_past_its_own_content_box` states: `sync_settings` re-reads the files every frame, so a
/// write to the resource would be gone, and `UiScale` is only fitted when a primary window exists.
fn files_editor(tag: &str, interface_scale: f32, text: &str) -> App {
    let root = fixtures::Fixture::new(&format!("wrap_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m");
    let page = root.join(PAGE);
    std::fs::create_dir_all(page.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&page, text).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join(TXT), "*plain* words, not markup\n").unwrap_or_else(|e| panic!("{e}"));
    let user = root.join(format!("user_settings_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(user.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&user, format!("version: 1\ninterface_scale: {interface_scale}\n")).expect("a settings file");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user));
    let window = app
        .world_mut()
        .spawn((Window { resolution: WindowResolution::new(2560, 1604), ..default() }, PrimaryWindow))
        .id();
    app.world_mut().get_mut::<Window>(window).expect("the window just spawned").resolution.set_scale_factor_override(Some(1.0));
    harness::resize_surface(&mut app, 2560, 1604).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "the Files workspace is reachable by its command");
    files::open_file(app.world_mut(), PAGE).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let editor = editor_of(&mut app);
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    settle_support::settle(&mut app);
    let want = bevy_dark_factory::surface::ui_scale_for_a_person(2560.0, 1.0, interface_scale);
    let got = app.world().resource::<UiScale>().0;
    assert!((got - want).abs() < 1e-3, "the editor is drawn at interface size {interface_scale}: UiScale is {got}, not {want}");
    app
}

fn editor_of(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<CodeEditor>>().single(world).unwrap_or_else(|e| panic!("one code editor is open: {e}"))
}

fn text(app: &mut App) -> String {
    let editor = editor_of(app);
    app.world().get::<CodeEditorValue>(editor).map(|v| v.text().to_owned()).unwrap_or_default()
}

fn caret(app: &mut App) -> CodePos {
    let editor = editor_of(app);
    app.world().get::<CodeEditor>(editor).map(|e| e.buffer().caret()).unwrap_or_else(|| panic!("no editor"))
}

fn caret_at(app: &mut App, line: usize, col: usize) {
    let editor = editor_of(app);
    let pos = CodePos::new(line, col);
    app.world_mut().get_mut::<CodeEditor>(editor).map(|mut e| e.select(pos, pos)).unwrap_or_else(|| panic!("no editor"));
    settle_support::settle(app);
}

fn select(app: &mut App, from: CodePos, to: CodePos) {
    let editor = editor_of(app);
    app.world_mut().get_mut::<CodeEditor>(editor).map(|mut e| e.select(from, to)).unwrap_or_else(|| panic!("no editor"));
    settle_support::settle(app);
}

fn key(app: &mut App, code: KeyCode, logical: Key, state: ButtonState) {
    let window = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PrimaryWindow>>().single(world).unwrap_or(Entity::PLACEHOLDER)
    };
    app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: logical, state, text: None, repeat: false, window });
    app.update();
}

fn chord_ctrl(app: &mut App, code: KeyCode, name: &str) {
    key(app, primary_key(), Key::Control, ButtonState::Pressed);
    key(app, code, Key::Character(name.into()), ButtonState::Pressed);
    key(app, code, Key::Character(name.into()), ButtonState::Released);
    key(app, KeyCode::ControlLeft, Key::Unidentified(NativeKey::Unidentified), ButtonState::Released);
    settle_support::settle(app);
}

// ── What is drawn ───────────────────────────────────────────────────────────────────────────

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

/// The code area of the editor: the box its text rows are drawn in, the gutter and the minimap not in it.
fn code_area(app: &mut App) -> Rect {
    let editor = editor_of(app);
    let world = app.world();
    let areas: Vec<Entity> = descendants(world, editor).into_iter().filter(|e| world.get::<Role>(*e) == Some(&Role::CodeArea)).collect();
    assert_eq!(areas.len(), 1, "one code area under the editor: {areas:?}");
    border_box(world, areas[0]).unwrap_or_else(|| panic!("the code area is laid out"))
}

/// **What one row paints**: the union of the boxes of its glyphs, physical px, or `None` for a row with none.
fn painted(world: &World, row: Entity) -> Option<Rect> {
    let box_ = border_box(world, row)?;
    let info = world.get::<TextLayoutInfo>(row)?;
    let computed = world.get::<ComputedNode>(row)?;
    let start = box_.min + computed.content_inset().min_inset;
    let mut out: Option<Rect> = None;
    for glyph in &info.glyphs {
        let at = start + glyph.position;
        let rect = Rect::from_corners(at, at + glyph.atlas_info.rect.size());
        out = Some(out.map_or(rect, |so_far| so_far.union(rect)));
    }
    out
}

/// The editor's drawn rows, top to bottom: `(the row, the words it holds)`.
fn drawn_rows(app: &mut App) -> Vec<(Entity, String)> {
    let editor = editor_of(app);
    let world = app.world();
    let mut found: Vec<(f32, Entity, String)> = descendants(world, editor)
        .into_iter()
        .filter(|e| world.get::<Role>(*e) == Some(&Role::CodeText))
        .filter(|e| world.get::<Node>(*e).is_some_and(|n| n.display != Display::None))
        .filter(|e| world.get::<Visibility>(*e) != Some(&Visibility::Hidden))
        .filter_map(|e| {
            // A row with Markdown (or any highlight) draws its words as spans and leaves its own text empty.
            let mut words = world.get::<Text>(e)?.0.clone();
            for kid in world.get::<Children>(e).into_iter().flat_map(|kids| kids.iter()) {
                if let Some(span) = world.get::<TextSpan>(kid) {
                    words.push_str(&span.0);
                }
            }
            let y = border_box(world, e)?.min.y;
            Some((y, e, words))
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found.into_iter().map(|(_, row, words)| (row, words)).collect()
}

/// The editor's drawn rows, top to bottom: `(words the row holds, what it paints)`.
fn rows(app: &mut App) -> Vec<(String, Option<Rect>)> {
    let drawn = drawn_rows(app);
    let world = app.world();
    drawn.into_iter().map(|(row, words)| (words, painted(world, row))).collect()
}

/// Every span under the editor that holds words: `(its words, its colour)`.
fn runs(app: &mut App) -> Vec<(String, Color)> {
    let editor = editor_of(app);
    let world = app.world();
    descendants(world, editor)
        .into_iter()
        .filter_map(|e| {
            let words = world.get::<TextSpan>(e)?;
            (!words.0.is_empty()).then(|| (words.0.clone(), world.get::<TextColor>(e).map_or(Color::NONE, |c| c.0)))
        })
        .collect()
}

// ── Acceptance 1 ────────────────────────────────────────────────────────────────────────────

/// **A long line and a long word are drawn inside the code area, whole, at every interface size.**
///
/// Without `CodeSoftWrap` the row of the long line draws only the columns in view and scrolls sideways: its glyphs
/// are then inside the box too, but the text the rows draw is a part of the line, which the second assertion fails.
#[test]
fn a_long_line_and_a_long_word_are_drawn_whole_inside_the_editors_content_box_at_five_interface_sizes() {
    for scale in INTERFACE_SCALES {
        let tag = format!("[interface size {scale}]");
        let mut app = files_editor(&format!("box_{}", (scale * 10.0) as u32), scale, PROBE);
        let area = code_area(&mut app);
        let probe = rows(&mut app).into_iter().find(|(words, _)| words.starts_with("0123")).and_then(|(_, rect)| rect);
        let probe = probe.unwrap_or_else(|| panic!("{tag}: the probe line is drawn"));
        let columns = (area.width() / (probe.width() / PROBE.len() as f32)).floor() as usize;
        assert!(columns >= 8, "{tag}: the code area is {} px wide, {columns} columns", area.width());

        let page = long_page(columns);
        let editor = editor_of(&mut app);
        app.world_mut().get_mut::<CodeEditorValue>(editor).map(|mut v| v.set(page.clone())).unwrap_or_else(|| panic!("no value"));
        settle_support::settle(&mut app);
        assert_eq!(text(&mut app), page, "{tag}: the editor holds the text it was given");
        let drawn = rows(&mut app);

        // Every glyph of every row is inside the code area.
        let mut spills = Vec::new();
        for (words, rect) in &drawn {
            let Some(rect) = rect else { continue };
            if rect.max.x > area.max.x + SLACK_PX || rect.min.x < area.min.x - SLACK_PX {
                spills.push(format!("`{}` is drawn from {:.1} to {:.1}, the code area is {:.1} to {:.1}", words.trim(), rect.min.x, rect.max.x, area.min.x, area.max.x));
            }
        }
        assert!(spills.is_empty(), "{tag}: {} rows draw past the code area: {spills:?}", spills.len());

        // And the rows draw the whole text: the lines joined, in order, nothing cut.
        let whole: String = page.lines().collect();
        let joined: String = drawn.iter().map(|(words, _)| words.as_str()).collect();
        assert_eq!(joined, whole, "{tag}: the rows draw the page whole ({} rows, {columns} columns in a box {:.0} px wide)", drawn.len(), area.width());
        assert!(drawn.len() >= 6, "{tag}: a paragraph of {} columns, a word of {} and a short line take at least six rows ({} drawn)", columns * 5 / 2, columns * 5 / 4, drawn.len());
    }
}

// ── Acceptance 2 ────────────────────────────────────────────────────────────────────────────

const MARKED: &str = "# A heading\n\nPlain words, *emphasis*, `code span` and a [link](notes.md).\n";

/// **A heading, an emphasis, a code span and a link are drawn in four inks, none the plain text's.**
#[test]
fn a_markdown_pages_heading_emphasis_code_and_link_are_drawn_in_inks_of_their_own() {
    let mut app = files_editor("ink", 1.0, MARKED);
    // Put the caret on the last line, away from the marks, so no caret-reveal rewrites the rows being read.
    caret_at(&mut app, 3, 0);
    let drawn = runs(&mut app);
    let ink = |needle: &str| -> Color {
        let found: Vec<&(String, Color)> = drawn.iter().filter(|(words, _)| words.contains(needle)).collect();
        assert_eq!(found.len(), 1, "one run holds `{needle}`: {drawn:?}");
        found[0].1
    };
    let plain = ink("Plain words");
    let heading = ink("A heading");
    let emphasis = ink("emphasis");
    let code = ink("code span");
    let link = ink("link");
    let marked = [("heading", heading), ("emphasis", emphasis), ("code span", code), ("link", link)];
    for (name, colour) in marked {
        assert_ne!(colour, plain, "the {name} is drawn in the plain text's ink: {drawn:?}");
    }
    for (i, (a, ca)) in marked.iter().enumerate() {
        for (b, cb) in &marked[i + 1..] {
            assert_ne!(ca, cb, "the {a} and the {b} are drawn in one ink: {drawn:?}");
        }
    }
}

// ── Acceptance 3 ────────────────────────────────────────────────────────────────────────────

/// A paragraph of one line, long enough to be several rows at any size the editor is drawn at.
const PARAGRAPH_ONCE: &str = "alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa quebec romeo sierra tango uniform victor whiskey xray yankee zulu alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima mike november oscar papa quebec romeo sierra tango uniform victor whiskey xray yankee zulu";

/// Four times the paragraph: a line of well over a thousand characters, many rows wide at the size the test draws at.
fn paragraph() -> String {
    [PARAGRAPH_ONCE; 4].join(" ")
}

/// The column each drawn row starts at in its line: the lengths of the rows before it, summed.
fn row_starts(app: &mut App) -> Vec<usize> {
    let mut at = 0;
    rows(app)
        .iter()
        .map(|(words, _)| {
            let start = at;
            at += words.chars().count();
            start
        })
        .collect()
}

/// **Up and Down go by drawn row, and an edit, a selection and an undo on a wrapped line are as on any line.**
#[test]
fn up_and_down_go_by_row_and_a_wrapped_line_takes_an_edit_a_selection_and_an_undo() {
    let mut app = files_editor("edit", 1.0, &format!("{}\nnext line\n", paragraph()));
    let original = text(&mut app);
    let paragraph = paragraph();
    let starts = row_starts(&mut app);
    let row_of_paragraph: Vec<usize> = starts.iter().copied().take_while(|s| *s < paragraph.len()).collect();
    assert!(row_of_paragraph.len() >= 3, "the paragraph wraps to at least three rows: {starts:?}");

    // Down moves one drawn row and keeps the column; Up goes back.
    caret_at(&mut app, 0, 4);
    bevy_gooey::testing::press(&mut app, KeyCode::ArrowDown);
    assert_eq!(caret(&mut app), CodePos::new(0, row_of_paragraph[1] + 4), "Down: the next drawn row, the same column ({starts:?})");
    bevy_gooey::testing::press(&mut app, KeyCode::ArrowDown);
    assert_eq!(caret(&mut app), CodePos::new(0, row_of_paragraph[2] + 4), "Down again: the row after");
    bevy_gooey::testing::press(&mut app, KeyCode::ArrowUp);
    assert_eq!(caret(&mut app), CodePos::new(0, row_of_paragraph[1] + 4), "Up: back a drawn row");

    // An edit in the middle of a row lands there, and the wrap is redrawn from the new text.
    let at = row_of_paragraph[1] + 4;
    bevy_gooey::testing::type_text(&mut app, "ZZ");
    settle_support::settle(&mut app);
    let mut edited = original.clone();
    edited.insert_str(at, "ZZ");
    assert_eq!(text(&mut app), edited, "the typed letters are in the line where the caret stood, and the text has no wrap in it");
    let drawn: String = rows(&mut app).iter().map(|(words, _)| words.as_str()).collect();
    assert!(drawn.starts_with(&edited[..paragraph.len() + 2]), "the rows draw the edited line whole");
    assert_eq!(caret(&mut app), CodePos::new(0, at + 2), "and the caret is after the letters");

    // A selection across a wrap is replaced by what is typed.
    select(&mut app, CodePos::new(0, row_of_paragraph[1] - 2), CodePos::new(0, row_of_paragraph[1] + 3));
    bevy_gooey::testing::type_text(&mut app, "Q");
    settle_support::settle(&mut app);
    let mut replaced = edited.clone();
    replaced.replace_range(row_of_paragraph[1] - 2..row_of_paragraph[1] + 3, "Q");
    assert_eq!(text(&mut app), replaced, "a selection that crosses a wrap is one selection, and typing replaces all of it");

    // Undo takes back both edits, and the text is the file's again.
    for _ in 0..8 {
        if text(&mut app) == original {
            break;
        }
        chord_ctrl(&mut app, KeyCode::KeyZ, "z");
    }
    assert_eq!(text(&mut app), original, "undo restores the text as the file has it");
}

// ── Acceptance 4 ────────────────────────────────────────────────────────────────────────────

/// Whether the editor wraps and draws Markdown: `(CodeSoftWrap, CodeMarkdown)`.
fn marks(app: &mut App) -> (bool, bool) {
    let editor = editor_of(app);
    (app.world().get::<CodeSoftWrap>(editor).is_some(), app.world().get::<CodeMarkdown>(editor).is_some())
}

/// **The wrap and the ink are chosen by the file shown**: a page has both, a `.txt` file wraps and is not drawn as Markdown, a map has
/// neither, and the page again has both.
#[test]
fn the_wrap_and_the_ink_follow_the_file_the_editor_shows() {
    let mut app = files_editor("follow", 1.0, MARKED);
    assert_eq!(marks(&mut app), (true, true), "a page is wrapped and drawn as Markdown");
    files::open_file(app.world_mut(), TXT).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    assert_eq!(marks(&mut app), (true, false), "a text file is prose that wraps, and its asterisks are not markup");
    files::open_file(app.world_mut(), MAP).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    assert_eq!(marks(&mut app), (false, false), "a map is a code file: neither");
    files::open_file(app.world_mut(), PAGE).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    assert_eq!(marks(&mut app), (true, true), "the page again: both");
}

/// **An unsaved edit survives going to a map and back**: the page is still unsaved, its text is the person's, and the caret is where it was
/// left. This is what a tab round trip keeps (the `Files` buffer holds the text); the wrap components are not what keeps it, so the test
/// would pass without them. It then checks the page still wraps and moves by drawn row after the round trip, which would not.
#[test]
fn an_unsaved_edit_survives_a_tab_change_between_a_page_and_a_map() {
    let mut app = files_editor("unsaved", 1.0, &format!("{}\nnext line\n", paragraph()));
    caret_at(&mut app, 0, 10);
    bevy_gooey::testing::type_text(&mut app, "ZZ");
    settle_support::settle(&mut app);
    let edited = text(&mut app);
    assert!(edited.starts_with(&format!("{}ZZ", &paragraph()[..10])), "the edit is in the text: {}", &edited[..20]);
    assert!(app.world().resource::<Files>().is_dirty(PAGE), "the page is unsaved");
    select(&mut app, CodePos::new(0, 3), CodePos::new(0, 8));

    files::open_file(app.world_mut(), MAP).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    assert_ne!(text(&mut app), edited, "the map is showing");
    assert!(app.world().resource::<Files>().is_dirty(PAGE), "going to the map did not save the page");

    files::open_file(app.world_mut(), PAGE).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    assert_eq!(text(&mut app), edited, "the page is back with the person's unsaved edit in it");
    let held = app.world().resource::<Files>().get(PAGE).map(|f| (f.text.clone(), f.disk.clone())).unwrap_or_default();
    assert_eq!(held.0, edited, "and the buffer holds it");
    assert_ne!(held.1, edited, "while the disk still holds the file as it was");
    let at = caret(&mut app);
    assert_eq!(at, CodePos::new(0, 8), "the caret is where the person left it (the end of the selection set before the tab change)");
    // The page still edits as a wrapped page: the caret moves by drawn row and a letter lands where the caret is.
    caret_at(&mut app, 0, 4);
    bevy_gooey::testing::press(&mut app, KeyCode::ArrowDown);
    assert!(caret(&mut app).col > 4, "Down moves by a drawn row after the round trip: {:?}", caret(&mut app));
}

// ── Acceptance 5 ────────────────────────────────────────────────────────────────────────────

/// **One column of the editor's face**, physical px: the width of gooey's hidden measuring row over the characters it holds.
fn column_px(app: &mut App) -> f32 {
    let editor = editor_of(app);
    let world = app.world();
    let probe = descendants(world, editor)
        .into_iter()
        .find(|e| world.get::<Role>(*e) == Some(&Role::CodeText) && world.get::<Visibility>(*e) == Some(&Visibility::Hidden))
        .unwrap_or_else(|| panic!("the editor measures its column on a hidden row"));
    let chars = world.get::<Text>(probe).map_or(0, |t| t.0.chars().count());
    let width = border_box(world, probe).map_or(0.0, |b| b.width());
    assert!(chars > 0 && width > 0.0, "the measuring row holds characters and is laid out: {chars} in {width} px");
    width / chars as f32
}

/// **A wrapped list item's later rows start under its text, not at the editor's left edge** (#612, the owner's pick of 2026-11-27:
/// `CodeWrapIndent::Same`). `- ` is two columns, so every row of the item after its first starts two columns right of the first
/// row, which starts with the `-`. The paragraph on the next line has no marker, so its rows start where the item's first row
/// does: the hang is the item's, not every wrapped row's. Read from each row's box against the editor's own column, rounded to
/// whole columns, so a pixel of layout rounding cannot move a row by a column and a hang of one column or none cannot pass.
#[test]
fn a_wrapped_list_items_later_rows_start_under_its_text_and_a_paragraphs_at_the_edge() {
    let item = format!("- {}", paragraph());
    let page = format!("{item}\n{}\n", paragraph());
    let mut app = files_editor("hang", 1.0, &page);
    let editor = editor_of(&mut app);
    assert_eq!(app.world().get::<CodeWrapIndent>(editor), Some(&CodeWrapIndent::Same), "a page's wrapped lines hang under their first word");
    let column = column_px(&mut app);
    let drawn = drawn_rows(&mut app);
    // The rows of each line are the ones whose words add up to it.
    let end_of = |from: usize, len: usize| -> usize {
        let (mut end, mut taken) = (from, 0);
        while end < drawn.len() && taken < len {
            taken += drawn[end].1.chars().count();
            end += 1;
        }
        assert_eq!(taken, len, "the rows from row {from} draw a line of {len} characters whole");
        end
    };
    let item_end = end_of(0, item.chars().count());
    let paragraph_end = end_of(item_end, paragraph().chars().count());
    let (item_rows, paragraph_rows) = (&drawn[..item_end], &drawn[item_end..paragraph_end]);
    assert!(item_rows.len() >= 3 && paragraph_rows.len() >= 3, "the item and the paragraph each wrap: {} and {} rows", item_rows.len(), paragraph_rows.len());
    assert!(item_rows[0].1.starts_with("- "), "the item's first row starts with its marker: {:?}", item_rows[0].1);

    let world = app.world();
    let left = |row: Entity| border_box(world, row).unwrap_or_else(|| panic!("a drawn row is laid out")).min.x;
    let edge = left(item_rows[0].0);
    let column_of = |row: Entity| ((left(row) - edge) / column).round() as i32;
    for (k, (row, words)) in item_rows.iter().enumerate().skip(1) {
        let at = column_of(*row);
        assert_eq!(at, 2, "the item's row {k} ({words:?}) starts at column {at}, not under the item's text at column 2 ({column} px a column)");
    }
    for (k, (row, words)) in paragraph_rows.iter().enumerate() {
        let at = column_of(*row);
        assert_eq!(at, 0, "the paragraph's row {k} ({words:?}) starts at column {at}: a line with no marker does not hang");
    }
    assert_eq!(text(&mut app), page, "a hang is drawn, never written");
}

/// **The host's primary modifier key** (#638): the editor's chords are `Modifiers::PRIMARY`, which is `Control` on
/// Linux and `Command` on macOS. This suite only ever ran on Linux, so a hardcoded `ControlLeft` passed everywhere
/// it ran and would have failed on a Mac -- which is why `big_mac` could not be a second gate host.
fn primary_key() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}
