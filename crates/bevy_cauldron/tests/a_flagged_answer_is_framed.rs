//! **A turn in which an assessor flagged a problem and a later operation fixed it is written with its two frames and as a pair** (DL-05, #464;
//! series labels from use, #459).
//!
//! The ledger (DL-01) files each `grimoire_decide` answer under the turn that is open. For an answer that flags a problem, or one that clears a
//! problem flagged earlier in the turn, it also takes the picture the editor is drawing, and files it under the answer's `frame`; and it stamps
//! every answer with how many of the turn's operations were behind it. When the turn is written, an assess-then-fix pair is written beside it
//! as a line of its own. The pair logic itself is proved with no editor in `crates/bevy_cauldron_bench/tests/negatives_on_purpose.rs`; this
//! proves the editor's side of it, through the bridge an agent's server reports over.
//!
//! The editor here has no window and so draws nothing: the test plays the renderer, as `a_turn_is_recorded_for_learning` does.

use bevy_cauldron::turn_ledger::TurnLedger;
use bevy_cauldron::turn_store::Polarity;

use crate::a_turn_is_recorded_for_learning::{call, editor, ledger_of, place, present, render_what_is_asked, until_written};

fn answer(found: bool) -> serde_json::Value {
    serde_json::json!({
        "params": {"check": "floating"},
        "answer": {"check": "floating", "finding": {"check": "floating", "p_yes": if found { 0.93 } else { 0.03 }, "threshold": 0.7, "found": found}},
        "model": "clef-flash",
        "endpoint": "http://127.0.0.1:8093/v1/systemone",
        "latency_ms": 300,
    })
}

/// **A flagged answer is framed, the fix that follows is the pair's work, and the clearing answer is framed too.**
#[test]
fn a_flagged_then_cleared_check_is_two_frames_and_one_pair() {
    let (mut app, labels) = editor("assessfix");
    let first = place(&mut app, [20.0, 18.0]);
    render_what_is_asked(&mut app);
    assert_eq!(app.world().resource::<TurnLedger>().shots_requested(), 1, "the before picture");

    // **The check flags a problem**: the editor takes a picture of what it is drawing, for the frame the answer was about.
    let said = call(&mut app, "grimoire/decide_report", answer(true));
    assert_eq!(said["recorded"], true, "{said}");
    assert_eq!(app.world().resource::<TurnLedger>().shots_requested(), 2, "a flag asks for a frame");
    render_what_is_asked(&mut app);

    // The agent fixes it, and asks again: the answer that clears a flag is framed too.
    let second = place(&mut app, [10.0, 12.0]);
    let said = call(&mut app, "grimoire/decide_report", answer(false));
    assert_eq!(said["recorded"], true, "{said}");
    assert_eq!(app.world().resource::<TurnLedger>().shots_requested(), 3, "the answer that clears a flag asks for a frame");
    render_what_is_asked(&mut app);

    present(&mut app, &[first.clone(), second.clone()], &["a bed to the east"]);
    until_written(&mut app);

    let read = ledger_of(&labels);
    assert!(read.damaged.is_empty(), "{:?}", read.damaged);
    let turn = read.turns().next().expect("a turn");
    // Each answer carries how much of the turn's work stood behind it, and the two framed ones carry their frame.
    assert_eq!(turn.decisions.len(), 2);
    assert_eq!(turn.decisions[0]["after_operations"], 1);
    assert_eq!(turn.decisions[1]["after_operations"], 2);
    let flagged_frame = turn.decisions[0]["frame"]["image"]["sha256"].as_str().unwrap_or_else(|| panic!("no frame on the flagged answer: {}", turn.decisions[0]));
    let cleared_frame = turn.decisions[1]["frame"]["image"]["sha256"].as_str().unwrap_or_else(|| panic!("no frame on the clearing answer: {}", turn.decisions[1]));
    assert_ne!(flagged_frame, cleared_frame, "two frames of one turn are two pictures");

    // **The pair is a line of its own, written with the turn**: one negative and one positive, silver.
    let pairs: Vec<_> = read.assess_fixes().collect();
    assert_eq!(pairs.len(), 1, "{:?}", read.lines.iter().map(|(_, l)| &l.body).collect::<Vec<_>>());
    let pair = pairs[0];
    assert_eq!(pair.source_turn, turn.id);
    assert_eq!(pair.check, "floating");
    assert_eq!(pair.fixing_operations, [second.as_str()], "the operation between the two answers is the fix");
    let examples = read.examples();
    assert_eq!(examples.iter().filter(|e| e.polarity == Polarity::Negative).count(), 1);
    assert_eq!(examples.iter().filter(|e| e.polarity == Polarity::Positive).count(), 1);
    assert_eq!(examples.iter().find(|e| e.polarity == Polarity::Negative).and_then(|e| e.frame.image.as_ref()).map(|i| i.sha256.as_str()), Some(flagged_frame));
    assert_eq!(examples.iter().find(|e| e.polarity == Polarity::Positive).and_then(|e| e.frame.image.as_ref()).map(|i| i.sha256.as_str()), Some(cleared_frame));
    assert!(examples.iter().all(|e| e.tier == "silver"));
}

/// **A turn that was flagged and never fixed writes no pair**, and an answer that flags nothing asks for no picture.
#[test]
fn a_flag_that_is_never_cleared_writes_no_pair() {
    let (mut app, labels) = editor("unfixed");
    let first = place(&mut app, [20.0, 18.0]);
    render_what_is_asked(&mut app);
    call(&mut app, "grimoire/decide_report", answer(false));
    assert_eq!(app.world().resource::<TurnLedger>().shots_requested(), 1, "an answer that flags nothing asks for no frame");
    call(&mut app, "grimoire/decide_report", answer(true));
    render_what_is_asked(&mut app);
    present(&mut app, &[first], &["a bed"]);
    until_written(&mut app);
    let read = ledger_of(&labels);
    assert_eq!(read.assess_fixes().count(), 0);
    assert_eq!(read.lines.len(), 1, "only the turn");
}
