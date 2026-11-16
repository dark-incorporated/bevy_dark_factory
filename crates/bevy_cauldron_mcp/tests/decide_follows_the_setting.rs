//! **`grimoire_decide` asks the model the person's `decision_model` setting chooses** (DM-03, #454), and says where it ran from the
//! setting, not from comparing addresses.
//!
//! Two stand-in servers on loopback, one System One and one OpenRouter. Nothing leaves the machine; no environment is read.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bevy_cauldron_mcp::decide::{DecideParams, SELF_TEST_CHECK, decide_asked, self_test_check};
use bevy_oracle::{Endpoints, REMOTE_WITHOUT_KEY};
use serde_json::{Value, json};

struct Stub {
    url: String,
    hits: Arc<Mutex<usize>>,
}

impl Stub {
    fn serving(path: &str, body: &'static str) -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}{path}", listener.local_addr().expect("an address"));
        let hits = Arc::new(Mutex::new(0));
        let counted = Arc::clone(&hits);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
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
                        let len = head.lines().find_map(|l| l.strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
                        if req.len() >= end + 4 + len {
                            *counted.lock().expect("hits") += 1;
                            break;
                        }
                    }
                }
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            }
        });
        Stub { url, hits }
    }

    fn hits(&self) -> usize {
        *self.hits.lock().expect("hits")
    }
}

const SYSTEM_ONE: &str = r#"{"model":"d3-flash","answers":{"q":{"type":"noul","noul":0.9},"ready":{"type":"noul","noul":0.9}}}"#;
const OPENROUTER: &str = r#"{"id":"gen-1","model":"perplexity/pplx-decider-v1.3","provider":"Perplexity","answers":{"q":{"type":"noul","noul":0.8},"ready":{"type":"noul","noul":0.8}},"usage":{"input_tokens":3,"output_tokens":0,"cost":0}}"#;

fn folder(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mcp-setting-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets/grimoire")).expect("a project");
    dir
}

fn settings(dir: &Path, text: &str) -> PathBuf {
    let path = dir.join("settings.yaml");
    std::fs::write(&path, text).expect("a settings file");
    path
}

fn params() -> DecideParams {
    let questions: BTreeMap<String, Value> = [("q".to_owned(), json!({"type": "noul", "instructions": "Is it there?"}))].into();
    DecideParams { questions: Some(questions), state: Some("a state in words".into()), ..Default::default() }
}

fn servers() -> (Stub, Stub) {
    (Stub::serving("/v1/systemone", SYSTEM_ONE), Stub::serving("/api/alpha/decisions", OPENROUTER))
}

fn endpoints(local: &Stub, remote: &Stub, key: Option<&str>) -> Endpoints {
    Endpoints { local: local.url.clone(), remote: remote.url.clone(), key: key.map(str::to_owned) }
}

#[test]
fn local_sends_to_the_system_one_stub_only_and_says_local() {
    let (local, remote) = servers();
    let dir = folder("local");
    let user = settings(&dir, "version: 1\ndecision_model: local\n");
    let out = decide_asked(&params(), &dir, None, None, Some(&user), &endpoints(&local, &remote, Some("sk-or-test"))).expect("a decision");
    assert_eq!((out["where"].as_str(), out["model"].as_str()), (Some("local"), Some("d3-flash")), "{out}");
    assert_eq!((local.hits(), remote.hits()), (1, 0), "Local never reaches the OpenRouter stub");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn remote_sends_to_the_openrouter_stub_only_and_says_remote() {
    let (local, remote) = servers();
    let dir = folder("remote");
    let user = settings(&dir, "version: 1\ndecision_model: remote\n");
    let out = decide_asked(&params(), &dir, None, None, Some(&user), &endpoints(&local, &remote, Some("sk-or-test"))).expect("a decision");
    assert_eq!(out["where"], "remote", "{out}");
    assert_eq!((local.hits(), remote.hits()), (0, 1), "Remote never reaches the System One stub");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_lan_address_chosen_as_local_says_local() {
    // The stand-in is on a loopback address that is not the shipped default: `where` follows the setting, not the address.
    let (local, remote) = servers();
    assert_ne!(local.url, bevy_oracle::LOCAL_ENDPOINT);
    let dir = folder("lan");
    let out = decide_asked(&params(), &dir, None, None, None, &endpoints(&local, &remote, None)).expect("a decision");
    assert_eq!(out["where"], "local", "{out}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn remote_without_a_key_is_refused_naming_the_variable_and_nothing_is_sent() {
    let (local, remote) = servers();
    let dir = folder("nokey");
    let user = settings(&dir, "version: 1\ndecision_model: remote\n");
    let err = decide_asked(&params(), &dir, None, None, Some(&user), &endpoints(&local, &remote, None)).expect_err("refused");
    assert!(err.contains("OPENROUTER_API_KEY"), "{err}");
    assert_eq!(err, REMOTE_WITHOUT_KEY);
    assert_eq!((local.hits(), remote.hits()), (0, 0), "never answered by the other model");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_project_file_that_sets_remote_is_said_in_the_answer_and_changes_nothing() {
    let (local, remote) = servers();
    let dir = folder("project");
    std::fs::write(dir.join("assets/grimoire/editor.yaml"), "version: 1\ndecision_model: remote\n").unwrap();
    let out = decide_asked(&params(), &dir, None, None, None, &endpoints(&local, &remote, Some("k"))).expect("a decision");
    assert_eq!(out["where"], "local", "{out}");
    let said = out["settings_problems"][0].as_str().expect("the problem is said");
    assert!(said.contains("editor.yaml") && said.contains("decision_model"), "{said}");
    assert_eq!((local.hits(), remote.hits()), (1, 0));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_self_test_row_passes_against_the_stub_and_fails_with_the_reason_against_a_closed_port() {
    let (local, remote) = servers();
    let dir = folder("selftest");
    let user = settings(&dir, "version: 1\ndecision_model: local\n");
    let pass = self_test_check(Some(&dir), Some(&user), &endpoints(&local, &remote, None));
    assert_eq!((pass["name"].as_str(), pass["ok"].as_bool()), (Some(SELF_TEST_CHECK), Some(true)), "{pass}");
    let detail = pass["detail"].as_str().expect("a detail");
    assert!(detail.contains("d3-flash") && detail.contains("local") && detail.contains("ms"), "{detail}");

    let closed = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let down = Endpoints { local: format!("http://127.0.0.1:{closed}/v1/systemone"), ..Endpoints::default() };
    let fail = self_test_check(Some(&dir), Some(&user), &down);
    assert_eq!(fail["ok"], false, "{fail}");
    assert!(fail["detail"].as_str().unwrap().contains(&closed.to_string()), "the reason names the endpoint: {fail}");

    let remote_user = settings(&dir, "version: 1\ndecision_model: remote\n");
    let nokey = self_test_check(Some(&dir), Some(&remote_user), &endpoints(&local, &remote, None));
    assert_eq!((nokey["ok"].as_bool(), nokey["detail"].as_str()), (Some(false), Some(REMOTE_WITHOUT_KEY)));
    std::fs::remove_dir_all(dir).unwrap();
}
