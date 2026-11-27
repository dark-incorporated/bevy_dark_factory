//! **Zen mode and the left panel fold the shell with the terminal raised and holding the keys** (#667).
//!
//! The measured defect: with the terminal collapsed, `Cmd+B` gave the viewport 60.09% and Zen gave it
//! 100%; with the terminal at a quarter it gave 27.38%, exactly what it was before either chord. The
//! cause is that `TerminalFocus` folds the shell into the same `Context::Typing` a name being typed
//! gives, so both chords were refused — and the earlier headless test passed because it started with
//! the terminal collapsed, where the shell does not hold the keyboard at all.
//!
//! So every test here **starts with the terminal raised at a quarter and holding the keys**, because
//! that is the state measured to fail. A terminal skips a named few chords and hands them to the
//! window (`chords::SKIPPED_BY_THE_SHELL`) — the rule every terminal keeps, and the reason `Ctrl+J`
//! already reached the pane in that state while these two reached nothing.
//!
//! What each asserts is the thing a person sees: the dock's laid-out width going to nothing and the
//! viewport taking the room, read from `ComputedNode`, which is what the issue measured itself.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::ui::ComputedNode;

use bevy_dark_factory::chrome::{Frame, ViewportSlot};
use bevy_dark_factory::chords::{self, Fixed};
use bevy_dark_factory::keys::Holder;
use bevy_dark_factory::shell_view::ShellView;
use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_pane::{TerminalFocus, TerminalPlacement};

use crate::fixtures;

/// A frame's slack: layout is physical and rounds.
const SLACK_PX: f32 = 2.0;

/// **The state the issue measured and the one every test here starts in**: the terminal raised to a
/// quarter, with the keyboard in it. Without the focus the shell is not what refuses the chords, and
/// the bug would not be in the test at all.
fn editor_with_the_terminal_holding_the_keys(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    bevy_dark_factory::harness::resize_surface(&mut app, 1600, 1000).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..12 {
        app.update();
    }
    app.world_mut().resource_mut::<Terminal>().set_placement(TerminalPlacement::Quarter);
    app.world_mut().resource_mut::<TerminalFocus>().take();
    for _ in 0..6 {
        app.update();
    }
    app
}

/// The primary modifier's key on this build, as `keys::MOD_KEYS` names it.
fn primary() -> KeyCode {
    bevy_dark_factory::keys::MOD_KEYS[0]
}

/// **A chord, pressed and let go**, the way a hand makes it: every key down in one frame so the
/// modifiers are held as the last goes down, frames for the editor to answer, then up.
fn chord(app: &mut App, keys: &[KeyCode]) {
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in keys {
            let logical_key = match key {
                KeyCode::KeyB => Key::Character("b".into()),
                KeyCode::KeyM => Key::Character("m".into()),
                _ => Key::Unidentified(NativeKey::Unidentified),
            };
            app.world_mut().write_message(KeyboardInput { key_code: *key, logical_key, state, text: None, repeat: false, window: Entity::PLACEHOLDER });
        }
        for _ in 0..6 {
            app.update();
        }
    }
}

fn size_of(app: &App, entity: Entity) -> Vec2 {
    app.world().get::<ComputedNode>(entity).map_or(Vec2::ZERO, |n| n.size() * n.inverse_scale_factor())
}

fn viewport(app: &App) -> Vec2 {
    size_of(app, app.world().resource::<Frame>().viewport)
}

fn window(app: &App) -> Vec2 {
    size_of(app, app.world().resource::<Frame>().root)
}

/// **The state the test needs is the state it has**: the pane raised, the shell holding the keys, and
/// the editor's own `Live` saying so. A test that skipped this could pass for the wrong reason, which
/// is exactly how #667 was reported fixed once and was not.
fn assert_the_terminal_holds_the_keys(app: &mut App, when: &str) {
    assert!(app.world().resource::<TerminalFocus>().holds(), "the shell holds the keys {when}");
    assert_eq!(app.world().resource::<Terminal>().placement(), TerminalPlacement::Quarter, "and the pane is at a quarter {when}");
    let live = *app.world().resource::<bevy_dark_factory::keys::Live>();
    assert_eq!(live.2, Holder::Terminal, "and `Live` names it as the holder {when}, so the shell can tell it from a text field: {live:?}");
}

/// **`Cmd+B` folds the left dock and the viewport takes its width, with the terminal at a quarter and
/// holding the keys** — the measurement the issue's first box asks for.
#[test]
fn the_left_panel_folds_with_the_terminal_raised_and_holding_the_keys() {
    let mut app = editor_with_the_terminal_holding_the_keys("zen_terminal_raised_left");
    assert_the_terminal_holds_the_keys(&mut app, "to begin with");
    let frame = *app.world().resource::<Frame>();
    let left_before = size_of(&app, frame.left);
    let viewport_before = viewport(&app);
    assert!(left_before.x > 40.0, "the left dock is on screen to begin with: {left_before:?}");
    assert!(!app.world().resource::<ShellView>().left_hidden, "and the view says nothing is hidden");

    chord(&mut app, &[primary(), KeyCode::KeyB]);

    let left_after = size_of(&app, frame.left);
    let viewport_after = viewport(&app);
    assert!(app.world().resource::<ShellView>().left_hidden, "Cmd+B hid the left panel, with the terminal holding the keys");
    assert_eq!(left_after, Vec2::ZERO, "the left dock lays out to nothing, and it is {left_after:?}");
    assert!(
        viewport_after.x >= viewport_before.x + left_before.x - SLACK_PX,
        "the viewport was {viewport_before:?} beside a left dock of {left_before:?} and is {viewport_after:?} without it: it did not take the room"
    );
}

/// **Zen mode gives the viewport the whole window, with the terminal at a quarter and holding the
/// keys** — the issue's second box, and the state that made Zen a no-op.
#[test]
fn zen_gives_the_viewport_the_window_with_the_terminal_raised_and_holding_the_keys() {
    let mut app = editor_with_the_terminal_holding_the_keys("zen_terminal_raised_zen");
    assert_the_terminal_holds_the_keys(&mut app, "to begin with");
    let viewport_before = viewport(&app);

    chord(&mut app, &[primary(), KeyCode::ShiftLeft, KeyCode::KeyM]);

    assert!(app.world().resource::<ShellView>().zen, "Shift+Cmd+M began Zen mode, with the terminal holding the keys");
    let whole = window(&app);
    let zen_viewport = viewport(&app);
    assert!(
        (zen_viewport - whole).abs().max_element() <= SLACK_PX,
        "Zen mode: the viewport is {zen_viewport:?} in a window of {whole:?}; it should be the whole window"
    );
    assert!(
        zen_viewport.x > viewport_before.x && zen_viewport.y > viewport_before.y,
        "and it grew in both directions: {viewport_before:?} to {zen_viewport:?}"
    );
    assert!(!app.world().resource::<TerminalFocus>().holds(), "Zen mode gave the terminal's keyboard back, as it does from collapsed");
}

/// **The list is one list**: what the shell reads and what a terminal hands back cannot drift apart.
///
/// The seven terminal chords are the pane's own and reach nobody else; the two shell chords are the
/// window's; every other chord is the shell's to read as bytes. Asserted over the whole table rather
/// than over the three names this test cares about, so a new chord has to be placed deliberately.
#[test]
fn the_shell_keeps_its_two_chords_and_reads_everything_else() {
    for named in chords::CHORDS {
        let the_window_keeps_it = !chords::reaches_the_shell(named.fixed);
        let shell_chord = chords::SKIPPED_BY_THE_SHELL.contains(&named.fixed);
        let pane_chord = format!("{:?}", named.fixed).starts_with("Terminal")
            || named.fixed == Fixed::EditorDevAgent;
        assert_eq!(
            the_window_keeps_it,
            shell_chord || pane_chord,
            "`{}` ({:?}) is kept from the shell: {the_window_keeps_it}, but is a shell chord: {shell_chord} and a pane chord: {pane_chord}",
            named.id,
            named.fixed
        );
    }
    assert_eq!(
        chords::SKIPPED_BY_THE_SHELL,
        [Fixed::ShellLeftPanel, Fixed::ShellZen],
        "the window keeps the left panel's chord and Zen mode's: the two a person presses because the terminal is in the way"
    );
    assert!(chords::reaches_the_shell(Fixed::Quit), "Quit is still the shell's to read as a byte, as every terminal does");
    assert!(!chords::reaches_the_shell(Fixed::TerminalCollapse), "and the pane's own chord reaches nobody else");
}
