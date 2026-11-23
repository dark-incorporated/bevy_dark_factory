//! **The INSPECTOR's `mount` / `front` row says both of its values whole, inside the panel** (#520,
//! `feedback-1795094167584`: *"This is squished to the point we can't see it."*).
//!
//! # What the owner saw
//!
//! A capture of the editor at 2176 x 1406 physical px (`ui_scale` 2.4, display factor 2.0). The left
//! INSPECTOR panel's border ran x = 89..583 there, and its content edge x = 548. The pick is the
//! `E face` text of the row holding `mount`, its chip, `front` and the front's value: on the capture
//! that value was broken over **two lines** (`E` over `face`) and its second line was drawn past the
//! panel's border.
//!
//! # Why the panel is staged by its width, and why that is asserted first
//!
//! The row's room is the panel's content width, so the same row at another panel width is another
//! layout. The capture's panel is **494 physical px** (x = 89..583). A staging that said so only in
//! a comment would measure whatever the day's dock arithmetic produces — and the arithmetic gives
//! 381 or 548 px depending on whether the divider has been dragged, neither of which is the
//! capture's. So the panel's width is **written** (`Sizing`, the component
//! `fit_docks_to_the_window` writes on a real window; no `PrimaryWindow` is spawned, so nothing here
//! argues with it), and the test's first assertion holds the drawn panel to the capture's 494 px.
//!
//! # What is measured
//!
//! Every text of the row **from its glyphs** (`TextLayoutInfo`, the union of the boxes of what it
//! paints): a node's box can be narrower than the words in it, and a value broken over two lines
//! inside its box is the other half of "squished". Two assertions, both the note's own complaint:
//!
//! 1. nothing of the row is drawn past the panel's content edge;
//! 2. each label and each value is drawn on **one line** — a value that needs two lines in a row this
//!    tall is the squeeze the note is about, whatever its right edge measures.
//!
//! Physical px throughout: `ComputedNode` and `UiGlobalTransform` report physical.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy_gooey::Role;

use bevy_dark_factory::chrome::{self, PanelWidth, Side};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::ControlId;
use bevy_dark_factory::tiles::{ImportState, Mode};

use crate::fixtures;
use crate::settle_support;

/// **The capture's panel**, physical px: its border ran x = 89..583.
const CAPTURE_PANEL_PX: f32 = 494.0;

/// The capture's `ui_scale`, which is the harness's own 1.2 at a 2.0 display factor.
const CAPTURE_UI_SCALE: f32 = 2.4;

/// How far the staged panel may stand from the capture's width, physical px — a whole-pixel layout
/// round on either border.
const WIDTH_SLACK_PX: f32 = 2.0;

/// How far a drawn edge may stand past the edge it is held to, physical px.
const SLACK_PX: f32 = 1.0;


/// The piece the panel describes: a chair with a front, as the capture's `ozea/chair.glb` is.
const PIECE: &str = "ozea/chair";



/// A node's border-box rect, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **What a node is drawn over**: its box, widened to the union of the boxes of the glyphs it paints.
fn drawn(world: &World, entity: Entity) -> Option<Rect> {
    let rect = border_box(world, entity)?;
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    let (Some(text), Some(info), Some(computed)) = (world.get::<Text>(entity), world.get::<TextLayoutInfo>(entity), world.get::<ComputedNode>(entity)) else {
        return Some(rect);
    };
    if text.0.trim().is_empty() || info.glyphs.is_empty() {
        return Some(rect);
    }
    let start = rect.min + computed.content_inset().min_inset;
    let mut painted = Rect::from_corners(start, start);
    for glyph in &info.glyphs {
        let at = start + glyph.position;
        painted = painted.union(Rect::from_corners(at, at + glyph.atlas_info.rect.size()));
    }
    Some(rect.union(painted))
}

/// **How many lines a text is drawn on**, counted off the glyphs themselves: every glyph carries the
/// position it was laid out at, so two glyphs at different heights are two lines whatever the box
/// around them says. Grouped to the whole physical px, which is the resolution a glyph is drawn at.
fn lines_of(world: &World, entity: Entity) -> Option<usize> {
    let info = world.get::<TextLayoutInfo>(entity)?;
    if info.glyphs.is_empty() {
        return None;
    }
    // **A line, not a pixel of baseline.** Every glyph on one line is positioned by its own top
    // edge, so an `E` and an `m` beside it start at different heights; what tells two lines apart is
    // a gap wider than half a glyph. Measured on this row, `mount` alone spans 5 px of glyph tops and
    // a second line is 29 px away.
    let tallest = info.glyphs.iter().map(|glyph| glyph.atlas_info.rect.size().y).fold(0.0f32, f32::max);
    let half = (tallest * 0.5).max(1.0);
    let mut tops: Vec<f32> = info.glyphs.iter().map(|glyph| glyph.position.y).collect();
    tops.sort_by(f32::total_cmp);
    let mut lines = 1usize;
    let mut last = tops.first().copied().unwrap_or_default();
    for top in tops.into_iter().skip(1) {
        if top - last > half {
            lines += 1;
        }
        last = top;
    }
    Some(lines)
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

/// What a node is, in words a person can read back.
fn name_of(world: &World, entity: Entity) -> String {
    if let Some(text) = world.get::<Text>(entity)
        && !text.0.trim().is_empty()
    {
        return format!("`{}`", text.0.trim());
    }
    match world.get::<Role>(entity) {
        Some(role) => format!("a {role:?}"),
        None => format!("{entity:?}"),
    }
}

/// **The Kit door on its MESHES page, with the capture's piece selected**, the INSPECTOR panel laid
/// out at the capture's 494 physical px.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("squished_front_row_{tag}_{}", std::process::id()))
        .descriptor(PIECE, "ozea")
        .place(PIECE, (0.0, 0.0))
        .build("m");
    // **A piece with a front**, which is what makes the row two label/value pairs: `front` and its
    // value are the pair the capture's pick is in. A piece with no front draws `none`, which is one
    // short word and would not squeeze.
    let library = root.join("assets/blueprint/furniture/library.yaml");
    fixtures::edit_library(&library, |lib| {
        for d in &mut lib.descriptors {
            if d.id == PIECE {
                d.align.front = Some(blueprint_model::descriptor::Face::East);
            }
        }
    });
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    // **The piece the panel describes**: the library row the capture's PIECES list has selected.
    app.world_mut().resource_mut::<ImportState>().selected_library_id = Some(PIECE.to_owned());
    settle_support::settle(&mut app);
    // **The capture's interface size**, stated rather than inferred: `UiScale` is what
    // `surface::fit_surface_to_window` writes from the window's density, and no window is spawned, so
    // the number this test sets is the number every `Val::Px` is multiplied by.
    app.world_mut().resource_mut::<UiScale>().0 = CAPTURE_UI_SCALE;
    // **The panel the row stands in**: the visible left one, at the capture's width.
    let panel = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &PanelWidth, &Node, &ComputedNode)>();
        q.iter(world)
            .filter(|(_, width, node, computed)| width.side == Side::Left && node.display != Display::None && computed.size().x > 0.0)
            .map(|(entity, _, _, _)| entity)
            .next()
            .expect("the Kit door draws a panel in the left dock")
    };
    app.world_mut()
        .get_mut::<bevy_gooey::Sizing>(panel)
        .expect("the panel is a sized box")
        .width = Some(bevy_gooey::Extent::Px(CAPTURE_PANEL_PX / CAPTURE_UI_SCALE));
    settle_support::settle(&mut app);
    app
}

/// **The `mount` / `front` row says both of its values whole, inside the panel, at the capture's width.**
#[test]
fn the_mount_and_front_row_says_both_values_inside_the_panel() {
    let mut app = editor("detail");
    let panel = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &PanelWidth, &Node, &ComputedNode)>();
        q.iter(world)
            .filter(|(_, width, node, computed)| width.side == Side::Left && node.display != Display::None && computed.size().x > 0.0)
            .map(|(entity, _, _, _)| entity)
            .next()
            .expect("the Kit door draws a panel in the left dock")
    };

    // **The staging, proved before the defect is measured.** A panel of some other width gives the
    // row some other room, and this issue is about the room the capture's panel left.
    let panel_box = border_box(app.world(), panel).expect("the panel is laid out");
    assert!(
        (panel_box.width() - CAPTURE_PANEL_PX).abs() <= WIDTH_SLACK_PX,
        "the INSPECTOR panel is drawn {:.1} px wide, not the {CAPTURE_PANEL_PX:.0} px the capture ran \
         (its border went x = 89..583) — this is not the layout #520 was filed about",
        panel_box.width()
    );

    // **The row `M` cycles** — the one the capture's pick is inside.
    let row = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &chrome::Control)>();
        q.iter(world)
            .find(|(_, control)| control.0 == ControlId::Mount)
            .map(|(entity, _)| entity)
            .expect("the detail panel draws the mount row")
    };
    let inset = app.world().get::<ComputedNode>(panel).map(ComputedNode::content_inset).unwrap_or_default();
    let content_right = panel_box.max.x - inset.max_inset.x;

    let mut refused = Vec::new();
    let mut texts = 0usize;
    for entity in under(app.world(), row) {
        let Some(rect) = drawn(app.world(), entity) else { continue };
        let is_text = app.world().get::<Text>(entity).is_some_and(|t| !t.0.trim().is_empty());
        if is_text {
            texts += 1;
        }
        // **1. Nothing of the row is drawn past the panel's content edge.**
        if rect.max.x > content_right + SLACK_PX {
            refused.push(format!(
                "{} is drawn to {:.1}, **{:.1} px past** the panel's content edge {content_right:.1}",
                name_of(app.world(), entity),
                rect.max.x,
                rect.max.x - content_right
            ));
        }
        // **2. Each word of the row is drawn whole, on one line.** A value broken in two is the
        // squeeze the note names, whether or not its edge happens to land inside the panel — and the
        // second line of a broken value is drawn *under* the value beside it, which is what "we can't
        // see it" looks like.
        if is_text
            && let Some(lines) = lines_of(app.world(), entity)
            && lines > 1
        {
            refused.push(format!(
                "{} is drawn on **{lines} lines**, not one — it is squeezed into a box narrower \
                 than its words ({rect:?})",
                name_of(app.world(), entity)
            ));
        }
    }
    assert!(texts >= 3, "the mount row draws only {texts} texts, so this measures nothing");
    assert!(
        refused.is_empty(),
        "the detail panel's mount/front row is squashed at the capture's panel width (#520):\n{}\n\
         [panel {panel_box:?}, its content edge {content_right:.1}, the row {:?}]",
        refused.join("\n"),
        border_box(app.world(), row)
    );
}
