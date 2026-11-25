//! **The Files page's caret readout is right-aligned on its row's edge, and the field line is one
//! control whose words sit inside their own box** (#542).
//!
//! # What was measured
//!
//! The 2026-11-23 review, on a 1360 x 850 surface at interface scale 1.0: the header's `Ln 1, Col 1`
//! was drawn at `x 800..835` inside a row that runs `462..1344` — **500 px from the row's right edge**,
//! because the path beside it was the only child that grew and the readout sat against the path's
//! end. Under the header the field line drew a **bordered, empty rounded box 153 x 29 px at
//! `x 1164..1316`** whose placeholder words began at `x 489` — 483 px to the left of the box they
//! belong to, because the row grew to fill the column and pushed the field to the far right while
//! its hint was laid out at the column's start.
//!
//! # What this holds
//!
//! The readout's right edge is the row's right edge, not the path's end, and the field line's words
//! and its box stand **inside the same box** — the field's own node — so a person reading the page
//! sees one control where the report saw a label in one column and a box in another.

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files::{self, FieldLine, FileHeader, FilePosition};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_gooey::{CodeEditor, TextInputOf};

use crate::fixtures;
use crate::settle_support;

/// The surface the review measured on.
const SURFACE: (u32, u32) = (1360, 850);
/// A page with lines to stand on.
const PAGE: &str = "# A heading\n\n*Emphasised words.*\n\n\ttabbed start then words\n";
/// How far, logical px, the readout's right edge may stand from the row's: a glyph's antialiased
/// edge and the row's own padding, not the 500 px the report measured.
const EDGE_SLACK_PX: f32 = 24.0;

fn page(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("files_header_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let rel = "design/style.md";
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, PAGE).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    harness::resize_surface(&mut app, SURFACE.0, SURFACE.1).unwrap_or_else(|e| panic!("{e}"));
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
    app
}

/// A node's drawn rect in **logical** px: `ComputedNode::size()` is physical, the sheet is logical.
fn drawn(app: &App, entity: Entity) -> Rect {
    let node = app.world().get::<ComputedNode>(entity).unwrap_or_else(|| panic!("{entity:?} is laid out"));
    let at = app.world().get::<UiGlobalTransform>(entity).unwrap_or_else(|| panic!("{entity:?} is placed")).translation;
    let scale = node.inverse_scale_factor();
    Rect::from_center_size(at * scale, node.size() * scale)
}

fn only<T: Component>(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<T>>().single(world).unwrap_or_else(|e| panic!("one {}: {e}", std::any::type_name::<T>()))
}

/// **The caret readout ends at the row's right edge**, not at the path's end.
#[test]
fn the_position_readout_is_right_aligned_on_the_header_rows_edge() {
    let mut app = page("right_aligned");
    let position = only::<FilePosition>(&mut app);
    let header = only::<FileHeader>(&mut app);
    let row = app.world().get::<ChildOf>(position).map(|c| c.parent()).expect("the readout stands in the header's row");
    let row_box = drawn(&app, row);
    let position_box = drawn(&app, position);
    let header_box = drawn(&app, header);
    assert!(position_box.width() > 0.0, "the readout is drawn: {position_box:?}");
    assert!(row_box.width() > header_box.width(), "the row is wider than the path in it, or the measurement cannot tell two alignments apart: row {row_box:?}, path {header_box:?}");
    let from_edge = row_box.max.x - position_box.max.x;
    assert!(
        from_edge.abs() <= EDGE_SLACK_PX,
        "the readout's right edge stands {from_edge} px from the row's right edge ({} to {}), not on it: it sits \
         against the path's end instead, which is the 500 px the review measured",
        position_box.max.x,
        row_box.max.x
    );
    // **And the path keeps the left end**, so the fix is not the row spreading its two children.
    let from_left = header_box.min.x - row_box.min.x;
    assert!(from_left.abs() <= EDGE_SLACK_PX, "the path stands {from_left} px from the row's left edge");
}

/// **The field line's words stand inside the field's own box**, not 483 px away in another column.
#[test]
fn the_field_lines_words_stand_inside_its_own_box() {
    let mut app = page("field_line");
    let field_line = only::<FieldLine>(&mut app);
    let box_of_field_line = drawn(&app, field_line);
    // The field's own node — the one whose `TextInputOf` names the field line — and the placeholder
    // gooey draws inside that field's cell: the `TextHint` whose row is this one.
    let (field, hint) = {
        let world = app.world_mut();
        let mut fields = world.query::<(Entity, &TextInputOf)>();
        let field = fields
            .iter(world)
            .filter(|(_, of)| of.0 == field_line)
            .map(|(entity, _)| entity)
            .next()
            .unwrap_or_else(|| panic!("the field line draws no field"));
        let mut hints = world.query::<(Entity, &TextInputOf)>();
        let hint = hints
            .iter(world)
            .filter(|(_, of)| of.0 == field_line)
            .map(|(entity, _)| entity)
            .find(|&hint| world.get::<bevy_gooey::TextHint>(hint).is_some())
            .unwrap_or_else(|| panic!("the field line's placeholder is drawn"));
        (field, hint)
    };
    let field_box = drawn(&app, field);
    assert!(field_box.width() > 0.0 && field_box.height() > 0.0, "the field is drawn: {field_box:?}");
    let hint_box = drawn(&app, hint);
    let inside = hint_box.min.x >= field_box.min.x - 1.0
        && hint_box.max.x <= field_box.max.x + 1.0
        && hint_box.min.y >= field_box.min.y - 1.0
        && hint_box.max.y <= field_box.max.y + 1.0;
    assert!(
        inside,
        "the placeholder {hint_box:?} stands inside the field's box {field_box:?}; the report measured it 483 px to the \
         left of its own box, in another column of the page"
    );
    // **The control is as wide as the row it heads**, not a 153 px box at the page's right edge: the
    // report's box was 153 px in a page some 880 px wide.
    assert!(
        box_of_field_line.width() > 0.0 && field_box.width() > box_of_field_line.width() * 0.5,
        "the field fills the width its row gives it: field {field_box:?}, row {box_of_field_line:?}"
    );
}

/// **The field line has a class of its own**, so `blueprint/ui_style_inspect` over it names the
/// control instead of the page's card.
#[test]
fn the_field_line_carries_a_class_of_its_own() {
    let mut app = page("field_class");
    let field_line = only::<FieldLine>(&mut app);
    let classes = app
        .world()
        .get::<bevy_gooey::StyleClass>(field_line)
        .map(|class| class.0.clone())
        .unwrap_or_default();
    assert!(
        classes.iter().any(|class| class == "chrome-field-line"),
        "the field line's own class names the control the inspector points at: {classes:?}"
    );
    // **And the inspector answers with it at the field's own centre**, which is where the review pointed.
    let centre = drawn(&app, field_line).center();
    let found = bevy_dark_factory::style_inspect::classed_node_at(app.world_mut(), centre).expect("a classed node stands at the field line's own centre");
    let inspection = bevy_dark_factory::style_inspect::inspect(app.world(), found).unwrap_or_else(|why| panic!("{why}"));
    assert!(
        inspection.classes.iter().any(|class| class == "chrome-field-line"),
        "the topmost classed node at the field's centre is the field line, not the page card: {inspection:?}"
    );
}
