//! **The shell's split divider is the sheet's `divider_w`, not gooey's target** (#689, #476, #550).
//!
//! # What was drawn
//!
//! The divider's `ComputedNode` on the Map workspace was **29.0 × 666.0 physical px** at a 1360×850 window and
//! UI scale 1.2 — 24.2 logical, which is `bevy_gooey`'s `MIN_TARGET_PX`. The sheet's `divider_w: 12` was read
//! once, in `size_the_shell_split`'s dock arithmetic, so the panes were measured with a 12 px seam and drawn
//! with a 24 px bar.
//!
//! # Why it is drawn now
//!
//! `bevy_gooey` grew `SplitThickness` (the knob `bevy_gooey#108` was filed for): the split root carries it, and
//! `update_splits` writes the number into the divider's own `Sizing`, keeping it across every restyle. The shell
//! spawns `SplitThickness(metrics.layout.divider_w)`, so the width a pane is measured with is the width drawn,
//! and this file holds the two together so they cannot part again.
//!
//! # The half-pixel shift
//!
//! The earlier attempt (`fleet/w525-split`, `05cae6af`) was held back because one protected test had to be
//! loosened for a half-pixel viewport shift nobody traced. The shift is the arithmetic, and it is expected: a
//! 12 px divider leaves 12 px more room than a 24 px one, and `size_the_shell_split` gives that room to the
//! viewport (it is the map that is owed the leftover). So the viewport is 12 logical px wider — 14.4 physical at
//! 1.2 — and a test that pinned the viewport's pixel width to the old divider is pinning a number that was
//! always a consequence. Nothing here is loosened: the divider's own assertion is tightened, from gooey's floor
//! to the sheet's number.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiScale};
use bevy_dark_factory::harness;

use crate::fixtures;
use crate::settle_support;

/// The interface scales the divider is measured at: the size the sheet's constants were written for, and the
/// capture's own 2.4 (the editor's 1.2 times the display's 2.0).
const SCALES: [f32; 2] = [1.0, 2.4];

/// How far a measured edge may stand from the number it is held to, logical px: layout rounds.
const SLACK_PX: f32 = 0.5;

fn editor(name: &str, scale: f32) -> App {
    let root = fixtures::Fixture::new(&format!("divider_seam_{name}_{}", std::process::id()))
        .descriptor("floor", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    settle_support::settle(&mut app);
    app
}

/// **Every divider the editor has laid out**, as `(entity, width in logical px)`.
///
/// Only the laid-out ones: the dock's body is a hidden `DockBody` whose pages are built but never laid out, so
/// their dividers measure 0, and a 0 px divider is not one anybody can see or grab.
fn laid_out_dividers(app: &mut App) -> Vec<(Entity, f32)> {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(Entity, &ComputedNode), With<bevy_gooey::SplitDivider>>();
    let mut found: Vec<(Entity, f32)> = query
        .iter(world)
        .map(|(entity, node)| (entity, node.size().x * node.inverse_scale_factor()))
        .filter(|(_, wide)| *wide > 0.0)
        .collect();
    found.sort_by(|(a, w), (b, v)| a.index().cmp(&b.index()).then(w.total_cmp(v)));
    found
}

/// **The drawn divider's logical thickness is the sheet's `divider_w`**, at every interface scale.
#[test]
fn the_drawn_divider_is_the_sheets_divider_w() {
    let sheet = bevy_dark_factory::style::metrics().layout.divider_w;
    let mut refused: Vec<String> = Vec::new();
    for scale in SCALES {
        let mut app = editor(&format!("width_{scale}"), scale);
        let found = laid_out_dividers(&mut app);
        assert!(
            !found.is_empty(),
            "the shell is a gooey split, so it must have a laid-out divider at scale {scale}; none was found"
        );
        for (entity, wide) in found {
            if (wide - sheet).abs() > SLACK_PX {
                refused.push(format!(
                    "[scale {scale}] divider {entity} is {wide:.2} logical px wide; the sheet's `layout.divider_w` \
                     is {sheet}"
                ));
            }
        }
    }
    assert!(
        refused.is_empty(),
        "**the split's channel is not the sheet's `divider_w` (#689)**: `size_the_shell_split` subtracts that \
         number from the room the panes share, so a divider wider than it is a bar the arithmetic never paid for:\n{}",
        refused.join("\n")
    );
}

/// **The split root carries the sheet's thickness**, which is what `update_splits` reads — so the number the
/// divider is drawn at and the number the sheet states are one value, not two that happen to agree.
#[test]
fn the_shell_split_carries_the_sheets_thickness() {
    let sheet = bevy_dark_factory::style::metrics().layout.divider_w;
    let mut app = editor("thickness", 1.0);
    let world = app.world_mut();
    let mut query = world.query_filtered::<&bevy_gooey::SplitThickness, With<bevy_dark_factory::chrome::ShellSplit>>();
    let carried: Vec<f32> = query.iter(world).map(|t| t.0).collect();
    assert_eq!(carried.len(), 1, "the shell builds one split, and it carries one thickness: {carried:?}");
    assert_eq!(
        carried[0], sheet,
        "the shell split's `SplitThickness` is the sheet's `layout.divider_w`, which `size_the_shell_split` already \
         subtracts"
    );
}
