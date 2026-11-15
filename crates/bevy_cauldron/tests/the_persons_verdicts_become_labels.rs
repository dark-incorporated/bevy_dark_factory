//! **What the person does about a turn becomes a label on it** (DL-02, #461; series labels from use, #459).
//!
//! The turn record (DL-01) is what the agent did. This is what the person made of it, appended to the same ledger as **new lines that name the
//! turn**: a record is never edited, a label is one more line.
//!
//! # What is proved
//!
//! 1. **Keep** and **Undo all** are gold labels (`kept`, `undone`), and **Dismiss** is no label: it is a line that says the hand-back was closed
//!    unread. An answer given before the turn's record is written waits, so a label never precedes the turn it is about.
//! 2. **A person's undo, or edit of the same pieces, after a Keep** is a gold `revised` label naming the operation that did it: the silent "kept,
//!    then fixed by hand" a Keep alone hides. A person's edit of pieces the turn did not touch is no label.
//! 3. **A snip over the turn's pieces** is a gold per-item label carrying the person's note and the region. Pieces no turn touched label nothing.
//! 4. **A later agent turn that takes the turn back** is a silver `superseded` label naming that turn.
//! 5. **Only a person's action is gold**: an agent's `operation_undo` over BRP takes a turn back and is no verdict.

use bevy::prelude::*;

use bevy_cauldron::ops;
use bevy_cauldron::present::{PresentationReply, Reply};
use bevy_cauldron::project::{OpenMap, Project};
use bevy_cauldron::turn_ledger::{REVISION_LOOK_FRAMES, TurnLedger};
use bevy_cauldron::turn_store::{Body, Label, Read};

use crate::a_turn_is_recorded_for_learning::{FRAMES_AT_MOST, call, editor, ledger_of, place, present, render_what_is_asked, until_written};

/// **The person presses a key**, through the key systems, as the modal reads it.
fn press(app: &mut App, code: KeyCode) {
    use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
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

/// **Cmd+Z**, as a real chord through the key systems: the person's own undo.
fn cmd_z(app: &mut App) {
    use bevy::input::ButtonState;
    use bevy::input::keyboard::{Key, KeyboardInput};
    let held = bevy_cauldron::keys::MOD_KEYS[0];
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(held);
    for state in [ButtonState::Pressed, ButtonState::Released] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyZ,
            logical_key: Key::Character("z".into()),
            state,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }
    app.update();
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(held);
    app.update();
}

/// **The person edits a piece with their own hand**: the operation runs as `Source::Human`, as a key's does, and the map is put back.
fn person_edit(app: &mut App, operation: &str, params: serde_json::Value) -> String {
    let mut ctx = ops::Ctx {
        view: None,
        trust_file: None,
        root: None,
        project: Some(app.world().resource::<Project>().clone()),
        open: Some(app.world().resource::<OpenMap>().clone()),
        source: ops::Source::Human,
        session: ops::session_id(),
        proposal: None,
    };
    let applied = ops::dispatch(&mut ctx, operation, &params).unwrap_or_else(|e| panic!("the person's {operation}: {e}"));
    let fresh = ctx.open.take().expect("the map comes back");
    *app.world_mut().resource_mut::<OpenMap>() = fresh;
    match applied {
        ops::Applied::Done { operation_id, .. } => operation_id,
        ops::Applied::Refused(why) => panic!("the person's {operation} was refused: {}", why.0),
    }
}

/// The placements the agent made, by id: every one but the fixture's own `bed@0`.
fn made(app: &App) -> Vec<String> {
    app.world().resource::<OpenMap>().map.placements.iter().map(|p| p.id.clone()).filter(|id| id != "bed@0").collect()
}

/// **A finished agent turn, handed back and not yet answered**: two beds. Returns the two operation ids.
fn a_turn(app: &mut App, at: ([f32; 2], [f32; 2])) -> Vec<String> {
    let first = place(app, at.0);
    render_what_is_asked(app);
    let second = place(app, at.1);
    let ids = vec![first, second];
    present(app, &ids, &["two beds"]);
    ids
}

fn labels_of(read: &Read) -> Vec<&Label> {
    read.labels().collect()
}

fn frames(app: &mut App, n: u32) {
    for _ in 0..n {
        app.update();
        render_what_is_asked(app);
    }
}

/// **1. Keep is a gold label, and it follows the turn it is about** even when the person answers before the record is written.
#[test]
fn keep_is_a_gold_label_that_waits_for_its_turn() {
    let (mut app, labels) = editor("verdict_kept");
    a_turn(&mut app, ([20.0, 18.0], [10.0, 12.0]));
    // **Answered at once**, while the record still waits for the view to settle.
    press(&mut app, KeyCode::KeyN);
    assert_eq!(app.world().resource::<TurnLedger>().written().len(), 0, "the record is not yet written when the person answers");
    until_written(&mut app);

    let read = ledger_of(&labels);
    assert!(read.damaged.is_empty(), "{:?}", read.damaged);
    let kinds: Vec<&str> = read.lines.iter().map(|(_, line)| match &line.body {
        Body::Turn(_) => "turn",
        Body::Label(_) => "label",
        Body::Dismissed(_) => "dismissed",
    }).collect();
    assert_eq!(kinds, ["turn", "label"], "the label follows the turn it names");
    let turn = read.turns().next().expect("a turn");
    let label = labels_of(&read)[0];
    assert_eq!((label.label.as_str(), label.tier.as_str()), ("kept", "gold"));
    assert_eq!(label.turn, turn.id, "the label names the turn");
    assert!(label.at_unix_ms >= turn.handed_back_unix_ms, "the label carries the time of the answer");
}

/// **1. Undo all is a gold label.**
#[test]
fn undo_all_is_a_gold_label() {
    let (mut app, labels) = editor("verdict_undone");
    a_turn(&mut app, ([20.0, 18.0], [10.0, 12.0]));
    press(&mut app, KeyCode::KeyY);
    until_written(&mut app);
    let read = ledger_of(&labels);
    let label = labels_of(&read)[0];
    assert_eq!((label.label.as_str(), label.tier.as_str()), ("undone", "gold"), "{label:?}");
    assert_eq!(label.turn, read.turns().next().expect("a turn").id);
}

/// **1. Dismiss is no label**: a line saying the hand-back was closed unread, and nothing that trains.
#[test]
fn dismiss_is_no_label() {
    let (mut app, labels) = editor("verdict_dismissed");
    a_turn(&mut app, ([20.0, 18.0], [10.0, 12.0]));
    app.world_mut().resource_mut::<PresentationReply>().0 = Some(Reply::Dismiss);
    app.update();
    until_written(&mut app);
    let read = ledger_of(&labels);
    assert_eq!(labels_of(&read).len(), 0, "a dismissal was recorded as a label: {:?}", labels_of(&read));
    let dismissed: Vec<_> = read.dismissals().collect();
    assert_eq!(dismissed.len(), 1);
    assert_eq!(dismissed[0].turn, read.turns().next().expect("a turn").id);
}

/// **2. A person's undo after a Keep is `revised`**, naming the undo.
#[test]
fn a_persons_undo_after_a_keep_is_a_revision() {
    let (mut app, labels) = editor("verdict_revised_undo");
    let ids = a_turn(&mut app, ([20.0, 18.0], [10.0, 12.0]));
    press(&mut app, KeyCode::KeyN);
    until_written(&mut app);
    frames(&mut app, 2 * REVISION_LOOK_FRAMES);
    assert_eq!(labels_of(&ledger_of(&labels)).len(), 1, "nothing was revised yet: only the Keep is a label");

    cmd_z(&mut app);
    frames(&mut app, 3 * REVISION_LOOK_FRAMES);
    let read = ledger_of(&labels);
    let all = labels_of(&read);
    assert_eq!(all.iter().map(|l| l.label.as_str()).collect::<Vec<_>>(), ["kept", "revised"], "{all:?}");
    let revised = all[1];
    assert_eq!(revised.tier, "gold");
    let by = revised.operation_id.as_deref().expect("a revision names the operation that did it");
    assert!(!ids.iter().any(|id| id == by), "the revising operation is the person's undo, not one of the agent's: {by}");
    assert!(revised.at_unix_ms >= all[0].at_unix_ms);
}

/// **2. A person's edit of the same pieces after a Keep is `revised`**, once.
#[test]
fn a_persons_edit_of_the_same_pieces_after_a_keep_is_a_revision() {
    let (mut app, labels) = editor("verdict_revised_edit");
    a_turn(&mut app, ([20.0, 18.0], [10.0, 12.0]));
    press(&mut app, KeyCode::KeyN);
    until_written(&mut app);
    let piece = made(&app).remove(0);

    let by = person_edit(&mut app, "placement_set_lift", serde_json::json!({"id": piece, "lift": 0.25}));
    frames(&mut app, 3 * REVISION_LOOK_FRAMES);
    let second = person_edit(&mut app, "placement_set_lift", serde_json::json!({"id": piece, "lift": 0.5}));
    frames(&mut app, 3 * REVISION_LOOK_FRAMES);

    let read = ledger_of(&labels);
    let revised: Vec<&Label> = labels_of(&read).into_iter().filter(|l| l.label == "revised").collect();
    assert_eq!(revised.len(), 1, "a turn is revised once: {revised:?}");
    assert_eq!(revised[0].operation_id.as_deref(), Some(by.as_str()), "the label names the first edit, not the second ({second})");
    assert_eq!(revised[0].tier, "gold");
}

/// **2. A person's edit of pieces the turn did not touch adds no label.**
#[test]
fn a_persons_edit_of_other_pieces_adds_no_label() {
    let (mut app, labels) = editor("verdict_untouched");
    a_turn(&mut app, ([20.0, 18.0], [10.0, 12.0]));
    press(&mut app, KeyCode::KeyN);
    until_written(&mut app);
    person_edit(&mut app, "placement_set_lift", serde_json::json!({"id": "bed@0", "lift": 0.25}));
    frames(&mut app, 4 * REVISION_LOOK_FRAMES);
    let read = ledger_of(&labels);
    assert_eq!(labels_of(&read).iter().map(|l| l.label.as_str()).collect::<Vec<_>>(), ["kept"], "an edit of the person's own piece labelled the agent's turn");
}

/// **3. A snip over the turn's pieces is a per-item label with the person's note and the region**; one over a piece no turn touched is none.
#[test]
fn a_snip_over_the_turns_pieces_is_an_item_label() {
    let (mut app, labels) = editor("verdict_snip");
    a_turn(&mut app, ([20.0, 18.0], [10.0, 12.0]));
    until_written(&mut app);
    let piece = made(&app).remove(0);
    let root = app.world().resource::<Project>().root.clone();

    let snip = |stem: &str, ids: &[&str]| bevy_grimoire_snip::SnipRecord {
        stem: stem.to_owned(),
        taken_at_ms: 1,
        map: "m".to_owned(),
        active_tool: "Select".to_owned(),
        note: "the bed is too low".to_owned(),
        rect_window_px: [10.0, 20.0, 110.0, 120.0],
        scale_factor: 1.0,
        rect_picture_px: [10, 20, 110, 120],
        size: [100, 100],
        picture: [800, 600],
        picture_file: format!("{stem}.png"),
        camera: None,
        cursor_world: None,
        placements: ids
            .iter()
            .map(|id| bevy_grimoire_snip::PlacedIn { id: (*id).to_owned(), world: [0.0; 3], stamp: None })
            .collect(),
    };
    {
        let mut ledger = app.world_mut().resource_mut::<TurnLedger>();
        ledger.snip_taken(&snip("snip-over-the-bed", &[piece.as_str(), "bed@0"]), &root);
        ledger.snip_taken(&snip("snip-over-nothing-of-the-turns", &["bed@0"]), &root);
    }
    let read = ledger_of(&labels);
    let items: Vec<&Label> = labels_of(&read).into_iter().filter(|l| l.label == "item").collect();
    assert_eq!(items.len(), 1, "only the snip over the turn's piece labels it: {items:?}");
    let item = items[0].item.as_ref().expect("the item");
    assert_eq!(items[0].tier, "gold");
    assert_eq!((item.source.as_str(), item.name.as_str()), ("snip", "snip-over-the-bed"));
    assert_eq!(item.note, "the bed is too low", "the person's words travel with the label");
    assert_eq!(item.pieces, [piece.as_str()], "only the turn's own pieces are named");
    assert_eq!(item.region["window_px"], serde_json::json!([10.0, 20.0, 110.0, 120.0]));
}

/// **4. A later agent turn that takes this one back is a silver `superseded` label** naming that turn.
#[test]
fn a_later_agent_turn_that_undoes_a_turn_supersedes_it() {
    let (mut app, labels) = editor("verdict_superseded");
    let ids = a_turn(&mut app, ([20.0, 18.0], [10.0, 12.0]));
    until_written(&mut app);
    // The person answers the first hand-back, so the agent's second may be asked.
    press(&mut app, KeyCode::KeyN);
    frames(&mut app, 4);

    // A second turn: the agent takes the first one's newest operation back, then hands its own work over.
    let undone = call(&mut app, "grimoire/operation_undo", serde_json::json!({"operation_id": ids[1], "dry_run": false}));
    assert_eq!(undone["outcome"], "applied", "{undone}");
    let third = place(&mut app, [5.0, 5.0]);
    render_what_is_asked(&mut app);
    present(&mut app, &[third], &["a bed instead"]);
    let mut waited = 0;
    while app.world().resource::<TurnLedger>().written().len() < 2 {
        app.update();
        render_what_is_asked(&mut app);
        waited += 1;
        assert!(waited < FRAMES_AT_MOST, "the second turn was not recorded");
    }

    let read = ledger_of(&labels);
    let turns: Vec<_> = read.turns().collect();
    assert_eq!(turns.len(), 2);
    let superseded: Vec<&Label> = labels_of(&read).into_iter().filter(|l| l.label == "superseded").collect();
    assert_eq!(superseded.len(), 1, "{:?}", labels_of(&read));
    assert_eq!(superseded[0].tier, "silver", "a later agent turn is a teacher's label, never gold");
    assert_eq!(superseded[0].turn, turns[0].id, "it is the earlier turn that was taken back");
    assert_eq!(superseded[0].by_turn.as_deref(), Some(turns[1].id.as_str()));
}

/// **5. Only a person's action is gold**: an agent that takes its own turn back over BRP has answered nothing.
#[test]
fn an_agents_undo_over_brp_is_no_verdict() {
    let (mut app, labels) = editor("verdict_agent");
    a_turn(&mut app, ([20.0, 18.0], [10.0, 12.0]));
    let took = call(&mut app, "grimoire/operation_undo", serde_json::json!({"turn": true, "dry_run": false}));
    assert_eq!(took["outcome"], "applied", "{took}");
    until_written(&mut app);
    frames(&mut app, 4 * REVISION_LOOK_FRAMES);
    let read = ledger_of(&labels);
    assert_eq!(labels_of(&read).len(), 0, "the agent's own undo was recorded as the person's verdict: {:?}", labels_of(&read));
    assert_eq!(read.dismissals().count(), 0);
}
