//! **The Design page's picture fits the column, and Edit mode keeps the page's width** (#437). Found recording the design workspace clip:
//!
//! 1. an inline page picture was wider than the page column and overlapped the scrollbar (by about 12 px);
//! 2. in Edit mode the page column shrank to about 170 px and clipped the "As an agent reads it" toggle, while the area to its left stayed empty.
//!
//! Both were measured at the two smallest windows the editor supports, 1024 x 640 and 1280 x 720. The second had a cause outside the page: the
//! arrangement dock (the map's drop target) stood in the viewport slot as a `width: 100%` sibling of the page, so the page shared the row with an empty box.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_cauldron::design_view::{DesignAgentToggle, DesignHost, PAGE_GUTTER_PX};
use bevy_cauldron::harness;
use bevy_cauldron::keys::{Action, binding};
use bevy_gooey::{CodeEditor, MarkdownPicture, MarkdownView};

use crate::fixtures;

/// The windows, in logical px as the harness's surface is sized (the editor's interface scale makes the layout narrower than this).
const WINDOWS_PX: [(u32, u32); 2] = [(1024, 640), (1280, 720)];
/// Frames run after each step, so layout settles.
const SETTLE: usize = 30;
/// One frame of the test clock, in seconds.
const FRAME_S: f32 = 1.0 / 60.0;
/// Rounding the layout may leave on an edge, in logical px.
const SLACK_PX: f32 = 0.5;
/// The picture's size in pixels: twice as wide as it is high, and far wider than the page column.
const PICTURE_PX: (u32, u32) = (1600, 800);
const PICTURE: &str = "design/board/wide.png";

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn write(root: &std::path::Path, rel: &str, text: &str) {
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().expect("a folder")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, text).unwrap_or_else(|e| panic!("{e}"));
}

/// A node's rect in logical px: `(left, top, right, bottom)`.
fn rect(app: &App, entity: Entity) -> (f32, f32, f32, f32) {
    let computed = app.world().get::<ComputedNode>(entity).unwrap_or_else(|| panic!("{entity} is laid out"));
    let at = app.world().get::<UiGlobalTransform>(entity).unwrap_or_else(|| panic!("{entity} has a place")).translation;
    let scale = computed.inverse_scale_factor();
    let (w, h) = (computed.size().x * scale, computed.size().y * scale);
    let (cx, cy) = (at.x * scale, at.y * scale);
    (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0)
}

fn press(app: &mut App, action: Action) {
    let row = binding(action);
    let mut keys = vec![row.key];
    if row.needs_mod {
        keys.push(if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft });
    }
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in &keys {
            app.world_mut().write_message(KeyboardInput {
                key_code: *code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
        app.update();
    }
    settle(app);
}

/// The Design workspace open on a game page that cites the wide picture, at `window`.
fn design(window: (u32, u32)) -> App {
    let root = fixtures::Fixture::new(&format!("design_fit_{}_{}", window.0, std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    write(&root, "design/style.md", "# The game\n\nGrey concrete, sodium light.\n\n![the fog, far wider than the column](board/wide.png \"fog\")\n\nAfter.\n");
    let full = root.join(PICTURE);
    std::fs::create_dir_all(full.parent().expect("a folder")).unwrap_or_else(|e| panic!("{e}"));
    image::RgbaImage::from_pixel(PICTURE_PX.0, PICTURE_PX.1, image::Rgba([200, 120, 40, 255])).save(&full).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, window.0, window.1).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME_S)));
    settle(&mut app);
    press(&mut app, Action::WorkspaceDesign);
    app
}

fn one<F: bevy::ecs::query::QueryFilter>(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, F>().single(world).unwrap_or_else(|e| panic!("one such node: {e}"))
}

/// **A picture on the page ends inside the column, a gutter short of its right edge** (clear of the scrollbar), never wider than the page, and keeps its shape.
#[test]
fn a_page_picture_fits_the_column_and_clears_the_scrollbar_at_both_sizes() {
    for window in WINDOWS_PX {
        let mut app = design(window);
        let page = one::<With<MarkdownView>>(&mut app);
        let picture = one::<(With<MarkdownPicture>, With<ImageNode>)>(&mut app);
        let (page_rect, pic) = (rect(&app, page), rect(&app, picture));
        let column_right = page_rect.2;
        assert!(
            pic.2 <= column_right - PAGE_GUTTER_PX + SLACK_PX,
            "{window:?}: the picture's right edge {:.1} is past the column's inner right edge {:.1} less the {PAGE_GUTTER_PX} px gutter (the page is {page_rect:?}, the picture {pic:?})",
            pic.2,
            column_right - PAGE_GUTTER_PX
        );
        assert!(pic.0 >= page_rect.0 - SLACK_PX, "{window:?}: the picture starts inside the column: {pic:?} in {page_rect:?}");
        let (w, h) = (pic.2 - pic.0, pic.3 - pic.1);
        assert!(w > 0.0 && h > 0.0, "{window:?}: the picture is drawn: {pic:?}");
        let ratio = w / h;
        let want = PICTURE_PX.0 as f32 / PICTURE_PX.1 as f32;
        assert!((ratio - want).abs() <= 0.05, "{window:?}: the picture keeps its shape: {w:.1} x {h:.1} is {ratio:.2}:1, the file is {want:.2}:1");
    }
}

/// **Edit mode keeps the page at its normal width**: the column is not narrower than the view mode's, the editor and the "As an agent reads it" toggle are
/// inside it, and nothing stands empty to its left.
#[test]
fn edit_mode_keeps_the_pages_width_and_the_agent_toggle_inside_it_at_both_sizes() {
    for window in WINDOWS_PX {
        let mut app = design(window);
        let host = one::<With<DesignHost>>(&mut app);
        let viewing = rect(&app, host);
        let view_width = viewing.2 - viewing.0;

        let edit = bevy_gooey::testing::find_by_label(&mut app, "Edit page").unwrap_or_else(|e| panic!("{window:?}: {e}"));
        bevy_gooey::testing::click(&mut app, edit);
        settle(&mut app);

        let editing = rect(&app, host);
        let edit_width = editing.2 - editing.0;
        assert!(edit_width >= view_width - SLACK_PX, "{window:?}: Edit mode squeezes the page column to {edit_width:.1} px, from {view_width:.1} in view mode");
        assert!((editing.0 - viewing.0).abs() <= SLACK_PX, "{window:?}: the column's left edge moved from {:.1} to {:.1}: the area to its left is empty", viewing.0, editing.0);

        let toggle_node = one::<With<DesignAgentToggle>>(&mut app);
        let toggle = rect(&app, toggle_node);
        assert!(
            toggle.0 >= editing.0 - SLACK_PX && toggle.2 <= editing.2 + SLACK_PX,
            "{window:?}: the 'As an agent reads it' toggle {toggle:?} is not inside its column {editing:?}"
        );
        let editor_node = one::<With<CodeEditor>>(&mut app);
        let editor = rect(&app, editor_node);
        assert!(
            editor.0 >= editing.0 - SLACK_PX && editor.2 <= editing.2 + SLACK_PX,
            "{window:?}: the editor {editor:?} is not inside its column {editing:?}"
        );
    }
}
