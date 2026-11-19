//! **`Cmd+J` cycles the terminal through maximised, collapsed and a quarter at the bottom** (#492, the owner:
//! *"I'd like Cmd+J to toggle three position, terminal maximized, minimized, and 1/4 at the bottom."*).
//!
//! - four presses from each of the three starting positions visit all three and come back to the one it started from;
//! - the pane's height is the body's, its collapsed state, and a quarter of the body, within 1 px;
//! - `Shift+Cmd+J` and View ▸ Maximise terminal still reach the maximised position from any of them;
//! - a drag of the grip is the person taking the height, and the cycle's next press starts from there;
//! - the status bar's tooltip says what the next press will do, because a cycle has no state to see.
//!
//! **The measures are the laid-out rectangles**, read from the pane's `ComputedNode` in logical px, as
//! `shift_primary_j_maximises_the_terminal` reads them. The body is the window below the menu bar, because that is exactly what a
//! maximised pane fills, so one number serves for "the body" and "the whole window below the menu bar".
//!
//! **The cycling tests read only what a person sees** — drawn or not, and how tall — so that this file compiles against the code
//! before #492 and fails on what it shows rather than on a name it cannot find. That is what makes the "fails before, passes
//! after" proof a measurement of the behaviour and not of the API.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_dark_factory::app_verbs::{self, AppVerb};
use bevy_dark_factory::menu_bar::EditorMenuBar;
use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_pane::{COLLAPSE_CHORD, TERMINAL_QUARTER_SHARE, TerminalPaneRoot, maximise_chord};

use crate::fixtures;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.world_mut().spawn((Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() }, PrimaryWindow));
    settle(&mut app, 12);
    app
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn window(app: &mut App) -> Entity {
    app.world_mut().query_filtered::<Entity, With<PrimaryWindow>>().single(app.world()).expect("one primary window")
}

fn send(app: &mut App, key_code: KeyCode, logical_key: Key, state: ButtonState) {
    let window = window(app);
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

/// **The platform's primary modifier, optionally Shift, and J**, as a hand makes it.
fn chord_j(app: &mut App, shift: bool) {
    let unidentified = || Key::Unidentified(NativeKey::Unidentified);
    let primary = bevy_dark_factory::keys::MOD_KEYS[0];
    for state in [ButtonState::Pressed, ButtonState::Released] {
        if shift {
            send(app, KeyCode::ShiftLeft, Key::Shift, state);
        }
        send(app, primary, unidentified(), state);
        send(app, KeyCode::KeyJ, Key::Character("j".into()), state);
        app.update();
    }
    settle(app, 4);
}

/// The pane's laid-out height, the room below the menu bar, and whether the pane is drawn — all in logical px.
fn heights(app: &mut App) -> (f32, f32, bool) {
    let world = app.world_mut();
    let (pane, node, parent) = world
        .query_filtered::<(&ComputedNode, &Node, &ChildOf), With<TerminalPaneRoot>>()
        .iter(world)
        .next()
        .map(|(computed, node, parent)| (computed.size().y * computed.inverse_scale_factor(), node.display != Display::None, parent.parent()))
        .expect("the terminal's pane");
    let root = world.get::<ComputedNode>(parent).map_or(0.0, |root| root.size().y * root.inverse_scale_factor());
    let menu = world.query_filtered::<&ComputedNode, With<EditorMenuBar>>().iter(world).next().map_or(0.0, |menu| menu.size().y * menu.inverse_scale_factor());
    (pane, root - menu, node)
}

/// **The position a person would name**, read off the two things the pane shows and the editor holds: drawn or not, and full or not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Seen {
    Maximised,
    Collapsed,
    Quarter,
}

fn seen(app: &mut App) -> Seen {
    let (height, body, drawn) = heights(app);
    if !drawn {
        Seen::Collapsed
    } else if (height - body).abs() <= 1.0 {
        Seen::Maximised
    } else {
        Seen::Quarter
    }
}

/// **Reach `placement` from the collapsed start with the chord alone**, so a starting position is entered the way a person enters
/// it and the walk cannot silently assert against its own writes.
fn start_at(app: &mut App, placement: Seen) {
    let presses = match placement {
        Seen::Collapsed => 0,
        Seen::Quarter => 1,
        Seen::Maximised => 2,
    };
    for _ in 0..presses {
        chord_j(app, false);
    }
    assert_eq!(seen(app), placement, "the test walked to {placement:?} from the collapsed start");
}

/// **Four presses from each starting position visit all three and come back**, and each position's height is the number the issue
/// states: the body's, collapsed, and a quarter of the body, within 1 px.
#[test]
fn cmd_j_four_times_from_each_position_visits_three_and_returns() {
    for start in [Seen::Maximised, Seen::Collapsed, Seen::Quarter] {
        let mut app = editor(&format!("cycle_from_{start:?}"));
        start_at(&mut app, start);
        let mut visited = vec![start];
        for press in 1..=4 {
            chord_j(&mut app, false);
            let now = seen(&mut app);
            visited.push(now);
            let (height, body, drawn) = heights(&mut app);
            match now {
                Seen::Maximised => {
                    assert!(drawn, "press {press} from {start:?}: a maximised pane is drawn");
                    assert!((height - body).abs() <= 1.0, "press {press} from {start:?}: maximised height {height} is the body {body}");
                }
                Seen::Collapsed => {
                    assert!(!drawn, "press {press} from {start:?}: a collapsed pane is out of the layout");
                    assert!(!app.world().resource::<bevy_dark_factory::terminal_pane::TerminalFocus>().holds(), "and gives the keys back");
                }
                Seen::Quarter => {
                    assert!(drawn, "press {press} from {start:?}: a quarter pane is drawn");
                    let want = body * TERMINAL_QUARTER_SHARE;
                    assert!(
                        (height - want).abs() <= 1.0,
                        "press {press} from {start:?}: the quarter is {height} px, and {TERMINAL_QUARTER_SHARE} of the body {body} is {want}"
                    );
                }
            }
        }
        let mut distinct = visited.clone();
        distinct.sort_by_key(|p| match p {
            Seen::Maximised => 0,
            Seen::Collapsed => 1,
            Seen::Quarter => 2,
        });
        distinct.dedup();
        assert_eq!(distinct.len(), 3, "four presses from {start:?} visit all three positions: {visited:?}");
        // **Three states return after three presses, and the issue's "four times ... and returns" has to be read that way**: a
        // three-position cycle cannot come back on an even count, so the walk's return is asserted at the press count the cycle
        // states (three), and the fourth press is what proves it starts round again.
        assert_eq!(visited[3], start, "the third press returns to where it started: {visited:?}");
        assert_eq!(visited[4], visited[1], "and the fourth press begins the next turn: {visited:?}");
    }
}

/// **`Shift+Cmd+J` and View ▸ Maximise terminal still reach the maximised position from any of the three**, each proved separately
/// because they are two doors onto the same state.
#[test]
fn shift_cmd_j_and_the_view_row_still_reach_maximised_from_any_position() {
    // **`Shift+Cmd+J` is a toggle and stays one** (#426): from any position that is not full it maximises, and from full it gives the
    // height back — which `shift_primary_j_maximises_the_terminal` pins and this test does not re-decide. So the issue's "from any
    // state" is read as "from any state that is not already maximised", and the from-full case asserts the give-back instead.
    for start in [Seen::Collapsed, Seen::Quarter] {
        let mut app = editor(&format!("maximise_from_{start:?}"));
        start_at(&mut app, start);
        chord_j(&mut app, true);
        assert_eq!(seen(&mut app), Seen::Maximised, "Shift+Cmd+J from {start:?} maximises");
        let (height, body, _) = heights(&mut app);
        assert!((height - body).abs() <= 1.0, "and the pane fills the body: {height} of {body}");
    }
    {
        let mut app = editor("maximise_from_already_full");
        start_at(&mut app, Seen::Maximised);
        chord_j(&mut app, true);
        assert_ne!(seen(&mut app), Seen::Maximised, "Shift+Cmd+J from full gives the height back, as the toggle's second press does (#426)");
        assert!(heights(&mut app).2, "and the pane is still on screen, at one of the cycle's raised positions");
    }

    for start in [Seen::Maximised, Seen::Collapsed, Seen::Quarter] {
        let mut app = editor(&format!("view_row_from_{start:?}"));
        start_at(&mut app, start);
        press_the_view_row(&mut app);
        if start == Seen::Maximised {
            // The row is View ▸ **Maximise** terminal, a toggle's second press from full: it gives the height back (#426), and what
            // #492 adds is that the height it gives back is a position the cycle knows.
            assert_ne!(seen(&mut app), Seen::Maximised, "View ▸ Maximise terminal from full gives the height back");
        } else {
            assert_eq!(seen(&mut app), Seen::Maximised, "View ▸ Maximise terminal from {start:?} maximises");
        }
    }
}

/// **The View ▸ Maximise terminal row, activated through the row's own door** (`app_verbs::run`), as a person's click reaches it.
fn press_the_view_row(app: &mut App) {
    app_verbs::run(app.world_mut(), AppVerb::MaximiseTerminal);
    settle(app, 6);
}

/// **A drag of the grip is the person taking the height**: the cycle's next turn starts from the dragged height rather than
/// snapping the pane back to the stated quarter (#492).
#[test]
fn a_dragged_height_is_the_cycles_next_starting_point() {
    let mut app = editor("cycle_drag");
    start_at(&mut app, Seen::Quarter);
    let (quarter, _, _) = heights(&mut app);
    let dragged = quarter + 40.0;
    // A drag writes the resource **and** the node, as `the_grip_resizes` does: the resource remembers the height for the next
    // screen and the node is what this frame draws.
    app.world_mut().resource_mut::<Terminal>().set_height(dragged);
    {
        let world = app.world_mut();
        let pane = world.query_filtered::<Entity, With<TerminalPaneRoot>>().single(world).expect("the pane");
        world.get_mut::<Node>(pane).expect("a Node").height = Val::Px(dragged);
    }
    settle(&mut app, 4);
    let (after, _, _) = heights(&mut app);
    assert!((after - dragged).abs() <= 1.0, "the pane is drawn at the dragged {dragged}, not {after}");
    // Round the cycle until it comes back to the quarter: Quarter -> Maximised -> Collapsed -> Quarter, which returns to the drag,
    // not to the stated fraction. The walk counts its own presses rather than assuming three land, so a press the shell consumed
    // cannot turn this into a false pass.
    // **One press first**, because the pane is already at the quarter: `Quarter -> Maximised -> Collapsed -> Quarter` is the turn
    // that must come back to the drag.
    chord_j(&mut app, false);
    assert_ne!(seen(&mut app), Seen::Quarter, "the first press leaves the quarter");
    let mut presses = 1;
    while seen(&mut app) != Seen::Quarter {
        chord_j(&mut app, false);
        presses += 1;
        assert!(presses <= 4, "the cycle never came back round to the quarter: {:?}", seen(&mut app));
    }
    assert_eq!(presses, 3, "the three positions take three presses to return");
    assert_eq!(seen(&mut app), Seen::Quarter, "the cycle is back at the quarter position");
    let (back, _, _) = heights(&mut app);
    assert!((back - dragged).abs() <= 1.0, "and at the person's height {dragged}, not the stated quarter: {back}");
}

/// **The status bar's tooltip says what the next press will do**, because a cycle has no state to see (#492).
#[test]
fn the_status_labels_tooltip_names_the_next_press() {
    use bevy_dark_factory::terminal_pane::{TerminalPlacement, status_tooltip_words};
    for placement in [TerminalPlacement::Collapsed, TerminalPlacement::Quarter, TerminalPlacement::Maximised] {
        let words = status_tooltip_words(placement);
        let subject = match placement.next() {
            TerminalPlacement::Collapsed => "hides",
            TerminalPlacement::Quarter => "quarter",
            TerminalPlacement::Maximised => "cover",
        };
        assert!(words.contains(subject), "from {placement:?} the tooltip says the next press {subject}: {words:?}");
        assert!(words.contains("Cmd+J") || words.contains("Ctrl+J"), "and names the chord: {words:?}");
    }
}

/// **The chord is the one a person will press**: `Cmd+J` with no Shift, and the three positions cycle in the owner's order.
#[test]
fn cmd_j_is_the_cycle_chord() {
    use bevy_dark_factory::terminal_pane::TerminalPlacement;
    assert_eq!(COLLAPSE_CHORD.key, KeyCode::KeyJ, "Cmd+J is the cycle's chord");
    assert_eq!(COLLAPSE_CHORD.modifiers, bevy_gooey::Modifiers::PRIMARY, "with no Shift: Shift+Cmd+J is still the maximise chord");
    assert_eq!(maximise_chord().key, KeyCode::KeyJ);
    assert_eq!(TerminalPlacement::Maximised.next(), TerminalPlacement::Collapsed, "maximised -> collapsed, the owner's order");
    assert_eq!(TerminalPlacement::Collapsed.next(), TerminalPlacement::Quarter, "collapsed -> a quarter");
    assert_eq!(TerminalPlacement::Quarter.next(), TerminalPlacement::Maximised, "a quarter -> maximised");
}
