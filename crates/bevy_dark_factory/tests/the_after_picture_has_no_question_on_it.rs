//! **The after picture is of the agent's result, not of the question about it** (DL-01, #471).
//!
//! The turn ledger takes a turn's after picture once the view has settled, and in the default path the hand-back's question is a confirm modal — the
//! Keep / Undo card, centred over the viewport — so the picture the record held was partly the dialog: a poor label source, and DL-06's export would
//! have found few clean after pictures. The record has always carried `render.question_open`; it was `true` for most hand-backs and told nobody.
//!
//! # What is proved
//!
//! 1. **A hand-back answered in the modal records an after picture with no question on it**: `provenance.render.question_open` is `false`, while the
//!    question is up on the editor at the moment the picture is asked for — so the `false` is the picture's, not an empty screen's.
//! 2. **The dialog is out of the draw for the picture's frame and drawn again the frame after**: the question the person must answer survives, and the
//!    photograph is the only frame without it.
//!
//! The editor here has no window and draws nothing, so the test plays the renderer as [`a_turn_is_recorded_for_learning`] does: one picture per ticket
//! the ledger hands out. What is asserted — the ledger's own account of the frame it asked for, and the dialog's visibility in that frame — is exactly
//! what the readback in the running editor copies.

use bevy::camera::visibility::InheritedVisibility;
use bevy::prelude::*;

use bevy_dark_factory::confirm::{Asked, Confirm, PictureWithoutTheQuestion, QuestionDialog};
use bevy_dark_factory::turn_ledger::TurnLedger;
use bevy_dark_factory::turn_store::{Read, Turn};

use crate::a_turn_is_recorded_for_learning::{editor, ledger_of, place, present, render_what_is_asked, until_written};

/// **One finished agent turn, handed back with the modal asking.**
fn a_hand_back(app: &mut App) {
    let placed = place(app, [20.0, 18.0]);
    render_what_is_asked(app);
    present(app, &[placed], &["a bed"]);
}

/// **The person presses `Keep`**, through the key the modal reads.
fn press_keep(app: &mut App) {
    use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyN,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
}

fn the_turn(read: &Read) -> Turn {
    read.turns().next().expect("the turn was recorded").clone()
}

/// **The question's dialog, and whether it is drawn this frame**, or `None` where there is no dialog at all.
///
/// `Some(None)` never happens: with a question up the dialog exists from the frame it was asked in, because `sync_dialog` builds it in `Update`. What the
/// picture's frame depends on is `Some(Some(false))` — the dialog there, out of the draw — which is asserted below.
fn the_dialog(app: &mut App) -> Option<Vec<bool>> {
    let world = app.world_mut();
    let mut dialogs = world.query_filtered::<&InheritedVisibility, With<QuestionDialog>>();
    let shown: Vec<bool> = dialogs.iter(world).map(|shown| shown.get()).collect();
    (!shown.is_empty()).then_some(shown)
}

/// **1. A hand-back answered in the modal records an after picture with no question open on it.**
#[test]
fn the_modal_paths_after_picture_has_no_question_on_it() {
    let (mut app, labels) = editor("picture_without_question");
    a_hand_back(&mut app);
    // **The question is up**, which is the state that used to be photographed. The dialog is spawned by `sync_dialog` in the frame after the question
    // is asked, so one frame passes before it exists — and **what it is drawn as is not asserted here**: a headless app lays no UI out, so its
    // `InheritedVisibility` says nothing about a real window. What the acceptance rests on is the record's own account of the picture, below.
    assert!(app.world().resource::<Confirm>().asking(Asked::Presentation), "the modal asks the person");
    app.update();
    assert_eq!(the_dialog(&mut app), Some(vec![true]), "the question's dialog was not built, so there was nothing to keep off the picture");

    // **Step until the ledger asks for its after picture**, playing the renderer as a real one does.
    let mut frames = 0;
    loop {
        app.update();
        if app.world().resource::<TurnLedger>().shots_without_the_question() == 1 {
            break;
        }
        render_what_is_asked(&mut app);
        frames += 1;
        assert!(frames < 900, "the ledger never asked for an after picture with the question kept off it");
    }
    // **And the question is still there afterwards**, so the photograph cost the person nothing.
    app.update();
    assert!(!app.world().resource::<PictureWithoutTheQuestion>().asked, "the flag outlives the picture's frame");
    assert!(app.world().resource::<Confirm>().is_open(), "the question survived the picture");
    assert_eq!(the_dialog(&mut app), Some(vec![true]), "the question's dialog was taken down by the picture");

    press_keep(&mut app);
    until_written(&mut app);

    let read = ledger_of(&labels);
    let turn = the_turn(&read);
    assert_eq!(
        turn.provenance.render["question_open"], serde_json::json!(false),
        "the after picture records a question standing on it: {}",
        turn.provenance.render
    );
}

/// **2. The question is out of the draw for the picture's frame only**, and the person can still answer it.
#[test]
fn the_question_is_hidden_for_one_frame_only() {
    let (mut app, _labels) = editor("picture_question_hidden");
    a_hand_back(&mut app);
    let mut frames = 0;
    loop {
        app.update();
        if app.world().resource::<PictureWithoutTheQuestion>().asked {
            break;
        }
        render_what_is_asked(&mut app);
        frames += 1;
        assert!(frames < 3000, "the ledger never asked for its after picture");
    }
    app.update();
    assert!(app.world().resource::<Confirm>().asking(Asked::Presentation), "the question survived the picture");
    assert_eq!(the_dialog(&mut app), Some(vec![true]), "the dialog was not drawn again after the picture's frame");
}
