//! **A panel heading's "?" sits on the title's centre line, inside its own pill** (#487,
//! `feedback-1794784645626`).
//!
//! # What the owner saw
//!
//! *"The icon and the text don't line up here."* Measured on the capture at `ui_scale` 2.4, in one
//! `INSPECTOR  [?]` row:
//!
//! - the word `INSPECTOR`'s ink is `x=125..344, y=414..451`, centre `y=432.5`;
//! - the button's pill is the `(34,34,34)` box at `x=348..492, y=413..467`, centre `y=440`;
//! - the `?` glyph is drawn at `x=341..461, y=414..453`, centre `y=433.5`.
//!
//! So the glyph is drawn **outside the left edge of its own pill** by 7 px and 6.5 px above the
//! pill's centre, and its ink starts at 341 while the word ends at 344 — they overlap.
//!
//! # What this file holds down
//!
//! The acceptance is three centres within 1 px at `interface_scale` 1.0 and 1.6, and the glyph's box
//! inside the pill's box on every side. Both are measured on the **world-space** rects of the three
//! nodes a heading row is made of: the title `Text`, the pill (`Role::Button`, the icon button's own
//! node) and the glyph (`Role::Icon`, the picture inside it).
//!
//! **A `PrimaryWindow` is spawned**, because a scale other than the harness's own is only a scale if
//! something fits the surface to a window — the same reason `a_dock_resize_never_covers_the_viewport`
//! spawns one.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::{PrimaryWindow, Window, WindowResolution};

use bevy_cauldron::harness;

use crate::fixtures;

/// The interface scales the acceptance names.
const SCALES: [f32; 2] = [1.0, 1.6];

/// How far two centres may stand apart, physical px — the acceptance's own 1 px, with the half-frame a
/// layout round can add.
const ALIGN_PX: f32 = 1.5;

/// Frames a layout needs to settle after a change of scale.
const SETTLE_FRAMES: usize = 14;

/// The window, physical px: a 1280 x 703 screen at the capture's 2.0 display factor.
const WINDOW_PHYSICAL: (u32, u32) = (2560, 1406);

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

fn editor(name: &str, scale: f32) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.world_mut().spawn((
        Window { resolution: WindowResolution::new(WINDOW_PHYSICAL.0, WINDOW_PHYSICAL.1), ..default() },
        PrimaryWindow,
    ));
    app.world_mut().resource_mut::<UiScale>().0 = scale;
    settle(&mut app);
    (root, app)
}

/// A node's border-box rect, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **The three nodes of one heading row**: the title, the pill and the glyph.
struct Heading {
    title: Entity,
    pill: Entity,
    glyph: Entity,
}

/// Every heading row the editor drew, with its three parts resolved.
///
/// The row is `help::PanelHeading`'s: its first child is the panel's title `Text`, and beside it is the
/// icon button, whose own node is the pill and whose child is the glyph.
fn headings(world: &mut World) -> Vec<Heading> {
    let rows: Vec<Entity> = world
        .query_filtered::<Entity, With<bevy_cauldron::help::PanelHeading>>()
        .iter(world)
        .collect();
    let mut found = Vec::new();
    for row in rows {
        let Some(children) = world.get::<Children>(row) else { continue };
        let mut title = None;
        let mut pill = None;
        for child in children.iter() {
            if world.get::<Text>(child).is_some() {
                title = Some(child);
            } else if world.get::<bevy_gooey::Role>(child) == Some(&bevy_gooey::Role::Button) {
                pill = Some(child);
            }
        }
        let (Some(title), Some(pill)) = (title, pill) else { continue };
        let glyph = world
            .get::<Children>(pill)
            .and_then(|kids| kids.iter().find(|child| world.get::<bevy_gooey::Role>(*child) == Some(&bevy_gooey::Role::Icon)));
        let Some(glyph) = glyph else { continue };
        found.push(Heading { title, pill, glyph });
    }
    // A query's order over archetypes is a hash; sort so two runs report the same rows in one order.
    found.sort_by_key(|h| (h.title.index(), h.pill.index()));
    found
}

/// **The word, its pill and the glyph share one centre line, and the glyph is inside its pill.**
#[test]
fn a_panel_headings_glyph_sits_on_its_titles_centre_line_inside_its_pill() {
    let mut refused: Vec<String> = Vec::new();
    for scale in SCALES {
        let (_root, mut app) = editor(&format!("panel_heading_{scale}_{}", std::process::id()), scale);
        let found = headings(app.world_mut());
        assert!(
            !found.is_empty(),
            "the Map door's panels each carry a heading row with a \"?\"; none was found at scale {scale}"
        );
        let world = app.world();
        for heading in found {
            let (Some(title), Some(pill), Some(glyph)) =
                (border_box(world, heading.title), border_box(world, heading.pill), border_box(world, heading.glyph))
            else {
                refused.push(format!("[scale {scale}] a heading row is not laid out"));
                continue;
            };
            let words = world.get::<Text>(heading.title).map(|t| t.0.clone()).unwrap_or_default();
            // **1. One centre line.** The vertical centre of all three, in the same units.
            let (ty, py, gy) = (title.center().y, pill.center().y, glyph.center().y);
            let spread = [ty, py, gy].iter().cloned().fold(f32::MIN, f32::max) - [ty, py, gy].iter().cloned().fold(f32::MAX, f32::min);
            if spread > ALIGN_PX {
                refused.push(format!(
                    "[scale {scale}] the row for `{words}` has three centre lines: the word {ty:.1}, its pill {py:.1}, \
                     the glyph {gy:.1} — a spread of {spread:.1} px"
                ));
            }
            // **2. The glyph is inside its pill, on every side.**
            if glyph.min.x < pill.min.x - ALIGN_PX
                || glyph.max.x > pill.max.x + ALIGN_PX
                || glyph.min.y < pill.min.y - ALIGN_PX
                || glyph.max.y > pill.max.y + ALIGN_PX
            {
                refused.push(format!(
                    "[scale {scale}] the glyph of `{words}` is not inside its pill: pill {pill:?}, glyph {glyph:?}"
                ));
            }
            // **3. And it does not overlap the words.** The capture's glyph began at x=341 while the
            // word ended at 344.
            if glyph.min.x < title.max.x - ALIGN_PX {
                refused.push(format!(
                    "[scale {scale}] the glyph of `{words}` overlaps its title: the word ends at {:.1}, the glyph \
                     begins at {:.1}",
                    title.max.x, glyph.min.x
                ));
            }
        }
    }
    assert!(
        refused.is_empty(),
        "a panel heading's \"?\" is off its title's line:\n{}",
        refused.join("\n")
    );
}
