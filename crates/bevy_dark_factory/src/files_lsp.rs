//! **rust-analyzer behind the Files editor** (#611, #698): the compiler's own errors on a Rust file, drawn over the
//! colours the editor's tokenizer already gives it, and its completions and hovers where the person asks for them.
//!
//! # Which server, and when it runs
//!
//! One server, `rust-analyzer` ([`PROGRAM`]), for `.rs` files only (owner, 2026-11-27). It is started **once per
//! project**: when the open project's root holds a `Cargo.toml` and a `.rs` file is open, from the project's root. It
//! lives until the project closes or another one opens: [`RustAnalyzer`] owns the process, and dropping the process
//! kills and reaps it, so a closed project or a closed editor leaves no server behind. The editor speaks the Language
//! Server Protocol to it over its standard input and output (JSON-RPC in `Content-Length` frames): `initialize`,
//! `initialized`, then `textDocument/didOpen`, `didChange`, `didSave` and `didClose` as Rust files open, change, are
//! written and close. A change is sent as the one edit that makes it when the server takes edits (rust-analyzer does),
//! and as the whole text when it does not; a save is sent when the server asked for saves, and is what reruns
//! rust-analyzer's `cargo check`.
//!
//! # What it adds, and what stays
//!
//! The tokenizer ([`crate::files_lang`]) colours, folds and outlines every file, a Rust file included, whether a server
//! runs or not: that is the baseline, not a fallback. The server adds three things:
//!
//! - **Findings**: what it publishes (`textDocument/publishDiagnostics`) joins the editor's own findings, through the
//!   one [`bevy_gooey::Diagnostics`] the loader's readers fill (`files::decorate_the_editor`).
//! - **Completion**: the editor's [`CodeCompletionRequested`] on a file the server holds becomes
//!   `textDocument/completion` ([`RustAnalyzer::ask_completion`]), and the answer is the editor's [`CodeCompletions`]
//!   at the word, in the server's order.
//! - **Hover**: [`CodeHoverRequested`] becomes `textDocument/hover` ([`RustAnalyzer::ask_hover`]), asked as plain text,
//!   and the answer is the editor's [`CodeHover`] on the bytes the server names.
//!
//! The tokenizer offers nothing to complete or say in Rust, so a server's answer is the only one, and a Rust file
//! without a server opens no list and no note. An answer comes on a later frame ([`talk_to_rust_analyzer`] hears it), and
//! is put on the editor only while it still shows the text it was asked on: an answer about text that has moved on is
//! dropped, and a newer question cancels the older one of its kind (`$/cancelRequest`).
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
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Mutex, PoisonError};
use std::thread::JoinHandle;

use bevy::prelude::*;
use bevy_gooey::{CodeCompletionRequested, CodeCompletions, CodeEditorValue, CodeHover, CodeHoverRequested, CodeSeverity, CompletionItem, CompletionTrigger, Diagnostic};
use lsp_types::notification::{
    Cancel, DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, DidSaveTextDocument, Initialized, Notification, PublishDiagnostics, ShowMessage,
};
use lsp_types::request::{Completion, HoverRequest, Initialize, Request};
use lsp_types::{
    CancelParams, ClientCapabilities, ClientInfo, CompletionClientCapabilities, CompletionContext, CompletionParams, CompletionResponse, CompletionTextEdit,
    CompletionTriggerKind, DiagnosticSeverity, DidChangeTextDocumentParams, DidCloseTextDocumentParams, DidOpenTextDocumentParams, DidSaveTextDocumentParams,
    GeneralClientCapabilities, Hover, HoverClientCapabilities, HoverContents, HoverParams, InitializeParams, InitializedParams, InsertTextFormat, MarkedString,
    MarkupKind, MessageType, NumberOrString, PartialResultParams, Position, PositionEncodingKind, PublishDiagnosticsClientCapabilities, PublishDiagnosticsParams,
    ShowMessageParams, TextDocumentClientCapabilities, TextDocumentContentChangeEvent, TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams,
    TextDocumentSyncCapability, TextDocumentSyncClientCapabilities, TextDocumentSyncKind, TextDocumentSyncSaveOptions, Uri, VersionedTextDocumentIdentifier,
    WorkDoneProgressParams, WorkspaceFolder,
};
use serde_json::{Value, json};

use crate::editor::EditorState;
use crate::files::{FileEditor, Files};
use crate::project::Project;

/// **The program started for Rust files**, looked for on `PATH`.
pub const PROGRAM: &str = "rust-analyzer";

/// **What a project's root holds for its server to be started**: a Cargo package or workspace.
pub const MANIFEST: &str = "Cargo.toml";

/// The files the server is told about, by extension.
pub const EXTENSION: &str = ".rs";

/// The JSON-RPC id of `initialize`, the first request; every question after it counts on from here.
const INITIALIZE_ID: i32 = 1;

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
    /// The program started: [`PROGRAM`], unless a test names another (a server that is not installed, or a fixed one).
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

    /// **Ask the server what completes the word at the caret** of `rel`, which the editor shows as `text`. `false` when no
    /// server can be asked (none runs, it is still starting, or it does not hold the file), so the editor answers as it
    /// would without one. The answer is put on the editor as its [`CodeCompletions`] on a later frame, while it still
    /// shows the text asked on.
    pub fn ask_completion(&mut self, request: &CodeCompletionRequested, rel: &str, text: &str) -> bool {
        let context = match request.trigger {
            CompletionTrigger::Character => CompletionContext {
                trigger_kind: CompletionTriggerKind::TRIGGER_CHARACTER,
                trigger_character: text.get(..request.offset).and_then(|head| head.chars().next_back()).map(String::from),
            },
            CompletionTrigger::Manual | CompletionTrigger::Typing => CompletionContext { trigger_kind: CompletionTriggerKind::INVOKED, trigger_character: None },
        };
        // The word the list narrows by starts where the editor's prefix does: that is where a choice is written from.
        let anchor = request.offset.saturating_sub(request.prefix.len());
        let Some((server, at)) = self.question(rel, text, request.offset) else { return false };
        let params = CompletionParams {
            text_document_position: at,
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
            context: Some(context),
        };
        server.ask::<Completion>(params, Ask { editor: request.entity, rel: rel.to_owned(), revision: request.revision, kind: AskKind::Completion { anchor } })
    }

    /// **Ask the server what to say about the text at `request.offset`** of `rel`, which the editor shows as `text`.
    /// `false` when no server can be asked. The answer is put on the editor as its [`CodeHover`] on a later frame, while it
    /// still shows the text asked on.
    pub fn ask_hover(&mut self, request: &CodeHoverRequested, rel: &str, text: &str) -> bool {
        let Some((server, at)) = self.question(rel, text, request.offset) else { return false };
        let params = HoverParams { text_document_position_params: at, work_done_progress_params: WorkDoneProgressParams::default() };
        server.ask::<HoverRequest>(params, Ask { editor: request.entity, rel: rel.to_owned(), revision: request.revision, kind: AskKind::Hover { offset: request.offset } })
    }

    /// **The server a question about `offset` in `rel` goes to**, with the position it names: a running server that has
    /// answered `initialize` and holds the file, told `text` first when it holds another, so the question is about the
    /// text the person sees.
    fn question(&mut self, rel: &str, text: &str, offset: usize) -> Option<(&mut Server, TextDocumentPositionParams)> {
        let server = self.server.as_mut().filter(|server| server.ready)?;
        let uri = server.documents.get(rel)?.uri.clone();
        if !text.is_char_boundary(offset) {
            return None;
        }
        server.change(rel, text);
        let position = position_of(text, offset, server.encoding);
        Some((server, TextDocumentPositionParams::new(TextDocumentIdentifier::new(uri), position)))
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

/// **A question asked for one editor**, kept under its request id until the answer comes.
struct Ask {
    /// The editor that asked: the answer is put on it.
    editor: Entity,
    /// The file it showed, and the revision of its text: an answer is put on the editor only while both still hold.
    rel: String,
    revision: u64,
    kind: AskKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AskKind {
    /// `textDocument/completion`: the list narrows from `anchor`, the byte offset the word at the caret starts at.
    Completion { anchor: usize },
    /// `textDocument/hover` at `offset`, a byte offset.
    Hover { offset: usize },
}

impl AskKind {
    /// Whether a newer question of this kind makes an older one moot.
    fn same_kind(self, other: AskKind) -> bool {
        matches!((self, other), (AskKind::Completion { .. }, AskKind::Completion { .. }) | (AskKind::Hover { .. }, AskKind::Hover { .. }))
    }
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
    /// The server takes a change as the edit that makes it (`textDocumentSync.change` is incremental); otherwise it is
    /// sent the whole text.
    incremental: bool,
    /// The server asked to be told of saves (`textDocumentSync.save`).
    wants_saves: bool,
    /// The project root as the server was told it, canonical, so the paths it names come back the same.
    root: PathBuf,
    /// The documents the server holds, by path under the project.
    documents: BTreeMap<String, Document>,
    /// The id the next question is sent under.
    next_id: i32,
    /// The questions asked and not yet answered, by request id.
    asks: BTreeMap<i32, Ask>,
    _process: Process,
}

/// One document as the server was last told it.
struct Document {
    uri: Uri,
    version: i32,
    text: String,
    /// [`crate::files::OpenFile::saves`] when the server was last told of a save, or when it was opened.
    saves: u64,
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
        let server = Server {
            to,
            from: Mutex::new(from),
            ready: false,
            encoding: Encoding::Utf16,
            incremental: false,
            wants_saves: false,
            root,
            documents: BTreeMap::new(),
            next_id: INITIALIZE_ID + 1,
            asks: BTreeMap::new(),
            _process: process,
        };
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

    /// **Send a question**, keeping what it was asked for under its id until the answer comes. An older question of the
    /// same kind from the same editor is cancelled: its answer would be about a caret or a pointer that has moved.
    fn ask<R: Request>(&mut self, params: R::Params, ask: Ask) -> bool {
        let params = match serde_json::to_value(params) {
            Ok(params) => params,
            Err(why) => {
                warn!("the {} request for rust-analyzer did not serialise: {why}", R::METHOD);
                return false;
            }
        };
        let moot: Vec<i32> = self.asks.iter().filter(|(_, older)| older.editor == ask.editor && older.kind.same_kind(ask.kind)).map(|(&id, _)| id).collect();
        for id in moot {
            self.asks.remove(&id);
            self.notify::<Cancel>(CancelParams { id: NumberOrString::Number(id) });
        }
        let id = self.next_id;
        self.next_id = self.next_id.checked_add(1).unwrap_or(INITIALIZE_ID + 1);
        self.asks.insert(id, ask);
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": R::METHOD, "params": params }));
        true
    }

    /// **Tell the server that `rel` holds `text` now**, when it holds something else: as the one edit that makes the change
    /// when the server takes edits, else as the whole text. A document the server does not hold is left for [`tell`] to open.
    fn change(&mut self, rel: &str, text: &str) {
        let (incremental, encoding) = (self.incremental, self.encoding);
        let Some(document) = self.documents.get_mut(rel) else { return };
        if document.text == text {
            return;
        }
        let change = if incremental {
            let (replaced, inserted) = edit_between(&document.text, text);
            let range = lsp_types::Range::new(position_of(&document.text, replaced.start, encoding), position_of(&document.text, replaced.end, encoding));
            TextDocumentContentChangeEvent { range: Some(range), range_length: None, text: inserted.to_owned() }
        } else {
            TextDocumentContentChangeEvent { range: None, range_length: None, text: text.to_owned() }
        };
        document.version += 1;
        text.clone_into(&mut document.text);
        let params = DidChangeTextDocumentParams { text_document: VersionedTextDocumentIdentifier::new(document.uri.clone(), document.version), content_changes: vec![change] };
        self.notify::<DidChangeTextDocument>(params);
    }

    /// **Tell the server that `rel` was written to disk**, when it was written since the server was last told and the server
    /// asked to hear it: rust-analyzer checks the project again on a save.
    fn saved(&mut self, rel: &str, saves: u64) {
        let Some(document) = self.documents.get_mut(rel) else { return };
        if document.saves == saves {
            return;
        }
        document.saves = saves;
        let uri = document.uri.clone();
        if self.wants_saves {
            self.notify::<DidSaveTextDocument>(DidSaveTextDocumentParams { text_document: TextDocumentIdentifier::new(uri), text: None });
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

/// **The `initialize` request's parameters**: the project as the one workspace folder, UTF-8 columns asked for first,
/// versioned diagnostics, saves told, completion, and hovers as plain text (the editor's note draws no Markdown).
fn initialize_params(root: &Path) -> Option<Value> {
    let uri = uri_of(root)?;
    let name = root.file_name().map_or_else(|| root.display().to_string(), |n| n.to_string_lossy().into_owned());
    let params = InitializeParams {
        process_id: Some(std::process::id()),
        workspace_folders: Some(vec![WorkspaceFolder { uri, name }]),
        capabilities: ClientCapabilities {
            general: Some(GeneralClientCapabilities { position_encodings: Some(vec![PositionEncodingKind::UTF8, PositionEncodingKind::UTF16]), ..Default::default() }),
            text_document: Some(TextDocumentClientCapabilities {
                synchronization: Some(TextDocumentSyncClientCapabilities { did_save: Some(true), ..Default::default() }),
                completion: Some(CompletionClientCapabilities::default()),
                hover: Some(HoverClientCapabilities { content_format: Some(vec![MarkupKind::PlainText]), ..Default::default() }),
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
/// project is open, read what it said and put its answers on the editors that asked, tell it what changed and what was
/// saved, and let it go with its project.
pub fn talk_to_rust_analyzer(
    mut rust: ResMut<RustAnalyzer>,
    project: Option<Res<Project>>,
    files: Res<Files>,
    editors: Query<(&FileEditor, &CodeEditorValue)>,
    mut state: Option<ResMut<EditorState>>,
    mut commands: Commands,
) {
    let root = project.as_deref().map(|p| p.root.clone());
    if rust.root != root {
        // **Another project, or none**: the server goes with the project it was started for.
        rust.forget();
        rust.settled = false;
        rust.root.clone_from(&root);
    }
    let Some(root) = root else { return };
    // The buffers belong to the project they were read from; a moment after a switch they may still be the last one's.
    let open: BTreeMap<&str, (&str, u64)> = match files.root() {
        Some(held) if held == root.as_path() => files
            .every_open()
            .iter()
            .filter(|(rel, file)| is_rust(rel) && file.problem.is_none())
            .map(|(rel, file)| (rel.as_str(), (file.text.as_str(), file.saves)))
            .collect(),
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
    let mut answers = Vec::new();
    let stopped = hear(server, &mut rust.published, &mut rust.generation, &mut answers, state.as_deref_mut());
    if let Some(why) = stopped {
        let line = stopped_line(&rust.program, &why);
        rust.forget();
        rust.settled = true;
        say(state.as_deref_mut(), line);
        return;
    }
    for (ask, result) in answers {
        answer(ask, result, server.encoding, &editors, &mut commands);
    }
    if server.ready {
        tell(server, &open, &mut rust.published);
    }
}

/// **Read everything the server said since the last frame**, keeping the answers to the editors' questions in `answers`.
/// `Some` when it stopped, saying why.
fn hear(
    server: &mut Server,
    published: &mut BTreeMap<String, Published>,
    generation: &mut u64,
    answers: &mut Vec<(Ask, Value)>,
    mut state: Option<&mut EditorState>,
) -> Option<String> {
    loop {
        let mut message = match server.from.get_mut().unwrap_or_else(PoisonError::into_inner).try_recv() {
            Ok(FromServer::Message(message)) => message,
            Ok(FromServer::Closed(why)) => return Some(why),
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => return Some("its output closed".to_owned()),
        };
        let method = message.get("method").and_then(Value::as_str).map(str::to_owned);
        let id = message.get("id").cloned();
        match (method, id) {
            // **The answer to `initialize`**: how the server counts columns, takes changes and wants saves.
            (None, Some(id)) if id == json!(INITIALIZE_ID) => {
                if let Some(error) = message.get("error") {
                    let said = error.get("message").and_then(Value::as_str).unwrap_or("no reason given");
                    return Some(format!("it refused to start: {said}"));
                }
                let encoding = message.pointer("/result/capabilities/positionEncoding").and_then(Value::as_str);
                server.encoding = if encoding == Some(PositionEncodingKind::UTF8.as_str()) { Encoding::Utf8 } else { Encoding::Utf16 };
                let sync = message.pointer("/result/capabilities/textDocumentSync").cloned().and_then(|s| serde_json::from_value::<TextDocumentSyncCapability>(s).ok());
                let (change, save) = match sync {
                    Some(TextDocumentSyncCapability::Kind(kind)) => (Some(kind), None),
                    Some(TextDocumentSyncCapability::Options(options)) => (options.change, options.save),
                    None => (None, None),
                };
                server.incremental = change == Some(TextDocumentSyncKind::INCREMENTAL);
                server.wants_saves = matches!(save, Some(TextDocumentSyncSaveOptions::Supported(true) | TextDocumentSyncSaveOptions::SaveOptions(_)));
                server.notify::<Initialized>(InitializedParams {});
                server.ready = true;
            }
            // **The answer to a question**: kept for the editor that asked. A refusal (the text moved on under it, or it
            // was cancelled) and the answer to a cancelled question have nothing to show.
            (None, Some(id)) => {
                let Some(ask) = id.as_i64().and_then(|id| i32::try_from(id).ok()).and_then(|id| server.asks.remove(&id)) else { continue };
                if message.get("error").is_some() {
                    continue;
                }
                let result = message.get_mut("result").map(Value::take).unwrap_or(Value::Null);
                answers.push((ask, result));
            }
            // **A request of the server's**: the editor offers no capability that asks for one, so each is answered as not had.
            (Some(method), Some(id)) => {
                server.send(json!({ "jsonrpc": "2.0", "id": id, "error": { "code": METHOD_NOT_FOUND, "message": format!("Dark Factory does not answer {method}") } }));
            }
            (Some(method), None) if method == PublishDiagnostics::METHOD => {
                let Some(params) = message.get_mut("params").map(Value::take).and_then(|p| serde_json::from_value::<PublishDiagnosticsParams>(p).ok()) else { continue };
                // Only what the server holds open is drawn; a publication for a file it was never told of is not one the editor shows.
                let Some(rel) = server.rel_of(&params.uri).filter(|rel| server.documents.contains_key(rel)) else { continue };
                *generation += 1;
                published.insert(rel, Published { generation: *generation, encoding: server.encoding, diagnostics: params.diagnostics });
            }
            (Some(method), None) if method == ShowMessage::METHOD => {
                let Some(params) = message.get_mut("params").map(Value::take).and_then(|p| serde_json::from_value::<ShowMessageParams>(p).ok()) else { continue };
                if params.typ == MessageType::ERROR {
                    say(state.as_deref_mut(), format!("rust-analyzer: {}", params.message));
                }
            }
            _ => {}
        }
    }
}

/// **Put the server's answer on the editor that asked**, while it still shows the file and the text it asked about: the
/// completions at the word, or the note on the hovered text. An answer about text that has moved on is dropped.
fn answer(ask: Ask, result: Value, encoding: Encoding, editors: &Query<(&FileEditor, &CodeEditorValue)>, commands: &mut Commands) {
    let Ok((editor, value)) = editors.get(ask.editor) else { return };
    if editor.shown.as_deref() != Some(ask.rel.as_str()) || value.revision() != ask.revision {
        return;
    }
    match ask.kind {
        AskKind::Completion { anchor } => {
            let items = completions_of(result, value.text(), anchor, encoding);
            commands.entity(ask.editor).try_insert(CodeCompletions { anchor, items });
        }
        AskKind::Hover { offset } => {
            if let Some((range, text)) = hover_of(result, value.text(), offset, encoding) {
                commands.entity(ask.editor).try_insert(CodeHover { revision: ask.revision, range, text });
            }
        }
    }
}

/// **The items of a completion answer the editor's list can offer from `anchor`**, in the server's order (its
/// `sortText`): each shows its label and detail, and writes its edit's text, or its own, over the word from `anchor`.
/// An item the list cannot apply as the server means it is left out: one whose edit starts elsewhere (a rewrite of the
/// expression before the word), one that brings edits elsewhere (an import), and a snippet.
fn completions_of(result: Value, text: &str, anchor: usize, encoding: Encoding) -> Vec<CompletionItem> {
    let items = match serde_json::from_value::<Option<CompletionResponse>>(result) {
        Ok(Some(CompletionResponse::Array(items))) => items,
        Ok(Some(CompletionResponse::List(list))) => list.items,
        Ok(None) | Err(_) => return Vec::new(),
    };
    let lines = line_starts(text);
    let starts_at_anchor = |at: Position| offset_of(text, &lines, at, encoding) == anchor;
    let mut offered: Vec<(String, CompletionItem)> = items
        .into_iter()
        .filter(|item| item.additional_text_edits.as_ref().is_none_or(Vec::is_empty) && item.insert_text_format != Some(InsertTextFormat::SNIPPET))
        .filter_map(|item| {
            let written = match item.text_edit {
                Some(CompletionTextEdit::Edit(edit)) => starts_at_anchor(edit.range.start).then_some(edit.new_text)?,
                Some(CompletionTextEdit::InsertAndReplace(edit)) => starts_at_anchor(edit.insert.start).then_some(edit.new_text)?,
                None => item.insert_text.unwrap_or_else(|| item.label.clone()),
            };
            let order = item.sort_text.unwrap_or_else(|| item.label.clone());
            let shown = CompletionItem::new(item.label).detail(item.detail.unwrap_or_default());
            let shown = if written == shown.label { shown } else { shown.insert(written) };
            Some((order, shown))
        })
        .collect();
    offered.sort_by(|(a, _), (b, _)| a.cmp(b));
    offered.into_iter().map(|(_, item)| item).collect()
}

/// **What a hover answer says, and the bytes it is about**: its text, about the range it names, or about the word at
/// `offset` when it names none. A server that sends Markdown though plain text was asked for has its code fences dropped.
/// `None` when it says nothing.
fn hover_of(result: Value, text: &str, offset: usize, encoding: Encoding) -> Option<(Range<usize>, String)> {
    let hover = serde_json::from_value::<Option<Hover>>(result).ok()??;
    let said: Vec<String> = match hover.contents {
        HoverContents::Scalar(said) => vec![marked(said)],
        HoverContents::Array(said) => said.into_iter().map(marked).collect(),
        HoverContents::Markup(said) if said.kind == MarkupKind::Markdown => vec![without_fences(&said.value)],
        HoverContents::Markup(said) => vec![said.value],
    };
    let said = said.join("\n\n").trim().to_owned();
    if said.is_empty() {
        return None;
    }
    let range = match hover.range {
        Some(range) => {
            let lines = line_starts(text);
            let start = offset_of(text, &lines, range.start, encoding);
            start..offset_of(text, &lines, range.end, encoding).max(start)
        }
        None => word_at(text, offset),
    };
    Some((range, said))
}

/// A marked string's text: Markdown without its fences, or a code block's code.
fn marked(said: MarkedString) -> String {
    match said {
        MarkedString::String(markdown) => without_fences(&markdown),
        MarkedString::LanguageString(code) => code.value,
    }
}

/// Markdown with its code fences' lines dropped: the note draws text, and a fence is only Markdown's.
fn without_fences(markdown: &str) -> String {
    markdown.lines().filter(|line| !line.trim_start().starts_with("```")).collect::<Vec<_>>().join("\n")
}

/// **The word at `offset`**: the letters, digits and underscores around it; empty at `offset` when there are none.
fn word_at(text: &str, offset: usize) -> Range<usize> {
    let (Some(head), Some(tail)) = (text.get(..offset), text.get(offset..)) else { return offset..offset };
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let start = head.char_indices().rev().take_while(|&(_, c)| is_word(c)).last().map_or(offset, |(at, _)| at);
    let end = tail.char_indices().find(|&(_, c)| !is_word(c)).map_or(text.len(), |(at, _)| offset + at);
    start..end
}

/// **Tell the server what the open Rust files hold now**: a file it has not seen is opened, a changed one is changed, a
/// saved one is said saved, and a closed one is closed and its findings dropped.
fn tell(server: &mut Server, open: &BTreeMap<&str, (&str, u64)>, published: &mut BTreeMap<String, Published>) {
    for (&rel, &(text, saves)) in open {
        if server.documents.contains_key(rel) {
            server.change(rel, text);
            server.saved(rel, saves);
        } else {
            let Some(uri) = server.uri_of(rel) else { continue };
            let item = TextDocumentItem::new(uri.clone(), "rust".to_owned(), 1, text.to_owned());
            server.documents.insert(rel.to_owned(), Document { uri, version: 1, text: text.to_owned(), saves });
            server.notify::<DidOpenTextDocument>(DidOpenTextDocumentParams { text_document: item });
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

/// **The protocol position of a byte offset in `text`**, the inverse of [`offset_of`]: its line, and its column counted
/// as the server counts them. An offset past the text is its end.
fn position_of(text: &str, offset: usize, encoding: Encoding) -> Position {
    let head = text.get(..offset).unwrap_or(text);
    let start = head.rfind('\n').map_or(0, |at| at + 1);
    let column = match encoding {
        Encoding::Utf8 => head.len() - start,
        Encoding::Utf16 => head[start..].encode_utf16().count(),
    };
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    Position::new(count(head.matches('\n').count()), count(column))
}

/// **The one edit that turns `old` into `new`**: the bytes both start and end with are kept, and the rest of `old` (the
/// range returned) is replaced by the rest of `new` (the text returned). Both ends fall on character boundaries.
fn edit_between<'a>(old: &str, new: &'a str) -> (Range<usize>, &'a str) {
    let mut head = old.bytes().zip(new.bytes()).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(head) || !new.is_char_boundary(head) {
        head -= 1;
    }
    let room = old.len().min(new.len()) - head;
    let mut tail = old.bytes().rev().zip(new.bytes().rev()).take(room).take_while(|(a, b)| a == b).count();
    while !old.is_char_boundary(old.len() - tail) || !new.is_char_boundary(new.len() - tail) {
        tail -= 1;
    }
    (head..old.len() - tail, &new[head..new.len() - tail])
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

        // **And back**: an offset is the position that reads as it, in either count.
        for encoding in [Encoding::Utf8, Encoding::Utf16] {
            for at in (0..=text.len()).filter(|&at| text.is_char_boundary(at)) {
                assert_eq!(offset_of(text, &lines, position_of(text, at, encoding), encoding), at, "{at} in {encoding:?}");
            }
        }
        assert_eq!(position_of(text, x, Encoding::Utf16), Position::new(1, 10));
    }

    #[test]
    fn a_change_is_the_one_edit_between_the_texts() {
        // **Applied to the old text, the edit makes the new one**, and it covers only what changed.
        let cases = [
            // `b` typed after `fn a`, and taken back: the edit is at offset 4, where the texts part.
            ("fn a() {}\n", "fn ab() {}\n", 4..4, "b"),
            ("fn ab() {}\n", "fn a() {}\n", 4..5, ""),
            ("let x = 1;", "let x = 1;", 10..10, ""),
            ("", "pub fn x() {}", 0..0, "pub fn x() {}"),
            // `é` and `è` share their first byte: the edit takes the whole character, not half of it.
            ("let é = 1;", "let è = 1;", 4..6, "è"),
            // A repeated letter: the shared head and tail do not overlap.
            ("aaa", "aa", 2..3, ""),
        ];
        for (old, new, replaced, inserted) in cases {
            let (range, text) = edit_between(old, new);
            assert_eq!((range.clone(), text), (replaced, inserted), "{old:?} -> {new:?}");
            assert_eq!(format!("{}{text}{}", &old[..range.start], &old[range.end..]), new, "{old:?} -> {new:?}");
        }
    }

    #[test]
    fn a_completion_answer_keeps_what_the_list_can_write_in_the_servers_order() {
        let text = "fn f() {\n    x.an\n}\n";
        let anchor = text.find("an").unwrap_or_else(|| panic!("the text has the word"));
        let answer = json!({ "isIncomplete": false, "items": [
            { "label": "answer", "detail": "fn() -> u32", "sortText": "2",
              "textEdit": { "range": { "start": { "line": 1, "character": 6 }, "end": { "line": 1, "character": 8 } }, "newText": "answer()" } },
            { "label": "another", "sortText": "1" },
            { "label": "imported", "sortText": "0", "additionalTextEdits": [{ "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } }, "newText": "use a::imported;\n" }] },
            { "label": "if", "sortText": "3", "textEdit": { "range": { "start": { "line": 1, "character": 4 }, "end": { "line": 1, "character": 8 } }, "newText": "if x {}" } },
            { "label": "snippet", "sortText": "4", "insertTextFormat": 2, "insertText": "snippet($0)" },
        ] });
        let offered = completions_of(answer, text, anchor, Encoding::Utf8);
        let read: Vec<(&str, &str, &str)> = offered.iter().map(|i| (i.label.as_str(), i.detail.as_str(), i.text())).collect();
        assert_eq!(read, [("another", "", "another"), ("answer", "fn() -> u32", "answer()")]);
        assert!(completions_of(Value::Null, text, anchor, Encoding::Utf8).is_empty(), "no answer offers nothing");
    }

    #[test]
    fn a_hover_answer_is_its_text_about_its_range() {
        let text = "pub fn answer() -> u32 {\n    42\n}\n";
        let markdown = json!({ "contents": { "kind": "markdown", "value": "```rust\npub fn answer() -> u32\n```\nThe answer." },
                               "range": { "start": { "line": 0, "character": 7 }, "end": { "line": 0, "character": 13 } } });
        let (range, said) = hover_of(markdown, text, 9, Encoding::Utf8).unwrap_or_else(|| panic!("the hover says something"));
        assert_eq!((&text[range], said.as_str()), ("answer", "pub fn answer() -> u32\nThe answer."));
        let plain = json!({ "contents": { "kind": "plaintext", "value": "u32" } });
        let (range, said) = hover_of(plain, text, 21, Encoding::Utf8).unwrap_or_else(|| panic!("the hover says something"));
        assert_eq!((&text[range], said.as_str()), ("u32", "u32"), "without a range it is about the word at the offset");
        assert_eq!(hover_of(Value::Null, text, 9, Encoding::Utf8), None);
        assert_eq!(hover_of(json!({ "contents": "" }), text, 9, Encoding::Utf8), None, "an empty hover says nothing");
    }
}
