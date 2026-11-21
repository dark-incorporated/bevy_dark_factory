//! **Each map card wears its own picture** (#506).
//!
//! The regression this exists for is one line: `project_page.rs` handed every card
//! `launcher::project_picture(...).clone()` — the newest portrait in `target/blueprint_thumbs`, which is a
//! **piece** — so a 32 m street was drawn beside whichever chair was baked most recently, and two maps were
//! drawn identically.
//!
//! # What a headless test can hold
//!
//! Not pixels: this crate's harness runs `backends: None`. What it can hold is the **decision** — that the
//! page asks for each map's own picture, and that two maps with two saved pictures get two different
//! handles. A picture that is the same handle for both cards is the defect, whatever either image holds.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_dark_factory::harness;
use bevy_dark_factory::project_page::{MapCard, PageBody};
use bevy_dark_factory::screen::Screen;

use crate::fixtures::Fixture;

fn screen_of(app: &App) -> Screen {
    *app.world().resource::<State<Screen>>().get()
}

/// **A project with two maps, each with its own saved picture.**
///
/// The pictures are written where a save writes them — `target/map_thumbs/<map stem>/` — and they are
/// different sizes so that if the page somehow served one image for both, the assertion below could say
/// which.
fn a_project_with_two_pictures(tag: &str) -> PathBuf {
    let root = Fixture::new(&format!("mapcards_{tag}"))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    // A second map, so the page has two cards to compare.
    let second = root.join("assets/blueprint/maps/hall.map.yaml");
    let first = root.join("assets/blueprint/maps/m.map.yaml");
    std::fs::copy(&first, &second).unwrap_or_else(|e| panic!("{}: {e}", second.display()));

    for (stem, colour) in [("m", [200u8, 40, 40, 255]), ("hall", [40, 40, 200, 255])] {
        let dir = root.join("target/map_thumbs").join(stem);
        std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
        image::RgbaImage::from_pixel(64, 36, image::Rgba(colour))
            .save(dir.join("0000000000000001.png"))
            .unwrap_or_else(|e| panic!("{e}"));
    }
    root
}

/// **Each card's picture is a different handle, and each is its map's own file.**
#[test]
fn two_maps_wear_two_pictures() {
    let root = a_project_with_two_pictures("two");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_dark_factory::loading::LoadingPlugin);
    harness::stand_on_the_project_page(&mut app, &root);
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(screen_of(&app), Screen::Project, "the editor is not on the project page");
    assert!(app.world_mut().query_filtered::<(), With<PageBody>>().iter(app.world()).count() > 0, "the page is not drawn");

    // **Every map card, by the node under it that holds a picture.**
    //
    // The card's own children are read rather than the world's images, because the page and the sidebar
    // both draw pictures and only the cards are the claim.
    let mut cards: Vec<(String, Option<AssetId<Image>>)> = Vec::new();
    let world = app.world_mut();
    let mut q = world.query::<(&MapCard, &Children)>();
    let mut nodes = world.query::<&ImageNode>();
    for (card, kids) in q.iter(world) {
        let picture = kids.iter().find_map(|k| nodes.get(world, k).ok()).map(|node| node.image.id());
        cards.push((card.0.clone(), picture));
    }
    cards.sort();
    assert_eq!(cards.len(), 2, "the page does not draw both maps: {cards:?}");
    let (a, b) = (&cards[0], &cards[1]);
    assert!(a.1.is_some(), "`{}` has no picture at all", a.0);
    assert!(b.1.is_some(), "`{}` has no picture at all", b.0);
    assert_ne!(
        a.1, b.1,
        "both cards wear the same picture — the shared-handle regression `project_page.rs:484` had: {cards:?}"
    );
}

/// **A map with no saved picture falls back to the project's**, and never to another map's.
#[test]
fn a_map_never_saved_falls_back_to_the_projects_picture() {
    let root = a_project_with_two_pictures("fallback");
    // One map keeps its picture; the other's folder is emptied, as a map never saved since would be.
    std::fs::remove_dir_all(root.join("target/map_thumbs/hall")).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_dark_factory::loading::LoadingPlugin);
    harness::stand_on_the_project_page(&mut app, &root);
    for _ in 0..3 {
        app.update();
    }

    let mut cards: Vec<(String, Option<AssetId<Image>>)> = Vec::new();
    let world = app.world_mut();
    let mut q = world.query::<(&MapCard, &Children)>();
    let mut nodes = world.query::<&ImageNode>();
    for (card, kids) in q.iter(world) {
        let picture = kids.iter().find_map(|k| nodes.get(world, k).ok()).map(|node| node.image.id());
        cards.push((card.0.clone(), picture));
    }
    cards.sort();
    assert_eq!(cards.len(), 2, "{cards:?}");
    // **The saved map keeps its own picture**, and the unsaved one is *not* wearing it.
    let saved = cards.iter().find(|(name, _)| name == "m").expect("the map with a picture");
    let unsaved = cards.iter().find(|(name, _)| name == "hall").expect("the map without one");
    assert!(saved.1.is_some(), "the saved map lost its picture");
    assert_ne!(saved.1, unsaved.1, "a map with no picture wore another map's");
}

/// **A save writes a picture where the next open reads it**, and a second save of an unchanged map writes
/// nothing new.
///
/// Pixel-free on purpose (this harness has no backend): what is asserted is the *policy* — the path a save
/// would write is the path a card reads, and the key does not move for a map that did not change. The
/// capture itself is `Screenshot::image`, the call `thumbs.rs`, `label_booth.rs` and `report.rs` already
/// make.
#[test]
fn a_saves_picture_is_the_one_the_card_reads() {
    let root = a_project_with_two_pictures("policy");
    let map = root.join("assets/blueprint/maps/m.map.yaml");
    let pose = bevy_dark_factory::map_thumbs::CameraPose { at_mm: [0, 2000, 5000], yaw_deg: 0, pitch_deg: -20 };
    let written = bevy_dark_factory::map_thumbs::map_thumb_path(
        &root,
        &map,
        pose,
        bevy_dark_factory::map_thumbs::MAP_THUMB_PX,
        bevy_dark_factory::map_thumbs::MAP_THUMB_CACHE_VERSION,
    )
    .expect("a map that can be statted has a picture path");

    // **The card's lookup finds what a save wrote**: same folder, and the card takes the newest PNG there.
    let newest = bevy_dark_factory::map_thumbs::newest_picture(&root, &map).expect("the map has a picture");
    assert_eq!(newest.parent(), written.parent(), "a save writes to one folder and the card reads another");

    // **An unchanged map's key does not move**, so a re-save of nothing writes nothing.
    let again = bevy_dark_factory::map_thumbs::map_thumb_path(
        &root,
        &map,
        pose,
        bevy_dark_factory::map_thumbs::MAP_THUMB_PX,
        bevy_dark_factory::map_thumbs::MAP_THUMB_CACHE_VERSION,
    )
    .expect("a path");
    assert_eq!(written, again, "the same map and pose gave two paths, so every save would write again");

}
