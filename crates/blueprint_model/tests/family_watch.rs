//! **A family settings file changing on disk is noticed** (#96's hot reload).
//!
//! `FamilyWatch` has no clock and makes no look of its own (#538): the editor's host calls `check` when the project index says the family
//! folder moved, and the index's debounce is what takes a change as settled, so the "within a second" of the name is the index's promise
//! (see `family_settings_reload.rs` in the editor, which waits on the wall clock for it).

use blueprint_model::family::{FamilyWatch, SettingsChange};

#[test]
fn an_edit_is_reported_once_by_crate_within_a_second_and_a_bad_edit_keeps_the_good_settings() {
    let root = std::env::temp_dir().join(format!("family_watch_{}", std::process::id()));
    let dir = root.join("family");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bevy_elements.yaml"), "sun: {hour: 6.0}").unwrap();
    let mut w = FamilyWatch::new(&root);
    assert!(w.check().is_empty(), "an untouched file is no change");

    std::fs::write(dir.join("bevy_elements.yaml"), "sun: {hour: 18.5}").unwrap();
    let got = w.check();
    assert_eq!(got, vec![SettingsChange::Reloaded { krate: "bevy_elements".into(), text: "sun: {hour: 18.5}".into() }]);
    assert!(w.check().is_empty(), "reported once");

    std::fs::write(dir.join("bevy_elements.yaml"), "sun: {hour: ").unwrap();
    let got = w.check();
    assert!(matches!(got.as_slice(), [SettingsChange::Invalid { krate, .. }] if krate == "bevy_elements"), "{got:?}");

    // Restoring the last good text is not a change: the good settings were never replaced.
    std::fs::write(dir.join("bevy_elements.yaml"), "sun: {hour: 18.5}").unwrap();
    assert!(w.check().is_empty());

    std::fs::remove_file(dir.join("bevy_elements.yaml")).unwrap();
    let got = w.check();
    assert_eq!(got, vec![SettingsChange::Removed { krate: "bevy_elements".into() }]);
    let _ = std::fs::remove_dir_all(&root);
}
