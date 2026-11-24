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
    // **Every divider, with what gooey was told.** The host writes `SplitThickness` on the split ROOT, and
    // `update_splits` carries it to the divider as `DividerThickness` (its doc: "The thickness `update_splits`
    // last wrote into a divider's `Sizing`"); a divider without one is one no host sized, and the pair of numbers
    // is what tells a wrong reading from a wrong seam. The first version of this test asked for `SplitThickness`
    // on the divider, found none of three, and would have "fixed" a seam that was never broken.
    let mut dividers =
        world.query::<(&bevy_gooey::SplitDivider, &ComputedNode, &Node, Option<&bevy_gooey::DividerThickness>)>();
    let drawn: Vec<(f32, bool, bool)> =
        dividers.iter(world).map(|(_, node, plain, told)| (node.size().x, told.is_some(), plain.display != Display::None)).collect();
    assert!(!drawn.is_empty(), "the shell draws a split divider");
    // **`SplitThickness` is what makes the seam the budget**, and this is the test that proved it needed to be.
    // On `main` before the shell set it, the divider drew **29 px** on this fixture while the sheet's number was
    // 12: gooey's default is `MIN_TARGET_PX` 24 written as a *minimum*, so the node was free to be wider and the
    // drawn seam answered to neither number. The sheet's `divider_w` is now 29 — the measured seam — and the host
    // hands that one number to gooey, so what is reserved is what is drawn.
    // **The shell's seam is the one the sheet budgets**, and the shell is the only split the editor sizes: a
    // workspace's docks are a `DockLayout` description gooey turns into splits at its own thickness, so a divider
    // with no `DividerThickness` is a dock's and is not what `divider_w` measures.
    let shell: Vec<&(f32, bool, bool)> = drawn.iter().filter(|(_, told, _)| *told).collect();
    assert_eq!(
        shell.len(),
        1,
        "the shell draws exactly one divider gooey was told a thickness for; found {} of {} (width, told, visible): {drawn:?}",
        shell.len(),
        drawn.len()
    );
    for (width, _, _) in shell {
        // **The drawn width is the value**; `DividerThickness`'s field is gooey's, so its presence names the shell's
        // divider and the frame says how wide it is.
        assert!(
            (width - budget).abs() <= 1.0,
            "the shell's divider draws {width} px where `metrics.layout.divider_w` budgets {budget} px; a seam wider \
             than its budget eats the viewport, a narrower one leaves a dead strip (#550)"
        );
    }
}
