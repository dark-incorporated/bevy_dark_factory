//! **The file editor says where the caret stands** (#525, `feedback-1795094391027`: *"This markdown editor is not great.
//! Please look at well polished editors like VSCode, PyCharm, Obsidian..."*).
//!
//! # What the editor did before
//!
//! VS Code, PyCharm and Obsidian all draw `Ln 7, Col 12` in a bar beside the file. This editor's header drew the file's
//! path and its dirty dot and nothing else — measured by moving the caret to `(2, 19)` and to `(9, 6)` and reading the
//! header both times: it said `design/style.md` and only `design/style.md`. The caret was the only mark of position, so a
//! person reading a long page had no number to work with.
//!
//! # What this measures
//!
//! The header's own row now carries a second text, `Ln <line>, Col <column>`, and it **follows the caret** — moved to a
//! line and a column, the readout says that line and that column. `Ln` is one-based (a person counts from 1) and `Col`
//! is the display column the caret is drawn at, so the number and the mark cannot disagree.

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files::{self, FilePosition};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_gooey::{CodeEditor, CodePos};

use crate::fixtures;
use crate::settle_support;


/// A page with a line long enough that a column past nine exists, and a tab so the display column can differ from the byte.
const PAGE: &str = "# A heading\n\n*Emphasised words.*\n\n\ttabbed start then words\n";



/// **A page on a real device**, because the headless one lays nothing out (#542).
///
/// `Device::None` registers every render type and creates no device, so no `ComputedNode` has a size —
/// measured on this tree, every node read `w=0`. The issue's numbers (`x 830..835` in a row `462..1344`
/// wide) came from a staged editor with a real window, and a geometry assertion needs the same.
fn page_on_a_real_device(tag: &str) -> (App, Entity) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("design/style", "alpha")
        .build("m");
    let rel = "design/style.md";
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, PAGE).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_stepped_at(
        &root,
        "m",
        None,
        bevy_dark_factory::tiles::Mode::default(),
        harness::Device::Real,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    harness::resize_surface(&mut app, 1400, 900).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let editor = app
        .world_mut()
        .query_filtered::<Entity, With<bevy_gooey::CodeEditor>>()
        .single(app.world_mut())
        .unwrap_or_else(|e| panic!("{e}"));
    (app, editor)
}

fn page(tag: &str) -> (App, Entity) {
    let root = fixtures::Fixture::new(&format!("md_position_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let rel = "design/style.md";
    let full = root.join(rel);
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
    files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let editor = app.world_mut().query_filtered::<Entity, With<CodeEditor>>().single(app.world_mut()).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    settle_support::settle(&mut app);
    (app, editor)
}

/// **What the position readout says** — the one text carrying the [`FilePosition`] marker.
fn readout(app: &mut App) -> String {
    let world = app.world_mut();
    let mut q = world.query_filtered::<&Text, With<FilePosition>>();
    q.iter(world).map(|t| t.0.clone()).collect::<Vec<_>>().join(" | ")
}

fn stand(app: &mut App, editor: Entity, line: usize, col: usize) {
    let Some(mut widget) = app.world_mut().get_mut::<CodeEditor>(editor) else { panic!("the editor is open") };
    widget.select(CodePos::new(line, col), CodePos::new(line, col));
    settle_support::settle(app);
}

/// **The readout exists and says where the caret stands**, moved to a line and column.
#[test]
fn the_header_reads_the_line_and_column_the_caret_stands_on() {
    let (mut app, editor) = page("stand");
    stand(&mut app, editor, 2, 10);
    assert_eq!(readout(&mut app).as_str(), "Ln 3, Col 11", "the caret at line 2 col 10 reads one-based as Ln 3, Col 11");
}

/// **It follows the caret**: move to another line and column and the one text says the new place — not a second readout
/// added beside the first.
#[test]
fn the_readout_follows_the_caret() {
    let (mut app, editor) = page("follow");
    stand(&mut app, editor, 0, 0);
    let first = readout(&mut app);
    stand(&mut app, editor, 2, 19);
    let second = readout(&mut app);
    assert_ne!(first, second, "the readout moved with the caret: {first:?} then {second:?}");
    assert_eq!(second.as_str(), "Ln 3, Col 20");
    // Exactly one readout, ever: a second added beside the first would be the drift this replaces.
    let world = app.world_mut();
    let count = world.query_filtered::<Entity, With<FilePosition>>().iter(world).count();
    assert_eq!(count, 1, "the header carries one position readout, not a stack");
}

/// **`Col` is the display column**, the column the caret is drawn at, so a tab takes its width and not one. The caret
/// just past a leading tab on line 4 stands at `Col 5`, not `Col 2` — the number and the drawn mark are one thing.
#[test]
fn the_column_is_the_display_column_so_a_tab_counts_its_width() {
    let (mut app, editor) = page("tab");
    stand(&mut app, editor, 4, 1);
    assert_eq!(
        readout(&mut app).as_str(),
        "Ln 5, Col 5",
        "one byte past a leading four-column tab is the caret's fifth display column"
    );
}

/// **The readout stands on the row's right edge, not against the path's end** (#542).
///
/// `TextLayout::new(Justify::Right, …)` justifies words within their own box. A box that only measures
/// its words has nothing to justify, and the path beside it — `TextFlow { grow: Some(1.0) }` — took the
/// row's free width, so the readout sat at the **path's** end: measured at 1600×1000 it was 500 px short
/// of the row's right edge, `x 830..835` inside a row `462..1344` wide.
///
/// **This needs a real device.** The headless editor registers render types and creates no device, so
/// no node is laid out and every rect reads zero; the assertion would pass vacuously against nothing.
#[test]
fn the_readout_stands_on_the_rows_right_edge() {
    let (mut app, _editor) = page_on_a_real_device("caret_row_edge");
    settle_support::settle(&mut app);

    let (position, row) = {
        let world = app.world_mut();
        let position = world
            .query_filtered::<Entity, With<FilePosition>>()
            .iter(world)
            .next()
            .expect("the page draws a position readout");
        // **The readout's own row, by parent**: a page can hold more than one file header, so "the
        // first `FileHeader`" pairs the readout with a row it is not in.
        let row = world
            .get::<ChildOf>(position)
            .map(|p| p.parent())
            .expect("the readout stands in a row");
        (position, row)
    };
    let rect = |entity: Entity| -> (f32, f32, f32) {
        let world = app.world();
        let node = world.get::<ComputedNode>(entity).expect("a laid-out node");
        let at = world
            .get::<bevy::ui::UiGlobalTransform>(entity)
            .expect("a laid-out transform");
        let r = bevy_dark_factory::ops_brp::node_rect(node, at);
        (r.x, r.w, r.h)
    };
    let (readout_left, readout_width, readout_height) = rect(position);
    let (row_left, row_width, row_height) = rect(row);

    // **The guard against a vacuous pass**: a headless build lays nothing out, and every assertion
    // below would hold against two zero-width boxes.
    assert!(
        row_width > 100.0 && readout_height > 0.0,
        "the page was not laid out (row {row_width} px wide, readout {readout_height} px tall), so this \
         test would prove nothing — it needs `Device::Real` (#542)"
    );

    let readout_right = readout_left + readout_width;
    let row_right = row_left + row_width;
    assert!(
        (row_right - readout_right).abs() <= 2.0,
        "**the position readout ends {:.0} px short of its row's right edge** (#542): the readout spans \
         {readout_left:.0}..{readout_right:.0}, the row {row_left:.0}..{row_right:.0} (tall {row_height:.0}). \
         `TextLayout::Right` justifies within the readout's own box, so it needs a box wider than its \
         words: `TextFlow {{ align: Some(AlignSelf::End) }}` puts it on the far edge without taking any of \
         the free width.",
        row_right - readout_right
    );
}
