//! **`blueprint_list_operations` can ask for one operation, and the guide says so** (#444, a wish from the style study).
//!
//! The list is every operation with every schema, which is more than an agent should have to scan to learn what one
//! operation takes. `operation` names one, and the editor answers with that operation's parameters and an example alone. This
//! server forwards the name; the compaction itself is the editor's, and is proved there
//! (`an_agent_can_ask_for_one_operations_help`). The guide points at it.
//!
//! The server is started as an agent's client starts it, against a stub editor that records what it is sent.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

type Calls = Arc<Mutex<Vec<(String, Value)>>>;

fn read_request(stream: &TcpStream) -> Value {
    let mut reader = BufReader::new(stream);
    let mut length = 0;
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("a request header line");
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some((name, value)) = line.split_once(':')
            && name.eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse().expect("a content length");
        }
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body).expect("the request body");
    serde_json::from_slice(&body).expect("the request is JSON")
}

/// **A stub editor** that answers `blueprint/operation_list` with one operation's help and records every call.
fn a_stub_editor(calls: Calls) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("a bound address").port();
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let calls = calls.clone();
            std::thread::spawn(move || {
                let request = read_request(&stream);
                let method = request["method"].as_str().unwrap_or_default().to_owned();
                let params = request["params"].clone();
                calls.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push((method.clone(), params.clone()));
                let body = match method.as_str() {
                    "blueprint/operation_list" if params["operation"] == "dta_set" => json!({
                        "jsonrpc": "2.0", "id": request["id"],
                        "error": {"code": -32000, "message": "`dta_set` is not an operation; the closest are `data_set`."},
                    }),
                    "blueprint/operation_list" => json!({
                        "jsonrpc": "2.0", "id": request["id"],
                        "result": {"operation": "data_set", "undo": "inverse", "parameters": [{"name": "file", "required": true}], "example": {"operation": "data_set", "parameters": {"file": "<file>"}}},
                    }),
                    _ => json!({"jsonrpc": "2.0", "id": request["id"], "result": {"known": true}}),
                }
                .to_string();
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            });
        }
    });
    port
}

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Server {
    fn start(port: u16) -> Server {
        let mut child = Command::new(env!("CARGO_BIN_EXE_dark_factory_mcp"))
            .env("BEVY_BRP_PORT", port.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("the server binary starts");
        let stdin = child.stdin.take().expect("the server has a stdin");
        let stdout = BufReader::new(child.stdout.take().expect("the server has a stdout"));
        let mut server = Server { child, stdin, stdout, next_id: 0 };
        let hello = server.request(
            "initialize",
            json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "list-test", "version": "1.0"}}),
        );
        assert!(hello.get("error").is_none(), "the handshake was refused: {hello}");
        server.notify("notifications/initialized", json!({}));
        server
    }

    fn notify(&mut self, method: &str, params: Value) {
        writeln!(self.stdin, "{}", json!({"jsonrpc": "2.0", "method": method, "params": params})).expect("stdin");
        self.stdin.flush().expect("stdin flushes");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        writeln!(self.stdin, "{}", json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).expect("stdin");
        self.stdin.flush().expect("stdin flushes");
        loop {
            let mut line = String::new();
            assert!(self.stdout.read_line(&mut line).expect("stdout is readable") > 0, "the server ended before it answered");
            let value: Value = serde_json::from_str(line.trim_end()).unwrap_or_else(|e| panic!("not JSON: {line} ({e})"));
            if value.get("id").and_then(Value::as_u64) == Some(id) {
                return value;
            }
        }
    }

    fn list(&mut self, arguments: Value) -> Value {
        self.request("tools/call", json!({"name": "blueprint_list_operations", "arguments": arguments}))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn sent(calls: &Calls) -> Vec<Value> {
    calls
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .filter(|(m, _)| m == "blueprint/operation_list")
        .map(|(_, p)| p.clone())
        .collect()
}

/// **The name reaches the editor, and the editor's answer comes back whole.**
#[test]
fn an_operation_name_is_forwarded_and_its_answer_returned() {
    let calls: Calls = Arc::default();
    let mut server = Server::start(a_stub_editor(calls.clone()));
    let got = server.list(json!({"operation": "data_set"}));
    assert!(got["result"]["isError"].as_bool() != Some(true), "{got}");
    assert_eq!(got["result"]["structuredContent"]["operation"], "data_set", "the editor's answer is returned: {got}");
    assert_eq!(sent(&calls), vec![json!({"query": null, "operation": "data_set"})], "the name reaches the editor with no query");

    // A refusal from the editor reaches the agent as a refusal, with its own words.
    let refused = server.list(json!({"operation": "dta_set"}));
    assert_eq!(refused["result"]["isError"].as_bool(), Some(true), "{refused}");
    assert!(refused.to_string().contains("data_set"), "the editor's closest operation is lost: {refused}");
}

/// **The tool's declared input has `operation`**, so a client that reads `tools/list` sends it.
#[test]
fn the_tool_declares_an_operation_parameter_and_says_what_it_is_for() {
    let mut server = Server::start(a_stub_editor(Arc::default()));
    let listed = server.request("tools/list", json!({}));
    let tool = listed["result"]["tools"]
        .as_array()
        .and_then(|tools| tools.iter().find(|t| t["name"] == "blueprint_list_operations"))
        .unwrap_or_else(|| panic!("the server offers no blueprint_list_operations: {listed}"));
    let operation = &tool["inputSchema"]["properties"]["operation"];
    assert!(operation.is_object(), "the input has no `operation`: {tool}");
    let description = tool["description"].as_str().unwrap_or_default();
    assert!(description.contains("operation") && description.contains("one operation"), "the tool does not say `operation` returns one operation: {description}");
}

/// **The guide points at the one-operation form, and says `blueprint_style` lists the topics that exist when it refuses one.**
#[test]
fn the_guide_points_at_one_operations_help_and_the_topics_that_exist() {
    let guide = dark_factory_mcp::docs::GUIDE;
    assert!(
        guide.contains("{\"operation\": \"data_set\"}") && guide.contains("one example"),
        "the guide does not tell an agent it can ask `blueprint_list_operations` for one operation and get its parameters and an example"
    );
    assert!(guide.contains("the topics that exist"), "the guide does not say that `blueprint_style` lists the topics that exist when it refuses one");
}
