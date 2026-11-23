//! **The terminal starts collapsed, and the status bar says how to raise it** (#424, the owner's 2026-11-11 decision, replacing #314's
//! raised default: *"The terminal should start out off. And a label at the bottom UI bar stating, 'Terminal Cmd+J'."*).
//!
//! - a headless boot shows the pane collapsed, and a status-bar label reading `Terminal Cmd+J` (`Ctrl+J` off a mac);
//! - clicking the label raises the pane, as the chord does;
//! - a raise is kept: quitting and relaunching opens the pane raised;
//! - the first launch ever has nothing saved and writes nothing.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};

use bevy_dark_factory::settings::UserSettingsFile;
use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_pane::{TerminalFocus, TerminalPaneRoot, TerminalStatusLabel, status_label_words};

use crate::fixtures;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The editor on a project, with a window, a POSIX shell pinned, and the user's settings file at `user` when one is given.
fn editor(tag: &str, user: Option<&PathBuf>) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    if let Some(user) = user {
        app.insert_resource(UserSettingsFile(user.clone()));
    }
    // No `Window`: the editor draws into its own surface image, which is where the pointer is aimed (a window would take the camera).
    for _ in 0..10 { app.update(); }
    app
}



/// **A click through `bevy_picking`'s own input**, on the image the editor draws its interface into (as `a_click_puts_the_keyboard_in_the_terminal`
/// does), at the centre of `entity`: gooey's `click` aims at the app's window, which is not where this editor's surface is.
fn click(app: &mut App, entity: Entity) {
    let at = app.world().get::<UiGlobalTransform>(entity).expect("the label is laid out").translation;
    let handle = app.world().get_resource::<bevy_dark_factory::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    for action in [PointerAction::Move { delta: Vec2::ZERO }, PointerAction::Press(PointerButton::Primary), PointerAction::Release(PointerButton::Primary)] {
        let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle: handle.clone(), scale_factor: 1.0 }), position: at };
        app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
        app.update();
    }
}

/// **`Cmd+J` (`Ctrl+J` off a mac)**, the pane's own chord: while the pane is maximised it covers the status band, so the label is
/// not there to click and the chord is the door that still is (#492).
fn chord_j(app: &mut App) {
    let unidentified = || bevy::input::keyboard::Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified);
    let primary = bevy_dark_factory::keys::MOD_KEYS[0];
    let window = app.world_mut().query_filtered::<Entity, With<bevy::window::PrimaryWindow>>().iter(app.world()).next();
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        for key in [primary, bevy::input::keyboard::KeyCode::KeyJ] {
            let logical = if key == bevy::input::keyboard::KeyCode::KeyJ { bevy::input::keyboard::Key::Character("j".into()) } else { unidentified() };
            app.world_mut().write_message(bevy::input::keyboard::KeyboardInput {
                key_code: key,
                logical_key: logical,
                state,
                text: None,
                repeat: false,
                window: window.unwrap_or(Entity::PLACEHOLDER),
            });
        }
        app.update();
    }
    for _ in 0..4 { app.update(); }
}

/// A scratch settings file's path, in a folder of its own.
fn user_file(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("terminal_collapsed_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir.join("settings.yaml")
}

/// Is the pane drawn (raised)?
fn raised(app: &mut App) -> bool {
    let world = app.world_mut();
    let panes: Vec<bool> = world.query_filtered::<&Node, With<TerminalPaneRoot>>().iter(world).map(|node| node.display != Display::None).collect();
    assert_eq!(panes.len(), 1, "one terminal pane: {panes:?}");
    panes[0]
}

/// The status bar's label: its entity and its words.
fn label(app: &mut App) -> (Entity, String) {
    let world = app.world_mut();
    let entity = world.query_filtered::<Entity, With<TerminalStatusLabel>>().iter(world).next().expect("the status bar carries the terminal's label");
    let words = world
        .get::<Children>(entity)
        .and_then(|kids| kids.iter().find_map(|kid| world.get::<Text>(kid).map(|text| text.0.clone())))
        .expect("the label says something");
    (entity, words)
}

fn kept(path: &PathBuf) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

/// **A headless boot shows the pane collapsed and a status-bar label reading `Terminal Cmd+J`; clicking it raises the pane.**
#[test]
fn the_pane_starts_collapsed_and_its_label_in_the_status_bar_raises_it() {
    let mut app = editor("collapsed_start", None);
    assert!(!raised(&mut app), "the terminal is on every screen but starts collapsed");
    assert!(app.world().resource::<Terminal>().is_collapsed());
    assert!(!app.world().resource::<TerminalFocus>().holds(), "a collapsed pane has no keys");

    let (entity, words) = label(&mut app);
    assert_eq!(words, status_label_words());
    assert!(words.starts_with("Terminal ") && words.ends_with("J"), "the label names the terminal and its chord: {words:?}");
    if cfg!(target_os = "macos") {
        assert_eq!(words, "Terminal Cmd+J", "on a mac the chord is Cmd+J");
    } else {
        assert_eq!(words, "Terminal Ctrl+J", "off a mac the chord is Ctrl+J");
    }

    let diagnostics = {
        let world = app.world();
        let at = world.get::<UiGlobalTransform>(entity).map(|t| t.translation);
        let size = world.get::<ComputedNode>(entity).map(|n| n.size());
        format!("label at {at:?}, size {size:?}")
    };
    click(&mut app, entity);
    for _ in 0..3 { app.update(); }
    let hovered: Vec<String> = {
        let world = app.world();
        world
            .resource::<bevy::picking::hover::HoverMap>()
            .values()
            .flat_map(|hits| hits.keys())
            .map(|hit| format!("{hit:?}:{}", world.get::<Name>(*hit).map_or(String::new(), |n| n.to_string())))
            .collect()
    };
    assert!(raised(&mut app), "a click on the label raises the pane, as the chord does ({diagnostics}; hovered {hovered:?})");
    assert!(app.world().resource::<TerminalFocus>().holds(), "and the keys are in the shell, as the chord leaves them");

    // **Owner exception (2026-11-18): the_terminal_starts_collapsed_with_a_label_in_the_status_bar.rs::the_pane_starts_collapsed_and_its_label_in_the_status_bar_raises_it:**
    // this module modelled the pane as a two-state toggle — "a second click lowers it again, as the chord does". #492 makes the chord,
    // and so the label, a three-position cycle (`Maximised -> Collapsed -> Quarter -> Maximised`, the owner's order), so the click after
    // a raised quarter covers the window instead of lowering it. The assertions below are re-pinned to that cycle; the raising, the
    // words and the keyboard are unchanged.
    click(&mut app, entity);
    for _ in 0..3 { app.update(); }
    assert!(raised(&mut app), "a second click covers the window: the cycle's position after a quarter is `Maximised` (#492)");
    assert!(app.world().resource::<Terminal>().is_maximised(), "and the editor agrees, not just the node");
    // The pane now covers the status band, so the label is not there to click: the chord is the door that still is, and it is the same
    // cycle's next press.
    chord_j(&mut app);
    assert!(!raised(&mut app), "the next press collapses it, keyboard in the shell or not");
    assert!(app.world().resource::<Terminal>().is_collapsed());
}

/// **Raising it, quitting and relaunching opens it raised.**
#[test]
fn a_raise_is_kept_and_the_next_launch_opens_the_pane_raised() {
    let user = user_file("kept");
    {
        let mut first = editor("collapsed_kept_first", Some(&user));
        assert!(!raised(&mut first), "the first launch opens collapsed");
        assert!(!user.exists(), "and keeps nothing: nothing was asked for yet");
        let (entity, _) = label(&mut first);
        click(&mut first, entity);
        for _ in 0..4 { app.update(); }
        assert!(raised(&mut first));
        let text = kept(&user);
        assert!(text.contains("terminal_collapsed: false"), "the raise is written to the user's settings file: {text:?}");
    }
    let mut second = editor("collapsed_kept_second", Some(&user));
    for _ in 0..6 { app.update(); }
    assert!(raised(&mut second), "the next launch opens the pane as it was left, raised");
    assert!(!second.world().resource::<Terminal>().is_collapsed());

    // And lowering it is kept the same way. **Owner exception (2026-11-18): `the_terminal_starts_collapsed_with_a_label_in_the_status_bar.rs::a_raise_is_kept_and_the_next_launch_opens_the_pane_raised`:**
    // the relaunch opens at the quarter (#492), so one click covers the window rather than lowering it; the lower is the cycle's next
    // press, taken by the chord because a maximised pane covers the status band. The persistence this test is about is unchanged.
    let (entity, _) = label(&mut second);
    click(&mut second, entity);
    for _ in 0..4 { app.update(); }
    assert!(raised(&mut second), "the relaunch opens at the quarter, and a click covers the window (#492)");
    assert!(second.world().resource::<Terminal>().is_maximised());
    chord_j(&mut second);
    assert!(!raised(&mut second), "the cycle's next press lowers it");
    assert!(kept(&user).contains("terminal_collapsed: true"), "the lower is written too: {:?}", kept(&user));
    let _ = std::fs::remove_dir_all(user.parent().unwrap_or(&user));
}
