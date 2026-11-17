//! **The decision model is one setting** (DM-03, #454): `decision_model: local` or `remote` in the person's settings file
//! chooses between a System One server and OpenRouter Decisions, and nothing else does.
//!
//! Two stand-in servers listen on loopback, one speaking System One and one OpenRouter; each records what reached it. No
//! live model is called and no process environment is read or written: where each choice goes is an [`Endpoints`] value.

use bevy_oracle::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A stand-in server: answers every request with `body` and keeps each request's head.
struct Stub {
    url: String,
    heads: Arc<Mutex<Vec<String>>>,
}

impl Stub {
    fn serving(path: &str, body: String) -> Stub {
        let listener = TcpListener::bind("127.0.0.1:0").expect("a loopback port");
        let url = format!("http://{}{path}", listener.local_addr().expect("an address"));
        let heads = Arc::new(Mutex::new(Vec::new()));
        let kept = Arc::clone(&heads);
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut s) = stream else { continue };
                let mut req = Vec::new();
                let mut buf = [0u8; 8192];
                // The whole request is read before the reply: closing with bytes unread sends a reset.
                loop {
                    let n = s.read(&mut buf).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    req.extend_from_slice(&buf[..n]);
                    if let Some(end) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&req[..end]).to_string();
                        let len = head
                            .to_ascii_lowercase()
                            .lines()
                            .find_map(|line| line.strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok()))
                            .unwrap_or(0);
                        if req.len() >= end + 4 + len {
                            kept.lock().expect("heads").push(head);
                            break;
                        }
                    }
                }
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Stub { url, heads }
    }

    fn hits(&self) -> Vec<String> {
        self.heads.lock().expect("heads").clone()
    }
}

/// What a System One server answers the one question with.
fn system_one_answer() -> String {
    r#"{"model":"d3-flash","answers":{"ready":{"type":"noul","noul":0.97}}}"#.to_owned()
}

/// What OpenRouter answers it with.
fn openrouter_answer() -> String {
    r#"{"id":"gen-dec-1","model":"perplexity/pplx-decider-v1.3","provider":"Perplexity","answers":{"ready":{"type":"noul","noul":0.95,"confidence":0.95}},"usage":{"input_tokens":5,"output_tokens":0,"cost":0}}"#.to_owned()
}

struct Servers {
    local: Stub,
    remote: Stub,
}

impl Servers {
    fn up() -> Servers {
        Servers { local: Stub::serving("/v1/systemone", system_one_answer()), remote: Stub::serving("/api/alpha/decisions", openrouter_answer()) }
    }

    fn endpoints(&self, key: Option<&str>) -> Endpoints {
        Endpoints { local: self.local.url.clone(), remote: self.remote.url.clone(), key: key.map(str::to_owned) }
    }
}

fn folder(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("oracle-setting-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a temp folder");
    dir
}

fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("a folder");
    std::fs::write(&path, text).expect("a file");
    path
}

fn one_question() -> Request {
    Request::new("ready").with_question("ready", Question::noul("Answer yes."))
}

#[test]
fn each_setting_value_sends_to_its_own_server_and_never_to_the_other() {
    for (value, own, other) in [("local", "local", "remote"), ("remote", "remote", "local")] {
        let servers = Servers::up();
        let dir = folder(value);
        let user = write(&dir, "settings.yaml", &format!("version: 1\ndecision_model: {value}\n"));
        let resolved = servers.endpoints(Some("sk-or-test")).resolve(Some(&user), None).expect("a resolved choice");
        assert_eq!(resolved.chosen.model.to_string(), value);

        let transport = Live::new(resolved.config);
        ask(&transport, &one_question()).unwrap_or_else(|e| panic!("{value}: {e}"));

        let (own_stub, other_stub) = match own {
            "local" => (&servers.local, &servers.remote),
            _ => (&servers.remote, &servers.local),
        };
        assert_eq!(own_stub.hits().len(), 1, "`{value}` sends to its own server once");
        assert!(other_stub.hits().is_empty(), "`{value}` never sends to the {other} server: {:?}", other_stub.hits());
        assert_eq!(transport.site().to_string(), value, "the transport says where it ran by the setting");
        std::fs::remove_dir_all(dir).expect("clean up");
    }
}

#[test]
fn only_remote_carries_the_key_and_only_remote_speaks_openrouter() {
    let servers = Servers::up();
    let dir = folder("wire");
    let local = write(&dir, "local.yaml", "version: 1\ndecision_model: local\n");
    let remote = write(&dir, "remote.yaml", "version: 1\ndecision_model: remote\n");
    let e = servers.endpoints(Some("sk-or-secret"));
    ask(&Live::new(e.resolve(Some(&local), None).unwrap().config), &one_question()).unwrap();
    ask(&Live::new(e.resolve(Some(&remote), None).unwrap().config), &one_question()).unwrap();
    let (l, r) = (servers.local.hits().remove(0).to_ascii_lowercase(), servers.remote.hits().remove(0).to_ascii_lowercase());
    assert!(l.starts_with("post /v1/systemone ") && !l.contains("authorization"), "Local sends no key: {l}");
    assert!(r.starts_with("post /api/alpha/decisions ") && r.contains("authorization: bearer sk-or-secret"), "{r}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn no_setting_at_all_is_local() {
    let servers = Servers::up();
    let dir = folder("unset");
    let e = servers.endpoints(Some("sk-or-test"));
    for user in [None, Some(dir.join("absent.yaml")), Some(write(&dir, "empty.yaml", "")), Some(write(&dir, "other.yaml", "version: 1\ntext_scale: 1.5\n"))] {
        let resolved = e.resolve(user.as_deref(), None).expect("local needs nothing");
        assert_eq!(resolved.chosen.model, DecisionModel::Local, "{user:?}");
        assert!(resolved.chosen.problems.is_empty(), "{user:?}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn remote_with_no_key_refuses_naming_the_variable_and_sends_nothing_anywhere() {
    let servers = Servers::up();
    let dir = folder("nokey");
    let user = write(&dir, "settings.yaml", "version: 1\ndecision_model: remote\n");
    for key in [None, Some("")] {
        let e = Endpoints::with_key_from(|_| key.map(str::to_owned));
        let e = Endpoints { local: servers.local.url.clone(), remote: servers.remote.url.clone(), ..e };
        let err = e.resolve(Some(&user), None).expect_err("Remote needs the key");
        assert!(matches!(err, Error::RemoteWithoutKey), "{err:?}");
        let said = err.to_string();
        assert!(said.contains("OPENROUTER_API_KEY"), "the refusal names the variable: {said}");
        assert_eq!(said, REMOTE_WITHOUT_KEY);
    }
    assert!(servers.local.hits().is_empty(), "never answered by the local model instead");
    assert!(servers.remote.hits().is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_project_file_that_sets_remote_is_a_problem_naming_it_and_the_choice_stays_the_persons() {
    let servers = Servers::up();
    let dir = folder("project");
    let project = write(&dir, "assets/blueprint/editor.yaml", "version: 1\ndecision_model: remote\n");
    let e = servers.endpoints(Some("sk-or-test"));

    // The person chose nothing: the project's Remote does not become theirs.
    let none = e.resolve(None, Some(&project)).unwrap();
    assert_eq!(none.chosen.model, DecisionModel::Local);
    assert_eq!(none.chosen.problems.len(), 1, "{:?}", none.chosen.problems);
    let problem = &none.chosen.problems[0];
    assert!(problem.file.ends_with("assets/blueprint/editor.yaml"), "the problem names the file: {problem}");
    assert!(problem.to_string().contains("decision_model") && problem.reason.contains("remote"), "{problem}");
    assert_eq!(none.config.endpoint, servers.local.url);

    // The person chose Local: still Local. The person chose Remote: still theirs, with the project's file ignored.
    let dir_user = write(&dir, "local.yaml", "version: 1\ndecision_model: local\n");
    assert_eq!(e.resolve(Some(&dir_user), Some(&project)).unwrap().chosen.model, DecisionModel::Local);
    let user_remote = write(&dir, "remote.yaml", "version: 1\ndecision_model: remote\n");
    let theirs = e.resolve(Some(&user_remote), Some(&project)).unwrap();
    assert_eq!((theirs.chosen.model, theirs.chosen.problems.len()), (DecisionModel::Remote, 1));

    // A project that sets Local says nothing the person has not: also not its to say.
    let local_project = write(&dir, "other/assets/blueprint/editor.yaml", "version: 1\ndecision_model: local\n");
    assert_eq!(e.resolve(Some(&user_remote), Some(&local_project)).unwrap().chosen.model, DecisionModel::Remote);
    assert!(servers.local.hits().is_empty() && servers.remote.hits().is_empty(), "resolving sends nothing");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_project_file_that_sets_nothing_here_is_no_problem() {
    let dir = folder("quiet");
    let project = write(&dir, "assets/blueprint/editor.yaml", "version: 1\ntext_scale: 1.5\n");
    let chosen = choose(None, Some(&project)).unwrap();
    assert_eq!((chosen.model, chosen.problems.is_empty()), (DecisionModel::Local, true));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_persons_file_that_does_not_read_asks_nothing_and_names_itself() {
    let servers = Servers::up();
    let dir = folder("bad");
    let e = servers.endpoints(Some("sk-or-test"));
    for (name, text) in [("typo.yaml", "version: 1\ndecision_model: remot\n"), ("not-yaml.yaml", "decision_model: [unclosed\n")] {
        let user = write(&dir, name, text);
        let err = e.resolve(Some(&user), None).expect_err("a half-read choice never falls back");
        let said = err.to_string();
        assert!(matches!(err, Error::Settings(_)) && said.contains(name) && said.contains("decision_model"), "{said}");
    }
    assert!(servers.local.hits().is_empty() && servers.remote.hits().is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_probe_names_the_model_where_it_ran_and_the_latency() {
    let servers = Servers::up();
    let e = servers.endpoints(Some("sk-or-test"));
    let local = probe(&Live::new(e.config(DecisionModel::Local).unwrap())).expect("the local stand-in answers");
    assert_eq!((local.model.as_str(), local.site), ("d3-flash", DecisionModel::Local));
    let remote = probe(&Live::new(e.config(DecisionModel::Remote).unwrap())).expect("the remote stand-in answers");
    assert_eq!((remote.model.as_str(), remote.site), ("perplexity/pplx-decider-v1.3", DecisionModel::Remote));
    assert!(local.latency < Duration::from_secs(5) && remote.latency < Duration::from_secs(5));
    // Each probe was one text-only request to its own server.
    assert_eq!((servers.local.hits().len(), servers.remote.hits().len()), (1, 1));
}

#[test]
fn the_probe_against_a_closed_port_fails_naming_the_endpoint() {
    let closed = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    let e = Endpoints { local: format!("http://127.0.0.1:{closed}/v1/systemone"), ..Endpoints::default() };
    let err = probe(&Live::new(e.config(DecisionModel::Local).unwrap())).expect_err("nothing is listening");
    assert!(matches!(err, Error::Unreachable { .. }) && err.to_string().contains(&closed.to_string()), "{err}");
}

#[test]
fn a_transport_that_does_not_say_where_it_ran_is_read_as_remote() {
    struct Silent;
    impl Transport for Silent {
        fn endpoint(&self) -> String {
            "http://192.168.1.5:8093/v1/systemone".into()
        }
        fn send(&self, _: &Request) -> Result<Response, Error> {
            Err(Error::Invalid("not used".into()))
        }
    }
    assert_eq!(Silent.site(), DecisionModel::Remote, "unknown is the one that may have sent pictures away");
    // And a LAN address chosen as Local reads `local`: where it ran is the setting, not a comparison of addresses.
    let lan = Config { endpoint: "http://192.168.1.5:8093/v1/systemone".into(), ..Config::default() };
    assert_eq!(Live::new(lan).site(), DecisionModel::Local);
}
