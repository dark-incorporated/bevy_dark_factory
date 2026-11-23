//! **One press of the `D` chords opens exactly one of the two panels** (#560).
//!
//! `Shift+Cmd+D` was written twice in `chords::CHORDS` — the frame profile's and the project report's — and
//! `Shortcut::just_pressed` compares the whole modifier set, so the one press satisfied both handlers and a report
//! opened over a frame profile that opened at the same moment. The chords are separate now: the project report keeps
//! `Shift+Cmd+D` and the profile took `Shift+Alt+Cmd+D`.
//!
//! `chords::tests::no_two_fixed_chords_share_a_press` proves no two rows of the table are one press. It cannot prove
//! what this file proves: that a press reaching the two *handlers* in `crate::keys::Phase::Act` moves one panel.

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::{ALT_KEYS, MOD_KEYS, SHIFT_KEYS};
use bevy_dark_factory::profile::ProfileOverlay;
use bevy_dark_factory::report::{Audience, ReportEdit};

use crate::fixtures;

/// Frames to let the door, the panel and its layout settle before a press.
const SETTLE: usize = 10;

#[derive(Resource, Default)]
struct Script {
    next: Option<Vec<KeyCode>>,
    held: Vec<KeyCode>,
}

/// **The press, on the frame its handlers read it.** Keys go down here and up on the next frame, so a chord is a
/// `just_pressed` exactly once — the way a person's chord arrives.
fn drive(mut script: ResMut<Script>, mut keys: ResMut<ButtonInput<KeyCode>>) {
    for key in script.held.drain(..) {
        keys.release(key);
    }
    let Some(press) = script.next.take() else { return };
    for key in &press {
        keys.press(*key);
    }
    script.held = press;
}

fn press(app: &mut App, keys: Vec<KeyCode>) {
    app.world_mut().resource_mut::<Script>().next = Some(keys);
    app.update();
    // The release frame, so the next press is a fresh `just_pressed` and not a held key.
    app.update();
}

/// The platform's primary, Shift and Alt together, with `key`.
fn chord(key: KeyCode, shift: bool, alt: bool) -> Vec<KeyCode> {
    let mut press = vec![MOD_KEYS[0], key];
    if shift {
        press.push(SHIFT_KEYS[0]);
    }
    if alt {
        press.push(ALT_KEYS[0]);
    }
    press
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.init_resource::<Script>();
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    app.add_systems(PreUpdate, drive.after(InputSystems));
    for _ in 0..SETTLE {
        app.update();
    }
    app
}

/// **Whether the frame profile's panel is drawn**, read off its own overlay: a shut panel is `Display::None`.
fn profile_is_open(app: &mut App) -> bool {
    let world = app.world_mut();
    let mut panels = world.query_filtered::<&Node, With<ProfileOverlay>>();
    panels.iter(world).any(|node| node.display != Display::None)
}

/// **`Shift+Alt+Cmd+D` opens the frame profile and starts no report.**
#[test]
fn the_profiles_chord_opens_the_profile_alone() {
    let mut app = editor("one_press_profile");
    assert!(!profile_is_open(&mut app), "the profile starts shut");
    press(&mut app, chord(KeyCode::KeyD, true, true));
    assert!(profile_is_open(&mut app), "Shift+Alt+Cmd+D is the frame profile's");
    assert_eq!(
        app.world().resource::<ReportEdit>().phase(),
        None,
        "and it does not also start a report, which is what a second owner of the press did"
    );
}

/// **`Shift+Cmd+D` starts the project report and opens no profile.**
#[test]
fn the_project_reports_chord_starts_the_report_alone() {
    let mut app = editor("one_press_report");
    press(&mut app, chord(KeyCode::KeyD, true, false));
    assert_eq!(
        app.world().resource::<ReportEdit>().audience(),
        Some(Audience::Project),
        "Shift+Cmd+D is the project's report"
    );
    assert!(!profile_is_open(&mut app), "and it does not also draw the frame profile");
}

/// **`Cmd+D` is still the developers' report**, and the panel stays shut.
#[test]
fn the_developers_chord_starts_its_report_alone() {
    let mut app = editor("one_press_developers");
    press(&mut app, chord(KeyCode::KeyD, false, false));
    assert_eq!(
        app.world().resource::<ReportEdit>().audience(),
        Some(Audience::Developers),
        "Cmd+D is the developers' report"
    );
    assert!(!profile_is_open(&mut app), "and it does not also draw the frame profile");
}
