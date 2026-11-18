//! **`Cmd+=` / `Cmd+-` step the whole interface's size, `Cmd+Shift+0` puts it back**
//! (`feedback-1794783912982`).
//!
//! > It seems like Cmd + "+" or "-" should change the layout size of the entire editor--like
//! > electron.
//!
//! Settings → Appearance has two scale levers — "Text size" and "Interface size" — and nothing on the
//! keyboard. A browser and an Electron app answer those two chords on every screen, so a hand reaches
//! for them here; `keys::BINDINGS` has no `Equal`, `Minus` or `NumpadAdd`/`NumpadSubtract` row at all.
//!
//! # The two levers are not the same chord's business
//!
//! The note says *"the layout size of the entire editor"*, and the editor's own words for that are
//! **Interface size**: `UiScale` multiplies every `Val::Px` and every font size, so 150% grows the
//! buttons, the spacing and the type together (`surface::ui_scale_for_a_person`). "Text size" moves
//! type alone and stays the Settings row's, which is what its own description says. So both chords
//! step **`interface_scale`**, and the Settings row and the chords are two callers of one write.
//!
//! # The steps are a list, not a multiply
//!
//! `100%, 110%, 125%, 150%, 175%, 200%` — a fixed ladder, so `Cmd+=` four times is 150% and not
//! `1.1^4 = 146.41%`, and a step up then a step down returns where it was.
//!
//! # Cmd+0 is taken, and the collision is settled rather than shadowed
//!
//! `Cmd+0` is **the tenth workspace, Design** (SG-05, #404), so the reset takes `Cmd+Shift+0`. The
//! second test presses both and asserts each does its own job.

use std::path::{Path, PathBuf};

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};

use bevy_dark_factory::setting_catalogue::INTERFACE_STEPS;
use bevy_dark_factory::settings::{SettingsLayer, UserSettingsFile};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// The platform's primary modifier, as the editor's own census means it (`keys::MOD_KEYS`).
fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

fn send(app: &mut App, key: KeyCode, state: ButtonState) {
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: Key::Unidentified(NativeKey::Unidentified),
        state,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
}

/// Hold `modifiers`, press `key` for one frame, let everything go.
fn chord(app: &mut App, modifiers: &[KeyCode], key: KeyCode) {
    let down: Vec<KeyCode> = modifiers.iter().copied().chain([key]).collect();
    for k in &down {
        send(app, *k, ButtonState::Pressed);
    }
    app.update();
    for k in &down {
        send(app, *k, ButtonState::Released);
    }
    for _ in 0..3 {
        app.update();
    }
}

/// **An editor with a window and a settings file of the test's own**, stepped until both have been read.
///
/// The window is what `surface::fit_surface_to_window` needs: the interface's `UiScale` is written
/// there and it returns without a `PrimaryWindow`, so a headless run with no window would show a
/// scale of never-updated 1.2 and could not tell 100% from 200%.
fn editor(tag: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(&format!("scale_chords_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let user = root.join(format!("user_config_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(user.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&user, "version: 1\n").expect("a settings file");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user.clone()));
    app.world_mut().spawn((
        Window { resolution: bevy::window::WindowResolution::new(1400, 900), ..default() },
        PrimaryWindow,
    ));
    for _ in 0..12 {
        app.update();
    }
    (app, user)
}

/// **The interface size the settings file now holds**: the value the Settings row would have written.
fn written_scale(user: &Path) -> Option<f32> {
    let text = std::fs::read_to_string(user).unwrap_or_default();
    SettingsLayer::read("test", &text).ok().and_then(|l| l.interface_scale)
}

/// **The interface size the running editor draws**, as the person's own multiple: `UiScale` over the
/// fit the window would otherwise have (`surface::ui_scale_for_a_person` with the person at 100%).
fn shown_scale(app: &App) -> f32 {
    let ui = app.world().resource::<UiScale>().0;
    let base = bevy_dark_factory::surface::ui_scale_for_a_person(1400.0, 1.0, 1.0);
    ui / base
}

/// **The two step chords walk the ladder up and down, and the reset returns to 100%.**
#[test]
fn the_step_chords_and_the_settings_row_write_one_value() {
    let (mut app, user) = editor("steps");
    assert_eq!(written_scale(&user), None, "the fixture's file states no interface size");
    assert!((shown_scale(&app) - 1.0).abs() < 1.0e-4, "and the editor draws 100%: {}", shown_scale(&app));

    // Up the ladder, one chord at a time. `=` arrives as `Equal` (the unshifted key, as Cmd+= does).
    for want in INTERFACE_STEPS.iter().skip(1) {
        chord(&mut app, &[primary()], KeyCode::Equal);
        let file = written_scale(&user).unwrap_or_else(|| panic!("the chord wrote no interface size"));
        assert!((file - want).abs() < 1.0e-4, "a step up writes {want}, and the file says {file}");
        let shown = shown_scale(&app);
        assert!((shown - want).abs() < 1.0e-3, "and the running editor draws {want}: it draws {shown}");
    }

    // The top is the top: one more chord stays at 200%.
    chord(&mut app, &[primary()], KeyCode::Equal);
    assert_eq!(written_scale(&user), Some(2.0), "200% is the last rung");

    // And down, one at a time, in reverse.
    for want in INTERFACE_STEPS.iter().rev().skip(1) {
        chord(&mut app, &[primary()], KeyCode::Minus);
        let file = written_scale(&user).unwrap_or_else(|| panic!("the chord wrote no interface size"));
        assert!((file - want).abs() < 1.0e-4, "a step down writes {want}, and the file says {file}");
        let shown = shown_scale(&app);
        assert!((shown - want).abs() < 1.0e-3, "and the editor follows down to {want}: it draws {shown}");
    }

    // A reset from anywhere returns to 100%.
    chord(&mut app, &[primary()], KeyCode::Equal);
    chord(&mut app, &[primary()], KeyCode::Equal);
    chord(&mut app, &[primary(), KeyCode::ShiftLeft], KeyCode::Digit0);
    assert_eq!(written_scale(&user), Some(1.0), "Cmd+Shift+0 resets to 100%");
    assert!((shown_scale(&app) - 1.0).abs() < 1.0e-3, "and the editor follows: {}", shown_scale(&app));
}

/// **The workspace chords keep working**: `Cmd+0` still opens Design, and it is the reset's own chord
/// (`Cmd+Shift+0`) that resets, so neither shadows the other.
#[test]
fn cmd_zero_is_still_the_design_workspace_and_the_reset_is_its_own_chord() {
    let (mut app, user) = editor("collision");
    let started = *app.world().resource::<Workspace>();

    chord(&mut app, &[primary()], KeyCode::Digit0);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Design, "Cmd+0 is the Design workspace");
    assert_ne!(started, Workspace::Design, "and the test started somewhere else, so that is a change");
    assert_eq!(written_scale(&user), None, "Cmd+0 is a door, not the reset: it wrote no size");

    chord(&mut app, &[primary()], KeyCode::Equal);
    assert_eq!(written_scale(&user), Some(1.1), "the step chord works from the Design workspace too");

    chord(&mut app, &[primary(), KeyCode::ShiftLeft], KeyCode::Digit0);
    assert_eq!(written_scale(&user), Some(1.0), "and Cmd+Shift+0 is the reset");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Design, "the reset does not leave the workspace");
}
