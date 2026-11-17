//! **No panel's controls are laid out past its own content box** (#494, plus the one assertion four captures share: #468, #481, #482).
//!
//! Four captures, four panels, one defect: a control laid out as if its panel were wider than it is.
//!
//! | Capture | Panel | What the person saw |
//! |---|---|---|
//! | `feedback-1793928048623` | Pieces (MESHES, right) | *"text running off of it, and looks squished and jumbled"* |
//! | `feedback-1794921716691` | MESHES (right) | *"Button is spilling outside."* |
//! | `feedback-1794783669349` | Agent (left) | *"The widgets are breaking out of the div here."* |
//! | `feedback-1794784926…` | PLACE (right) | *"what makes these ready to use? And this layout feels a bit jumbled"* |
//!
//! # What #494's own frame says, measured
//!
//! The issue's table puts the spilling control's right border at `2496..2499` and its panel's content edge at `2528`, and calls
//! that "29 px past". It is 29 px **inside**. The frame's real overflow is two rows lower: at `y = 536..568` a shelf chip's
//! fill runs to **x = 2547**, which is **17 px past the panel's own right border at 2530** and into the 30 px gutter to the
//! window's edge. Reading the whole band rather than the rows the table chose is what finds it, so this file asserts the
//! property over every descendant rather than pinning one control's number.
//!
//! # What a node's box is, here
//!
//! The rect from `ComputedNode` through `UiGlobalTransform`; a **text's** box can be narrower than the words drawn in it, which
//! is precisely the Pieces-panel defect, so a text's rect is widened to `TextLayoutInfo::size` — the size of what is drawn.
//! Bounds are physical px, and two boxes are compared directly; nothing here is aimed at, so nothing is divided by a scale.
//!
//! **No escape hatch for clipping.** A panel that hides an overflowing child has still laid it out as if the panel were wider,
//! and the next control in that column inherits the mistake. #494's Wanted asks for both, so the assertion is the stronger one:
//! the layout is inside the box.

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

/// The interface scales a panel is held at: the editor's own at a scale factor of 1, and roughly the capture's 2.4 over a 2.0
/// display less the editor's 1.2 baseline.
const INTERFACE_SCALES: [f32; 2] = [1.0, 2.0];

/// Frames a layout needs to settle after the width or the scale moves.
const SETTLE_FRAMES: usize = 12;

/// How far a measured edge may stand past the one it is held to, physical px: layout rounds.
const SLACK_PX: f32 = 1.0;

/// How many spills one panel's layout reports before the message stops being a list and becomes a wall: the first few identify
/// the control and the rest are the same defect over again.
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

/// Every entity under `root`, `root` included — **but not past a scroll viewport**.
///
/// A scroll viewport's children are laid out taller than it on purpose: their overflow *is* the scroll. Walking into one would hold
/// the fifth row of a log to a rule the first four satisfy, and report a list that scrolls as a layout defect. The viewport itself is
/// measured (it must be inside the panel); what it holds is not.
fn under(world: &World, root: Entity) -> Vec<Entity> {
    // **Not past a scroll viewport.** A viewport's children are laid out taller than it on purpose — their overflow *is* the scroll —
    // and #481's acceptance holds a switch to *"the scroll viewport's content box"*, which is the viewport, not the rows inside it.
    // Walking in would hold the fifth row of a log to a rule the first four satisfy and report a list that scrolls as a defect. The
    // viewport itself is measured; what it scrolls is not.
    let mut found = vec![root];
    let mut next = 0;
    while next < found.len() {
        let at = found[next];
        // **A scroll viewport's rows and a tooltip's box are not panel layout.** A viewport's children are laid out taller than it on
        // purpose; a tooltip is a `Popover` drawn *outside* the panel it belongs to, which is the whole point of a tooltip — gooey
        // places it "one gap outside the panel the widget is in" so it never covers the next control. Holding either to the panel
        // would report the two things that are meant to escape.
        let escapes = matches!(
            world.get::<bevy_gooey::Role>(at),
            Some(bevy_gooey::Role::ScrollViewport | bevy_gooey::Role::Tooltip)
        );
        if !escapes
            && let Some(children) = world.get::<Children>(at)
        {
            found.extend(children.iter());
        }
        next += 1;
    }
    found
}

/// **The rect a text draws in**: what its glyphs fill, from where its content box starts.
///
/// Only texts are measured. A container's own box is laid out against the *padding* box it was given, and an empty text node is a
/// spacer with no glyphs in it, so holding either to the content edge reports the scroll gutter and the insets as overflow — 13 px
/// of noise per row that says nothing about whether a person can read the panel. A text is the thing the four captures pointed at:
/// words drawn past the edge.
fn covers(world: &World, entity: Entity) -> Option<Rect> {
    let text = world.get::<Text>(entity)?;
    if text.0.trim().is_empty() {
        return None;
    }
    let info = world.get::<TextLayoutInfo>(entity)?;
    if info.size.x <= 0.0 || info.size.y <= 0.0 {
        return None;
    }
    let rect = border_box(world, entity)?;
    let inset = world.get::<ComputedNode>(entity).map(ComputedNode::content_inset).unwrap_or_default();
    let start = rect.min + Vec2::new(inset.min_inset.x, inset.min_inset.y);
    Some(Rect::from_corners(start, start + info.size))
}

/// What a node is, for a message a person can act on: its words, its name, or where it is.
fn name_of(world: &World, entity: Entity) -> String {
    if let Some(text) = world.get::<Text>(entity)
        && !text.0.trim().is_empty()
    {
        return text.0.clone();
    }
    match world.get::<Name>(entity) {
        Some(name) => name.as_str().to_owned(),
        None => format!("{entity:?}"),
    }
}

/// Lay a panel out at `width` skin px and `scale` interface scale, the way `fit_docks_to_the_window` writes it in a window.
fn lay_out(app: &mut App, panel: Entity, width: f32, scale: f32) {
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    app.world_mut()
        .get_mut::<bevy_gooey::Sizing>(panel)
        .unwrap_or_else(|| panic!("a panel is a sized box"))
        .width = Some(bevy_gooey::Extent::Px(width));
    settle(app);
}

/// **Every descendant of `panel` ends at or inside the panel's content box**, as sentences naming what did not.
fn spills(app: &mut App, panel: Entity, tag: &str) -> Vec<String> {
    let world = app.world();
    let Some(panel_box) = border_box(world, panel) else { return vec![format!("{tag}: the panel is not laid out")] };
    let inset = world.get::<ComputedNode>(panel).map(ComputedNode::content_inset).unwrap_or_default();
    let contents_right = panel_box.max.x - inset.max_inset.x;
    let contents_left = panel_box.min.x + inset.min_inset.x;
    let mut refused = Vec::new();
    for entity in under(world, panel) {
        let Some(rect) = covers(world, entity) else { continue };
        if rect.max.x > contents_right + SLACK_PX {
            refused.push(format!(
                "{tag}: `{}` ends at {:.1}, {:.1} px past the content edge {contents_right:.1}",
                name_of(world, entity),
                rect.max.x,
                rect.max.x - contents_right
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

/// The panel a marker sits in, walking up to the node that carries a [`PanelWidth`] **on the side the marker's pane is on**.
///
/// The side is required, not incidental: a left-hand marker walking up to whatever `PanelWidth` it met first would measure the
/// dock rather than the pane, and the dock is as wide as the widest panel in it — a test that measured that would report the whole
/// dock's room as one panel's overflow.
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

/// The panels the four captures name that this door draws, each found by the marker its own module publishes.
fn panels_here(app: &mut App) -> Vec<(&'static str, Entity, f32)> {
    // **One query at a time**: `single` borrows the world it queries, so each query is built, used and dropped before the next.
    let mut out = Vec::new();
    // **PLACE** (Map door, right): the palette whose rows the fourth capture points at.
    let place = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<PaletteList>>().single(world).ok()
    };
    if let Some(list) = place {
        out.push(("PLACE", panel_of(app, list, Side::Right, "the palette"), chrome::LIST_W));
    }
    // **MESHES** (Kit door, right): the candidate list and the shelf strip above it — the second and third captures' panel.
    let meshes = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<CandidateList>>().single(world).ok()
    };
    if let Some(list) = meshes {
        out.push(("MESHES", panel_of(app, list, Side::Right, "the candidate list"), chrome::LIST_W));
    }
    // **FAMILIAR** (Agent door, left): the scrolling pane whose toggle the third capture points at.
    let night_shift = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<AgentPane>>().single(world).ok()
    };
    if let Some(pane) = night_shift {
        out.push(("FAMILIAR", panel_of(app, pane, Side::Left, "the agent pane"), chrome::CONTROLS_W));
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

/// **At the capture's window, two interface scales and two panel widths, every control of every panel is inside the panel.**
#[test]
fn no_panel_lays_a_control_outside_its_own_content_box() {
    let mut refused = Vec::new();
    let mut measured = 0;
    let mut seen: Vec<&str> = Vec::new();
    for (door, mode) in [("map", Mode::Map), ("meshes", Mode::Meshes), ("agent", Mode::Agent)] {
        let mut app = editor(door, mode);
        let panels = panels_here(&mut app);
        assert!(!panels.is_empty(), "the {door} door drew none of the panels this test measures");
        for (tag, panel, preferred) in panels {
            // **A panel that is not showing is not measured.** The editor spawns every door's panels and hides the doors that are
            // not showing (`Display::None`), so a `PaletteList` still exists on the MESHES door with children laid out at whatever
            // width the hidden door last had. Measuring it reports the *other* door's layout as this one's overflow.
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
    for tag in ["PLACE", "MESHES", "FAMILIAR", "INSPECTOR"] {
        assert!(seen.contains(&tag), "{tag} was not measured, so this file is not the four captures' check; it saw {seen:?}");
    }
    assert!(measured >= 16, "four panels at two scales and two widths is sixteen layouts, and this measured {measured}");
    assert!(refused.is_empty(), "a panel lays a control outside its own content box:\n{}", refused.join("\n"));
}
