//! **An agent can answer its own hand-back, and that answer is not the person's verdict** (DL-02, #470).
//!
//! The turn ledger records a Keep, Undo all or Dismiss as a **gold** label, and the series says only a person's own action may be gold. It could not
//! tell whose hands pressed: the answer reached `present::answer_presentation` as a modal answer or a `PresentationReply`, and both are set from key
//! and pointer input with no source attached. So an agent that put its own result to the person and then answered it through `bevy_fairy/input`'s
//! injected cursor got a gold `kept` on the turn, from a label that says a person judged it.
//!
//! # What is proved
//!
//! 1. **An injected pointer marks its frame as an agent's**, which is the choke point `bevy_fairy/input` crosses and a person's own mouse never does.
//! 2. **An answer that arrives in such a frame is no gold label**: the hand-back is still answered — the result is kept, which is what the hand-back
//!    asked — and the ledger records no verdict, so the training set learns nothing from a judgement nobody made.
//! 3. **A person's own keypress is still a gold `kept`**, which is what makes the first two mean something; and **a chord an agent asked for marks its
//!    own frame** (`grimoire/run_command`), so an `edit.undo` run by an agent is the agent's too.
//!
//! **The pixel route is not driven here.** A headless editor draws no laid-out dialog for a synthetic click to land on, so the path from the injected
//! cursor through the dialog's button is owed a test on a host with a window, and is recorded on #470 as what remains. What decides the label — whose
//! hands the frame holds, and what `answer_presentation` does with it — is proved here.

use bevy::input::ButtonState;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowResolution};

use bevy_cauldron::hands::Hands;
use bevy_cauldron::ops::Source;
use bevy_cauldron::present::{PresentationReply, Reply};
use bevy_cauldron::turn_store::{Label, Read};

use crate::a_turn_is_recorded_for_learning::{editor, ledger_of, place, present, render_what_is_asked, until_written};

fn labels_of(read: &Read) -> Vec<&Label> {
    read.labels().collect()
}

/// **The person presses a key**, through the key systems, as the modal reads it.
fn press(app: &mut App, code: KeyCode) {
    use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

/// **A finished agent turn, handed back with the modal asking.**
fn a_hand_back(app: &mut App) {
    let placed = place(app, [20.0, 18.0]);
    render_what_is_asked(app);
    present(app, &[placed.clone()], &["a bed"]);
}

/// **An agent takes the pointer**, exactly as `bevy_fairy/input`'s `Cursor` does before it clicks anything.
fn an_agent_takes_the_pointer(app: &mut App) {
    app.world_mut().spawn((Window { resolution: WindowResolution::new(1280, 720), ..default() }, PrimaryWindow));
    app.world_mut().resource_mut::<bevy_debugger_bevy::DebugCursor>().0 = Some(Vec2::new(400.0, 300.0));
    for _ in 0..3 {
        app.update();
        render_what_is_asked(app);
    }
}

/// **1. An injected pointer marks the frame as an agent's**, and a person's own mouse never does.
#[test]
fn an_injected_pointer_marks_the_frame_as_an_agents() {
    let (mut app, _labels) = editor("hands_agent");
    app.world_mut().spawn((Window { resolution: WindowResolution::new(1280, 720), ..default() }, PrimaryWindow));
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().resource::<Hands>().are_the_persons(), "no injected pointer, so the frame is the person's");
    app.world_mut().resource_mut::<bevy_debugger_bevy::DebugCursor>().0 = Some(Vec2::new(400.0, 300.0));
    for _ in 0..2 {
        app.update();
    }
    // **A button edge while the injected cursor owns the pointer** is the press `bevy_fairy/input` makes, and `surface::inject_clicks` marks it.
    app.world_mut().write_message(bevy::input::mouse::MouseButtonInput {
        button: bevy::input::mouse::MouseButton::Left,
        state: ButtonState::Pressed,
        window: Entity::PLACEHOLDER,
    });
    app.update();
    assert_eq!(
        app.world().resource::<Hands>().source(),
        Source::Agent,
        "an agent's press did not mark its frame, so no reader downstream could tell it from a person's"
    );
    app.world_mut().write_message(bevy::input::mouse::MouseButtonInput {
        button: bevy::input::mouse::MouseButton::Left,
        state: ButtonState::Released,
        window: Entity::PLACEHOLDER,
    });
    for _ in 0..4 {
        app.update();
    }
    assert!(app.world().resource::<Hands>().are_the_persons(), "the agent's mark outlived its press");
}

/// **2. An Accept that arrives in an agent's frame is no gold label**, and the hand-back is still answered.
#[test]
fn a_brp_injected_accept_is_not_a_gold_label() {
    let (mut app, labels) = editor("verdict_agent_click");
    a_hand_back(&mut app);
    assert!(app.world().resource::<bevy_cauldron::confirm::Confirm>().is_open(), "the hand-back's question is on screen");
    an_agent_takes_the_pointer(&mut app);
    assert_eq!(app.world().resource::<Hands>().source(), Source::Agent, "the agent is driving the frame the answer lands in");

    // **The answer the agent's press produces**, handed over in a frame an agent is driving.
    app.world_mut().resource_mut::<PresentationReply>().0 = Some(Reply::Accept);
    app.update();
    until_written(&mut app);

    let read = ledger_of(&labels);
    let kept: Vec<&Label> = labels_of(&read).into_iter().filter(|label| label.label == "kept" && label.tier == "gold").collect();
    assert!(kept.is_empty(), "an agent's own Accept was recorded as the person's gold verdict: {:?}", labels_of(&read));
    assert!(
        app.world().resource::<bevy_cauldron::present::Presenting>().0.as_ref().is_some_and(|presented| presented.answer.is_some()),
        "the hand-back is still answered: the agent reads what happened from grimoire/state"
    );
}

/// **3. A person's own press is a gold `kept`**, which is what makes the test above mean something.
#[test]
fn a_persons_keypress_is_still_a_gold_label() {
    let (mut app, labels) = editor("verdict_person_key");
    a_hand_back(&mut app);
    assert!(app.world().resource::<Hands>().are_the_persons(), "the person's own frame");
    press(&mut app, KeyCode::KeyN);
    until_written(&mut app);
    let read = ledger_of(&labels);
    let kept: Vec<&Label> = labels_of(&read).into_iter().filter(|label| label.label == "kept").collect();
    assert_eq!(kept.len(), 1, "the person's Keep was not recorded: {:?}", labels_of(&read));
    assert_eq!(kept[0].tier, "gold");
}

/// **4. A chord an agent asked for marks its own frame**, which is `grimoire/run_command`'s path (`edit.undo` run by an agent).
#[test]
fn an_agents_chord_marks_the_frame() {
    let (mut app, _labels) = editor("verdict_agent_chord");
    for _ in 0..2 {
        app.update();
    }
    assert!(app.world().resource::<Hands>().are_the_persons(), "with no agent driving, the frame is the person's");
    let live = *app.world().resource::<bevy_cauldron::keys::Live>();
    let command = bevy_cauldron::commands::by_id("edit.undo").expect("edit.undo is a command");
    app.world_mut()
        .resource_mut::<bevy_cauldron::command_run::Chords>()
        .queue_from(&command, live, Source::Agent);
    app.update();
    assert_eq!(app.world().resource::<Hands>().source(), Source::Agent, "an agent's chord did not mark its own frame");
    for _ in 0..3 {
        app.update();
    }
    assert!(app.world().resource::<Hands>().are_the_persons(), "the agent's mark outlived its chord");
}
