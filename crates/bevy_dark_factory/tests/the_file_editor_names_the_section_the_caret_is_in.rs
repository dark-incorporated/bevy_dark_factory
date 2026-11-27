//! **The file editor names the section the caret stands in** (#612, the owner's pick of 2026-11-27 from the polish
//! candidates: *"a breadcrumb for the current symbol/section"*).
//!
//! # What the editor did before
//!
//! The row over the editor said the file's path and `Ln, Col`, and nothing said which part of a page the caret was in. The
//! outline (View ▸ Show outline) lists every heading but not the one that holds the caret, so a person reading deep in a long
//! design page scrolled up to learn where they were. Measured before this change: no text in the Files workspace named a
//! heading of the open page, wherever the caret stood. VS Code and Obsidian draw the chain of sections as a breadcrumb.
//!
//! # What this measures
//!
//! A row over the editor reads the page's name and every heading whose section holds the caret, outermost first. It follows
//! the caret into a nested section, out to a sibling and to a new top-level section; it is drawn above the code editor, inside
//! the page; and a file with no headings draws no such row, and the row comes back with the page.

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files::{self, FileBreadcrumb, FilesHost};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_gooey::{CodeEditor, CodePos};

use crate::fixtures;
use crate::settle_support;

const PAGE_REL: &str = "design/notes.md";
const MAP_REL: &str = "assets/blueprint/maps/m.map.yaml";

/// **A page of nested sections with words above the first heading.** Its lines, from 0: the words (0), a blank (1),
/// `# Combat` (2), its words (3), `## Weapons` (4), `### Swords` (5), its words (6), `## Armour` (7), its words (8),
/// `# Travel` (9), its words (10).
const PAGE: &str = concat!(
    "Words before any heading.\n",
    "\n",
    "# Combat\n",
    "Swing words.\n",
    "## Weapons\n",
    "### Swords\n",
    "Edge words.\n",
    "## Armour\n",
    "Plate words.\n",
    "# Travel\n",
    "Road words.\n",
);

/// How far a laid-out box may stand past another, physical px (layout rounds to whole pixels).
const SLACK_PX: f32 = 1.0;

fn page(tag: &str) -> (App, Entity) {
    let root = fixtures::Fixture::new(&format!("crumb_{tag}_{}", std::process::id()))
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
    let editor = editor_of(&mut app);
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    settle_support::settle(&mut app);
    (app, editor)
}

fn editor_of(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<CodeEditor>>().single(world).unwrap_or_else(|e| panic!("one code editor is open: {e}"))
}

fn stand(app: &mut App, editor: Entity, line: usize, col: usize) {
    let Some(mut widget) = app.world_mut().get_mut::<CodeEditor>(editor) else { panic!("the editor is open") };
    widget.select(CodePos::new(line, col), CodePos::new(line, col));
    settle_support::settle(app);
}

/// **The one breadcrumb's entity**: exactly one text carries [`FileBreadcrumb`], ever.
fn crumb_entity(app: &mut App) -> Entity {
    let world = app.world_mut();
    let found: Vec<Entity> = world.query_filtered::<Entity, With<FileBreadcrumb>>().iter(world).collect();
    assert_eq!(found.len(), 1, "the editor carries one breadcrumb, not a stack: {found:?}");
    found[0]
}

/// **What the breadcrumb says.**
fn crumb(app: &mut App) -> String {
    let entity = crumb_entity(app);
    app.world().get::<Text>(entity).map(|text| text.0.clone()).unwrap_or_default()
}

/// A node's border box, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **Whether the breadcrumb is drawn**: its row is laid out (not `Display::None`) and the crumbs have a height.
fn drawn(app: &mut App) -> bool {
    let entity = crumb_entity(app);
    let world = app.world();
    let row_shown = world.get::<ChildOf>(entity).and_then(|row| world.get::<Node>(row.parent())).is_some_and(|node| node.display != Display::None);
    row_shown && border_box(world, entity).is_some_and(|rect| rect.height() > 0.0)
}

/// **The breadcrumb names the page and every section that holds the caret**, outermost first: in `### Swords`, which is in
/// `## Weapons`, which is in `# Combat`.
#[test]
fn the_breadcrumb_names_every_section_that_holds_the_caret() {
    let (mut app, editor) = page("nested");
    stand(&mut app, editor, 6, 2);
    assert_eq!(crumb(&mut app).as_str(), "notes.md \u{203A} Combat \u{203A} Weapons \u{203A} Swords");
    assert!(drawn(&mut app), "the breadcrumb of a page with headings is drawn");
}

/// **It follows the caret**: out of `### Swords` to its sibling's sibling `## Armour` (the deeper sections close), to the
/// next top-level section, onto a heading's own line, and above the first heading, where only the page is named.
#[test]
fn the_breadcrumb_follows_the_caret_from_section_to_section() {
    let (mut app, editor) = page("follow");
    let walk = [
        ((8, 3), "notes.md \u{203A} Combat \u{203A} Armour"),
        ((10, 0), "notes.md \u{203A} Travel"),
        ((2, 0), "notes.md \u{203A} Combat"),
        ((4, 5), "notes.md \u{203A} Combat \u{203A} Weapons"),
        ((0, 4), "notes.md"),
    ];
    for ((line, col), want) in walk {
        stand(&mut app, editor, line, col);
        assert_eq!(crumb(&mut app).as_str(), want, "the caret on line {line} is in {want:?}");
        assert!(drawn(&mut app), "the breadcrumb stays drawn while the page has headings, the caret on line {line}");
    }
}

/// **The breadcrumb is drawn over the editor, inside the page**: its box ends where the code editor's begins or above it,
/// and it stands inside the editor's page horizontally.
#[test]
fn the_breadcrumb_is_drawn_above_the_editor_inside_the_page() {
    let (mut app, editor) = page("above");
    stand(&mut app, editor, 6, 0);
    let entity = crumb_entity(&mut app);
    let world = app.world_mut();
    let host = world.query_filtered::<Entity, With<FilesHost>>().single(world).unwrap_or_else(|e| panic!("one editor page: {e}"));
    let world = app.world();
    let crumb_box = border_box(world, entity).unwrap_or_else(|| panic!("the breadcrumb is laid out"));
    let editor_box = border_box(world, editor).unwrap_or_else(|| panic!("the editor is laid out"));
    let host_box = border_box(world, host).unwrap_or_else(|| panic!("the page is laid out"));
    assert!(crumb_box.height() > 0.0, "the breadcrumb has a height: {crumb_box:?}");
    assert!(
        crumb_box.max.y <= editor_box.min.y + SLACK_PX,
        "the breadcrumb {crumb_box:?} is drawn above the editor {editor_box:?}"
    );
    assert!(
        crumb_box.min.x >= host_box.min.x - SLACK_PX && crumb_box.max.x <= host_box.max.x + SLACK_PX,
        "the breadcrumb {crumb_box:?} stands inside the page {host_box:?}"
    );
}

/// **A file with no headings draws no breadcrumb**: a map has no sections, so the row is not drawn and takes no line from
/// the editor; the page opened again draws it again, naming where its caret is.
#[test]
fn a_file_without_headings_draws_no_breadcrumb_and_the_page_draws_it_again() {
    let (mut app, editor) = page("map");
    stand(&mut app, editor, 6, 0);
    assert!(drawn(&mut app), "the page draws its breadcrumb");
    files::open_file(app.world_mut(), MAP_REL).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    assert!(!drawn(&mut app), "a map has no headings, so no breadcrumb is drawn: {:?}", crumb(&mut app));
    assert_eq!(crumb(&mut app).as_str(), "", "the breadcrumb says nothing over a file with no sections");
    files::open_file(app.world_mut(), PAGE_REL).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    assert!(drawn(&mut app), "the page draws its breadcrumb again");
    assert!(crumb(&mut app).starts_with("notes.md"), "the breadcrumb names the page again: {:?}", crumb(&mut app));
}
