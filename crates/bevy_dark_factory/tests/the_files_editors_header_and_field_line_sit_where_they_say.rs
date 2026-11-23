//! **The Files editor's header reads at the row's two ends, and its field line is one control** (#542).
//!
//! # What the page drew before
//!
//! Measured on the capture at `0c74e140` (1360 x 850, interface scale 1.0), a pixel-run scan of the header row
//! `y=182..198` found the path's runs at `477..749` and `Ln 1, Col 1` at **800..835** inside a row that was
//! **462..1344** wide: the readout stood 500 px from the row's right edge, because the path took the row's free width
//! and the position sat against the path's end. Under it a bordered box stood at **1164..1316** while its own
//! placeholder drew at **489..681** — 483 px to the left of the box it belonged to, because `text_input` lays its
//! label and its field out at the row's two ends and nothing tied the words to the frame.
//!
//! # What this measures
//!
//! Every number below is read from the laid-out nodes, so the test states the page rather than describing it:
//! the position's right edge against the row's, the field's words inside the field's own box, and the field line
//! carrying a style class of its own.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files::{self, FieldLine, FileHeader, FilePosition};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_gooey::{Role, StyleClass, TextHint};

use crate::fixtures;

const SETTLE: usize = 16;

/// A page to open: three lines, so the readout has a line and a column to report.
const PAGE: &str = "# A heading\n\nsome words\n";

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

/// **The Files workspace, with its page laid out and one file open.**
fn page(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("files_header_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let rel = "design/header-row.md";
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, PAGE).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    harness::resize_surface(&mut app, 1400, 900).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(&mut app);
    files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

/// A node's rect in logical px: `(left, top, right, bottom)`.
fn rect(app: &App, entity: Entity) -> Option<(f32, f32, f32, f32)> {
    let computed = app.world().get::<ComputedNode>(entity)?;
    let at = app.world().get::<UiGlobalTransform>(entity)?.translation;
    let scale = computed.inverse_scale_factor();
    let (w, h) = (computed.size().x * scale, computed.size().y * scale);
    let (cx, cy) = (at.x * scale, at.y * scale);
    Some((cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0))
}

/// The one node carrying `T`.
fn marked<T: Component>(app: &mut App, what: &str) -> Entity {
    let world = app.world_mut();
    let mut query = world.query_filtered::<Entity, With<T>>();
    let found: Vec<Entity> = query.iter(world).collect();
    assert_eq!(found.len(), 1, "one node carries {what}: {found:?}");
    found[0]
}

/// **`Ln n, Col m` stands at the row's right edge, not at the path's end.**
#[test]
fn the_caret_readout_is_right_aligned_on_the_header_row() {
    let mut app = page("right");
    let header = marked::<FileHeader>(&mut app, "the path");
    let position = marked::<FilePosition>(&mut app, "the position readout");
    let position_rect = rect(&app, position).expect("the readout is laid out");
    let header_rect = rect(&app, header).expect("the path is laid out");
    // The row that holds both: the path's parent.
    let row = app.world().get::<ChildOf>(header).map(|parent| parent.parent()).expect("the path stands in a row");
    let row_rect = rect(&app, row).expect("the row is laid out");
    println!(
        "measured: row {:.0}..{:.0}, path {:.0}..{:.0}, readout {:.0}..{:.0}",
        row_rect.0, row_rect.2, header_rect.0, header_rect.2, position_rect.0, position_rect.2
    );
    assert!(
        row_rect.2 - position_rect.2 <= 2.0,
        "**the readout's right edge is {:.0} px from the row's** (#542): row right {:.0}, readout right {:.0}. \
         The path grows into the row and the readout keeps its words' width, so the readout ends at the row's edge.",
        row_rect.2 - position_rect.2,
        row_rect.2,
        position_rect.2
    );
    assert!(
        position_rect.0 > header_rect.2,
        "**the readout stands past the path's end** (#542): path ends {:.0}, readout starts {:.0}.",
        header_rect.2,
        position_rect.0
    );
}

/// **The field line's words are inside the field line's own box, and it says what it sets.**
#[test]
fn the_field_lines_words_stand_inside_its_own_box() {
    let mut app = page("field");
    let line = marked::<FieldLine>(&mut app, "the field line");
    let line_rect = rect(&app, line).expect("the field line is laid out");
    // The field and its hint are the row's own children (the hint stands inside the field's cell).
    let world = app.world();
    let direct: Vec<Entity> = world.get::<Children>(line).map(|c| c.iter().collect()).unwrap_or_default();
    let mut family: Vec<Entity> = direct.clone();
    for child in &direct {
        if let Some(grandchildren) = world.get::<Children>(*child) {
            family.extend(grandchildren.iter());
        }
    }
    let field = family
        .iter()
        .copied()
        .find(|entity| world.get::<Role>(*entity) == Some(&Role::TextField))
        .expect("the row holds a text field");
    let hint = family
        .iter()
        .copied()
        .find(|entity| world.get::<Role>(*entity) == Some(&Role::TextHint))
        .expect("the row holds a hint");
    let field_rect = rect(&app, field).expect("the field is laid out");
    let hint_rect = rect(&app, hint).expect("the hint is laid out while the field is empty");
    println!(
        "measured: line {:.0}..{:.0}, field {:.0}..{:.0}, words {:.0}..{:.0}",
        line_rect.0, line_rect.2, field_rect.0, field_rect.2, hint_rect.0, hint_rect.2
    );
    assert!(
        hint_rect.0 >= field_rect.0 && hint_rect.2 <= field_rect.2,
        "**the field's words are inside its own box** (#542): field {:.0}..{:.0}, words {:.0}..{:.0}. Before, the \
         words stood 483 px left of the box they belonged to and a person could not read them as one control.",
        field_rect.0,
        field_rect.2,
        hint_rect.0,
        hint_rect.2
    );
    assert!(
        hint_rect.0 >= line_rect.0 && hint_rect.2 <= line_rect.2,
        "and inside the field line's own box: line {:.0}..{:.0}, words {:.0}..{:.0}",
        line_rect.0,
        line_rect.2,
        hint_rect.0,
        hint_rect.2
    );
    // **The control names itself** (#542): `blueprint/ui_style_inspect` answers with the topmost *classed* node, and
    // before this the only class over the box was the page card's.
    let classes = app.world().get::<StyleClass>(line).expect("the field line carries style classes");
    assert!(
        classes.0.iter().any(|class| class == "chrome-field-line"),
        "the field line carries a class of its own, so the inspector names the control and not the card: {:?}",
        classes.0
    );
    let words = app.world().get::<TextHint>(line).map(|hint| hint.0.clone()).unwrap_or_default();
    assert_eq!(
        words,
        files::FIELD_LINE_HINT,
        "the hint is the sentence the module names, so the control and its words cannot drift"
    );
    assert!(
        !words.trim().is_empty() && !words.starts_with("Set a field"),
        "the hint says what to type, in a person's words, rather than repeating the old label: {words:?}"
    );
    let shown = app.world().get::<Text>(hint).map(|text| text.0.clone()).unwrap_or_default();
    assert_eq!(shown, words, "the drawn hint is the row's hint");
    assert!(
        field_rect.2 > field_rect.0 && field_rect.3 > field_rect.1,
        "the field has a box a person can see: {field_rect:?}"
    );
}
