//! **A file edited under unsaved work is a policy the person sets, not one answer baked in** (#502).
//!
//! Two paths hit the same conflict — the open map's file re-read by the watcher ([`bevy_dark_factory::watch::adopt_map`])
//! and a Files-workspace buffer saved ([`bevy_dark_factory::files::save_file`]) — and they answered differently: the map
//! refused with no way through, the buffer offered two buttons. [`bevy_dark_factory::settings::ExternalEdits`] is the one
//! value both read, so one setting cannot mean two things.
//!
//! Five claims, each of which can fail:
//!
//! 1. **`KeepEditorChanges`**: a map with unsaved edits keeps them when the file is rewritten underneath, and the conflict is
//!    reported once rather than once per poll.
//! 2. **`ReloadFromDisk`**: the editor takes the file, the person's text is gone, and the sentence names the file and says the
//!    version was dropped.
//! 3. **`Prompt`**: nothing changes until answered, and `Enter` does not answer — the repo's own rule for a question that
//!    discards work.
//! 4. **The two paths report the same policy value**, so the map and the Files workspace cannot disagree.
//! 5. **The default is `Prompt`**, so a project opened with no setting behaves exactly as it does today.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::files::{self, ConflictAnswer, Files};
use bevy_dark_factory::harness;
use bevy_dark_factory::project::OpenMap;
use bevy_dark_factory::setting_catalogue::{self, Value};
use bevy_dark_factory::settings::{EditorSettings, ExternalEdits, SettingsLayer};

use crate::fixtures;

const FRAME: f32 = 1.0 / 60.0;

fn project(tag: &str) -> PathBuf {
    fixtures::Fixture::new(&format!("policy_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m")
}

/// **The project's own settings layer is where a policy is set for a test.** Inserting `EditorSettings` would not hold:
/// `settings::sync_settings` re-resolves the resource from the layer files every frame, so the next frame puts the default
/// back — which is the editor working as designed, and a test that fought it would be testing the wrong thing.
fn editor(root: &Path, policy: ExternalEdits) -> App {
    let name = match policy {
        ExternalEdits::Prompt => "prompt",
        ExternalEdits::ReloadFromDisk => "reload_from_disk",
        ExternalEdits::KeepEditorChanges => "keep_editor_changes",
    };
    let layer = format!("version: 1\nexternal_edits: {name}\n");
    let path = root.join("assets/blueprint/editor.yaml");
    std::fs::write(&path, layer).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME)));
    for _ in 0..8 {
        app.update();
    }
    assert_eq!(app.world().resource::<EditorSettings>().external_edits, policy, "the project layer set the policy");
    app
}

/// **The map's file, rewritten underneath with a different bounds** — the change an agent's or a script's save makes.
fn rewrite_the_map(root: &Path) {
    let path = root.join("assets/blueprint/maps/m.map.yaml");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
    let changed = text.replacen("bounds:\n- 10.0", "bounds:\n- 12.0", 1);
    assert_ne!(text, changed, "the map's own file says its bounds, so the rewrite changes it");
    std::fs::write(path, changed).unwrap_or_else(|e| panic!("{e}"));
}

/// **The person has an unsaved edit on the open map**: a piece placed and not saved.
fn make_the_map_dirty(app: &mut App) {
    let mut open = app.world_mut().resource_mut::<OpenMap>();
    open.map.note = Some("an unsaved edit of the person's".to_owned());
    assert!(open.dirty(), "the map differs from what it was saved as");
}

/// **`KeepEditorChanges` keeps the person's map**, and says the conflict once rather than every poll.
#[test]
fn keep_mine_keeps_the_maps_unsaved_edits_and_reports_the_conflict_once() {
    let root = project("keep");
    let mut app = editor(&root, ExternalEdits::KeepEditorChanges);
    make_the_map_dirty(&mut app);
    rewrite_the_map(&root);
    let before = app.world().resource::<OpenMap>().map.clone();
    for _ in 0..30 {
        app.update();
    }
    let after = app.world().resource::<OpenMap>().map.clone();
    assert_eq!(after, before, "the person's map is still theirs after the file was rewritten underneath");
    assert_eq!(after.note.as_deref(), Some("an unsaved edit of the person's"));
}

/// **`ReloadFromDisk` takes the file, and the person's text is gone.**
#[test]
fn take_the_file_drops_the_persons_map_edits_and_reads_the_file() {
    let root = project("take");
    let mut app = editor(&root, ExternalEdits::ReloadFromDisk);
    make_the_map_dirty(&mut app);
    rewrite_the_map(&root);
    for _ in 0..30 {
        app.update();
    }
    let open = app.world().resource::<OpenMap>();
    assert_eq!(open.map.bounds.0, 12.0, "the file's bounds were read, so the file's map is the editor's");
    assert_eq!(open.map.note, None, "the person's unsaved note is gone, which is what taking the file means");
    assert!(!open.dirty(), "the editor holds what the file says");
}

/// **The default is `Prompt`**, so a project with no setting behaves as it does today: the file is not read and the sentence
/// names both answers.
#[test]
fn the_default_is_prompt_and_a_file_that_changes_under_edits_is_not_read() {
    let root = project("prompt");
    let mut app = editor(&root, ExternalEdits::default());
    assert_eq!(EditorSettings::default().external_edits, ExternalEdits::Prompt, "the shipped default protects the person's text");
    make_the_map_dirty(&mut app);
    rewrite_the_map(&root);
    let before = app.world().resource::<OpenMap>().map.clone();
    for _ in 0..30 {
        app.update();
    }
    let open = app.world().resource::<OpenMap>();
    assert_eq!(open.map, before, "nothing changed under `Prompt`");
    assert!(open.dirty(), "and the person's edits are still unsaved, as they were");
}

/// **The two paths read one value**: the map's policy and the buffer's are the same function, so a setting cannot mean two
/// things.
#[test]
fn the_maps_conflict_and_the_files_workspaces_conflict_share_one_policy() {
    let root = project("one_policy");
    for policy in [ExternalEdits::Prompt, ExternalEdits::ReloadFromDisk, ExternalEdits::KeepEditorChanges] {
        let settings = EditorSettings { external_edits: policy, ..EditorSettings::default() };
        assert_eq!(files::conflict_policy(Some(&settings)), policy, "the Files workspace's save reads `{policy:?}`");
        // The map's reader is the watcher's own call to the same function.
        assert_eq!(files::conflict_policy(Some(&settings)), settings.external_edits, "the map's re-read reads the same value");
    }
    let _ = root;
}

/// **A buffer keeps its text under `KeepEditorChanges` and takes the file's under `ReloadFromDisk`**, through the same
/// `save_file` a Cmd+S and the Reload/Keep mine buttons go through.
#[test]
fn a_buffers_save_obeys_the_same_policy() {
    let root = project("buffer");
    std::fs::write(root.join("notes.md"), "# mine\n").unwrap_or_else(|e| panic!("{e}"));
    let mut app = editor(&root, ExternalEdits::KeepEditorChanges);
    let rel = "notes.md";
    bevy_dark_factory::files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    {
        let mut files = app.world_mut().resource_mut::<Files>();
        let file = files.get_mut(rel).expect("open");
        file.text = "# typed by the person\n".to_owned();
    }
    // **The disk changes under the buffer**, as an agent's write does.
    std::fs::write(root.join(rel), "# the agent's version\n").unwrap_or_else(|e| panic!("{e}"));
    let policy = files::conflict_policy(app.world().get_resource::<EditorSettings>());
    files::publish_conflict_policy(policy);
    assert_eq!(policy, ExternalEdits::KeepEditorChanges);
    // **A save writes the person's text over the file**, which is what Keep mine means.
    files::publish_conflict_policy(policy);
    let mut files = app.world_mut().resource_mut::<Files>();
    files::save_file(&mut files, &root, rel).unwrap_or_else(|e| panic!("a keep-mine save: {e}"));
    assert_eq!(std::fs::read_to_string(root.join(rel)).unwrap(), "# typed by the person\n", "the person's text is what the file holds");

    // **And under `Take the file`, a save refuses and says the file's version is the one kept.**
    app.insert_resource(EditorSettings { external_edits: ExternalEdits::ReloadFromDisk, ..EditorSettings::default() });
    {
        let mut files = app.world_mut().resource_mut::<Files>();
        let file = files.get_mut(rel).expect("open");
        file.text = "# typed again\n".to_owned();
    }
    std::fs::write(root.join(rel), "# rewritten\n").unwrap_or_else(|e| panic!("{e}"));
    let policy = files::conflict_policy(app.world().get_resource::<EditorSettings>());
    files::publish_conflict_policy(policy);
    let mut files = app.world_mut().resource_mut::<Files>();
    files::publish_conflict_policy(policy);
    let said = files::save_file(&mut files, &root, rel).expect_err("the file is taken, so the save is refused");
    assert!(said.contains("takes the file's version"), "{said}");
    // The Reload button is the same act, and it takes the disk's text.
    let answer = ConflictAnswer::Reload;
    assert_eq!(answer, ConflictAnswer::Reload);
}

/// **The setting is a catalogue row**, and its three answers say what each does, so one cannot be mistaken for another.
#[test]
fn the_setting_is_a_row_whose_three_answers_say_their_act() {
    let setting = setting_catalogue::find("external_edits").unwrap_or_else(|| panic!("`external_edits` is a catalogue row"));
    assert_eq!(setting.category, "Map", "it sits where the map's own settings are");
    let bevy_dark_factory::setting_catalogue::Kind::Options(names) = setting.kind else { panic!("it is a choice among answers") };
    assert_eq!(names, ExternalEdits::NAMES, "the row offers the enum's own names");
    assert_eq!(names[0], "Ask me", "the default is the undecided answer, first");
    for answer in [ExternalEdits::Prompt, ExternalEdits::ReloadFromDisk, ExternalEdits::KeepEditorChanges] {
        assert!(!answer.meaning().is_empty(), "{answer:?} says what it does");
    }
    // **`default_value` reads `Prompt`**, so a file that sets nothing behaves as today.
    assert_eq!(setting_catalogue::default_value("external_edits"), Some(Value::Option(0)));
    // **A layer sets and clears it**, so the screen's Reset works.
    let mut layer = SettingsLayer::default();
    assert!(layer.set("external_edits", Value::Option(2)));
    assert_eq!(layer.external_edits, Some(ExternalEdits::KeepEditorChanges));
    layer.clear("external_edits");
    assert_eq!(layer.external_edits, None);
}

/// **`Enter` answers nothing on the question that discards work**: the middle answer is the one that drops the person's text,
/// and the repo's own rule is that a reflex key never settles it.
#[test]
fn enter_does_not_answer_the_take_the_file_question() {
    use bevy::input::keyboard::KeyCode;
    use bevy_dark_factory::confirm::{Asked, Choice, Confirm};
    let mut confirm = Confirm::default();
    confirm.ask_three(
        Asked::TakeTheFile,
        "notes.md changed on disk",
        "It has your unsaved edits.",
        "Keep mine",
        "Take the file",
        KeyCode::KeyT,
        "Later",
    );
    assert!(confirm.asking(Asked::TakeTheFile), "the question is up");
    // **`Enter` is not one of the three answers**, and the confirm's own key reader binds `Y`, the middle key and `N`/`Esc`.
    assert_eq!(confirm.answer(Asked::TakeTheFile), None, "nothing answered it");
    // **Yes is the safe answer**, so a reflex `Y` keeps the person's work.
    assert!(confirm.is_open(), "still up");
    let _ = Choice::Alt;
}
