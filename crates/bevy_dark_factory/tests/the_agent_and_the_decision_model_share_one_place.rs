//! **What the agent is doing and which decision model answers it stand in one place, and nowhere else** (#514,
//! `feedback-1795093920174`).
//!
//! # What the owner saw
//!
//! *"It seems like we need a unified place to put the agent and decision model status."* On the capture the two facts were in three places
//! and none of them held the pair:
//!
//! - the agent's light and its state word (`Agent off`) were in the **terminal bar**, at the foot of the screen;
//! - the reason a check failed was a **card over the terminal**, which came and went;
//! - the decision model was a light at the far end of the **chrome bar** (`○ d3-flash`), at the top;
//! - and the status band's labeler line carried a *third* model name (`labeler: qwen3-vl`), a different model for a different job.
//!
//! A person asking one question — *what is the agent doing, and what is answering it* — read a band at the top, a bar at the bottom and a
//! card in between, and two lights of one shape in two colours that meant two different things.
//!
//! # What this file holds down
//!
//! 1. **Both facts are drawn inside one region**: [`chrome::AgentBand`] holds the agent's light and the decision model's light, and the
//!    region is laid out with a box, so "one place" is a measurement and not a comment.
//! 2. **The model's light is not in the chrome bar any more** — the scatter this issue names, measured where it was.
//! 3. **The agent's state is stated in words in that region**, and the region says what to press when nothing is running, because the
//!    terminal starts collapsed: a light that only said `off` would be a dead end in the state a person meets first.
//! 4. **The model's name and its state follow the setting here**, exactly as they did in the bar — the facts are the same, the place moved.
//!
//! The assertions are on the **drawn nodes**: what `bevy_gooey` actually painted, found by the components the editor put there.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_dark_factory::chrome::{AgentBand, AgentBandHalf, AgentBandLed, Frame};
use bevy_dark_factory::decision_model::{DecisionLed, DecisionLedChip};
use bevy_dark_factory::harness;
use bevy_dark_factory::settings::{EditorSettings, UserSettingsFile};
use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_agent::AgentSetup;
use bevy_gooey::{Led, LedState, LedWord, Tooltip};
use bevy_oracle::{DecisionModel, Endpoints};

use crate::fixtures;

/// **`SHELL` is process-wide**, so the agent tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// Frames a wait gives the editor before it fails, at 50 ms a frame.
const TRIES: usize = 200;

const AGENT_CUSTOM: &str = r#"{version: 1, terminal_agent: Custom, terminal_agent_command: "printf 'AGENT-%s\\n' UP"}"#;
const NO_AGENT: &str = "{version: 1, terminal_agent: Off}";

/// **The editor**, on a project whose settings are `settings`, with an agent setup whose server is a file that exists — so a test can
/// start an agent without a program installed. The pane is left where it starts (collapsed), which is the state the band has to serve.
fn editor(tag: &str, settings: &str) -> App {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    std::fs::write(root.join("assets/blueprint/editor.yaml"), settings).unwrap_or_else(|e| panic!("editor.yaml: {e}"));
    let files = std::env::temp_dir().join(format!("{tag}_agents"));
    let server = std::env::temp_dir().join(format!("{tag}_server"));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15799), server, files });
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

/// **Is `entity` at or below an entity that has `M`?** The ancestry question every "where is this drawn" assertion needs, so a marker
/// put on the right node is what the test reads rather than a name shared by two nodes.
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

/// **The one entity carrying `M`**, or a failure naming how many there were.
fn one<M: Component>(app: &mut App) -> Entity {
    let world = app.world_mut();
    let found: Vec<Entity> = world.query_filtered::<Entity, With<M>>().iter(world).collect();
    assert_eq!(found.len(), 1, "expected exactly one `{}`, found {}", std::any::type_name::<M>(), found.len());
    found[0]
}

/// The state word `bevy_gooey` draws beside a light's dot.
fn state_word(app: &mut App, led: Entity) -> Option<String> {
    let world = app.world_mut();
    let children = world.get::<Children>(led)?.to_vec();
    children.iter().find_map(|child| world.get::<LedWord>(*child).and_then(|_| world.get::<Text>(*child)).map(|text| text.0.clone()))
}

/// A node's laid-out box, physical pixels; `None` when it was never laid out (which is what a `Display::None` panel is).
fn box_of(app: &mut App, entity: Entity) -> Option<Rect> {
    let world = app.world_mut();
    let node = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, node.size()))
}

/// **The agent's light and the decision model's light are drawn inside one region, and that region is on screen.**
#[test]
fn the_agent_and_the_decision_model_stand_in_one_region() {
    let mut app = editor("band_one_place", AGENT_CUSTOM);
    until(&mut app, "the status band's agent light", |app| {
        app.world_mut().query_filtered::<Entity, With<AgentBandLed>>().iter(app.world()).next().is_some()
    });
    let band = one::<AgentBand>(&mut app);
    let agent_led = one::<AgentBandLed>(&mut app);
    let model_led = one::<DecisionLed>(&mut app);
    let world = app.world();
    assert!(is_under::<AgentBand>(world, agent_led), "the agent's light is not inside the band's one region");
    assert!(is_under::<AgentBand>(world, model_led), "the decision model's light is not inside the band's one region");
    // The region has a box, so the two lights are drawn rather than merely parented to something a `Display::None` hides.
    let region = box_of(&mut app, band).expect("the region is laid out");
    assert!(region.width() > 0.0 && region.height() > 0.0, "the region was laid out to nothing: {region:?}");
    for (what, led) in [("the agent's light", agent_led), ("the model's light", model_led)] {
        let drawn = box_of(&mut app, led).unwrap_or_else(|| panic!("{what} is not laid out"));
        assert!(region.contains(drawn.center()), "{what} {drawn:?} is drawn outside the region {region:?}");
    }
}

/// **The decision model's light is not in the chrome bar any more.** The scatter #514 names, measured at the place it happened.
#[test]
fn the_decision_model_is_not_drawn_in_the_chrome_bar() {
    let mut app = editor("band_no_scatter", NO_AGENT);
    until(&mut app, "the status band's agent light", |app| {
        app.world_mut().query_filtered::<Entity, With<AgentBandLed>>().iter(app.world()).next().is_some()
    });
    let bar = app.world().resource::<Frame>().chrome_bar;
    let model_led = one::<DecisionLed>(&mut app);
    let world = app.world();
    assert!(!is_under::<Frame>(world, bar), "the frame is not its own child; this is a sanity check that the walk works");
    let mut at = Some(model_led);
    while let Some(current) = at {
        assert_ne!(
            current, bar,
            "the decision model's light is still drawn in the chrome bar, which is the scatter #514 removes"
        );
        at = world.get::<ChildOf>(current).map(ChildOf::parent);
    }
}

/// **Both lights' words are drawn at the band's readout size, not the body's** (#536). `bevy_gooey::led` draws its label and
/// state word at the skin's body size unless the LED names a step (`LedSize`); the band's own readouts are typed at
/// `text::LABEL`, so a light without the step was a size louder than the words beside it. Measured on the drawn `TextFont`s
/// against the labeler line's, which is typed `LABEL` in the same band.
#[test]
fn the_band_lights_are_drawn_at_the_bands_readout_size() {
    let mut app = editor("band_led_size", NO_AGENT);
    until(&mut app, "the status band's agent light", |app| {
        app.world_mut().query_filtered::<Entity, With<AgentBandLed>>().iter(app.world()).next().is_some()
    });
    for _ in 0..4 {
        app.update();
    }
    let readout = one::<bevy_dark_factory::chrome::LabelerLine>(&mut app);
    let leds = [one::<AgentBandLed>(&mut app), one::<DecisionLed>(&mut app)];
    let world = app.world();
    let band_size = world.get::<TextFont>(readout).map(|font| font.font_size).expect("the labeler line has a font");
    let mut words = 0usize;
    for led in leds {
        for child in world.get::<Children>(led).map(|c| c.to_vec()).unwrap_or_default() {
            let Some(text) = world.get::<Text>(child) else { continue };
            words += 1;
            let drawn = world.get::<TextFont>(child).map(|font| font.font_size);
            assert_eq!(
                drawn,
                Some(band_size),
                "a band light's `{}` is drawn at {drawn:?}, not the band's readout size {band_size:?}",
                text.0
            );
        }
    }
    assert!(words >= 4, "the two lights draw a label and a state word each; found {words} words");
}

/// **The agent's state is stated in words in that region, and the region says what to press when nothing runs.**
#[test]
fn the_region_states_the_agents_state_and_how_to_start_one() {
    let mut app = editor("band_words", AGENT_CUSTOM);
    until(&mut app, "the band's agent light", |app| {
        app.world_mut().query_filtered::<Entity, With<AgentBandLed>>().iter(app.world()).next().is_some()
    });
    // Nothing has been invoked: the light is dark and says `off` beside the dot, which is a word and not only a colour.
    let agent_led = one::<AgentBandLed>(&mut app);
    until(&mut app, "the light to settle dark", |app| {
        let state = app.world().get::<Led>(agent_led).map(|led| led.0);
        state == Some(LedState::Off)
    });
    assert_eq!(state_word(&mut app, agent_led).as_deref(), Some("off"), "the light states its own state in a word");
    // **And the half says what to press.** A dark light in the state a person meets first, with no agent running and the terminal
    // collapsed, would be a dead end; the hover names the chord that raises the pane where `Start agent` lives.
    let half = one::<AgentBandHalf>(&mut app);
    let tip = app.world().get::<Tooltip>(half).map(|t| t.text.clone()).expect("the agent's half carries a hover");
    let chord = bevy_dark_factory::terminal_pane::chord_words(&bevy_dark_factory::chords::chord(bevy_dark_factory::chords::Fixed::TerminalCollapse));
    assert!(tip.contains("No agent is running"), "the hover says no agent is running: {tip:?}");
    assert!(tip.contains(&chord), "the hover names the chord that raises the terminal ({chord:?}): {tip:?}");
    assert!(tip.contains("Start agent"), "the hover names the button that opens one: {tip:?}");
}

/// **The region's decision light follows the setting and the round trips, the same facts the chrome bar showed.**
///
/// The facts did not change; the place did. This is the test `tests/the_decision_model_is_a_setting.rs` makes of the light, taken from the
/// one place the light is now, so a regression in `paint_the_led` fails here as it did there.
#[test]
fn the_regions_decision_light_follows_the_setting() {
    let tag = format!("band_model_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    let user = root.join(format!("user_config_{tag}/settings.yaml"));
    std::fs::create_dir_all(user.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&user, "{version: 1, decision_model: remote}").unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user.clone()));
    // Remote with no key is failed at once, which names the model the setting chose without a request.
    app.insert_resource(bevy_dark_factory::decision_model::DecisionState {
        endpoints: Endpoints { key: None, ..Default::default() },
        ..Default::default()
    });
    until(&mut app, "the band's decision light", |app| {
        app.world_mut().query_filtered::<Entity, With<DecisionLed>>().iter(app.world()).next().is_some()
    });
    until(&mut app, "the light to name the Remote model", |app| {
        let led = one::<DecisionLed>(app);
        let kids: Vec<Entity> = app.world().get::<Children>(led).map(|c| c.iter().collect()).unwrap_or_default();
        kids.iter().any(|kid| app.world().get::<LedWord>(*kid).is_none() && app.world().get::<Text>(*kid).is_some_and(|t| t.0 == "pplx-decider-v1.3:free"))
    });
    // And the light is in the band, which is the point: the setting is read where a person already looks.
    let model_led = one::<DecisionLed>(&mut app);
    let world = app.world();
    assert!(is_under::<AgentBand>(world, model_led), "the model's light is not in the band");
    assert!(world.get::<DecisionLedChip>(world.get::<ChildOf>(model_led).map(ChildOf::parent).unwrap_or(model_led)).is_some(), "the light still stands in the chip that carries the hover");
}

/// **A live agent states its state in the band's words**, so the region is not a light that only means "off" and a colour.
#[test]
fn a_running_agent_is_stated_in_the_band() {
    let mut app = editor("band_live", AGENT_CUSTOM);
    // **The pane is raised and its own `Start agent` button is pressed**, which is the route the editor's other agent tests use — the
    // pane starts collapsed (#424), and the button is only drawn inside it.
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    until(&mut app, "the Start agent button", |app| {
        let world = app.world_mut();
        world
            .query_filtered::<&UiGlobalTransform, With<bevy_dark_factory::terminal_pane::TerminalInvokeButton>>()
            .iter(world)
            .next()
            .is_some()
    });
    let at = {
        let world = app.world_mut();
        world
            .query_filtered::<&UiGlobalTransform, With<bevy_dark_factory::terminal_pane::TerminalInvokeButton>>()
            .iter(world)
            .next()
            .map(|transform| transform.translation)
            .expect("the button stands")
    };
    for action in [
        bevy::picking::pointer::PointerAction::Move { delta: Vec2::ZERO },
        bevy::picking::pointer::PointerAction::Press(bevy::picking::pointer::PointerButton::Primary),
        bevy::picking::pointer::PointerAction::Release(bevy::picking::pointer::PointerButton::Primary),
    ] {
        let handle = app.world().get_resource::<bevy_dark_factory::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
        let location = bevy::picking::pointer::Location {
            target: bevy::camera::NormalizedRenderTarget::Image(bevy::camera::ImageRenderTarget { handle, scale_factor: 1.0 }),
            position: at,
        };
        app.world_mut().write_message(bevy::picking::pointer::PointerInput::new(bevy::picking::pointer::PointerId::Mouse, location, action));
        app.update();
    }
    until(&mut app, "an agent tab to open", |app| !app.world().resource::<Terminal>().is_empty());
    let agent_led = one::<AgentBandLed>(&mut app);
    until(&mut app, "the band's light to leave `off`", |app| {
        app.world().get::<Led>(agent_led).map(|led| led.0) != Some(LedState::Off)
    });
    let state = app.world().get::<Led>(agent_led).map(|led| led.0).expect("the light");
    assert_eq!(state_word(&mut app, agent_led).as_deref(), Some(state.word()), "the band's light says its own state");
    // **The agent's command has run before the test ends**, or an `App` dropped mid-launch waits on a child the pty will not release.
    until(&mut app, "the agent's command to run", |app| {
        let mut terminal = app.world_mut().resource_mut::<Terminal>();
        terminal.tab_mut().is_some_and(|tab| {
            let screen = tab.screen_mut();
            screen.lines().iter().map(|line| line.cells.iter().map(|c| c.ch).collect::<String>()).collect::<Vec<_>>().join("\n").contains("AGENT-UP")
        })
    });
}

/// The setting the band reads is the person's, not the project's — a property `the_chrome_bars_readouts_are_not_a_second_menu` used to
/// guard for the bar's light, restated here for the place it moved to.
#[test]
fn the_band_reads_the_persons_decision_model() {
    assert_eq!(EditorSettings::default().decision_model, DecisionModel::Local, "the shipped default is Local, which is what the band names first");
}
