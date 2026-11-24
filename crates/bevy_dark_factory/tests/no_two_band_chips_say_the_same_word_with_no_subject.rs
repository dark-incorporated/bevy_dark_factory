//! **Every chip in the status band names its subject** (#597): two lights stood side by side reading `○ off` and `○ off`, so a reader could not
//! tell which was the agent and which the decision model.
//!
//! # What the owner saw
//!
//! A pixel scan of the band's left group at `1600x1000`, row `y=985`, found a hollow ring at `x 30..52` and a second at `x 55..77` with the glyph
//! `off` drawn in it — `○ off off ○ d3-flash`. The two values come from different places (the drawn agent tab's `AgentPost` and the decision
//! model's link), so a reader who guessed which was which was wrong half the time.
//!
//! # Why it happened
//!
//! Not a wording call: a write. `bevy_gooey::led` draws two words beside a light's dot — the light's **name** and its **state** — and marks the
//! state one `LedWord`. `terminal_agent::paint_the_band` wrote the state into *every* text child of the light, so it overwrote `Agent` with `off`
//! before the frame was drawn. The subject was not missing from the design; it was erased every frame.
//!
//! # What this file holds down
//!
//! Every light in the band is read **as it is drawn** — by walking the band's own children and taking the `Text` of each word, in layout order —
//! and each must state its own subject. The rule is stated as the issue states it: two chips may not read the same word with no subject, so
//! the assertion is on the drawn sentence, not on a constant that happens to hold the right string.
//!
//! **At both sizes the issue names**, `1600x1000` and `1100x700`: a band laid out narrower is where a subject would be dropped to save room.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::chrome::{AgentBand, AgentBandHalf, AgentBandLed, AGENT_LABEL};
use bevy_dark_factory::decision_model::{DecisionLed, DecisionLedChip};
use bevy_dark_factory::harness;
use bevy_gooey::{LedState, LedWord, Tooltip};

use crate::fixtures;

/// One frame of the test clock, in seconds.
const FRAME_S: f32 = 1.0 / 60.0;
/// The two sizes the issue measured the band at.
const SIZES: [(u32, u32); 2] = [(1600, 1000), (1100, 700)];
/// Frames a wait gives the editor before it fails.
const TRIES: usize = 120;

/// The editor, on a project with one piece, laid out at `size`.
fn editor(tag: &str, size: (u32, u32)) -> App {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, size.0, size.1).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME_S)));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn until(app: &mut App, why: &str, test: impl Fn(&mut App) -> bool) {
    for _ in 0..TRIES {
        app.update();
        if test(app) {
            return;
        }
    }
    panic!("{why}");
}

/// **The words a light draws, in layout order**: its dot is not a word, and the name comes before the state (`bevy_gooey::led`'s own bundle).
///
/// An `LedWord` is marked as such by gooey, so the pair is split the way gooey splits it rather than by position in a `Vec`.
fn words_of(app: &mut App, led: Entity) -> (String, String) {
    let world = app.world_mut();
    let children: Vec<Entity> = world.get::<Children>(led).map(|c| c.iter().collect()).unwrap_or_default();
    let mut name = String::new();
    let mut state = String::new();
    for child in children {
        let Some(text) = world.get::<Text>(child) else { continue };
        if world.get::<LedWord>(child).is_some() {
            state.clone_from(&text.0);
        } else {
            name.clone_from(&text.0);
        }
    }
    (name, state)
}

/// **A light in the band names both its subject and its state, and no two of them say the same word with no subject.**
#[test]
fn no_two_band_chips_say_the_same_word_with_no_subject() {
    let mut failures = Vec::new();
    for size in SIZES {
        let mut app = editor("band_subjects", size);
        until(&mut app, "the band's two lights", |app| {
            let world = app.world_mut();
            let agent = world.query_filtered::<Entity, With<AgentBandLed>>().iter(world).count();
            let model = world.query_filtered::<Entity, With<DecisionLed>>().iter(world).count();
            agent == 1 && model == 1
        });
        let agent = app.world_mut().query_filtered::<Entity, With<AgentBandLed>>().iter(app.world()).next().expect("the agent's light");
        let model = app.world_mut().query_filtered::<Entity, With<DecisionLed>>().iter(app.world()).next().expect("the model's light");
        // The model's light stands in its chip, and the chip in the band; the agent's half likewise. `AgentBand` is the region the pair shares.
        {
            let world = app.world();
            let in_band = |entity: Entity| {
                let mut at = entity;
                for _ in 0..3 {
                    let Some(parent) = world.get::<ChildOf>(at).map(ChildOf::parent) else { return false };
                    if world.get::<AgentBand>(parent).is_some() {
                        return true;
                    }
                    at = parent;
                }
                false
            };
            assert!(in_band(agent), "{size:?}: the agent's light is not in the band");
            assert!(in_band(model), "{size:?}: the model's light is not in the band");
        }
        // **Read after a frame has drawn them**: the write that erased the subject ran every frame, and this is the state a person sees.
        app.update();
        let agent_words = words_of(&mut app, agent);
        let model_words = words_of(&mut app, model);
        // The subject is the light's own: the agent's is `AGENT_LABEL`, which is also its accessible name.
        if agent_words.0 != AGENT_LABEL {
            failures.push(format!("{size:?}: the agent's light reads {:?} where it must name its subject {AGENT_LABEL:?}", agent_words));
        }
        if model_words.0.is_empty() {
            failures.push(format!("{size:?}: the model's light draws no subject: {model_words:?}"));
        }
        // And the rule the issue states: the two lights are told apart by their own text, so their subjects differ.
        if agent_words.0 == model_words.0 {
            failures.push(format!("{size:?}: the two lights name the same subject {:?}: {agent_words:?} and {model_words:?}", agent_words.0));
        }
        // Each still says what it is doing — **the agent's light draws its state word** (the model's is hidden by `quiet_the_word`, which is why
        // its subject is the model's own name and its state lives in the hover). The defect this test forbids was the subject erased, not a
        // state dropped.
        const STATES: [LedState; 4] = [LedState::Off, LedState::Live, LedState::Busy, LedState::Failed];
        if STATES.iter().all(|s| s.word() != agent_words.1) {
            failures.push(format!("{size:?}: the agent's light draws no state word beside its dot: {:?}", agent_words));
        }
        // **`○ off off` was the defect in one line**: the light's two words were the same, so its subject was gone.
        if agent_words.0 == agent_words.1 {
            failures.push(format!("{size:?}: the agent's light says the same word twice, so it names no subject: {:?}", agent_words));
        }
        // The hover says the whole sentence, so the band is not the only place the pair can be read; it names the subject too.
        let half = app.world_mut().query_filtered::<Entity, With<AgentBandHalf>>().iter(app.world()).next().expect("the agent's half");
        let tip = app.world().get::<Tooltip>(half).map(|t| t.text.clone()).unwrap_or_default();
        // **Case-insensitive**, because the hover answers in a sentence ("No agent is running…") and a sentence
        // that names the subject in lower case does name it; the capitalised label is the chip's word, not the
        // only spelling a reader needs (#597).
        if !tip.to_lowercase().contains(&AGENT_LABEL.to_lowercase()) {
            failures.push(format!("{size:?}: the agent's half does not name its subject in the hover: {tip:?}"));
        }
        let chip = app
            .world_mut()
            .query_filtered::<Entity, With<DecisionLedChip>>()
            .iter(app.world())
            .next()
            .expect("the model's chip");
        let tip = app.world().get::<Tooltip>(chip).map(|t| t.text.clone()).unwrap_or_default();
        if !tip.contains("Decision model") {
            failures.push(format!("{size:?}: the model's chip does not name its subject in the hover: {tip:?}"));
        }
    }
    assert!(failures.is_empty(), "{} band light(s) do not name their subject:\n{}", failures.len(), failures.join("\n"));
}
