//! **The guide tells an agent to check its work by asking** (#456, DM-05).
//!
//! Decision models (series #451) are an agent's fastest look at how its work looks. A tool nobody is told
//! to call is a tool nobody calls, so three things are shipped, and each has a test here:
//!
//! - **The guide's section**, "Check your work by asking", in the place where "Show your work" used to end
//!   with one sentence about uncertain judgements.
//! - **The tool's description**, which leads with the use rather than the mechanism.
//! - **The server's instructions**, which name `grimoire_decide` after `grimoire_guide` and never carry the
//!   guide's text (AG-01: the rule is in the guide, and the instructions only point to it).
//!
//! **The server is called as an outside agent calls it**: a subprocess, MCP over stdio, on a port nothing
//! listens on.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

/// **The heading the issue names.**
const HEADING: &str = "## Check your work by asking";
/// **The section it follows**: the old closing sentence of "Show your work" is gone, and this takes its place.
const SHOW_HEADING: &str = "## Show your work";
/// **The section it precedes**: a check comes before the hand-back.
const HAND_BACK_HEADING: &str = "## Hand back";
/// **How the tool's description starts**, the issue's sentence.
const LEAD: &str = "The fastest way to check how your work looks: one still and up to eight questions";

/// **The sentence the section replaces**, which sent an agent to the tool only "when a judgement is uncertain".
const OLD_SENTENCE: &str = "When a judgement is uncertain, ask a model rather than guessing.";

fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("an ephemeral port is available");
    listener.local_addr().expect("the listener has an address").port()
}

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    initialize: Value,
}

impl Server {
    fn start() -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron_mcp"))
            .env("BEVY_BRP_PORT", closed_port().to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the server binary starts");
        let stdin = child.stdin.take().expect("the server has a stdin");
        let stdout = BufReader::new(child.stdout.take().expect("the server has a stdout"));
        let mut server = Server { child, stdin, stdout, next_id: 0, initialize: Value::Null };
        server.initialize = server.request(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {},
                   "clientInfo": {"name": "bevy_grimoire's look-then-ask test", "version": "0.1.0"}}),
        );
        assert!(server.initialize.get("error").is_none(), "the server refused the handshake: {}", server.initialize);
        server.notify("notifications/initialized", json!({}));
        server
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        writeln!(self.stdin, "{}", json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))
            .expect("the server's stdin is writable");
        self.stdin.flush().expect("the server's stdin flushes");
        loop {
            let mut line = String::new();
            let read = self.stdout.read_line(&mut line).expect("the server's stdout is readable");
            assert!(read > 0, "the server's stdout ended before it answered");
            let value: Value = serde_json::from_str(line.trim_end()).expect("the server speaks JSON lines");
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                return value;
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        writeln!(self.stdin, "{}", json!({"jsonrpc": "2.0", "method": method, "params": params}))
            .expect("the server's stdin is writable");
        self.stdin.flush().expect("the server's stdin flushes");
    }

    fn guide(&mut self) -> String {
        let answer = self.request("tools/call", json!({"name": "grimoire_guide", "arguments": {}}));
        answer["result"]["content"][0]["text"].as_str().unwrap_or_else(|| panic!("no guide text: {answer}")).to_owned()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn section<'a>(guide: &'a str, heading: &str) -> &'a str {
    let start = guide.find(heading).unwrap_or_else(|| panic!("the guide has no `{heading}` section"));
    let rest = &guide[start + heading.len()..];
    let end = rest.find("\n## ").map_or(guide.len(), |at| start + heading.len() + at);
    &guide[start..end]
}

#[test]
fn the_guide_says_to_check_your_work_by_asking_and_says_how() {
    let guide = Server::start().guide();
    let asking = section(&guide, HEADING);
    assert!(
        guide.find(SHOW_HEADING) < guide.find(HEADING) && guide.find(HEADING) < guide.find(HAND_BACK_HEADING),
        "`{HEADING}` belongs between `{SHOW_HEADING}` and `{HAND_BACK_HEADING}`"
    );
    assert!(!guide.contains(OLD_SENTENCE), "the old closing sentence of \"Show your work\" is replaced, not kept beside the section");
    assert!(!section(&guide, SHOW_HEADING).contains("grimoire_decide"), "\"Show your work\" no longer carries the tool");

    for (what, needle) in [
        ("the tool", "`grimoire_decide`"),
        ("the one-call form", "`view: true`"),
        ("when: after a visible change", "after every change the person will see"),
        ("when: before the hand-back", "`grimoire_present`"),
        ("how many: four or five", "four or five"),
        ("how many: the cost of more", "more than eight"),
        ("a good question names something from the request", "names something"),
        ("prefer noul and choice", "`noul`"),
        ("prefer noul and choice", "`choice`"),
        ("a noul is a statement", "statement"),
        ("fix and re-ask at most twice", "at most twice"),
        ("report the rest", "hand-back"),
        ("a still from capture_still is accepted", "`grimoire_capture_still`"),
        ("advisory", "advisory"),
    ] {
        assert!(asking.to_lowercase().contains(&needle.to_lowercase()), "the section does not cover {what} (`{needle}`):\n{asking}");
    }
    // The owner's forest example: a request, and its questions with their types.
    assert!(asking.contains("pine forest"), "the forest example is missing:\n{asking}");
    for kind in ["`noul`", "`choice`", "`score`", "`floating`"] {
        assert!(asking.contains(kind), "the forest example does not use {kind}:\n{asking}");
    }
}

#[test]
fn the_tool_description_leads_with_the_use() {
    let mut server = Server::start();
    let listed = server.request("tools/list", json!({}));
    let decide = listed["result"]["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["name"] == "grimoire_decide"))
        .unwrap_or_else(|| panic!("the server offers no grimoire_decide: {listed}"));
    let description = decide["description"].as_str().expect("the tool has a description");
    assert!(description.starts_with(LEAD), "the description leads with the mechanism, not the use: {description}");
    assert!(description.contains("in about a second"), "{description}");
    let cap_mib = bevy_cauldron_mcp::decide::IMAGE_BYTES_MAX / (1024 * 1024);
    assert!(description.contains(&format!("at most {cap_mib} MiB")), "the description states the picture cap: {description}");
}

#[test]
fn the_instructions_name_the_tool_after_the_guide_and_never_carry_the_guide() {
    let server = Server::start();
    let instructions = server.initialize["result"]["instructions"].as_str().expect("the server has instructions");
    let guide_at = instructions.find("grimoire_guide").expect("the instructions name the guide");
    let decide_at = instructions.find("grimoire_decide").unwrap_or_else(|| panic!("the instructions never name grimoire_decide: {instructions}"));
    assert!(guide_at < decide_at, "the guide is named first: {instructions}");
    assert!(!instructions.contains("four or five"), "the rule is in the guide; the instructions only point to it: {instructions}");
    assert!(instructions.len() < 1200, "the instructions stay short ({} bytes): {instructions}", instructions.len());
}
