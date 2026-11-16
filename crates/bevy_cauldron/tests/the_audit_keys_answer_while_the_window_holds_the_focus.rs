//! **The Audit row's keys answer with nobody in particular at the keyboard** (DL-07, #466; found while recording its clip).
//!
//! The row's keys were refused for a whole session in a real editor. Bevy's input dispatch puts the **primary window** in
//! `InputFocus` at startup and leaves it there, so the row's guard — `focus.get().is_some()`, meant to say *a text field
//! has the keyboard* — read the window as a text field and answered nothing. Measured on big's gate build, 2026-11-16:
//!
//! | arrival in the Agent workspace | `InputFocus.current_focus` | Y, N and S answered |
//! |---|---|---|
//! | the `workspace.agent` command, or the `Ctrl+9` chord | the primary window | 0 |
//! | a click on the activity bar's Agent icon (which clears the focus) | `None` | 3 |
//!
//! A headless editor never had the window in `InputFocus`, which is why `the_person_audits_the_questions` passed and only
//! the real window showed it. This test puts the window there, as `InputDispatchPlugin` does, and holds the row to
//! answering anyway — and to staying quiet while a **real element** (a text field) has the keyboard, which is what the
//! guard is for.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input_focus::InputFocus;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_cauldron::audit_view::Audit;
use bevy_cauldron::workspace::Workspace;

use crate::a_turn_is_recorded_for_learning::{editor, ledger_of, place, present, until_written};

/// An editor with one turn recorded and the Agent workspace open, by the command a person's chord runs.
fn recorded(tag: &str) -> (App, std::path::PathBuf) {
    let (mut app, labels) = editor(tag);
    for mut camera in app.world_mut().query_filtered::<&mut Camera, With<bevy_cauldron::view::MainCamera>>().iter_mut(app.world_mut()) {
        camera.viewport = Some(bevy::camera::Viewport { physical_position: UVec2::new(10, 10), physical_size: UVec2::new(100, 100), ..default() });
    }
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    let first = place(&mut app, [20.0, 18.0]);
    present(&mut app, &[first], &["a bed"]);
    until_written(&mut app);
    press(&mut app, KeyCode::KeyN, "n");
    for _ in 0..5 {
        app.update();
    }
    let command = bevy_cauldron::commands::by_id("workspace.agent").expect("`workspace.agent` is a command");
    let live = *app.world().resource::<bevy_cauldron::keys::Live>();
    app.world_mut().resource_mut::<bevy_cauldron::command_run::Chords>().queue(&command, live);
    for _ in 0..30 {
        app.update();
    }
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Agent, "the Agent workspace did not open");
    (app, labels)
}

fn press(app: &mut App, code: KeyCode, text: &str) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: Key::Character(text.into()),
            state,
            text: (state == ButtonState::Pressed).then(|| text.into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
}

fn answers(labels: &std::path::Path) -> usize {
    ledger_of(labels).audits().count()
}

/// **The keys answer while the primary window holds the focus**, which is what a real editor's `InputFocus` is.
#[test]
fn the_keys_answer_while_the_window_holds_the_focus() {
    let (mut app, labels) = recorded("audit_window_focus");
    // Bevy's `InputDispatchPlugin` sets this at startup and nothing clears it, so a real editor runs with it set.
    let window = app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("a primary window");
    app.world_mut().resource_mut::<InputFocus>().set(window, bevy::input_focus::FocusCause::Navigated);
    assert!(app.world().resource::<Audit>().offers.len() >= 3, "the record's questions were not offered");
    assert_eq!(app.world().resource::<InputFocus>().get(), Some(window));

    for (code, text) in [(KeyCode::KeyY, "y"), (KeyCode::KeyN, "n"), (KeyCode::KeyS, "s")] {
        let before = answers(&labels);
        press(&mut app, code, text);
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(answers(&labels), before + 1, "`{text}` answered nothing while the window held the focus");
    }
}

/// **A focused element still takes the keyboard back**: the row answers nothing while a text field has it.
#[test]
fn a_focused_element_still_keeps_the_row_quiet() {
    let (mut app, labels) = recorded("audit_element_focus");
    let field = app.world_mut().spawn(()).id();
    app.world_mut().resource_mut::<InputFocus>().set(field, bevy::input_focus::FocusCause::Navigated);
    let before = answers(&labels);
    press(&mut app, KeyCode::KeyY, "y");
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(answers(&labels), before, "the row answered while an element had the keyboard");
}
