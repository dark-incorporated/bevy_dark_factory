//! **The guide's one arm switch: a section is not served, and the tool it names still is** (#457, DM-06).
//!
//! The issue measures two arms of the benchmark: the guide as it ships, and the guide with "Check your work
//! by asking" absent. The decision already made is that the without arm is **not a second copy of the guide**
//! — two copies drift, and an arm measured on a copy is not the arm a person is handed. So the server gained
//! `--omit-guide-sections`, the prose stayed in the one `guide.md`, and this test is what makes the switch a
//! switch rather than a claim:
//!
//! - **served with the flag absent**, the guide is `guide.md` byte for byte, section and all;
//! - **served with the section named**, the section is gone, every other section is there with its text, and
//!   the guide is shorter by the section and its heading only;
//! - **the tool the section names is still offered and still answers**, because the arm asks what an agent
//!   does without the guide's word, not without the tool;
//! - **a heading the guide does not have is refused at start**, not silently ignored: an arm that removed
//!   nothing would be reported as an arm it never ran (the rule `--hide-tools` already follows).
//!
//! **The server is called as an outside agent calls it**: a subprocess, MCP over stdio, on a port nothing
//! listens on. `--print-guide` is read too, because it is what a run names and hashes the arm by.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

/// **The section the two arms differ by**, exactly as `guide.md` spells its heading.
const OMITTED: &str = "## Check your work by asking";

/// **The sections on either side of it**, which an arm's cut must not touch.
const BEFORE: &str = "## Show your work";
const AFTER: &str = "## Hand back";

/// **A local port with nothing listening on it**: bind an ephemeral port, then let it go.
fn closed_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind an ephemeral port");
    listener.local_addr().expect("the bound port").port()
}

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    initialize: Value,
}

impl Server {
    /// **Start the server with `args`, exactly as the editor's MCP config would**, and shake hands.
    fn start(args: &[&str]) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron_mcp"))
            .args(args)
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
                   "clientInfo": {"name": "bevy_grimoire's arm-switch test", "version": "0.1.0"}}),
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

    /// **The guide as this server serves its shipped layer**, with no editor behind it.
    fn guide(&mut self) -> String {
        let answer = self.request("tools/call", json!({"name": "grimoire_guide", "arguments": {}}));
        answer["result"]["content"][0]["text"].as_str().unwrap_or_else(|| panic!("no guide text: {answer}")).to_owned()
    }

    /// The names in `tools/list`.
    fn tools(&mut self) -> Vec<String> {
        let listed = self.request("tools/list", json!({}));
        listed["result"]["tools"]
            .as_array()
            .unwrap_or_else(|| panic!("no tool list: {listed}"))
            .iter()
            .filter_map(|t| t["name"].as_str().map(str::to_owned))
            .collect()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// **The guide `guide.md` compiles to**, which is what the arm with nothing omitted must serve.
const SHIPPED: &str = include_str!("../src/guide.md");

/// **Run the server's `--print-guide`**, which is how a run names and hashes an arm: the arm's own arguments,
/// then `--print-guide`, so the text comes from the same parse the serving path reads.
fn printed(args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron_mcp"))
        .args(args)
        .arg("--print-guide")
        .output()
        .expect("the server binary runs");
    assert!(
        out.status.success(),
        "`--print-guide` failed ({}): {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).expect("the guide is UTF-8")
}

#[test]
fn with_nothing_omitted_the_guide_is_the_shipped_file() {
    // **The control arm**: an arm that names nothing must serve the guide as it is written, or the two arms
    // would differ by more than the section under test.
    let mut bare = Server::start(&[]);
    // **The served text starts with `guide.md`**, and then says the person's and the project's layers were not
    // read (no editor is listening here). The shipped layer is compared as a prefix, which is what an agent in
    // an unreachable editor is given; the layers are the editor's and are covered by `the_guide_has_the_editors_layers`.
    let served = bare.guide();
    assert!(
        served.starts_with(SHIPPED),
        "a server with no --omit-guide-sections does not serve `guide.md` first:\n{}",
        &served[..served.len().min(400)]
    );
    assert_eq!(printed(&[]), SHIPPED, "`--print-guide` with nothing omitted is not `guide.md`");
    // **An empty list is the same arm**, which is how `ask-with-guide` states itself in `variants.yaml`.
    assert_eq!(printed(&["--omit-guide-sections", ""]), SHIPPED, "an empty --omit-guide-sections changed the guide");
}

#[test]
fn the_named_section_is_not_served_and_every_other_section_is() {
    let args = ["--omit-guide-sections", "Check your work by asking"];
    let mut server = Server::start(&args);
    let guide = server.guide();

    assert!(!guide.contains(OMITTED), "the section is still served:\n{guide}");
    assert!(!guide.contains("grimoire_decide"), "the section's text is still served:\n{guide}");
    assert!(!guide.contains("would change your next operation"), "the section's second paragraph is still served:\n{guide}");
    // **Its neighbours, and their text, are untouched**, so the cut is the section and not the guide.
    for kept in [BEFORE, AFTER, "## Orient", "## Change", "## Hand back", "## After a task: reflect"] {
        assert!(guide.contains(kept), "the cut removed `{kept}` too:\n{guide}");
    }
    assert!(guide.contains("Let the editor show the change"), "\"Show your work\" lost its text:\n{guide}");
    assert!(guide.contains("Every task ends with `grimoire_present`"), "\"Hand back\" lost its text:\n{guide}");

    // **The guide is shorter by the section and nothing else**: from its heading's first byte to the next
    // heading's first byte, which is what a section's cut is.
    let from = SHIPPED.find(OMITTED).expect("the shipped guide has the section");
    let to = SHIPPED[from..]
        .find("\n## ")
        .map(|at| from + at + 1)
        .expect("a heading follows the section in the shipped guide");
    let section_len = to - from;
    // **The shipped layer alone**, read from `--print-guide`, because `grimoire_guide` appends a sentence
    // saying the person's and the project's guides were not read (no editor is listening here). The cut is
    // measured on the shipped layer, where the section lives.
    let served_shipped = printed(&args);
    let removed = SHIPPED.len() - served_shipped.len();
    assert_eq!(
        removed, section_len,
        "the arm removed {removed} bytes where the section is {section_len}: the cut is not the section alone"
    );
    // **The sections still come in the order they came in**, which a cut that took a heading's line and left
    // the text would break.
    assert!(
        guide.find(BEFORE) < guide.find(AFTER) && guide.find("## Orient") < guide.find(BEFORE),
        "the sections are out of order after the cut:\n{guide}"
    );

    // **`--print-guide` is the shipped layer**, so the served text starts with it; the rest is the sentence
    // saying the person's and the project's guides were not read, which needs an editor to read them.
    assert!(guide.starts_with(&printed(&args)), "`--print-guide` and `grimoire_guide` serve different guides");
}

#[test]
fn the_tool_the_section_names_is_still_offered_and_still_refuses_by_reaching_the_editor() {
    // **The arm is about the guide's word, not the tool.** A run that hid `grimoire_decide` as well would
    // answer "what does an agent do without the tool", which is #412's question with `--hide-tools`.
    let mut bare = Server::start(&[]);
    let mut arm = Server::start(&["--omit-guide-sections", "Check your work by asking"]);
    assert_eq!(
        bare.tools(),
        arm.tools(),
        "--omit-guide-sections changed the tool list; it is the guide's arm, not a tool's"
    );
    assert!(arm.tools().iter().any(|t| t == "grimoire_decide"), "the arm does not offer grimoire_decide");
    // **And it answers**: with no editor it says so rather than being absent, which is what the arm measures.
    let answer = arm.request(
        "tools/call",
        json!({"name": "grimoire_decide", "arguments": {"questions": {"bed": {"type": "noul", "instructions": "The bed is there."}}}}),
    );
    let text = answer["result"]["content"][0]["text"].as_str().unwrap_or_default();
    assert!(
        !text.is_empty(),
        "grimoire_decide answered nothing under the arm: {answer}"
    );
}

#[test]
fn a_heading_the_guide_has_not_is_refused_at_start() {
    // **An arm that removes nothing measures the whole guide while its report says it removed a section.**
    let out = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron_mcp"))
        .args(["--omit-guide-sections", "Check your work by asking please"])
        .env("BEVY_BRP_PORT", closed_port().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("the server binary runs");
    assert_eq!(out.status.code(), Some(2), "a heading the guide has not is not a refusal at start: {}", out.status);
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("Check your work by asking please"), "the refusal does not name the heading it refused: {said}");
    assert!(said.contains(OMITTED), "the refusal does not name the heading that would have matched: {said}");

    // **And the real heading is accepted**, which is what makes the refusal above the name and not the flag.
    let mut good = Server::start(&["--omit-guide-sections", OMITTED]);
    assert!(!good.guide().contains(OMITTED), "the heading with its `## ` is not accepted");
}

#[test]
fn the_sections_this_build_can_omit_are_the_guides_own() {
    // **The list a run reads is the guide's**, so a heading the guide renames is a list that changed rather
    // than an arm whose argument started failing.
    let out = Command::new(env!("CARGO_BIN_EXE_bevy_cauldron_mcp"))
        .arg("--list-guide-sections")
        .output()
        .expect("the server binary runs");
    assert!(out.status.success(), "--list-guide-sections failed: {}", String::from_utf8_lossy(&out.stderr));
    let listed: Vec<String> = String::from_utf8_lossy(&out.stdout).lines().map(str::to_owned).collect();
    let in_the_file: Vec<String> = SHIPPED
        .lines()
        .filter(|line| line.starts_with("## "))
        .map(|line| line.trim_end().to_owned())
        .collect();
    assert_eq!(listed, in_the_file, "the server lists different sections than `guide.md` has");
    assert!(listed.iter().any(|h| h == OMITTED), "the section the two arms differ by is not in the list: {listed:?}");
}
