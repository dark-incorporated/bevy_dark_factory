//! **A file opened for reading shows its minimap laid out, with no keystroke** (#540): a bar per
//! line at its own `top` and a width that is not nothing, and the band at the top and height the
//! buffer's first visible line and the window's line count give.
//!
//! # What was measured
//!
//! On a 1600 x 1000 capture the minimap's 124 px lane at `x=1204..1327` was one flat fill on the
//! frame `bedroom.map.yaml` opened: a colour census over `y=240..800` found the ground 56,168 px,
//! the band 11,036 px and **zero pixels** of the bar colour `rgb(112,104,91)`. The same query
//! returned 90 bar nodes, all `width: Auto → 0.0 px` at `y=244.0`. One typed character later the
//! lane drew 89 bars at widths `6, 8, 9, 11 …` on 89 distinct `top`s of a 3–4 px pitch.
//!
//! # Where the defect is, and what this measures here
//!
//! `bevy_gooey`'s `draw_navigation` spawned the minimap's root and band and **returned**, and the
//! layout of the bars sat behind a redraw key that included the buffer's revision — so nothing
//! asked again until the text changed. The fix is gooey's (`dark-incorporated/bevy_gooey#117`); what
//! this file holds is the editor's half of the acceptance: **open a file, type nothing, and read
//! the lane**. A bar per line, each at a `top` of its own, none of them nothing wide, and a band
//! whose height is the part in view.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::workspace::Workspace;
use bevy_gooey::{CodeEditor, Role};

use crate::fixtures;
use crate::settle_support;

/// The surface the review captured at.
const SURFACE: (u32, u32) = (1600, 1000);

/// A map file of 80 lines against a window of about 30, so the lane has more lines than slots and
/// every bar is its own row — the shape the report measured (`bedroom.map.yaml`).
fn longer_than_the_window() -> String {
    let mut text = String::from("version: 3\nname: long\norigin:\n- 0.0\n- 0.0\n- 0.0\nbounds:\n- 10.0\n- 3.0\n- 10.0\nplacements:\n");
    for i in 0..70 {
        text.push_str(&format!("- id: floor@{i}\n  descriptor: alpha/floor\n  at:\n  - {}.0\n  - 0.0\n  yaw: 0.0\n", i % 5));
    }
    text.push_str("stamps: []\nlocations: []\n");
    text
}

const MAP: &str = "assets/blueprint/maps/m.map.yaml";

fn opened() -> (App, Entity) {
    let root = fixtures::Fixture::new(&format!("minimap_lane_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .build("m");
    let text = longer_than_the_window();
    std::fs::write(root.join(MAP), &text).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, SURFACE.0, SURFACE.1).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "the Files workspace is open");
    files::open_file(app.world_mut(), MAP).unwrap_or_else(|e| panic!("{e}"));
    // **No key, no pointer, no second open**: the settle is the frames the editor needs to lay out
    // what the open spawned, and nothing else happens before the lane is read.
    settle_support::settle(&mut app);
    let editor = app.world_mut().query_filtered::<Entity, With<CodeEditor>>().single(app.world_mut()).unwrap_or_else(|e| panic!("{e}"));
    (app, editor)
}

/// A node's drawn rect in **logical** px (`ComputedNode::size()` is physical; the sheet is logical).
fn drawn(app: &App, entity: Entity) -> Rect {
    let node = app.world().get::<ComputedNode>(entity).unwrap_or_else(|| panic!("{entity:?} is laid out"));
    let at = app.world().get::<UiGlobalTransform>(entity).unwrap_or_else(|| panic!("{entity:?} is placed")).translation;
    let scale = node.inverse_scale_factor();
    Rect::from_center_size(at * scale, node.size() * scale)
}

fn role_of(app: &App, entity: Entity) -> Option<Role> {
    app.world().get::<Role>(entity).copied()
}

fn descendants(world: &World, root: Entity) -> Vec<Entity> {
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        let Some(children) = world.get::<Children>(entity) else { continue };
        for child in children.iter() {
            found.push(child);
            stack.push(child);
        }
    }
    found
}

/// **The bars are drawn where a person reads them, on the first frame the file is open.**
#[test]
fn a_file_opened_with_no_keystroke_draws_a_bar_for_each_line_at_its_own_top() {
    let (app, editor) = opened();
    let parts = descendants(app.world(), editor);
    let lane: Vec<Entity> = parts.iter().copied().filter(|e| role_of(&app, *e) == Some(Role::CodeMinimap)).collect();
    assert_eq!(lane.len(), 1, "the editor draws one minimap lane");
    let lane_box = drawn(&app, lane[0]);
    assert!(lane_box.height() > 100.0, "the lane has a lane's height: {lane_box:?}");
    let bars: Vec<Entity> = parts.iter().copied().filter(|e| role_of(&app, *e) == Some(Role::CodeMinimapBar)).collect();
    assert!(bars.len() > 10, "a bar per line of a file longer than the window: {}", bars.len());
    let mut tops: Vec<f32> = Vec::new();
    let mut widths: Vec<f32> = Vec::new();
    for bar in &bars {
        let node = app.world().get::<Node>(*bar).expect("a bar has a node");
        if node.display == Display::None {
            continue;
        }
        tops.push(drawn(&app, *bar).min.y);
        widths.push(match node.width {
            Val::Px(px) => px,
            other => panic!("a drawn bar is a px wide, not {other:?} (the defect was `Auto -> 0.0 px`)"),
        });
    }
    assert!(tops.len() > 10, "{} bars are drawn, not hidden", tops.len());
    assert!(widths.iter().all(|w| *w > 0.0), "no drawn bar is nothing wide: {} of {} were 0 px", widths.iter().filter(|w| **w == 0.0).count(), widths.len());
    tops.sort_by(f32::total_cmp);
    let distinct = tops.windows(2).filter(|w| (w[1] - w[0]).abs() > 0.5).count() + 1;
    assert_eq!(distinct, tops.len(), "every bar stands at a `top` of its own: {tops:?}");
    // **A bar is inside the lane**, so its width is a length a person can see and not one that ran away.
    for bar in &bars {
        let box_of_bar = drawn(&app, *bar);
        assert!(
            box_of_bar.min.y >= lane_box.min.y - 1.0 && box_of_bar.max.y <= lane_box.max.y + 1.0,
            "a bar {box_of_bar:?} stands inside the lane {lane_box:?}"
        );
    }
}

/// **The band is the part in view**: its height is the visible line count of the lane's slots, and
/// it moves when the editor scrolls with nothing typed.
#[test]
fn the_view_band_is_the_part_in_view_and_follows_a_scroll_with_no_edit() {
    let (mut app, editor) = opened();
    let parts = descendants(app.world(), editor);
    let lane = parts.iter().copied().find(|e| role_of(&app, *e) == Some(Role::CodeMinimap)).expect("a lane");
    let lane_box = drawn(&app, lane);
    let band = parts.iter().copied().find(|e| role_of(&app, *e) == Some(Role::CodeMinimapBand)).expect("a band");
    let band_box = drawn(&app, band);
    let bars_drawn = parts.iter().filter(|e| role_of(&app, **e) == Some(Role::CodeMinimapBar)).count();
    assert!(
        band_box.height() > lane_box.height() * 0.1,
        "the band covers the visible lines, not 3 % of the lane: band {band_box:?} of lane {lane_box:?} ({bars_drawn} bar slots)"
    );
    // **A wheel with no edit moves it**: the editor scrolls, the buffer's revision does not change.
    let revision = app.world().get::<CodeEditor>(editor).map(|e| e.buffer().revision()).unwrap_or_default();
    let before = drawn(&app, band);
    app.world_mut().get_mut::<CodeEditor>(editor).expect("the editor").scroll_to_line(60);
    settle_support::settle(&mut app);
    let after = drawn(&app, band);
    assert_eq!(
        app.world().get::<CodeEditor>(editor).map(|e| e.buffer().revision()).unwrap_or_default(),
        revision,
        "the scroll changed no text"
    );
    assert!(after.min.y > before.min.y + 10.0, "the band followed the scroll: {before:?} to {after:?}");
}
