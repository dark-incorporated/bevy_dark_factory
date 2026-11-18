//! **A refusal is drawn as tall as its text, in one node, and its strip says what it is** (#479).
//!
//! Found in a person's capture of the Map door (`feedback-1794783594150`, DarkFactory 0.6.8, mac_air, 2560 x 1406 at `ui_scale` 2.4,
//! map `deadite_caverns`, 2026-11-15 16:59): *"What's going on here?"*. The frame shows a PROBLEMS strip across the lower right whose
//! text is the `play.yaml` refusal — a paragraph of seven lines — cut off by the strip's own height, so the trailing sentence
//! (`play.yaml: how to run this game.`) and the tail of the `command:` line are drawn nowhere.
//!
//! The refusal itself is correct: a project with no `play.yaml` has no way to play, and `blueprint_model::play::PlayConfig::load` says
//! so with the YAML a person should write. What the capture is about is **the room the answer is given**.
//!
//! What is held down here:
//!
//! 1. the whole refusal — its sentence naming the file, `version: 1`, the whole `command:` line and the trailing sentence — is present
//!    **in one node**, so nothing can wrap it into a second node that is clipped;
//! 2. the strip the refusal stands in is at least as tall as the refusal it holds, and the toast at least as tall as its own laid-out
//!    body: a `Text`'s `String` is what was handed to the layout, and what a person reads is what the node was given room to draw;
//! 3. Dismissing the refusal takes the strip down and leaves the map exactly where it was.
//!
//! The last measured detail is a number rather than a sentence, because this defect has been "fixed" once already by a change that did
//! not move it: the strip was 18 px tall around a 167 px alert, from `Extent::Percent(100.0)` resolving against a parent that has no
//! height.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::ComputedNode;
use bevy::window::{PrimaryWindow, Window};

use bevy_dark_factory::chrome::ProblemBanner;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::ops_play::PlayButton;
use bevy_dark_factory::tiles::Mode;
use bevy_dark_factory::view::MainCamera;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// Frames a layout and a toast need to settle.
const SETTLE_FRAMES: usize = 8;

/// How far a measured edge may stand past the one it is held to, physical px: layout rounds.
const SLACK_PX: f32 = 1.0;

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// The editor on a project with no `play.yaml`, standing in the Game workspace where Play lives.
fn without_a_play_file(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    assert!(
        !root.join("play.yaml").exists(),
        "the premise: the fixture must not ship a play.yaml, or there is no refusal to read"
    );
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
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<PlayButton>>();
        query
            .iter(world)
            .find(|&e| world.get::<PlayButton>(e) == Some(&PlayButton::Play))
            .expect("the PLAY section is on screen with its Play button")
    };
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: play });
    settle(app);
}

/// A node's laid-out height in logical px.
fn height_of(app: &App, entity: Entity) -> f32 {
    app.world()
        .get::<ComputedNode>(entity)
        .map_or(0.0, |node| node.size().y * node.inverse_scale_factor())
}

/// **The whole refusal is one node's text, and the strip is tall enough to draw it.**
///
/// Failed before the fix: the `ToastStack` was `Extent::Percent(100.0)` of a `placed` row with no height, so it was handed 18 px of room
/// around a 167 px alert, and `bevy_gooey`'s `Role::Toast` writes `Overflow::clip()` — the seven-line refusal was drawn three lines and
/// cut. The text itself was always whole; the room was not.
#[test]
fn the_play_refusal_is_one_node_and_its_strip_is_tall_enough_to_draw_it() {
    let mut app = without_a_play_file("play_refusal_room");
    press_play(&mut app);

    // The refusal's own words, as one node holds them.
    let banners: Vec<(Entity, String)> = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<ProblemBanner>>()
            .iter(world)
            .filter_map(|e| world.get::<Text>(e).map(|t| (e, t.0.clone())))
            .collect()
    };
    let world = app.world();
    assert_eq!(banners.len(), 1, "exactly one node is marked as the refusal's text, and {} are", banners.len());
    let (banner, words) = &banners[0];
    for part in [
        "does not exist",
        "version: 1",
        "command: [\"cargo\", \"run\", \"--\", \"{map}\"]",
        "how to run this game",
    ] {
        assert!(
            words.contains(part),
            "the refusal names {part:?} and the node says {words:?}: the whole answer has to be in one text, because a second \
             node is one the strip can clip on its own"
        );
    }

    // **The room the words were given.** A text's `String` is what was handed to the layout; `TextLayoutInfo` is what was drawn.
    let laid_out = world
        .get::<TextLayoutInfo>(*banner)
        .map(|info| info.size.y * world.get::<ComputedNode>(*banner).map_or(1.0, ComputedNode::inverse_scale_factor))
        .unwrap_or(0.0);
    assert!(laid_out > 0.0, "the refusal is laid out at all; its glyph box is {laid_out} px tall");
    let banner_box = world
        .get::<ComputedNode>(*banner)
        .map(|node| node.size().y * node.inverse_scale_factor())
        .unwrap_or(0.0);
    assert!(
        banner_box + SLACK_PX >= laid_out,
        "the refusal's box is {banner_box:.1} px tall and its words are {laid_out:.1} px: the node cannot clip its own text"
    );

    // **And the strip it stands in is as tall as the refusal.** This is the measured defect: gooey's `ToastStack` is what the toasts
    // are laid out in, and `Role::Toast` clips each one to the room the stack hands it. The layer around it is a hidden wrapper with no
    // height of its own by design, so the stack is the box that has to be tall.
    let stack = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<bevy_gooey::ToastStack>>()
            .iter(world)
            .next()
            .unwrap_or_else(|| panic!("the toast stack is the box every refusal stands in"))
    };
    let stack_height = height_of(&app, stack);
    assert!(
        stack_height >= laid_out,
        "the strip a refusal stands in is {stack_height:.1} px tall around a {laid_out:.1} px refusal, so the answer is drawn cut"
    );
}

/// **Dismissing the refusal takes the strip down and leaves the map exactly where it was.**
#[test]
fn dismissing_the_refusal_leaves_the_map_where_it_was() {
    let mut app = without_a_play_file("play_refusal_dismiss");
    press_play(&mut app);
    let camera = |app: &mut App| -> Option<(f32, f32, f32)> {
        let world = app.world_mut();
        world
            .query_filtered::<&Transform, With<MainCamera>>()
            .iter(world)
            .next()
            .map(|t| (t.translation.x, t.translation.y, t.translation.z))
    };
    let camera_before = camera(&mut app);
    let tool_before = app.world().resource::<bevy_dark_factory::editor::EditorState>().brush;
    let refusals_before = app.world_mut().query_filtered::<Entity, With<ProblemBanner>>().iter(app.world()).count();
    assert!(refusals_before >= 1, "the refusal is up before Dismiss is pressed");

    // **The Dismiss button, activated** — the same `Activate` a click and Enter send. Pressing the toast's `ToastAction` directly
    // would not do: gooey's `toast_on_activate` is what marks a toast dismissed, and it runs on the *button*, not the action.
    let dismiss = {
        let world = app.world_mut();
        let mut buttons = world.query_filtered::<(Entity, &bevy_gooey::Role), ()>();
        buttons
            .iter(world)
            .find(|(_, role)| **role == bevy_gooey::Role::ToastAction)
            .map(|(entity, _)| entity)
            .expect("the sticky refusal toast carries a Dismiss button")
    };
    app.world_mut().trigger(bevy_gooey::Activate { entity: dismiss });
    // A toast fades on `Time<Real>`, so the frames have to be given real time to pass in: eight frames of a headless update
    // take microseconds, and the toast would still be standing in its first fade step.
    for _ in 0..SETTLE_FRAMES {
        std::thread::sleep(std::time::Duration::from_millis(50));
        app.update();
    }

    let camera_after = camera(&mut app);
    let tool_after = app.world().resource::<bevy_dark_factory::editor::EditorState>().brush;
    assert_eq!(
        camera_before, camera_after,
        "Dismiss moved the camera: it is a message about a project file, not a view command"
    );
    assert_eq!(tool_before, tool_after, "Dismiss changed the armed tool");
    // **The refusal is gone.** The strip itself stays standing (an empty layer the editor keeps for the next refusal), and it is
    // `Pickable::IGNORE`, so what matters is that the message went and the map behind it did not move.
    let refusals_after = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<ProblemBanner>>().iter(world).count()
    };
    let toasts = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_gooey::Toast>>().iter(world).count()
    };
    assert_eq!(
        (refusals_after, toasts),
        (0, 0),
        "Dismiss left the refusal on screen: {refusals_after} marked texts and {toasts} toasts"
    );
}
