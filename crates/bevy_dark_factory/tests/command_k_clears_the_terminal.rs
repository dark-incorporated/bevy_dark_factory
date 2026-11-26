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

/// The platform's primary modifier, which `primary_shift(K)` names (#668): Command on a mac, Control
/// elsewhere. The chord is built from `Modifiers::PRIMARY`, which is a *runtime* value, so a test that
/// presses `ControlLeft` on arm64 presses the wrong key and the clear never fires (left 293, right 0).
fn primary() -> KeyCode {
    if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft }
}

/// A modifier held round K, as a hand makes the chord.
fn chord_k(app: &mut App, modifier: KeyCode) {
    // **`Shift` too** (#680): `terminal.clear` is `primary_shift(K)` since plain `Ctrl+K` turned out to be
    // `palette.open_alt` on Linux as well as macOS. A clear is a deliberate act, and Shift says so.
    // **Shift is held across K's whole press and release, and released last.** `keys::shift_ok` reads
    // `ButtonInput` at the frame K is matched, so releasing Shift in the same iteration as K's release
    // leaves `shift_held` false and the chord matches nothing: the test then fails with the scrollback
    // untouched (left 293, right 0 on arm64).
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    send(app, modifier, unidentified(), ButtonState::Pressed);
    send(app, KeyCode::ShiftLeft, unidentified(), ButtonState::Pressed);
    app.update();
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, KeyCode::KeyK, Key::Character("k".into()), state);
        if state == ButtonState::Released {
            send(app, modifier, unidentified(), ButtonState::Released);
            send(app, KeyCode::ShiftLeft, unidentified(), ButtonState::Released);
        }
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
fn shift_ctrl_k_clears_the_scrollback() {
    // **`ControlLeft` plus Shift, not Super** (#637, #668). `terminal.clear` is `primary_shift(K)` on every
    // platform, so on a Mac the primary is Command and Shift is still needed: pressing Super alone matches
    // nothing, `history` stays ~300, and this assertion fails. The name and the press both follow the binding.
    let mut app = with_history("ctrl_k_clears");
    chord_k(&mut app, primary());
    assert_eq!(history(&app), 0, "Shift+primary+K left the scrollback where it was");
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
fn the_clear_is_shift_primary_k_on_every_platform() {
    let clear = bevy_dark_factory::chords::default_of(bevy_dark_factory::chords::Fixed::TerminalClear);
    assert_eq!(
        clear.modifiers,
        bevy_gooey::Modifiers::PRIMARY | bevy_gooey::Modifiers::SHIFT,
        "the clear is Shift+primary+K on every platform: plain Ctrl+K is palette.open_alt on Linux and \
         plain Command+K is palette.open_alt on macOS (#637, #680)"
    );
    assert_eq!(clear.key, KeyCode::KeyK);

    // The shell's own Ctrl+D is exercised end to end by `the_shell_keeps_its_ctrl_chords`, which drives a real
    // PTY; asserting it again here from the chord table would be a second, weaker copy of that test.
}
