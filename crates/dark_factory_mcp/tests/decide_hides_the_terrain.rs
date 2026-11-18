//! **`blueprint_decide` honours `hide` for the turn it is asked in** (#472, acceptance 4).
//!
//! `blueprint_decide`'s `floating` and `clipping` checks take `view` themselves, so on an underground map they
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

use dark_factory_mcp::brp::Brp;
use dark_factory_mcp::decide::grab_through;
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

/// **A stand-in editor over BRP**, answering fairy's two capture methods and `blueprint/surface_hide`, and
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
    let mut cut: Option<f32> = None;
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
                "blueprint/surface_hide" => {
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
                "blueprint/section_cut" => {
                    let before = cut;
                    let asked_for = params.get("cut_m").and_then(Value::as_f64).map(|m| m as f32);
                    if let Some(m) = asked_for
                        && (!m.is_finite() || m <= 0.0)
                    {
                        let out = json!({"jsonrpc": "2.0", "id": req["id"], "error": {
                            "code": -32000,
                            "message": format!("`{m}` is not a distance the plane can stand at: `cut_m` is metres from the camera along its view and must be above zero. Nothing was changed.")}})
                        .to_string();
                        let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
                        continue;
                    }
                    cut = asked_for;
                    json!({"cut_m": cut, "was": before})
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

/// **The one call `blueprint_decide` makes for its `view`**, with `hide` naming the terrain.
#[tokio::test(flavor = "multi_thread")]
async fn the_picture_decide_asks_about_is_taken_with_the_terrain_hidden() {
    let editor = editor();
    let (png, was) = grab_through(&editor.brp(), "offscreen", Some(&["terrain".to_owned()]), None, true)
        .await
        .expect("the picture is taken");
    assert!(!png.is_empty(), "a picture came back");
    assert_eq!(was, Some(vec![]), "nothing was hidden before the call: {was:?}");

    let asked = editor.asked();
    let hides: Vec<&Asked> = asked.iter().filter(|a| a.method == "blueprint/surface_hide").collect();
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
    brp.call("blueprint/surface_hide", json!({"hide": ["water"]})).await.expect("the water is hidden");
    let (_png, was) = grab_through(&brp, "offscreen", Some(&["terrain".to_owned()]), None, true)
        .await
        .expect("the picture is taken");
    assert_eq!(was, Some(vec!["water".to_owned()]), "the call says what stood before it: {was:?}");
    let after = brp.call("blueprint/surface_hide", json!({"hide": ["water"]})).await.expect("asked again");
    assert_eq!(after["was"], json!(["water"]), "**the person's water is still hidden afterwards**: {after}");
}

/// **With no `hide` the editor is not asked at all** — no round trip, so nothing on the view can move. That is
/// the parameter's default, and it is the still's: an unused call changes nothing.
#[tokio::test(flavor = "multi_thread")]
async fn with_no_hide_the_view_is_never_touched() {
    let editor = editor();
    let (_png, was) = grab_through(&editor.brp(), "offscreen", None, None, true).await.expect("the picture is taken");
    assert_eq!(was, None, "the call names no view, so it reports none");
    let asked = editor.asked();
    assert!(
        asked.iter().all(|a| a.method != "blueprint/surface_hide"),
        "**the editor is never asked to hide anything**: {asked:#?}"
    );
    assert_eq!(asked.iter().filter(|a| a.method == "bevy_fairy/screenshot").count(), 1, "and one picture was taken");
}

/// **A name the editor does not draw is refused in the editor's own words**, never silently ignored, and the
/// refusal is the still's — because it is the same `blueprint/surface_hide` verb under both tools.
#[tokio::test(flavor = "multi_thread")]
async fn a_surface_the_editor_does_not_draw_is_refused_naming_the_list() {
    let editor = editor();
    let said = grab_through(&editor.brp(), "offscreen", Some(&["terain".to_owned()]), None, true)
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
/// parameter is the one `blueprint_capture_still` has, not a second vocabulary a caller has to learn.
#[test]
fn the_parameters_hide_is_the_stills_own_and_defaults_to_none() {
    let plain: dark_factory_mcp::decide::DecideParams = serde_json::from_value(json!({"view": true})).expect("a call with no hide");
    assert_eq!(plain.hide, None, "a call that names no hide hides nothing");

    let named: dark_factory_mcp::decide::DecideParams =
        serde_json::from_value(json!({"view": true, "hide": ["terrain"]})).expect("a call naming the terrain");
    assert_eq!(named.hide, Some(vec!["terrain".to_owned()]), "and the names are the still's");

    // **`deny_unknown_fields` still holds**, so a misspelled parameter is a refusal rather than a silent no-op.
    assert!(
        serde_json::from_value::<dark_factory_mcp::decide::DecideParams>(json!({"view": true, "hidden": ["terrain"]})).is_err(),
        "a misspelled parameter is refused, so a hide that never happened cannot read as one that did"
    );
}

/// **`cut_m` is the caller's own number, it reaches the camera, and it is put back after the picture.**
///
/// **Why a plane at all**, because the code will outlive this comment: `hide` alone draws the interior's *near*
/// surface — the vault and the far walls — because front-face culling keeps the faces pointing away from the
/// camera, and the floor is **behind** the vault along every ray from above. Measured on #472 against the same SDF
/// the editor meshes: three cull modes, none of which reaches a floor. A plane removes geometry instead of
/// choosing faces, so the floor is what remains — and how much to remove is where the room's near wall stands,
/// which is a property of the room, not of the camera. That is why the caller states it and the editor does not
/// derive it.
///
/// **The four column caps at 18.4 m from one still are NOT satisfiable by a depth cut**, and this test does not
/// pretend otherwise: a depth cut is a plane in *view* space, so it cuts each column at a different height — at
/// the value that shows the crypt's floor, the four columns read at 13.4, 3.4, 3.4 and 7.5 m. A true horizontal
/// plan cut is the follow-up that would show all four caps at once, and it needs geometry the editor does not
/// have today.
#[tokio::test(flavor = "multi_thread")]
async fn cut_m_reaches_the_camera_and_is_put_back_after_the_picture() {
    let editor = editor();
    let brp = editor.brp();
    // **The camera's own near is whatever it was**, and the call must not be the thing that decides it.
    let before = brp.call("blueprint/section_cut", json!({"cut_m": null})).await.expect("the camera answers");
    assert_eq!(before["cut_m"], Value::Null, "no plane stands to begin with: {before}");

    let (_png, was) = grab_through(&brp, "offscreen", Some(&["terrain".to_owned()]), Some(31.0), true)
        .await
        .expect("the picture is taken");
    assert_eq!(was, Some(vec![]), "the call reports what was hidden before it");

    let cuts: Vec<_> = editor.asked().into_iter().filter(|a| a.method == "blueprint/section_cut").collect();
    // The first is this test's own look at what stood before it; the capture's are the two after it.
    assert_eq!(cuts.len(), 3, "this test's own call, then the plane placed and put back: {cuts:#?}");
    assert_eq!(cuts[1].params["cut_m"], json!(31.0), "**the plane is the caller's own number**, not a constant");
    assert_eq!(cuts[2].params["cut_m"], Value::Null, "and it is put back to the camera's own near");
}

/// **A refused capture still puts the plane back.** An editor that fails the picture must not leave the person
/// looking at the inside of a hill, which is the failure the surfaces' restore was written for and this repeats.
#[tokio::test(flavor = "multi_thread")]
async fn a_refused_capture_still_puts_the_plane_back() {
    let editor = editor();
    let brp = editor.brp();
    // **A surface name the editor does not draw** refuses the call after the plane is already placed, because the
    // plane is placed first (it is what makes the interior visible). The restore must still run.
    let said = grab_through(&brp, "offscreen", Some(&["terain".to_owned()]), Some(31.0), true)
        .await
        .expect_err("a typo is refused");
    assert!(said.contains("terain"), "{said}");
    let cuts: Vec<_> = editor.asked().into_iter().filter(|a| a.method == "blueprint/section_cut").collect();
    assert_eq!(cuts.len(), 2, "the plane was placed, and put back despite the refusal: {cuts:#?}");
    assert_eq!(cuts[0].params["cut_m"], json!(31.0), "placed first, which is what makes the interior visible");
    assert_eq!(cuts[1].params["cut_m"], Value::Null, "the camera's own near is restored");
}

/// **`cut_m` without `hide` is refused, in one sentence, before the editor is asked anything** — the rule both
/// tools share, because a plane with nothing hidden removes the air in front of a room that is still buried.
#[test]
fn a_plane_without_a_hide_is_refused_by_the_one_rule_both_tools_call() {
    use dark_factory_mcp::decide::check_cut;
    let said = check_cut(None, Some(31.0)).expect_err("a plane with no hide is refused");
    assert!(said.contains("cut_m: 31") && said.contains("needs `hide`"), "it names both and the rule: {said}");
    let said = check_cut(Some(&[]), Some(31.0)).expect_err("an empty hide is no hide");
    assert!(said.contains("needs `hide`"), "{said}");
    // **And the numbers a plane cannot stand at** are refused with their own sentence.
    let said = check_cut(Some(&["terrain".to_owned()]), Some(0.0)).expect_err("zero is not a distance");
    assert!(said.contains("above zero"), "{said}");
    let said = check_cut(Some(&["terrain".to_owned()]), Some(-4.0)).expect_err("negative is not a distance");
    assert!(said.contains("above zero"), "{said}");
    // **The coherent calls pass**, which is what keeps the rule from refusing the tool's ordinary use.
    assert!(check_cut(Some(&["terrain".to_owned()]), Some(31.0)).is_ok());
    assert!(check_cut(Some(&["terrain".to_owned()]), None).is_ok(), "a still with no plane is the ordinary call");
    assert!(check_cut(None, None).is_ok(), "and a still with neither is the plainest call there is");
}

/// **A still with no `hide` and no `cut_m` asks the editor for nothing but the picture**, which is the shape
/// acceptance 2 needs: the option changes nothing when unused.
#[tokio::test(flavor = "multi_thread")]
async fn no_hide_and_no_cut_m_asks_the_editor_for_nothing_but_a_picture() {
    let editor = editor();
    let brp = editor.brp();
    let (_png, was) = grab_through(&brp, "offscreen", None, None, true).await.expect("the picture is taken");
    assert_eq!(was, None, "the call names no view");
    let asked = editor.asked();
    assert!(
        asked.iter().all(|a| a.method != "blueprint/section_cut"),
        "**no plane is placed**: {asked:#?}"
    );
    assert!(asked.iter().all(|a| a.method != "blueprint/surface_hide"), "and nothing is hidden");
    assert_eq!(asked.iter().filter(|a| a.method == "bevy_fairy/screenshot").count(), 1, "one picture");
}
