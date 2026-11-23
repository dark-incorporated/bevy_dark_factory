//! **The rig bench's list does not show by default, and `Cmd`/`Ctrl`+`R` opens it** (#522, `feedback-1795094222660`).
//!
//! # What the owner saw
//!
//! *"I'd rather this not show by default, but a keyboard shortcut (Cmd+some key) to show here so it may be opened quickly."* The capture's
//! pick is the Animation workspace's right-hand column, `rect_px` `[1767, 335, 2147, 1315]` — 380 by 980 physical px, the whole RIGS
//! panel, 264 logical px wide at `ui_scale` 2.4. It was drawn on every visit to the workspace, whether or not the person was choosing a rig.
//!
//! # What this file holds down
//!
//! 1. **The list is not drawn at boot**: its column is `Display::None` and lays out to nothing, so the viewport has the width back.
//! 2. **The chord opens it, and the chord closes it again** — the same key both ways, measured on the drawn node.
//! 3. **The chord is stated where the panel used to be**: the RIGS heading carries it, which is the house rule for a control that can
//!    vanish. A person who has never pressed `Cmd+R` reads what opens the list.
//! 4. **The chord is in no other binding's way** — the census's own collision test covers the table, and this file pins the chord itself so a
//!    future rebind that takes `Cmd+R` fails here by name.
//!
//! The assertions are on the drawn nodes: the `Node::display` the layout reads and the box that follows from it.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy_dark_factory::anim_tab::{RigBench, RigToggleHere, rigs_toggle_words};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::{self, Action, BINDINGS, Context, Live, MOD_NAME};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;
use crate::settle_support;

/// **Frames the editor is given to settle** after a workspace switch: the transition runs the frame after the key and the panels are spawned

/// The editor on the fixture project, settled, in the Animation workspace.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("rigs_hidden_{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    switch_to_animation(&mut app);
    app
}



/// **Enter the Animation workspace through its command**, the way the palette does, so the test reads the workspace a person reaches with
/// `Cmd+5` and not a state it poked in.
fn switch_to_animation(app: &mut App) {
    let id = format!("workspace.{}", Workspace::Animation.id());
    let command = bevy_dark_factory::commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<bevy_dark_factory::command_run::Chords>().queue(&command, live);
    settle_support::settle(app);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Animation, "the command did not open the Animation workspace");
}

/// **Press a chord** the way a person does: a `KeyboardInput` for the modifier and the key, then their releases. The census reads
/// `just_pressed` from `ButtonInput`, which the input plugin fills from these messages — writing the messages is what the editor's other
/// key tests do, and pressing `ButtonInput` directly would be cleared at the head of the next frame.
fn press(app: &mut App, action: Action) {
    let binding = keys::binding(action);
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
    for key in keys::MOD_KEYS {
        tap(app, key, bevy::input::ButtonState::Pressed);
    }
    tap(app, binding.key, bevy::input::ButtonState::Pressed);
    tap(app, binding.key, bevy::input::ButtonState::Released);
    for key in keys::MOD_KEYS {
        tap(app, key, bevy::input::ButtonState::Released);
    }
}

/// **Is the rig list drawn?** Its column's `display`, and the box the layout gave it.
fn drawn(app: &mut App) -> (Display, Option<Rect>) {
    let world = app.world_mut();
    let entity = world
        .query_filtered::<Entity, With<RigBench>>()
        .iter(world)
        .next()
        .expect("the Animation workspace builds the rig bench's column");
    let display = world.get::<Node>(entity).map(|node| node.display).expect("the column is a layout node");
    let box_ = world.get::<ComputedNode>(entity).map(|node| {
        let at = world.get::<UiGlobalTransform>(entity).map(|transform| transform.translation).unwrap_or_default();
        Rect::from_center_size(at, node.size())
    });
    (display, box_)
}

/// **The list is put away when the workspace opens, and it costs the viewport no width.**
#[test]
fn the_rig_list_is_not_drawn_at_boot() {
    let mut app = editor("boot");
    let (display, box_) = drawn(&mut app);
    assert_eq!(display, Display::None, "the rig bench's list is drawn when the Animation workspace opens");
    assert!(
        box_.is_none_or(|b| b.width() <= 0.0 || b.height() <= 0.0),
        "a list that is `Display::None` must lay out to nothing, but it has a box {box_:?}"
    );
}

/// **The chord draws the list, and the chord again puts it away.** One key, both directions, on the drawn node.
#[test]
fn the_chord_shows_the_list_and_the_chord_hides_it_again() {
    let mut app = editor("chord");
    assert_eq!(drawn(&mut app).0, Display::None, "the list starts put away");
    press(&mut app, Action::ToggleRigs);
    settle_support::settle(&mut app);
    let (display, box_) = drawn(&mut app);
    assert_eq!(display, Display::Flex, "the chord did not draw the list");
    assert!(box_.is_some_and(|b| b.width() > 0.0 && b.height() > 0.0), "the drawn list has no box: {box_:?}");
    // And away again.
    press(&mut app, Action::ToggleRigs);
    settle_support::settle(&mut app);
    assert_eq!(drawn(&mut app).0, Display::None, "the chord did not put the list away again");
}

/// **The panel says how it comes back.** The Animation workspace's left panel carries the sentence in both states, so a person who has
/// never pressed the chord reads what opens the bench — the house rule for a control that can vanish.
#[test]
fn the_workspace_states_the_chord_in_both_states() {
    let mut app = editor("stated");
    let says = |app: &mut App| -> String {
        let world = app.world_mut();
        world
            .query_filtered::<&Text, With<RigToggleHere>>()
            .iter(world)
            .next()
            .map(|text| text.0.clone())
            .expect("the left panel states the chord")
    };
    let closed = says(&mut app);
    assert!(closed.contains(MOD_NAME), "the sentence names the modifier {MOD_NAME:?}: {closed:?}");
    assert!(closed.contains('R'), "the sentence names the key: {closed:?}");
    assert!(closed.to_lowercase().contains("bench"), "the sentence says what the chord shows: {closed:?}");
    assert_eq!(closed, rigs_toggle_words(), "the sentence and the census name one chord");
    // **And it is still there with the bench drawn**, which is the whole point: it says how the bench comes back, and it is on screen
    // whenever the workspace is.
    press(&mut app, Action::ToggleRigs);
    settle_support::settle(&mut app);
    assert_eq!(says(&mut app), closed, "the sentence changed when the bench was drawn; it has to read in both states");
    assert_eq!(drawn(&mut app).0, Display::Flex, "the bench is drawn");
}

/// **The chord is `Cmd`/`Ctrl`+`R`, and the census has it once, on the Anim tab, in no other context's way.**
///
/// The collisions over the whole table are `keys::the_key_space_has_no_collisions`'s; this pins *which* chord the owner's shortcut is, so a
/// future row that takes `Cmd+R` fails here by name rather than only in a wall of table.
#[test]
fn the_chord_is_command_r_and_no_other_binding_takes_it() {
    let mine = keys::binding(Action::ToggleRigs);
    assert_eq!(mine.key, KeyCode::KeyR, "the list's chord is not R");
    assert!(mine.needs_mod, "the list's chord does not take the command modifier");
    assert_eq!(mine.context, Context::Anim, "the list belongs to the Animation workspace");
    let clashes: Vec<&str> = BINDINGS
        .iter()
        .filter(|b| b.action != Action::ToggleRigs && b.key == mine.key && b.needs_mod == mine.needs_mod && b.context.overlaps(mine.context))
        .map(|b| b.does)
        .collect();
    assert!(clashes.is_empty(), "another binding takes {MOD_NAME}+R where the rig list is live: {clashes:?}");
}
