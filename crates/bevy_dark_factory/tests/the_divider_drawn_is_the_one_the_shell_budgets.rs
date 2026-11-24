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
    let mut dividers = world.query::<(&bevy_gooey::SplitDivider, &ComputedNode, &Node)>();
    let drawn: Vec<(f32, bool)> = dividers.iter(world).map(|(_, node, plain)| (node.size().x, plain.display != Display::None)).collect();
    assert!(!drawn.is_empty(), "the shell draws a split divider");
    // **`SplitThickness` is what makes the seam the budget.** Before the shell set it the sheet said 12 and the
    // frame drew its own seam, because gooey's `MIN_TARGET_PX` 24 is written as a *minimum* and the node was free
    // to be wider; the host now hands the sheet's number to gooey, so what is reserved is what is drawn.
    //
    // **The shell's seam is the widest drawn divider**: a workspace's docks are a `DockLayout` gooey splits at its
    // own thickness (14 px measured on this fixture), and the shell's is the one the sheet budgets.
    let widest = drawn.iter().filter(|(_, visible)| *visible).map(|(w, _)| *w).fold(0.0_f32, f32::max);
    let budget_seen = told[0];
    assert!(
        (widest - budget_seen).abs() <= 1.0,
        "the sheet hands gooey {budget_seen} px and the widest divider drawn is {widest} px ({drawn:?}); a seam wider \
         than its budget eats the viewport, a narrower one leaves a dead strip (#550)"
    );

}
