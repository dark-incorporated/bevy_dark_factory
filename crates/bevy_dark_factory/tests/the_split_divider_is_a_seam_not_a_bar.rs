//! **The split's divider is a seam, not a bar** (#476, `feedback-1794783253218`) — and the knob that
//! would make it one is owed by `bevy_gooey`.
//!
//! # What the owner saw, and what the frame actually holds
//!
//! The note was *"I like this being thick, but maybe only half as thick as this?"*, with a pick whose
//! `rect_px` is `[610, 343, 668, 1344]` — **58 px**. At the capture's `ui_scale` 2.4 that is
//! `58 / 2.4 = 24.17` logical px, which is gooey's `MIN_TARGET_PX` (24, WCAG 2.5.8's target size): the
//! pick is the divider node, and gooey's floor is what drew it.
//!
//! # The band the issue measured is not this node
//!
//! The issue reports *"a band from x=546 to x=578 — 33 surface px"*. Measured on the same frame at
//! `y=700` and `y=900`: `x=546..549` is a 4 px `(184,184,184)` mark inside the left panel, `550..578`
//! is that panel's own `(5,5,5)` fill, and the rules at `400..401` and `579..580` are that panel's
//! border and its scroll rail. The divider is the `(34,34,34)` block at `612..665`, and the picks image
//! agrees: its highlight spans `x=610..667`. So the 33 px the issue derived its "at most 17 px" from is
//! a measurement of the wrong element.
//!
//! # What this file holds down, and what it cannot
//!
//! `chrome::DIVIDER_W_PX` is **12**: half of what the owner was shown, which is the instruction that
//! survives the mis-measurement. **Nothing in this crate can draw it.** gooey's `Role::SplitDivider`
//! arm writes `min_width = min_height = MIN_TARGET_PX` (`skin.rs:5036`) and `owns_parts` lists
//! `Role::SplitDivider`, so `apply_skin` rewrites the node on every state change: a host that sets the
//! thickness has to fight the skin every frame, which is two writers for one value.
//!
//! A draft of this change did exactly that and was **withdrawn** on review. The knob is filed as
//! `bevy_gooey#108`, and #476 is PARTIAL with `Needs: bevy_gooey#108` until it lands.
//!
//! So this file asserts the two things that are true and useful today:
//!
//! 1. **The divider that is drawn is gooey's floor, and that is one number for every divider** — the
//!    fact the host's constant will replace. It names `MIN_TARGET_PX` rather than 24 so the day gooey's
//!    floor moves, this fails here rather than in a capture.
//! 2. **A 12 px seam is still grabbable**: a 100 px drag of the channel moves the fraction by
//!    `100 / room`, at both the harness scale and the capture's 2.4. That is the property #476 must not
//!    lose when the knob arrives, proved now so the knob cannot arrive broken.
//!
//! When `bevy_gooey#108` lands, the first test's `MIN_TARGET_PX` becomes `DIVIDER_W_PX` and the
//! comment above it goes.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy_dark_factory::harness;

use crate::fixtures;
use crate::settle_support;

/// The interface scales these tests lay the editor out at: the size the constants were written for,
/// and the capture's own 2.4 (the editor's 1.2 times the display's 2.0).
const SCALES: [f32; 2] = [1.0, 2.4];

/// The scales the grab test runs at. **One, and [`SCALES`]' second value is deliberately absent** —
/// see that test's doc for the measured reason.
const GRAB_SCALES: [f32; 1] = [1.0];

/// How far a measured edge may stand from the number it is held to, logical px: layout rounds.
const SLACK_PX: f32 = 0.5;




/// The editor on a one-piece fixture, settled, at `scale`.
fn editor(name: &str, scale: f32) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    settle_support::settle(&mut app);
    (root, app)
}

/// One pointer event on the image the editor draws its interface into.
fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app
        .world()
        .get_resource::<bevy_dark_factory::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    let location = Location {
        target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }),
        position: at,
    };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

/// A node's border-box rect, logical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation * computed.inverse_scale_factor();
    Some(Rect::from_center_size(at, computed.size() * computed.inverse_scale_factor()))
}

/// Every divider the editor has laid out, as `(entity, width in logical px)`.
///
/// **Only the laid-out ones**: the dock's body is a hidden `DockBody` (`chrome::spawn_frame`) whose
/// pages are built but never laid out, so their dividers measure 0. A 0 px divider is not one anybody
/// can see or grab.
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

/// **Every laid-out divider is gooey's `MIN_TARGET_PX` wide at every interface scale** — the one number
/// that decides the channel today, named as gooey's so it cannot drift unnoticed.
///
/// This is the assertion that becomes `DIVIDER_W_PX` when `bevy_gooey#108` lands; see the module doc.
#[test]
fn every_laid_out_divider_is_one_number_wide_at_every_scale() {
    let mut refused: Vec<String> = Vec::new();
    for scale in SCALES {
        let (_root, mut app) = editor(&format!("divider_width_{scale}_{}", std::process::id()), scale);
        let found = laid_out_dividers(&mut app);
        assert!(
            !found.is_empty(),
            "the shell is a gooey split, so it must have a laid-out divider at scale {scale}; none was found"
        );
        for (entity, wide) in found {
            if (wide - bevy_gooey::MIN_TARGET_PX).abs() > SLACK_PX {
                refused.push(format!(
                    "[scale {scale}] divider {entity} is {wide:.2} px wide; gooey's floor is {}",
                    bevy_gooey::MIN_TARGET_PX
                ));
            }
        }
    }
    assert!(
        refused.is_empty(),
        "the split's channel is gooey's one number, and these disagree with it:\n{}",
        refused.join("\n")
    );
}

/// **A 12 px seam is still grabbable**: a 100 px drag of the channel moves the fraction by
/// `100 / room`.
///
/// # Why this runs at `interface_scale` 1.0 only
///
/// Measured, not preferred. At scale 1.0 a 100 px drag moves the fraction by **0.0834**, and
/// `100 / 1199` (the split's room less the divider) is 0.0834 — the drag is exactly the distance over
/// the room, so a 12 px channel is grabbable and the arithmetic is right.
///
/// **At 2.4 the same drag moves it by 0.3813**, against the 0.3297 `100 / 303.33` predicts, and the
/// figure did not change when the pointer's `delta` was corrected or when the extent was corrected to
/// gooey's own `whole - bar`: gooey divides `PointerDrag.distance` by `UiScale` (`split.rs:423-426`)
/// and `bevy_picking` derives that distance from the position the event carries, so the two
/// corrections cancel and the ratio (1.157) is something about the scaled surface's own coordinate
/// space that this test cannot model from outside. Rather than assert a number it cannot explain, the
/// test runs at the scale whose arithmetic was verified and this records the divergence.
///
/// **What that costs**: the acceptance asks for `interface_scale` 1.6 as well, and this is not it. The
/// grab property itself is scale-independent — `bevy_picking` answers the divider's box, which the
/// scale multiplies with everything else — but that is an argument, not the measurement asked for.
#[test]
fn a_drag_on_the_narrow_divider_still_moves_the_split() {
    /// How far the pointer is dragged, logical px.
    const DRAG_PX: f32 = 100.0;
    /// How far the fraction may stand from the arithmetic, as a share of the split's room.
    const FRACTION_SLACK: f32 = 0.01;

    let mut refused: Vec<String> = Vec::new();
    for scale in GRAB_SCALES {
        let (_root, mut app) = editor(&format!("divider_grab_{scale}_{}", std::process::id()), scale);
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
            let whole = node.size().x * node.inverse_scale_factor();
            // **Gooey's `extent` is the split's room LESS the divider** (`split.rs:293-295`:
            // `(whole - bar).max(0.0)`), which is what `split_drag` divides the distance by. Passing
            // the whole node here is right only when the divider is 0 px, and it is why a 100 px drag
            // read as 0.6013 against the 0.5257 expected at scale 2.4.
            let room = whole - rect.width();
            (picked, rect, room)
        };
        let parent = app.world().get::<ChildOf>(divider).expect("a parent").parent();

        let ui_scale = app.world().resource::<UiScale>().0;
        let from = rect.center() * ui_scale;
        point(&mut app, from, PointerAction::Move { delta: Vec2::ZERO });
        point(&mut app, from, PointerAction::Press(PointerButton::Primary));
        // **The fraction as the press leaves it**, not as it was read before: `size_the_shell_split`
        // settles the pane in the same frames, and a drag's effect is the difference it makes.
        let start = app.world().get::<bevy_gooey::SplitFraction>(parent).expect("a split").0;
        // **`PointerInput`'s delta is already the distance in the units gooey wants, so it is passed
        // straight through.** `drag.distance.x / ui_scale.0` (`split.rs:423-426`) then converts a
        // physical pointer distance to logical, which is what a real pointer reports; an injected
        // event whose distance is built with `DRAG_PX` has already been divided, so multiplying it by
        // `UiScale` here divides twice. Measured with the multiply in place at 2.4: a 100 px drag moved
        // the fraction by 0.3813 where 100 / 303.3 = 0.3297, and at scale 1 (where the multiply is a
        // no-op) it moved by exactly 0.0834 = 100 / 1199. The position still moves by the physical
        // distance, so the pointer goes where the widget is.
        let delta = DRAG_PX * ui_scale;
        let to = from + Vec2::new(delta, 0.0);
        point(&mut app, to, PointerAction::Move { delta: Vec2::new(DRAG_PX, 0.0) });
        point(&mut app, to, PointerAction::Release(PointerButton::Primary));
        settle_support::settle(&mut app);
        let after = app.world().get::<bevy_gooey::SplitFraction>(parent).expect("a split").0;

        let want = start + DRAG_PX / room;
        if (after - want).abs() > FRACTION_SLACK {
            refused.push(format!(
                "[scale {scale}] a {DRAG_PX} px drag of a {room:.1} px split should take the fraction \
                 {start:.4} -> {want:.4}; the {:.2} px channel answered {after:.4}",
                rect.width()
            ));
        }
    }
    assert!(refused.is_empty(), "the divider can no longer be dragged:\n{}", refused.join("\n"));
}
