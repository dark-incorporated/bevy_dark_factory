//! **A map card's picture is drawn inside the card it belongs to** (#515,
//! `feedback-1795093990429`: *"This graphic is breakout out of its div"*).
//!
//! # What the owner saw
//!
//! A capture of the editor's Project workspace at 2176 x 1406 physical px (`ui_scale` 2.4, display
//! factor 2.0). Its pick is the map card `bedroom`'s `ImageBox`. Measured on the capture: the card's
//! fill runs x = 142..422 and its baked portrait runs x = 166..530 — the picture is **108 px past the
//! card's right edge**, over the panel's border and into the gutter.
//!
//! # Why the card's row is staged by its width, and why that is asserted first
//!
//! The card is the width its wrapping row leaves it, so the same card in another row is another
//! layout. On the capture that row's room was the content of a **380 physical px** panel (its border
//! ran x = 89..469), a dock beside a viewport, and the card came out 280 px — narrower than the
//! 384 px (160 logical) picture box, which is the whole defect.
//!
//! **That panel is no longer one a person can reach** (#694, the owner's "the panel fills the
//! centre"). The Project workspace's panel takes the centre and follows its pane every frame
//! (`the_left_panel_follows_its_pane`), so a width written on it is overwritten, and its divider is
//! out of the layout. Measured at `ui_scale` 2.4: at the harness's 1536 px surface the panel is
//! 1419 px, and narrowing the surface to 497 px leaves it 1381 px, because the frame will not lay
//! out narrower than its own content (1498 px). A card there is its full 184 logical px (441.6
//! physical), wider than the picture, so a panel-width staging would measure a card the defect
//! cannot touch and pass with the #515 fix reverted.
//!
//! So the staging is the **card's own row**: its width is written (`Sizing`) to the room the
//! capture's panel left the card, and the test's first assertion holds the drawn card to the
//! capture's 280 px. Every number below is then about the card the note was taken on, which is the
//! narrow card any wrapping row narrower than a card makes.
//!
//! # What is measured
//!
//! Every node under the card, by its drawn box (`ComputedNode` and `UiGlobalTransform`), held to the
//! card's **border box**: the div the note says the picture broke out of. Physical px throughout.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy_gooey::Role;

use bevy_dark_factory::harness;
use bevy_dark_factory::project_page::MapCard;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;
use crate::settle_support;

/// The capture's `ui_scale`, which is the harness's own 1.2 at a 2.0 display factor.
const CAPTURE_UI_SCALE: f32 = 2.4;

/// **The capture's card**, physical px: its fill ran x = 142..422.
const CAPTURE_CARD_PX: f32 = 280.0;

/// How far the card may stand from the capture's, physical px: the picture on the capture is a
/// 3D bake of `bedroom` while the fixture's is a flat swatch, and a card's height does not change
/// its width — this is the tolerance for the fold at the row's right end.
const CARD_SLACK_PX: f32 = 6.0;

/// How far a drawn edge may stand past the edge it is held to, physical px: layout rounds.
const SLACK_PX: f32 = 1.0;

/// A node's border-box rect, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// Every entity under `root`, `root` excluded.
fn under(world: &World, root: Entity) -> Vec<Entity> {
    let mut found = Vec::new();
    let mut queue = vec![root];
    while let Some(at) = queue.pop() {
        if let Some(children) = world.get::<Children>(at) {
            for kid in children.iter() {
                found.push(kid);
                queue.push(kid);
            }
        }
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

/// **The Project workspace as the owner met it**: a project with a baked portrait, its map card's
/// row given the room the capture's 380 physical px panel left the card (#694: see the module doc).
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("map_card_picture_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    // **A project with a picture**, as the owner's was. The newest PNG under `target/blueprint_thumbs`
    // is what a card draws; with none the card falls back to the splash and the defect's own cause —
    // a picture that keeps its 160 x 90 box while the card around it narrows — cannot be seen.
    let thumbs = root.join("target/blueprint_thumbs");
    std::fs::create_dir_all(&thumbs).expect("the project has a thumbnail folder");
    image::RgbaImage::from_pixel(128, 128, image::Rgba([90, 120, 160, 255]))
        .save(thumbs.join("0000000000000001.png"))
        .expect("a baked portrait");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    // **The door the capture was taken in**: the Project workspace inside the editor, not the home.
    *app.world_mut().resource_mut::<Workspace>() = Workspace::Project;
    settle_support::settle(&mut app);
    // **The capture's interface size**, stated rather than inferred: `UiScale` is what
    // `surface::fit_surface_to_window` writes from the window's density, and no window is spawned, so
    // the number this test sets is the number every `Val::Px` is multiplied by.
    app.world_mut().resource_mut::<UiScale>().0 = CAPTURE_UI_SCALE;
    // **The card's row, at the room the capture's panel left the card** (#694): the panel takes the
    // centre now and cannot be narrowed to the capture's 380 px, so the row the card wraps in is
    // written instead. Nothing writes this row's width but this test.
    let card = app
        .world_mut()
        .query_filtered::<Entity, With<MapCard>>()
        .single(app.world())
        .expect("the project's map has a card");
    let row = app.world().get::<ChildOf>(card).expect("the card stands in its row").parent();
    let width = Some(bevy_gooey::Extent::Px(CAPTURE_CARD_PX / CAPTURE_UI_SCALE));
    match app.world_mut().get_mut::<bevy_gooey::Sizing>(row) {
        Some(mut sizing) => sizing.width = width,
        None => {
            app.world_mut().entity_mut(row).insert(bevy_gooey::Sizing { width, ..default() });
        }
    }
    settle_support::settle(&mut app);
    app
}

/// **The map card's picture is drawn inside the card, at the width the capture's card took.**
#[test]
fn a_map_cards_picture_stays_inside_its_card() {
    let mut app = editor("project");
    let card = app
        .world_mut()
        .query_filtered::<Entity, With<MapCard>>()
        .single(app.world())
        .expect("the project's map has a card");
    let card_box = border_box(app.world(), card).expect("the card is laid out");
    // **The staging, proved before the defect is measured: the card, which is the div the note
    // names.** The capture's card is 280 px; a card wider than that has more room than the owner's
    // did (the full 441.6 px card a centred panel draws is wider than the 384 px picture) and would
    // pass on a layout nobody complained of.
    assert!(
        (card_box.width() - CAPTURE_CARD_PX).abs() <= CARD_SLACK_PX,
        "the map card is drawn {:.1} px wide, not the {CAPTURE_CARD_PX:.0} px the capture's fill ran \
         (x = 142..422): the row of cards has a different room than the frame the note was taken on",
        card_box.width()
    );

    let picture = under(app.world(), card)
        .into_iter()
        .find(|e| app.world().get::<Role>(*e) == Some(&Role::ImageBox))
        .expect("the card holds its picture — this card has a baked portrait, so it draws one");
    let picture_box = border_box(app.world(), picture).expect("the picture is laid out");

    let mut refused = Vec::new();
    for entity in under(app.world(), card) {
        let Some(rect) = border_box(app.world(), entity) else { continue };
        if rect.width() <= 0.0 || rect.height() <= 0.0 {
            continue;
        }
        if rect.max.x > card_box.max.x + SLACK_PX {
            refused.push(format!(
                "{} is drawn to {:.1}, **{:.1} px past** the card's right edge {:.1}",
                name_of(app.world(), entity),
                rect.max.x,
                rect.max.x - card_box.max.x,
                card_box.max.x
            ));
        } else if rect.min.x < card_box.min.x - SLACK_PX {
            refused.push(format!(
                "{} starts at {:.1}, **{:.1} px before** the card's left edge {:.1}",
                name_of(app.world(), entity),
                rect.min.x,
                card_box.min.x - rect.min.x,
                card_box.min.x
            ));
        }
    }
    assert!(
        refused.is_empty(),
        "a map card draws past the card it stands in (#515):\n{}\n\
         [card {card_box:?} ({:.1} px wide), the picture {picture_box:?} ({:.1} x {:.1})]",
        refused.join("\n"),
        card_box.width(),
        picture_box.width(),
        picture_box.height()
    );
    // **And the picture is not merely clipped by the card**: it is narrower than the 160 x 90 box it
    // asks for, because the box gave way to the room the card had.
    assert!(
        picture_box.width() < 160.0 * CAPTURE_UI_SCALE - SLACK_PX,
        "the picture is {:.1} px wide, the whole {:.1} px box it asks for: the card narrowed and the \
         picture kept its size, which is the spill this test is for",
        picture_box.width(),
        160.0 * CAPTURE_UI_SCALE
    );
}
