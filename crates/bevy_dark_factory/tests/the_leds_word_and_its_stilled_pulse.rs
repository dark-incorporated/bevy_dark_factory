//! **The LED a person is told to read carries its state in a word, and reduced motion stills its pulse** (#478).
//!
//! The owner's capture (`feedback-1794783529068`, `feedback-1794238731605`, `-1794238762065`, `-1794277713471`) is the same
//! complaint four times across three app versions: *"The glow on these LEDs is still too bright."* / *"covers too much area"*.
//! The halo comes from `bevy_gooey`'s `led` (`src/led.rs`), whose `GLOW_BLUR_DOTS`, `GLOW_SPREAD_DOTS` and `GLOW_ALPHA` are
//! **private constants at this repository's `f1904be` pin and on gooey's `main` today** (measured 2026-11-18), so the three
//! numbers the issue asks to change are not reachable from here without vendoring the crate, which `AGENTS.md` forbids.
//! That half of the issue is `bevy-coven/bevy_gooey#91` (open) and is filed there.
//!
//! **What IS this editor's, and is held down here**, is the part of the acceptance that does not depend on the glow:
//!
//! - the LED's **state word** stands beside the dot and stays readable — it is what carries the state, so a dimmer halo can
//!   never be the reason a person cannot tell live from busy (WCAG 2.2 SC 1.4.1: colour is never the only signal);
//! - **reduced motion stills the pulse**: a `Busy` LED under `MotionSettings { reduced: true }` draws one steady glow rather
//!   than animating one (SC 2.3.3).
//!
//! Both are asserted against `bevy_gooey`'s own `update_leds`, from a scene built here, so this file measures the same code the
//! terminal's LEDs run and would fail if the editor ever stopped getting the word or the stylling.

use bevy::prelude::*;
use bevy::ui::BoxShadow;

use bevy_gooey::{GooeySkin, LedState, LedWord, Role};

use crate::fixtures;

/// Frames a LED needs: `apply_skin` writes the dot in the frame it appears, `update_leds` the glow in `Style`.
const SETTLE_FRAMES: usize = 4;

/// The one text of an LED, and the dot's glow.
fn drawn(app: &mut App, led: Entity) -> (String, Option<BoxShadow>) {
    let world = app.world_mut();
    let children: Vec<Entity> = world.get::<Children>(led).map(|k| k.iter().collect()).unwrap_or_default();
    let mut words = String::new();
    let mut glow = None;
    for child in children {
        if world.get::<LedWord>(child).is_some()
            && let Some(text) = world.get::<Text>(child)
        {
            words = text.0.clone();
        }
        if world.get::<Role>(child) == Some(&Role::LedDot)
            && let Some(shadow) = world.get::<BoxShadow>(child)
        {
            glow = Some(shadow.clone());
        }
    }
    (words, glow)
}

/// An editor with the gooey style set running, and one LED of `state` in it.
fn with_an_led(tag: &str, state: LedState, reduced_motion: bool) -> (App, Entity) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    // **The person's motion setting, in the file a person writes.** `settings::sync_settings` re-reads the layer every frame and
    // `ui::follow_the_motion_setting` copies its `reduced_motion` into `bevy_gooey::MotionSettings`, so a resource written by hand
    // is overwritten before `update_leds` reads it (measured: both arms drew the identical steady 0.55).
    let file = root.join("assets/blueprint/editor.yaml");
    std::fs::write(&file, format!("{{version: 1, reduced_motion: {reduced_motion}}}"))
        .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // **And motion has to be running at all.** `lands_travel` is `reduced || is_off()`, and a headless harness leaves
    // `duration_scale` at 0 — `is_off()` — so the moving arm would draw a still glow too and the test would pass while proving
    // nothing. The editor's follow writes `reduced` only, so this value survives its frame.
    app.world_mut().resource_mut::<bevy_gooey::MotionSettings>().duration_scale = 1.0;
    let skin = app.world().resource::<GooeySkin>().clone();
    // `bevy_gooey::led` already carries the `Led` component, so spawning it twice is a panic at spawn (two of one component
    // in a bundle), not a test of anything.
    let led = app.world_mut().spawn(bevy_gooey::led("Agent", state)).id();
    // The skin has to reach it: the harness's root carries the editor's, and this is spawned at the world's root.
    app.world_mut().entity_mut(led).insert(bevy_gooey::SkinScope(skin));
    // **`update_leds` is gooey's own**, run in its `Style` set (`bevy_gooey`'s `lib.rs`), so the editor's plugin is all that is
    // needed for the dot and its glow to be written — there is no second registration to make here.
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    // **The premise, asserted rather than assumed**: motion is running, and the gooey flag follows the person's setting — so
    // the two arms differ in the thing the test is about and in nothing else.
    let motion = app.world().resource::<bevy_gooey::MotionSettings>().clone();
    assert!(!motion.is_off(), "motion is not running, so a still glow would prove nothing: {motion:?}");
    assert_eq!(
        motion.reduced, reduced_motion,
        "the reduced-motion setting did not reach gooey's MotionSettings, so this fixture proves nothing"
    );
    (app, led)
}

fn spread_of(values: &[f32]) -> f32 {
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for v in values { lo = lo.min(*v); hi = hi.max(*v); }
    hi - lo
}

/// **The state word stands beside the dot and says the state**, for every state, including the two the capture is about.
#[test]
fn every_leds_state_word_stands_beside_its_dot() {
    for state in [LedState::Off, LedState::Live, LedState::Busy, LedState::Failed] {
        let (mut app, led) = with_an_led("led_word", state, false);
        let (words, glow) = drawn(&mut app, led);
        assert_eq!(
            words,
            state.word(),
            "a {state:?} LED's word is {:?}: the dot's colour is never the only signal, and it is the word a person reads when \
             the halo is dimmed",
            state.word()
        );
        match state {
            LedState::Off => assert!(glow.is_none_or(|g| g.0.is_empty()), "an off LED draws no glow at all"),
            _ => assert!(glow.is_some_and(|g| !g.0.is_empty()), "a {state:?} LED draws its glow"),
        }
    }
}

/// **Reduced motion stills the pulse**: a `Busy` LED's glow is one value over time, not an animation.
///
/// This is the acceptance item that is this editor's to hold: the halo's size and alpha are gooey's (gooey#91), but whether a
/// person who asked for reduced motion gets a pulsing square or a steady one is decided by the setting the editor writes.
#[test]
fn reduced_motion_stills_the_busy_pulse() {
    let alphas = |reduced: bool| -> Vec<f32> {
        let (mut app, led) = with_an_led("led_pulse", LedState::Busy, reduced);
        let mut out = Vec::new();
        for _ in 0..6 {
            // Real time has to pass for a pulse to have moved: `PULSE_SECONDS` is 1.2 s.
            std::thread::sleep(std::time::Duration::from_millis(120));
            app.update();
            let (_, glow) = drawn(&mut app, led);
            out.push(glow.and_then(|g| g.0.first().map(|layer| layer.color.alpha())).unwrap_or(0.0));
        }
        out
    };
    let steady = alphas(true);
    let moving = alphas(false);
    // **The control**: with motion allowed the same LED does pulse, or "it did not move" below would be a claim about the fixture
    // rather than about the setting. Measured: 0.51 -> 0.85 -> 0.31, a spread of 0.54.
    assert!(
        spread_of(&moving) > 0.1,
        "the moving arm drew {moving:?}, a spread of {}: the LED does not pulse at all here, so a still one proves nothing",
        spread_of(&moving)
    );
    let spread = spread_of(&steady);
    assert!(
        spread <= f32::EPSILON,
        "a Busy LED under reduced motion drew {steady:?}, a spread of {spread}: the pulse has to stop when a person asks it to"
    );
}
