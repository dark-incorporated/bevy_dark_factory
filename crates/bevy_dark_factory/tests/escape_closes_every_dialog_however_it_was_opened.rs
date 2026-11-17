//! **Escape closes Settings, the keymap, project configuration and New project, however each was opened** (#446).
//!
//! Reported from a real window under Xvfb: with Settings open, Escape did nothing and only Close closed it. The earlier tests of this
//! (`modal_keys_do_not_reach_the_editor`, `a_new_project_form_opens_ready_to_type_in`) either opened the dialog by writing its message or sent gooey's
//! `Cancel` straight to the card, so they never walked the person's way: the click on the button that opens it, then a real key event.
//!
//! Each test here opens its dialog the way a person does (the button, the chord), with a primary window present as in the real editor, sends Escape as
//! `bevy_gooey::testing::press` does (a `KeyboardInput` on the window, folded into `ButtonInput` by Bevy's own input system), and reads whether the dialog is
//! still there after the frames ran.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_dark_factory::harness;
use bevy_dark_factory::keymap_screen::KeymapDialog;
use bevy_dark_factory::launcher::LauncherPlugin;
use bevy_dark_factory::new_project::FormCard;
use bevy_dark_factory::project_config::ProjectConfigDialog;
use bevy_dark_factory::screen::Screen;
use bevy_dark_factory::settings_screen::SettingsDialog;
use bevy_gooey::testing::{click, find_by_label, press};

use crate::fixtures;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// An editor with a primary window, as a real run has.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    updates(&mut app, 4);
    app
}

/// The same editor standing on the home, as the launcher's own tests put it there.
fn on_the_home(tag: &str) -> App {
    let mut app = editor(tag);
    app.add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    updates(&mut app, 6);
    app
}

fn count<C: Component>(app: &mut App) -> usize {
    app.world_mut().query_filtered::<(), With<C>>().iter(app.world()).count()
}

/// **A button pressed as a person presses it**: the pointer moves to it and clicks. With a window present the pointer is the window's cursor, which
/// `surface::retarget_pointer` reads every frame, so the cursor is put on the button first.
fn click_on(app: &mut App, label: &str) {
    let button = find_by_label(app, label).unwrap_or_else(|e| panic!("no `{label}` to click: {e:?}"));
    let centre = app.world().get::<UiGlobalTransform>(button).expect("a placed button").translation;
    let mut windows = app.world_mut().query_filtered::<&mut Window, With<PrimaryWindow>>();
    windows.single_mut(app.world_mut()).expect("a primary window").set_physical_cursor_position(Some(centre.as_dvec2()));
    updates(app, 2);
    click(app, button);
    updates(app, 6);
}

/// The primary modifier and one key, down together then up, each a real `KeyboardInput`.
fn chord(app: &mut App, key: KeyCode) {
    let primary = if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft };
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("a primary window");
    let event = |code: KeyCode, state: ButtonState| KeyboardInput { key_code: code, logical_key: Key::Unidentified(NativeKey::Unidentified), state, text: None, repeat: false, window };
    for (code, state) in [(primary, ButtonState::Pressed), (key, ButtonState::Pressed), (key, ButtonState::Released), (primary, ButtonState::Released)] {
        app.world_mut().write_message(event(code, state));
        app.update();
    }
    updates(app, 4);
}

/// **A dialog opened while the terminal has the keyboard still answers Escape** (the real window's case: the NightShift's terminal owns the keys until a click lands
/// elsewhere, and `terminal_pane::hold_the_keyboard` took `Escape` out of the frame for the shell, so gooey's `Cancel` never fired and only the Close button
/// worked). A modal dialog takes the keyboard from the shell while it is up.
#[test]
fn escape_closes_settings_and_the_keymap_while_the_terminal_has_the_keyboard() {
    use bevy_dark_factory::keymap_screen::OpenKeymap;
    use bevy_dark_factory::settings_screen::OpenSettings;
    use bevy_dark_factory::terminal_pane::TerminalFocus;
    let mut app = editor("esc_terminal_holds");
    for (what, closes) in [("Settings", 0usize), ("the keymap", 1)] {
        app.world_mut().resource_mut::<TerminalFocus>().take();
        updates(&mut app, 2);
        if closes == 0 {
            app.world_mut().write_message(OpenSettings);
        } else {
            app.world_mut().write_message(OpenKeymap);
        }
        updates(&mut app, 6);
        // The shell has the keyboard again after the dialog came up, as a click in its pane gives it.
        app.world_mut().resource_mut::<TerminalFocus>().take();
        updates(&mut app, 2);
        let open = |app: &mut App| if closes == 0 { count::<SettingsDialog>(app) } else { count::<KeymapDialog>(app) };
        assert_eq!(open(&mut app), 1, "{what} is open");
        press(&mut app, KeyCode::Escape);
        assert_eq!(open(&mut app), 0, "Escape closes {what} while the terminal has the keyboard");
    }
}

/// **Settings opened by the chord is closed by Escape.**
#[test]
fn escape_closes_settings_opened_by_the_chord() {
    let mut app = editor("esc_settings_chord");
    chord(&mut app, KeyCode::Comma);
    assert_eq!(count::<SettingsDialog>(&mut app), 1, "the chord opens Settings");
    press(&mut app, KeyCode::Escape);
    assert_eq!(count::<SettingsDialog>(&mut app), 0, "Escape closes Settings");
}

/// **Settings opened by the home's button is closed by Escape.**
#[test]
fn escape_closes_settings_opened_by_the_homes_button() {
    let mut app = on_the_home("esc_settings_button");
    click_on(&mut app, "Settings");
    assert_eq!(count::<SettingsDialog>(&mut app), 1, "the home's Settings button opens it");
    press(&mut app, KeyCode::Escape);
    assert_eq!(count::<SettingsDialog>(&mut app), 0, "Escape closes Settings");
}

/// **The keymap, opened by Settings' own button, is closed by Escape.**
#[test]
fn escape_closes_the_keymap_opened_from_settings() {
    let mut app = on_the_home("esc_keymap");
    click_on(&mut app, "Settings");
    click_on(&mut app, "Keymap");
    assert_eq!(count::<KeymapDialog>(&mut app), 1, "Settings' Keymap button opens the keymap");
    press(&mut app, KeyCode::Escape);
    assert_eq!(count::<KeymapDialog>(&mut app), 0, "Escape closes the keymap");
}

/// **Project configuration, opened by Settings' own button, is closed by Escape.**
#[test]
fn escape_closes_project_configuration_opened_from_settings() {
    let mut app = editor("esc_project_config");
    chord(&mut app, KeyCode::Comma);
    click_on(&mut app, "Project configuration");
    assert_eq!(count::<ProjectConfigDialog>(&mut app), 1, "Settings' Project configuration button opens it");
    press(&mut app, KeyCode::Escape);
    assert_eq!(count::<ProjectConfigDialog>(&mut app), 0, "Escape closes project configuration");
}

/// **New project, opened by the home's button, is closed by Escape.**
#[test]
fn escape_closes_the_new_project_form_opened_by_the_homes_button() {
    let mut app = on_the_home("esc_new_project");
    click_on(&mut app, "New project");
    assert_eq!(count::<FormCard>(&mut app), 1, "the home's New project button opens the form");
    press(&mut app, KeyCode::Escape);
    assert_eq!(count::<FormCard>(&mut app), 0, "Escape closes the New project form");
}
