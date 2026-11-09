//! **`family_set` writes one field of one of a family crate's files, `family/<crate>/<Type>.ron`, and is one
//! undo** (#96). The file here is a data file no build registers as a settings type, so only its RON is held to.

use bevy_dark_factory::ops::{self, Applied, Ctx};
use serde_json::json;

use crate::fixtures;

fn project(tag: &str, listed: &str) -> (Ctx, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    std::fs::create_dir_all(root.join("family/bevy_elements")).unwrap();
    std::fs::write(root.join("family.ron"), format!("(version: 1, crates: [\"{listed}\"])")).unwrap();
    let file = root.join("family/bevy_elements/Sky.ron");
    std::fs::write(&file, "// the sun\n(sun: (hour: 12.0, tilt: 0.4), wind: 3)\n").unwrap();
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_dark_factory::project::Project>().clone();
    (Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() }, file)
}

fn set(ctx: &mut Ctx, value: &str) -> Result<Applied, ops::Refusal> {
    ops::dispatch(ctx, "family_set", &json!({"crate": "bevy_elements", "type": "Sky", "field": "sun.hour", "value": value}))
}

#[test]
fn a_field_is_written_in_place_and_one_undo_puts_it_back() {
    let (mut ctx, file) = project("undo", "bevy_elements");
    let before = std::fs::read_to_string(&file).unwrap();
    assert!(matches!(set(&mut ctx, "6.5").unwrap(), Applied::Done { .. }));
    let after = std::fs::read_to_string(&file).unwrap();
    assert_eq!(after, "// the sun\n(sun: (hour: 6.5, tilt: 0.4), wind: 3)\n", "only the value's bytes change");
    ops::undo(&mut ctx, None).unwrap();
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before, "one undo restores the file exactly");
}

#[test]
fn an_unlisted_crate_or_a_bad_value_changes_nothing() {
    let (mut ctx, file) = project("family_set_is_an_operation_refuse", "bevy_flora");
    let before = std::fs::read_to_string(&file).unwrap();
    let e = match set(&mut ctx, "6.5") {
        Err(e) | Ok(Applied::Refused(e)) => e.0,
        Ok(_) => panic!("an unlisted crate was written"),
    };
    assert!(e.contains("not listed in family.ron"), "{e}");
    let (mut ctx2, file2) = project("badvalue", "bevy_elements");
    assert!(matches!(set(&mut ctx2, "(1.0"), Err(_) | Ok(Applied::Refused(_))));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), before);
    assert_eq!(std::fs::read_to_string(&file2).unwrap(), before);
    let _ = &ctx2;
}

#[test]
fn a_type_with_no_file_is_refused_with_the_way_to_write_its_defaults() {
    let (mut ctx, _) = project("family_set_missing", "bevy_elements");
    let e = match ops::dispatch(&mut ctx, "family_set", &json!({"crate": "bevy_elements", "type": "Moonlight", "field": "lux", "value": "1.0"})) {
        Err(e) | Ok(Applied::Refused(e)) => e.0,
        Ok(_) => panic!("a file that does not exist was written"),
    };
    assert_eq!(e, "family/bevy_elements/Moonlight.ron does not exist; switch bevy_elements on in the Plugins door to write its defaults");
}
