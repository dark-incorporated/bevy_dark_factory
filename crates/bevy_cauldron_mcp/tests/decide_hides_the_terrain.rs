//! **`grimoire_decide` honours `hide` for the turn it is asked in** (#472, acceptance 4).
//!
//! `grimoire_decide`'s `floating` and `clipping` checks take `view` themselves, so on an underground map they
//! were asking about the rock's *outside* — a question about a surface the room is behind. This proves what
//! fixes that, through the one function both tools call (`decide::grab_through`) against a stand-in editor
//! that records every method asked of it and remembers what it has hidden:
//!
//! 1. **the picture handed to the model is taken with the terrain hidden** — the `surface_hide` naming
//!    `terrain` arrives *before* the capture, so the frame the model is sent is the cut one;
//! 2. **the surfaces are put back afterwards** — a second `surface_hide` arrives after the capture and
//!    restores what stood before it, so the person's screen is as it was before the question.
//!
//! It also proves the parameter is the still's own: the same names, the same default (nothing asked, nothing
//! changed), and the editor's own refusal on a name it does not draw — never a second implementation of
//! hiding, and never a second vocabulary.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bevy_cauldron_mcp::brp::Brp;
use bevy_cauldron_mcp::decide::grab_through;
use serde_json::{Value, json};

/// **One BRP call a stand-in editor was asked**: the method, its parameters, and when it arrived.
#[derive(Clone, Debug)]
struct Asked {
    method: String,
    params: Value,
    at: Duration,
}

/// **A stand-in editor that records every method it is asked**, so a test reads what the server did to the
/// view and when it did it relative to the capture.
struct FakeEditor {
    port: u16,
    asked: Arc<Mutex<Vec<Asked>>>,
}

/// The surfaces the editor draws, in its own words — `outliner::HIDABLE`'s three.
const HIDABLE: [&str; 3] = ["terrain", "volume", "water"];

/// **A stand-in editor over BRP**, answering fairy's two capture methods and `grimoire/surface_hide`, and
/// **remembering what it has hidden** so a test can read the view at the moment of the capture.
fn editor() -> FakeEditor {
    let listener = TcpListener::bind("127.0.0.1:0").expect("an ephemeral port");
    let port = listener.local_addr().expect("the bound port").port();
    let asked: Arc<Mutex<Vec<Asked>>> = Arc::default();
    let seen = asked.clone();
    // A 1 x 1 transparent PNG: nothing here reads a pixel, only that a picture came back.
    let png = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8DwHwAFAAH/q842iQAAAABJRU5ErkJggg==";
    let started = Instant::now();
    let mut hidden: Vec<String> = Vec::new();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut s) = stream else { continue };
            let mut raw = Vec::new();
            let mut buf = [0u8; 4096];
            let body = loop {
                let n = s.read(&mut buf).unwrap_or(0);
                raw.extend_from_slice(&buf[..n]);
                if let Some(end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                    let head = String::from_utf8_lossy(&raw[..end]).to_ascii_lowercase();
                    let len = head
                        .lines()
                        .find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                        .unwrap_or(0);
                    if raw.len() >= end + 4 + len {
                        break raw[end + 4..end + 4 + len].to_vec();
                    }
                }
                if n == 0 {
                    break Vec::new();
                }
            };
            let Ok(req) = serde_json::from_slice::<Value>(&body) else { continue };
            let method = req["method"].as_str().unwrap_or_default().to_owned();
            let params = req["params"].clone();
            seen.lock().unwrap().push(Asked { method: method.clone(), params: params.clone(), at: started.elapsed() });
            let result = match method.as_str() {
                "bevy_fairy/screenshot" => json!({"id": 1}),
                // **What the view was while this picture was taken**, so a test can tell a still taken under
                // the cut from one taken without it, without reading a pixel.
                "bevy_fairy/capture" => json!({"status": "done", "png_base64": png, "hidden_at_capture": hidden}),
                "grimoire/surface_hide" => {
                    let before = hidden.clone();
                    let named: Vec<String> = params["hide"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
                        .unwrap_or_default();
                    if let Some(bad) = named.iter().find(|n| !HIDABLE.contains(&n.as_str())) {
                        // **The editor's own refusal, word for word the shape `ops_brp::surface_hide` sends**:
                        // it names what was asked for and every name that would have worked, and changes nothing.
                        let out = json!({"jsonrpc": "2.0", "id": req["id"], "error": {
                            "code": -32000,
                            "message": format!("`{bad}` is not a surface this editor draws. Say one of {}. Nothing was changed.", HIDABLE.join(", "))}})
                        .to_string();
                        let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
                        continue;
                    }
                    hidden = named;
                    json!({"hidden": hidden, "was": before, "drawn_now": []})
                }
                _ => json!({}),
            };
            let out = json!({"jsonrpc": "2.0", "id": req["id"], "result": result}).to_string();
            let _ = write!(
                s,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}",
                out.len()
            );
        }
    });
    FakeEditor { port, asked }
}

impl FakeEditor {
    fn brp(&self) -> Brp {
        Brp::new(format!("http://127.0.0.1:{}/", self.port))
    }
    fn asked(&self) -> Vec<Asked> {
        self.asked.lock().unwrap().clone()
    }
}

/// **The one call `grimoire_decide` makes for its `view`**, with `hide` naming the terrain.
#[tokio::test(flavor = "multi_thread")]
async fn the_picture_decide_asks_about_is_taken_with_the_terrain_hidden() {
    let editor = editor();
    let (png, was) = grab_through(&editor.brp(), "offscreen", Some(&["terrain".to_owned()]), true)
        .await
        .expect("the picture is taken");
    assert!(!png.is_empty(), "a picture came back");
    assert_eq!(was, Some(vec![]), "nothing was hidden before the call: {was:?}");

    let asked = editor.asked();
    let hides: Vec<&Asked> = asked.iter().filter(|a| a.method == "grimoire/surface_hide").collect();
    let capture = asked.iter().find(|a| a.method == "bevy_fairy/screenshot").expect("a capture was asked for");
    assert_eq!(hides.len(), 2, "one hide and one restore, and nothing else: {asked:#?}");
    assert_eq!(hides[0].params["hide"], json!(["terrain"]), "the first call hides the terrain");
    assert!(hides[0].at <= capture.at, "**the hide arrives before the picture**, so the model is sent the cut one");
    assert!(hides[1].at >= capture.at, "and the restore arrives after it");
    assert_eq!(hides[1].params["hide"], json!([]), "the restore names what stood before: nothing was hidden");
}

/// **A view that was already hidden is put back as it was**, not unhidden: the restore is what stood before,
/// not an empty list. A check asked twice in a row must leave the person's screen where it found it.
#[tokio::test(flavor = "multi_thread")]
async fn what_was_hidden_before_the_question_is_what_is_hidden_after_it() {
    let editor = editor();
    let brp = editor.brp();
    // The person already has the water hidden, for a reason of their own.
    brp.call("grimoire/surface_hide", json!({"hide": ["water"]})).await.expect("the water is hidden");
    let (_png, was) = grab_through(&brp, "offscreen", Some(&["terrain".to_owned()]), true)
        .await
        .expect("the picture is taken");
    assert_eq!(was, Some(vec!["water".to_owned()]), "the call says what stood before it: {was:?}");
    let after = brp.call("grimoire/surface_hide", json!({"hide": ["water"]})).await.expect("asked again");
    assert_eq!(after["was"], json!(["water"]), "**the person's water is still hidden afterwards**: {after}");
}

/// **With no `hide` the editor is not asked at all** — no round trip, so nothing on the view can move. That is
/// the parameter's default, and it is the still's: an unused call changes nothing.
#[tokio::test(flavor = "multi_thread")]
async fn with_no_hide_the_view_is_never_touched() {
    let editor = editor();
    let (_png, was) = grab_through(&editor.brp(), "offscreen", None, true).await.expect("the picture is taken");
    assert_eq!(was, None, "the call names no view, so it reports none");
    let asked = editor.asked();
    assert!(
        asked.iter().all(|a| a.method != "grimoire/surface_hide"),
        "**the editor is never asked to hide anything**: {asked:#?}"
    );
    assert_eq!(asked.iter().filter(|a| a.method == "bevy_fairy/screenshot").count(), 1, "and one picture was taken");
}

/// **A name the editor does not draw is refused in the editor's own words**, never silently ignored, and the
/// refusal is the still's — because it is the same `grimoire/surface_hide` verb under both tools.
#[tokio::test(flavor = "multi_thread")]
async fn a_surface_the_editor_does_not_draw_is_refused_naming_the_list() {
    let editor = editor();
    let said = grab_through(&editor.brp(), "offscreen", Some(&["terain".to_owned()]), true)
        .await
        .expect_err("a typo is refused");
    assert!(said.contains("terain"), "the refusal names what was asked for: {said}");
    for surface in HIDABLE {
        assert!(said.contains(surface), "and every name that would have worked: {said}");
    }
    let asked = editor.asked();
    assert!(
        asked.iter().all(|a| a.method != "bevy_fairy/screenshot"),
        "a refused hide takes no picture: {asked:#?}"
    );
}

/// **`DecideParams` carries `hide` in the still's own words**, and its default is no hiding at all: the
/// parameter is the one `grimoire_capture_still` has, not a second vocabulary a caller has to learn.
#[test]
fn the_parameters_hide_is_the_stills_own_and_defaults_to_none() {
    let plain: bevy_cauldron_mcp::decide::DecideParams = serde_json::from_value(json!({"view": true})).expect("a call with no hide");
    assert_eq!(plain.hide, None, "a call that names no hide hides nothing");

    let named: bevy_cauldron_mcp::decide::DecideParams =
        serde_json::from_value(json!({"view": true, "hide": ["terrain"]})).expect("a call naming the terrain");
    assert_eq!(named.hide, Some(vec!["terrain".to_owned()]), "and the names are the still's");

    // **`deny_unknown_fields` still holds**, so a misspelled parameter is a refusal rather than a silent no-op.
    assert!(
        serde_json::from_value::<bevy_cauldron_mcp::decide::DecideParams>(json!({"view": true, "hidden": ["terrain"]})).is_err(),
        "a misspelled parameter is refused, so a hide that never happened cannot read as one that did"
    );
}
