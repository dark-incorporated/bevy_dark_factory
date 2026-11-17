//! **A `blueprint_decide` call is reported to the editor, which files it under the agent turn that is open** (DL-01, #460).
//!
//! The editor is the one writer of the turn ledger, and several MCP servers can run at once, so a server writes nothing: it tells the editor what
//! it asked and what came back. This proves the report's shape, which is what the ledger keeps; the editor's side (the turn it is filed under)
//! is `bevy_dark_factory`'s `a_turn_is_recorded_for_learning`.

use dark_factory_mcp::decide::{DecideParams, report};
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

/// **The endpoint is kept without a key in it** (review of #460): userinfo, query and fragment are dropped; scheme, host, port and path stay.
#[test]
fn an_endpoint_is_reported_without_userinfo_query_or_fragment() {
    use dark_factory_mcp::decide::public_endpoint;
    assert_eq!(public_endpoint("https://user:secret@api.example.com:8443/v1/systemone?key=abc123#frag"), "https://api.example.com:8443/v1/systemone");
    assert_eq!(public_endpoint("http://127.0.0.1:8093/v1/systemone"), "http://127.0.0.1:8093/v1/systemone");
    assert_eq!(public_endpoint("https://token@host?x=1"), "https://host");
    let params = DecideParams { check: Some("floating".to_owned()), ..DecideParams::default() };
    let said = report(&params, &Ok(json!({})), None, "https://u:p@host/v1?key=k", 1);
    assert_eq!(said["endpoint"], "https://host/v1");
}

/// **An error is bounded** (review of #460): a long one is cut, and says so.
#[test]
fn a_long_error_is_cut_and_says_so() {
    use dark_factory_mcp::decide::ERROR_CAP_CHARS;
    let params = DecideParams { check: Some("floating".to_owned()), ..DecideParams::default() };
    let long = "é".repeat(ERROR_CAP_CHARS * 3);
    let said = report(&params, &Err(long.clone()), None, "http://h/", 1);
    let kept = said["error"].as_str().expect("an error");
    assert!(kept.chars().count() < ERROR_CAP_CHARS + 60, "{}", kept.chars().count());
    assert!(kept.starts_with(&long[..ERROR_CAP_CHARS * 2]) && kept.ends_with("characters)"), "{kept}");
}

/// **An error that names the endpoint carries no key** (review of #460): `bevy_oracle::Error` words the address as it was asked, `?key=` and all,
/// and so can a response body that quotes another.
#[test]
fn an_error_that_names_the_endpoint_carries_no_key() {
    let params = DecideParams { check: Some("floating".to_owned()), ..DecideParams::default() };
    let endpoint = "https://user:hunter2@api.example.com/v1/decide?key=SECRET123";
    let said = report(
        &params,
        &Err(format!(
            "decision endpoint {endpoint} refused the request with status 401: {{\"error\":\"bad key at https://other.example.com/x?token=TOKEN456.\"}}"
        )),
        None,
        endpoint,
        1,
    );
    let kept = said["error"].as_str().expect("an error");
    for secret in ["hunter2", "SECRET123", "TOKEN456", "?key=", "?token="] {
        assert!(!kept.contains(secret), "`{secret}` is in the stored error: {kept}");
    }
    assert!(kept.contains("https://api.example.com/v1/decide"), "the address is still named, without its key: {kept}");
}
