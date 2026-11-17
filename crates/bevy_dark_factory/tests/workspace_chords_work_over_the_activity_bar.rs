//! **A chord the activity bar's tooltip names is a chord that works from anywhere** (#483, `feedback-1794783843215`).
//!
//! The owner, with the pointer on the activity bar's tab list: *"It seems like the shortcuts don't work for these?"*
//!
//! The bar builds each tab's tooltip from the workspace's own command — the title and the chord from `workspace.<id>` in the command
//! registry (`activity_bar::describe_the_tabs`) — so the bar already promises a key per icon. These tests hold it to that promise,
//! from the two places the report names: the pointer over the bar, and the pointer over the map.
//!
//! # The two causes the issue names, and what is measured
//!
//! 1. **The bar taking focus and swallowing the key.** The workspace chords are `Context::Global`
//!    (`keys::Action::WorkspaceProject` … `WorkspaceDesign`), and the dispatcher reads them whatever the pointer is over, so a chord
//!    being dead while the pointer is on the rail would be the bar's own key handling, not the binding.
//! 2. **The pick and the chord going different ways.** A pick goes through `go_to_the_workspace` → `door_open`, which is the same path
//!    the chord takes, so a pick that works and a chord that does not means the key never reached the dispatcher.
//!
//! Both are settled here: the chord must open the workspace it names with the pointer at `(20, 500)` — over the bar — and over the
//! map. And each tab's tooltip chord must equal `keys::chord` for its action, which is the thing the bar claims to show.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput};
use bevy::prelude::*;
use bevy_gooey::Tooltip;

use bevy_dark_factory::activity_bar::ActivityStrip;
use bevy_dark_factory::keys::{self, Action};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// Frames a test settles for after a key, so the switch and its layout have run.
const SETTLE: usize = 8;

/// How many frames a wait watches for a switch.
const TRIES: usize = 60;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.world_mut().spawn((bevy::window::Window { resolution: bevy::window::WindowResolution::new(1600, 900), ..default() }, bevy::window::PrimaryWindow));
    settle(&mut app, 16);
    app
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

fn until(app: &mut App, why: &str, test: impl Fn(&mut App) -> bool) {
    for _ in 0..TRIES {
        for _ in 0..2 {
            app.update();
        }
        if test(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("{why}");
}

/// **The pointer, moved to `at` in the image the editor draws into.** No press: the question is whether a chord works while the
/// pointer *is over* the bar, which is the state the report was made in.
fn point_at(app: &mut App, at: Vec2) {
    let handle = app.world().get_resource::<bevy_dark_factory::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }), position: at };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, PointerAction::Move { delta: Vec2::ZERO }));
    settle(app, 3);
}

/// **Press a workspace's chord**, as its binding spells it: the platform's primary modifier and its digit.
fn press_workspace_chord(app: &mut App, action: Action) {
    let binding = keys::binding(action);
    use bevy::input::ButtonState;
    let window = app.world_mut().query_filtered::<Entity, With<bevy::window::PrimaryWindow>>().single(app.world()).ok();
    let send = |app: &mut App, key_code: KeyCode, state: ButtonState| {
        app.world_mut().write_message(bevy::input::keyboard::KeyboardInput {
            key_code,
            logical_key: bevy::input::keyboard::Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: window.unwrap_or(Entity::PLACEHOLDER),
        });
    };
    for state in [ButtonState::Pressed, ButtonState::Released] {
        send(app, keys::MOD_KEYS[0], state);
        send(app, binding.key, state);
        app.update();
    }
    settle(app, SETTLE);
}

/// The workspace the editor has open.
fn open_workspace(app: &App) -> Option<Workspace> {
    app.world().get_resource::<Workspace>().copied()
}

/// **Every registered workspace's chord opens the workspace it names**, with the pointer over the bar and over the map.
#[test]
fn every_workspace_chord_opens_its_workspace_with_the_pointer_over_the_bar_and_over_the_map() {
    let mut app = editor("chords_everywhere");
    let workspaces: Vec<Workspace> = {
        let world = app.world_mut();
        let strip = world.query::<&ActivityStrip>().iter(world).next().map(|strip| strip.0.clone()).unwrap_or_default();
        assert!(!strip.is_empty(), "the activity bar lists the built workspaces, or this test proves nothing");
        strip
    };
    for pointer in [Vec2::new(20.0, 500.0), Vec2::new(760.0, 300.0)] {
        for workspace in &workspaces {
            let action = Action::ALL.into_iter().find(|a| a.workspace() == Some(*workspace));
            let action = action.unwrap_or_else(|| panic!("{workspace:?} has no chord action"));
            // Somewhere else first, so the press has to move the workspace rather than find it already open.
            let elsewhere = workspaces.iter().copied().find(|w| w != workspace);
            if let Some(elsewhere) = elsewhere {
                let other = Action::ALL.into_iter().find(|a| a.workspace() == Some(elsewhere)).expect("a chord");
                point_at(&mut app, pointer);
                press_workspace_chord(&mut app, other);
            }
            point_at(&mut app, pointer);
            press_workspace_chord(&mut app, action);
            until(&mut app, &format!("{workspace:?}'s chord ({}) did not open it with the pointer at {pointer:?}", keys::chord(action)), |app| {
                open_workspace(app) == Some(*workspace)
            });
        }
    }
}

/// **Each tab's tooltip chord is `keys::chord` for its action**, which is what the bar claims to show.
///
/// gooey keeps the chord in the tooltip's `shortcut` (drawn on a keycap beside the words), so this reads that field: the bar's promise
/// is the *pair*, the workspace's name and the key that opens it, and a keycap is where a person reads the key.
#[test]
fn each_tabs_tooltip_chord_is_the_chord_its_action_answers_to() {
    let mut app = editor("tooltip_chords");
    let strips: Vec<Workspace> = {
        let world = app.world_mut();
        world.query::<&ActivityStrip>().iter(world).next().map(|strip| strip.0.clone()).unwrap_or_default()
    };
    assert!(!strips.is_empty(), "the activity bar lists the built workspaces");
    until(&mut app, "the bar never drew a tooltip with a shortcut", |app| {
        let world = app.world_mut();
        world.query::<&Tooltip>().iter(world).any(|tooltip| tooltip.shortcut.is_some())
    });
    let drawn: Vec<(String, Option<String>)> = {
        let world = app.world_mut();
        world.query::<&Tooltip>().iter(world).map(|tooltip| (tooltip.text.clone(), tooltip.shortcut.map(|s| s.label()))).collect()
    };
    for workspace in &strips {
        let action = Action::ALL.into_iter().find(|a| a.workspace() == Some(*workspace)).expect("a chord action");
        // **The chord the bar writes is the chord its action answers to**, read through gooey's own label because that is the form the
        // tooltip draws: `⌘1` and `Cmd+1` are one chord in two spellings (`Shortcut::label` is symbols on a mac, `keys::chord` is words),
        // and every other surface that shows a keycap — the menus, the palette — draws gooey's. So the comparison is against the action's
        // binding as a `Shortcut`, not against a second spelling of it. That the two name the same key is `keys::chord`'s own job.
        let wanted = bevy_gooey::Shortcut::new(bevy_gooey::Modifiers::PRIMARY, keys::binding(action).key).label();
        let title = workspace.title();
        let (_, shortcut) = drawn
            .iter()
            .find(|(text, _)| text == title)
            .unwrap_or_else(|| panic!("no tooltip names {title}: {drawn:?}"));
        let shortcut = shortcut.as_deref().unwrap_or_else(|| panic!("{title}'s tooltip carries no chord: {drawn:?}"));
        assert_eq!(shortcut, wanted, "{title}'s tooltip shows the chord its action answers to");
    }
}

/// **A chord a context refuses says so in the tooltip**, rather than promising a key that cannot fire.
///
/// The workspace chords are `Context::Global`, so the one context that refuses them all is `Context::Typing` — the shell, or a field,
/// has the keyboard. Measured here: `keys::fires_in(Context::Global, Context::Typing)` is false for every workspace action, which is
/// the fact a tooltip must not hide. Every chord the bar shows is therefore shown with the context it needs, which is the editor
/// itself: nothing typing.
#[test]
fn a_chord_that_a_context_refuses_is_not_promised_without_saying_so() {
    let typing = keys::Live(keys::Context::Typing, keys::Stance::Idle, keys::Holder::Tab, keys::Rebound::NONE);
    for action in Action::ALL {
        if action.workspace().is_none() {
            continue;
        }
        assert!(
            !keys::fires_in(keys::binding(action).context, typing.0),
            "{} is refused while a field or the shell types, and the bar must not promise it then",
            keys::chord(action)
        );
        // And it does fire in the editor's own context: the two together are the whole answer to "is the chord real?".
        let editing = keys::Live(keys::Context::Map, keys::Stance::Idle, keys::Holder::Tab, keys::Rebound::NONE);
        assert!(keys::fires_in(keys::binding(action).context, editing.0), "{} fires while the editor has the keys", keys::chord(action));
    }
}

/// **While the terminal holds the keys, a tab's tooltip says so** (#483's third acceptance item: *"If a chord is refused in a context,
/// the tooltip says which."*).
///
/// This is the mechanism behind the owner's report, measured: the workspace chords are `Context::Global`, the terminal holding the keys
/// sets the live context to `Context::Typing` (`editor::sense_context` folds `TerminalFocus` into the same gate a text field gives), and
/// `keys::fires_in` refuses every global chord there. So the key on the keycap cannot fire, and the words must say who has it rather
/// than leaving a person pressing a chord that does nothing.
#[test]
fn a_tab_tooltip_names_who_has_the_keys_while_the_terminal_does() {
    let mut app = editor("tooltip_typing");
    // Nothing is typing: every tab says its name and nothing else, which is what it has always said.
    let quiet: Vec<String> = {
        let world = app.world_mut();
        world
            .query::<(&Tooltip, &bevy_gooey::IconTab)>()
            .iter(world)
            .map(|(tooltip, _)| tooltip.text.clone())
            .collect()
    };
    assert!(!quiet.is_empty(), "the bar drew its tabs");
    assert!(
        quiet.iter().all(|text| !text.contains("click instead")),
        "with the editor holding the keys a tooltip is just the name: {quiet:?}"
    );

    // The terminal takes the keyboard, which is the state a person is in right after starting an agent.
    app.world_mut().resource_mut::<bevy_dark_factory::terminal_pane::TerminalFocus>().take();
    until(&mut app, "the tooltips never said who has the keys", |app| {
        let world = app.world_mut();
        world.query::<&bevy_gooey::IconTab>().iter(world).count() > 0
            && world.query::<&Tooltip>().iter(world).any(|tooltip| tooltip.text.contains("has the keys"))
    });
    // **The bar's own tooltips**, found under the strip: the editor has other tooltips (the decision model's note among them), and a
    // sweep of every `Tooltip` in the world would be asserting about those too.
    let tabs: Vec<Workspace> = {
        let world = app.world_mut();
        world.query::<&ActivityStrip>().iter(world).next().map(|strip| strip.0.clone()).unwrap_or_default()
    };
    let said: Vec<String> = {
        let world = app.world_mut();
        world
            .query::<&Tooltip>()
            .iter(world)
            .map(|tooltip| tooltip.text.clone())
            .filter(|text| tabs.iter().any(|workspace| text.starts_with(workspace.title())))
            .collect()
    };
    assert_eq!(said.len(), tabs.len(), "every tab of the bar has its tooltip: {said:?}");
    assert!(
        said.iter().all(|text| text.contains("has the keys") && text.contains("click instead")),
        "every tab names the reason and the way in: {said:?}"
    );

    // And the keys back in the editor takes the reason away again, so a stale promise is not left on screen.
    app.world_mut().resource_mut::<bevy_dark_factory::terminal_pane::TerminalFocus>().release();
    until(&mut app, "the tooltips kept the reason after the keys went back", |app| {
        let world = app.world_mut();
        world.query::<&Tooltip>().iter(world).filter(|tooltip| tooltip.text.contains(" — ")).all(|tooltip| !tooltip.text.contains("has the keys"))
    });
}

/// **`tooltip_words` is one expression**, so the words a person reads and an assertion about them are the same sentence.
#[test]
fn the_tooltip_words_name_the_workspace_and_only_the_reason_when_there_is_one() {
    use bevy_dark_factory::activity_bar::tooltip_words;
    let editing = keys::Live(keys::Context::Map, keys::Stance::Idle, keys::Holder::Tab, keys::Rebound::NONE);
    let typing = keys::Live(keys::Context::Typing, keys::Stance::Idle, keys::Holder::Tab, keys::Rebound::NONE);
    assert_eq!(tooltip_words(Workspace::Map, Some(&editing)), Workspace::Map.title(), "with the editor holding the keys: the name alone");
    assert_eq!(tooltip_words(Workspace::Map, None), Workspace::Map.title(), "and with no live context at all, as a headless harness has");
    let refused = tooltip_words(Workspace::Map, Some(&typing));
    assert!(refused.starts_with(Workspace::Map.title()), "the name still leads: {refused}");
    assert!(refused.contains("has the keys") && refused.contains("click instead"), "and it says why and what to do: {refused}");
}
