//! **No panel's controls are laid out past its own content box** (#494), and the same assertion holds the three
//! earlier captures of the family (#468's Pieces panel, #481's Agent panel, #482's left pane) in one test, so the
//! four cannot regress separately.
//!
//! | Capture | Panel | What the person saw |
//! |---|---|---|
//! | `feedback-1794921716691` | MESHES / PLACE (right) | *"Button is spilling outside."* |
//! | `feedback-1793928048623` | Pieces (MESHES, right) | *"text running off of it, and looks squished and jumbled"* |
//! | `feedback-1794783669349` | Agent (left) | *"The widgets are breaking out of the div here."* |
//! | `feedback-1794783743883` | PLACE (left pane, narrow divider) | *"Stuff is breaking out of this layout, depending on how far left the divider bar is moved."* |
//!
//! # What #494's own frame says, measured
//!
//! The issue's table puts the spilling control's right border at `2496..2499` and its panel's content edge at
//! `2528`, and calls that "29 px past". It is 29 px **inside**. The frame's real overflow is the shelf strip's
//! chip: at `y = 530..580` its fill runs from `x = 2184` to **`x = 2547`**, which is **17 px past the panel's own
//! right border at 2530**, out into the 30 px gutter between the panel and the window's edge. The control the
//! table chose is a *button inside* the panel that is fine; reading the whole band rather than the rows the table
//! chose is what finds the one that is not. So this file asserts the property over every descendant, node boxes
//! included, rather than pinning one control's number.
//!
//! # What a node's box is, here
//!
//! The rect from `ComputedNode` through `UiGlobalTransform`; a **text's** rect is widened to `TextLayoutInfo::size`
//! — what its glyphs fill — because a text's box can be narrower than the words drawn in it, which is the
//! Pieces-panel defect. Both matter and both are measured: a chip whose *box* runs past the border draws its fill
//! there even when its words are inside.
//!
//! **No escape hatch for clipping.** A panel that hides an overflowing child has still laid it out as if the panel
//! were wider, and the next control in that column inherits the mistake. #494's Wanted asks for both, so the
//! assertion is the stronger one: the layout is inside the box.
//!
//! **A scroll viewport's own children are walked here.** #481's acceptance holds a switch to *the scroll viewport's
//! content box*, which is what the Agent panel's overflow was: a row of that viewport. The viewport's rail band is
//! left out of the room it gives (`chrome::SCROLL_GUTTER`), because a control drawn under the rail is drawn over
//! the scrollbar — the capture's own complaint.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};

use bevy_dark_factory::agent_view::AgentPane;
use bevy_dark_factory::chrome::{self, PanelWidth, Side};
use bevy_dark_factory::editor::{MapControlsPane, PaletteList};
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::{CandidateList, Mode};

use crate::fixtures;

/// The capture's window, physical px (`feedback-1794921716691`: 2560 x 1604).
const WINDOW: (u32, u32) = (2560, 1604);

/// **The interface scales #494's acceptance names**: 1.0, and 1.6 — the capture was taken at 2.4 over the editor's 1.2
/// baseline, so 1.6 sits between the editor's own identity and the frame's. A panel has to fit at both.
const INTERFACE_SCALES: [f32; 2] = [1.0, 1.6];

/// Frames a layout needs to settle after the width or the scale moves.
const SETTLE_FRAMES: usize = 12;

/// How far a measured edge may stand past the one it is held to, physical px: layout rounds.
const SLACK_PX: f32 = 1.0;

/// How many spills one panel's layout reports before the message stops being a list and becomes a wall: the first few
/// identify the control and the rest are the same defect over again.
const REPORT_LIMIT: usize = 10;

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

/// **The rect a node draws in**: its box, widened to what its glyphs fill when it carries words.
fn covers(world: &World, entity: Entity) -> Option<Rect> {
    let rect = border_box(world, entity)?;
    match (world.get::<Text>(entity), world.get::<TextLayoutInfo>(entity), world.get::<ComputedNode>(entity)) {
        (Some(text), Some(info), Some(computed)) if !text.0.trim().is_empty() && info.size.x > 0.0 && info.size.y > 0.0 => {
            let inset = computed.content_inset();
            let start = rect.min + Vec2::new(inset.min_inset.x, inset.min_inset.y);
            Some(rect.union(Rect::from_corners(start, start + info.size)))
        }
        _ => Some(rect),
    }
}

/// Every entity under `root` — **`root` itself out** — into a scroll viewport's children, and never into a tooltip.
///
/// The root is the panel, and it is the box everything else is held to: measuring it against its own content inset
/// reports its padding as overflow, 13 px of noise that says nothing about a control. A tooltip is a `Popover`
/// drawn one gap *outside* the panel it belongs to, deliberately, for the same reason.
fn under(world: &World, root: Entity) -> Vec<Entity> {
    let mut found: Vec<Entity> = Vec::new();
    if let Some(children) = world.get::<Children>(root) {
        found.extend(children.iter().filter(|kid| !is_a_tooltip(world, *kid)));
    }
    let mut next = 0;
    while next < found.len() {
        let at = found[next];
        if !is_a_tooltip(world, at)
            && let Some(children) = world.get::<Children>(at)
        {
            found.extend(children.iter().filter(|kid| !is_a_tooltip(world, *kid)));
        }
        next += 1;
    }
    found
}

/// **Is this node a tooltip**, or inside one? A `Popover` is drawn outside the panel on purpose.
fn is_a_tooltip(world: &World, entity: Entity) -> bool {
    std::iter::successors(Some(entity), |at| world.get::<ChildOf>(*at).map(ChildOf::parent))
        .any(|at| matches!(world.get::<bevy_gooey::Role>(at), Some(bevy_gooey::Role::Tooltip)))
}

/// What a node is, for a message a person can act on: its words, its name, or its role.
fn name_of(world: &World, entity: Entity) -> String {
    if let Some(text) = world.get::<Text>(entity)
        && !text.0.trim().is_empty()
    {
        return format!("`{}`", text.0);
    }
    if let Some(name) = world.get::<Name>(entity) {
        return name.as_str().to_owned();
    }
    match world.get::<bevy_gooey::Role>(entity) {
        Some(role) => format!("a {role:?}"),
        None => format!("{entity:?}"),
    }
}

/// Lay a panel out at `width` skin px and `scale` interface scale, the way `fit_docks_to_the_window` writes it.
fn lay_out(app: &mut App, panel: Entity, width: f32, scale: f32) {
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    app.world_mut()
        .get_mut::<bevy_gooey::Sizing>(panel)
        .unwrap_or_else(|| panic!("a panel is a sized box"))
        .width = Some(bevy_gooey::Extent::Px(width));
    settle(app);
}

/// **Every descendant of `panel` ends at or inside the panel's content box**, as sentences naming what did not.
///
/// A row inside a scroll viewport is held to the viewport's content box less the rail's band
/// ([`chrome::SCROLL_GUTTER`]); everything else is held to the panel's own content box.
fn spills(app: &mut App, panel: Entity, tag: &str) -> Vec<String> {
    let world = app.world();
    let Some(panel_box) = border_box(world, panel) else { return vec![format!("{tag}: the panel is not laid out")] };
    let inset = world.get::<ComputedNode>(panel).map(ComputedNode::content_inset).unwrap_or_default();
    let contents_right = panel_box.max.x - inset.max_inset.x;
    let contents_left = panel_box.min.x + inset.min_inset.x;
    let mut refused = Vec::new();
    for entity in under(world, panel) {
        let Some(rect) = covers(world, entity) else { continue };
        // **A row of a scroll viewport is held where it is drawn**, which is the viewport's content box less the
        // rail: a control under the rail is the very thing these four captures point at.
        let held_to = scroll_room_right(world, entity, contents_right);
        if rect.max.x > held_to + SLACK_PX {
            refused.push(format!(
                "{tag}: `{}` ends at {:.1}, {:.1} px past the content edge {held_to:.1}",
                name_of(world, entity),
                rect.max.x,
                rect.max.x - held_to
            ));
        }
        if rect.min.x < contents_left - SLACK_PX {
            refused.push(format!(
                "{tag}: `{}` starts at {:.1}, {:.1} px before the content edge {contents_left:.1}",
                name_of(world, entity),
                rect.min.x,
                contents_left - rect.min.x
            ));
        }
        if refused.len() > REPORT_LIMIT {
            refused.push(format!("{tag}: …and more; the first {REPORT_LIMIT} are above"));
            return refused;
        }
    }
    refused
}

/// **The right edge `entity` is held to**: its nearest scroll viewport's content box less the rail's band, or the
/// panel's own content edge when it is in no viewport.
fn scroll_room_right(world: &World, entity: Entity, panel_edge: f32) -> f32 {
    // **The viewport itself is held to the panel**, not to its own rail lane: its inset is the lane it
    // keeps for the rail, so measuring it against that says the viewport is 6 px past itself.
    let mut at = world.get::<ChildOf>(entity).map(ChildOf::parent).unwrap_or(entity);
    loop {
        if matches!(world.get::<bevy_gooey::Role>(at), Some(bevy_gooey::Role::ScrollViewport)) {
            let box_of = border_box(world, at).unwrap_or_default();
            let inset = world.get::<ComputedNode>(at).map(ComputedNode::content_inset).unwrap_or_default();
            return box_of.max.x - inset.max_inset.x;
        }
        match world.get::<ChildOf>(at) {
            Some(parent) => at = parent.parent(),
            None => return panel_edge,
        }
    }
}

/// The panel a marker sits in, walking up to the node that carries a [`PanelWidth`] **on the side the marker's pane is on**.
///
/// The side is required: a left-hand marker walking up to whatever `PanelWidth` it met first would measure the dock
/// rather than the pane, and the dock is as wide as the widest panel in it.
fn panel_of(app: &mut App, inside: Entity, side: Side, tag: &str) -> Entity {
    let world = app.world_mut();
    let mut at = inside;
    loop {
        if world.get::<PanelWidth>(at).is_some_and(|panel| panel.side == side) {
            return at;
        }
        at = world
            .get::<ChildOf>(at)
            .unwrap_or_else(|| panic!("{tag}'s marker is inside a panel"))
            .parent();
    }
}

/// A fixture project with meshes to import and pieces to place, so every panel has rows.
fn editor(tag: &str, mode: Mode) -> App {
    let root = fixtures::Fixture::new(&format!("panel_fit_{tag}_{}", std::process::id()))
        .descriptor("crate", "alpha")
        .descriptor("barrel", "alpha")
        .descriptor("wall_low", "alpha")
        .pack("props", &["crate", "barrel", "wall_low", "a_long_mesh_name_that_has_to_wrap_inside_its_row"])
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

/// The panels the four captures name that this build draws, each found by the marker its own module publishes.
fn panels_here(app: &mut App) -> Vec<(&'static str, Entity, f32)> {
    let mut out = Vec::new();
    // **PLACE** (Map door, right): the palette whose shelf strip the fourth capture's chip is in.
    let place = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PaletteList>>().single(world).ok()
    };
    if let Some(list) = place {
        out.push(("PLACE", panel_of(app, list, Side::Right, "the palette"), chrome::LIST_W));
    }
    // **MESHES** (Kit door, right): the candidate list and the shelf strip above it — #494's own capture.
    let meshes = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<CandidateList>>().single(world).ok()
    };
    if let Some(list) = meshes {
        out.push(("MESHES", panel_of(app, list, Side::Right, "the candidate list"), chrome::LIST_W));
    }
    // **AGENT** (Agent door, left): the scrolling pane whose toggle the third capture points at.
    let agent = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<AgentPane>>().single(world).ok()
    };
    if let Some(pane) = agent {
        out.push(("AGENT", panel_of(app, pane, Side::Left, "the agent pane"), chrome::CONTROLS_W));
    }
    // **INSPECTOR** (Map door, left): the pane at a narrow divider (#482).
    let inspector = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<MapControlsPane>>().single(world).ok()
    };
    if let Some(pane) = inspector {
        out.push(("INSPECTOR", panel_of(app, pane, Side::Left, "the inspector pane"), chrome::CONTROLS_W));
    }
    out
}

/// **At the capture's window, #494's two interface scales and two panel widths, every control of all four panels is
/// inside the panel it is drawn in** — and a scroll viewport's rows are inside the room the rail leaves them.
#[test]
fn no_panel_draws_a_control_past_its_own_content_box() {
    let mut refused = Vec::new();
    let mut measured = 0;
    let mut seen: Vec<&str> = Vec::new();
    for (door, mode) in [("map", Mode::Map), ("meshes", Mode::Meshes), ("agent", Mode::Agent)] {
        let mut app = editor(door, mode);
        let panels = panels_here(&mut app);
        assert!(!panels.is_empty(), "the {door} door drew none of the panels this test measures");
        for (tag, panel, preferred) in panels {
            // **A panel that is not showing is not measured.** The editor spawns every door's panels and hides the
            // doors that are not showing, so a `PaletteList` still exists on the MESHES door with children laid out at
            // whatever width the hidden door last had; measuring it reports the other door's layout as this one's.
            let showing = {
                let world = app.world();
                world.get::<Node>(panel).is_some_and(|node| node.display != Display::None)
                    && world.get::<ComputedNode>(panel).is_some_and(|computed| computed.size().x > 0.0)
            };
            if !showing {
                continue;
            }
            seen.push(tag);
            for scale in INTERFACE_SCALES {
                for (width_name, width) in [("preferred", preferred), ("drag minimum", chrome::DOCK_MIN_W)] {
                    lay_out(&mut app, panel, width, scale);
                    refused.extend(spills(&mut app, panel, &format!("{tag} [{width_name} {width:.0} px, interface scale {scale}]")));
                    measured += 1;
                }
            }
        }
    }
    for tag in ["PLACE", "MESHES", "AGENT", "INSPECTOR"] {
        assert!(seen.contains(&tag), "{tag} was not measured, so this file is not the four captures' check; it saw {seen:?}");
    }
    assert!(measured >= 16, "four panels at two scales and two widths is sixteen layouts, and this measured {measured}");
    assert!(refused.is_empty(), "a panel draws a control outside its own content box:\n{}", refused.join("\n"));
}
