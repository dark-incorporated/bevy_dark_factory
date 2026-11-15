//! **OpenRouter Decisions as the second protocol** (#453, DM-02). No test here calls a live model: they
//! read the request a transport would send, parse bodies as strings, or serve a canned body from a local
//! socket. The one recorded exchange is `fixtures/openrouter/` (see `tools/decide/record-openrouter.sh`).

use bevy_oracle::*;

fn lookup<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |k| pairs.iter().find(|(name, _)| *name == k).map(|(_, v)| (*v).to_owned())
}

#[test]
fn the_default_protocol_is_system_one_and_nothing_leaves_the_lan() {
    let c = Config::from_lookup(|_| None);
    assert_eq!(c.protocol, Protocol::SystemOne);
    assert!(c.endpoint.starts_with("http://127.0.0.1:"), "{c:?}");
}

#[test]
fn one_variable_chooses_openrouter_with_its_endpoint_model_and_key() {
    let c = Config::from_lookup(lookup(&[
        ("GRIMOIRE_DECIDE_PROTOCOL", "OpenRouter"),
        ("OPENROUTER_API_KEY", "sk-or-test"),
        // A key meant for another endpoint kind is never sent to OpenRouter.
        ("GRIMOIRE_DECIDE_KEY", "cloudflare-token"),
    ]));
    assert_eq!(c.protocol, Protocol::OpenRouter);
    assert_eq!(c.endpoint, "https://openrouter.ai/api/alpha/decisions");
    assert_eq!(c.model.as_deref(), Some("perplexity/pplx-decider-v1.3:free"));
    assert_eq!(c.key.as_deref(), Some("sk-or-test"));
}

#[test]
fn system_one_never_reads_the_openrouter_key() {
    let c = Config::from_lookup(lookup(&[("OPENROUTER_API_KEY", "sk-or-test")]));
    assert_eq!(c.key, None);
}

#[test]
fn an_unknown_protocol_is_kept_so_the_first_request_can_refuse_it() {
    let c = Config::from_lookup(lookup(&[("GRIMOIRE_DECIDE_PROTOCOL", "openrouterr")]));
    assert_eq!(c.protocol, Protocol::Unrecognised("openrouterr".into()));
}

#[test]
fn debug_names_that_a_key_is_set_and_never_the_key() {
    let c = Config { key: Some("sk-or-SECRET".into()), ..Config::default() };
    let shown = format!("{c:?}");
    assert!(!shown.contains("SECRET") && shown.contains("<set>"), "{shown}");
}
