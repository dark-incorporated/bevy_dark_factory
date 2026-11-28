//! **A file opened for reading draws its minimap on the frame it is first shown** (#540, `bevy_gooey#117`).
//!
//! The lane is an outline of the file down the code editor's right edge: one bar a line, as long as the
//! line, and a band saying which lines are in view. Until `bevy_gooey` v0.59.0 (`a3585c50`) the parts were
//! grown with `Commands` and then written from a query those commands had not reached, so a file opened
//! for reading — no keystroke, so no buffer revision — drew a flat lane with no bars at all. This file
//! is the editor's half of that: it opens a file, never types, and reads what the lane laid out.
//!
//! The colours a bar and a band's edge are held to are gooey's own (`GooeySkin::minimap_bar`,
//! `minimap_band_edge`, held by its `tests/skin.rs`). Every number in the first three tests is read off the
//! world: a bar's `top` and `width` from its `Node`, the lane's pitch from the bars, the lines in view from
//! `CodeEditor::visible_lines`. The fourth is the picture: the editor on a real device
//! (`harness::Device::Real`), captured the way every capture in the editor is taken (`harness::capture`),
//! and the bar colour read off the frame's pixels in the lane. It runs where there is an adapter (big under
//! Xvfb, a Mac) and says `SKIPPED` where there is none.
//!
//! # The band
//!
//! **The band is the text in view: it starts at the lane's row of the view's first line and is as many
//! rows tall as the view holds lines**, on the frame the file opens and after a scroll with no edit. Until
//! `bevy_gooey` v0.64.1 (`bevy_gooey#123`) it counted its last row as `first_row + drawn - 1` with
//! `drawn = first_row + needed`, `first_row` twice: right at the top of the file, and grown by the distance
//! scrolled after a scroll (measured 44 lines tall in a 20-line view after `scroll_to_line(30)`). The view is
//! moved two ways here: by `CodeEditor::scroll_to_line`, the view's own move, and by a `MouseWheel` turned
//! over the editor, the person's, which gooey's `code_on_wheel` reads through the pointer's `HoverMap`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use bevy::input::InputSystems;
use bevy::input::mouse::{MouseScrollPixelsPerLine, MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::picking::hover::HoverMap;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::PrimaryWindow;

use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files;
use bevy_dark_factory::harness::{self, Device};
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::workspace::Workspace;
use bevy_gooey::{CodeEditor, CodeMinimap, GooeySkin, Role};

use crate::fixtures;
use crate::settle_support;

/// **The pitch the lane draws at**, px: `MINIMAP_BAR_PX * 2.0` in `bevy_gooey`'s `code_overlay.rs`, a bar
/// and the gap under it.
const BAR_PITCH_PX: f32 = 3.0;

/// **How many lines of body the file holds**, before its nine-line YAML header.
const BODY_LINES: usize = 60;

/// A project whose one map file is longer than the window, and the file's path.
fn project(tag: &str) -> (PathBuf, String) {
    const REL: &str = "assets/blueprint/maps/long.map.yaml";
    let root = fixtures::Fixture::new(&format!("minimap_{tag}_{}", std::process::id())).descriptor("alpha/floor", "alpha").build("m");
    let mut text = String::from("version: 3\nname: long\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 10.0\n- 3.0\n- 10.0\nplacements:\n");
    // Every line is non-empty and every one is wider than the lane's 20-column floor, so a bar of no
    // width is a missing layout rather than a blank line.
    for line in 0..BODY_LINES {
        text.push_str(&format!("  # a line that makes the buffer long enough to scroll, number {line}\n"));
    }
    text.push_str("stamps: []\nlocations: []\n");
    let path = root.join(REL);
    std::fs::create_dir_all(path.parent().expect("a maps folder")).expect("a maps folder");
    std::fs::write(&path, text).expect("the long map is written");
    (root, REL.to_owned())
}

fn run_command(app: &mut App, id: &str) {
    let command = commands::by_id(id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(app);
}

fn editor_of(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<CodeEditor>>().single(world).unwrap_or_else(|e| panic!("one code editor is open: {e}"))
}

/// **An editor on `root` in the Files workspace, with `rel` open, having never typed.** The window is
/// spawned because the code editor lays its view out against the interface size the window gives it, and
/// a lane with no height has no bars.
fn files_editor(root: &Path, rel: &str) -> App {
    files_editor_on(root, rel, Device::None)
}

/// [`files_editor`] on a chosen device. The surface is sized to the window before the first frame, as
/// `a_markdown_page_wraps_and_is_drawn_in_ink` sizes it, so a real renderer never draws a frame at the
/// placeholder size and then resizes under a camera that has already read it (`surface::spawn_surface`).
fn files_editor_on(root: &Path, rel: &str, device: Device) -> App {
    let mut app = harness::build_stepped_at(root, "m", None, bevy_dark_factory::tiles::Mode::default(), device).unwrap_or_else(|e| panic!("{e}"));
    let window = Window::default();
    let (width, height) = (window.resolution.physical_width(), window.resolution.physical_height());
    app.world_mut().spawn((window, PrimaryWindow));
    harness::resize_surface(&mut app, width, height).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    run_command(&mut app, "workspace.files");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "the Files workspace is reachable by its command");
    files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    app
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

/// One bar's own node, as the lane left it.
#[derive(Debug, Clone, Copy)]
struct Bar {
    top: f32,
    width: f32,
    shown: bool,
}

/// The lane's band, as `(top, height)`, and its bars, in slot order.
fn lane(app: &App, editor: Entity) -> (Option<(f32, f32)>, Vec<Bar>) {
    let world = app.world();
    let mut band = None;
    let mut bars = Vec::new();
    for entity in under(world, editor) {
        let Some(node) = world.get::<Node>(entity) else { continue };
        match world.get::<Role>(entity).copied() {
            Some(Role::CodeMinimapBand) => band = Some((written(node.top, "the band's top"), written(node.height, "the band's height"))),
            Some(Role::CodeMinimapBar) => {
                bars.push(Bar { top: written(node.top, "a bar's top"), width: written(node.width, "a bar's width"), shown: node.display != Display::None });
            }
            _ => {}
        }
    }
    (band, bars)
}

/// A value the lane wrote, or a panic naming the one it did not: `Val::Auto` on a bar's `top` or `width` or
/// the band's `top` or `height` is a part no layout reached, which is the defect this file is about.
fn written(v: Val, what: &str) -> f32 {
    match v {
        Val::Px(p) => p,
        other => panic!("{what} carries {other:?}, which no layout wrote"),
    }
}

/// **The lane is laid out on the frame the file opens, with no keystroke** (#540 acceptance 1).
///
/// The file is opened and the editor stepped; nothing is typed, so the buffer's revision never moves and
/// a lane that waits for one cannot pass. On v0.58.0 every bar read `top: 0`, `width: 0` and the band
/// `height: 0`, because the pool was spawned bare and the layout returned before writing it.
#[test]
fn a_file_opened_for_reading_draws_its_minimap_bars_on_the_first_frame() {
    let (root, rel) = project("bars");
    let mut app = files_editor(&root, rel.as_str());
    let editor = editor_of(&mut app);
    assert!(app.world().get::<CodeMinimap>(editor).is_some(), "the Files editor asks for a minimap");

    let (band, bars) = lane(&app, editor);
    let drawn: Vec<Bar> = bars.iter().copied().filter(|b| b.shown).collect();
    assert!(drawn.len() > 1, "the lane draws a bar a line: {} of {} bars show", drawn.len(), bars.len());

    // **A bar is as long as its line**, and every line in this file is wider than the lane's floor, so a
    // drawn bar of no width is the missing layout and not a blank line.
    let empty: Vec<usize> = drawn.iter().enumerate().filter(|(_, b)| b.width <= 0.0).map(|(i, _)| i).collect();
    assert!(empty.is_empty(), "{} of {} drawn bars have no width, so the lane was never laid out: slots {empty:?}", empty.len(), drawn.len());

    // **Distinct `top`s, a pitch apart.** A pool spawned at the origin and never written stacks every bar
    // at `top: 0`; the lane's own pitch is what says they were placed.
    let mut tops: Vec<f32> = drawn.iter().map(|b| b.top.round()).collect();
    tops.sort_by(|a, b| a.partial_cmp(b).expect("a finite top"));
    tops.dedup();
    assert!(tops.len() > 1, "every bar stands at the same `top` ({:?}), so they were never laid out", tops.first());
    for pair in tops.windows(2) {
        let step = pair[1] - pair[0];
        assert!((step - BAR_PITCH_PX).abs() < 1.0, "drawn bars stand {step} px apart, not the lane's {BAR_PITCH_PX} px pitch: {:?}", &tops[..tops.len().min(8)]);
    }

    // **The band is the text in view** (#540 acceptance 2): it starts at the view's first line, which is the
    // file's first, and it is as many rows tall as the view holds lines.
    let (band_top, band_height) = band.expect("the lane draws a view band");
    let view = visible(&app, editor);
    assert_eq!(view.start, 0, "the file opens at its first line");
    assert_eq!(band_top, view.start as f32 * BAR_PITCH_PX, "the band starts at the lane's row of the view's first line, {}", view.start);
    assert_eq!(band_height, view.len() as f32 * BAR_PITCH_PX, "the band is as tall as the {} lines in view ({view:?})", view.len());
    let lane_height = drawn.len() as f32 * BAR_PITCH_PX;
    assert!(
        band_height < lane_height,
        "the band is {band_height} px of a {lane_height} px lane for a {BODY_LINES}-line file in a window that shows less: it covers the whole file, not the part in view"
    );
}

/// **The lines the view holds**, first to one past the last, as the editor reports them.
fn visible(app: &App, editor: Entity) -> std::ops::Range<usize> {
    app.world().get::<CodeEditor>(editor).map(CodeEditor::visible_lines).unwrap_or_else(|| panic!("the editor is open"))
}

/// **How far down the file the view is moved**, in lines.
const SCROLLED_TO: usize = 30;

/// **The band follows the view when it scrolls, with no edit** (#540 acceptance 2, `bevy_gooey#123`): after the
/// view moves down the file, the band starts at the lane's row of the view's new first line and is as tall as
/// the lines in view, the height it had at the top. The buffer's revision does not move, so nothing typed
/// drew it. On v0.61.0 the band grew by the distance scrolled.
#[test]
fn the_band_follows_the_view_when_it_scrolls_with_no_edit() {
    let (root, rel) = project("scroll");
    let mut app = files_editor(&root, rel.as_str());
    let editor = editor_of(&mut app);
    let (band, bars) = lane(&app, editor);
    let (_, opened_height) = band.expect("the lane draws a view band");
    let (lines, revision) =
        app.world().get::<CodeEditor>(editor).map(|e| (e.buffer().len_lines(), e.buffer().revision())).unwrap_or_else(|| panic!("the editor is open"));
    // A lane that holds every line does not scroll itself, so a line's row in the lane is its number.
    assert_eq!(bars.len(), lines, "the lane holds a bar for each of the file's {lines} lines");

    app.world_mut().get_mut::<CodeEditor>(editor).unwrap_or_else(|| panic!("the editor is open")).scroll_to_line(SCROLLED_TO);
    settle_support::settle(&mut app);
    let view = visible(&app, editor);
    assert!(view.start > 0, "the view moved down the file: it shows lines {view:?}");
    let (band, _) = lane(&app, editor);
    let (top, height) = band.expect("the lane draws a view band");
    assert_eq!(top, view.start as f32 * BAR_PITCH_PX, "the band starts at the lane's row of the view's first line, {}", view.start);
    assert_eq!(height, view.len() as f32 * BAR_PITCH_PX, "the band is as tall as the {} lines in view ({view:?})", view.len());
    assert_eq!(height, opened_height, "the view holds as many lines as it did at the top, so the band is as tall as it was");
    let now = app.world().get::<CodeEditor>(editor).map(|e| e.buffer().revision()).unwrap_or_else(|| panic!("the editor is open"));
    assert_eq!(now, revision, "nothing was typed: the band followed the view, not an edit");
}

/// **How many notches the wheel is turned down the file**: each is a [`MouseScrollPixelsPerLine`] (100 px by
/// default) of the window, which gooey's `code_on_wheel` divides by `UiScale` into the editor's own px.
const NOTCHES: f32 = 2.0;

/// **Where the window's cursor stands**, written in `PreUpdate` after the input systems: the way
/// `capture_feedback_1794147411369` points before it turns the wheel, so the pointer the picking backend
/// reads is the window's own.
#[derive(Resource, Default)]
struct Aim(Option<Vec2>);

fn aim(at: Res<Aim>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(at.0);
    }
}

/// A node's border box on the surface, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **The node a pointer stands over**: the nearest hit of the first pointer whose `HoverMap` entry holds one,
/// which is the node `code_on_wheel` walks up from to find the editor it scrolls.
fn nearest_hit(app: &App) -> Option<Entity> {
    let hover = app.world().get_resource::<HoverMap>()?;
    hover.values().find_map(|hits| hits.iter().min_by(|a, b| a.1.depth.total_cmp(&b.1.depth)).map(|(hit, _)| *hit))
}

/// **The band follows the view when the person's wheel scrolls it, with no edit** (#540 acceptance 2, "by
/// wheel"). The pointer is put over the editor, the wheel is turned [`NOTCHES`] notches down as a `MouseWheel`
/// message, and the view is read where gooey left it: moved by exactly the wheel's distance, so it was the
/// wheel that moved it. The band then starts at the lane's row of the view's new first line and is as tall
/// as it was at the top, because the viewport holds as many rows. A wheel stops wherever its pixels end,
/// between two lines as often as not, so the view's own line count (which counts a line cut at its head and
/// one cut at its foot) is the band's lines or one more. The buffer's revision does not move.
#[test]
fn the_band_follows_the_view_when_the_wheel_scrolls_it_with_no_edit() {
    let (root, rel) = project("wheel");
    let mut app = files_editor(&root, rel.as_str());
    let editor = editor_of(&mut app);
    let (band, _) = lane(&app, editor);
    let (opened_top, opened_height) = band.expect("the lane draws a view band");
    let revision = app.world().get::<CodeEditor>(editor).map(|e| e.buffer().revision()).unwrap_or_else(|| panic!("the editor is open"));

    // **The pointer over the text**: a quarter of the way into the code area from its top-left, among the
    // rows. The surface is the window's size at a scale factor of 1, so a surface pixel is a window pixel.
    let over = {
        let world = app.world();
        let area = under(world, editor)
            .into_iter()
            .find(|e| world.get::<Role>(*e) == Some(&Role::CodeArea))
            .and_then(|area| border_box(world, area))
            .unwrap_or_else(|| panic!("the editor's code area is laid out"));
        area.min + area.size() * 0.25
    };
    app.init_resource::<Aim>();
    app.add_systems(PreUpdate, aim.after(InputSystems));
    app.world_mut().resource_mut::<Aim>().0 = Some(over);
    settle_support::until(&mut app, "the pointer's hover reaches the interface", |app| nearest_hit(app).is_some());
    let hit = nearest_hit(&app).unwrap_or_else(|| panic!("the pointer stands over a node"));
    assert!(
        under(app.world(), editor).contains(&hit),
        "the pointer at {over} stands over the editor's text, not over {hit:?} ({:?}, {:?})",
        app.world().get::<Role>(hit),
        app.world().get::<Name>(hit)
    );

    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).unwrap_or_else(|e| panic!("one primary window: {e}"));
    app.world_mut().write_message(MouseWheel { unit: MouseScrollUnit::Line, x: 0.0, y: -NOTCHES, window, phase: TouchPhase::Moved });
    settle_support::settle(&mut app);

    let ratio = app.world().get_resource::<MouseScrollPixelsPerLine>().copied().unwrap_or_default();
    let ui_scale = app.world().resource::<UiScale>().0;
    let wheeled = NOTCHES * ratio / ui_scale;
    let scrolled = app.world().get::<CodeEditor>(editor).map(CodeEditor::scroll).unwrap_or_else(|| panic!("the editor is open")).y;
    assert!(
        (scrolled - wheeled).abs() < 0.5,
        "the wheel moved the view by its own distance: {NOTCHES} notches of {} px at UiScale {ui_scale} is {wheeled} px, and the view is scrolled {scrolled} px",
        NOTCHES * ratio
    );
    let view = visible(&app, editor);
    assert!(view.start > 0, "the wheel moved the view down the file: it shows lines {view:?}");
    let (band, _) = lane(&app, editor);
    let (top, height) = band.expect("the lane draws a view band");
    eprintln!(
        "{NOTCHES} notches over {over} at UiScale {ui_scale}: the view scrolled {scrolled} px to lines {view:?}; the band moved from top {opened_top} to {top}, {height} px tall"
    );
    assert_ne!(top, opened_top, "the band moved with the view");
    assert_eq!(top, view.start as f32 * BAR_PITCH_PX, "the band starts at the lane's row of the view's first line, {}", view.start);
    assert_eq!(height, opened_height, "the viewport holds as many rows as it did at the top, so the band is as tall as it was");
    let band_lines = (height / BAR_PITCH_PX).round() as usize;
    assert!(
        view.len() == band_lines || view.len() == band_lines + 1,
        "the band is the {band_lines} lines the viewport holds from its first; the view shows {} lines ({view:?}), which are those or those and a sliver of the next",
        view.len()
    );
    let now = app.world().get::<CodeEditor>(editor).map(|e| e.buffer().revision()).unwrap_or_else(|| panic!("the editor is open"));
    assert_eq!(now, revision, "nothing was typed: the wheel moved the band, not an edit");
}

/// **How near a texel must be to the bar's colour to be the bar**, per channel. The bar is opaque
/// (`GooeySkin::minimap_bar` is `Color::srgb`) and drawn into the surface's sRGB texels, so it arrives
/// within a rounding of its own value.
const INK_TOLERANCE: u8 = 3;

/// **A captured frame of a file opened for reading shows its bars in the lane** (#540 acceptance 3): the editor
/// on a real device, the file opened and never typed in, the frame read back through `harness::capture` (the
/// editor's own `Screenshot` of its surface), and the bar colour read off the picture's pixels. Two readings:
/// every bar laid out clear of the band paints the bar colour at its own centre, and a census of the lane
/// outside the band finds the colour on at least as many pixel rows as there are such bars. The census is the
/// issue's own measurement, which counted **zero** bar-coloured pixels in the 124 px lane on the frame a file
/// opened (`bevy_gooey` pinned at `8647648`). The frame is kept as `fixtures::kept_picture("minimap_opened")`.
#[test]
fn a_captured_frame_of_a_file_opened_for_reading_paints_the_bar_colour_in_the_lane() {
    if !harness::real_device_available() {
        eprintln!("SKIPPED: no wgpu adapter on this host, so no frame can be rendered or read back");
        return;
    }
    let (root, rel) = project("pixels");
    let mut app = files_editor_on(&root, rel.as_str(), Device::Real);
    let editor = editor_of(&mut app);
    let revision = app.world().get::<CodeEditor>(editor).map(|e| e.buffer().revision()).unwrap_or_else(|| panic!("the editor is open"));
    let picture = harness::capture(&mut app).unwrap_or_else(|e| panic!("{e}"));
    let frame = picture.try_into_dynamic().unwrap_or_else(|e| panic!("the capture is a picture: {e:?}")).to_rgba8();
    let kept = fixtures::kept_picture("minimap_opened");
    frame.save(&kept).unwrap_or_else(|e| panic!("the frame is kept at {}: {e}", kept.display()));
    eprintln!("the captured frame: {}", kept.display());

    let world = app.world();
    let ink = world.resource::<GooeySkin>().minimap_bar().to_srgba().to_u8_array();
    let parts = under(world, editor);
    let part = |role: Role| parts.iter().copied().find(|e| world.get::<Role>(*e) == Some(&role)).unwrap_or_else(|| panic!("the lane has a {role:?}"));
    let lane_box = border_box(world, part(Role::CodeMinimap)).unwrap_or_else(|| panic!("the lane is laid out"));
    let band_box = border_box(world, part(Role::CodeMinimapBand)).unwrap_or_else(|| panic!("the band is laid out"));
    let clear_of_band = |r: &Rect| r.max.y <= band_box.min.y || r.min.y >= band_box.max.y;
    let bars: Vec<Rect> = parts
        .iter()
        .filter(|e| world.get::<Role>(**e) == Some(&Role::CodeMinimapBar))
        .filter(|e| world.get::<Node>(**e).is_some_and(|n| n.display != Display::None))
        .filter_map(|e| border_box(world, *e))
        .filter(|r| r.width() > 0.0 && r.height() > 0.0 && lane_box.contains(r.min) && lane_box.contains(r.max) && clear_of_band(r))
        .collect();
    assert!(bars.len() > 10, "the lane lays out bars below the band for a {BODY_LINES}-line file: {} of them", bars.len());

    let is_bar = |x: u32, y: u32| x < frame.width() && y < frame.height() && (0..3).all(|c| frame.get_pixel(x, y).0[c].abs_diff(ink[c]) <= INK_TOLERANCE);
    let unpainted: Vec<Rect> = bars.iter().copied().filter(|r| !is_bar(r.center().x as u32, r.center().y as u32)).collect();
    assert!(
        unpainted.is_empty(),
        "{} of {} bars clear of the band do not paint the bar colour {ink:?} at their centre, first {:?} (frame kept at {})",
        unpainted.len(),
        bars.len(),
        unpainted.first(),
        kept.display()
    );

    // **The census**: every texel of the lane outside the band, counted where it is the bar colour.
    let (mut texels, mut rows) = (0usize, BTreeSet::new());
    for y in lane_box.min.y.ceil() as u32..lane_box.max.y.floor() as u32 {
        if (band_box.min.y..=band_box.max.y).contains(&(y as f32 + 0.5)) {
            continue;
        }
        for x in lane_box.min.x.ceil() as u32..lane_box.max.x.floor() as u32 {
            if is_bar(x, y) {
                texels += 1;
                rows.insert(y);
            }
        }
    }
    eprintln!("the lane {lane_box:?} outside the band {band_box:?}: {texels} texels of the bar colour {ink:?} on {} rows, {} bars", rows.len(), bars.len());
    assert!(texels > 0, "the lane outside the band holds no texel of the bar colour {ink:?}: a flat lane with no bars (frame kept at {})", kept.display());
    assert!(rows.len() >= bars.len(), "the bar colour stands on {} pixel rows for {} bars: a bar is a row of its own", rows.len(), bars.len());
    let now = app.world().get::<CodeEditor>(editor).map(|e| e.buffer().revision()).unwrap_or_else(|| panic!("the editor is open"));
    assert_eq!(now, revision, "nothing was typed before the capture");
}
