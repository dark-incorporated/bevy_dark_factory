//! **A file opened for reading draws its minimap on the frame it is first shown** (#540, `bevy_gooey#117`).
//!
//! The lane is an outline of the file down the code editor's right edge: one bar a line, as long as the
//! line, and a band saying which lines are in view. Until `bevy_gooey` v0.59.0 (`a3585c50`) the parts were
//! grown with `Commands` and then written from a query those commands had not reached, so a file opened
//! for reading — no keystroke, so no buffer revision — drew a flat lane with no bars at all. This file
//! is the editor's half of that: it opens a file, never types, and reads what the lane laid out.
//!
//! What it does **not** prove: the colours a bar and a band's edge are held to are gooey's own
//! (`GooeySkin::minimap_bar`, `minimap_band_edge`, held by its `tests/skin.rs`), and a pixel census of the
//! lane needs a renderer (`harness::Device::Real`), which is a separate, opt-in test. Every number here
//! is read off the world: a bar's `top` and `width` from its `Node`, the lane's pitch from the bars, the
//! lines in view from `CodeEditor::visible_lines`.
//!
//! # The band
//!
//! **The band is the text in view: it starts at the lane's row of the view's first line and is as many
//! rows tall as the view holds lines**, on the frame the file opens and after a scroll with no edit. Until
//! `bevy_gooey` v0.64.1 (`bevy_gooey#123`) it counted its last row as `first_row + drawn - 1` with
//! `drawn = first_row + needed`, `first_row` twice: right at the top of the file, and grown by the distance
//! scrolled after a scroll (measured 44 lines tall in a 20-line view after `scroll_to_line(30)`). The scroll
//! here is `CodeEditor::scroll_to_line`, the view's own move, not a wheel: the band reads the same glided
//! view whichever moved it.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::workspace::Workspace;
use bevy_gooey::{CodeEditor, CodeMinimap, Role};

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
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
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
