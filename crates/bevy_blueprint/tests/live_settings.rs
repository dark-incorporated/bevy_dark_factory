//! **A running game applies its project's settings files to the crates' resources**: one file per
//! settings resource, `<root>/family/<crate>/<TypeIdent>.ron`, read as the crate's own type through
//! reflection. Every file is applied on the first frame, an edit is applied once it settles, a deleted
//! file puts the default back, and a file that does not read leaves the previous settings.
//!
//! The settings type here is this test binary's own, so its crate is `live_settings`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_blueprint::live::BlueprintLivePlugin;
use serde::{Deserialize, Serialize};

#[derive(Resource, Reflect, Default, Serialize, Deserialize, Clone, Debug, PartialEq)]
#[reflect(Resource, Default, Serialize, Deserialize)]
#[serde(default)]
struct HornSettings {
    volume: f32,
    honks: u32,
    tunes: Vec<String>,
}

fn root(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("blueprint_live_settings_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(file(&root).parent().unwrap_or(&root)).unwrap();
    std::fs::create_dir_all(root.join("assets/blueprint")).unwrap();
    root
}

fn file(root: &Path) -> PathBuf {
    blueprint_model::family::settings_path(root, "live_settings", "HornSettings")
}

fn app(root: &Path) -> App {
    let mut app = App::new();
    // The live plugin's map reload reads the `AssetServer`, as in a game.
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .register_type::<HornSettings>()
        .add_plugins(BlueprintLivePlugin { dir: root.join("assets/blueprint"), maps: Vec::new(), root: root.to_path_buf() })
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(600)));
    app.update();
    app
}

/// Writes the file and steps frames past the watch's settle time.
fn save(app: &mut App, root: &Path, text: Option<&str>) {
    std::thread::sleep(Duration::from_millis(20));
    match text {
        Some(text) => std::fs::write(file(root), text).unwrap(),
        None => std::fs::remove_file(file(root)).unwrap(),
    }
    for _ in 0..8 {
        app.update();
    }
}

fn horn(app: &App) -> Option<HornSettings> {
    app.world().get_resource::<HornSettings>().cloned()
}

#[test]
fn a_settings_file_present_at_start_is_applied_on_the_first_frame() {
    let root = root("start");
    std::fs::write(file(&root), "(volume: 0.5)").unwrap();
    let app = app(&root);
    assert_eq!(horn(&app), Some(HornSettings { volume: 0.5, honks: 0, tunes: Vec::new() }), "the missing field is the type's default");
}

#[test]
fn an_edit_is_applied_a_bad_one_keeps_the_last_good_and_a_deleted_file_is_the_default() {
    let root = root("edit");
    std::fs::write(file(&root), "(volume: 0.5)").unwrap();
    let mut app = app(&root);

    save(&mut app, &root, Some("(volume: 0.25, honks: 3)"));
    assert_eq!(horn(&app), Some(HornSettings { volume: 0.25, honks: 3, tunes: Vec::new() }));

    save(&mut app, &root, Some("(volume: \"loud\")"));
    assert_eq!(horn(&app), Some(HornSettings { volume: 0.25, honks: 3, tunes: Vec::new() }), "a value of the wrong type is refused");
    save(&mut app, &root, Some("(volume: 0."));
    assert_eq!(horn(&app), Some(HornSettings { volume: 0.25, honks: 3, tunes: Vec::new() }), "a file cut off is refused");

    save(&mut app, &root, None);
    assert_eq!(horn(&app), Some(HornSettings::default()), "no file: the crate's default");
}

#[test]
fn a_file_no_registered_settings_type_reads_is_left_alone() {
    let root = root("data");
    let data = root.join("family/live_settings/Script.ron");
    std::fs::write(&data, "(lines: [\"hello\"])").unwrap();
    let app = app(&root);
    assert_eq!(horn(&app), None, "nothing was inserted for a data file");
    assert_eq!(std::fs::read_to_string(&data).unwrap(), "(lines: [\"hello\"])");
}

/// The file is the whole value: a list the file shortens is that short, not the running list with its
/// first elements replaced.
#[test]
fn a_list_the_file_shortens_is_that_short() {
    let root = root("list");
    std::fs::write(file(&root), "(tunes: [\"a\", \"b\", \"c\"])").unwrap();
    let mut app = app(&root);
    save(&mut app, &root, Some("(tunes: [\"z\"])"));
    assert_eq!(horn(&app).map(|h| h.tunes), Some(vec!["z".to_owned()]));
}
