//! **The LED's word is drawn at the size of the text it sits with, at every text size** (#422; the owner: *"This should be the same
//! sized text as everything else."*).
//!
//! `bevy_gooey::led` sizes its words from the skin's `text_size * text_scale`, while the editor's own labels take a type role
//! (`chrome::TypeRole`) times the person's text size, so with `text_scale` 1.45 the two diverged: `Agent live` was visibly larger than
//! every chip beside it. The LED in the bar now matches the bar's chips, and the LED on the self-test card matches the card's rows.

use bevy::prelude::*;

use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_agent::AgentSetup;
use bevy_dark_factory::terminal_pane::{TerminalAgentCard, TerminalAgentCardRow, TerminalBar, TerminalInvokeButton, TerminalNewTab, TerminalTabChip};
use bevy_gooey::{Led, LedWord, Role};

use crate::fixtures;

const TRIES: usize = 200;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The editor, pane raised, whose project's settings choose a custom agent and the text size `scale`.
fn editor(tag: &str, scale: f32) -> App {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/blueprint/editor.yaml");
    let settings = format!(r#"{{version: 1, text_scale: {scale}, terminal_agent: Custom, terminal_agent_command: "printf 'AGENT-%s\\n' UP"}}"#);
    std::fs::write(&file, settings).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let files = std::env::temp_dir().join(format!("{tag}_agents"));
    let server = std::env::temp_dir().join(format!("{tag}_server"));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15799), server, files });
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    app
}

fn until(app: &mut App, why: &str, test: impl Fn(&mut App) -> bool) {
    for _ in 0..TRIES {
        for _ in 0..2 {
            app.update();
        }
        if test(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{why}");
}

/// **Start the agent with `Enter`**, the invitation's own second door: the pane holds the keys and invites an agent, and Enter opens
/// the agent tab (`enter_invokes_the_agent`). Used where a click on the Invoke button cannot reach it — measured at text scale 3.0
/// the button is drawn 33 px outside the window (see the owner exception on the test).
fn invoke_with_enter(app: &mut App) {
    until(app, "the Start agent button was never drawn", |app| {
        let world = app.world_mut();
        world.query_filtered::<&UiGlobalTransform, With<TerminalInvokeButton>>().iter(world).next().is_some()
    });
    let world = app.world_mut();
    let window = world.query_filtered::<Entity, With<bevy::window::PrimaryWindow>>().iter(world).next().unwrap_or(Entity::PLACEHOLDER);
    // **The shell holds the keys**, which is what `enter_invokes_the_agent` asks for: a person at the prompt presses Enter and the
    // pane's invitation opens the agent.
    world.resource_mut::<bevy_dark_factory::terminal_pane::TerminalFocus>().take();
    world.write_message(bevy::input::keyboard::KeyboardInput {
        key_code: bevy::input::keyboard::KeyCode::Enter,
        logical_key: bevy::input::keyboard::Key::Enter,
        state: bevy::input::ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    // The release, so the next frame's `just_pressed` is clean.
    world.write_message(bevy::input::keyboard::KeyboardInput {
        key_code: bevy::input::keyboard::KeyCode::Enter,
        logical_key: bevy::input::keyboard::Key::Enter,
        state: bevy::input::ButtonState::Released,
        text: None,
        repeat: false,
        window,
    });
}

fn is_under<M: Component>(world: &World, entity: Entity) -> bool {
    let mut at = Some(entity);
    while let Some(current) = at {
        if world.get::<M>(current).is_some() {
            return true;
        }
        at = world.get::<ChildOf>(current).map(ChildOf::parent);
    }
    false
}

/// The font size of the first word of the LED under `M` (its state word, `LedWord`), and of its label.
fn led_sizes<M: Component>(app: &mut App) -> Option<(FontSize, FontSize)> {
    let world = app.world_mut();
    let leds: Vec<Entity> = world.query_filtered::<Entity, With<Led>>().iter(world).filter(|led| is_under::<M>(world, *led)).collect();
    let led = *leds.first()?;
    let kids = world.get::<Children>(led)?.to_vec();
    let word = kids.iter().find(|kid| world.get::<LedWord>(**kid).is_some())?;
    let label = kids.iter().find(|kid| world.get::<LedWord>(**kid).is_none() && world.get::<Role>(**kid) == Some(&Role::Label))?;
    Some((world.get::<TextFont>(*word)?.font_size, world.get::<TextFont>(*label)?.font_size))
}

/// The font size of the first chip word in the bar: a tab's, which is a chip like the rest.
fn chip_size(app: &mut App) -> Option<FontSize> {
    let world = app.world_mut();
    let chips: Vec<Entity> = world.query_filtered::<Entity, Or<(With<TerminalTabChip>, With<TerminalNewTab>)>>().iter(world).collect();
    chips.iter().find_map(|chip| world.get::<Children>(*chip).and_then(|kids| kids.iter().find_map(|kid| world.get::<TextFont>(kid).map(|font| font.font_size))))
}

/// The font size of a row of the self-test card.
fn row_size(app: &mut App) -> Option<FontSize> {
    let world = app.world_mut();
    world.query::<(&TerminalAgentCardRow, &TextFont)>().iter(world).filter(|(row, _)| row.0.is_some()).map(|(_, font)| font.font_size).next()
}

/// **A test reads the font size of the LED's word and of a chip in the same bar at text scale 1.0 and 1.45; they are equal** — and
/// so on the card, against its rows, and at the ends of the range a file may ask for.
#[test]
fn the_leds_word_is_the_size_of_a_chip_in_the_bar_and_of_a_row_on_the_card_at_every_text_size() {
    // **Owner exception (2026-11-18): `the_leds_word_is_the_size_of_the_text_beside_it.rs::the_leds_word_is_the_size_of_a_chip_in_the_bar_and_of_a_row_on_the_card_at_every_text_size`:**
    // the wait named the agent starting, which at text scale 3.0 cannot happen because the Invoke button is drawn 33 px outside the
    // window (pane 1569 px over a 1536 px window, pre-existing on main: this branch writes no width at all). The sizes are what the
    // test measures and they are asserted below unchanged.
    //
    // So the agent is started by **Enter in the shell** rather than by a click on the Invoke button: the key is the pane's own
    // second door onto the same invitation (`enter_invokes_the_agent`), it is where the pointer cannot reach at any scale, and it
    // leaves the whole test measuring one thing — the LED's word against the text beside it.
    for scale in [1.0_f32, 1.45, 0.5, 3.0] {
        let mut app = editor(&format!("led_size_{}", (scale * 100.0) as u32), scale);
        invoke_with_enter(&mut app);
        until(&mut app, "the agent's card and the chips at the chosen text size", |app| {
            let chip = chip_size(app);
            app.world().resource::<Terminal>().len() == 1
                && led_sizes::<TerminalBar>(app).is_some()
                && led_sizes::<TerminalAgentCard>(app).is_some()
                && row_size(app).is_some()
                && chip == Some(FontSize::Px(10.0 * scale))
        });
        for _ in 0..4 {
            app.update();
        }
        let chip = chip_size(&mut app).expect("a chip in the bar");
        let (word, label) = led_sizes::<TerminalBar>(&mut app).expect("the bar's LED");
        assert_eq!(word, chip, "text scale {scale}: the bar's LED word is a chip's size");
        assert_eq!(label, chip, "text scale {scale}: and so is its name");
        let row = row_size(&mut app).expect("a row of the card");
        let (card_word, card_label) = led_sizes::<TerminalAgentCard>(&mut app).expect("the card's LED");
        assert_eq!(card_word, row, "text scale {scale}: the card's LED word is a row's size");
        assert_eq!(card_label, row, "text scale {scale}: and so is its name");
    }
}
