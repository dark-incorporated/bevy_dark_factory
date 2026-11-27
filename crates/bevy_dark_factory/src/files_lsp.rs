//! **rust-analyzer behind the Files editor** (#611): the compiler's own errors on a Rust file, drawn over the colours
//! the editor's tokenizer already gives it.
//!
//! # Which server, and when it runs
//!
//! One server, `rust-analyzer` ([`PROGRAM`]), for `.rs` files only (owner, 2026-11-27). It is started **once per
//! project**: when the open project's root holds a `Cargo.toml` and a `.rs` file is open, from the project's root. It
//! lives until the project closes or another one opens: [`RustAnalyzer`] owns the process, and dropping the process
//! kills and reaps it, so a closed project or a closed editor leaves no server behind. The editor speaks the Language
//! Server Protocol to it over its standard input and output (JSON-RPC in `Content-Length` frames): `initialize`,
//! `initialized`, then `textDocument/didOpen`, `didChange` (the whole text) and `didClose` as Rust files open, change
//! and close.
//!
//! # What it adds, and what stays
//!
//! The tokenizer ([`crate::files_lang`]) colours, folds and outlines every file, a Rust file included, whether a server
//! runs or not: that is the baseline, not a fallback. The server adds what it publishes
//! (`textDocument/publishDiagnostics`) to the editor's own findings, through the one [`bevy_gooey::Diagnostics`] the
//! loader's readers fill (`files::decorate_the_editor`). Completion and hover are not asked of it yet.
//!
//! # When it is absent
//!
//! A machine without `rust-analyzer` on `PATH` gets one problem on the status line saying so and how to install it
//! ([`absent_line`]), once per project, and the editor goes on with its own colours and no compiler errors. A server
//! that stops is said the same way, once, with the last line it wrote to its error stream, and is not started again
//! for that project.
//!
//! # Off the main thread
//!
//! The process's pipes are read and written by threads of its own (`Server::spawn`): one reads frames from its
//! output, one writes the editor's frames to its input, one keeps the last line of its error stream. They talk to the
//! frame through channels, which [`talk_to_rust_analyzer`] drains once a frame without waiting.

use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Mutex, PoisonError};
use std::thread::JoinHandle;

use bevy::prelude::*;
use bevy_gooey::{CodeSeverity, Diagnostic};
use lsp_types::notification::{DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Initialized, Notification, PublishDiagnostics, ShowMessage};
use lsp_types::request::{Initialize, Request};
use lsp_types::{
    ClientCapabilities, ClientInfo, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams, GeneralClientCapabilities,
    InitializeParams, InitializedParams, MessageType, NumberOrString, Position, PositionEncodingKind, PublishDiagnosticsClientCapabilities, PublishDiagnosticsParams,
    ShowMessageParams, TextDocumentClientCapabilities, TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem, Uri, VersionedTextDocumentIdentifier,
    WorkspaceFolder,
};
use serde_json::{Value, json};

use crate::editor::EditorState;
use crate::files::Files;
use crate::project::Project;

/// **The program started for Rust files**, looked for on `PATH`.
pub const PROGRAM: &str = "rust-analyzer";

/// **What a project's root holds for its server to be started**: a Cargo package or workspace.
pub const MANIFEST: &str = "Cargo.toml";

/// The files the server is told about, by extension.
pub const EXTENSION: &str = ".rs";

/// The JSON-RPC id of the one request the editor makes.
const INITIALIZE_ID: i64 = 1;

/// JSON-RPC's code for a method the receiver does not have: the answer to every request the server makes.
const METHOD_NOT_FOUND: i64 = -32601;

/// **The status line when the server cannot be started**: what is missing, what the person keeps, and how to install it.
pub fn absent_line(program: &Path, why: &io::Error) -> String {
    let program = program.display();
    if why.kind() == io::ErrorKind::NotFound {
        format!(
            "{program} is not installed (it is not on PATH): Rust files keep the editor's own colours but show no compiler errors. Install it with `rustup component add rust-analyzer`"
        )
    } else {
        format!("{program} could not be started ({why}): Rust files keep the editor's own colours but show no compiler errors")
    }
}

/// **The status line when a running server stopped**, with the last thing it said.
pub fn stopped_line(program: &Path, why: &str) -> String {
    format!("{} stopped ({why}): Rust files keep the editor's own colours but show no compiler errors until the project is opened again", program.display())
}

/// Whether `rel` is a file the server is told about.
pub fn is_rust(rel: &str) -> bool {
    rel.ends_with(EXTENSION)
}

// ── The state the frame keeps ────────────────────────────────────────────────

/// **The open project's Rust server**, when it has one, and what the server has published for the open files.
#[derive(Resource)]
pub struct RustAnalyzer {
    /// The program started: [`PROGRAM`], unless a test names one that is not installed.
    program: PathBuf,
    /// The project root this state belongs to: another root drops the server and everything it published.
    root: Option<PathBuf>,
    /// The running server.
    server: Option<Server>,
    /// Whether this project's server was settled without one running: it could not start, stopped, or the project
    /// has no `Cargo.toml`. Said once, and not tried again until another project opens.
    settled: bool,
    /// What the server published last, by path under the project, for the files it holds.
    published: BTreeMap<String, Published>,
    /// Counts every publication, so an editor can tell a new one from the one it drew.
    generation: u64,
}

impl Default for RustAnalyzer {
    fn default() -> Self {
        RustAnalyzer::with_program(PROGRAM)
    }
}

impl RustAnalyzer {
    /// The state for a server started as `program`.
    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        RustAnalyzer { program: program.into(), root: None, server: None, settled: false, published: BTreeMap::new(), generation: 0 }
    }

    /// Whether a server is running for the open project.
    pub fn is_running(&self) -> bool {
        self.server.is_some()
    }

    /// **Which publication for `rel` the editor would draw**: 0 when there is none, so an editor that drew one sees the
    /// change when it goes.
    pub fn generation_of(&self, rel: &str) -> u64 {
        self.published.get(rel).map_or(0, |p| p.generation)
    }

    /// **The server's findings on `rel`, placed in `text`**: a position past the text's end is held at it.
    pub fn diagnostics_for(&self, rel: &str, text: &str) -> Vec<Diagnostic> {
        let Some(published) = self.published.get(rel) else { return Vec::new() };
        let lines = line_starts(text);
        published
            .diagnostics
            .iter()
            .map(|d| {
                let start = offset_of(text, &lines, d.range.start, published.encoding);
                let end = offset_of(text, &lines, d.range.end, published.encoding).max(start);
                Diagnostic::new(start..end, severity_of(d.severity), message_of(d))
            })
            .collect()
    }

    /// Let the server go (its process is killed) and everything it published.
    fn forget(&mut self) {
        self.server = None;
        self.published.clear();
    }
}

/// One file's latest publication.
struct Published {
    generation: u64,
    /// How the server counts columns, so the positions are read the way they were written.
    encoding: Encoding,
    diagnostics: Vec<lsp_types::Diagnostic>,
}

/// **How a position's column is counted**: the protocol's default is UTF-16 code units; the editor asks for UTF-8 bytes
/// first, which is what its own ranges are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Encoding {
    Utf8,
    Utf16,
}

// ── The process ──────────────────────────────────────────────────────────────

/// **The server's process, killed and reaped when it is dropped**: a closed project leaves no server running.
struct Process(Child);

impl Drop for Process {
    fn drop(&mut self) {
        // A process that already exited answers the kill with an error, and that is the state wanted.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// What the reading thread hands the frame.
enum FromServer {
    Message(Value),
    /// The server's output closed: why, as well as can be said.
    Closed(String),
}

/// **One running server**: its process, the two channels to its threads, and the documents it holds.
struct Server {
    /// Dropped first (fields drop in the order they are declared), so the writing thread ends; the process goes last.
    to: Sender<Value>,
    /// Behind a lock only because a resource is shared between threads and a receiver is not: the frame is its one reader,
    /// and reads it through `get_mut`, which takes no lock.
    from: Mutex<Receiver<FromServer>>,
    /// The `initialize` answer came and `initialized` was sent: documents may be opened.
    ready: bool,
    encoding: Encoding,
    /// The project root as the server was told it, canonical, so the paths it names come back the same.
    root: PathBuf,
    /// The documents the server holds, by path under the project.
    documents: BTreeMap<String, Document>,
    _process: Process,
}

/// One document as the server was last told it.
struct Document {
    uri: Uri,
    version: i32,
    text: String,
}

impl Server {
    /// **Start `program` in `root`** and send it `initialize`; its pipes go to three threads of their own.
    fn spawn(program: &Path, root: &Path) -> io::Result<Server> {
        let root = std::fs::canonicalize(root)?;
        let params = initialize_params(&root).ok_or_else(|| io::Error::other(format!("{} is not a path a server can be told", root.display())))?;
        let mut child = Command::new(program).current_dir(&root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
        let pipes = (child.stdin.take(), child.stdout.take(), child.stderr.take());
        // Held from here, so any refusal below kills what was started.
        let process = Process(child);
        let (Some(input), Some(output), Some(errors)) = pipes else { return Err(io::Error::other("its pipes were not opened")) };
        let (to, outgoing) = channel::<Value>();
        let (incoming, from) = channel::<FromServer>();
        let errors = std::thread::Builder::new().name("rust-analyzer stderr".into()).spawn(move || last_line(errors))?;
        std::thread::Builder::new().name("rust-analyzer out".into()).spawn(move || read_frames(output, &incoming, errors))?;
        std::thread::Builder::new().name("rust-analyzer in".into()).spawn(move || write_frames(input, &outgoing))?;
        let server = Server { to, from: Mutex::new(from), ready: false, encoding: Encoding::Utf16, root, documents: BTreeMap::new(), _process: process };
        server.send(json!({ "jsonrpc": "2.0", "id": INITIALIZE_ID, "method": Initialize::METHOD, "params": params }));
        Ok(server)
    }

    /// Hand one message to the writing thread. A writer that has gone is heard as the output closing, so nothing is said here.
    fn send(&self, message: Value) {
        let _ = self.to.send(message);
    }

    fn notify<N: Notification>(&self, params: N::Params) {
        match serde_json::to_value(params) {
            Ok(params) => self.send(json!({ "jsonrpc": "2.0", "method": N::METHOD, "params": params })),
            Err(why) => warn!("the {} notification for rust-analyzer did not serialise: {why}", N::METHOD),
        }
    }

    /// **The `file:` URI of `rel`** under the root the server was told.
    fn uri_of(&self, rel: &str) -> Option<Uri> {
        uri_of(&self.root.join(rel))
    }

    /// **The path under the project a URI names**, `/`-separated as the Files workspace keys its buffers.
    fn rel_of(&self, uri: &Uri) -> Option<String> {
        let path = url::Url::parse(uri.as_str()).ok()?.to_file_path().ok()?;
        let rel = path.strip_prefix(&self.root).ok()?;
        Some(rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"))
    }
}

/// **The `initialize` request's parameters**: the project as the one workspace folder, UTF-8 columns asked for first, and
/// versioned diagnostics.
fn initialize_params(root: &Path) -> Option<Value> {
    let uri = uri_of(root)?;
    let name = root.file_name().map_or_else(|| root.display().to_string(), |n| n.to_string_lossy().into_owned());
    let params = InitializeParams {
        process_id: Some(std::process::id()),
        workspace_folders: Some(vec![WorkspaceFolder { uri, name }]),
        capabilities: ClientCapabilities {
            general: Some(GeneralClientCapabilities { position_encodings: Some(vec![PositionEncodingKind::UTF8, PositionEncodingKind::UTF16]), ..Default::default() }),
            text_document: Some(TextDocumentClientCapabilities {
                publish_diagnostics: Some(PublishDiagnosticsClientCapabilities { version_support: Some(true), ..Default::default() }),
                ..Default::default()
            }),
            ..Default::default()
        },
        client_info: Some(ClientInfo { name: "Dark Factory".to_owned(), version: Some(env!("CARGO_PKG_VERSION").to_owned()) }),
        ..Default::default()
    };
    serde_json::to_value(params).ok()
}

fn uri_of(path: &Path) -> Option<Uri> {
    url::Url::from_file_path(path).ok()?.as_str().parse().ok()
}

// ── The threads ──────────────────────────────────────────────────────────────

/// **Read the server's frames** until its output closes, handing each message to the frame; then say why it closed.
fn read_frames(output: impl Read, to_frame: &Sender<FromServer>, errors: JoinHandle<String>) {
    let mut reader = BufReader::new(output);
    let broke = loop {
        match read_frame(&mut reader) {
            Ok(Some(message)) => {
                if to_frame.send(FromServer::Message(message)).is_err() {
                    // The frame let the server go: nobody is listening.
                    return;
                }
            }
            Ok(None) => break None,
            Err(why) => break Some(why.to_string()),
        }
    };
    // The process is gone or going, so its error stream ends too: its last line is the best account of why.
    let said = errors.join().unwrap_or_default();
    let why = match (broke, said.is_empty()) {
        (Some(broke), _) => format!("its output could not be read: {broke}"),
        (None, false) => format!("it said: {said}"),
        (None, true) => "its output closed".to_owned(),
    };
    let _ = to_frame.send(FromServer::Closed(why));
}

/// **One frame**: headers up to a blank line, of which `Content-Length` is the one read, then that many bytes of JSON.
/// `None` at the end of the stream.
fn read_frame(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut length = None;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let header = line.trim_end_matches(['\r', '\n']);
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            length = value.trim().parse::<usize>().ok();
        }
    }
    let length = length.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "a frame without a Content-Length"))?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    serde_json::from_slice(&body).map(Some).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// **Write the editor's messages to the server** as frames, until the editor lets the server go or the pipe breaks.
fn write_frames(mut input: impl Write, from_frame: &Receiver<Value>) {
    for message in from_frame {
        let body = message.to_string();
        if write!(input, "Content-Length: {}\r\n\r\n{body}", body.len()).and_then(|()| input.flush()).is_err() {
            return;
        }
    }
}

/// **Keep the last line the server wrote to its error stream**, reading it to the end so the server never blocks on a full pipe.
fn last_line(errors: impl Read) -> String {
    let mut last = String::new();
    for line in BufReader::new(errors).lines() {
        let Ok(line) = line else { break };
        if !line.trim().is_empty() {
            last = line;
        }
    }
    last
}

// ── The frame ────────────────────────────────────────────────────────────────

/// **Keep the project's Rust server in step with the open files**, once a frame: start it when a Rust file of a Cargo
/// project is open, read what it said, tell it what changed, and let it go with its project.
pub fn talk_to_rust_analyzer(mut rust: ResMut<RustAnalyzer>, project: Option<Res<Project>>, files: Res<Files>, mut state: Option<ResMut<EditorState>>) {
    let root = project.as_deref().map(|p| p.root.clone());
    if rust.root != root {
        // **Another project, or none**: the server goes with the project it was started for.
        rust.forget();
        rust.settled = false;
        rust.root.clone_from(&root);
    }
    let Some(root) = root else { return };
    // The buffers belong to the project they were read from; a moment after a switch they may still be the last one's.
    let open: BTreeMap<&str, &str> = match files.root() {
        Some(held) if held == root.as_path() => {
            files.every_open().iter().filter(|(rel, file)| is_rust(rel) && file.problem.is_none()).map(|(rel, file)| (rel.as_str(), file.text.as_str())).collect()
        }
        _ => BTreeMap::new(),
    };
    if rust.server.is_none() && !rust.settled && !open.is_empty() {
        rust.settled = true;
        if !root.join(MANIFEST).is_file() {
            // A project that is not a Cargo package has nothing for the server to read: its Rust files keep the editor's colours.
            return;
        }
        match Server::spawn(&rust.program, &root) {
            Ok(server) => {
                info!("started {} for {}", rust.program.display(), root.display());
                rust.server = Some(server);
                rust.settled = false;
            }
            Err(why) => {
                let line = absent_line(&rust.program, &why);
                say(state.as_deref_mut(), line);
                return;
            }
        }
    }
    let rust = &mut *rust;
    let Some(server) = rust.server.as_mut() else { return };
    let stopped = hear(server, &mut rust.published, &mut rust.generation, state.as_deref_mut());
    if let Some(why) = stopped {
        let line = stopped_line(&rust.program, &why);
        rust.forget();
        rust.settled = true;
        say(state.as_deref_mut(), line);
        return;
    }
    if server.ready {
        tell(server, &open, &mut rust.published);
    }
}

/// **Read everything the server said since the last frame.** `Some` when it stopped, saying why.
fn hear(server: &mut Server, published: &mut BTreeMap<String, Published>, generation: &mut u64, mut state: Option<&mut EditorState>) -> Option<String> {
    loop {
        let message = match server.from.get_mut().unwrap_or_else(PoisonError::into_inner).try_recv() {
            Ok(FromServer::Message(message)) => message,
            Ok(FromServer::Closed(why)) => return Some(why),
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => return Some("its output closed".to_owned()),
        };
        let method = message.get("method").and_then(Value::as_str);
        let id = message.get("id").cloned();
        match (method, id) {
            // **An answer**: the only request made is `initialize`.
            (None, Some(id)) => {
                if id != json!(INITIALIZE_ID) {
                    continue;
                }
                if let Some(error) = message.get("error") {
                    let said = error.get("message").and_then(Value::as_str).unwrap_or("no reason given");
                    return Some(format!("it refused to start: {said}"));
                }
                let encoding = message.pointer("/result/capabilities/positionEncoding").and_then(Value::as_str);
                server.encoding = if encoding == Some(PositionEncodingKind::UTF8.as_str()) { Encoding::Utf8 } else { Encoding::Utf16 };
                server.notify::<Initialized>(InitializedParams {});
                server.ready = true;
            }
            // **A request of the server's**: the editor offers no capability that asks for one, so each is answered as not had.
            (Some(method), Some(id)) => {
                server.send(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": METHOD_NOT_FOUND, "message": format!("Dark Factory does not answer {method}") } }));
            }
            (Some(method), None) if method == PublishDiagnostics::METHOD => {
                let Some(params) = message.get("params").cloned().and_then(|p| serde_json::from_value::<PublishDiagnosticsParams>(p).ok()) else { continue };
                // Only what the server holds open is drawn; a publication for a file it was never told of is not one the editor shows.
                let Some(rel) = server.rel_of(&params.uri).filter(|rel| server.documents.contains_key(rel)) else { continue };
                *generation += 1;
                published.insert(rel, Published { generation: *generation, encoding: server.encoding, diagnostics: params.diagnostics });
            }
            (Some(method), None) if method == ShowMessage::METHOD => {
                let Some(params) = message.get("params").cloned().and_then(|p| serde_json::from_value::<ShowMessageParams>(p).ok()) else { continue };
                if params.typ == MessageType::ERROR {
                    say(state.as_deref_mut(), format!("rust-analyzer: {}", params.message));
                }
            }
            _ => {}
        }
    }
}

/// **Tell the server what the open Rust files hold now**: a file it has not seen is opened, a changed one is sent whole,
/// and a closed one is closed and its findings dropped.
fn tell(server: &mut Server, open: &BTreeMap<&str, &str>, published: &mut BTreeMap<String, Published>) {
    for (&rel, &text) in open {
        let message = match server.documents.get_mut(rel) {
            None => {
                let Some(uri) = server.uri_of(rel) else { continue };
                let item = TextDocumentItem::new(uri.clone(), "rust".to_owned(), 1, text.to_owned());
                server.documents.insert(rel.to_owned(), Document { uri, version: 1, text: text.to_owned() });
                Some(Message::Open(DidOpenTextDocumentParams { text_document: item }))
            }
            Some(document) if document.text != text => {
                document.version += 1;
                text.clone_into(&mut document.text);
                Some(Message::Change(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier::new(document.uri.clone(), document.version),
                    content_changes: vec![TextDocumentContentChangeEvent { range: None, range_length: None, text: text.to_owned() }],
                }))
            }
            Some(_) => None,
        };
        match message {
            Some(Message::Open(params)) => server.notify::<DidOpenTextDocument>(params),
            Some(Message::Change(params)) => server.notify::<DidChangeTextDocument>(params),
            None => {}
        }
    }
    let closed: Vec<String> = server.documents.keys().filter(|rel| !open.contains_key(rel.as_str())).cloned().collect();
    for rel in closed {
        if let Some(document) = server.documents.remove(&rel) {
            server.notify::<DidCloseTextDocument>(DidCloseTextDocumentParams { text_document: TextDocumentIdentifier::new(document.uri) });
        }
        published.remove(&rel);
    }
}

/// The two document messages a frame may send for one file.
enum Message {
    Open(DidOpenTextDocumentParams),
    Change(DidChangeTextDocumentParams),
}

/// **Say a line on the status**, as a problem: a person reads it there, and the log keeps it.
fn say(state: Option<&mut EditorState>, line: String) {
    warn!("{line}");
    if let Some(state) = state {
        state.status.problem(line);
    }
}

// ── Positions ────────────────────────────────────────────────────────────────

/// The byte offset each line of `text` starts at.
fn line_starts(text: &str) -> Vec<usize> {
    std::iter::once(0).chain(text.match_indices('\n').map(|(at, _)| at + 1)).collect()
}

/// **The byte offset of a protocol position in `text`**: its line, then its column counted as the server counts them,
/// held inside the line and on a character boundary.
fn offset_of(text: &str, lines: &[usize], at: Position, encoding: Encoding) -> usize {
    let Some(&start) = usize::try_from(at.line).ok().and_then(|line| lines.get(line)) else { return text.len() };
    let end = text[start..].find('\n').map_or(text.len(), |at| start + at);
    let line = &text[start..end];
    let column = usize::try_from(at.character).unwrap_or(usize::MAX);
    let within = match encoding {
        Encoding::Utf8 => {
            let mut within = column.min(line.len());
            while !line.is_char_boundary(within) {
                within -= 1;
            }
            within
        }
        Encoding::Utf16 => {
            let mut units = 0;
            line.char_indices()
                .find(|&(_, c)| {
                    let reached = units >= column;
                    units += c.len_utf16();
                    reached
                })
                .map_or(line.len(), |(at, _)| at)
        }
    };
    start + within
}

/// How the editor draws a severity the protocol names; a finding without one is drawn as a warning.
fn severity_of(severity: Option<DiagnosticSeverity>) -> CodeSeverity {
    match severity {
        Some(DiagnosticSeverity::ERROR) => CodeSeverity::Error,
        Some(DiagnosticSeverity::INFORMATION) => CodeSeverity::Info,
        Some(DiagnosticSeverity::HINT) => CodeSeverity::Hint,
        _ => CodeSeverity::Warning,
    }
}

/// **What the note on a finding says**: the server's message, then who found it and the error's code.
fn message_of(d: &lsp_types::Diagnostic) -> String {
    let source = d.source.as_deref().unwrap_or(PROGRAM);
    match &d.code {
        Some(NumberOrString::String(code)) => format!("{} ({source} {code})", d.message),
        Some(NumberOrString::Number(code)) => format!("{} ({source} {code})", d.message),
        None => format!("{} ({source})", d.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_is_read_by_its_content_length() {
        let body = r#"{"jsonrpc":"2.0","method":"x"}"#;
        let wire = format!("Content-Length: {}\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{body}", body.len());
        let mut reader = BufReader::new(wire.as_bytes());
        let read = read_frame(&mut reader).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(read, Some(json!({ "jsonrpc": "2.0", "method": "x" })));
        assert_eq!(read_frame(&mut reader).unwrap_or_else(|e| panic!("{e}")), None, "the end of the stream is no frame");
    }

    #[test]
    fn a_position_is_a_byte_offset_in_either_count() {
        // `é` is two bytes and one UTF-16 unit; `𝄞` is four bytes and two units.
        let text = "fn a() {}\nlet é𝄞 = x;\n";
        let lines = line_starts(text);
        let x = text.find('x').unwrap_or_else(|| panic!("the text has an x"));
        let utf8 = Position::new(1, u32::try_from(x - lines[1]).unwrap_or_else(|e| panic!("{e}")));
        assert_eq!(offset_of(text, &lines, utf8, Encoding::Utf8), x);
        // `let ` 4 + `é` 1 + `𝄞` 2 + ` = ` 3 units.
        assert_eq!(offset_of(text, &lines, Position::new(1, 10), Encoding::Utf16), x);
        assert_eq!(offset_of(text, &lines, Position::new(1, 500), Encoding::Utf8), text.len() - 1, "a column past the line is held at its end");
        assert_eq!(offset_of(text, &lines, Position::new(9, 0), Encoding::Utf8), text.len(), "a line past the text is its end");
        assert_eq!(offset_of(text, &lines, Position::new(1, 5), Encoding::Utf8), lines[1] + 4, "a column inside a character is held at its start");
    }
}
