//! **Look, then ask** (#456, DM-05): after a visible change an agent takes one still and asks four or five
//! typed questions, in one `grimoire_decide` call.
//!
//! No live model is called. The exchange is a recorded one ([`Replay`], which answers only the request it
//! was recorded for) or a stand-in that keeps what it was sent, so a test reads what the model was told.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use base64::Engine as _;
use bevy_cauldron_mcp::decide::{DecideParams, NamedImage, decide, name_the_model};
use bevy_oracle::{Answer, Config, Error, Question, Replay, Request, Response, Transport};
use serde_json::{Value, json};

/// **Time the recorded endpoint takes to answer**, so the answer's `latency_ms` has something to measure.
const ENDPOINT_DELAY: Duration = Duration::from_millis(40);

/// **A recorded endpoint at a chosen address**, slow enough to measure. [`Replay`] answers from the
/// recording; the address is what the answer's `where` is read from.
struct Recorded {
    endpoint: String,
    replay: Replay,
}

impl Transport for Recorded {
    fn endpoint(&self) -> String {
        self.endpoint.clone()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        std::thread::sleep(ENDPOINT_DELAY);
        self.replay.send(request)
    }
}

/// **The model's name as the recorded response carries it.**
const MODEL: &str = "vllm-sr/d3-flash";

/// **The owner's forest example, as five ad-hoc questions** (the issue's `floating` check is a named
/// check; the fifth ad-hoc question says the same thing in words).
fn forest_questions() -> BTreeMap<String, Value> {
    [
        ("covers", json!({"type": "noul", "instructions": "The trees cover the north hill."})),
        (
            "kind",
            json!({"type": "choice", "instructions": "What are most of the trees?",
                   "criteria": {"pine": "needles, conical", "broadleaf": "round crowns", "mixed": "both, in equal part"}}),
        ),
        (
            "dense",
            json!({"type": "score", "instructions": "How dense is the canopy?",
                   "criteria": ["bare", "sparse", "medium", "dense"]}),
        ),
        ("path", json!({"type": "noul", "instructions": "The path to the shrine is visible and unblocked."})),
        ("floats", json!({"type": "noul", "instructions": "No tree floats above the ground."})),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

fn forest_params() -> DecideParams {
    DecideParams {
        questions: Some(forest_questions()),
        state: Some("The person asked: plant a dense pine forest on the north hill, keep the path to the shrine clear.".into()),
        view: true,
        ..Default::default()
    }
}

/// **The still `view: true` takes**, as the base64 PNG the server hands `decide`.
fn view_png() -> String {
    base64::engine::general_purpose::STANDARD.encode(b"the editor's still")
}

/// **What recording the five-question exchange would have written.**
fn recording() -> Replay {
    let p = forest_params();
    let mut request = Request::new(p.state.clone().unwrap());
    request.images = vec![format!("data:image/png;base64,{}", view_png())];
    for (id, q) in forest_questions() {
        request.questions.insert(id, serde_json::from_value::<Question>(q).unwrap());
    }
    let answers = BTreeMap::from([
        ("covers".to_owned(), Answer::Noul { noul: 0.91 }),
        (
            "kind".to_owned(),
            Answer::Choice {
                choice: "pine".into(),
                probabilities: BTreeMap::from([("pine".into(), 0.7), ("broadleaf".into(), 0.1), ("mixed".into(), 0.2)]),
                confidence: 0.7,
            },
        ),
        (
            "dense".to_owned(),
            Answer::Score {
                score: 2.6,
                legend: BTreeMap::from([("0".into(), "bare".into()), ("3".into(), "dense".into())]),
                probabilities: BTreeMap::from([("2".into(), 0.4), ("3".into(), 0.6)]),
                confidence: 0.6,
            },
        ),
        ("path".to_owned(), Answer::Noul { noul: 0.12 }),
        ("floats".to_owned(), Answer::Noul { noul: 0.03 }),
    ]);
    Replay::new(request, Response { model: Some(MODEL.into()), answers, usage: None })
}

fn project(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-look-then-ask-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets/grimoire")).unwrap();
    dir
}

#[test]
fn one_call_with_view_and_five_questions_returns_five_answers_and_names_the_model_where_and_latency() {
    let dir = project("five");
    let local = Recorded { endpoint: Config::default().endpoint, replay: recording() };
    let out = decide(&forest_params(), &dir, Some(&view_png()), &local).unwrap();

    assert_eq!(out["answers"].as_object().unwrap().len(), 5, "{out}");
    assert_eq!(out["answers"]["covers"]["noul"], 0.91);
    assert_eq!(out["answers"]["kind"]["choice"], "pine");
    assert_eq!(out["answers"]["path"]["noul"], 0.12, "a blocked path is a low P(yes): the agent's cue to look again");
    assert_eq!(out["model"], MODEL, "the answer names the model that answered: {out}");
    assert_eq!(out["where"], "local", "the editor's own default endpoint is local: {out}");
    let ms = out["latency_ms"].as_u64().unwrap_or_else(|| panic!("latency_ms is a whole number of milliseconds: {out}"));
    assert!(ms >= ENDPOINT_DELAY.as_millis() as u64, "{ms} ms measured for an endpoint that takes {ENDPOINT_DELAY:?}");
    assert_eq!(out["endpoint"], Config::default().endpoint);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_endpoint_other_than_the_local_default_is_where_remote() {
    let dir = project("remote");
    let remote = Recorded { endpoint: "https://openrouter.ai/api/alpha/decisions".into(), replay: recording() };
    let out = decide(&forest_params(), &dir, Some(&view_png()), &remote).unwrap();
    assert_eq!(out["where"], "remote", "screenshots may leave the LAN, so anything but the local default says so: {out}");
    std::fs::remove_dir_all(dir).unwrap();
}

/// **A stand-in that keeps what it was sent** and answers every noul 0.5, naming `MODEL`.
struct Keeps {
    sent: Mutex<Vec<Request>>,
}
impl Transport for Keeps {
    fn endpoint(&self) -> String {
        Config::default().endpoint
    }
    fn send(&self, r: &Request) -> Result<Response, Error> {
        self.sent.lock().unwrap().push(r.clone());
        let answers = r.questions.keys().map(|id| (id.clone(), Answer::Noul { noul: 0.5 })).collect();
        Ok(Response { model: Some(MODEL.into()), answers, usage: None })
    }
}

fn keeps() -> Keeps {
    Keeps { sent: Mutex::new(Vec::new()) }
}

#[test]
fn a_named_check_names_the_model_where_and_latency_too() {
    let dir = project("check");
    let model = keeps();
    let p = DecideParams {
        check: Some("blank_frame".into()),
        images: vec![NamedImage { name: "view".into(), path: write(&dir, "view.png", b"a view") }],
        ..Default::default()
    };
    let out = decide(&p, &dir, None, &model).unwrap();
    assert_eq!(out["model"], MODEL, "{out}");
    assert_eq!(out["where"], "local");
    assert!(out["latency_ms"].is_u64(), "{out}");
    std::fs::remove_dir_all(dir).unwrap();
}

fn write(dir: &Path, rel: &str, bytes: &[u8]) -> String {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bytes).unwrap();
    path.to_string_lossy().into_owned()
}

fn decoded(url: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD.decode(url.split_once("base64,").unwrap().1).unwrap()
}

fn one_question() -> BTreeMap<String, Value> {
    [("ok".to_owned(), json!({"type": "noul", "instructions": "Is it right?"}))].into()
}

#[test]
fn a_still_from_capture_still_is_accepted_in_images_by_path_and_by_fingerprint() {
    let dir = project("stills");
    // Where `grimoire_capture_still` writes: `<project>/target/loop/captures/<label>/still-<ms>.png`, and
    // the answer's `path` is that file, absolute.
    let before = write(&dir, "target/loop/captures/before/still-100.png", b"the still before");
    let after_bytes = b"the still after".to_vec();
    write(&dir, "target/loop/captures/after/still-200.png", &after_bytes);
    let fingerprint = bevy_cauldron_mcp::fingerprint(&after_bytes);

    let model = keeps();
    let p = DecideParams {
        questions: Some(one_question()),
        images: vec![
            NamedImage { name: "before".into(), path: before },
            NamedImage { name: "after".into(), path: fingerprint },
        ],
        ..Default::default()
    };
    decide(&p, &dir, None, &model).unwrap();
    let sent = model.sent.lock().unwrap();
    let images: Vec<Vec<u8>> = sent[0].images.iter().map(|i| decoded(i)).collect();
    assert_eq!(images, vec![b"the still before".to_vec(), after_bytes], "the pair reaches the model in the order given");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_fingerprint_no_still_has_is_refused_and_says_how_to_take_one() {
    let dir = project("nofp");
    write(&dir, "target/loop/captures/before/still-100.png", b"the still before");
    let p = DecideParams {
        questions: Some(one_question()),
        images: vec![NamedImage { name: "after".into(), path: "0123456789abcdef".into() }],
        ..Default::default()
    };
    let err = decide(&p, &dir, None, &keeps()).unwrap_err();
    assert!(err.contains("0123456789abcdef") && err.contains("grimoire_capture_still"), "{err}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_fingerprint_names_only_stills_under_the_projects_captures() {
    let dir = project("fenced");
    let bytes = b"a picture that is not a capture".to_vec();
    write(&dir, "assets/other.png", &bytes);
    let p = DecideParams {
        questions: Some(one_question()),
        images: vec![NamedImage { name: "after".into(), path: bevy_cauldron_mcp::fingerprint(&bytes) }],
        ..Default::default()
    };
    assert!(decide(&p, &dir, None, &keeps()).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_model_the_endpoint_did_not_name_is_named_by_the_configured_one_and_never_overwritten() {
    let mut unnamed = json!({"answers": {}, "model": null});
    name_the_model(&mut unnamed, Some("configured-model"));
    assert_eq!(unnamed["model"], "configured-model");
    let mut named = json!({"answers": {}, "model": MODEL});
    name_the_model(&mut named, Some("configured-model"));
    assert_eq!(named["model"], MODEL);
    let mut neither = json!({"answers": {}, "model": null});
    name_the_model(&mut neither, None);
    assert!(neither["model"].is_null());
}
