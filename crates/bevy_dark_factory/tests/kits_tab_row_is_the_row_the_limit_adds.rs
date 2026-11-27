//! **The tab row the interface-size limit adds is the row the Kit lays out** (#670, follow-up).
//!
//! A one-panel workspace folds its tab into the chrome bar, so the layout table behind the limit
//! (`surface::LAYOUT_BY_TEXT_SIZE`) is read in a frame with no tab row, and the limit and the shell's dock rule add the Kit's row
//! back. They added a fixed 35 px: the strip's least height, which is the row only up to the text size where the tabs' own type and
//! inset outgrow it. At text 300% the Kit's row is taller, so the limit was loose by the difference on the one workspace that has the row.
//! Now both read `surface::tab_row_at`, measured per text size in `surface::TAB_ROW_BY_TEXT_SIZE`; these tests measure the Kit's row in
//! the laid-out editor and hold the table and the limit to it.

use std::path::Path;

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::window::{PrimaryWindow, Window, WindowResolution};

use bevy_dark_factory::chrome::Frame;
use bevy_dark_factory::harness;
use bevy_dark_factory::settings::{EditorSettings, UserSettingsFile};
use bevy_dark_factory::surface::{LAYOUT_BY_TEXT_SIZE, TAB_ROW_BY_TEXT_SIZE, fit_interface_scale, tab_row_at};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// The window the rows are measured in: 1280 x 1200 logical at a display factor of 2, the census table's window.
const WINDOW: (u32, u32) = (2560, 2400);
const DISPLAY_FACTOR: f32 = 2.0;
const SETTLE_FRAMES: usize = 18;

/// How far a reading may stand above its table row, in layout units: whole-pixel layout.
const BELOW: f32 = 0.5;
/// How far the table may stand above a reading, in layout units: rounding up to a whole unit.
const ABOVE: f32 = 1.0;

/// The Kit, laid out at `text_scale` in [`WINDOW`], with the person's interface size at 100%.
fn kit_at(root: &Path, text_scale: f32) -> App {
    let user = root.join(format!("user_settings_{text_scale}/settings.yaml"));
    std::fs::create_dir_all(user.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&user, format!("version: 1\ntext_scale: {text_scale}\ninterface_scale: 1.0\n")).expect("a settings file");
    let mut app = harness::build_headless_at(root, "m", None, Workspace::Kit.opens_on()).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.insert_resource(UserSettingsFile(user));
    let window = app.world_mut().spawn((Window { resolution: WindowResolution::new(WINDOW.0, WINDOW.1), ..default() }, PrimaryWindow)).id();
    let mut window = app.world_mut().get_mut::<Window>(window).expect("the window just spawned");
    window.resolution.set_scale_factor_override(Some(DISPLAY_FACTOR));
    harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Kit, "the editor did not open on the Kit");
    let applied = app.world().resource::<EditorSettings>().text_scale;
    assert!((applied - text_scale).abs() < 1e-3, "the editor runs at text size {applied}, not {text_scale}");
    app
}

/// The Kit's tab row as laid out, in layout units (logical px over `UiScale`, what the shell's dock rule reads).
fn measured_row(app: &App) -> f32 {
    let strip = app.world().resource::<Frame>().door_strip;
    let node = app.world().get::<ComputedNode>(strip).expect("the Kit's door strip is laid out");
    node.size().y * node.inverse_scale_factor()
}

fn project(tag: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(&format!("kit_tab_row_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m")
}

/// **The table is the Kit's row at every text size**: never below a reading (the limit would let a panel collapse on the Kit) and not
/// more than a unit above one (it would limit a size the Kit holds).
#[test]
fn the_tab_row_table_is_what_the_kit_lays_out() {
    let root = project("table");
    let mut readings = Vec::new();
    let mut failures = Vec::new();
    for (text_scale, row) in TAB_ROW_BY_TEXT_SIZE {
        let app = kit_at(&root, text_scale);
        let measured = measured_row(&app);
        readings.push(format!("({text_scale}, {measured:.2})"));
        if measured > row + BELOW || row - measured > ABOVE {
            failures.push(format!("text {text_scale}: the row is {row} in the table and {measured:.2} on the Kit"));
        }
    }
    assert!(failures.is_empty(), "TAB_ROW_BY_TEXT_SIZE no longer matches the Kit's row:\n  {}\nreadings: {}", failures.join("\n  "), readings.join(", "));
    // One table of text sizes, so the two are read at the same rows.
    let sizes: Vec<f32> = LAYOUT_BY_TEXT_SIZE.iter().map(|row| row.0).collect();
    let row_sizes: Vec<f32> = TAB_ROW_BY_TEXT_SIZE.iter().map(|row| row.0).collect();
    assert_eq!(sizes, row_sizes, "the tab row is measured at the layout table's text sizes");
}

/// **At text 300% on the Kit, the limit is the Map's need plus the dock plus the row the Kit lays out** — not plus 35.
///
/// At 300% a panel needs more than the dock's threshold, so the only heights a choice is held at are the need plus the dock's row and up
/// (`surface::fit_interface_scale`). The smallest window that holds 100% is found from the function and turned back into a frame
/// height; it must be at least the Map's need, the dock's row and the Kit's measured tab row (below it, a panel on the Kit has no row to
/// read), and no further above it than the table's rounding up ([`ABOVE`]) and the limit's own one unit of slack. With the old 35 it
/// stood about 13 units short (the Kit lays out 48.39 on bmb and 48.68 on big at 300%).
#[test]
fn the_limit_on_the_kit_at_text_300_adds_the_row_the_kit_lays_out() {
    const TEXT: f32 = 3.0;
    let root = project("limit");
    let app = kit_at(&root, TEXT);
    let row = measured_row(&app);
    assert!(row > 35.0 + BELOW, "at text 300% the Kit's tab row ({row:.2}) is taller than the strip's least height of 35");
    let table = tab_row_at(TEXT);
    assert!(table >= row - BELOW && table - row <= ABOVE, "tab_row_at(3.0) is {table} and the Kit lays out {row:.2}");

    // The smallest logical height, at the reference width (no growth), whose limit leaves 100% as it is.
    let width = 1280.0;
    let (mut short, mut tall) = (100.0f32, 5000.0f32);
    assert!(fit_interface_scale(width, short, TEXT, 1.0).limited && !fit_interface_scale(width, tall, TEXT, 1.0).limited);
    for _ in 0..60 {
        let mid = 0.5 * (short + tall);
        if fit_interface_scale(width, mid, TEXT, 1.0).limited {
            short = mid;
        } else {
            tall = mid;
        }
    }
    let layout = bevy_dark_factory::style::metrics().layout;
    let frame = tall / layout.ui_scale;
    let need = LAYOUT_BY_TEXT_SIZE.iter().find(|r| r.0 == TEXT).map(|r| r.2).expect("the table has a 300% row");
    let expected = need + layout.bottom_dock_h + row;
    // The limit's slack (`surface::LAYOUT_SLACK`, one layout unit) stands on top of the row the table rounds up.
    let limit_slack = 1.0;
    assert!(
        frame >= expected && frame - expected <= ABOVE + limit_slack,
        "the limit holds 100% at text 300% from a {frame:.2}-unit frame; the Map's need {need}, the dock {} and the Kit's row {row:.2} make {expected:.2}",
        layout.bottom_dock_h
    );
}
