//! **An agent can ask a carve list what it made** (#473): `grimoire/volume_report`.
//!
//! `volume_edit` takes one entry per call and answers `edited`, and `grimoire_preview_operation`
//! previews one edit against the map — so **every defect the issue names exists only in the
//! sequence**, and neither door can show one. Three were found in one session of work by hand, with a
//! Python voxel sampler over the YAML.
//!
//! **The three `at:` checks the issue's acceptance block names are here verbatim**, on a lair laid out
//! to those coordinates: `[[40, 6.5, 31.9]]` (the dais's top step) is `solid: true`, `[[40, 19, 40]]`
//! (the crown vault) is `solid: false`, `[[47, 9, 26]]` (the rerouted gallery) is `solid: false`.
//!
//! **It is a read**: no file written, `grimoire_state.map.dirty` stays `false`, and no line joins
//! `oplog.jsonl` — which is the reason it is a verb and not an operation.

use bevy::prelude::*;
use bevy_cauldron::project::{OpenMap, Project};
use bevy_grimoire_core::volume::{Edit, Mode, Shape, Volume};
use serde_json::{Value, json};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_cauldron::ops_brp::OpsBridgePlugin);
    for _ in 0..4 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: Value) -> Result<Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut().run_system_with(id, Some(params)).map_err(|e| format!("{method}: {e:?}"))?.map_err(|e| format!("{method}: {e:?}"))
}

fn write_volume(app: &App, text: &str) {
    let dir = app.world().resource::<Project>().grimoire_dir.join("volume");
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    std::fs::write(dir.join("m.yaml"), text).unwrap_or_else(|e| panic!("{e}"));
}

fn oplog_lines(app: &App) -> usize {
    let dir = app.world().resource::<Project>().project_dir.clone();
    std::fs::read_to_string(dir.join("oplog.jsonl")).map_or(0, |t| t.lines().filter(|l| !l.trim().is_empty()).count())
}

/// **The lair of the issue, in miniature, at its own coordinates**: a crypt carved out of the rock at
/// y 6, a dais and a column added inside it, a crown vault above, and a gallery carved last at x 47.5
/// — the issue's third position, which clipped the dais's top step by 0.2 m.
///
/// **Written through the core writer**, so the file is exactly the shape the reader accepts: a hand
/// YAML here would test the file's spelling rather than the report.
fn lair() -> Vec<Edit> {
    let edit = |shape: Shape, mode: Mode| Edit { shape, mode, smooth_m: 0.0 };
    vec![
        edit(Shape::Box { center: [41.0, 9.0, 32.0], half_m: [3.0, 3.0, 4.0] }, Mode::Carve),
        edit(Shape::Box { center: [40.0, 6.4, 31.9], half_m: [8.0, 0.4, 2.0] }, Mode::Add),
        edit(Shape::Box { center: [43.0, 11.0, 32.0], half_m: [0.4, 5.0, 0.4] }, Mode::Add),
        edit(Shape::Sphere { center: [40.0, 19.0, 40.0], radius_m: 2.0 }, Mode::Carve),
        edit(Shape::Capsule { a: [47.5, 9.0, 26.0], b: [47.5, 9.0, 34.0], radius_m: 1.0 }, Mode::Carve),
    ]
}

fn write_lair(app: &App, edits: Vec<Edit>) {
    let volume = Volume { version: bevy_grimoire_core::volume::VOLUME_VERSION, edits };
    match bevy_grimoire_core::yaml::to_string(&volume) {
        Ok(text) => write_volume(app, &text),
        Err(p) => panic!("the volume writes: {}", p.line()),
    }
}

#[test]
fn the_three_at_checks_the_issue_names_answer_solid_or_air() {
    let mut app = editor("volume_report_at");
    write_lair(&app, lair());
    let report = call(
        &mut app,
        "grimoire/volume_report",
        json!({"at": [[40.0, 6.5, 31.9], [40.0, 19.0, 40.0], [47.0, 9.0, 26.0]]}),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let at = report["at"].as_array().unwrap_or_else(|| panic!("the answer has an at list: {report}"));
    assert_eq!(at.len(), 3, "one answer per point asked: {report}");
    // **The three checks verbatim.** The dais's top step is solid; the crown vault and the rerouted
    // gallery are air.
    assert_eq!(at[0]["solid"], json!(true), "at [[40, 6.5, 31.9]] the dais's top step is solid: {}", at[0]);
    assert_eq!(at[1]["solid"], json!(false), "at [[40, 19, 40]] the crown vault is air: {}", at[1]);
    assert_eq!(at[2]["solid"], json!(false), "at [[47, 9, 26]] the rerouted gallery is air: {}", at[2]);
    // And the entries that contain each point are named, in list order.
    assert!(at[0]["containing"].as_array().is_some_and(|c| c.contains(&json!(1))), "the dais is named: {}", at[0]);
    assert!(at[2]["containing"].as_array().is_some_and(|c| c.contains(&json!(4))), "the gallery is named: {}", at[2]);
}

#[test]
fn a_crown_vault_reports_as_an_isolated_component_of_one_and_is_not_an_error() {
    let mut app = editor("volume_report_vault");
    write_lair(&app, lair());
    let report = call(&mut app, "grimoire/volume_report", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let components = report["components"].as_array().unwrap_or_else(|| panic!("components is a list: {report}"));
    let vault = components
        .iter()
        .find(|c| c.as_array().is_some_and(|r| r.contains(&json!(3))))
        .unwrap_or_else(|| panic!("the crown vault is a component of its own: {components:?}"));
    assert_eq!(vault, &json!([3]), "a component of one: {vault}");
    // **Not an error**: the vault is not in `isolated`, and no cut names it.
    assert!(
        report["isolated"].as_array().is_none_or(|i| !i.contains(&json!(3))),
        "a crown vault is not isolated and not an error: {}",
        report["isolated"]
    );
    let cuts = report["carves_that_cut_a_later_solid"].as_array().unwrap_or_else(|| panic!("{report}"));
    assert!(cuts.iter().all(|c| c["solid"] != json!(3)), "the vault cuts no Add: {cuts:?}");
}

#[test]
fn an_add_a_later_carve_cuts_through_is_named_by_the_report() {
    let mut app = editor("volume_report_cut");
    // A column added, then a gallery carved through it after: the second defect in the issue's table.
    let edit = |shape: Shape, mode: Mode| Edit { shape, mode, smooth_m: 0.0 };
    write_lair(
        &app,
        vec![
            edit(Shape::Box { center: [0.0, 8.0, 0.0], half_m: [0.5, 4.0, 0.5] }, Mode::Add),
            edit(Shape::Capsule { a: [-8.0, 8.0, 0.0], b: [8.0, 8.0, 0.0], radius_m: 1.0 }, Mode::Carve),
        ],
    );
    let report = call(&mut app, "grimoire/volume_report", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let cuts = report["carves_that_cut_a_later_solid"].as_array().unwrap_or_else(|| panic!("{report}"));
    let cut = cuts.iter().find(|c| c["solid"] == json!(0)).unwrap_or_else(|| panic!("the column was cut: {cuts:?}"));
    assert_eq!(cut["cut_by"], json!(1), "by the gallery after it: {cut}");
    assert!(cut["overlapping_m"].as_f64().is_some_and(|m| m > 0.0), "and how deep: {cut}");
}

#[test]
fn the_report_names_every_entry_with_its_kind_mode_and_reach() {
    let mut app = editor("volume_report_entries");
    write_lair(&app, lair());
    let report = call(&mut app, "grimoire/volume_report", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let entries = report["entries"].as_array().unwrap_or_else(|| panic!("{report}"));
    assert_eq!(entries.len(), 5, "every entry is named: {entries:?}");
    assert_eq!(entries[0]["shape"], json!("Box"));
    assert_eq!(entries[0]["mode"], json!("Carve"));
    assert_eq!(entries[1]["mode"], json!("Add"));
    assert_eq!(entries[4]["shape"], json!("Capsule"));
    // The dais's box reaches its own half-extents about its centre: bottom 6.0, top 6.8.
    let b = &entries[1]["bounds"];
    assert!((b["min_m"][1].as_f64().unwrap_or_default() - 6.0).abs() < 1e-3, "the dais's bottom: {b}");
    assert!((b["max_m"][1].as_f64().unwrap_or_default() - 6.8).abs() < 1e-3, "and its top step: {b}");
    assert_eq!(report["map"], json!("m"), "and it says which map it answered for: {report}");
}

#[test]
fn the_report_is_a_read_and_changes_nothing() {
    let mut app = editor("volume_report_read_only");
    write_lair(&app, lair());
    app.update();
    let before_log = oplog_lines(&app);
    let before_dirty = app.world().resource::<OpenMap>().dirty();
    let before_file = std::fs::read_to_string(app.world().resource::<Project>().grimoire_dir.join("volume").join("m.yaml")).unwrap();

    let _ = call(&mut app, "grimoire/volume_report", json!({"at": [[40.0, 6.5, 31.9]]})).unwrap_or_else(|e| panic!("{e}"));
    app.update();

    assert_eq!(oplog_lines(&app), before_log, "**no line joins oplog.jsonl**: a report is not a change to the person's project");
    assert_eq!(app.world().resource::<OpenMap>().dirty(), before_dirty, "the map is not marked dirty");
    let after_file = std::fs::read_to_string(app.world().resource::<Project>().grimoire_dir.join("volume").join("m.yaml")).unwrap();
    assert_eq!(after_file, before_file, "and the volume file is byte-for-byte what it was");
    let state = call(&mut app, "grimoire/state", json!({})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(state["map"]["dirty"], json!(false), "grimoire_state says so too: {state}");
}

#[test]
fn a_map_with_no_volume_reports_an_empty_list() {
    let mut app = editor("volume_report_empty");
    let report = call(&mut app, "grimoire/volume_report", json!({"at": [[0.0, 0.0, 0.0]]})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report["entries"], json!([]), "no volume, no entries: {report}");
    assert_eq!(report["components"], json!([]), "{report}");
    assert_eq!(report["at"][0]["solid"], json!(false), "and the point asked is not solid: {report}");
}
