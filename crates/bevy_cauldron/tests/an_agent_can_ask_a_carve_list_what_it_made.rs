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

/// **A file under the project's grimoire directory**, beside the volume one: a terrain, for the test
/// that asks about a named map's own ground.
fn write(app: &App, dir: &str, file: &str, text: &str) {
    let dir = app.world().resource::<Project>().grimoire_dir.join(dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    std::fs::write(dir.join(file), text).unwrap_or_else(|e| panic!("{dir:?}/{file}: {e}"));
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
    // **The value the report actually makes**, not merely one above zero: the gallery runs along `x`
    // through the column's whole 1 m `x` and `z` footprint, so what it took is a 1 m block and the
    // thinnest of those extents is 1 m.
    assert!(
        cut["overlapping_m"].as_f64().is_some_and(|m| (m - 1.0).abs() < 1e-3),
        "the gallery takes a 1 m block out of the 1 m column, so 1.0 m went: {cut}"
    );
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

/// **A map name that is a path is refused, not answered with an empty list** (#473 review, item 1).
///
/// `volume_edit` refuses a name holding `/`, `\`, `.` or nothing, because the name arrives from an
/// untrusted caller. The report resolved the same raw string into `<grimoire_dir>/volume/<map>.yaml`
/// with no check and swallowed the parse failure, so `map: "../../some-file"` read a path outside
/// `volume/` and answered "this map carves nothing" instead of the refusal the edit path gives.
#[test]
fn a_map_name_that_is_a_path_is_refused_by_the_report_too() {
    let mut app = editor("volume_report_bad_name");
    write_lair(&app, lair());
    for bad in ["../../some-file", "a/b", "a\\b", "", "m.yaml"] {
        let refused = call(&mut app, "grimoire/volume_report", json!({"map": bad}))
            .expect_err(&format!("`{bad}` is not a map name, so the report must refuse it"));
        assert!(
            refused.contains("is not a map name"),
            "**the refusal is the edit path's own sentence**, not an empty report: `{bad}`: {refused}"
        );
        assert!(refused.contains("Nothing was written"), "and it says nothing was written: {refused}");
    }
    // The map opened for this fixture is still reported on by name, so the guard refuses paths and
    // not the name the edit path writes.
    let good = call(&mut app, "grimoire/volume_report", json!({"map": "m"})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(good["entries"].as_array().map(Vec::len), Some(5), "a map name still reports: {good}");
}

/// **Every terrain here is flat rock at `height_m`**, a 4x4 field about the map's own origin: enough
/// for `height_at` to answer everywhere a point is asked about.
fn flat_ground(height_m: f32) -> String {
    let heights = vec![height_m.to_string(); 16].join(", ");
    format!("{{origin: [0.0, 0.0], cell_m: 1.0, width: 4, depth: 4, heights: [{heights}]}}")
}

/// **A named map is reported over its own ground, not the open map's** (#473 review, item 2).
///
/// The volume came from `p.map` while the ground came from `ground_of(project, open)`, which derives
/// the heightfield from the open map's path. With `map: "other"` while `m` is open, the report measured
/// `other`'s edits against `m`'s terrain. Two maps at different heights prove it: one point is rock
/// on the one ground and air on the other.
#[test]
fn a_named_map_is_reported_over_its_own_ground() {
    let mut app = editor("volume_report_own_ground");
    // The open map sits on rock at 10 m; the map asked about sits on rock at 20 m.
    write(&app, "terrain", "m.yaml", &flat_ground(10.0));
    write(&app, "terrain", "other.yaml", &flat_ground(20.0));
    // No volume for `other`: the ground alone makes solid, which is exactly what is under test.
    let point = [1.0, 15.0, 1.0];
    let report = call(&mut app, "grimoire/volume_report", json!({"map": "other", "at": [point]}))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(report["map"], json!("other"), "it answered for the map asked about: {report}");
    assert_eq!(
        report["at"][0]["solid"],
        json!(true),
        "**15 m is rock in `other`, whose ground is at 20 m**, so the report read `other`'s terrain \
         rather than the open map's (which would say air at 15 m): {report}"
    );
    // The control: the same point over the open map's own ground, at 10 m, is air.
    let open = call(&mut app, "grimoire/volume_report", json!({"at": [point]})).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(open["map"], json!("m"), "and the unnamed call is the open map: {open}");
    assert_eq!(open["at"][0]["solid"], json!(false), "15 m is above the open map's 10 m ground: {open}");
}

/// **A carve list whose reach would hold the frame is refused, naming the limit** (#473 review, item 5).
#[test]
fn a_carve_list_too_large_to_flood_fill_is_refused_naming_the_limit() {
    let mut app = editor("volume_report_budget");
    let edit = |shape: Shape, mode: Mode| Edit { shape, mode, smooth_m: 0.0 };
    // One carve 60 m on every axis: ~14 M samples at the report's 0.25 m cell.
    write_lair(&app, vec![edit(Shape::Box { center: [0.0, 30.0, 0.0], half_m: [30.0; 3] }, Mode::Carve)]);
    let refused = call(&mut app, "grimoire/volume_report", json!({}))
        .expect_err("a flood fill that large would stall the editor, so the report is refused");
    let limit = bevy_grimoire_core::volume_report::MAX_VOID_CELLS;
    assert!(refused.contains(&limit.to_string()), "**the refusal names the limit**: {refused}");
    assert!(refused.contains("Nothing was changed"), "and that nothing was written: {refused}");
}
