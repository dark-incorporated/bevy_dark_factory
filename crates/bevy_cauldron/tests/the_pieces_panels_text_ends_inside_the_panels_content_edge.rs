//! **The Pieces panel's text stays inside the panel** (#468, the part that is the editor's own). Found in a person's capture of the Kit workspace's
//! MESHES tab (`feedback-1793928048623`, Cauldron 0.2.7: *"text running off of it, and looks squished and jumbled"*), then measured on the shipped
//! 0.6.8 at the capture's own scales: a window of 2560 x 1440 physical px, UI scale 2.4, the right panel at its drag-resized minimum.
//!
//! - the hint `right: meshes / left: tiles` was `NoWrap` and ended 32 px past the panel's content edge;
//! - a long file name (`surveillance_console.glb`) was one unbreakable word in a box narrower than its glyphs, and was drawn out past the panel's
//!   border (85 px past the content edge at the capture's scales).
//!
//! A library with the names of the capture is laid out for real in a headless editor and every text under the panel is measured **from its glyphs**
//! (`TextLayoutInfo`, the size of what is drawn: a node's own box can be narrower than the words in it, which is the whole defect), at three widths of
//! the right column and two UI scales. The numbers are physical px: `ComputedNode` and `UiGlobalTransform` report physical, and the injected cursor
//! takes logical, so a position is divided by the scale before it is aimed at.
//!
//! **What is not here:** a pack band whose wrapped name is taller than the band (58 px under a 65 or 130 px text), and the rows drawn over one another
//! that follows. That is `bevy_gooey#105`: a `band()` in a scroll viewport whose rows overflow it is squeezed to its `min_height`, in gooey alone. #468
//! waits on it with `Needs: bevy_gooey#105`, and its assertions (every band as tall as its text, no two texts overlapping) are added here when it closes.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy_cauldron::chrome::{DOCK_MIN_W, LIST_W, PanelWidth, Side};
use bevy_cauldron::harness;
use bevy_cauldron::tiles::{CandidateList, ImportState, Mode};

use crate::fixtures;

/// The capture's window, physical px.
const WINDOW_PHYSICAL: (u32, u32) = (2560, 1440);

/// The capture's UI scale (the editor's 1.2 times the display's scale factor of 2) and the editor's own at a factor of 1.
const UI_SCALES: [f32; 2] = [2.4, 1.0];

/// The pack names of the capture, whose own spellings the project does not control: one that wraps once at the minimum width, and a path that wraps four times.
const PACKS: [(&str, &[&str]); 3] = [
    ("characters", &["hero"]),
    ("dimensional_crab", &["claw", "shell"]),
    ("kenney_prototype-kit/Models/GLB format", &["surveillance_console", "vending_machine", "a_short_one"]),
];

/// A pack with more meshes than the panel is tall: a list that overflows its viewport is the one a flex column squeezes, which is the capture's (`ozea` held hundreds).
const MANY: usize = 120;

/// A long library id, for the library shelf.
const LONG_ID: &str = "site/surveillance_console_with_a_very_long_name";

/// Frames a layout needs to settle after a change of scale or width.
const SETTLE_FRAMES: usize = 12;

/// How far a measured edge may stand past the one it is held to, physical px: layout rounds.
const SLACK_PX: f32 = 1.0;

/// A text as it is drawn: its rect, in physical px, from its glyphs and not from its box alone.
struct Drawn {
    words: String,
    rect: Rect,
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// The right-hand panel: the one that holds the candidate list.
fn panel_of(app: &mut App) -> Entity {
    let world = app.world_mut();
    let list = world.query_filtered::<Entity, With<CandidateList>>().single(world).unwrap_or_else(|e| panic!("one candidate list: {e}"));
    let mut at = list;
    loop {
        if world.get::<PanelWidth>(at).is_some_and(|p| p.side == Side::Right) {
            return at;
        }
        at = world.get::<ChildOf>(at).unwrap_or_else(|| panic!("the candidate list is in a right-hand panel")).parent();
    }
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

/// Every drawn text under `root`: its box widened to what its glyphs fill.
fn texts(world: &World, root: Entity) -> Vec<Drawn> {
    let mut out = Vec::new();
    for entity in under(world, root) {
        let (Some(text), Some(info), Some(computed)) = (world.get::<Text>(entity), world.get::<TextLayoutInfo>(entity), world.get::<ComputedNode>(entity)) else { continue };
        let Some(rect) = border_box(world, entity) else { continue };
        if text.0.trim().is_empty() || rect.width() <= 0.0 || rect.height() <= 0.0 || info.size.x <= 0.0 {
            continue;
        }
        let inset = computed.content_inset();
        let start = rect.min + Vec2::new(inset.min_inset.x, inset.min_inset.y);
        let run = Rect::from_corners(start, start + info.size);
        out.push(Drawn { words: text.0.clone(), rect: rect.union(run) });
    }
    out
}

/// Lay the panel out at `width_px` of the skin's px (`Sizing`, which `fit_docks_to_the_window` writes in a real window) and `scale` of UI scale.
fn lay_out(app: &mut App, panel: Entity, width_px: f32, scale: f32) {
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    app.world_mut().get_mut::<bevy_gooey::Sizing>(panel).expect("the panel is a sized box").width = Some(bevy_gooey::Extent::Px(width_px));
    settle(app);
}

/// What is wrong with the panel as it stands, as sentences: a text whose glyphs end past the panel's content edge.
fn defects(app: &mut App, panel: Entity, shelf: &str, tag: &str) -> Vec<String> {
    let world = app.world();
    let mut refused = Vec::new();
    let Some(panel_box) = border_box(world, panel) else { return vec![format!("{tag} {shelf}: the panel is not laid out")] };
    let inset = world.get::<ComputedNode>(panel).map(ComputedNode::content_inset).unwrap_or_default();
    let content_right = panel_box.max.x - inset.max_inset.x;
    let drawn = texts(world, panel);
    if drawn.len() < 4 {
        refused.push(format!("{tag} {shelf}: only {} texts are drawn under the panel", drawn.len()));
    }
    if !drawn.iter().any(|d| d.words == "surveillance_console.glb") {
        refused.push(format!("{tag} {shelf}: the long file name is not drawn, so its fit is not measured"));
    }

    for d in &drawn {
        if d.rect.max.x > content_right + SLACK_PX {
            refused.push(format!("{tag} {shelf}: `{}` ends at {:.1}, {:.1} px past the panel's content edge {content_right:.1}", d.words, d.rect.max.x, d.rect.max.x - content_right));
        }
    }

    refused
}

fn editor(tag: &str) -> App {
    let mut fixture = fixtures::Fixture::new(&format!("pieces_text_{tag}_{}", std::process::id())).descriptor(LONG_ID, "alpha");
    for (pack, meshes) in PACKS {
        fixture = fixture.pack(pack, meshes);
    }
    let many: Vec<String> = (0..MANY).map(|i| format!("ozea_{i:03}")).collect();
    fixture = fixture.pack("ozea", &many.iter().map(String::as_str).collect::<Vec<_>>());
    let root = fixture.build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, WINDOW_PHYSICAL.0, WINDOW_PHYSICAL.1).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    // Every pack open, as the capture's were: the scan the first frames run opens the first and folds the rest.
    app.world_mut().resource_mut::<ImportState>().folded_packs.clear();
    settle(&mut app);
    app
}

/// **At the capture's scales and one more, at three widths of the right column, every text of the MESHES tab's Pieces panel ends at or inside the panel's content edge.**
#[test]
fn the_pieces_panels_text_ends_inside_the_panels_content_edge() {
    let mut app = editor("candidates");
    assert!(app.world().resource::<ImportState>().scanned, "entering MESHES scanned the project's packs");
    let panel = panel_of(&mut app);
    let mut refused = Vec::new();
    for scale in UI_SCALES {
        for (name, width) in [("minimum", DOCK_MIN_W), ("ordinary", LIST_W), ("one and a half times", LIST_W * 1.5)] {
            lay_out(&mut app, panel, width, scale);
            refused.extend(defects(&mut app, panel, "candidates", &format!("[UI scale {scale}, {name} {width:.0} px]")));
        }
    }
    assert!(refused.is_empty(), "the Pieces panel's text runs past its content edge:\n{}", refused.join("\n"));
}
