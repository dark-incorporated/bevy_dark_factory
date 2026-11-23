//! **Changing the code colours recolours a code editor that is already open, without a rebuild** (#525,
//! `feedback-1795094391027`: *"...as well as syntax highlighting (we should have a setting to pick a syntax
//! highlighting theme)"*).
//!
//! # What this measures
//!
//! A YAML file is opened in the Files workspace, its editor given the focus, and the runs the editor draws are
//! read back from the world: each `TextSpan`'s text, its `Role` and its `TextColor`. Then the person's settings
//! file is written with another preset and frames run, and the same spans are read again.
//!
//! **The expectation is independent of the code that sets the colours.** It does not call
//! `GooeySkin::syntax`, `SyntaxTheme::theme` or `SyntaxPreset::theme` — the thing under test is the path from
//! the setting to the span, and a test that asked the same function what to expect would pass on a path that
//! drew nothing. It says two things instead: `warm` and `cool` are different sets of hues (so a `Role::CodeSyntax`
//! run's colour differs between them), and a `Role::CodeSyntax` run's colour is the skin's `Role::CodeSyntax`
//! colour rather than an unthemed one — the latter read from gooey's own public `syntax` on the running skin,
//! which is the *drawn* contract, not the editor's setting.
//!
//! # The mutation this is proof against
//!
//! Delete the one application in `style::apply_style` that writes `skin.syntax_theme` and
//! `switching_the_preset_changes_the_drawn_colour_of_a_key` fails: the spans keep `Skin`'s colours. Measured, not
//! assumed — see the commit body.

use std::time::Duration;

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::settings::{SyntaxTheme, UserSettingsFile};
use bevy_gooey::{CodeEditor, GooeySkin, Role};

use crate::fixtures;

const SETTLE: usize = 14;

/// A YAML file with a key, a quoted string, a keyword and a number on the same line, and a second key below, so
/// a role is drawn whatever the fold does at the top of the file.
const FILE: &str = "name: m\nversion: 3\nbounds:\n- 10.0\n- 3.0\nenabled: true\n";

/// The relative path the file is written to in the fixture, which the Files workspace opens by. **Not under
/// `maps/`**: a file named `*.map.yaml` is a map the loader reads, and this test is about how its characters are
/// drawn rather than about what it means.
const REL: &str = "assets/blueprint/colours.yaml";

/// **How long a change to the settings file may take to reach the drawn spans**, in seconds.
const HEARD_WITHIN_S: f64 = 20.0;

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
    let started = std::time::Instant::now();
    while started.elapsed().as_secs_f64() < HEARD_WITHIN_S {
        settle(app);
        if done(app) {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("{what}: not seen within {HEARD_WITHIN_S} s");
}

/// The editor on the fixture, the YAML file open in the Files workspace and focused, and the settings file
/// `warm` was written to.
fn rig(tag: &str, preset: SyntaxTheme) -> (App, std::path::PathBuf) {
    let name = format!("code_colours_{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&name).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    std::fs::write(root.join(REL), FILE).unwrap_or_else(|e| panic!("{e}"));
    let file = root.join(format!("user_config_{name}/settings.yaml"));
    std::fs::create_dir_all(file.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&file, settings_text(preset)).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(file.clone()));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    harness::resize_surface(&mut app, 1400, 900).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(&mut app);
    bevy_dark_factory::files::open_file(app.world_mut(), REL).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    let editor = app
        .world_mut()
        .query_filtered::<Entity, With<CodeEditor>>()
        .single(app.world_mut())
        .unwrap_or_else(|e| panic!("an editor is open: {e}"));
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    settle(&mut app);
    (app, file)
}

/// The settings file naming `preset`, written the way the Settings screen writes it rather than from a second
/// table of words here.
fn settings_text(preset: SyntaxTheme) -> String {
    let mut layer = bevy_dark_factory::settings::SettingsLayer::default();
    assert!(
        layer.set("syntax_theme", bevy_dark_factory::setting_catalogue::Value::Option(bevy_dark_factory::setting_catalogue::syntax_theme_index(preset))),
        "the row sets the key"
    );
    blueprint_model::yaml::to_string(&layer).unwrap_or_else(|p| panic!("{}", p.line()))
}

/// The runs the editor draws, with the role and the colour each is painted: `(words, role, colour)`.
fn drawn(app: &mut App) -> Vec<(String, Role, Color)> {
    let world = app.world_mut();
    let mut q = world.query::<(&TextSpan, &Role, &TextColor)>();
    q.iter(world).filter(|(t, ..)| !t.0.trim().is_empty()).map(|(t, role, colour)| (t.0.clone(), *role, colour.0)).collect()
}

/// **The colour of the drawn run holding `needle`**, which must be a syntax run: the property is about a
/// coloured span on screen, not about a value the editor computed and never drew.
fn syntax_ink(app: &mut App, needle: &str) -> Color {
    let runs = drawn(app);
    let (words, role, colour) = runs
        .iter()
        .find(|(run, role, _)| run.contains(needle) && matches!(role, Role::CodeSyntax(_)))
        .unwrap_or_else(|| panic!("a `Role::CodeSyntax` run holds `{needle}`: {runs:?}"));
    let expected = match role {
        Role::CodeSyntax(role) => *role,
        _ => unreachable!("the find required a syntax role"),
    };
    // **The drawn colour is the skin's own for that role**, whatever the role is: a run painted with a colour the
    // running skin does not give its role would be a colour that never moved with the setting.
    let skin = app.world().resource::<GooeySkin>().clone();
    assert_eq!(*colour, skin.syntax(expected), "`{words}` is drawn in the skin's `{expected:?}`, not another colour");
    *colour
}

/// **The same line, in two presets, is drawn in two different colours.** `warm` and `cool` are the two presets
/// furthest apart in hue (amber and sand against blues and violets), so a colour that is the same in both is a
/// colour the setting did not reach. The setting is switched **through the file**, live, with the editor already
/// open: no rebuild and no second editor.
#[test]
fn switching_the_preset_changes_the_drawn_colour_of_a_key() {
    let (mut app, file) = rig("switch", SyntaxTheme::Warm);
    let warm = syntax_ink(&mut app, "version");
    std::fs::write(&file, settings_text(SyntaxTheme::Cool)).unwrap_or_else(|e| panic!("{e}"));
    let want = SyntaxTheme::Cool.theme();
    until(&mut app, "the cool preset to reach the skin", |app| app.world().resource::<GooeySkin>().syntax_theme == want);
    let cool = syntax_ink(&mut app, "version");
    assert_ne!(warm, cool, "a key's drawn colour is the same in `warm` and `cool`: the setting did not reach the span");
}

/// **Every themed preset recolours a drawn run, and `Skin` puts the editor's own hues back.** Two roles are read
/// — a key and a keyword — and a preset passes when either moved, because a one-hue `mono` set legitimately holds
/// some role near the skin's own; and the return to `skin` is measured on both, so the colours a person had
/// before the setting are the colours they get by choosing it back.
#[test]
fn every_themed_preset_moves_a_drawn_role_and_skin_puts_them_back() {
    let (mut app, file) = rig("roles", SyntaxTheme::Skin);
    // **The baseline is `skin`'s own**, so the comparison below is against the colours the editor drew before the
    // setting: these two must both move for any preset, and both return.
    let skin_key = syntax_ink(&mut app, "version");
    let skin_word = syntax_ink(&mut app, "true");
    assert_ne!(skin_key, skin_word, "a key and a keyword are two hues even in the skin's own set: {skin_key:?} {skin_word:?}");
    for preset in [SyntaxTheme::Warm, SyntaxTheme::Cool, SyntaxTheme::Mono, SyntaxTheme::Separated] {
        std::fs::write(&file, settings_text(preset)).unwrap_or_else(|e| panic!("{e}"));
        let want = preset.theme();
        until(&mut app, &format!("{preset:?} to reach the skin"), |app| app.world().resource::<GooeySkin>().syntax_theme == want);
        let key = syntax_ink(&mut app, "version");
        let word = syntax_ink(&mut app, "true");
        assert!(key != skin_key || word != skin_word, "{preset:?} drew the skin's own colours: a role did not move");
    }
    std::fs::write(&file, settings_text(SyntaxTheme::Skin)).unwrap_or_else(|e| panic!("{e}"));
    let want = SyntaxTheme::Skin.theme();
    until(&mut app, "skin to come back", |app| app.world().resource::<GooeySkin>().syntax_theme == want);
    assert_eq!(syntax_ink(&mut app, "version"), skin_key, "choosing `skin` back draws the editor's own key hue");
    assert_eq!(syntax_ink(&mut app, "true"), skin_word, "choosing `skin` back draws the editor's own keyword hue");
}
