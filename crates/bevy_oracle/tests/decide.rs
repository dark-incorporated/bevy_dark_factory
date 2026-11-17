use bevy_oracle::*;
use std::path::PathBuf;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// The request the fixture was recorded from, built through the typed API.
fn fixture_request() -> Request {
    Request::new("A screenshot of the bevy_blueprint map editor.")
        .with_image("<data URL of docs/changelog/media/2026-10-24-S1.5.png, 640x390 PNG>")
        .with_question("has_ui", Question::noul("Does the image show an editor window with panels or a toolbar?"))
        .with_question(
            "dominant",
            Question::choice(
                "What fills most of the image?",
                [
                    ("map_view", "a map or 3D scene"),
                    ("text", "mostly text"),
                    ("blank", "an empty or single-colour frame"),
                ],
            ),
        )
        .with_question(
            "clarity",
            Question::score(
                "How readable and uncluttered is the image?",
                ["unreadable", "poor", "fair", "good", "excellent"],
            ),
        )
}

#[test]
fn request_serializes_to_the_recorded_api_shape() {
    let recorded: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(fixtures().join("request.json")).unwrap()).unwrap();
    assert_eq!(serde_json::to_value(fixture_request()).unwrap(), recorded);
}

#[test]
fn a_named_model_is_sent_as_model() {
    let mut r = fixture_request();
    r.model = Some("@cf/cloudflare/clef".into());
    assert_eq!(serde_json::to_value(r).unwrap()["model"], "@cf/cloudflare/clef");
}

#[test]
fn replay_gives_the_recorded_answers_with_no_network() {
    let replay = Replay::load(&fixtures()).unwrap();
    let r = ask(&replay, &fixture_request()).unwrap();
    assert!((r.answers["has_ui"].p_yes().unwrap() - 0.9291138751388158).abs() < 1e-12);
    assert!((r.answers["dominant"].p_option("map_view").unwrap() - 0.9771808547074415).abs() < 1e-12);
    match &r.answers["clarity"] {
        Answer::Score { score, legend, .. } => {
            assert!((score - 2.1405404734421807).abs() < 1e-12);
            assert_eq!(legend["4"], "excellent");
        }
        other => panic!("{other:?}"),
    }
    // Same again: same answers.
    assert_eq!(ask(&replay, &fixture_request()).unwrap(), r);
}

#[test]
fn replay_refuses_a_request_it_was_not_recorded_for() {
    let replay = Replay::load(&fixtures()).unwrap();
    let other = fixture_request().with_question("extra", Question::noul("Anything?"));
    assert!(matches!(ask(&replay, &other), Err(Error::Fixture(_))));
}

#[test]
fn a_response_that_skips_or_mistypes_a_question_is_an_error() {
    let recorded = Replay::load(&fixtures()).unwrap().send(&fixture_request()).unwrap();
    // Ask `has_ui` as a score: the recorded answer is a noul.
    let mut req = fixture_request();
    req.questions.insert("has_ui".into(), Question::score("x", ["a", "b"]));
    let replay2 = Replay::new(req.clone(), recorded.clone());
    assert!(matches!(ask(&replay2, &req), Err(Error::BadResponse { .. })));
    // Skipped: the recorded response lacks an answer the request asks for.
    let mut more = fixture_request();
    more.questions.insert("extra".into(), Question::noul("Anything?"));
    assert!(matches!(ask(&Replay::new(more.clone(), recorded), &more), Err(Error::BadResponse { .. })));
}

#[test]
fn requests_over_the_api_limits_are_refused_before_sending() {
    let replay = Replay::load(&fixtures()).unwrap();
    assert!(matches!(ask(&replay, &Request::new("s")), Err(Error::Invalid(_))));
    let mut r = fixture_request();
    r.images = vec!["x".into(); 5];
    assert!(matches!(ask(&replay, &r), Err(Error::Invalid(m)) if m.contains("5 images")));
}

#[test]
fn an_unreachable_endpoint_is_an_error_naming_it() {
    let cfg = Config { endpoint: "http://127.0.0.1:1/v1/systemone".into(), timeout_secs: 2, ..Config::default() };
    match ask(&Http::new(cfg), &fixture_request()) {
        Err(e @ Error::Unreachable { .. }) => assert!(e.to_string().contains("127.0.0.1:1/v1/systemone")),
        other => panic!("{other:?}"),
    }
}

/// A one-shot local server answering with `status` and `body`.
///
/// It reads the whole request (headers, then `Content-Length` bytes) before it answers: a socket
/// closed with request bytes still unread sends a reset, and the client then sees `Connection reset
/// by peer` instead of the answer. One `read` returned only part of the request in 165 of 200 runs.
fn serve_once(status: u16, body: &'static str) -> String {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/systemone", l.local_addr().unwrap());
    std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
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
                    .find_map(|line| line.strip_prefix("content-length:"))
                    .and_then(|v| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                if req.len() >= end + 4 + len {
                    break;
                }
            }
        }
        let _ = write!(
            s,
            "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    });
    url
}

#[test]
fn a_refused_request_is_an_error_with_endpoint_status_and_body() {
    let url = serve_once(401, r#"{"error":"bad key"}"#);
    let cfg = Config { endpoint: url.clone(), ..Config::default() };
    match ask(&Http::new(cfg), &fixture_request()) {
        Err(Error::Refused { endpoint, status, body }) => {
            assert_eq!((endpoint, status), (url, 401));
            assert!(body.contains("bad key"));
        }
        other => panic!("{other:?}"),
    }
}

/// llama-swap answers when it cannot start the model (a GPU lease on big evicted it and its launch script
/// refused to load it again): measured 2026-11-15 on big, HTTP 500 in 0.25 s with the body below. That is not
/// a refusal of the request: the server cannot serve right now, so it is `Unreachable`, with the status, the
/// gateway's own words and what to do about it. Any 502, 503 or 504 says the same.
#[test]
fn a_gateway_that_cannot_run_the_model_is_unreachable_with_the_remedy() {
    const LLAMA_SWAP_500: &str = r#"{"error":"unspecific error: upstream command exited prematurely","src":"llama-swap"}"#;
    for (status, body) in [(500, LLAMA_SWAP_500), (502, "x"), (503, "x"), (504, "x")] {
        let url = serve_once(status, body);
        let cfg = Config { endpoint: url.clone(), ..Config::default() };
        match ask(&Http::new(cfg), &fixture_request()) {
            Err(e @ Error::Unreachable { .. }) => {
                let said = e.to_string();
                assert!(said.contains(&url), "names the endpoint: {said}");
                assert!(said.contains(&status.to_string()), "names the status: {said}");
                assert!(said.contains(body), "keeps the gateway's words: {said}");
                assert!(said.contains("gpu-tenant status"), "names the remedy: {said}");
            }
            other => panic!("{status}: {other:?}"),
        }
    }
}

#[test]
fn a_server_error_from_the_model_itself_is_still_a_refusal() {
    let url = serve_once(500, r#"{"error":"out of memory"}"#);
    let cfg = Config { endpoint: url, ..Config::default() };
    assert!(matches!(ask(&Http::new(cfg), &fixture_request()), Err(Error::Refused { status: 500, .. })));
}

/// `big.local` resolves to link-local and global IPv6 addresses as well as the LAN's IPv4 one, and big's
/// firewall drops IPv6 SYNs to :8081. A client that tries the addresses in order sat 56.9 s on the two
/// dead IPv6 ones before the answer came from IPv4 (0.44 s by literal IP), a whole 60 s timeout spent on a
/// name. Decisions are asked over IPv4: here `localhost` has a server on `::1` only, and it is not reached.
#[test]
fn a_name_is_resolved_to_its_ipv4_address_only() {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("[::1]:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = l.accept() {
            let mut buf = [0u8; 8192];
            let _ = s.read(&mut buf);
            let _ = s.write_all(b"HTTP/1.1 401 X\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
        }
    });
    let cfg = Config { endpoint: format!("http://localhost:{port}/v1/systemone"), timeout_secs: 5, ..Config::default() };
    match ask(&Http::new(cfg), &fixture_request()) {
        Err(Error::Unreachable { .. }) => {}
        other => panic!("reached the IPv6-only server: {other:?}"),
    }
}

#[test]
fn http_round_trips_the_recorded_response() {
    let body: &'static str =
        Box::leak(std::fs::read_to_string(fixtures().join("response.json")).unwrap().into_boxed_str());
    let cfg = Config { endpoint: serve_once(200, body), ..Config::default() };
    let r = ask(&Http::new(cfg), &fixture_request()).unwrap();
    assert_eq!(r, Replay::load(&fixtures()).unwrap().send(&fixture_request()).unwrap());
}

#[test]
fn config_defaults_local_and_the_environment_names_only_the_remote_key() {
    let d = Endpoints::default().config(DecisionModel::Local).unwrap();
    assert!(d.endpoint == "http://big.local:8081/upstream/d3-flash/v1/systemone" && d.key.is_none() && d.model.is_none());
    assert_eq!(d, Config::default());
    // Where each choice goes is the `Endpoints` value, so a test (or a stand-in server) names both.
    let e = Endpoints { local: "http://lan.example:8093/v1/systemone".into(), remote: "https://router.example/decisions".into(), key: Some("k".into()) };
    let (local, remote) = (e.config(DecisionModel::Local).unwrap(), e.config(DecisionModel::Remote).unwrap());
    assert_eq!((local.endpoint.as_str(), local.key.as_deref()), ("http://lan.example:8093/v1/systemone", None));
    assert_eq!((remote.endpoint.as_str(), remote.model.as_deref(), remote.key.as_deref()), ("https://router.example/decisions", Some("perplexity/pplx-decider-v1.3:free"), Some("k")));
}

fn s(p_yes: f64, truth: bool) -> Sample {
    Sample { p_yes, truth }
}

#[test]
fn calibration_picks_the_threshold_that_maximizes_agreement() {
    // Separable at 0.6..0.7; 0.9 labelled no is the one error any threshold must carry.
    let set = [s(0.1, false), s(0.2, false), s(0.6, false), s(0.7, true), s(0.8, true), s(0.95, true), s(0.9, false)];
    let c = calibrate("floating", "clef-flash", "2026-10-29", &set).unwrap();
    // Best: threshold between 0.6 and 0.7 -> 6/7 agree.
    assert!((c.threshold - 0.65).abs() < 1e-9, "{c:?}");
    assert!((c.agreement - 6.0 / 7.0).abs() < 1e-9);
    assert_eq!((c.set_size, c.model.as_str(), c.date.as_str()), (7, "clef-flash", "2026-10-29"));
    assert!(c.is_blocking(0.85) && !c.is_blocking(0.9));
}

#[test]
fn a_perfectly_separable_set_gets_the_gap_midpoint_nearest_half() {
    let set = [s(0.05, false), s(0.3, false), s(0.9, true), s(0.99, true)];
    let c = calibrate("c", "m", "d", &set).unwrap();
    assert_eq!(c.agreement, 1.0);
    assert!((c.threshold - 0.6).abs() < 1e-9);
}

#[test]
fn a_model_that_is_inverted_is_reported_as_poor_agreement_not_flipped() {
    let set = [s(0.9, false), s(0.8, false), s(0.1, true), s(0.2, true)];
    let c = calibrate("c", "m", "d", &set).unwrap();
    assert!(c.agreement <= 0.5);
}

#[test]
fn a_set_that_cannot_calibrate_is_an_error() {
    assert!(calibrate("c", "m", "d", &[]).is_err());
    assert!(calibrate("c", "m", "d", &[s(0.1, false), s(0.2, false)]).is_err());
    assert!(calibrate("c", "m", "d", &[s(f64::NAN, true), s(0.2, false)]).is_err());
}
