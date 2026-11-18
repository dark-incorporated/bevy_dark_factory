//! **A modifier is not text, and a chord does not save the note** (#491).
//!
//! The owner's capture `feedback-1794921233218` recorded a note that read `"Cmd "` — four characters —
//! and no Save press. The chord that opened the note box was `Cmd+D`; the next thing typed was a
//! chord (`Cmd+J`), and the note was written out with only the `Cmd ` prefix of it.
//!
//! Two defects, one path, both about the same field:
//!
//! 1. **A modifier never belongs in a note's text.** Command, Control, Shift or Option pressed alone
//!    leaves the field exactly as it was, and asks for no save.
//! 2. **A chord must not save.** `Cmd` and a letter while the box has the keyboard is the editor's
//!    business, not the note's: the note keeps exactly the text that was typed and the report stays
//!    open.
//!
//! And the ordinary case still works: pasted and typed text arrives, and what the box commits is
//! exactly what was typed.
//!
//! The note box is a gooey `text_area` — Bevy's own `TextInput` — so the keys are driven the way the
//! platform delivers them: a `KeyboardInput` message per press and release, carrying the `text` the
//! platform would attach. Nothing here writes the field's text directly.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState, InputSystems};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::MOD_KEYS;
use bevy_dark_factory::report::{Phase, ReportEdit};

use crate::fixtures;

#[derive(Default)]
struct Frame {
    keys: Vec<KeyCode>,
    pointer: Option<Vec2>,
    mouse: Option<bool>,
}

#[derive(Resource, Default)]
struct Script {
    next: Option<Frame>,
    held: Vec<KeyCode>,
}

fn drive(
    mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
) {
    let frame = script.next.take();
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let Some(frame) = frame else {
        return;
    };
    for key in &frame.keys {
        keys.press(*key);
    }
    script.held = frame.keys;
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(frame.pointer);
    }
    match frame.mouse {
        Some(true) => mouse.press(MouseButton::Left),
        Some(false) => mouse.release(MouseButton::Left),
        None => {}
    }
}

fn step(app: &mut App, frame: Frame) {
    app.world_mut().resource_mut::<Script>().next = Some(frame);
    app.update();
}

/// **The editor, with a Window so a chord and a pointer have somewhere to land.**
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag)
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn window(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("the test spawned a primary window")
}

/// One press or release of a key, carrying the text the platform would attach to it.
fn key_message(app: &mut App, code: KeyCode, logical: Key, text: Option<&str>, state: ButtonState) {
    let window = window(app);
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: logical,
        state,
        text: text.map(Into::into),
        repeat: false,
        window,
    });
}

/// **A character typed**: press and release, with its own text.
fn typed(app: &mut App, c: char) {
    let text = c.to_string();
    for state in [ButtonState::Pressed, ButtonState::Released] {
        key_message(
            app,
            KeyCode::KeyA,
            Key::Character(text.as_str().into()),
            Some(text.as_str()),
            state,
        );
    }
    app.update();
}

/// **A modifier pressed and let go, alone.** The platform attaches no text to it.
///
/// Stepped over several frames after the release: a box that ends its edit on lost focus reports its
/// final change a frame or two later, and a test that read the note in the release's own frame would
/// miss exactly the save this issue is about.
fn modifier_alone(app: &mut App, code: KeyCode, logical: Key) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        key_message(app, code, logical.clone(), None, state);
        app.update();
    }
    for _ in 0..4 {
        app.update();
    }
}

/// **A chord**: the modifier and the letter go down in one frame and come up in the next, which is
/// how a keyboard delivers them and what the editor's own chord readers see.
fn chord(app: &mut App, modifier: KeyCode, modifier_logical: Key, letter: KeyCode, c: char) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(modifier);
    let text = c.to_string();
    key_message(app, modifier, modifier_logical.clone(), None, ButtonState::Pressed);
    key_message(
        app,
        letter,
        Key::Character(text.as_str().into()),
        Some(text.as_str()),
        ButtonState::Pressed,
    );
    app.update();
    key_message(app, letter, Key::Character(text.as_str().into()), None, ButtonState::Released);
    key_message(app, modifier, modifier_logical, None, ButtonState::Released);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(modifier);
    for _ in 0..4 {
        app.update();
    }
}

/// The platform's primary modifier, physical and logical.
fn primary() -> (KeyCode, Key) {
    #[cfg(target_os = "macos")]
    let logical = Key::Super;
    #[cfg(not(target_os = "macos"))]
    let logical = Key::Control;
    (MOD_KEYS[0], logical)
}

/// **`Cmd+D`, then `Enter`: a report to the developers with the whole screen picked, so the note box
/// is up and its field has the keyboard.**
fn note_box(tag: &str) -> App {
    let mut app = editor(tag);
    let (modifier, _) = primary();
    step(&mut app, Frame { keys: vec![modifier, KeyCode::KeyD], ..default() });
    assert_eq!(
        app.world().resource::<ReportEdit>().phase(),
        Some(Phase::Picking),
        "Cmd+D opens a report ready to point"
    );
    // Enter with nothing picked is the whole screen.
    for state in [ButtonState::Pressed, ButtonState::Released] {
        key_message(&mut app, KeyCode::Enter, Key::Enter, None, state);
    }
    app.update();
    assert_eq!(
        app.world().resource::<ReportEdit>().phase(),
        Some(Phase::Note),
        "the whole screen is picked and the note box is asked for"
    );
    // **The box arrives one frame later**, so the Enter that chose the subject is not its first
    // newline: see `report_box`. Then it is settled: its field takes the keyboard a frame after it
    // is spawned, and a key typed before that has no field to land in.
    for _ in 0..6 {
        app.update();
    }
    app
}

/// **Every modifier pressed alone leaves the note exactly as it was, and asks for no save.**
///
/// Command, Control, Shift and Option are all in this: a person reaching for a chord presses one
/// before the letter, and a modifier that lands in the note is a defect in the note, not a keystroke.
#[test]
fn a_modifier_alone_leaves_the_note_unchanged_and_saves_nothing() {
    for (code, logical, name) in [
        (MOD_KEYS[0], primary().1, "the platform's command modifier"),
        (KeyCode::ControlLeft, Key::Control, "Control"),
        (KeyCode::ShiftLeft, Key::Shift, "Shift"),
        (KeyCode::AltLeft, Key::Alt, "Option"),
    ] {
        let mut app = note_box(&format!("modifier_alone_{code:?}"));
        // A note with words in it, so a modifier that appended a space or a prefix shows up.
        typed(&mut app, 'C');
        typed(&mut app, 'm');
        typed(&mut app, 'd');
        let before = app.world().resource::<ReportEdit>().note().to_owned();
        assert_eq!(before, "Cmd", "the three letters typed arrive: got {before:?}");

        modifier_alone(&mut app, code, logical);
        let after = app.world().resource::<ReportEdit>().note().to_owned();
        assert_eq!(after, before, "{name} pressed alone changed the note: {before:?} became {after:?}");
        assert!(
            !app.world().resource::<ReportEdit>().save_asked(),
            "{name} pressed alone asked for a save"
        );
        assert_eq!(
            app.world().resource::<ReportEdit>().phase(),
            Some(Phase::Note),
            "{name} pressed alone closed the report"
        );
    }
}

/// **A chord pressed while the note box has the keyboard saves nothing and changes nothing.**
///
/// The owner's capture (`feedback-1794921233218`) recorded a note that read `"Cmd "` and stopped, no
/// Save pressed: the chord that opened the box was `Cmd+D`, and the next thing typed was another
/// chord. Measured headless on 2026-11-18: a chord that moves the keyboard out of the field — a bare
/// `Alt` tapped for the menu bar, `Tab`, a `Shift`+`Tab` — ends the edit, and the box used to report
/// that as a final change and **save the note**. The note keeps exactly what was typed and the report
/// stays open.
#[test]
fn a_chord_while_the_note_box_has_the_keyboard_saves_nothing() {
    // **Every chord that takes the keyboard away**, driven the way the platform delivers it.
    for (name, keys, logical, modifier) in [
        ("Alt, tapped for the menu bar", KeyCode::AltLeft, Key::Alt, None),
        // **Tab may reach the box's own buttons** — that is the box's tab order, not a defect — so
        // these two assert only what #491 is about: no save and an unchanged note.
        ("Tab", KeyCode::Tab, Key::Tab, None),
        ("Shift+Tab", KeyCode::Tab, Key::Tab, Some(KeyCode::ShiftLeft)),
    ] {
        let mut app = note_box(&format!("chord_{keys:?}_{modifier:?}"));
        for c in ['C', 'm', 'd', ' '] {
            typed(&mut app, c);
        }
        assert_eq!(
            app.world().resource::<ReportEdit>().note(),
            "Cmd ",
            "the words are in the note before {name}"
        );

        if let Some(held) = modifier {
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(held);
            key_message(&mut app, held, Key::Shift, None, ButtonState::Pressed);
        }
        for state in [ButtonState::Pressed, ButtonState::Released] {
            key_message(&mut app, keys, logical.clone(), None, state);
            app.update();
        }
        if let Some(held) = modifier {
            key_message(&mut app, held, Key::Shift, None, ButtonState::Released);
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(held);
        }
        for _ in 0..4 {
            app.update();
        }

        let report = app.world().resource::<ReportEdit>();
        assert!(
            !report.save_asked(),
            "{name} asked for a save, so the chord ended the note"
        );
        assert_eq!(
            report.phase(),
            Some(Phase::Note),
            "{name} closed the report, so the note was saved by a chord"
        );
        assert_eq!(
            report.note(),
            "Cmd ",
            "{name} changed the note: it reads {:?}",
            report.note()
        );
        // **A bare modifier steals nothing at all**: the box still takes the next letter. `Tab`
        // is exempt because moving to Save or Cancel is the box's own tab order.
        if keys == KeyCode::AltLeft {
            typed(&mut app, 'X');
            assert_eq!(
                app.world().resource::<ReportEdit>().note(),
                "Cmd X",
                "{name} left the box unable to take the next letter"
            );
        }
    }
}

/// **The owner's own chord, literally: `Cmd ` typed, then `J` with Command held.**
///
/// Bullet 2 of #491 in its own words. The note keeps exactly `Cmd ` and the report stays open —
/// `Cmd+J` is the terminal's chord and must not be taken as the note's, nor as the note's end.
#[test]
fn cmd_then_j_with_command_held_leaves_the_note_exactly_as_typed() {
    let mut app = note_box("cmd_j_literal");
    for c in ['C', 'm', 'd', ' '] {
        typed(&mut app, c);
    }
    assert_eq!(app.world().resource::<ReportEdit>().note(), "Cmd ");

    let (modifier, logical) = primary();
    chord(&mut app, modifier, logical, KeyCode::KeyJ, 'j');

    let report = app.world().resource::<ReportEdit>();
    assert_eq!(
        report.note(),
        "Cmd ",
        "the chord changed the note: it reads {:?}",
        report.note()
    );
    assert_eq!(
        report.phase(),
        Some(Phase::Note),
        "the report is still open, so no chord saved it"
    );
    assert!(!report.save_asked(), "no save was asked for");
}

/// **Pasted and typed text arrives, and the box commits exactly what was typed.**
///
/// The clipboard is the editor's own `bevy_clipboard::Clipboard` resource, set here rather than read
/// from the desktop, so the test is the paste path and not the machine it runs on.
#[test]
fn pasted_and_typed_text_is_exactly_what_the_note_keeps() {
    let mut app = note_box("paste_and_type");
    // Put words on the clipboard the way another application would.
    app.world_mut()
        .resource_mut::<bevy::clipboard::Clipboard>()
        .set_text("pasted words")
        .expect("the clipboard takes text");
    let (modifier, logical) = primary();
    chord(&mut app, modifier, logical, KeyCode::KeyV, 'v');
    assert_eq!(
        app.world().resource::<ReportEdit>().note(),
        "pasted words",
        "Cmd+V pastes into the note"
    );
    typed(&mut app, ' ');
    for c in "and typed".chars() {
        typed(&mut app, c);
    }
    assert_eq!(
        app.world().resource::<ReportEdit>().note(),
        "pasted words and typed",
        "what was pasted and what was typed, in order"
    );

    // **That is what the box will commit**: the note the record writes is this text, whole.
    let committed = app.world().resource::<ReportEdit>().note().to_owned();
    assert_eq!(committed, "pasted words and typed");

    // `Cmd+Enter` ends the edit, which is the box's own save: what it commits is what was typed.
    let (modifier, logical) = primary();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(modifier);
    key_message(&mut app, modifier, logical.clone(), None, ButtonState::Pressed);
    key_message(&mut app, KeyCode::Enter, Key::Enter, None, ButtonState::Pressed);
    app.update();
    key_message(&mut app, KeyCode::Enter, Key::Enter, None, ButtonState::Released);
    key_message(&mut app, modifier, logical, None, ButtonState::Released);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(modifier);
    app.update();

    // A headless editor has no picture to write, so the save itself is refused and the report
    // closes — but the box asked for it, which is the point: the note was committed, whole.
    for _ in 0..4 {
        app.update();
    }
    assert!(
        app.world().resource::<ReportEdit>().phase().is_none(),
        "Cmd+Enter ends the edit and the report is saved or refused, never left open"
    );
}
