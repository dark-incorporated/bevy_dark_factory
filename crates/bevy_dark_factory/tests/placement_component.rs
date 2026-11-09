//! **A piece carries a crate's authoring component** (#103): `placement_set_component` saves it on the row
//! under its type path, the piece's entity in the viewport gets the component by reflection (a `Flicker`
//! with the `PointLight` it brings), and one undo takes both back off.
#![cfg(feature = "bevy_lights")]

use bevy::prelude::*;
use bevy::reflect::TypePath;
use bevy_dark_factory::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn editor(name: &str) -> (std::path::PathBuf, App, Ctx) {
    let root = fixtures::Fixture::new(name).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    std::fs::write(root.join("family.ron"), "(version: 1, crates: [\"bevy_lights\"])").unwrap();
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_dark_factory::project::Project>().clone();
    let open = app.world().resource::<bevy_dark_factory::project::OpenMap>().clone();
    (root, app, Ctx { project: Some(project), open: Some(open), session: name.to_owned(), ..Ctx::default() })
}

fn saved(root: &std::path::Path) -> blueprint_model::map::Map {
    let text = std::fs::read_to_string(root.join("assets/blueprint/maps/m.map.ron")).unwrap();
    ron::from_str(&text).unwrap()
}

/// Hands the context's map to the app and runs it, as the editor's own dispatch does.
fn show(app: &mut App, ctx: &Ctx) {
    if let Some(open) = ctx.open.clone() {
        app.world_mut().insert_resource(open);
    }
    for _ in 0..3 {
        app.update();
    }
}

/// The piece's entity: whether it carries a `Flicker` and a `PointLight`.
fn piece(app: &mut App, id: &str) -> (bool, bool) {
    let mut q = app.world_mut().query::<(&bevy_dark_factory::editor::Placement, Has<bevy_lights::Flicker>, Has<PointLight>)>();
    q.iter(app.world()).find(|(p, ..)| p.0 == id).map(|(_, f, l)| (f, l)).unwrap_or_else(|| panic!("no entity for {id}"))
}

#[test]
fn a_flicker_put_on_a_piece_is_saved_drawn_with_its_light_and_undone() {
    let (root, mut app, mut ctx) = editor("piece_flicker");
    let id = ctx.open.as_ref().unwrap().map.placements[0].id.clone();
    let path = bevy_lights::Flicker::type_path();
    let done = ops::dispatch(&mut ctx, "placement_set_component", &json!({"id": id, "component": path, "value": "(base: 900.0, phase: 1.3, depth: 0.15)"})).unwrap();
    assert!(matches!(done, Applied::Done { .. }), "{done:?}");
    let row = &saved(&root).placements[0];
    let read: bevy_lights::Flicker = ron::from_str(&row.components[path]).unwrap();
    assert_eq!((read.base, read.phase, read.depth), (900.0, 1.3, 0.15));
    show(&mut app, &ctx);
    assert_eq!(piece(&mut app, &id), (true, true), "the piece's entity carries the crate's Flicker and the PointLight it brings");

    ops::undo(&mut ctx, None).unwrap();
    assert!(saved(&root).placements[0].components.is_empty(), "undo takes the component back off the row");
    show(&mut app, &ctx);
    assert_eq!(piece(&mut app, &id), (false, false), "and off the entity, with its light");
}

#[test]
fn a_component_this_build_does_not_register_is_refused() {
    let (root, _app, mut ctx) = editor("piece_component_unknown");
    let id = ctx.open.as_ref().unwrap().map.placements[0].id.clone();
    let r = ops::dispatch(&mut ctx, "placement_set_component", &json!({"id": id, "component": "nope::X", "value": "()"}));
    let said = match r {
        Ok(Applied::Refused(e)) | Err(e) => e.0,
        Ok(other) => panic!("an unregistered component was accepted: {other:?}"),
    };
    assert!(said.contains("no registered component nope::X"), "{said}");
    assert!(saved(&root).placements[0].components.is_empty());
}

#[test]
fn a_value_the_components_own_type_cannot_read_is_refused_and_nothing_is_written() {
    let (root, _app, mut ctx) = editor("piece_flicker_bad");
    let id = ctx.open.as_ref().unwrap().map.placements[0].id.clone();
    let r = ops::dispatch(&mut ctx, "placement_set_component", &json!({"id": id, "component": bevy_lights::Flicker::type_path(), "value": "(base: \"bright\")"}));
    let said = match r {
        Ok(Applied::Refused(e)) | Err(e) => e.0,
        Ok(other) => panic!("a wrongly typed field was accepted: {other:?}"),
    };
    assert!(said.contains("Flicker"), "{said}");
    assert!(saved(&root).placements[0].components.is_empty());
}
