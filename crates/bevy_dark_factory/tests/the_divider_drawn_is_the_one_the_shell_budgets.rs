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
    // **Every divider, with what gooey was told**: a `SplitDivider` with no `SplitThickness` is one the shell
    // never sized, and naming both numbers is what tells a wrong reading from a wrong seam.
    let mut dividers =
        world.query::<(&bevy_gooey::SplitDivider, &ComputedNode, &Node, Option<&bevy_gooey::SplitThickness>)>();
    let drawn: Vec<(f32, Option<f32>, bool)> =
        dividers.iter(world).map(|(_, node, plain, told)| (node.size().x, told.map(|t| t.0), plain.display != Display::None)).collect();
    assert!(!drawn.is_empty(), "the shell draws a split divider");
    // **`SplitThickness` is what makes the seam the budget**, and this is the test that proved it needed to be.
    // On `main` before the shell set it, the divider drew **29 px** on this fixture while the sheet's number was
    // 12: gooey's default is `MIN_TARGET_PX` 24 written as a *minimum*, so the node was free to be wider and the
    // drawn seam answered to neither number. The sheet's `divider_w` is now 29 — the measured seam — and the host
    // hands that one number to gooey, so what is reserved is what is drawn.
    for (width, told, visible) in &drawn {
        // A divider the shell never sized still draws gooey's default, and this names it rather than blaming it.
        assert!(
            told.is_some(),
            "a visible divider ({visible}) was drawn at {width} px with no `SplitThickness`, so its seam answers to \
             gooey's default (a 24 px MINIMUM, and 29 measured on the fixture) and not to the sheet (#550)"
        );
        assert!(
            (width - budget).abs() <= 1.0,
            "the divider draws {width} px where `metrics.layout.divider_w` budgets {budget} px and gooey was told \
             {told:?}; a seam wider than its budget eats the viewport, a narrower one leaves a dead strip (#550)"
        );
    }
}
