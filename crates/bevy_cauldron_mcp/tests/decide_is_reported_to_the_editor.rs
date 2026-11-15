//! **A `grimoire_decide` call is reported to the editor, which files it under the agent turn that is open** (DL-01, #460).
//!
//! The editor is the one writer of the turn ledger, and several MCP servers can run at once, so a server writes nothing: it tells the editor what
//! it asked and what came back. This proves the report's shape, which is what the ledger keeps; the editor's side (the turn it is filed under)
//! is `bevy_cauldron`'s `a_turn_is_recorded_for_learning`.

use bevy_cauldron_mcp::decide::{DecideParams, report};
use serde_json::json;

#[test]
fn a_report_carries_the_questions_the_answer_the_model_where_it_ran_and_how_long_it_took() {
    let params = DecideParams { check: Some("floating".to_owned()), ..DecideParams::default() };
    let answer = Ok(json!({"check": "floating", "finding": {"check": "floating", "p_yes": 0.02, "threshold": 0.7, "found": false}}));
    let said = report(&params, &answer, Some("clef-flash"), "http://127.0.0.1:8093/v1/systemone", 412);
    assert_eq!(said["params"]["check"], "floating");
    assert_eq!(said["answer"]["finding"]["found"], false);
    assert_eq!(said["error"], serde_json::Value::Null);
    assert_eq!(said["model"], "clef-flash");
    assert_eq!(said["endpoint"], "http://127.0.0.1:8093/v1/systemone");
    assert_eq!(said["latency_ms"], 412);
}

#[test]
fn a_refused_call_is_reported_with_its_reason_and_no_answer() {
    let params = DecideParams { check: Some("nonesuch".to_owned()), ..DecideParams::default() };
    let said = report(&params, &Err("no check named `nonesuch`".to_owned()), None, "http://127.0.0.1:8093/v1/systemone", 3);
    assert_eq!(said["answer"], serde_json::Value::Null);
    assert_eq!(said["error"], "no check named `nonesuch`");
    assert_eq!(said["model"], serde_json::Value::Null, "a model nobody named is not invented");
}
