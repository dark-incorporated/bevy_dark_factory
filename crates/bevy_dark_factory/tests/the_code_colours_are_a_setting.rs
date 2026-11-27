//! **The code editor's colours are a setting, and the file says what the person chose** (#525,
//! `feedback-1795094391027`: *"...as well as syntax highlighting (we should have a setting to pick a syntax
//! highlighting theme)"*).
//!
//! `bevy_gooey` 0.58.0 gave `GooeySkin` a `syntax_theme` (its issue #116): the hues every `Role::CodeSyntax`
//! colour is derived from, with the contrast search still run, so a theme chooses hue and never whether the
//! bar is met. This is the editor's side of it: a person-level setting naming one of gooey's five presets.
//!
//! # What these assert
//!
//! - **It reads and writes as one of the five**, in the spelling the file uses (`warm`, `cool`, `mono`,
//!   `separated`, `skin`), through the same `SettingsLayer` every other key goes through, and a person's file
//!   laid over the project's resolves the way `theme` does.
//! - **A wrong value is refused, naming the field**, by `SettingsLayer::read` — not by a later stage, and not
//!   with the value silently dropped to the default.
//! - **It is a catalogue row** whose five names are the presets' own, in the order the enum declares them.
//!
//! **Through the file, not the resource**: a test that assigned `EditorSettings` would be overwritten by
//! `sync_settings`, so the two that read the running editor write the file and run frames.

use std::time::Duration;

use bevy::prelude::*;
use bevy_dark_factory::setting_catalogue::{self, Kind, Value};
use bevy_dark_factory::settings::{EditorSettings, SettingsLayer, SyntaxTheme, UserSettingsFile};
use bevy_gooey::GooeySkin;

use crate::fixtures;
use crate::settle_support::{self, Frames};




/// The pause between the settles [`until`] runs.
const PAUSE: Duration = Duration::from_millis(20);

/// **How long a change to the settings file may take to reach the running skin**, in settles [`PAUSE`] apart: at
/// least 20 s, while the settings index's sweep is about a second, so this is many times that and a failure is a
/// change never heard. Counted, not timed, so a starved host does not run it out early (#644).
const HEARD_WITHIN: Frames = Frames::covering(Duration::from_secs(20), PAUSE);

fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    for _ in 0..HEARD_WITHIN.0 {
        settle_support::settle(app);
        if done(app) {
            return;
        }
        std::thread::sleep(PAUSE);
    }
    panic!("{what}: not seen within {HEARD_WITHIN:?}");
}

/// **The settings file a layer would write for `theme`** — the screen's own write path, so the word the test
/// reads is the word the screen saves and not a second table here.
fn written(theme: SyntaxTheme) -> String {
    let mut layer = SettingsLayer::default();
    assert!(layer.set("syntax_theme", Value::Option(setting_catalogue::syntax_theme_index(theme))), "the row sets it");
    blueprint_model::yaml::to_string(&layer).unwrap_or_else(|p| panic!("{}", p.line()))
}

/// The word a written layer gives the key.
fn word_in(text: &str) -> String {
    text.lines().find_map(|line| line.trim().strip_prefix("syntax_theme: ")).unwrap_or_else(|| panic!("no `syntax_theme` in {text:?}")).trim().to_owned()
}

/// **Each of the five reads from the file, and a layer writes it back as the same word.**
#[test]
fn every_preset_reads_and_round_trips_through_the_file() {
    // **The five words a person may write**, which is also the five the screen writes.
    assert_eq!(SyntaxTheme::ALL.map(|theme| word_in(&written(theme))), ["skin", "warm", "cool", "mono", "separated"], "the file's five words");
    for want in SyntaxTheme::ALL {
        let word = word_in(&written(want));
        let text = format!("version: 1\nsyntax_theme: {word}\n");
        let layer = SettingsLayer::read("settings.yaml", &text).unwrap_or_else(|r| panic!("`{word}` reads: {}", r.sentence()));
        assert_eq!(layer.syntax_theme, Some(want), "`{word}` is {want:?}");
        // **The write is the read's own word**: the screen sets it by index, and the file it writes must read back as
        // the same variant, or a Reset would show a second answer.
        let back = written(want);
        assert_eq!(word_in(&back), word, "`{word}` writes as itself");
        let reread = SettingsLayer::read("settings.yaml", &back).unwrap_or_else(|r| panic!("{}", r.sentence()));
        assert_eq!(reread.syntax_theme, Some(want), "{want:?} round trips");
    }
}

/// **A value that is not one of the five is refused, naming the field and the file.** The refusal is the
/// `SettingsLayer::read` one every key goes through, so the editor's status line names both.
#[test]
fn a_wrong_value_is_refused_naming_the_field_and_the_file() {
    let refusal = SettingsLayer::read("user/settings.yaml", "version: 1\nsyntax_theme: neon\n")
        .expect_err("`neon` is not a preset");
    assert_eq!(refusal.file, "user/settings.yaml", "the refusal names the file: {}", refusal.sentence());
    assert_eq!(refusal.field.as_deref(), Some("syntax_theme"), "the refusal names the field: {}", refusal.sentence());
    assert!(refusal.reason.contains("neon"), "the refusal quotes the value it refused: {}", refusal.reason);
    // **And a number is not a name**: the key takes the five words, not an index a person would have to look up.
    let refusal = SettingsLayer::read("settings.yaml", "version: 1\nsyntax_theme: 2\n").expect_err("an index is not a preset");
    assert_eq!(refusal.field.as_deref(), Some("syntax_theme"), "an index is refused at the field too: {}", refusal.sentence());
}

/// **The person's file and the project's resolve the way `theme` does**: the project's wins, and a layer that
/// leaves the key out falls through to the one below.
#[test]
fn the_project_layer_wins_and_an_absent_key_falls_through() {
    let user = SettingsLayer::read("user.yaml", "version: 1\nsyntax_theme: warm\n").unwrap_or_else(|r| panic!("{}", r.sentence()));
    let project = SettingsLayer::read("project.yaml", "version: 1\nsyntax_theme: cool\n").unwrap_or_else(|r| panic!("{}", r.sentence()));
    assert_eq!(EditorSettings::resolve(Some(&user), Some(&project)).syntax_theme, SyntaxTheme::Cool, "the project's wins");
    let quiet = SettingsLayer::read("quiet.yaml", "version: 1\ntext_scale: 1.25\n").unwrap_or_else(|r| panic!("{}", r.sentence()));
    assert_eq!(EditorSettings::resolve(Some(&user), Some(&quiet)).syntax_theme, SyntaxTheme::Warm, "the person's stands when the project says nothing");
    assert_eq!(EditorSettings::default().syntax_theme, SyntaxTheme::Skin, "the shipped default is the colours the editor drew before the setting");
}

/// **It is a row on the Settings screen**: an Appearance choice whose five names are the presets', and whose
/// default is `Skin` — the colours a person already has, so nobody's editor moves until they choose.
#[test]
fn the_setting_is_a_row_whose_five_names_are_the_presets() {
    let setting = setting_catalogue::find("syntax_theme").unwrap_or_else(|| panic!("`syntax_theme` is a catalogue row"));
    assert_eq!(setting.category, "Appearance", "it sits with the other looks");
    let Kind::Options(names) = setting.kind else { panic!("it is a choice among five presets") };
    assert_eq!(names, setting_catalogue::SYNTAX_THEMES, "the row offers the presets' own names");
    assert_eq!(names.len(), SyntaxTheme::ALL.len(), "every preset is offered");
    assert_eq!(setting_catalogue::default_value("syntax_theme"), Some(Value::Option(0)), "`Skin` is first, the shipped default");
    for (at, preset) in SyntaxTheme::ALL.into_iter().enumerate() {
        assert_eq!(setting_catalogue::syntax_theme_index(preset), at, "{preset:?} sits where the row offers it");
        assert_eq!(names[at], format!("{preset:?}"), "the row's word is the variant's own: {preset:?}");
        assert!(!preset.meaning().is_empty(), "{preset:?} says what it does");
    }
    // **A layer sets and clears it**, so the screen's Reset works.
    let mut layer = SettingsLayer::default();
    assert!(layer.set("syntax_theme", Value::Option(2)));
    assert_eq!(layer.syntax_theme, Some(SyntaxTheme::Cool));
    layer.clear("syntax_theme");
    assert_eq!(layer.syntax_theme, None, "Reset leaves the key to the layer below");
}

/// **The stored setting is what the person chose, and applying it writes nothing back.** The editor is built,
/// a preset is written into the person's file, frames run until the running skin carries it; then the file on
/// disk is read again and must be the text that was written, byte for byte. A skin builder that wrote its own
/// choice into the settings file would pass "the skin moved" and fail here.
#[test]
fn the_stored_choice_is_the_persons_and_applying_it_writes_nothing_back() {
    let tag = format!("syntax_theme_stored_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    let user = root.join(format!("user_config_{tag}/settings.yaml"));
    std::fs::create_dir_all(user.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    let chosen = "version: 1\nsyntax_theme: warm\n";
    std::fs::write(&user, chosen).unwrap_or_else(|e| panic!("{e}"));
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user.clone()));
    let want = SyntaxTheme::Warm.theme();
    until(&mut app, "the person's choice to reach the skin", |app| app.world().resource::<GooeySkin>().syntax_theme == want);
    // The resource the editor resolved says the same thing.
    assert_eq!(app.world().resource::<EditorSettings>().syntax_theme, SyntaxTheme::Warm, "the setting resolved from the file");
    assert_eq!(std::fs::read_to_string(&user).unwrap_or_else(|e| panic!("{e}")), chosen, "applying the choice rewrote the person's file");
    // **Switch it again and the same holds**: the write-on-apply would show up on the second choice too.
    let chosen = "version: 1\nsyntax_theme: mono\n";
    std::fs::write(&user, chosen).unwrap_or_else(|e| panic!("{e}"));
    let want = SyntaxTheme::Mono.theme();
    until(&mut app, "the second choice to reach the skin", |app| app.world().resource::<GooeySkin>().syntax_theme == want);
    assert_eq!(std::fs::read_to_string(&user).unwrap_or_else(|e| panic!("{e}")), chosen, "the second choice rewrote the file");
}
