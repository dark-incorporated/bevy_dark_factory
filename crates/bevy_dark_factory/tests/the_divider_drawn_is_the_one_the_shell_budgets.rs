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
    // **The sheet's number, read the way the shell reads it**: `metrics()` is the live table `StylePlugin`
    // swaps in, and `compiled_default()` is the sheet before a skin. The test names both, so a run that disagrees
    // says which table it disagrees about rather than leaving the reader to guess.
    //
    // **The sheet's number lives in `style/editor.yaml`, not in `LayoutMetrics::default`**: the compiled sheet is
    // built by reading and merging that YAML (`COMPILED`), and the Rust `Default` is not what runs. Two runs of
    // this test and one of the doc test read 12 while `style.rs` said 29, because the value had been changed in the
    // wrong file — the YAML was, and is, the source of every metric.
    let live = metrics().layout.divider_w;
    let compiled = bevy_dark_factory::style::compiled_default().metrics.layout.divider_w;
    assert_eq!(live, compiled, "the live sheet and the compiled sheet disagree on the divider: {live} vs {compiled}");
    let budget = live;
    // **The shell's own split root**, which is the one entity the editor gives a `SplitThickness` — gooey keeps the
    // divider's copy private ("not for hosts to write"), so the root is how a host names its seam.
    let mut roots = app.world_mut().query::<&bevy_gooey::SplitThickness>();
    let told: Vec<f32> = roots.iter(app.world()).map(|t| t.0).collect();
    assert_eq!(told.len(), 1, "the editor sizes exactly one split — the shell's: {told:?}");

    // **Every divider**, so a failure can say which one it measured.
    let world = app.world_mut();
    // **Every number the divider's node holds**, so a failure says why it is the width it is instead of leaving the
    // reader to model flexbox: the plain `Node`'s width, padding, border and minimum, and the computed box.
    let mut dividers = world.query::<(&bevy_gooey::SplitDivider, &ComputedNode, &Node)>();
    let mut drawn: Vec<(f32, bool)> = Vec::new();
    let mut parts: Vec<String> = Vec::new();
    for (_, node, plain) in dividers.iter(world) {
        drawn.push((node.size().x, plain.display != Display::None));
        parts.push(format!(
            "node width={:?} pad={:?} border={:?} min={:?} :: computed={:?}",
            plain.width, plain.padding, plain.border, plain.min_width, node.size()
        ));
    }
    let inverse_scale = {
        let mut again = world.query::<&ComputedNode>();
        again.iter(world).next().map(bevy::ui::ComputedNode::inverse_scale_factor).unwrap_or(1.0)
    };
    assert!(!drawn.is_empty(), "the shell draws a split divider");
    // **`SplitThickness` is what makes the seam the budget.** Before the shell set it the sheet said 12 and the
    // frame drew its own seam, because gooey's `MIN_TARGET_PX` 24 is written as a *minimum* and the node was free
    // to be wider; the host now hands the sheet's number to gooey, so what is reserved is what is drawn.
    //
    // **The shell's seam is the widest drawn divider**: a workspace's docks are a `DockLayout` gooey splits at its
    // own thickness (14 px measured on this fixture), and the shell's is the one the sheet budgets.
    // **`ComputedNode::size()` is PHYSICAL pixels; the sheet is logical.** This test compared the two for four runs
    // and blamed gooey for the difference: 29 logical drew 34.8 physical (read as 35), 35 logical drew 42.
    // `computed.inverse_scale_factor()` is the logical size, and the border the node reports (`0.8333` where the sheet
    // says `1.0`) is the same scale seen from the other side.
    // `inverse_scale_factor()` is bevy's own conversion from a computed size back to logical px, so this divides by
    // exactly the number the layout multiplied by.
    let drawn_logical: Vec<(f32, bool)> =
        drawn.iter().map(|(width, visible)| (width * inverse_scale, *visible)).collect();
    let widest = drawn_logical.iter().filter(|(_, visible)| *visible).map(|(w, _)| *w).fold(0.0_f32, f32::max);
    let node = budget + 6.0;
    assert!(
        (widest - node).abs() <= 1.0,
        "the sheet reserves {budget} logical px, the divider's node is {node} logical px (grip and borders inside), \
         and the widest divider drawn is {widest} logical px at scale {inverse_scale} ({drawn:?}); the nodes say \
         {parts:?} (#550)"
    );

}
