//! **The split's divider is a seam, not a bar** (#476, `feedback-1794783253218`).
//!
//! # What the owner saw, and what the frame actually holds
//!
//! The note was *"I like this being thick, but maybe only half as thick as this?"*, with a pick whose
//! `rect_px` is `[610, 343, 668, 1344]` — **58 px**. At the capture's `ui_scale` 2.4 that is
//! `58 / 2.4 = 24.17` logical px, which is exactly [`DIVIDER_W`]'s 24: the pick is the divider node,
//! and the node is as wide as the constant says.
//!
//! # The band the issue measured is not this node
//!
//! The issue reports *"a band from x=546 to x=578 — 33 surface px"*. Measured on the same frame at
//! `y=700` and `y=900`: `x=546..549` is a 4 px `(184,184,184)` mark inside the left panel, `550..578`
//! is that panel's own `(5,5,5)` fill, and the rules at `400..401` and `579..580` are the panel's
//! border and its scroll rail. The divider is the `(34,34,34)` block at `612..665` — 54 px of fill
//! inside the node's 58 — and the picks image agrees, its highlight spanning `x=610..667`.
//!
//! So the 33 px the issue derived its "at most 17 px" from is a measurement of the wrong element.
//! What survives is the owner's own word — *half* — so the seam is held to half of 24 at most, and
//! the arithmetic is stated against [`DIVIDER_W`] rather than against a number re-read from a frame.
//!
//! # What this file holds down
//!
//! 1. **One constant decides the divider's thickness, everywhere.** Not "the shell's divider happens
//!    to measure 24 because gooey's `MIN_TARGET_PX` is also 24": the node is written from
//!    [`DIVIDER_W`], so the two cannot agree by coincidence and then drift apart.
//! 2. **The seam is no wider than the constant**, at both interface scales — which is half what the
//!    owner was shown, because the constant is half what it was.
//! 3. **The grip is still grabbable**: a drag still moves the fraction by the distance dragged over
//!    the room, on a channel narrower than gooey's own target size.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy_cauldron::chrome::DIVIDER_W;
use bevy_cauldron::harness;

use crate::fixtures;

/// The interface scales this test lays the editor out at: the size the constants were written for,
/// and the capture's own 2.4 (the editor's 1.2 times the display's 2.0).
const SCALES: [f32; 2] = [1.0, 2.4];

/// **The scale the harness builds at** — [`bevy_cauldron::chrome::EDITOR_UI_SCALE`], which is what the
/// editor's own drag test (`the_shells_divider_moves_under_a_drag`) drags at. The drag test below uses
/// this rather than 2.4 because a pointer's distance is in logical px and the arithmetic
/// (`split_drag`, `start + distance / extent`) is `ui_scale`-independent only at the scale the editor
/// was built for; at 2.4 the harness's physical surface makes "100 logical px" ambiguous.
const HARNESS_SCALE: f32 = bevy_cauldron::chrome::EDITOR_UI_SCALE;

/// How far a measured edge may stand from the constant it is held to, logical px: layout rounds.
const SLACK_PX: f32 = 0.5;

/// The most the seam between the two panes may cost, logical px — **the constant itself**, plus the
/// same rounding slack.
///
/// Written against [`DIVIDER_W`] rather than as a literal, because that is the whole claim: one named
/// constant decides the thickness, so the seam measures the constant. It was written as half the
/// constant for one round, which is not a second claim but a second arithmetic — and it made the test
/// fail on a correct build, which is how the mistake was found.
const SEAM_MAX_PX: f32 = DIVIDER_W + SLACK_PX;

/// Frames a layout needs to settle after a change of scale.
const SETTLE_FRAMES: usize = 12;

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// The editor on a one-piece fixture, settled, at `scale`.
fn editor(name: &str, scale: f32) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    settle(&mut app);
    (root, app)
}

/// A node's border-box rect, in logical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at * computed.inverse_scale_factor(), computed.size() * computed.inverse_scale_factor()))
}

/// Every divider the editor has laid out, as `(entity, width in logical px)`.
///
/// **Only the laid-out ones**: the dock's body is a hidden `DockBody` (`chrome::spawn_frame`) whose
/// pages are built but never laid out, so their dividers measure 0. A 0 px divider is not one anybody
/// can see or grab, and a constant cannot be said to decide a width no layout produced.
fn laid_out_dividers(app: &mut App) -> Vec<(Entity, f32)> {
    let world = app.world_mut();
    let mut query = world.query_filtered::<(Entity, &ComputedNode), With<bevy_gooey::SplitDivider>>();
    let mut found: Vec<(Entity, f32)> = query
        .iter(world)
        .map(|(entity, node)| (entity, node.size().x * node.inverse_scale_factor()))
        .filter(|(_, wide)| *wide > 0.0)
        .collect();
    // A query's order over archetypes is a hash, so sort: two runs report the same rows in one order.
    found.sort_by(|(a, w), (b, v)| a.index().cmp(&b.index()).then(w.total_cmp(v)));
    found
}

/// **The divider's node width is [`DIVIDER_W`], at every interface scale and for every divider.**
#[test]
fn every_laid_out_divider_is_as_wide_as_the_editors_constant() {
    let mut refused: Vec<String> = Vec::new();
    for scale in SCALES {
        let (_root, mut app) = editor(&format!("divider_width_{scale}_{}", std::process::id()), scale);
        let found = laid_out_dividers(&mut app);
        assert!(
            !found.is_empty(),
            "the shell is a gooey split, so it must have a laid-out divider at scale {scale}; none was found"
        );
        for (entity, wide) in found {
            if (wide - DIVIDER_W).abs() > SLACK_PX {
                refused.push(format!(
                    "[scale {scale}] divider {entity} is {wide:.2} px wide; the editor's own constant says {DIVIDER_W:.2}"
                ));
            }
        }
    }
    assert!(
        refused.is_empty(),
        "a divider's thickness is one named constant, and these disagree with it:\n{}",
        refused.join("\n")
    );
}

/// **The seam between the shell's two panes is no wider than that constant**, at both scales.
///
/// Measured as the gap between the left pane's right edge and the viewport's left edge — what a
/// pointer must cross to go from one to the other — rather than as the divider's own box, so a pane
/// carrying a margin of its own could not hide half of it.
#[test]
fn the_seam_between_the_panes_is_no_wider_than_the_constant() {
    let mut refused: Vec<String> = Vec::new();
    for scale in SCALES {
        let (_root, app) = editor(&format!("divider_seam_{scale}_{}", std::process::id()), scale);
        let world = app.world();
        let frame = world.resource::<bevy_cauldron::chrome::Frame>();
        let (left, viewport) = (frame.left, frame.viewport);
        let left_box = border_box(world, left).unwrap_or_else(|| panic!("the left region is laid out at scale {scale}"));
        let right_box = border_box(world, viewport).unwrap_or_else(|| panic!("the viewport region is laid out at scale {scale}"));
        let seam = right_box.min.x - left_box.max.x;
        if seam > SEAM_MAX_PX {
            refused.push(format!(
                "[scale {scale}] the seam between the panes is {seam:.2} px; {SEAM_MAX_PX:.2} is the most {DIVIDER_W:.2} allows \
                 (the left pane ends at {:.1}, the viewport starts at {:.1})",
                left_box.max.x,
                right_box.min.x
            ));
        }
    }
    assert!(refused.is_empty(), "the divider is a bar a pointer must cross:\n{}", refused.join("\n"));
}

/// **The grip is still grabbable on the narrower channel.**
///
/// The drag is the editor's own: press the divider's centre and move `DRAG_PX` along the axis, and the
/// fraction must move by that distance over the split's room — the arithmetic
/// `the_shells_divider_moves_under_a_drag` holds the whole divider to. A seam narrowed past the point
/// where the pointer can find it is a seam nobody can move.
#[test]
fn a_drag_on_the_narrow_divider_still_moves_the_split() {
    /// How far the pointer is dragged, logical px.
    const DRAG_PX: f32 = 100.0;
    /// How far the fraction may stand from the arithmetic, as a share of the split's room.
    const FRACTION_SLACK: f32 = 0.01;

    let (_root, mut app) = editor(&format!("divider_grab_{}", std::process::id()), HARNESS_SCALE);
    // The widest laid-out divider: the shell's own channel, as `the_shells_divider_moves_under_a_drag` picks it.
    let (divider, rect, room) = {
        let world = app.world_mut();
        let mut query = world.query_filtered::<(Entity, &ComputedNode), With<bevy_gooey::SplitDivider>>();
        let picked = query
            .iter(world)
            .filter(|(_, node)| node.size().x > 0.0)
            .max_by(|(_, a), (_, b)| a.size().x.total_cmp(&b.size().x))
            .map(|(entity, _)| entity)
            .expect("the shell has a laid-out divider");
        let rect = border_box(world, picked).expect("the divider is laid out");
        let parent = world.get::<ChildOf>(picked).expect("the divider is in a split").parent();
        let node = world.get::<ComputedNode>(parent).expect("the split is laid out");
        let room = node.size().x * node.inverse_scale_factor();
        (picked, rect, room)
    };
    let _ = divider;
    let parent = app.world().get::<ChildOf>(divider).expect("a parent").parent();

    let handle = app
        .world()
        .get_resource::<bevy_cauldron::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    // `PointerInput` wants physical positions and the rect above is logical, so the scale is applied
    // here — aiming a logical point at a scaled interface lands on whatever widget is there.
    let scale = app.world().resource::<UiScale>().0;
    let from = rect.center() * scale;
    for action in [
        PointerAction::Move { delta: Vec2::ZERO },
        PointerAction::Press(PointerButton::Primary),
    ] {
        let location = Location {
            target: NormalizedRenderTarget::Image(ImageRenderTarget { handle: handle.clone(), scale_factor: 1.0 }),
            position: from,
        };
        app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
        app.update();
    }
    // **The fraction as the press leaves it**, not as it was read before the press: `size_the_shell_split`
    // and `the_left_panel_follows_its_pane` settle the pane in the same frames, and a drag's own effect is
    // the difference it makes, not the difference from a number two systems have since rewritten.
    let start = app.world().get::<bevy_gooey::SplitFraction>(parent).expect("a split").0;
    let to = from + Vec2::new(DRAG_PX * scale, 0.0);
    {
        let location = Location {
            target: NormalizedRenderTarget::Image(ImageRenderTarget { handle: handle.clone(), scale_factor: 1.0 }),
            position: to,
        };
        app.world_mut()
            .write_message(PointerInput::new(PointerId::Mouse, location, PointerAction::Move { delta: Vec2::new(DRAG_PX, 0.0) }));
        app.update();
    }
    let release = Location {
        target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }),
        position: to,
    };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, release, PointerAction::Release(PointerButton::Primary)));
    settle(&mut app);
    let after = app.world().get::<bevy_gooey::SplitFraction>(parent).expect("a split").0;

    let want = start + DRAG_PX / room;
    assert!(
        (after - want).abs() < FRACTION_SLACK,
        "a {DRAG_PX} px drag of a {room:.1} px split should take the fraction {start:.4} -> {want:.4}; \
         the {:.2} px channel answered {after:.4}",
        rect.width()
    );
}
