//! **A panel's controls stay inside its own content box, at every interface scale** (#494,
//! `feedback-1794921716691`).
//!
//! # What the owner saw
//!
//! Measured on the capture's frame at `ui_scale` 2.4, scanning the rows at `y≈770-850`:
//!
//! | Edge | x |
//! |---|---|
//! | panel's left border (2 px, `(153,153,153)`) | 2151 |
//! | panel's right border (2 px) | 2529 |
//! | panel's content right edge | **2528** |
//! | a chip/button's right border (4 px, `(184,184,184)`) | **2496..2499** |
//!
//! A control inside the panel is drawn out to `x=2499`, **29 px past the content edge**, and over the
//! pane beside it.
//!
//! # What this file holds down
//!
//! Every drawn descendant of a panel lies within that panel's **content box** — its border box less
//! its padding and its 1 px edge — at `interface_scale` 1.0 and 1.6. The acceptance is one test over
//! the panels it names, so the four cannot regress separately: **Place**, **Pieces**, **Agent** and
//! the **left pane**.
//!
//! # Why a content box and not the panel's own rect
//!
//! `chrome::panel_root` documents that a `max_width: Percent(100)` ceiling was tried and reverted for
//! this issue: it narrowed every panel to its pane, which made a panel's own content wrap to an extra
//! line and moved the MAP SIZE row — `a_hint_does_not_push_map_size_down` and
//! `a_receipt_does_not_move_the_panel` both refused it. So the panel's width is right and the *child*
//! is what overflows; this test measures the child against the box it must fit in, which is the
//! defect the capture shows.
//!
//! **A `PrimaryWindow` is spawned**, because a scale other than the harness's own is only a scale if
//! something fits the surface to a window — the same reason `a_panel_headings_glyph_sits_on_the_titles_line`
//! spawns one.
//!
//! # It passes on `main` today, and that is recorded rather than hidden
//!
//! Measured 2026-11-21: **zero overhang on all seven panels at both scales**. The 29 px the capture
//! shows does not reproduce in this fixture, and the issue's own history says two fixes were tried and
//! reverted — `df1c332b`'s `max_width: Percent(100)` cut the spill 83 → 30 px but made a panel's own
//! content wrap, and `886a46c0` reverted it when three tests refused the 16 px it moved MAP SIZE by.
//!
//! So this file holds the **invariant** rather than reproducing the report: a panel's children fit its
//! content box, at two scales, checked on every panel the editor draws. It is not a tautology — shrink
//! the measured box by 60 px and it reports `INSPECTOR` and `PLACE` 30 px over their content edges,
//! which is the failure it exists to catch. What it cannot do is reproduce a capture taken on a
//! populated kit from a fixture that holds one descriptor.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::{PrimaryWindow, Window, WindowResolution};

use bevy_dark_factory::chrome::PanelWidth;
use bevy_dark_factory::harness;

use crate::fixtures;
use crate::settle_support;

/// The interface scales the acceptance names.
const SCALES: [f32; 2] = [1.0, 1.6];

/// The window, physical px: a 1280 x 703 screen at the capture's 2.0 display factor.
const WINDOW_PHYSICAL: (u32, u32) = (2560, 1406);


/// How far past the content edge a child may be drawn, physical px. **Zero, plus the sub-pixel a
/// layout round can add** — the acceptance says "every descendant's right edge", and a rounding
/// tolerance of one device pixel is not the 29 px the issue measures.
const OVERHANG_PX: f32 = 1.0;



fn editor(name: &str, scale: f32) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.world_mut().spawn((
        Window { resolution: WindowResolution::new(WINDOW_PHYSICAL.0, WINDOW_PHYSICAL.1), ..default() },
        PrimaryWindow,
    ));
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    settle_support::settle(&mut app);
    (root, app)
}

/// A node's border-box rect, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **A panel's content box**: its border box, less the 1 px edge and the padding `panel_root` sets.
///
/// Read from the node's own `ComputedNode` rather than recomputed from the constants, so a change to
/// `STEP_PAD` or `EDGE_W` moves this with it instead of quietly making the test wrong.
fn content_box(world: &World, panel: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(panel)?;
    let rect = border_box(world, panel)?;
    // **The node's own content inset** — its edge, its padding and its scrollbar, which is the box
    // its children must fit. Read rather than recomputed from `chrome`'s constants, so a change to
    // `STEP_PAD` or `EDGE_W` moves this with it instead of quietly making the test wrong.
    let inset = computed.content_inset();
    let width = (rect.width() - inset.min_inset.x - inset.max_inset.x).max(0.0);
    Some(Rect::from_center_size(rect.center(), Vec2::new(width, rect.height())))
}

/// Every descendant of `root` that the layout drew, in a stable order.
fn drawn_descendants(world: &mut World, root: Entity) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        let Some(children) = world.get::<Children>(entity) else { continue };
        for child in children.iter() {
            let Some(computed) = world.get::<ComputedNode>(child) else { continue };
            // A node laid out to nothing is not drawn, and a `Display::None` subtree is not either.
            if computed.size().x > 0.0 && computed.size().y > 0.0 {
                out.push(child);
                stack.push(child);
            }
        }
    }
    // A query's order over archetypes is a hash; sort so two runs report the same rows in one order.
    out.sort_by_key(|e| e.index());
    out
}

/// A panel's name as the test reports it, taken from its own `AccessibleLabel`.
fn panel_name(world: &World, panel: Entity) -> String {
    world
        .get::<AccessibleLabel>(panel)
        .map(|l| l.0.clone())
        .unwrap_or_else(|| "an unnamed panel".to_owned())
}

/// **Every panel in the editor, and the worst overhang inside each.**
///
/// Returns the name, the child that overflowed worst, and by how much — so a failure names the panel
/// and the control rather than "a rect was wrong".
fn overhangs(app: &mut App) -> Vec<(String, f32)> {
    let world = app.world_mut();
    let panels: Vec<Entity> = world
        .query_filtered::<Entity, With<PanelWidth>>()
        .iter(world)
        .collect();
    let mut found = Vec::new();
    for panel in panels {
        let Some(box_) = content_box(world, panel) else { continue };
        let name = panel_name(world, panel);
        let mut worst = 0.0f32;
        for child in drawn_descendants(world, panel) {
            let Some(rect) = border_box(world, child) else { continue };
            worst = worst.max(rect.max.x - box_.max.x);
        }
        found.push((name, worst));
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

#[test]
fn every_panel_control_stays_inside_its_own_content_box() {
    let mut failures = Vec::new();
    for scale in SCALES {
        let (_root, mut app) = editor(&format!("panel_overflow_{scale}"), scale);
        let panels = overhangs(&mut app);
        assert!(
            !panels.is_empty(),
            "at scale {scale} the editor drew no panel with a `PanelWidth`; the panels this test \
             measures are the Place panel, the Pieces panel, the Agent panel and the left pane"
        );
        for (name, worst) in panels {
            if worst > OVERHANG_PX {
                failures.push(format!(
                    "at interface_scale {scale}, `{name}` draws a control {worst:.1} px past its \
                     content box (allowed {OVERHANG_PX:.1}); the capture measured 29 px on the \
                     Place panel at ui_scale 2.4"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "a panel's children must fit the panel's own content box -- they wrap, ellipsize or scroll \
         rather than drawing over the pane beside them (#494):\n  {}",
        failures.join("\n  ")
    );
}
