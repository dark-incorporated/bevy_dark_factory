//! **Command+K clears the terminal, as in VS Code on a mac; Ctrl+K stays the shell's** (#326).
//!
//! The chord is on the Command key (Super off a mac), never the primary one, so Ctrl+K reaches the shell
//! as its kill-line everywhere. Clearing keeps the prompt's line as the first line and drops everything
//! above it, as xterm.js's `clear()` does for VS Code.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_pane::TerminalFocus;

use crate::the_terminal_paints_grounds_and_the_selection::{editor_with_terminal, feed};

/// Lines of output, so there is history to clear.
const PRINTED: usize = 300;

fn send(app: &mut App, key_code: KeyCode, logical_key: Key, state: ButtonState) {
    let window = app
        .world_mut()
        .query_filtered::<Entity, With<PrimaryWindow>>()
        .single(app.world())
        .expect("one primary window");
    app.world_mut().write_message(KeyboardInput {
        key_code,
        text: match (&logical_key, state) {
            (Key::Character(c), ButtonState::Pressed) => Some(c.clone()),
            _ => None,
        },
        logical_key,
        state,
        repeat: false,
        window,
    });
}

/// A modifier held round K, as a hand makes the chord.
fn chord_k(app: &mut App, modifier: KeyCode) {
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, modifier, unidentified(), state);
        send(app, KeyCode::KeyK, Key::Character("k".into()), state);
        app.update();
    }
    for _ in 0..3 { app.update(); }
}

fn history(app: &App) -> usize {
    app.world().resource::<Terminal>().tab().expect("a shell is open").screen().scrolled_lines()
}

fn with_history(tag: &str) -> App {
    let mut app = editor_with_terminal(tag);
    app.world_mut().resource_mut::<TerminalFocus>().take();
    let text: String = (1..=PRINTED).map(|n| format!("{n}\r\n")).collect();
    feed(&mut app, text.as_bytes());
    for _ in 0..3 { app.update(); }
    assert!(history(&app) > 0, "the output left no history, so this test proves nothing");
    app
}

#[test]
fn ctrl_k_clears_the_scrollback() {
    // **`ControlLeft`, not Super** (#637). `terminal.clear` is `Modifiers::CTRL` on every platform since c450152,
    // so pressing Super (meta) matches nothing, `history` stays ~300, and this assertion fails. The name and the
    // press both follow the binding.
    let mut app = with_history("ctrl_k_clears");
    // **`Modifiers::CTRL` everywhere** (#637): a Mac's Command is the wrong key and nothing would clear.
    chord_k(&mut app, KeyCode::ControlLeft);
    assert_eq!(history(&app), 0, "Ctrl+K left the scrollback where it was");
    let mut terminal = app.world_mut().resource_mut::<Terminal>();
    let screen = terminal.tab_mut().expect("a shell is open").screen_mut();
    screen.lines();
    assert_eq!(screen.cursor().row, 0, "the prompt's line is the first line after a clear, as VS Code leaves it");
}

/// **The clear is `Ctrl+K` on every platform, and the shell keeps the control chords it owns** (#637).
///
/// What replaced `ctrl_k_is_the_shells_and_clears_nothing`: that test asserted the editor does **not** bind
/// Ctrl+K so the shell could keep its kill-line. #637 moved the clear to `Modifiers::CTRL` everywhere to end a
/// real macOS collision with `palette.open_alt`, and the two assertions cannot both hold. **The editor winning
/// is the deliberate choice** -- a person pressing Ctrl+K in the terminal sees it clear, as in VS Code.
///
/// What must still be true is that the editor has not claimed the chords the shell actually uses — and that is
/// `the_shell_keeps_its_ctrl_chords`'s job, which drives a real PTY. **This test does not duplicate it**: an earlier
/// draft of this comment claimed it pinned Ctrl+D as unclaimed, and no such assertion was written. Saying a test
/// checks something it does not is the same defect this pass spent the day finding.
#[test]
fn the_clear_is_ctrl_k_on_every_platform() {
    let clear = bevy_dark_factory::chords::default_of(bevy_dark_factory::chords::Fixed::TerminalClear);
    assert_eq!(clear.modifiers, bevy_gooey::Modifiers::CTRL, "the clear is Ctrl+K on every platform");
    assert_eq!(clear.key, KeyCode::KeyK);

    // The shell's own Ctrl+D is exercised end to end by `the_shell_keeps_its_ctrl_chords`, which drives a real
    // PTY; asserting it again here from the chord table would be a second, weaker copy of that test.
}
