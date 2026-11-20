//! **Bake checks the project against itself, not each file against its own YAML** (#512).
//!
//! `bake` proved every file survived its own round trip and nothing more, so a map could name a family
//! crate the game was never built with and the release shipped — a brazier with no `bevy_lights` is a
//! brazier that is silently unlit, and the failure only appears in a build. This is the check the issue
//! asks for: the project's *data* names things, and every one of them must exist in the project.
//!
//! Four facts, each a different silent failure:
//!
//! - a map's `family: {bevy_lights: …}` naming a crate `family.yaml` omits is refused, naming the map,
//!   the placement and the crate;
//! - a project whose data names only listed crates bakes exactly as it did before — the regression that
//!   matters, because a check that fires on a good project is a check nobody keeps;
//! - `bake --needs` prints the family crates the data requires, so a release script can compare them
//!   with the game's manifest;
//! - a stamp naming a composition the project does not define is refused, naming the map and the id.

use std::path::{Path, PathBuf};

use blueprint_model::bake::{self, Needs};

/// A scratch folder of this test's own, named for it and this process.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("blueprint-needs-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{dir:?}: {e}"));
    dir
}

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("a parent")).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    std::fs::write(&path, text).unwrap_or_else(|e| panic!("{path:?}: {e}"));
}

/// **The smallest project that bakes**: a kit with one piece, and a map that places it.
///
/// The library and the map are the two files every check here reads, and keeping the fixture to them
/// means a refusal is about the fact under test and not about some other file's shape.
fn a_project(root: &Path) {
    write(
        root,
        "assets/blueprint/library.yaml",
        r#"version: 1
descriptors:
  - id: brazier
    mesh: null
    extent:
      footprint: [0.5, 0.5]
      height: 1.0
"#,
    );
}

fn a_map(root: &Path, name: &str, body: &str) {
    write(
        root,
        &format!("assets/blueprint/maps/{name}.map.yaml"),
        &format!(
            r#"version: 3
name: {name}
origin: [0.0, 0.0, 0.0]
bounds: [8.0, 3.0, 8.0]
bash: null
{body}
note: null
"#
        ),
    );
}

fn a_family(root: &Path, crates: &[&str]) {
    let listed: Vec<String> = crates.iter().map(|c| format!("  - {c}\n")).collect();
    write(root, "family.yaml", &format!("version: 1\ncrates:\n{}", listed.concat()));
}

/// **A map naming a family crate the project does not list is refused by `bake`**, naming all three.
#[test]
fn a_map_naming_a_family_crate_the_project_omits_is_refused() {
    let root = scratch("omits").join("project");
    a_project(&root);
    a_family(&root, &[]);
    a_map(
        &root,
        "yard",
        "placements:\n  - id: lamp\n    descriptor: brazier\n    at: [1.0, 1.0]\n    yaw: 0.0\n    \
         family:\n      bevy_lights: 'Flicker { hz: 2.0 }'\n",
    );

    let out = scratch("omits-bake");
    let problem = bake::bake(&root, &out).expect_err("a crate nothing lists was admitted");
    let line = problem.line();
    assert!(line.contains("yard"), "the refusal does not name the map: {line}");
    assert!(line.contains("lamp"), "the refusal does not name the placement: {line}");
    assert!(line.contains("bevy_lights"), "the refusal does not name the crate: {line}");
    // **A `Problem` with a `where` and a `fix`**, so `report.rs` and the MCP reader show it like every
    // other refusal rather than as a bare sentence.
    let json = problem.to_json();
    assert!(json.get("where").is_some(), "a refusal with no location: {json}");
    assert!(json.get("fix").is_some(), "a refusal with no fix: {json}");
    // **And nothing was written**: a refused bake leaves the folder as it was.
    assert!(!out.join("manifest.json").exists(), "a refused bake wrote a manifest");
}

/// **A project whose data names only listed crates bakes exactly as it did before.**
#[test]
fn a_project_that_names_only_listed_crates_bakes_unchanged() {
    let root = scratch("listed").join("project");
    a_project(&root);
    a_family(&root, &["bevy_lights"]);
    a_map(
        &root,
        "yard",
        "placements:\n  - id: lamp\n    descriptor: brazier\n    at: [1.0, 1.0]\n    yaw: 0.0\n    \
         family:\n      bevy_lights: 'Flicker { hz: 2.0 }'\n",
    );

    let out = scratch("listed-bake");
    let report = bake::bake(&root, &out).expect("a good project was refused");
    assert!(report.files > 0, "nothing was baked");
    // **And what it printed is the same requirement it checks against**, so a release script reading
    // `--needs` and a bake refusing cannot disagree.
    let needs = bake::needs(&root).expect("the project's needs are readable");
    assert_eq!(needs.family_crates, vec!["bevy_lights".to_owned()]);
}

/// **A map whose `family` is absent needs nothing**, and a project with no `family.yaml` still bakes.
#[test]
fn a_project_with_no_family_data_needs_no_family_crate() {
    let root = scratch("none").join("project");
    a_project(&root);
    a_map(&root, "yard", "placements:\n  - id: crate\n    descriptor: brazier\n    at: [1.0, 1.0]\n    yaw: 0.0\n");

    let out = scratch("none-bake");
    bake::bake(&root, &out).expect("a project that names no crate was refused");
    let needs = bake::needs(&root).expect("readable");
    assert!(needs.family_crates.is_empty(), "a project naming no crate needs one: {needs:?}");
}

/// **`bake --needs` prints the family crates the data requires**, which is the half that cannot be
/// wrong — `bevy_dark_factory` cannot see the game's own `Cargo.toml`, so printing the requirement and
/// letting the release script compare it is the honest shape.
#[test]
fn needs_prints_every_family_crate_the_data_names_in_order() {
    let root = scratch("needs").join("project");
    a_project(&root);
    a_family(&root, &["bevy_lights", "bevy_water"]);
    a_map(
        &root,
        "yard",
        "placements:\n  - id: lamp\n    descriptor: brazier\n    at: [1.0, 1.0]\n    yaw: 0.0\n    \
         family:\n      bevy_water: 'Level { depth: 0.4 }'\n      bevy_lights: 'Flicker { hz: 2.0 }'\n",
    );
    a_map(
        &root,
        "hall",
        "placements:\n  - id: lamp2\n    descriptor: brazier\n    at: [1.0, 1.0]\n    yaw: 0.0\n    \
         family:\n      bevy_lights: 'Flicker { hz: 4.0 }'\n",
    );

    let needs: Needs = bake::needs(&root).expect("readable");
    // **Sorted and deduplicated**: two maps naming `bevy_lights` is one requirement, and the order is
    // the same on every machine so a manifest comparison is a comparison and not a coin toss.
    assert_eq!(needs.family_crates, vec!["bevy_lights".to_owned(), "bevy_water".to_owned()]);
    // **The file it came from is named**, so a person meeting the requirement knows where to look.
    assert_eq!(needs.files, 2, "both maps are read: {needs:?}");
}

/// **A stamp naming a composition the project does not define is refused**, naming the map and the id.
#[test]
fn a_stamp_of_a_composition_the_project_does_not_define_is_refused() {
    let root = scratch("stamp").join("project");
    a_project(&root);
    // A compositions file that defines `desk_only`, which the map below does not stamp.
    write(
        &root,
        "assets/blueprint/compositions.yaml",
        r#"version: 1
compositions:
  - id: desk_only
    members: []
"#,
    );
    a_map(
        &root,
        "yard",
        "placements: []\nstamps:\n  - id: chair_and_desk\n    of: missing_group\n    at: [0.0, 0.0]\n    yaw: 0.0\n",
    );

    let out = scratch("stamp-bake");
    let problem = bake::bake(&root, &out).expect_err("a stamp of nothing was admitted");
    let line = problem.line();
    assert!(line.contains("yard"), "the refusal does not name the map: {line}");
    assert!(line.contains("missing_group"), "the refusal does not name the missing id: {line}");
    assert!(!out.join("manifest.json").exists(), "a refused bake wrote a manifest");
}
