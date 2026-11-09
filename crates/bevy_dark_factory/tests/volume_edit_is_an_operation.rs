//! **`volume_edit` saves the carve list beside the map, one undo per edit** (#101).

use blueprint_model::volume::{Mode, Shape, Volume};
use bevy_dark_factory::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn project(tag: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_dark_factory::project::Project>().clone();
    let file = project.blueprint_dir.join("volume").join("m.ron");
    let open = app.world().resource::<bevy_dark_factory::project::OpenMap>().clone();
    (Ctx { project: Some(project), open: Some(open), session: "test".to_owned(), ..Ctx::default() }, file)
}

fn load(file: &std::path::Path) -> Volume {
    ron::from_str(&std::fs::read_to_string(file).unwrap()).unwrap()
}

fn done(r: Result<Applied, ops::Refusal>) -> String {
    match r.unwrap() {
        Applied::Done { operation_id, .. } => operation_id,
        _ => panic!("refused"),
    }
}

#[test]
fn carves_round_trip_through_the_project_and_each_is_one_undo() {
    let (mut ctx, file) = project("volume");
    let hill = json!({"map": "m", "edit": {"shape": {"Box": {"center": [0.0, 1.0, 0.0], "half_m": [4.0, 2.0, 4.0]}}, "mode": "Add"}});
    let tunnel = json!({"map": "m", "edit": {"shape": {"Capsule": {"a": [-5.0, 1.0, 0.0], "b": [5.0, 1.0, 0.0], "radius_m": 1.0}}, "mode": "Carve", "smooth_m": 0.25}});
    done(ops::dispatch(&mut ctx, "volume_edit", &hill));
    let second = done(ops::dispatch(&mut ctx, "volume_edit", &tunnel));
    let v = load(&file);
    assert_eq!(v.edits.len(), 2);
    assert_eq!(v.edits[1].mode, Mode::Carve);
    assert!(matches!(v.edits[1].shape, Shape::Capsule { radius_m, .. } if radius_m == 1.0));
    assert!(v.sdf([0.0, 1.0, 0.0], f32::INFINITY) > 0.0, "the tunnel is open at the middle");
    assert!(v.sdf([0.0, 2.8, 0.0], f32::INFINITY) < 0.0, "the hill still stands over it");
    ops::undo(&mut ctx, Some(&second)).unwrap();
    assert_eq!(load(&file).edits.len(), 1, "one undo takes back one edit");
}

#[test]
fn clearing_is_one_undo_and_a_bad_edit_writes_nothing() {
    let (mut ctx, file) = project("volume_clear");
    done(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "edit": {"shape": {"Sphere": {"center": [0.0, 0.0, 0.0], "radius_m": 2.0}}, "mode": "Add"}})));
    let clear = done(ops::dispatch(&mut ctx, "volume_edit", &json!({"map": "m", "clear": true})));
    assert!(!file.exists());
    ops::undo(&mut ctx, Some(&clear)).unwrap();
    assert_eq!(load(&file).edits.len(), 1);
    let bad = json!({"map": "m", "edit": {"shape": {"Sphere": {"center": [0.0, 0.0, 0.0], "radius_m": 0.0}}, "mode": "Carve"}});
    assert!(matches!(ops::dispatch(&mut ctx, "volume_edit", &bad), Err(_) | Ok(Applied::Refused(_))));
    assert_eq!(load(&file).edits.len(), 1);
}

#[test]
fn a_cover_is_saved_on_the_volume_and_one_undo_puts_the_old_one_back() {
    let (mut ctx, file) = project("volume_cover_op");
    let snowy = json!({"map": "m", "cover": {"snow": 0.8, "wetness": 0.2}});
    done(ops::dispatch(&mut ctx, "volume_set_cover", &snowy));
    let v = load(&file);
    assert_eq!(v.cover, Some(blueprint_model::volume::VolumeCover { snow: 0.8, wetness: 0.2 }));
    assert_eq!(v.version, blueprint_model::volume::VOLUME_VERSION, "the file says the version it was written in");
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(load(&file).cover, None, "undo puts the weather back in charge");

    let wet = ops::dispatch(&mut ctx, "volume_set_cover", &json!({"map": "m", "cover": {"snow": 1.5, "wetness": 0.0}}));
    let said = match wet {
        Ok(Applied::Refused(e)) | Err(e) => e.0,
        Ok(other) => panic!("a cover above 1 was accepted: {other:?}"),
    };
    assert!(said.contains("snow must be from 0 to 1"), "{said}");
}
