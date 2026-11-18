//! **No panel draws a control past its own content box** (#494), and the three earlier captures of the same defect are
//! held by the same assertion, so the four cannot regress separately.
//!
//! | Capture | Panel | What the person said |
//! |---|---|---|
//! | `feedback-1794921716691` | PLACE, right dock | *"Button is spilling outside."* |
//! | `feedback-1793928048623` | Pieces, MESHES | *"text running off of it, and looks squished and jumbled"* |
//! | `feedback-1794783669349` | AGENT, left dock | *"The widgets are breaking out of the div here."* |
//! | `feedback-1794783743883` | INSPECTOR, left dock at a narrow divider | *"Stuff is breaking out of this layout, depending on how far left the divider bar is moved."* |
//!
//! # What the #494 frame actually says
//!
//! Its table puts a chip's right border at `2496..2499` against a content edge at `2528` and calls that "29 px past";
//! it is 29 px **inside**. Reading the whole band instead of the rows the table picked finds the one control that is
//! not: at `y = 530..580` the shelf strip's chip paints out to `x = 2547`, **17 px past the panel's own right border**
//! at 2530, into the gutter between the panel and the window's edge. So this file asserts the property over every
//! descendant rather than pinning one control's number.
//!
//! **The capture's panel is 378 physical px wide at its `ui_scale` 2.4** (`2151..2529` on the frame), which is
//! `157.5` in the `Val::Px` units a panel's width is written in — its drag minimum, [`chrome::DOCK_MIN_W`]. That is
//! the width the acceptance calls "the capture's width".
//!
//! # What a descendant's right edge means here
//!
//! The rect from `ComputedNode` through `UiGlobalTransform`, and for a text the **union of its box and its glyphs**
//! (`TextLayoutInfo::size`): a text's box can be narrower than the words drawn in it, which is the Pieces panel's
//! whole defect. A node's box matters on its own — a chip whose box runs past the border paints its fill there even
//! when its words are inside.
//!
//! **Not clipping, laying out.** A panel that hides what does not fit has still laid the child out as though the panel
//! were wider, and the control after it in that column inherits the mistake; #494's Wanted asks for both, so the
//! assertion is the stronger one.
//!
//! **Two boxes are legitimate and are not measured**: a tooltip, which is a `Popover` drawn one gap outside the panel
//! it belongs to on purpose, and the panel root itself, whose own padding would read as 13 px of overflow.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};

use bevy_dark_factory::agent_view::AgentPane;
use bevy_dark_factory::chrome::{self, PanelWidth, Side};
use bevy_dark_factory::editor::{MapControlsPane, PaletteList};
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::{CandidateList, Mode};

use crate::fixtures;

/// **The capture's window**, physical px (`feedback-1794921716691` was 2560 x 1604).
const WINDOW: (u32, u32) = (2560, 1604);

/// **The capture's panel width in the units a panel is written in**: `378` physical px on the frame at its `ui_scale`
/// 2.4. It is [`chrome::DOCK_MIN_W`] to the pixel, which is why the frame is the one that shows the defect worst.
const CAPTURE_PANEL_W: f32 = 378.0 / 2.4;

/// **The interface scales the acceptance names**: the editor's own at a display factor of 1, and 1.6 between that and
/// the capture's 2.4 (its `ui_scale` over the editor's 1.2 baseline).
const INTERFACE_SCALES: [f32; 3] = [1.0, 1.6, 2.4];

/// **The text size the capture was taken at**, as a multiple of the skin's type scale: its chips are drawn about 1.44
/// times `text::CONTROL`, the same ratio its body text is drawn at. `ui_scale` 2.4 is the *interface* scale, which
/// multiplies every length; this multiplies the type, and the two together are what the frame shows.
const CAPTURE_TEXT_SCALE: f32 = 1.44;

/// Frames a layout needs to settle after the width or the scale moves.
const SETTLE_FRAMES: usize = 12;

/// How far a measured edge may stand past the one it is held to, physical px: layout rounds to whole pixels.
const SLACK_PX: f32 = 1.0;

/// How many spills one layout reports before the message stops naming controls and becomes a wall.
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

/// **The rect a node draws in**: its box, widened to what its glyphs fill when it carries words — and **cut to what its
/// ancestors clip**, because a clipped glyph paints nothing past the clip.
///
/// Both halves are real and neither is enough alone. A text's box can be narrower than the words in it, which is the
/// Pieces panel's whole defect (#468): `surveillance_console.glb` was drawn out past the border from a box that was
/// inside it. And a cell that is `Overflow::clip` — every gooey table cell is — draws its text only up to its own edge, so
/// measuring the glyph run there would report a spill that no pixel shows.
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
/// This is the drawn truth and `TextLayoutInfo::size` is not quite it. `size` is the laid-out block, which can stand a
/// character wider than the room it was given (measured: `The agent has made no changes yet` reports `134 px` of block in
/// a `126 px` box; its last glyph ends at 126). A pixel is drawn where a glyph is, so a run that fills its box is inside
/// it however the block measured — and a run that overhangs its box is out, however the block measured. `None` for a
/// node with no glyphs laid out yet.
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

/// Every entity under `root` — the root itself out, tooltips out.
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

/// **Is this node a tooltip**, or inside one? A tooltip is drawn outside its panel on purpose.
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

/// Lay a panel out at `width` skin px and `scale` interface scale, the way `fit_docks_to_the_window` writes it.
fn lay_out(app: &mut App, panel: Entity, width: f32, scale: f32) {
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    app.world_mut()
        .get_mut::<bevy_gooey::Sizing>(panel)
        .unwrap_or_else(|| panic!("a panel is a sized box"))
        .width = Some(bevy_gooey::Extent::Px(width));
    settle(app);
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
            refused.push(format!("{tag}: `{}` ends at {:.1}, {:.1} px past {edge:.1}", name_of(world, entity), rect.max.x, rect.max.x - edge));
        }
        if rect.min.x < content_left - SLACK_PX {
            refused.push(format!("{tag}: `{}` starts at {:.1}, {:.1} px before {content_left:.1}", name_of(world, entity), rect.min.x, content_left - rect.min.x));
        }
        if refused.len() >= REPORT_LIMIT {
            refused.push(format!("{tag}: …and more, the first {REPORT_LIMIT} are above"));
            break;
        }
    }
    refused
}

/// The panel a marker sits in, walking up to the node that carries a [`PanelWidth`] on `side`.
fn panel_of(app: &mut App, inside: Entity, side: Side, tag: &str) -> Entity {
    let world = app.world_mut();
    let mut at = inside;
    loop {
        if world.get::<PanelWidth>(at).is_some_and(|panel| panel.side == side) {
            return at;
        }
        at = world.get::<ChildOf>(at).unwrap_or_else(|| panic!("{tag}'s marker is inside a panel")).parent();
    }
}

/// A project with pieces to place and meshes to import, on the door whose panels the test measures.
///
/// `full` is the capture's own state or its opposite: `feedback-1794921716691` was taken on a project whose library was
/// **empty**, so its panel draws `NOT IMPORTED (0)` and `MESHES (0)` rather than a piece's name.
fn editor(tag: &str, mode: Mode, full: bool) -> App {
    let fixture = fixtures::Fixture::new(&format!("panel_box_{tag}_{}", std::process::id()));
    let fixture = if full {
        fixture
            .descriptor("crate", "alpha")
            .descriptor("barrel", "alpha")
            .descriptor("wall_low", "alpha")
            .pack("props", &["crate", "barrel", "wall_low", "a_mesh_name_long_enough_to_wrap_inside_its_row"])
    } else {
        fixture
    };
    let root = fixture.build("m");
    // **The person's text size, from their settings file** — the one lever the capture's panel width does not explain
    // by itself. `feedback-1794921716691`'s chips are drawn ~1.44x the skin's `text::CONTROL`, so the file says so,
    // which is how the setting reaches `chrome::text_scale` in a running editor (`ui::follow_the_text_size`).
    let settings_file = root.join(format!("user_config_panel_box_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(settings_file.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&settings_file, format!("{{version: 1, text_scale: {CAPTURE_TEXT_SCALE}}}")).expect("a settings file");
    let mut app = harness::build_headless_at(&root, "m", None, mode).unwrap_or_else(|e| panic!("the fixture opens: {e}"));
    app.insert_resource(bevy_dark_factory::settings::UserSettingsFile(settings_file));
    harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

/// The panels the four captures name that this build draws: its tag, its entity, and the width it prefers.
fn panels_here(app: &mut App) -> Vec<(&'static str, Entity, f32)> {
    let mut out = Vec::new();
    let one = |app: &mut App, marker: &dyn Fn(&mut App) -> Option<Entity>| marker(app);
    if let Some(list) = one(app, &|app| app.world_mut().query_filtered::<Entity, With<PaletteList>>().single(app.world_mut()).ok()) {
        out.push(("PLACE", panel_of(app, list, Side::Right, "the palette"), chrome::LIST_W));
    }
    if let Some(list) = one(app, &|app| app.world_mut().query_filtered::<Entity, With<CandidateList>>().single(app.world_mut()).ok()) {
        out.push(("PIECES", panel_of(app, list, Side::Right, "the candidate list"), chrome::LIST_W));
    }
    if let Some(pane) = one(app, &|app| app.world_mut().query_filtered::<Entity, With<AgentPane>>().single(app.world_mut()).ok()) {
        out.push(("AGENT", panel_of(app, pane, Side::Left, "the agent pane"), chrome::CONTROLS_W));
    }
    if let Some(pane) = one(app, &|app| app.world_mut().query_filtered::<Entity, With<MapControlsPane>>().single(app.world_mut()).ok()) {
        out.push(("INSPECTOR", panel_of(app, pane, Side::Left, "the inspector pane"), chrome::CONTROLS_W));
    }
    out
}

/// **At the capture's window, at interface scale 1.0 and 1.6, at the capture's own panel width and at the widest a
/// panel prefers, every descendant of all four panels is inside the box it is drawn in.**
#[test]
fn no_panel_draws_a_control_past_its_content_box() {
    let mut refused = Vec::new();
    let mut measured = 0;
    let mut seen: Vec<&str> = Vec::new();
    for (door, mode, full) in [("map", Mode::Map, true), ("meshes", Mode::Meshes, true), ("capture", Mode::Meshes, false), ("agent", Mode::Agent, true)] {
        let mut app = editor(door, mode, full);
        let panels = panels_here(&mut app);
        for (tag, panel, preferred) in panels {
            // **A panel this door does not show is not measured.** The editor spawns every door's panels and hides the
            // doors; a hidden panel still has children laid out at whatever width its door last had.
            let showing = {
                let world = app.world();
                world.get::<Node>(panel).is_some_and(|node| node.display != Display::None)
                    && world.get::<ComputedNode>(panel).is_some_and(|computed| computed.size().x > 0.0)
            };
            if !showing {
                continue;
            }
            if !seen.contains(&tag) {
                seen.push(tag);
            }
            for scale in INTERFACE_SCALES {
                for (width_name, width) in [("the capture's width", CAPTURE_PANEL_W), ("its preferred width", preferred)] {
                    lay_out(&mut app, panel, width, scale);
                    refused.extend(spills(&mut app, panel, &format!("{tag} [{width_name} {width:.0} px, interface scale {scale}]")));
                    measured += 1;
                }
            }
        }
    }
    for tag in ["PLACE", "PIECES", "AGENT", "INSPECTOR"] {
        assert!(seen.contains(&tag), "{tag} was never on screen, so this file is not the four captures' check; it saw {seen:?}");
    }
    assert!(measured >= 16, "four panels at two scales and two widths is sixteen layouts; this measured {measured}");
    assert!(refused.is_empty(), "a panel draws a control outside the box it is drawn in:\n{}", refused.join("\n"));
}
