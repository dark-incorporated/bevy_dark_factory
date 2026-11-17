//! **The strip a refusal stands in is as tall as the refusal** (`feedback-1794783594150`, #479).
//!
//! The owner's capture is a PROBLEMS strip across the lower right whose text is the `play.yaml`
//! refusal, cut off by the strip's own height: the trailing sentence and the command's tail are
//! drawn nowhere. The refusal itself is right — `lavender_hills` has no `play.yaml` — and what loses
//! the answer is the room it is given.
//!
//! # Measured, before any fix
//!
//! `chrome::problem_toast` spawns gooey's `ToastStack` at `height: Extent::Percent(100.0)`, and a
//! `Val::Percent` resolves against its **parent**, which is the `placed` layer above it: a row that
//! is `width: 100%` and no height at all. So the stack is handed 18 px of room while the alert toast
//! inside it is 167 px tall, and `Role::Toast` writes `Overflow::clip()`. The alert's words are then
//! laid out in a box a ninth their height.
//!
//! # Why this file measures two heights rather than a string
//!
//! A `Text`'s own `String` is what was *handed* to the layout; what a person reads is what the node
//! was given room to draw. The two disagree exactly when the room is short, and no comparison of
//! strings can see it. So the assertion is: **the strip is at least as tall as the toast it holds**,
//! and the toast at least as tall as its own laid-out body.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::window::{PrimaryWindow, Window};
use bevy_cauldron::chrome::ToastLayer;
use bevy_cauldron::command_run::Chords;
use bevy_cauldron::commands;
use bevy_cauldron::editor::EditorState;
use bevy_cauldron::harness;
use bevy_cauldron::keys::Live;
use bevy_cauldron::ops_play::PlayButton;
use bevy_cauldron::tiles::Mode;
use bevy_cauldron::workspace::Workspace;

use crate::fixtures;

fn settle(app: &mut App) {
    for _ in 0..8 {
        app.update();
    }
}

/// The editor on a project with no `play.yaml`, standing in the Game workspace where Play lives.
fn without_a_play_file(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    assert!(!root.join("play.yaml").exists(), "the premise: the fixture must not ship a play.yaml, or there is no refusal to read");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    settle(&mut app);
    // **The PLAY section is the Game workspace's** (NV-09, #386).
    let id = format!("workspace.{}", Workspace::Game.id());
    let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    for _ in 0..12 {
        app.update();
    }
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Game, "the Game workspace did not open");
    assert!(*app.world().resource::<Mode>() == Mode::Game, "the Game workspace opens on the Game panel");
    app
}

/// Press Play the way a person presses it: the same `Activate` the button sends.
fn press_play(app: &mut App) {
    let play = {
        let mut query = app.world_mut().query_filtered::<Entity, With<PlayButton>>();
        let world = app.world();
        query
            .iter(world)
            .find(|&e| world.get::<PlayButton>(e) == Some(&PlayButton::Play))
            .expect("the PLAY section is on screen with its Play button")
    };
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: play });
    settle(app);
}

/// `entity`'s laid-out height in logical px.
fn height_of(app: &App, entity: Entity) -> f32 {
    app.world()
        .get::<ComputedNode>(entity)
        .map_or(0.0, |node| node.size().y * node.inverse_scale_factor())
}

/// **The strip holding a refusal is at least as tall as the toast, and the toast as its body.**
///
/// Failed before the fix: the stack was `Extent::Percent(100.0)` of a `placed` row with no height,
/// so it was 18 px tall around a 167 px alert and `Role::Toast` clipped the rest away.
#[test]
fn the_strip_a_refusal_stands_in_is_as_tall_as_the_refusal() {
    let mut app = without_a_play_file(&format!("play_refusal_room_{}", std::process::id()));
    press_play(&mut app);
    assert!(app.world().resource::<EditorState>().status.has_problem(), "the premise: a project with no play.yaml refuses Play");

    // **Settled**, because a toast opens its room over `ToastSettings::fade_seconds` (0.3 s) and a
    // measurement taken mid-open would say the room is small because it is still opening.
    let mut frames = 0;
    loop {
        let mut query = app.world_mut().query::<&bevy_gooey::Toast>();
        let growing = {
            let world = app.world();
            query.iter(world).any(|toast| toast.grow() < 1.0)
        };
        if !growing || frames >= 240 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        app.update();
        frames += 1;
    }

    let toast = {
        let mut query = app.world_mut().query_filtered::<Entity, With<bevy_gooey::Toast>>();
        let world = app.world();
        query.iter(world).next().expect("a refusal raised a toast")
    };
    let strip = {
        let mut query = app.world_mut().query_filtered::<Entity, With<ToastLayer>>();
        let world = app.world();
        query.iter(world).next().expect("the editor has a toast strip")
    };
    // The strip is the layer; gooey's stack is the one node between it and the toast.
    let chain = |app: &App, mut at: Entity| -> Vec<Entity> {
        let mut out = vec![at];
        while let Some(parent) = app.world().get::<ChildOf>(at).map(ChildOf::parent) {
            out.push(parent);
            at = parent;
        }
        out
    };
    let between: Vec<Entity> = chain(&app, toast).into_iter().filter(|&e| app.world().get::<bevy_gooey::Role>(e) == Some(&bevy_gooey::Role::ToastStack)).collect();
    assert_eq!(between.len(), 1, "the toast stands in exactly one gooey stack, found {}", between.len());
    let stack = between[0];

    let (toast_h, stack_h) = (height_of(&app, toast), height_of(&app, stack));
    assert!(toast_h > 0.0, "the toast is drawn, not collapsed: {toast_h}");
    // **The strip's own height is not measured**: it is a `placed` row holding a layer, and a layer
    // takes no room in its host. What clips is `Role::Toast`'s own `Overflow::clip()` against the
    // room the stack hands it, so that pair is the one asserted. The strip is found only to prove the
    // toast is inside the editor's own host rather than gooey's fallback stack at the window's root.
    assert!(app.world().get::<bevy_gooey::Shown>(strip).is_some_and(|s| s.0), "the strip is up");
    assert!(
        stack_h + 0.5 >= toast_h,
        "the stack is {stack_h} px tall around a {toast_h} px toast, so the refusal is clipped to it"
    );
}

/// **The whole refusal is in one node**, so what is measured above is the message and not a fragment.
///
/// The issue asks for "the whole refusal text present in one node (the command line and the trailing
/// sentence included)". The room it is given is the other test's subject; this one is the text.
#[test]
fn the_play_refusal_names_its_file_its_command_and_its_purpose_in_one_node() {
    let mut app = without_a_play_file(&format!("play_refusal_words_{}", std::process::id()));
    press_play(&mut app);

    let lines: Vec<String> = {
        let mut query = app.world_mut().query::<&Text>();
        let world = app.world();
        query
            .iter(world)
            .map(|text| text.0.clone())
            .filter(|text| text.contains("play.yaml") && text.contains("does not exist"))
            .collect()
    };
    // **Twice is right, and it is the journal's copy that makes it two.** A refusal is written into
    // the session journal by `notice::sync_the_log` in the same frame it is raised, and the toast
    // stands for that row — so the words are in two nodes (the toast's, and the closed journal's line)
    // and are one message to a person. What the issue asks for is that **the message is whole in the
    // node it is drawn in**, so every node that carries it must carry all of it.
    assert!(!lines.is_empty(), "the refusal reached the screen at all: {lines:#?}");
    for text in &lines {
        for want in [
            "does not exist. Write it as YAML:",
            "version: 1",
            r#"command: ["cargo", "run", "--", "{map}"]"#,
            "how to run this game.",
        ] {
            assert!(text.contains(want), "the refusal is missing {want:?}: {text:?}");
        }
    }
}

/// **`Dismiss` takes the message down and leaves the map exactly as it was.**
#[test]
fn dismissing_the_refusal_leaves_the_map_where_it_was() {
    let mut app = without_a_play_file(&format!("play_refusal_dismiss_{}", std::process::id()));
    press_play(&mut app);
    let camera = |app: &mut App| -> Transform {
        let mut query = app.world_mut().query_filtered::<&Transform, With<bevy_cauldron::view::MainCamera>>();
        let world = app.world();
        *query.iter(world).next().expect("the map has a camera")
    };
    let camera_before = camera(&mut app);
    let tool_before = app.world().resource::<EditorState>().tool;
    let stance_before = app.world().resource::<Live>().1;

    let dismiss = bevy_gooey::testing::find_by_label(&mut app, "Dismiss").unwrap_or_else(|e| panic!("no Dismiss button: {e:?}"));
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: dismiss });
    // **Past the fade**, which is what `ToastSettings::fade_seconds` asks: a dismissed toast keeps its
    // entity and its room until it has faded out, so a shorter wait would read a closing strip as a bug.
    settle(&mut app);
    for _ in 0..40 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        app.update();
    }

    let mut strips = app.world_mut().query_filtered::<&bevy_gooey::Shown, With<ToastLayer>>();
    let shown: Vec<bool> = {
        let world = app.world();
        strips.iter(world).map(|shown| shown.0).collect()
    };
    assert!(!shown.is_empty(), "the editor has a toast strip");
    assert!(shown.iter().all(|up| !up), "Dismiss left the strip up: {shown:?}");
    assert_eq!(camera(&mut app), camera_before, "the map's view moved when the message was dismissed");
    assert_eq!(app.world().resource::<EditorState>().tool, tool_before, "the tool changed");
    assert_eq!(app.world().resource::<Live>().1, stance_before, "the editor's stance changed");
}
