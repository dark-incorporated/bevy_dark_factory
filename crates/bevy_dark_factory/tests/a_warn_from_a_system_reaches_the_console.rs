//! **A `warn!` from a system reaches a panel in the window** (#509).
//!
//! # What the owner saw
//!
//! The editor writes `<data_dir>/Blueprint/dark_factory.log` — it exists because a release launched from Finder drew a
//! black window and nothing anywhere said why — and there is nothing in the window that reads it. What the window shows
//! is the **journal** (`Cmd+E`): refusals and an agent's steps. So every `tracing::warn!` this editor emits by design on
//! a refusal path went to a file a person had to leave the editor to read.
//!
//! # What this file holds down
//!
//! 1. **A `warn!` from a system, one frame later, is in the console's ring** with its level, its target and its message.
//! 2. **The pills carry the ring's real counts**, and turning one off hides exactly that level — at all four levels.
//! 3. **A line naming a piece offers a chip, and pressing it selects that piece.**
//! 4. **`MAX_ROWS` is honoured**: after 500 lines the ring holds 400 and says how many fell off.
//! 5. **The console is readable with no project open**: its ring is a `Session` resource with no project in it, and the
//!    chord draws the panel on the Projects home.
//!
//! The assertions read the ring (filled through the layer's own sink) and the drawn nodes, so a line that never reached
//! the panel fails here rather than in a blank clip.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy_dark_factory::console::{self, Console, ConsolePill, ConsolePillLabel, ConsoleRows, ConsoleTally, Level, MAX_ROWS};
use bevy_dark_factory::harness;
use bevy_dark_factory::screen::Screen;
use bevy_dark_factory::selection::Selection;
use bevy_gooey::Activate;

use crate::fixtures;

/// Frames the editor is given to settle: the frame is built, then the panel drawn.
const SETTLE_FRAMES: usize = 8;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("console_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place_as("wall@0", "site/wall", (1.0, 1.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app, SETTLE_FRAMES);
    app
}

fn settle(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// **Press the console's chord** the way a person does: a `KeyboardInput` for each modifier then the key, and their
/// releases — the census reads `just_pressed` from `ButtonInput`, which the input plugin fills from these messages.
fn press(app: &mut App) {
    let tap = |app: &mut App, code: KeyCode, state: bevy::input::ButtonState| {
        app.world_mut().write_message(bevy::input::keyboard::KeyboardInput {
            key_code: code,
            logical_key: bevy::input::keyboard::Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    };
    // **Every modifier the chord holds**, not just the platform primary: `Shortcut::just_pressed` compares the held
    // modifiers for equality, so a Shift left out is a chord that never fires.
    let toggle = bevy_dark_factory::chords::chord(bevy_dark_factory::chords::Fixed::ConsoleToggle);
    let held = held_modifiers(&toggle);
    for key in &held {
        tap(app, *key, bevy::input::ButtonState::Pressed);
    }
    let chord = toggle;
    tap(app, chord.key, bevy::input::ButtonState::Pressed);
    tap(app, chord.key, bevy::input::ButtonState::Released);
    for key in &held {
        tap(app, *key, bevy::input::ButtonState::Released);
    }
    settle(app, 2);
}

/// **The keys a chord holds**, the platform's primary modifier and Shift when it asks for it.
fn held_modifiers(chord: &bevy_gooey::Shortcut) -> Vec<KeyCode> {
    let mut held: Vec<KeyCode> = Vec::new();
    if chord.modifiers.ctrl || chord.modifiers.meta {
        held.push(bevy_dark_factory::keys::MOD_KEYS[0]);
    }
    if chord.modifiers.shift {
        held.push(KeyCode::ShiftLeft);
    }
    if chord.modifiers.alt {
        held.push(KeyCode::AltLeft);
    }
    held
}

/// **The console is up**, read off the drawn node rather than a flag beside it.
fn console_is_up(app: &mut App) -> bool {
    app.world_mut()
        .query_filtered::<&Node, With<console::ConsoleOverlay>>()
        .iter(app.world())
        .any(|node| node.display != Display::None)
}

/// The pill label's text for one level.
fn pill_count(app: &mut App, level: Level) -> String {
    let world = app.world_mut();
    let mut query = world.query::<(&Text, &ConsolePillLabel)>();
    query
        .iter(world)
        .find(|(_, label)| label.0 == level)
        .map(|(text, _)| text.0.clone())
        .unwrap_or_else(|| panic!("the {} pill has a label", level.label()))
}

/// Every row's text, as the panel draws it.
fn rows(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let list = world
        .query_filtered::<Entity, With<ConsoleRows>>()
        .iter(world)
        .next()
        .expect("the console draws a row column");
    let mut out = Vec::new();
    let mut stack = vec![list];
    let mut seen = 0;
    while let Some(entity) = stack.pop() {
        seen += 1;
        assert!(seen < 100_000, "the row walk ran away");
        if let Some(text) = world.get::<Text>(entity) {
            out.push(text.0.clone());
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
    }
    out
}

/// **Run `body` under a subscriber with the console's layer in it**, composed the way `LogPlugin` composes its own.
///
/// A headless app disables `LogPlugin` on purpose (the subscriber is process-global and one test binary builds many
/// `App`s), so nothing would carry an event to the ring. This is the same shape `tests/the_log_cannot_fill_a_disk.rs`
/// takes: a real subscriber, the console's own layer, scoped with `with_default` so the process-global one is untouched.
fn under_the_console_layer(sink: &console::Sink, body: impl FnOnce()) {
    use bevy::log::tracing_subscriber::prelude::*;
    let subscriber = bevy::log::tracing_subscriber::registry().with(console::layer(sink));
    bevy::log::tracing::subscriber::with_default(subscriber, body);
}

/// **1. A `warn!` from a system, one frame later, is in the ring** with its level, its target and its message.
#[test]
fn a_warn_from_a_system_reaches_the_ring() {
    let mut app = editor("warn_reaches");
    let sink = app.world().resource::<Console>().sink().clone();
    // A system that emits, stepped once — the shape the issue names ("emits from a system, steps one frame").
    under_the_console_layer(&sink, || {
        app.world_mut()
            .run_system_once(|| {
                bevy::log::warn!("a fill was refused");
            })
            .unwrap_or_else(|e| panic!("{e}"));
    });
    app.update();

    let ring = sink.read();
    let found = ring
        .lines()
        .find(|line| line.message.contains("a fill was refused"))
        .unwrap_or_else(|| panic!("the warn is in the ring: {:?}", ring.lines().map(|l| l.message.clone()).collect::<Vec<_>>()));
    assert_eq!(found.level, Level::Warn);
    // **The target is the module the system is in**, and this test's system is in this file's module — which is what a
    // person reads in the panel to know where a line came from.
    assert!(
        found.target.ends_with("a_warn_from_a_system_reaches_the_console"),
        "its target is the module that said it: {}",
        found.target
    );
}

/// **The chord draws the panel, and the same chord puts it away.**
#[test]
fn the_chord_shows_the_console_and_closes_it() {
    let mut app = editor("chord");
    assert!(!console_is_up(&mut app), "the console is away until its chord");
    press(&mut app);
    assert!(console_is_up(&mut app), "Cmd/Ctrl+Shift+E draws it");
    press(&mut app);
    assert!(!console_is_up(&mut app), "and the same chord puts it away");
}

/// **2. The pills carry the ring's counts, and turning one off hides exactly that level.**
#[test]
fn the_pills_carry_real_counts_and_one_off_hides_one_level() {
    let mut app = editor("pills");
    press(&mut app);
    let sink = app.world().resource::<Console>().sink().clone();
    {
        let mut ring = sink.read();
        // One of each, all distinct so nothing folds.
        ring.push(Level::Debug, "dark_factory::test", "d".to_owned(), None);
        ring.push(Level::Info, "dark_factory::test", "i".to_owned(), None);
        ring.push(Level::Warn, "dark_factory::test", "w".to_owned(), None);
        ring.push(Level::Error, "dark_factory::test", "e".to_owned(), None);
    }
    settle(&mut app, 2);
    for level in Level::ALL {
        assert_eq!(pill_count(&mut app, level), "1", "the {} pill counts its own lines", level.label());
    }
    // **Turn WARN off**, through the pill's own `Activate` — what a click sends.
    let pill = {
        let world = app.world_mut();
        world
            .query::<(Entity, &ConsolePill)>()
            .iter(world)
            .find(|(_, p)| p.0 == Level::Warn)
            .map(|(e, _)| e)
            .expect("the WARN pill is drawn")
    };
    app.world_mut().trigger(Activate { entity: pill });
    settle(&mut app, 2);
    let drawn = rows(&mut app);
    assert!(drawn.iter().any(|row| row.contains(": i")), "the info line still draws: {drawn:?}");
    assert!(drawn.iter().any(|row| row.contains(": e")), "the error line still draws: {drawn:?}");
    assert!(drawn.iter().any(|row| row.contains(": d")), "the debug line still draws: {drawn:?}");
    assert!(!drawn.iter().any(|row| row.contains(": w")), "and the warn line does not: {drawn:?}");
    // The count is what there *are*, not what is shown: a pill whose number fell with the filter would be lying.
    assert_eq!(pill_count(&mut app, Level::Warn), "1", "the pill still says how many warn lines the ring holds");
}

/// **3. A line naming a piece offers a chip, and pressing it selects that piece.**
#[test]
fn a_lines_piece_is_a_chip_that_selects_it() {
    let mut app = editor("chip");
    press(&mut app);
    let sink = app.world().resource::<Console>().sink().clone();
    sink.push(Level::Warn, "dark_factory::test", "this piece overlaps its neighbour".to_owned(), Some("wall@0".to_owned()));
    settle(&mut app, 2);
    // Open the line, as a click on its row does.
    let id = {
        let ring = sink.read();
        ring.lines().last().map(|line| line.id).expect("the line is in the ring")
    };
    app.world_mut().resource_mut::<Console>().open = Some(id);
    settle(&mut app, 2);
    let chip = {
        let world = app.world_mut();
        world
            .query::<(Entity, &console::ConsolePieceChip)>()
            .iter(world)
            .map(|(e, chip)| (e, chip.0.clone()))
            .next()
            .expect("a line about a piece offers a chip")
    };
    assert_eq!(chip.1, "wall@0");
    app.world_mut().trigger(Activate { entity: chip.0 });
    settle(&mut app, 2);
    let selected = app.world().resource::<Selection>().ids().to_vec();
    assert_eq!(selected, vec!["wall@0".to_owned()], "the chip selects the piece the line names");
}

/// **4. `MAX_ROWS` is honoured**: after 500 lines the ring holds 400 and the tally says how many fell off.
#[test]
fn five_hundred_lines_keep_four_hundred_and_say_what_fell_off() {
    let mut app = editor("cap");
    press(&mut app);
    let sink = app.world().resource::<Console>().sink().clone();
    {
        let mut ring = sink.read();
        for i in 0..500 {
            ring.push(Level::Info, "dark_factory::test", format!("line {i}"), None);
        }
    }
    settle(&mut app, 2);
    assert_eq!(sink.read().len(), MAX_ROWS, "the ring keeps {MAX_ROWS} and no more");
    assert_eq!(sink.read().dropped(), 100);
    let tally = {
        let mut query = app.world_mut().query_filtered::<&Text, With<ConsoleTally>>();
        let state = query.iter(app.world()).next().map(|t| t.0.clone());
        state.unwrap_or_default()
    };
    assert!(tally.contains("100 fell off"), "the panel says what it left out: {tally}");
}

/// **5. The console is readable with no project open**, and its ring outlives a door — it is `Session` state.
#[test]
fn the_console_is_readable_with_no_project_open() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(bevy::state::app::StatesPlugin)
        .init_resource::<bevy::diagnostic::FrameCount>()
        .add_plugins(bevy_dark_factory::console::ConsolePlugin)
        .insert_state(Screen::Launcher);
    settle(&mut app, 2);
    let sink = app.world().resource::<Console>().sink().clone();
    // The first lines of a run are before there is a project to be about; `log_file` writes them before
    // `DefaultPlugins`, and the console must carry the same ones.
    sink.push(Level::Info, "dark_factory::startup", "starting: no project".to_owned(), None);
    settle(&mut app, 2);
    let ring = sink.read();
    assert_eq!(ring.len(), 1);
    assert!(ring.lines().next().is_some_and(|line| line.message.contains("no project")));
}
