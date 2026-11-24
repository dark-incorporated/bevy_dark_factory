//! **The seam the shell paints is the seam its layout budgets** (#550).
//!
//! `metrics.layout.divider_w` is 12 (the owner's #476), and until 0.8.4 nothing handed it to gooey, so the
//! divider kept gooey's default **24**: `docs/design/layout-model.md:36` recorded the disagreement —
//! "3.3 visible inside the target this build drew (24; … is 12 since #476)". The shell now sets
//! `SplitThickness` from that one metric, and this reads the node back: a change to either side alone fails.

use bevy::prelude::*;

use bevy_dark_factory::harness;
use bevy_dark_factory::style::metrics;

use crate::fixtures;

#[test]
fn the_divider_drawn_is_the_one_the_shell_budgets() {
    let root = fixtures::Fixture::new("divider_matches_budget").descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }

    // **The divider is the split's own node**, which gooey gives a `ComputedNode`; its width is what a person
    // can press. The budget is the metric the layout model and every width calc read.
    let budget = metrics().layout.divider_w;
    let world = app.world_mut();
    let mut dividers = world.query::<(&bevy_gooey::SplitDivider, &ComputedNode)>();
    let drawn: Vec<f32> = dividers.iter(world).map(|(_, node)| node.size().x).collect();
    assert!(!drawn.is_empty(), "the shell draws a split divider");
    for width in &drawn {
        // **Measured, not read off the component**: a node's size is what the frame drew. The tolerance is a
        // device pixel at the fixture's scale, because a 12 px channel lands on a fractional boundary.
        assert!(
            (width - budget).abs() <= 1.0,
            "the divider draws {width} px where `metrics.layout.divider_w` budgets {budget} px; a seam wider than \
             its budget eats the viewport, and one narrower than it leaves a dead strip (#550)"
        );
    }
}
