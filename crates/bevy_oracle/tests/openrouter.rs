//! **OpenRouter Decisions as the second protocol** (#453, DM-02). No test here calls a live model: they
//! read the request a transport would send, parse bodies as strings, or serve a canned body from a local
//! socket. The one recorded exchange is `fixtures/openrouter/` (see `tools/decide/record-openrouter.sh`).

use bevy_oracle::*;

fn lookup<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |k| pairs.iter().find(|(name, _)| *name == k).map(|(_, v)| (*v).to_owned())
}

#[test]
fn the_default_protocol_is_system_one_and_nothing_leaves_the_lan() {
    let c = Endpoints::default().config(DecisionModel::Local).unwrap();
    assert_eq!(c.protocol, Protocol::SystemOne);
    assert!(c.endpoint.starts_with("http://big.local:"), "{c:?}");
}

#[test]
fn the_remote_choice_gets_openrouter_with_its_endpoint_model_and_key() {
    let c = Endpoints::with_key_from(lookup(&[("OPENROUTER_API_KEY", "sk-or-test")])).config(DecisionModel::Remote).unwrap();
    assert_eq!(c.protocol, Protocol::OpenRouter);
    assert_eq!(c.endpoint, "https://openrouter.ai/api/alpha/decisions");
    assert_eq!(c.model.as_deref(), Some("perplexity/pplx-decider-v1.3:free"));
    assert_eq!(c.key.as_deref(), Some("sk-or-test"));
}

#[test]
fn system_one_never_reads_the_openrouter_key() {
    let c = Endpoints::with_key_from(lookup(&[("OPENROUTER_API_KEY", "sk-or-test")])).config(DecisionModel::Local).unwrap();
    assert_eq!(c.key, None);
}

#[test]
fn debug_names_that_a_key_is_set_and_never_the_key() {
    let c = Config { key: Some("sk-or-SECRET".into()), ..Config::default() };
    let shown = format!("{c:?}");
    assert!(!shown.contains("SECRET") && shown.contains("<set>"), "{shown}");
}

#[path = "support/smoke_request.rs"]
mod smoke_request;

use serde_json::{Value, json};
use std::sync::mpsc;

const PNG_URL: &str = "data:image/png;base64,iVBORw0KGgo=";
const JPEG_URL: &str = "data:image/jpeg;base64,/9j/4AAQ";
const PERPLEXITY: &str = "perplexity/pplx-decider-v1.3:free";

fn asked() -> Request {
    Request::new("A screenshot of the editor.")
        .with_image(PNG_URL)
        .with_image(JPEG_URL)
        .with_question("has_ui", Question::noul("Is there a toolbar?"))
        .with_question("look", Question::choice("Which look?", [("a", "calm"), ("b", "busy")]))
}

// ---- request shaping ----

#[test]
fn images_ride_inside_state_as_image_url_parts_and_never_at_the_top_level() {
    let body = wire_request(&asked(), PERPLEXITY).unwrap();
    assert!(body.get("images").is_none(), "OpenRouter answers 400 to a top-level `images`: {body}");
    assert_eq!(
        body["state"],
        json!([
            "A screenshot of the editor.",
            {"type": "image_url", "image_url": {"url": PNG_URL}},
            {"type": "image_url", "image_url": {"url": JPEG_URL}},
        ])
    );
    assert_eq!(body["model"], PERPLEXITY);
    assert_eq!(body["questions"]["has_ui"], json!({"type": "noul", "instructions": "Is there a toolbar?"}));
    assert_eq!(body["questions"]["look"]["criteria"]["b"], "busy");
}

#[test]
fn a_perplexity_model_carries_the_fixed_provider_block() {
    let body = wire_request(&asked(), PERPLEXITY).unwrap();
    assert_eq!(
        body["provider"],
        json!({"only": ["perplexity"], "allow_fallbacks": false, "data_collection": "deny", "zdr": true})
    );
}

#[test]
fn a_model_of_another_provider_gets_no_invented_provider_block() {
    let body = wire_request(&asked(), "typesafe/jev-1.14").unwrap();
    assert!(body.get("provider").is_none(), "{body}");
}

#[test]
fn a_text_only_request_keeps_state_as_the_plain_string() {
    let r = Request::new("The cat sat on the mat.").with_question("q", Question::noul("About an animal?"));
    assert_eq!(wire_request(&r, PERPLEXITY).unwrap()["state"], "The cat sat on the mat.");
}

#[test]
fn an_image_that_is_not_a_base64_data_url_is_refused_before_anything_is_sent() {
    for bad in ["https://example.com/a.png", "data:image/gif;base64,R0lG", "<placeholder>"] {
        let r = Request::new("s").with_image(PNG_URL).with_image(bad).with_question("q", Question::noul("?"));
        match wire_request(&r, PERPLEXITY) {
            Err(Error::Invalid(m)) => assert!(m.contains("image 2"), "{m}"),
            other => panic!("{bad}: {other:?}"),
        }
    }
}

#[test]
fn the_smoke_request_is_the_public_changelog_image_and_nothing_else() {
    let r = smoke_request::request();
    assert!(smoke_request::PUBLIC_IMAGE.starts_with("docs/changelog/media/"));
    assert_eq!(r.images.len(), 1);
    assert_eq!(r.images[0], data_url(&std::fs::read(smoke_request::public_image_path()).unwrap()));
    let body = wire_request(&r, PERPLEXITY).unwrap();
    assert!(body.get("images").is_none() && body["state"].as_array().is_some_and(|s| s.len() == 2));
}

// ---- over a local socket ----

/// What a one-shot local server saw, and what it was told to answer.
struct Served {
    url: String,
    /// The raw request (head and body) once the server has read it.
    seen: mpsc::Receiver<String>,
}

impl Served {
    fn head_and_body(&self) -> (String, Value) {
        let raw = self.seen.recv_timeout(std::time::Duration::from_secs(10)).expect("the server saw a request");
        let (head, body) = raw.split_once("\r\n\r\n").expect("a request has a head and a body");
        (head.to_owned(), serde_json::from_str(body).expect("the body is JSON"))
    }
}

/// Answer one request with `status`, the extra header lines and `body`, reading the whole request first
/// (see `tests/decide.rs`: an unread request makes the client see a reset instead of the answer).
fn serve_once(status: u16, headers: &'static [&'static str], body: String) -> Served {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/api/alpha/decisions", listener.local_addr().unwrap());
    let (tx, seen) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut req = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = s.read(&mut buf).unwrap_or(0);
            if n == 0 {
                break;
            }
            req.extend_from_slice(&buf[..n]);
            if let Some(end) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                let head = String::from_utf8_lossy(&req[..end]).to_ascii_lowercase();
                let len = head
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if req.len() >= end + 4 + len {
                    break;
                }
            }
        }
        let _ = tx.send(String::from_utf8_lossy(&req).into_owned());
        let extra: String = headers.iter().map(|h| format!("{h}\r\n")).collect();
        let _ = write!(
            s,
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    });
    Served { url, seen }
}

fn openrouter_config(url: &str) -> Config {
    Config {
        protocol: Protocol::OpenRouter,
        endpoint: url.to_owned(),
        model: Some(PERPLEXITY.to_owned()),
        key: Some("sk-or-test-key".to_owned()),
        timeout_secs: 10,
    }
}

fn answered(provider: &str) -> String {
    json!({
        "id": "gen-dec-1789738314-X5e5eKGQdvR9rblyX250",
        "model": "perplexity/pplx-decider-v1.3-20261101",
        "provider": provider,
        "answers": {
            "has_ui": {"type": "noul", "noul": 0.93, "confidence": 0.93},
            "look": {"type": "choice", "choice": "a", "probabilities": {"a": 0.8, "b": 0.2}, "confidence": 0.8}
        },
        "usage": {"input_tokens": 612, "output_tokens": 0, "cost": 0, "input_tokens_details": {"cached_tokens": 0}}
    })
    .to_string()
}

#[test]
fn the_transport_sends_the_bearer_key_and_the_openrouter_body() {
    let server = serve_once(200, &[], answered("Perplexity"));
    let t = OpenRouterDecisions::new(openrouter_config(&server.url));
    ask(&t, &asked()).unwrap();
    let (head, body) = server.head_and_body();
    assert!(head.starts_with("POST /api/alpha/decisions "), "{head}");
    assert!(head.to_ascii_lowercase().contains("authorization: bearer sk-or-test-key"), "{head}");
    assert!(body.get("images").is_none() && body["state"][1]["type"] == "image_url", "{body}");
    assert_eq!(body["provider"]["zdr"], true);
    assert_eq!(body["model"], PERPLEXITY);
}

#[test]
fn the_answer_names_the_model_provider_id_and_cost_that_served_it() {
    let server = serve_once(200, &[], answered("Perplexity"));
    let r = ask(&Live::new(openrouter_config(&server.url)), &asked()).unwrap();
    assert_eq!(r.model.as_deref(), Some("perplexity/pplx-decider-v1.3-20261101"));
    let u = r.usage.unwrap();
    assert_eq!(u.provider.as_deref(), Some("Perplexity"));
    assert_eq!(u.id.as_deref(), Some("gen-dec-1789738314-X5e5eKGQdvR9rblyX250"));
    assert_eq!((u.cost, u.input_tokens, u.output_tokens), (Some(0.0), 612, 0));
    assert!((r.answers["has_ui"].p_yes().unwrap() - 0.93).abs() < 1e-12);
}

#[test]
fn a_perplexity_request_answered_by_another_provider_is_an_error() {
    let server = serve_once(200, &[], answered("SomeoneElse"));
    match ask(&OpenRouterDecisions::new(openrouter_config(&server.url)), &asked()) {
        Err(Error::BadResponse { detail, .. }) => assert!(detail.contains("SomeoneElse"), "{detail}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_missing_key_is_named_and_nothing_is_sent() {
    // No server exists: reaching for the network would be `Unreachable`, not `Invalid`.
    let cfg = Config { key: None, ..openrouter_config("http://127.0.0.1:1/api/alpha/decisions") };
    match ask(&OpenRouterDecisions::new(cfg), &asked()) {
        Err(Error::Invalid(m)) => assert!(m.contains("OPENROUTER_API_KEY"), "{m}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_key_is_in_no_error() {
    let server = serve_once(401, &[], r#"{"error":{"code":401,"message":"No auth credentials found"}}"#.into());
    let e = ask(&OpenRouterDecisions::new(openrouter_config(&server.url)), &asked()).unwrap_err();
    assert!(matches!(e, Error::Refused { status: 401, .. }), "{e:?}");
    assert!(!format!("{e} {e:?}").contains("sk-or-test-key"));
}

// ---- a refusal is not a verdict ----

#[test]
fn a_refused_question_is_an_error_naming_it_not_a_finding() {
    let body = json!({
        "id": "gen-dec-1", "model": "m", "provider": "Perplexity",
        "answers": {
            "has_ui": {"type": "noul", "noul": 0.9},
            "look": {"type": "refusal"}
        },
        "usage": {"input_tokens": 1, "output_tokens": 0, "cost": 0}
    })
    .to_string();
    let server = serve_once(200, &[], body);
    match ask(&OpenRouterDecisions::new(openrouter_config(&server.url)), &asked()) {
        Err(e @ Error::Refusal { .. }) => {
            let Error::Refusal { question, .. } = &e else { unreachable!() };
            assert_eq!(question, "look");
            assert!(e.to_string().contains("`look`") && e.to_string().contains("not a verdict"), "{e}");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_refusal_from_a_replay_is_the_same_error() {
    let request = Request::new("s").with_question("q", Question::noul("?"));
    let mut response = Response { model: None, answers: Default::default(), usage: None };
    response.answers.insert("q".into(), Answer::Refusal);
    assert!(matches!(ask(&Replay::new(request.clone(), response), &request), Err(Error::Refusal { question, .. }) if question == "q"));
}

#[test]
fn an_unexpected_answer_type_or_a_typeless_answer_is_named_never_a_number() {
    for (answer, needle) in [
        (json!({"type": "box", "box": {"top": 0, "bottom": 1, "left": 0, "right": 1}}), "unexpected type `box`"),
        (json!({"noul": 0.9}), "no `type`"),
        (json!({"type": "noul"}), "malformed noul answer"),
    ] {
        let body = json!({"model": "m", "answers": {"q": answer}, "usage": {"input_tokens": 1, "output_tokens": 0}}).to_string();
        let server = serve_once(200, &[], body);
        let request = Request::new("s").with_question("q", Question::noul("?"));
        match ask(&OpenRouterDecisions::new(openrouter_config(&server.url)), &request) {
            Err(Error::BadResponse { detail, .. }) => assert!(detail.contains("question `q`") && detail.contains(needle), "{detail}"),
            other => panic!("{needle}: {other:?}"),
        }
    }
}

#[test]
fn a_missing_answer_or_a_mistyped_one_is_still_caught_by_check_answers() {
    let body = json!({"model": "m", "answers": {"has_ui": {"type": "choice", "choice": "a", "probabilities": {"a": 1.0}}}, "usage": {"input_tokens": 1, "output_tokens": 0}}).to_string();
    let server = serve_once(200, &[], body);
    match ask(&OpenRouterDecisions::new(openrouter_config(&server.url)), &asked()) {
        Err(Error::BadResponse { detail, .. }) => assert!(detail.contains("`has_ui`"), "{detail}"),
        other => panic!("{other:?}"),
    }
}

// ---- 402, 404 and 429 are named, carry OpenRouter's reason, and are not retried ----

#[test]
fn a_402_is_the_credits_error_with_the_limit_source() {
    let server = serve_once(
        402,
        &[],
        r#"{"error":{"code":402,"message":"Insufficient credits. Add more using https://openrouter.ai/credits","metadata":{"limit_source":"key"}}}"#.into(),
    );
    match ask(&OpenRouterDecisions::new(openrouter_config(&server.url)), &asked()) {
        Err(Error::OpenRouter { failure: OpenRouterFailure::CreditsExhausted, reason, endpoint }) => {
            assert!(reason.contains("Insufficient credits") && reason.contains("limit_source"), "{reason}");
            assert_eq!(endpoint, server.url);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_404_says_the_privacy_filter_matched_no_provider_and_is_not_relaxed() {
    let server = serve_once(404, &[], r#"{"error":{"code":404,"message":"No endpoints found matching your data policy"}}"#.into());
    let e = ask(&OpenRouterDecisions::new(openrouter_config(&server.url)), &asked()).unwrap_err();
    assert!(matches!(&e, Error::OpenRouter { failure: OpenRouterFailure::NoEndpoint, reason, .. } if reason.contains("data policy")), "{e:?}");
    assert!(e.to_string().contains("never relaxed"), "{e}");
}

#[test]
fn a_429_carries_retry_after_and_is_not_retried() {
    let server = serve_once(429, &["Retry-After: 17"], r#"{"error":{"code":429,"message":"Rate limit exceeded: free-models-per-min"}}"#.into());
    // The server answers one request and exits: a hidden retry would meet a closed port and come
    // back `Unreachable`, not this error.
    let e = ask(&OpenRouterDecisions::new(openrouter_config(&server.url)), &asked()).unwrap_err();
    match &e {
        Error::OpenRouter { failure: OpenRouterFailure::RateLimited { retry_after }, reason, .. } => {
            assert_eq!(retry_after.as_deref(), Some("17"));
            assert!(reason.contains("free-models-per-min"), "{reason}");
        }
        other => panic!("{other:?}"),
    }
    assert!(e.to_string().contains("retry after 17"), "{e}");
}

#[test]
fn any_other_status_is_the_generic_refusal_with_the_body() {
    let server = serve_once(400, &[], r#"{"error":{"code":400,"message":"images is not allowed"}}"#.into());
    match ask(&OpenRouterDecisions::new(openrouter_config(&server.url)), &asked()) {
        Err(Error::Refused { status: 400, body, .. }) => assert!(body.contains("images is not allowed")),
        other => panic!("{other:?}"),
    }
}

// ---- one protocol per run ----

#[test]
fn live_speaks_the_protocol_the_config_names_and_the_wrong_transport_refuses() {
    let server = serve_once(200, &[], answered("Perplexity"));
    ask(&Live::new(openrouter_config(&server.url)), &asked()).unwrap();
    assert!(server.head_and_body().1.get("provider").is_some(), "Live::new(openrouter) sent the OpenRouter body");

    // `Http` is System One only; a config that says openrouter is never sent in the other shape.
    let wrong = Http::new(openrouter_config("http://127.0.0.1:1/x"));
    assert!(matches!(ask(&wrong, &asked()), Err(Error::Invalid(m)) if m.contains("openrouter")));
    let wrong = OpenRouterDecisions::new(Config::default());
    assert!(matches!(ask(&wrong, &asked()), Err(Error::Invalid(m)) if m.contains("systemone")));
}

// ---- the recorded exchange ----

fn recorded() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/openrouter")
}

/// Replays the one real exchange recorded with `Recorder` around [`Live`] over OpenRouter. It cannot
/// run until someone with a key records it, and it must never pass silently without it: so it is
/// ignored with the command that makes it runnable, and fails loudly with that command when run
/// without the fixture. `OPENROUTER_API_KEY` exists on no host as of 2026-11-15.
#[test]
#[ignore = "needs fixtures/openrouter recorded with OPENROUTER_API_KEY: run tools/decide/record-openrouter.sh"]
fn the_recorded_openrouter_exchange_replays_with_no_network() {
    let replay = Replay::load(&recorded()).unwrap_or_else(|e| {
        panic!("{e}\nno recorded fixture yet: run `OPENROUTER_API_KEY=... tools/decide/record-openrouter.sh`")
    });
    let request = smoke_request::request();
    let r = ask(&replay, &request).expect("the recorded exchange answers the smoke request");
    let usage = r.usage.as_ref().expect("OpenRouter always sends usage");
    assert!(
        usage.provider.as_deref().is_some_and(|p| p.eq_ignore_ascii_case("perplexity")),
        "the fixture must come from the provider the privacy block pins: {usage:?}"
    );
    assert!(r.model.as_deref().is_some_and(|m| m.starts_with("perplexity/pplx-decider")), "{:?}", r.model);
    assert!(usage.cost.is_some_and(|c| c >= 0.0) && usage.id.is_some(), "{usage:?}");
    for id in ["has_ui", "dominant", "clarity"] {
        assert!(r.answers.contains_key(id), "no recorded answer for `{id}`");
    }
    // The recorded request is the public image and nothing else.
    assert_eq!(replay.send(&request).unwrap(), r);
}

#[test]
fn the_openrouter_key_comes_from_the_environment_never_from_a_settings_file() {
    let dir = std::env::temp_dir().join(format!("oracle-key-from-env-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let user = dir.join("settings.yaml");
    std::fs::write(&user, "version: 1\ndecision_model: remote\nOPENROUTER_API_KEY: sk-or-from-a-file\n").unwrap();

    let without = Endpoints::with_key_from(|_| None);
    assert!(matches!(without.resolve(Some(&user), None), Err(Error::RemoteWithoutKey)), "a key in a settings file must be ignored");
    let with = Endpoints::with_key_from(lookup(&[("OPENROUTER_API_KEY", "sk-or-from-env")]));
    let resolved = with.resolve(Some(&user), None).unwrap();
    assert_eq!(resolved.config.key.as_deref(), Some("sk-or-from-env"));
    std::fs::remove_dir_all(dir).unwrap();
}
