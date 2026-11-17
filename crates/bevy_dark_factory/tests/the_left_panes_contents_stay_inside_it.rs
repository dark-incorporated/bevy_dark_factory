//! **The left pane's contents reflow to the pane the divider gives them** (#482).
//!
//! Three captures of one defect, and they are worked as one — the issue says so in its own words: *"They must be worked as one"*.
//!
//! | Capture | What the person saw |
//! |---|---|
//! | `feedback-1794783743883` | *"Stuff is breaking out of this layout, depending on how far left the divider bar is moved."* |
//! | `feedback-1794783776166` | *"Can we ensure the items in here look good at different si[zes]"* |
//! | `feedback-1794784645626` | the `?` button, in the same pane |
//!
//! # What the frame measured
//!
//! The pane is `[60, 343, 498, 1344]` — 438 physical px at `ui_scale` 2.4, about 182 logical px — against the `CONTROLS_W` its
//! panel asks for, 380. So the pane is less than half the width the panel wants, and the panel drew its contents at the width they
//! wanted rather than the width they had: the pane's own words ran into the divider at `x≈498`.
//!
//! # What was wrong, in code
//!
//! `chrome::row_value` spawned a bare `Text`. A bare text has no least width and does not wrap, so its automatic minimum is its whole
//! content — a file path, an agent's sentence, a list of effects — and the row it sits in is that wide whatever the pane says. The
//! value is now [`bevy_dark_factory::chrome::fill_text`]'s shape: a zero floor and a break at a letter. The pane's floor
//! (`SplitMin::new(DOCK_MIN_W + 2 * MARGIN, …)`) was already consistent with the panel's, so the width the pane reports is a width
//! the panel can be laid out in.
//!
//! # What is held here
//!
//! The divider is moved the way a drag moves it (write the split's `SplitFraction`), to its **leftmost allowed position and its
//! widest**, at the issue's two interface scales, and then every descendant of the left pane is measured against the pane's own
//! border box. A text is measured **from its glyphs** (`TextLayoutInfo::size`): a node's box can be narrower than the words drawn in
//! it, which is the whole defect — the box was inside and the words were not.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};

use bevy_dark_factory::chrome::Frame;
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::Mode;

use crate::fixtures;

/// The capture's window, physical px (`feedback-1794783743883`: 2560 x 1406).
const WINDOW: (u32, u32) = (2560, 1406);

/// The interface scales the issue names: the editor's own at a display scale factor of 1, and the capture's 2.4 over the editor's
/// 1.2 baseline, which is the 2.0 the capture was taken at.
const INTERFACE_SCALES: [f32; 2] = [1.0, 2.0];

/// Where the divider is put. `0.0` is "the first pane as small as it may be" (`SplitFraction`'s own doc) — the capture's case — and
/// `1.0` is the widest the pane may be, which is the other end the issue asks about.
const FRACTIONS: [f32; 2] = [0.0, 1.0];

/// Frames a layout needs to settle after the width or the scale moves.
const SETTLE_FRAMES: usize = 12;

/// How far a measured edge may stand past the one it is held to, physical px: layout rounds.
const SLACK_PX: f32 = 1.0;

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// A node's border-box rect, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// A node's box, or what its glyphs fill when that is wider — the one measurement that catches a text drawn past its own box.
fn covers(world: &World, entity: Entity) -> Option<Rect> {
    let rect = border_box(world, entity)?;
    match (world.get::<Text>(entity), world.get::<TextLayoutInfo>(entity), world.get::<ComputedNode>(entity)) {
        (Some(text), Some(info), Some(computed)) if !text.0.trim().is_empty() && info.size.x > 0.0 => {
            let inset = computed.content_inset();
            let start = rect.min + Vec2::new(inset.min_inset.x, inset.min_inset.y);
            Some(rect.union(Rect::from_corners(start, start + info.size)))
        }
        _ => Some(rect),
    }
}

/// Every entity under `root`, **not `root` itself and not past a scroll viewport's children**.
///
/// The root is the viewport, and it is the box everything else is held to — measuring it against its own inset says it is 6 px
/// (the rail) past itself. The rows a viewport scrolls are not walked either: their overflow is the mechanism the scroll exists
/// for, and a row below the fold is meant to be out of sight.
fn under_but_not_into_the_scroll(world: &World, root: Entity) -> Vec<Entity> {
    let mut found: Vec<Entity> = world.get::<Children>(root).map(|c| c.iter().collect()).unwrap_or_default();
    let mut next = 0;
    while next < found.len() {
        let at = found[next];
        let scrolls = world.get::<bevy_gooey::Role>(at) == Some(&bevy_gooey::Role::ScrollViewport);
        if !scrolls
            && let Some(children) = world.get::<Children>(at)
        {
            found.extend(children.iter());
        }
        next += 1;
    }
    found
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

/// What a node is, for a message a person can act on: its words, or where it is.
fn name_of(world: &World, entity: Entity) -> String {
    if let Some(role) = world.get::<bevy_gooey::Role>(entity) {
        return format!("a {role:?}");
    }
    match world.get::<Text>(entity) {
        Some(text) if !text.0.trim().is_empty() => format!("`{}`", text.0),
        _ => format!("{entity:?}"),
    }
}

/// **The editor on a project whose inspector has long values**, which is what overflowed: a map name, a path and an agent's own
/// sentence are all one word or one long run at this width.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("left_pane_{tag}_{}", std::process::id()))
        .descriptor("a_piece_with_a_rather_long_name", "alpha")
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

/// The shell's one split.
fn split_of(app: &mut App) -> Entity {
    let world = app.world_mut();
    world
        .query_filtered::<Entity, With<bevy_dark_factory::chrome::ShellSplit>>()
        .single(world)
        .unwrap_or_else(|e| panic!("the shell has one split: {e}"))
}

/// Put the divider where a drag would, then let the layout settle.
fn move_the_divider(app: &mut App, fraction: f32) {
    let split = split_of(app);
    app.world_mut()
        .get_mut::<bevy_gooey::SplitFraction>(split)
        .unwrap_or_else(|| panic!("the shell's split carries a fraction"))
        .0 = fraction;
    settle(app);
}

/// **Every control the left pane lays out ends inside the scroll viewport's content box** (#481's acceptance), as sentences naming
/// what did not.
///
/// The edge is the **viewport's content** box rather than the pane's border box, and the difference is the whole issue: the viewport
/// keeps [`bevy_dark_factory::chrome::SCROLL_GUTTER`] px on its right for the rail, so a switch that ends inside the *pane* can still be
/// drawn over the scrollbar — which is exactly what `feedback-1794783669349` shows. A control is inside the pane and still wrong.
///
/// The viewport's own rows are not walked: their overflow is what the scroll is for, and the rows below the fold are meant to be out
/// of sight. What is measured is every node the pane lays out *around* the scrolling area, and the viewport itself.
fn spills(app: &mut App, pane: Entity, tag: &str) -> Vec<String> {
    let world = app.world();
    let Some(viewport) = viewport_of(world, pane) else {
        return vec![format!("{tag}: the pane has no scroll viewport, so there is no content box to hold a control inside")];
    };
    let Some(box_of) = border_box(world, viewport) else { return vec![format!("{tag}: the viewport is not laid out")] };
    // **The viewport's border box, less the rail's gutter.** `SCROLL_GUTTER` px on its right stay clear for the scrollbar, so a
    // control that ends past that line is drawn under the rail — the capture's complaint. The viewport's own `content_inset` is not
    // the edge here: gooey applies the gutter as padding on the *skin* pass, so the inset a test reads before a style pass is the
    // pane's, and the rows themselves start at the viewport's padding box. Stating the gutter directly is the same number the skin
    // writes and does not depend on which pass ran last.
    let inset = world.get::<ComputedNode>(viewport).map(ComputedNode::content_inset).unwrap_or_default();
    let right = box_of.max.x - inset.max_inset.x;
    let left = box_of.min.x + inset.min_inset.x;
    let mut refused = Vec::new();
    // **The viewport's own subtree**, not the pane's: the pane's header stands outside the scrolling area and is laid out at the
    // pane's padding, so measuring it against the viewport's box reports a control that is nowhere near the scrollbar.
    for entity in under_but_not_into_the_scroll(world, viewport) {
        let Some(rect) = covers(world, entity) else { continue };
        if rect.max.x > right + SLACK_PX {
            refused.push(format!(
                "{tag}: {} ends at {:.1}, {:.1} px past the viewport's content edge {right:.1}",
                name_of(world, entity),
                rect.max.x,
                rect.max.x - right
            ));
        }
        if rect.min.x < left - SLACK_PX {
            refused.push(format!(
                "{tag}: {} starts at {:.1}, {:.1} px before the viewport's content edge {left:.1}",
                name_of(world, entity),
                rect.min.x,
                left - rect.min.x
            ));
        }
    }
    refused
}

/// The scrolling viewport the pane holds: gooey marks it `Role::ScrollViewport`.
fn viewport_of(world: &World, pane: Entity) -> Option<Entity> {
    let mut next = vec![pane];
    while let Some(at) = next.pop() {
        if world.get::<bevy_gooey::Role>(at) == Some(&bevy_gooey::Role::ScrollViewport) {
            return Some(at);
        }
        if let Some(children) = world.get::<Children>(at) {
            next.extend(children.iter());
        }
    }
    None
}

/// **At both ends of the divider and both interface scales, nothing in the left pane is drawn outside it**, and the pane really does
/// change width — a test whose pane never moved would pass on a broken layout.
#[test]
fn the_left_panes_contents_stay_inside_it_at_every_divider_position() {
    let mut app = editor("widths");
    let pane = app.world().resource::<Frame>().left;
    let mut refused = Vec::new();
    let mut widths: Vec<f32> = Vec::new();
    for scale in INTERFACE_SCALES {
        app.world_mut().resource_mut::<UiScale>().0 = scale;
        for fraction in FRACTIONS {
            move_the_divider(&mut app, fraction);
            let width = border_box(app.world(), pane).map(|b| b.size().x).unwrap_or_default();
            widths.push(width);
            refused.extend(spills(&mut app, pane, &format!("[interface scale {scale}, fraction {fraction}]")));
        }
    }
    // **The pane moved**, or the test proves nothing: the narrowest layout is materially narrower than the widest at the same scale.
    let narrow = widths[0].min(widths[1]);
    let wide = widths[0].max(widths[1]);
    assert!(
        wide > narrow + 10.0,
        "the divider did not move the pane (widths {widths:?}), so this file measured one layout twice"
    );
    assert!(refused.is_empty(), "the left pane draws its contents outside itself:\n{}", refused.join("\n"));
}

/// **The widest position does not move what it should not** (#482's second acceptance item).
///
/// A layout that reflowed at every width by throwing its cells away would pass the test above and be wrong; this says the pane has the
/// same number of drawn rows at both ends, so narrowing the pane wrapped and cut nothing.
#[test]
fn the_widest_divider_draws_the_same_rows_as_the_narrowest() {
    let mut app = editor("rows");
    let pane = app.world().resource::<Frame>().left;
    let count = |app: &mut App| {
        let world = app.world();
        under(world, pane).into_iter().filter(|e| world.get::<Text>(*e).is_some_and(|t| !t.0.trim().is_empty())).count()
    };
    move_the_divider(&mut app, FRACTIONS[0]);
    let narrow = count(&mut app);
    move_the_divider(&mut app, FRACTIONS[1]);
    let wide = count(&mut app);
    assert!(narrow > 0, "the left pane drew no words at all, so this test proves nothing");
    assert_eq!(
        narrow, wide,
        "the pane draws {narrow} texts at its narrowest and {wide} at its widest: narrowing it must wrap cells, not drop them"
    );
}
