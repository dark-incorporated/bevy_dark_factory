//! **The editor's style sheet is live** (`crate::style`): an edit to any of its layers — the checkout's own
//! sheet, the person's `style.yaml`, the project's `editor_style.yaml` — reaches the running skin, the metrics
//! and the class rules within a few polls, with no rebuild; a file that does not read is refused naming the
//! field and the last good style stays; a file that is gone takes its layer with it.
//!
//! **Through the files, not the resources**, the way a person (or the editor-development agent) works: the
//! editor is built first, the files are written afterwards, and only what the editor shows is read back. The
//! clock is stepped (`TimeUpdateStrategy::ManualDuration`), so a poll is due on every update and nothing
//! waits on the wall clock.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::live::EditorReloads;
use bevy_gooey::GooeySkin;
use blueprint_model::live::Outcome;

use crate::fixtures;

/// One update's worth of real time: more than the style sheet's 0.1 s poll.
const STEP: Duration = Duration::from_millis(150);

/// Enough updates for a write to be seen, read, merged and cascaded.
const SETTLE_UPDATES: usize = 4;

fn settle(app: &mut App) {
    for _ in 0..SETTLE_UPDATES {
        app.update();
    }
}

/// A checkout as `editor_source` must name one: the crate's manifest and the shipped sheet, in a temp folder,
/// so the base layer can be edited without touching this repository.
fn fake_checkout(root: &Path) -> PathBuf {
    let checkout = root.join("checkout");
    let style = checkout.join("crates/bevy_dark_factory/src/style");
    std::fs::create_dir_all(&style).expect("a style folder");
    std::fs::write(checkout.join("crates/bevy_dark_factory/Cargo.toml"), "[package]\nname = \"bevy_dark_factory\"\n").expect("a manifest");
    std::fs::write(style.join("editor.yaml"), include_str!("../src/style/editor.yaml")).expect("the base sheet");
    checkout
}

struct Editor {
    app: App,
    checkout: PathBuf,
    user_style: PathBuf,
    project_style: PathBuf,
}

fn editor(name: &str) -> Editor {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let checkout = fake_checkout(&root);
    let config = root.join("user_config");
    std::fs::create_dir_all(&config).expect("a config folder");
    let settings = config.join("settings.yaml");
    std::fs::write(&settings, format!("version: 1\neditor_source: {}\n", checkout.display())).expect("a settings file");
    let user_style = config.join("style.yaml");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(STEP));
    app.insert_resource(bevy_dark_factory::settings::UserSettingsFile(settings));
    app.insert_resource(bevy_dark_factory::style::UserStyleFile(user_style.clone()));
    settle(&mut app);
    let project_style = app.world().resource::<bevy_dark_factory::project::Project>().project_dir.join(bevy_dark_factory::style::PROJECT_FILE);
    Editor { app, checkout, user_style, project_style }
}

fn gap(app: &App) -> f32 {
    app.world().resource::<GooeySkin>().gap
}

/// The `editor_style` verdict on the file whose name ends with `file`.
fn verdict(app: &App, file: &str) -> Option<(Outcome, Option<String>, String)> {
    app.world()
        .resource::<EditorReloads>()
        .kinds()
        .iter()
        .find(|k| k.kind == bevy_dark_factory::style::KIND && k.file.ends_with(file))
        .map(|k| (k.outcome, k.field.clone(), k.reason.clone()))
}

#[test]
fn a_users_style_layer_reaches_the_skin_is_refused_when_broken_and_goes_when_deleted() {
    let mut editor = editor("style_user");
    let base = bevy_dark_factory::style::compiled_default().skin.gap;
    assert_eq!(gap(&editor.app), base, "the running skin starts as the editor's own sheet");

    std::fs::write(&editor.user_style, "version: 1\nskin: {gap: 9.0}\n").expect("a style layer");
    settle(&mut editor.app);
    assert_eq!(gap(&editor.app), 9.0, "the person's layer is laid over the editor's sheet");
    assert_eq!(verdict(&editor.app, "style.yaml").map(|v| v.0), Some(Outcome::Applied));

    std::fs::write(&editor.user_style, "version: 1\nskin: {gapp: 1}\n").expect("a broken layer");
    settle(&mut editor.app);
    let (outcome, field, reason) = verdict(&editor.app, "style.yaml").expect("a verdict on the user's layer");
    assert_eq!(outcome, Outcome::Refused, "a key the skin does not have is refused: {reason}");
    assert!(
        field.as_deref().is_some_and(|f| f.contains("skin.gapp")) || reason.contains("gapp"),
        "the refusal names the key: field {field:?}, reason {reason}"
    );
    assert_eq!(gap(&editor.app), 9.0, "the last good layer stays");

    std::fs::remove_file(&editor.user_style).expect("the layer deleted");
    settle(&mut editor.app);
    assert_eq!(gap(&editor.app), base, "with the layer gone, the editor's own sheet is the skin again");
    assert_eq!(verdict(&editor.app, "style.yaml"), None, "a file that is gone is no longer refused");
}

/// **A layer of another `version` is refused**, one ahead and one from before the first, naming the field: the
/// `editor_style` kind is versioned by its own type (`bevy_dark_factory::style::STYLE_VERSION`).
#[test]
fn a_style_layer_of_another_version_is_refused() {
    for version in [0, bevy_dark_factory::style::STYLE_VERSION + 1] {
        let refused = bevy_dark_factory::style::read_layer("style.yaml", &format!("version: {version}\nskin: {{gap: 4.0}}\n"))
            .expect_err("another version is refused");
        assert_eq!(refused.kind, "editor_style");
        assert_eq!(refused.field.as_deref(), Some("version"), "{}", refused.sentence());
    }
    assert!(bevy_dark_factory::style::read_layer("style.yaml", "skin: {gap: 4.0}\n").is_ok(), "a layer that states no version is this one");
}

#[test]
fn an_edit_to_the_checkouts_own_sheet_reaches_the_running_editor() {
    let mut editor = editor("style_base");
    let sheet = editor.checkout.join("crates/bevy_dark_factory/src/style/editor.yaml");
    let text = std::fs::read_to_string(&sheet).expect("the sheet");
    std::fs::write(&sheet, text.replace("  gap: 5.0\n", "  gap: 7.0\n")).expect("an edited sheet");
    settle(&mut editor.app);
    assert_eq!(gap(&editor.app), 7.0, "the checkout's sheet is read live: no rebuild");

    std::fs::write(&sheet, text.replace("body: 11.0, heading", "body: 14.0, heading")).expect("a metric edited");
    settle(&mut editor.app);
    assert_eq!(bevy_dark_factory::style::metrics().r#type.body, 14.0, "a metric edited in the sheet is the running metric");
    assert_eq!(gap(&editor.app), 5.0, "and the gap is the file's again");
}

#[test]
fn a_projects_rule_styles_a_classed_node_live() {
    let mut editor = editor("style_project");
    let card = editor.app.world_mut().spawn((bevy_gooey::panel(), bevy_gooey::StyleClass::of("test-card"))).id();
    editor.app.world_mut().spawn(bevy_gooey::screen()).add_child(card);
    settle(&mut editor.app);
    std::fs::write(&editor.project_style, "version: 1\nrules:\n  \".test-card\": {padding: !Px 13}\n").expect("a project layer");
    settle(&mut editor.app);
    let padding = editor.app.world().get::<Node>(card).expect("a node").padding;
    assert_eq!(padding.left, Val::Px(13.0), "the project's rule pads the classed node");
    assert_eq!(verdict(&editor.app, "editor_style.yaml").map(|v| v.0), Some(Outcome::Applied));

    let lines = editor.app.world().resource::<bevy_dark_factory::style::StyleLines>();
    let line = lines.lines.iter().find(|((_, selector), _)| selector == ".test-card").map(|(_, line)| *line);
    assert_eq!(line, Some(3), "the inspector can say which line wrote the rule");
}

/// **A type size in the person's layer resizes a label already on screen**, without a respawn: the label keeps its
/// role, and the role reads the sheet's size now.
#[test]
fn a_type_size_in_a_users_layer_resizes_a_label_where_it_stands() {
    let mut editor = editor("style_type");
    let label = editor.app.world_mut().spawn((Text::new("body"), bevy_dark_factory::chrome::typed(bevy_dark_factory::chrome::text::BODY))).id();
    settle(&mut editor.app);
    let size = |app: &App| app.world().get::<TextFont>(label).expect("a font").font_size;
    let body = || bevy_dark_factory::chrome::font(bevy_dark_factory::chrome::text::BODY).font_size;
    let shipped = size(&editor.app);
    assert_eq!(shipped, body(), "the shipped body size");

    std::fs::write(&editor.user_style, "version: 1\nmetrics: {type: {body: 14.0}}\n").expect("a user layer");
    settle(&mut editor.app);
    assert_eq!(bevy_dark_factory::style::metrics().r#type.body, 14.0, "the layer's body size is the running one");
    assert_eq!(size(&editor.app), body(), "the same label, at the layer's body size");
    assert_ne!(size(&editor.app), shipped, "which is not the size it was spawned at");
}

/// How many `RebuildPanels` the editor has asked for.
#[derive(Resource, Default)]
struct Rebuilds(usize);

fn count_rebuilds(mut asked: MessageReader<bevy_dark_factory::style::RebuildPanels>, mut rebuilds: ResMut<Rebuilds>) {
    rebuilds.0 += asked.read().count();
}

/// **A measure read where panels are spawned asks for the door to be rebuilt; a type size does not**: a dock's
/// height is read once, when the dock is spawned, so only a rebuild shows its edit, while a label resizes in place.
#[test]
fn a_layout_measure_asks_for_a_rebuild_and_a_type_size_does_not() {
    let mut editor = editor("style_rebuild");
    editor.app.init_resource::<Rebuilds>();
    editor.app.add_systems(Update, count_rebuilds.after(bevy_dark_factory::style::apply_style));
    settle(&mut editor.app);

    std::fs::write(&editor.user_style, "version: 1\nmetrics: {type: {body: 14.0}}\n").expect("a type size");
    settle(&mut editor.app);
    assert_eq!(editor.app.world().resource::<Rebuilds>().0, 0, "a type size resizes labels where they stand");

    std::fs::write(&editor.user_style, "version: 1\nmetrics: {type: {body: 14.0}, layout: {bottom_dock_h: 200.0}}\n").expect("a dock height");
    settle(&mut editor.app);
    assert_eq!(bevy_dark_factory::style::metrics().layout.bottom_dock_h, 200.0, "the running measure is the layer's");
    assert_eq!(editor.app.world().resource::<Rebuilds>().0, 1, "and the door is rebuilt once to show it");
}
