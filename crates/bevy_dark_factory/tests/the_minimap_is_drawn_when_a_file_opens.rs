//! **The Files editor's minimap is laid out on the frame a file opens, with no keystroke** (#540, part of).
//!
//! # The measurement this states
//!
//! On the capture at `0c74e140` the minimap's 124 px lane was one flat fill on the frame `bedroom.map.yaml` opened: a
//! colour census over `y=240..800` returned the ground `rgb(34,32,28)` 56,168 px, the view band `rgb(121,91,43)`
//! 11,036 px, and **zero pixels** of the bar colour `rgb(112,104,91)` outside the band. After a single typed character
//! the same query returned 89 bars with real widths (`6, 8, 9, 11 …`) at 89 distinct pitches of 3-4 px.
//!
//! # Where the defect is, and why this file is `#[ignore]`d
//!
//! **Not here.** `bevy_gooey`'s `draw_navigation` (`code_overlay.rs`, pin `8647648`) spawns the minimap's root and its
//! band and then `return`s; every bar's `left`, `width` and `top`, and the band's `top` and `height`, are written a
//! frame later behind `let redraw = editor.overlay.minimap_key.as_ref() != Some(&key)`, and the `minimap_key` is set
//! on the frame the bars are *spawned*. A spawned overlay part is not in the layout query until the next frame, so the
//! bars exist with `width: Auto -> 0.0 px` and never hear the write, and `key` carries `editor.buffer.revision()`, so
//! with nothing typed `redraw` stays false and they keep their zero widths forever. Typing changes the revision,
//! changes the key, and the second pass finds the nodes already there.
//!
//! That is filed as **dark-incorporated/bevy_gooey#117**, and `files.rs` asks for nothing beyond `CodeMinimap`, so
//! there is no editor-side change that would make these assertions pass. Running them here rather than describing
//! them is what makes the issue closable the day the pin moves: delete the `#[ignore]` and they are the proof.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_gooey::{CodeEditor, Role};

use crate::fixtures;

/// Enough frames for the editor, the overlay's roots and a second layout pass — many more than a person waits.
const SETTLE: usize = 30;

/// **A file longer than the window**, so the lane has more lines than slots and the band cannot span all of it.
fn long_page() -> String {
    (0..120).map(|n| format!("line {n}: {}\n", "x".repeat(n % 40 + 4))).collect()
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

/// **The Files workspace with a long file open and not one key typed.**
fn open_a_file(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("minimap_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let rel = "design/long-page.md";
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, long_page()).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    harness::resize_surface(&mut app, 1360, 850).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(&mut app);
    files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    // **No key, no click, no edit**: the whole point is the frame the file opened on.
    settle(&mut app);
    app
}

/// Every node playing `role`, as `(width, top)` in logical px.
fn parts(app: &mut App, role: Role) -> Vec<(f32, f32)> {
    let world = app.world_mut();
    let mut query = world.query::<(&Role, &ComputedNode, &UiGlobalTransform)>();
    query
        .iter(world)
        .filter(|(carried, ..)| **carried == role)
        .map(|(_, node, at)| (node.size().x * node.inverse_scale_factor(), at.translation.y * node.inverse_scale_factor()))
        .collect()
}

/// **A file opened for reading draws bars at their own tops on the first frame, without a keystroke.**
#[test]
#[ignore = "blocked on dark-incorporated/bevy_gooey#117: draw_navigation lays the bars out only after a buffer change; this repo asks files.rs for nothing beyond CodeMinimap, so the fix is the pin"]
fn opening_a_file_lays_out_its_minimap_bars_with_no_keystroke() {
    let mut app = open_a_file("bars");
    let editor_is_open = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<CodeEditor>>();
        query.iter(world).count()
    };
    assert_eq!(editor_is_open, 1, "the file is open in one editor");
    let bars = parts(&mut app, Role::CodeMinimapBar);
    assert!(!bars.is_empty(), "the lane has bar slots on the frame the file opens");
    let drawn: Vec<(f32, f32)> = bars.iter().copied().filter(|(width, _)| *width > 0.0).collect();
    assert!(
        !drawn.is_empty(),
        "**no bar is drawn on the frame the file opens** (#540): {} slots, every one {:.1} px wide. The minimap is a \
         flat lane until a keystroke changes the buffer's revision.",
        bars.len(),
        bars.iter().map(|(width, _)| *width).fold(0.0_f32, f32::max)
    );
    // Distinct pitch: a lane whose bars all sat at one `top` would be a single mark, not a map of the page.
    let mut tops: Vec<u32> = drawn.iter().map(|(_, top)| top.round() as u32).collect();
    tops.sort_unstable();
    tops.dedup();
    assert!(
        tops.len() > drawn.len() / 2,
        "the bars stand at their own lines: {} distinct tops over {} drawn bars",
        tops.len(),
        drawn.len()
    );
}

/// **The view band is the visible window of the page, not a misplaced stripe.**
#[test]
#[ignore = "blocked on dark-incorporated/bevy_gooey#117: the band's top and height are written after the early return too"]
fn the_view_band_covers_the_lines_the_window_shows() {
    let mut app = open_a_file("band");
    let bands = parts(&mut app, Role::CodeMinimapBand);
    assert_eq!(bands.len(), 1, "one band over the lane");
    let lane = parts(&mut app, Role::CodeMinimap);
    assert_eq!(lane.len(), 1, "one lane");
    let (_band_width, band_top) = bands[0];
    // The band's top against the lane's: on an unedited file it was drawn at the wrong place as well as the bars.
    let (_lane_width, lane_top) = lane[0];
    assert!(
        band_top >= lane_top - 1.0,
        "**the band stands inside the lane** (#540): lane top {lane_top:.0}, band top {band_top:.0}"
    );
}
