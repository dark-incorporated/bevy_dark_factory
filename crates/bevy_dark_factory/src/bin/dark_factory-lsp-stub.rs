//! **`dark_factory-lsp-stub`: a fixed Language Server Protocol peer, for the Files editor's tests** (#698).
//!
//! The editor starts its Rust server through one path ([`bevy_dark_factory::files_lsp`]), from one setting: the program
//! [`bevy_dark_factory::files_lsp::RustAnalyzer::with_program`] names. A test names this program there in place of
//! `rust-analyzer`, so what the editor does with a server's completions, hovers and findings is measured against answers
//! the test knows, and without rust-analyzer installed. Nothing in the editor knows this program.
//!
//! It speaks JSON-RPC in `Content-Length` frames on its standard input and output, and answers:
//!
//! - `initialize`: UTF-8 columns, changes as edits (incremental sync), saves wanted, completion and hover.
//! - `textDocument/didOpen`, `didChange` (an edit or the whole text) and `didSave`: it keeps the text, and publishes one
//!   information finding over the file's first three bytes saying what it holds (`held`).
//! - `textDocument/completion`: four items (`completion`), of which the editor's list can write two.
//! - `textDocument/hover`: `stub hover on `<word>``, about the word at the position in the text it holds.
//! - `shutdown`, then `exit`; any other request is answered as a method it does not have.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Write};

use serde_json::{Value, json};

/// One document as the editor told it.
struct Document {
    version: i64,
    text: String,
    saves: u64,
}

fn main() {
    let mut input = BufReader::new(io::stdin().lock());
    let mut output = io::stdout().lock();
    let mut documents: BTreeMap<String, Document> = BTreeMap::new();
    while let Some(message) = read(&mut input) {
        let method = message.get("method").and_then(Value::as_str).unwrap_or_default();
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let uri = params.pointer("/textDocument/uri").and_then(Value::as_str).unwrap_or_default().to_owned();
        let Some(id) = message.get("id").cloned() else {
            // **A notification**: the document it is about changes, and what it holds now is published.
            match method {
                "exit" => return,
                "textDocument/didOpen" => {
                    let text = params.pointer("/textDocument/text").and_then(Value::as_str).unwrap_or_default().to_owned();
                    let version = params.pointer("/textDocument/version").and_then(Value::as_i64).unwrap_or(0);
                    documents.insert(uri.clone(), Document { version, text, saves: 0 });
                }
                "textDocument/didChange" => {
                    let Some(document) = documents.get_mut(&uri) else { continue };
                    document.version = params.pointer("/textDocument/version").and_then(Value::as_i64).unwrap_or(document.version);
                    for change in params.get("contentChanges").and_then(Value::as_array).into_iter().flatten() {
                        apply(&mut document.text, change);
                    }
                }
                "textDocument/didSave" => {
                    let Some(document) = documents.get_mut(&uri) else { continue };
                    document.saves += 1;
                }
                "textDocument/didClose" => {
                    documents.remove(&uri);
                    continue;
                }
                _ => continue,
            }
            if let Some(document) = documents.get(&uri) {
                let finding = json!({ "range": range(0, 0, 0, 3), "severity": 3, "source": "stub", "message": held(document) });
                send(&mut output, &json!({ "jsonrpc": "2.0", "method": "textDocument/publishDiagnostics", "params": { "uri": uri, "diagnostics": [finding] } }));
            }
            continue;
        };
        if method.is_empty() {
            // An answer to a request of its own: it makes none.
            continue;
        }
        let at = params.get("position").map(|p| (field(p, "line"), field(p, "character")));
        let result = match (method, at) {
            ("initialize", _) => json!({ "capabilities": {
                "positionEncoding": "utf-8",
                "textDocumentSync": { "openClose": true, "change": 2, "save": { "includeText": false } },
                "completionProvider": { "triggerCharacters": [".", ":"] },
                "hoverProvider": true,
            } }),
            ("shutdown", _) => Value::Null,
            ("textDocument/completion", Some((line, character))) => {
                let text = documents.get(&uri).map_or("", |d| d.text.as_str());
                completion(text, line, character)
            }
            ("textDocument/hover", Some((line, character))) => {
                let text = documents.get(&uri).map_or("", |d| d.text.as_str());
                let (start, end) = word(text, line, character, true);
                let said = line_of(text, line).get(start..end).unwrap_or_default().to_owned();
                json!({ "contents": { "kind": "plaintext", "value": format!("stub hover on `{said}`") }, "range": range(line, start, line, end) })
            }
            _ => {
                send(&mut output, &json!({ "jsonrpc": "2.0", "id": id, "error": { "code": -32601, "message": format!("the stub does not answer {method}") } }));
                continue;
            }
        };
        send(&mut output, &json!({ "jsonrpc": "2.0", "id": id, "result": result }));
    }
}

/// **What the stub says it holds**, the message of the one finding it publishes: the document's version, how many saves
/// it was told of, and its whole text, quoted as Rust's `{:?}` quotes it.
fn held(document: &Document) -> String {
    format!("stub v{}, {} saves, holds {:?}", document.version, document.saves, document.text)
}

/// **The fixed completion answer at a position**, in an order its `sortText` puts right: `answer_stub` replaces the word
/// before the position with `answer_stub()`, `another_stub` writes its label; `imported_stub` brings an import and
/// `postfix_stub` rewrites from the line's start, which the editor's list cannot apply.
fn completion(text: &str, line: usize, character: usize) -> Value {
    let (start, _) = word(text, line, character, false);
    json!({ "isIncomplete": false, "items": [
        { "label": "postfix_stub", "detail": "a rewrite", "sortText": "3", "textEdit": { "range": range(line, 0, line, character), "newText": "postfix" } },
        { "label": "another_stub", "detail": "const u32", "sortText": "2" },
        { "label": "answer_stub", "detail": "fn() -> u32", "sortText": "1", "textEdit": { "range": range(line, start, line, character), "newText": "answer_stub()" } },
        { "label": "imported_stub", "detail": "an import", "sortText": "0", "additionalTextEdits": [{ "range": range(0, 0, 0, 0), "newText": "use stub::imported_stub;\n" }] },
    ] })
}

/// **Apply one change the editor sent**: an edit over its range (UTF-8 columns, as `initialize` said), or the whole text.
fn apply(text: &mut String, change: &Value) {
    let new = change.get("text").and_then(Value::as_str).unwrap_or_default();
    let Some(range) = change.get("range") else {
        new.clone_into(text);
        return;
    };
    let offset = |at: &str| range.get(at).map_or(0, |p| offset(text, field(p, "line"), field(p, "character")));
    let (start, end) = (offset("start"), offset("end"));
    if start <= end && end <= text.len() && text.is_char_boundary(start) && text.is_char_boundary(end) {
        text.replace_range(start..end, new);
    }
}

/// A position's number field.
fn field(position: &Value, name: &str) -> usize {
    position.get(name).and_then(Value::as_u64).and_then(|n| usize::try_from(n).ok()).unwrap_or(0)
}

/// The byte offset of a line and a UTF-8 column; past the text is its end.
fn offset(text: &str, line: usize, character: usize) -> usize {
    let start = if line == 0 { Some(0) } else { text.match_indices('\n').nth(line - 1).map(|(at, _)| at + 1) };
    start.map_or(text.len(), |start| (start + character).min(text.len()))
}

/// Line `line` of the text, without its line break.
fn line_of(text: &str, line: usize) -> &str {
    text.split('\n').nth(line).unwrap_or_default()
}

/// **The word at a column of a line**, as columns: the letters, digits and underscores before it, and after it too when
/// `around` (a hover's word; a completion's is only what was typed before the caret).
fn word(text: &str, line: usize, character: usize, around: bool) -> (usize, usize) {
    let line = line_of(text, line).as_bytes();
    let is_word = |b: &u8| b.is_ascii_alphanumeric() || *b == b'_';
    let at = character.min(line.len());
    let start = at - line[..at].iter().rev().take_while(|b| is_word(b)).count();
    let end = if around { at + line[at..].iter().take_while(|b| is_word(b)).count() } else { at };
    (start, end)
}

fn range(start_line: usize, start: usize, end_line: usize, end: usize) -> Value {
    json!({ "start": { "line": start_line, "character": start }, "end": { "line": end_line, "character": end } })
}

/// **One frame** from the editor: headers to a blank line, then `Content-Length` bytes of JSON. `None` at the end.
fn read(input: &mut impl BufRead) -> Option<Value> {
    let mut length = None;
    let mut line = String::new();
    loop {
        line.clear();
        if input.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let mut body = vec![0; length?];
    input.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

/// One frame to the editor. A pipe that broke ends nothing here: the next read finds the end.
fn send(output: &mut impl Write, message: &Value) {
    let body = message.to_string();
    let _ = write!(output, "Content-Length: {}\r\n\r\n{body}", body.len()).and_then(|()| output.flush());
}
