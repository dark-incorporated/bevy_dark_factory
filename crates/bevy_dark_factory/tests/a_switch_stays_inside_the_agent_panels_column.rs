//! **A switch in the AGENT panel stays inside its column** (#481), at the width the owner's capture was taken at and
//! with the divider dragged as far left as it is allowed to go.
//!
//! `feedback-1794783669349` — Cauldron 0.6.8, mac_air, 2560x1406 at `ui_scale` 2.4 — with the note *"The widgets are
//! breaking out of the div here."* and a pick on a `Toggle`/`ToggleTrack` at `[578, 809, 693, 866]`. Measured on the
//! frame: the scroll viewport's content ends at `x≈673`, and the toggle's track is drawn from `x=631` to `x=728` — its
//! knob and body sit **55 px past** the viewport's edge, in the scrollbar's gutter and over the panel's border.
//!
//! # Why the assertion is over every descendant
//!
//! The capture names one switch; the fault is the row's layout, and a row that overflows takes its neighbours in the
//! same column with it. So this file asks the property the issue's acceptance asks — *every* drawn control of the AGENT
//! panel ends inside the content box it is drawn in — rather than pinning one toggle's right edge to one number.
//!
//! **The panel's own width is whatever the divider leaves.** The divider is dragged to its leftmost allowed position
//! (a `SplitFraction` of 0, which gooey holds at `SplitMin`) and the panel follows its pane
//! (`chrome::the_left_panel_follows_its_pane`), so the width measured is the width a person can actually reach by
//! holding the divider's arrow key down. That is the fault's worst case: nothing can be narrower.
//!
//! **Not clipping, laying out.** A panel that hides what does not fit has still laid the child out as though the panel
//! were wider, and the control after it in that column inherits the mistake. The Wanted asks for reflow, so this
//! measures what is *drawn*: a node's box, and for a text the union of its box and its glyphs, cut to whatever ancestor
//! clips it (`Overflow::scroll_y` on the viewport hides anything past its own box).

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};

use bevy_dark_factory::agent_view::AgentPane;
use bevy_dark_factory::chrome::{self, PanelWidth, Side};
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::Mode;

use crate::fixtures;

/// **The capture's window**, physical px (`feedback-1794783669349` was 2560 x 1406).
const WINDOW: (u32, u32) = (2560, 1406);

/// The interface scales the acceptance names: the editor's own at a display factor of 1, the capture's 2.4, and 1.6
/// between them.
const INTERFACE_SCALES: [f32; 3] = [1.0, 1.6, 2.4];

/// **The capture's own interface scale** (`feedback-1794783669349`: 2560x1406 at `ui_scale` 2.4).
const CAPTURE_SCALE: f32 = 2.4;

/// Frames a layout needs to settle after the divider or the scale moves.
const SETTLE_FRAMES: usize = 14;

/// How far a measured edge may stand past the one it is held to, physical px: layout rounds to whole pixels.
const SLACK_PX: f32 = 1.0;

/// How many spills one layout reports before the message becomes a wall.
const REPORT_LIMIT: usize = 8;

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

/// **The rect a node draws in**: its box, widened to what its glyphs fill when it carries words — and cut to what its
/// ancestors clip, because a clipped glyph paints nothing past the clip.
fn covers(world: &World, entity: Entity) -> Option<Rect> {
    let rect = border_box(world, entity)?;
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    let drawn = match (world.get::<Text>(entity), world.get::<TextLayoutInfo>(entity), world.get::<ComputedNode>(entity)) {
        (Some(text), Some(info), Some(computed)) if !text.0.trim().is_empty() => match glyphs(rect, computed, info) {
            Some(painted) => rect.union(painted),
            None => rect,
        },
        _ => rect,
    };
    Some(clipped(world, entity, drawn))
}

/// **What a text actually paints**: the union of every glyph's own box, in physical px.
///
/// A pixel is drawn where a glyph is, so a run that fills its box is inside it however the laid-out block measured, and
/// a run that overhangs its box is out however the block measured. `None` for a node with no glyphs laid out yet.
fn glyphs(box_of: Rect, computed: &ComputedNode, info: &TextLayoutInfo) -> Option<Rect> {
    info.glyphs.first()?;
    let inset = computed.content_inset();
    let start = box_of.min + Vec2::new(inset.min_inset.x, inset.min_inset.y);
    let mut painted: Option<Rect> = None;
    for glyph in &info.glyphs {
        let at = start + glyph.position;
        let cell = Rect::from_corners(at, at + glyph.atlas_info.rect.size());
        painted = Some(match painted {
            Some(so_far) => so_far.union(cell),
            None => cell,
        });
    }
    painted
}

/// What an ancestor with `overflow: clip` or a scroll hides: the drawn rect, cut to the nearest one that clips it.
fn clipped(world: &World, entity: Entity, drawn: Rect) -> Rect {
    let mut out = drawn;
    let mut at = world.get::<ChildOf>(entity).map(ChildOf::parent);
    while let Some(node) = at {
        let clips = world
            .get::<Node>(node)
            .is_some_and(|style| style.overflow.x == Overflow::clip_x().x || style.overflow.y == Overflow::clip_y().y);
        if clips && let Some(box_of) = border_box(world, node) {
            out = out.intersect(box_of);
        }
        at = world.get::<ChildOf>(node).map(ChildOf::parent);
    }
    out
}

/// Every entity under `root` — the root itself out, tooltips out (a tooltip is drawn outside its panel on purpose).
fn under(world: &World, root: Entity) -> Vec<Entity> {
    let kids_of = |at: Entity| -> Vec<Entity> {
        world
            .get::<Children>(at)
            .map(|kids| kids.iter().filter(|kid| !is_a_tooltip(world, *kid)).collect())
            .unwrap_or_default()
    };
    let mut found = kids_of(root);
    let mut next = 0;
    while next < found.len() {
        let more = kids_of(found[next]);
        found.extend(more);
        next += 1;
    }
    found
}

/// Is this node a tooltip, or inside one?
fn is_a_tooltip(world: &World, entity: Entity) -> bool {
    std::iter::successors(Some(entity), |at| world.get::<ChildOf>(*at).map(ChildOf::parent))
        .any(|at| matches!(world.get::<bevy_gooey::Role>(at), Some(bevy_gooey::Role::Tooltip)))
}

/// What a node is, in words a person can act on: its text, its name, or its role.
fn name_of(world: &World, entity: Entity) -> String {
    if let Some(text) = world.get::<Text>(entity)
        && !text.0.trim().is_empty()
    {
        return format!("`{}`", text.0.trim());
    }
    if let Some(name) = world.get::<Name>(entity) {
        return format!("{name}");
    }
    match world.get::<bevy_gooey::Role>(entity) {
        Some(role) => format!("a {role:?}"),
        None => format!("{entity:?}"),
    }
}

/// **The right edge `entity` is held to**: the content box of the nearest scroll viewport above it — the room the rail
/// lane leaves, which is what #481's acceptance names — or the panel's own content edge when it is in no viewport.
fn held_to(world: &World, entity: Entity, panel_edge: f32) -> f32 {
    let mut at = world.get::<ChildOf>(entity).map(ChildOf::parent).unwrap_or(entity);
    loop {
        let is_viewport = matches!(world.get::<bevy_gooey::Role>(at), Some(bevy_gooey::Role::ScrollViewport));
        if is_viewport {
            let Some(box_of) = border_box(world, at) else { return panel_edge };
            let inset = world.get::<ComputedNode>(at).map(ComputedNode::content_inset).unwrap_or_default();
            return box_of.max.x - inset.max_inset.x;
        }
        match world.get::<ChildOf>(at) {
            Some(parent) => at = parent.parent(),
            None => return panel_edge,
        }
    }
}

/// **Every descendant of `panel` ends at or inside the box it is drawn in**, as sentences naming what did not.
fn spills(app: &mut App, panel: Entity, tag: &str) -> Vec<String> {
    let world = app.world();
    let Some(panel_box) = border_box(world, panel) else { return vec![format!("{tag}: the panel is not laid out")] };
    let inset = world.get::<ComputedNode>(panel).map(ComputedNode::content_inset).unwrap_or_default();
    let content_right = panel_box.max.x - inset.max_inset.x;
    let content_left = panel_box.min.x + inset.min_inset.x;
    let mut refused = Vec::new();
    for entity in under(world, panel) {
        let Some(rect) = covers(world, entity) else { continue };
        let edge = held_to(world, entity, content_right);
        if rect.max.x > edge + SLACK_PX {
            refused.push(format!(
                "{tag}: `{}` ends at {:.1}, {:.1} px past {edge:.1}",
                name_of(world, entity),
                rect.max.x,
                rect.max.x - edge
            ));
        }
        if rect.min.x < content_left - SLACK_PX {
            refused.push(format!(
                "{tag}: `{}` starts at {:.1}, {:.1} px before {content_left:.1}",
                name_of(world, entity),
                rect.min.x,
                content_left - rect.min.x
            ));
        }
        if refused.len() >= REPORT_LIMIT {
            refused.push(format!("{tag}: …and more, the first {REPORT_LIMIT} are above"));
            break;
        }
    }
    refused
}

/// The AGENT panel's root: the node carrying a left [`PanelWidth`] that the pane sits under.
fn agent_panel(app: &mut App) -> Entity {
    let pane = app
        .world_mut()
        .query_filtered::<Entity, With<AgentPane>>()
        .single(app.world_mut())
        .expect("the Agent workspace draws its pane");
    let mut at = pane;
    loop {
        if app.world().get::<PanelWidth>(at).is_some_and(|panel| panel.side == Side::Left) {
            return at;
        }
        at = app
            .world()
            .get::<ChildOf>(at)
            .unwrap_or_else(|| panic!("the agent pane is inside a panel"))
            .parent();
    }
}

/// An editor opened on the Agent workspace with a piece to place.
///
/// **No window.** A `PrimaryWindow` makes `surface::fit_surface_to_window` compute `UiScale` from the window's
/// density and the person's interface size and write it every frame (`1.2 * growth * scale_factor`), which would
/// overwrite the scale a case states. Without one that system returns at its first line and the scale this file
/// writes is the scale the layout runs at — the same arrangement the sibling #494 test
/// (`a_panel_keeps_its_controls_inside_its_content_box`) uses to stage a panel at the capture's width.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("agent_switch_{tag}_{}", std::process::id()))
        .descriptor("crate", "alpha")
        .pack("props", &["crate"])
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Agent).unwrap_or_else(|e| panic!("the fixture opens: {e}"));
    harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

/// **The divider to its leftmost allowed position, and the pane's panel sized to the room that leaves.**
///
/// The leftmost a person can drag the shell's divider is gooey's `SplitMin` for the first pane, which the frame
/// spawns as [`chrome::DOCK_MIN_W`] plus the two margins the panel carries, and
/// `chrome::the_left_panel_follows_its_pane` sizes the panel to that pane less those margins — **never under
/// [`chrome::DOCK_MIN_W`]**. So the panel at the divider's floor is exactly `DOCK_MIN_W` wide, which is the panel
/// `feedback-1794783669349` measured at 378 physical px (its `ui_scale` 2.4), and it is the width a press-and-drag
/// on the divider can reach.
///
/// The fraction is driven to 0 as well, so a run that changes the frame's split arithmetic is measured on the
/// position the acceptance names rather than on a number this test chose on its own.
fn drag_the_divider_left(app: &mut App, panel: Entity) {
    let split = app
        .world_mut()
        .query_filtered::<Entity, With<bevy_gooey::Split>>()
        .iter(app.world())
        .next()
        .expect("the shell has one split");
    if let Some(mut fraction) = app.world_mut().get_mut::<bevy_gooey::SplitFraction>(split) {
        fraction.0 = 0.0;
    }
    let mut sizing = app
        .world_mut()
        .get_mut::<bevy_gooey::Sizing>(panel)
        .expect("a panel is a sized box");
    sizing.width = Some(bevy_gooey::Extent::Px(chrome::DOCK_MIN_W));
    settle(app);
}

/// The panel's written width in `Val::Px` units, or `NaN` when it is not an `Extent::Px`.
fn width_of(app: &App, panel: Entity) -> f32 {
    app.world()
        .get::<bevy_gooey::Sizing>(panel)
        .and_then(|sizing| sizing.width)
        .map(|width| match width {
            bevy_gooey::Extent::Px(px) => px,
            _ => f32::NAN,
        })
        .unwrap_or(f32::NAN)
}

/// Every entity the AGENT panel draws that is a switch, named for a report.
fn switches_in(app: &mut App, panel: Entity) -> Vec<Entity> {
    let candidates: Vec<Entity> = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<bevy_gooey::Role>>()
            .iter(world)
            .collect()
    };
    let in_panel = {
        let world = app.world();
        under(world, panel)
    };
    candidates
        .into_iter()
        .filter(|entity| {
            in_panel.contains(entity)
                && matches!(app.world().get::<bevy_gooey::Role>(*entity), Some(bevy_gooey::Role::Toggle))
        })
        .collect()
}

/// **At the capture's window, at three interface scales, with the divider at its leftmost, nothing in the AGENT panel
/// is drawn outside the box it is drawn in.**
#[test]
fn no_control_in_the_agent_panel_is_drawn_past_its_column() {
    let mut app = editor("leftmost");
    let panel = agent_panel(&mut app);
    let mut refused = Vec::new();
    for scale in INTERFACE_SCALES {
        app.world_mut().resource_mut::<UiScale>().0 = scale;
        drag_the_divider_left(&mut app, panel);
        let width = width_of(&app, panel);
        let drawn = border_box(app.world(), panel).map_or(0.0, |box_of| box_of.width());
        assert!(
            drawn > 0.0,
            "at interface scale {scale} the AGENT panel has no width, so this layout measures nothing"
        );
        // **The panel is at the divider's floor, not at its preferred width.** The capture's fault is the panel being
        // as narrow as a person can drag it; a panel still at `CONTROLS_W` would pass this file on a layout nobody
        // complained about.
        assert!(
            (width - chrome::DOCK_MIN_W).abs() <= 1.0,
            "at interface scale {scale} the AGENT panel is {width:.1} px, not the {:.1} px the divider's leftmost \
             position gives it — this layout is not the one #481 was filed about",
            chrome::DOCK_MIN_W
        );
        assert!(
            !switches_in(&mut app, panel).is_empty(),
            "at interface scale {scale} the AGENT panel draws no switch, so this is not the panel the capture names"
        );
        refused.extend(spills(
            &mut app,
            panel,
            &format!("AGENT [divider leftmost, width {width:.0} px, drawn {drawn:.0} px, interface scale {scale}]"),
        ));
    }
    assert!(
        refused.is_empty(),
        "the AGENT panel draws a control outside the box it is drawn in (#481):\n{}",
        refused.join("\n")
    );
}

/// **The switch the capture picked is inside the panel's column**, named rather than swept up in the property above.
///
/// `feedback-1794783669349` picks a `Toggle`/`ToggleTrack` at `[578, 809, 693, 866]` and measures its track drawn from
/// `x=631` to `x=728` against a viewport content edge at `x≈673` — **55 px past**, knob and body in the scrollbar's
/// gutter. So this asserts on the **track**, the thing whose right end the frame measured: a toggle's row can be
/// inside while the track it pushes past the edge is not, which is exactly the shape a row with no floor has.
#[test]
fn the_agent_panels_switch_is_inside_its_column() {
    let mut app = editor("switch");
    let panel = agent_panel(&mut app);
    app.world_mut().resource_mut::<UiScale>().0 = CAPTURE_SCALE;
    drag_the_divider_left(&mut app, panel);

    let switches = switches_in(&mut app, panel);
    assert!(!switches.is_empty(), "the AGENT panel draws a switch");
    let tracks: Vec<Entity> = {
        let world = app.world();
        switches
            .iter()
            .flat_map(|switch| under(world, *switch))
            .filter(|entity| matches!(world.get::<bevy_gooey::Role>(*entity), Some(bevy_gooey::Role::ToggleTrack)))
            .collect()
    };
    assert_eq!(tracks.len(), switches.len(), "every switch in the AGENT panel draws one track");

    let mut past = Vec::new();
    for track in tracks {
        let world = app.world();
        let drawn = covers(world, track).expect("the track is laid out");
        let panel_box = border_box(world, panel).expect("the panel is laid out");
        let inset = world.get::<ComputedNode>(panel).map(ComputedNode::content_inset).unwrap_or_default();
        let edge = held_to(world, track, panel_box.max.x - inset.max_inset.x);
        if drawn.max.x > edge + SLACK_PX {
            past.push(format!("a switch's track ends at {:.1}, {:.1} px past {edge:.1}", drawn.max.x, drawn.max.x - edge));
        }
    }
    assert!(
        past.is_empty(),
        "the AGENT panel draws a switch past the column it is drawn in (#481):\n{}",
        past.join("\n")
    );
}
