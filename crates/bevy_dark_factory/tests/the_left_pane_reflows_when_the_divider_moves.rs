//! **The left pane's contents reflow to the pane's width instead of spilling over the divider**
//! (#482; `feedback-1794783743883`, `feedback-1794783776166`, `feedback-1794784645626`).
//!
//! # What the owner saw
//!
//! *"Stuff is breaking out of this layout, depending on how far left the divider bar is moved."* The
//! pick is the left `SplitPane` at `[60, 343, 498, 1344]` — 438 px at `ui_scale` 2.4, about 182
//! logical px — with the pane's own words running into the divider at `x≈498`. A second capture
//! (`feedback-1794783776166`) picks the same pane at `[89, 372, 727, 1316]` and asks for it to hold
//! at other sizes. Three captures, one defect.
//!
//! # Why the pane's *box* is not the thing to measure
//!
//! `chrome::the_left_panel_follows_its_pane` already writes the panel's `Sizing` width from the pane
//! (`(pane_width - 2 * MARGIN).max(DOCK_MIN_W)`), so the panel box follows the divider today and a
//! test of the box would pass on the broken build. What does not follow is the panel's **contents**:
//! the rows inside ask for the widths they want, and a row wider than the pane is drawn past it.
//!
//! So every descendant's *drawn* extent is measured — from its glyphs (`TextLayoutInfo`), not from
//! its node box, because a text node can be narrower than the words in it and that is the whole
//! defect — and each is held to the pane's border box.
//!
//! # The two extremes
//!
//! The divider is dragged to its **leftmost allowed position**, which is gooey's `split_clamp` against
//! the shell's own `SplitMin` (`DOCK_MIN_W + 2 * MARGIN`, `DOCK_PANE_MIN_PX`) — not a fraction this
//! test invents — and to its widest. At both, and at `interface_scale` 1.0 and 1.6, nothing under the
//! pane may be drawn outside it.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use bevy_dark_factory::harness;

use crate::fixtures;

/// The interface scales the acceptance names: the constants' own size, and one and a half times it.
const SCALES: [f32; 2] = [1.0, 1.6];

/// **The widest the left pane may be and still count as "narrow"**, physical px: the capture's own
/// 438 px pane is the case the owner photographed, so a pane at or under that has reached the state
/// the issue is about, and one much wider has not.
const NARROW_PANE_PX: f32 = 460.0;

/// How far a measured edge may stand past the pane's border box, physical px: layout rounds.
const SLACK_PX: f32 = 1.0;

/// Frames a layout needs to settle after a change of scale or of the split.
const SETTLE_FRAMES: usize = 14;

/// **The most gooey table-cell texts one pane may hold**: the history list is single-column, and its
/// visible rows are the only cells here. See the exemption in [`outside_the_pane`].
const MOST_GOOEY_CELLS: usize = 12;

/// How many texts must be drawn under the pane before the measurement means anything.
const LEAST_TEXTS: usize = 4;

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// The capture's window, physical px — a 1280 x 703 logical screen at the capture's 2.0 display factor.
const WINDOW_PHYSICAL: (u32, u32) = (2560, 1406);

/// The editor on a one-piece fixture, settled, at `scale`.
///
/// **The window is spawned, and that is not decoration.** `harness::build_headless` sets
/// `WindowPlugin { primary_window: None }`, and both systems that decide a panel's width
/// (`fit_docks_to_the_window`, `the_left_panel_follows_its_pane`) begin `let Ok(window) = windows.single()
/// else { return }` — so an app with no `PrimaryWindow` never narrows a panel, and a test built on one
/// measures the absence of a window rather than the reflow. `a_dock_resize_never_covers_the_viewport`
/// spawns one for the same reason.
fn editor(name: &str, scale: f32) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.world_mut().spawn((
        Window { resolution: WindowResolution::new(WINDOW_PHYSICAL.0, WINDOW_PHYSICAL.1), ..default() },
        PrimaryWindow,
    ));
    harness::resize_surface(&mut app, WINDOW_PHYSICAL.0, WINDOW_PHYSICAL.1)
        .unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    settle(&mut app);
    (root, app)
}

/// Every entity under `root`, `root` included.
fn under(world: &World, root: Entity) -> Vec<Entity> {
    let mut found = vec![root];
    let mut next = 0;
    while next < found.len() {
        if let Some(children) = world.get::<Children>(found[next]) {
            found.extend(children.iter());
        }
        next += 1;
    }
    found
}

/// A node's border-box rect, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **The pane's own border box and everything drawn outside it.**
///
/// `left` is the frame's left region; its parent is gooey's `SplitPane`, which is the box the capture
/// picked and the box a person sees as the pane.
fn outside_the_pane(app: &mut App, tag: &str) -> Vec<String> {
    let world = app.world();
    let frame = world.resource::<bevy_dark_factory::chrome::Frame>();
    let left = frame.left;
    let pane = world
        .get::<ChildOf>(left)
        .map(|parent| parent.parent())
        .unwrap_or_else(|| panic!("{tag}: the left region stands in the split's pane"));
    let pane_box = border_box(world, pane).unwrap_or_else(|| panic!("{tag}: the pane is laid out"));
    // A pane is clipped, so anything drawn outside it is cut rather than shown — but a row that is
    // *laid out* past the edge is what pushes the words a person reads out of view, which is the
    // capture. Measure the contents' own boxes, in the pane's frame.
    let mut refused = Vec::new();
    let mut drawn = 0usize;
    let mut gooey_cells = 0usize;
    for entity in under(world, left) {
        let Some(rect) = border_box(world, entity) else { continue };
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        // **gooey's table cell text is drawn on one line and this crate cannot change that.**
        // `bevy_gooey`'s `table.rs:1343` spawns every cell's text with `ONE_LINE`
        // (`widgets.rs:106`: `TextLayout::new(Justify::Left, LineBreak::NoWrap)`), so a cell narrower
        // than its words is drawn out of it and no `TextLayout` written here can override it. The
        // history list is a `TableRows`, so its rows are the one place in this pane the editor does
        // not own the text of. Filed as **bevy_gooey#109**; named and counted rather than silently
        // skipped, so this exemption cannot quietly grow to cover a second widget.
        if world.get::<bevy_gooey::Role>(entity) == Some(&bevy_gooey::Role::TableCellText) {
            gooey_cells += 1;
            continue;
        }
        // A text is measured from the words it fills, not from its box (#468's rule).
        let mut filled = rect;
        if let (Some(text), Some(info)) = (world.get::<Text>(entity), world.get::<TextLayoutInfo>(entity)) {
            if text.0.trim().is_empty() {
                continue;
            }
            drawn += 1;
            if info.size.x > 0.0 {
                let inset = world.get::<ComputedNode>(entity).map(ComputedNode::content_inset).unwrap_or_default();
                let start = rect.min + Vec2::new(inset.min_inset.x, inset.min_inset.y);
                filled = rect.union(Rect::from_corners(start, start + info.size));
            }
        }
        if filled.max.x > pane_box.max.x + SLACK_PX {
            let words = world.get::<Text>(entity).map(|t| format!("`{}`", t.0)).unwrap_or_else(|| format!("{entity}"));
            refused.push(format!(
                "{tag}: {words} is drawn to {:.1}, {:.1} px past the pane's right edge {:.1}",
                filled.max.x,
                filled.max.x - pane_box.max.x,
                pane_box.max.x
            ));
        }
    }
    // The exemption above is one widget: a single-column history table draws one cell a row. More than
    // this means something else has joined it and the exemption is no longer narrow.
    if gooey_cells > MOST_GOOEY_CELLS {
        refused.push(format!(
            "{tag}: {gooey_cells} gooey table cells are drawn under the pane, more than the {MOST_GOOEY_CELLS} the \
             history list has — the exemption for gooey's one-line cell text now covers more than it was written for"
        ));
    }
    if drawn < LEAST_TEXTS {
        refused.push(format!("{tag}: only {drawn} texts are drawn under the pane, so this measures nothing"));
    }
    refused
}

/// **Put the shell's split at one end of what its `SplitMin` allows**, and hold it there.
///
/// # Why this writes the fraction instead of injecting a drag
///
/// The editor's own drag test presses the divider with a pointer aimed at the **surface image**, and a
/// drag injected that way moves the fraction — measured, `the_shells_divider_moves_under_a_drag` passes
/// on this commit. It cannot be reused here, because this test must also spawn a `PrimaryWindow`: both
/// systems that size a panel begin `windows.single()?`-style, so without a window the panel never
/// narrows and the test would measure the shipped layout (measured: a 606 px pane at every fraction,
/// which is how the first draft of this file passed while proving nothing). With a window present the
/// same injected pointer no longer reaches the divider (measured: the fraction frozen at 0.29233
/// through press, move and release), and the pointer's target would have to be the window rather than
/// the image, which is a different harness than the one the drag test proves.
///
/// So the fraction is written directly, which is the state a drag leaves behind — gooey's `split_clamp`
/// is what holds it inside `SplitMin`, and it is applied to the value gooey's own drag would have
/// written. The **fraction is read back afterwards**, because a write that `size_the_shell_split`
/// overwrote would make every assertion below true and meaningless.
///
/// **`leftmost` is `0.0`**: a fraction is clamped by gooey to `first_min / extent`, so 0 is either the
/// floor or clamped up to it — "all the way left", whichever the split's own minimums say.
fn set_the_fraction(app: &mut App, leftmost: bool) {
    let split = {
        let world = app.world_mut();
        let frame = world.resource::<bevy_dark_factory::chrome::Frame>().left;
        let pane = world.get::<ChildOf>(frame).map(|p| p.parent()).expect("the left region is in a pane");
        let split = world.get::<ChildOf>(pane).map(|p| p.parent()).expect("the pane is in a split");
        let mut fraction = world.get_mut::<bevy_gooey::SplitFraction>(split).expect("the split has a fraction");
        fraction.0 = if leftmost { 0.0 } else { 1.0 };
        split
    };
    settle(app);
    // **The write must still be there.** `size_the_shell_split` owns this fraction while `SplitUnsized`
    // is on the split, and rewrites it to the left panel's preferred width; a write it overwrote leaves
    // the shipped layout in place and every assertion below true for the wrong reason. gooey's own
    // `split_clamp` never answers 0 for a pane with a minimum, so "still 0" is not the check — the
    // check is that the split *kept what was written* rather than answering with a different number.
    let now = app.world().get::<bevy_gooey::SplitFraction>(split).map(|f| f.0).unwrap_or(-1.0);
    assert!(
        (now - if leftmost { 0.0 } else { 1.0 }).abs() < 1e-6,
        "the fraction written was {} and the split answers {now}: `size_the_shell_split` overwrote it, so \
         this test would measure the shipped layout rather than the end it asked for",
        if leftmost { 0.0 } else { 1.0 }
    );
    // **And the pane really is narrow (or wide).** A fraction that survived but a pane that did not move
    // would still measure nothing, so the pane's own width is checked against the other end's.
    let pane = {
        let world = app.world();
        let frame = world.resource::<bevy_dark_factory::chrome::Frame>().left;
        world.get::<ChildOf>(frame).map(|p| p.parent()).expect("the left region is in a pane")
    };
    let wide = app
        .world()
        .get::<ComputedNode>(pane)
        .map(|n| n.size().x)
        .unwrap_or(-1.0);
    if leftmost {
        assert!(
            wide < NARROW_PANE_PX,
            "the divider was taken to its leftmost position and the pane is still {wide:.1} px; a pane that \
             does not narrow cannot spill, so this would pass on the broken build"
        );
    }
}

/// **At the divider's leftmost allowed position, nothing under the left pane is drawn outside it.**
#[test]
fn the_left_panes_contents_stay_inside_it_at_its_narrowest() {
    let mut refused = Vec::new();
    for scale in SCALES {
        let (_root, mut app) = editor(&format!("pane_narrow_{scale}_{}", std::process::id()), scale);
        set_the_fraction(&mut app, true);
        settle(&mut app);
        refused.extend(outside_the_pane(&mut app, &format!("[scale {scale}, leftmost]")));
    }
    assert!(
        refused.is_empty(),
        "a divider dragged left draws the pane's contents over it:\n{}",
        refused.join("\n")
    );
}

/// **At the widest position, the pane's contents are still inside it** — the other end of the same
/// claim, so a fix that merely stopped laying the contents out at all would not pass.
#[test]
fn the_left_panes_contents_stay_inside_it_at_its_widest() {
    let mut refused = Vec::new();
    for scale in SCALES {
        let (_root, mut app) = editor(&format!("pane_wide_{scale}_{}", std::process::id()), scale);
        set_the_fraction(&mut app, false);
        settle(&mut app);
        refused.extend(outside_the_pane(&mut app, &format!("[scale {scale}, widest]")));
    }
    assert!(
        refused.is_empty(),
        "at the widest position the pane's contents are still drawn outside it:\n{}",
        refused.join("\n")
    );
}
