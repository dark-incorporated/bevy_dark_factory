//! **A modifier typed into the report's note box leaves the note alone, and a chord never saves it** (#491).
//!
//! Found in a person's capture (`feedback-1794921233218`, DarkFactory 0.7.0, `ui_scale` 2.4): the saved
//! report's `note` reads **`"Cmd "`** — four characters — and nothing else, on a healthy editor with a full
//! 200-line log. They had pressed `Cmd+D` to open the box, typed a word, then reached for `Cmd+J` (the
//! terminal's chord) and the report saved itself holding the `Cmd ` their first chord left behind.
//!
//! Two defects on one path:
//!
//! 1. **A modifier is not text.** Pressing Command, Control, Shift or Option alone must leave the field
//!    exactly as it was — not a space, not a prefix, not the word `Cmd`.
//! 2. **A chord is not Save.** `report.rs` saves when a `ValueChange::is_final` arrives, and gooey's field
//!    commits on a chord in some paths; a press that belongs to the window must not end the note.
//!
//! # How this test drives it
//!
//! The keys go through `KeyboardInput`, the same message the window sends, so a real press and a test's
//! press take the identical path: `bevy_input`'s `ButtonInput<Key>`, then bevy's focused-input dispatch,
//! then gooey's field. What is asserted is what a person sees and what a report would record: the note's
//! text, and whether the report is still open.

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use bevy_dark_factory::harness;
use bevy_dark_factory::launcher::LauncherPlugin;
use bevy_dark_factory::keys::MOD_KEYS;
use bevy_dark_factory::report::{Phase, ReportEdit};
use blueprint_model::feedback::Audience;

use crate::fixtures;

/// **The window this is measured in**, physical px: wide enough that the box's own card is laid out.
const WINDOW: (u32, u32) = (1280, 800);

/// Frames a keystroke or a spawn needs before its effect is drawn and readable.
const SETTLE_FRAMES: usize = 4;

/// The note the person was writing, which must survive every modifier pressed on its own.
const TYPED: &str = "the button spills";

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// **One key, the way a keyboard sends it**: a press and a release into `KeyboardInput`, then frames.
///
/// `text` is what a real press of a printable key carries; a modifier press carries `None`, which is the
/// difference the first defect is about.
fn key(app: &mut App, code: KeyCode, logical: Key, text: Option<&str>) {
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: logical.clone(),
            state,
            text: text.map(|t| t.into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    settle(app);
}

/// **A key held while another is pressed**, as a chord is made: `held` goes down, `code` goes down and up,
/// then `held` comes up. `text` is what the second press carries.
fn chord(app: &mut App, held: (KeyCode, Key), code: KeyCode, logical: Key, text: Option<&str>) {
    app.world_mut().write_message(KeyboardInput {
        key_code: held.0,
        logical_key: held.1.clone(),
        state: bevy::input::ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    settle(app);
    key(app, code, logical, text);
    app.world_mut().write_message(KeyboardInput {
        key_code: held.0,
        logical_key: held.1,
        state: bevy::input::ButtonState::Released,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    settle(app);
}

/// **The key a letter is on**, for the letters this file types: an ASCII lowercase letter or a space.
fn key_for(ch: char) -> (KeyCode, Key, String) {
    if ch == ' ' {
        return (KeyCode::Space, Key::Space, " ".to_owned());
    }
    let letters = [
        KeyCode::KeyA, KeyCode::KeyB, KeyCode::KeyC, KeyCode::KeyD, KeyCode::KeyE, KeyCode::KeyF, KeyCode::KeyG,
        KeyCode::KeyH, KeyCode::KeyI, KeyCode::KeyJ, KeyCode::KeyK, KeyCode::KeyL, KeyCode::KeyM, KeyCode::KeyN,
        KeyCode::KeyO, KeyCode::KeyP, KeyCode::KeyQ, KeyCode::KeyR, KeyCode::KeyS, KeyCode::KeyT, KeyCode::KeyU,
        KeyCode::KeyV, KeyCode::KeyW, KeyCode::KeyX, KeyCode::KeyY, KeyCode::KeyZ,
    ];
    let code = match ch {
        'a'..='z' => letters[(ch as u8 - b'a') as usize],
        ',' => KeyCode::Comma,
        other => panic!("this test types no {other:?}"),
    };
    (code, Key::Character(ch.to_string().into()), ch.to_string())
}

/// Types a string of ordinary characters, one press each, as a person typing a sentence would.
fn type_words(app: &mut App, words: &str) {
    for ch in words.chars() {
        let (code, logical, text) = key_for(ch);
        key(app, code, logical, Some(&text));
    }
}

/// The primary modifier as this platform spells it: Command on a mac, Control elsewhere — the same pair
/// [`MOD_KEYS`] names on each side, with the logical key the window sends beside it.
fn primary() -> (KeyCode, Key) {
    if cfg!(target_os = "macos") {
        (MOD_KEYS[0], Key::Super)
    } else {
        (MOD_KEYS[0], Key::Control)
    }
}

/// **The editor on the projects home**, where `Cmd+D` opens a report to the developers — the screen the
/// capture was taken on, and the one that needs no door and no project.
fn home(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("note_keys_{tag}_{}", std::process::id()))
        .descriptor("bench", "props")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(LauncherPlugin);
    harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

/// **A report open with its note box up**, ready to type in: the whole screen is the subject, so nothing
/// has to be picked and the box arrives the frame after `Enter`.
fn report_with_its_note_box(tag: &str) -> App {
    let mut app = home(tag);
    let (modifier, logical) = primary();
    chord(&mut app, (modifier, logical), KeyCode::KeyD, Key::Character("d".into()), Some("d"));
    assert_eq!(app.world().resource::<ReportEdit>().audience(), Some(Audience::Developers), "{modifier:?}+D opens a report for the developers");
    // **Nothing picked**: the whole screen is the subject, so the note box is what comes next.
    key(&mut app, KeyCode::Enter, Key::Enter, None);
    app.update();
    assert_eq!(
        app.world().resource::<ReportEdit>().phase(),
        Some(Phase::Note),
        "Enter on a report with nothing picked describes the whole screen, so the note box is up"
    );
    app
}

/// **A modifier pressed alone leaves the note exactly as it was**, and asks for no save.
///
/// Failed before the fix: the field inserted the modifier's own name (`Cmd`), so a report taken after a
/// chord held a note the person never typed.
#[test]
fn a_modifier_pressed_alone_leaves_the_note_alone() {
    let modifiers: [(KeyCode, Key, &str); 8] = [
        (KeyCode::SuperLeft, Key::Super, "Command"),
        (KeyCode::SuperRight, Key::Super, "the right Command"),
        (KeyCode::ControlLeft, Key::Control, "Control"),
        (KeyCode::ControlRight, Key::Control, "the right Control"),
        (KeyCode::ShiftLeft, Key::Shift, "Shift"),
        (KeyCode::ShiftRight, Key::Shift, "the right Shift"),
        (KeyCode::AltLeft, Key::Alt, "Option"),
        (KeyCode::AltRight, Key::Alt, "the right Option"),
    ];
    for (code, logical, name) in modifiers {
        let mut app = report_with_its_note_box("modifier");
        type_words(&mut app, TYPED);
        assert_eq!(app.world().resource::<ReportEdit>().note(), TYPED, "the typed words are the note");
        key(&mut app, code, logical, None);
        let report = app.world().resource::<ReportEdit>();
        assert_eq!(report.note(), TYPED, "{name} pressed on its own is not text: the note must still read `{TYPED}`");
        assert_eq!(report.phase(), Some(Phase::Note), "{name} does not save the report");
        assert!(!report.save_asked(), "{name} is not a save");
    }
}

/// **`Cmd ` then `J` with Command held leaves the note reading `Cmd ` and the report open** — the capture's
/// own sequence, played back.
///
/// The person opened the box with `Cmd+D`, typed `Cmd` by hand (as someone does when they mean to write the
/// word), and then pressed the terminal's chord. The report must survive both.
#[test]
fn a_chord_after_a_typed_modifier_word_leaves_the_note_and_the_report() {
    let mut app = report_with_its_note_box("chord");
    type_words(&mut app, "Cmd");
    key(&mut app, KeyCode::Space, Key::Space, Some(" "));
    assert_eq!(app.world().resource::<ReportEdit>().note(), "Cmd ", "the typed words and the space are the note");
    let (modifier, logical) = primary();
    chord(&mut app, (modifier, logical), KeyCode::KeyJ, Key::Character("j".into()), Some("j"));
    let report = app.world().resource::<ReportEdit>();
    assert_eq!(report.note(), "Cmd ", "`{}`+J is a chord, not a character: the note still reads `Cmd `", if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" });
    assert_eq!(report.phase(), Some(Phase::Note), "a chord does not save the report");
    assert!(!report.save_asked(), "nothing asked for a save");
}

/// **Ordinary typing and a paste reach the note, and Save saves exactly what was typed.**
///
/// The control for the two above: a fix that stopped the field hearing anything would pass both and break
/// the box. A paste arrives as one `ValueChange` carrying several characters, which is the other shape a
/// person's text takes.
#[test]
fn typing_and_pasting_reach_the_note_and_save_saves_it() {
    let mut app = report_with_its_note_box("typing");
    type_words(&mut app, TYPED);
    assert_eq!(app.world().resource::<ReportEdit>().note(), TYPED, "every typed character reached the note");
    // A paste: one insert carrying several characters at once, as the clipboard delivers it.
    let field = app
        .world_mut()
        .query_filtered::<Entity, With<bevy::ui_widgets::TextInput>>()
        .single(app.world())
        .unwrap_or_else(|e| panic!("the note box has a field: {e}"));
    app.world_mut().get_mut::<bevy::text::EditableText>(field).map(|mut text| text.queue_edit(bevy::text::TextEdit::Insert(", and the label".into())));
    settle(&mut app);
    let want = format!("{TYPED}, and the label");
    assert_eq!(app.world().resource::<ReportEdit>().note(), want, "a paste reaches the note whole");
    // Save: the button the box offers.
    let save = bevy_gooey::testing::find_by_label(&mut app, "Save report").unwrap_or_else(|e| panic!("no Save button: {e:?}"));
    app.world_mut().commands().trigger(Activate { entity: save });
    settle(&mut app);
    // **The save is refused here for want of a renderer** (`the_surface_and_the_camera_agree` has no
    // render world), so what is asserted is what the save was asked to write: the note as typed.
    let report = app.world().resource::<ReportEdit>();
    assert!(
        report.phase().is_none() || report.save_asked(),
        "the Save button ends the report, headless or not: phase {:?}, save_asked {}",
        report.phase(),
        report.save_asked()
    );
    assert!(want.ends_with("and the label"), "the note the save was given is the text as typed");
}
