//! **A calibration belongs to one model** (#455, DM-04).
//!
//! A check's threshold and its agreement were measured on one model's probabilities; another model's
//! P(yes) is on another scale (`clef-flash` 0.24 and `d3-flash` are not the same dial). So a check
//! carries a calibration *per model*, and a finding names the model that answered and whether this check
//! is calibrated for it. Asked of a model it was not calibrated for, a check still answers (it is
//! advisory) but says `uncalibrated for <model>` and can never block the gate.
//!
//! No test here calls a live model: answers come from a stand-in that names the model it pretends to be,
//! and the one HTTP exchange (`GET /v1/models`) is a stub server on a loopback port.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;

use bevy_oracle::{
    Answer, Calibration, Check, Checks, Config, Error, Evidence, Expecting, Http, Request, Response, Transport,
    evidence_from, run_checks,
};

/// The agreement bar every run here uses.
const BAR: f64 = 0.9;
/// The P(yes) the stand-in answers with.
const P_YES: f64 = 0.6;
/// The author's default threshold, under `P_YES`: any model without its own calibration finds something.
const DEFAULT_THRESHOLD: f64 = 0.5;
/// clef-flash's calibrated threshold, over `P_YES`: on clef-flash the same answer is not a finding.
const CLEF_THRESHOLD: f64 = 0.7;
/// d3-flash's calibrated threshold, under `P_YES`.
const D3_THRESHOLD: f64 = 0.4;
/// Agreement of a record that has earned the right to block.
const EARNED: f64 = 0.95;

/// A stand-in model: answers every `noul` with `P_YES` and names itself as `model`.
struct Named(Option<&'static str>);

impl Transport for Named {
    fn endpoint(&self) -> String {
        "stand-in".into()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        let answers = request.questions.keys().map(|id| (id.clone(), Answer::Noul { noul: P_YES })).collect();
        Ok(Response { model: self.0.map(str::to_owned), answers, usage: None })
    }
}

fn record(model: &str, threshold: f64) -> Calibration {
    Calibration {
        check: "floating".into(),
        model: model.into(),
        set_size: 20,
        agreement: EARNED,
        threshold,
        date: "2026-11-15".into(),
        correct: 19,
        wrong: 1,
        excluded: Vec::new(),
    }
}

fn check(calibrations: Vec<Calibration>) -> Check {
    Check {
        name: "floating".into(),
        question: "Is the piece floating?".into(),
        images: vec!["after".into()],
        facts: false,
        threshold: DEFAULT_THRESHOLD,
        after_edit: false,
        calibrations,
    }
}

fn evidence() -> Evidence {
    let mut images = BTreeMap::new();
    images.insert("after".to_owned(), "data:image/png;base64,AAAA".to_owned());
    evidence_from(&images, None)
}

#[test]
fn a_check_calibrated_for_model_a_answered_by_model_b_is_marked_uncalibrated_and_cannot_block() {
    let c = check(vec![record("clef-flash", CLEF_THRESHOLD)]);
    let finding = c.run(&Named(Some("d3-flash")), &evidence()).unwrap();

    assert_eq!(finding.model.as_deref(), Some("d3-flash"), "the finding names the model that answered");
    assert!(!finding.is_calibrated(), "clef-flash's record says nothing about d3-flash");
    assert_eq!(finding.uncalibrated_note().as_deref(), Some("uncalibrated for d3-flash"));
    assert!(finding.found, "it still answers: P(yes) {P_YES} is over the author's default {DEFAULT_THRESHOLD}");
    assert_eq!(finding.threshold, DEFAULT_THRESHOLD, "clef-flash's threshold is not borrowed");
    assert!(!finding.is_blocking(BAR), "an uncalibrated finding cannot block");

    // And through the run: advisory, never blocking, the run does not fail.
    let set = Checks { checks: vec![c], ..Checks::default() };
    let verdict = run_checks(&set, &Named(Some("d3-flash")), &|_: &Check| Ok(evidence()), BAR);
    assert!(verdict.blocking.is_empty());
    assert_eq!(verdict.advisory.len(), 1);
    assert!(!verdict.fails(), "a check asked of the wrong model never fails a gate");
    assert!(verdict.report().contains("uncalibrated for d3-flash"), "{}", verdict.report());
}

#[test]
fn the_model_it_was_calibrated_for_answers_with_its_own_threshold_and_may_block() {
    let c = check(vec![record("clef-flash", CLEF_THRESHOLD)]);
    let finding = c.run(&Named(Some("clef-flash")), &evidence()).unwrap();
    assert!(finding.is_calibrated());
    assert_eq!(finding.uncalibrated_note(), None);
    assert_eq!(finding.threshold, CLEF_THRESHOLD, "a calibrated model uses the threshold it earned");
    assert!(!finding.found, "P(yes) {P_YES} is under clef-flash's {CLEF_THRESHOLD}");

    let both = check(vec![record("clef-flash", CLEF_THRESHOLD), record("d3-flash", D3_THRESHOLD)]);
    let d3 = both.run(&Named(Some("d3-flash")), &evidence()).unwrap();
    assert_eq!(d3.threshold, D3_THRESHOLD, "each model reads its own record");
    assert!(d3.found && d3.is_blocking(BAR), "calibrated for d3-flash at {EARNED}: it may block");
    assert!(both.may_block(Some("d3-flash"), BAR) && both.may_block(Some("clef-flash"), BAR));
    assert!(!both.may_block(Some("pplx-decider"), BAR) && !both.may_block(None, BAR));
}

#[test]
fn an_answer_that_does_not_name_its_model_is_never_calibrated() {
    let c = check(vec![record("clef-flash", CLEF_THRESHOLD)]);
    let finding = c.run(&Named(None), &evidence()).unwrap();
    assert_eq!(finding.model, None);
    assert!(!finding.is_calibrated(), "a record cannot be matched to an answerer nobody named");
    assert!(!finding.is_blocking(BAR));
    assert_eq!(finding.uncalibrated_note().as_deref(), Some("uncalibrated for an unnamed model"));
}

#[test]
fn a_checks_file_with_the_old_single_calibration_is_refused_not_ignored() {
    let old = "checks:\n- name: a\n  question: q\n  threshold: 0.5\n  calibration: {check: a, model: clef-flash, \
               set_size: 2, agreement: 1.0, threshold: 0.5, date: '2026-11-03'}\n";
    match Checks::from_yaml(old) {
        Err(Error::Checks(m)) => assert!(m.contains("calibrations"), "says what replaced it: {m}"),
        other => panic!("a stale `calibration` would silently lose its authority: {other:?}"),
    }
    let null = "checks:\n- name: a\n  question: q\n  threshold: 0.5\n  calibration: null\n";
    assert!(Checks::from_yaml(null).is_ok(), "an empty old key says nothing and is harmless");
}

#[test]
fn a_calibrations_list_round_trips_through_checks_yaml() {
    let set = Checks {
        checks: vec![check(vec![record("clef-flash", CLEF_THRESHOLD), record("d3-flash", D3_THRESHOLD)])],
        ..Checks::default()
    };
    assert_eq!(Checks::from_yaml(&set.to_yaml().unwrap()).unwrap(), set);
}

/// A loopback server that answers one `GET` with `body`, and returns the request line it saw.
fn stub(body: &'static str) -> (String, std::thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handle = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        let mut buf = [0u8; 2048];
        let n = s.read(&mut buf).unwrap();
        let line = String::from_utf8_lossy(&buf[..n]).lines().next().unwrap_or("").to_owned();
        write!(s, "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).unwrap();
        line
    });
    (base, handle)
}

fn http(base: &str) -> Http {
    Http::new(Config { endpoint: format!("{base}/v1/systemone"), timeout_secs: 5, ..Config::default() })
}

#[test]
fn the_model_an_endpoint_reports_must_be_the_label_a_calibration_wears() {
    let (base, seen) = stub(r#"{"object":"list","data":[{"id":"d3-flash","object":"model"}]}"#);
    let http = http(&base);
    let served = http.served_models().unwrap();
    assert_eq!(seen.join().unwrap(), "GET /v1/models HTTP/1.1", "the list is read from the endpoint's own host");
    assert_eq!(served, vec!["d3-flash".to_owned()]);

    bevy_oracle::verify_served("d3-flash", &http.endpoint(), &served).expect("the label is what the endpoint serves");
    match bevy_oracle::verify_served("clef-flash", &http.endpoint(), &served) {
        Err(Error::ModelMismatch { label, served, .. }) => {
            assert_eq!(label, "clef-flash");
            assert_eq!(served, vec!["d3-flash".to_owned()]);
        }
        other => panic!("a record labelled clef-flash must not be made from d3-flash: {other:?}"),
    }
}

#[test]
fn an_answer_from_another_model_than_the_label_is_refused() {
    let request = Request::new("s").with_question("q", bevy_oracle::Question::noul("?"));
    let ok = Expecting::new(&Named(Some("clef-flash")), "clef-flash").send(&request);
    assert!(ok.is_ok());
    match Expecting::new(&Named(Some("d3-flash")), "clef-flash").send(&request) {
        Err(Error::ModelMismatch { label, served, .. }) => {
            assert_eq!(label, "clef-flash");
            assert_eq!(served, vec!["d3-flash".to_owned()]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_guard_does_not_change_where_the_model_ran() {
    // A wrapper that forgot `site` would read as Remote (the trait's default) and say pictures left the LAN.
    let replay = bevy_oracle::Replay::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")).unwrap();
    assert_eq!(Expecting::new(&replay, "clef-flash").site(), bevy_oracle::DecisionModel::Local);
}
