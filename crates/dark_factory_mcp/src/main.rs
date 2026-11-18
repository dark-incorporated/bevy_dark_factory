//! **The MCP server** — the bevy_blueprint editor, over the Model Context Protocol.
//!
//! # What it is for
//!
//! The editor's verbs used to live in **key handlers**. An agent has no keyboard, so the only way
//! to reach the editor was to synthesise keystrokes — which is a simulation of a person, and the
//! brief's rule is explicit: *"Never drive the UI by clicking."*
//!
//! This server is the other door. **It exposes the editor's operations as MCP tools**, and each
//! tool dispatches through the editor's own registry — the same function a key handler calls — so
//! an agent and a person are doing **one** thing, not two.
//!
//! # The tools
//!
//! | tool | what it does |
//! |---|---|
//! | `blueprint_guide` | how to work in DarkFactory: which tools in what order, and what the person expects |
//! | `blueprint_list_operations` | every operation, its parameters' schema, and how it is undone; `query` searches them |
//! | `blueprint_preview_operation` | what an operation would change; nothing changes |
//! | `blueprint_apply_operation` | run one, and record it in the operation log |
//! | `blueprint_undo_operation` | take one change back by its id, as a new log line |
//! | `blueprint_read_oplog` | the log, newest last |
//! | `blueprint_frame_view` | frame the Map camera on the map, a selection, placements or a rectangle |
//! | `blueprint_state` | what the editor is holding now, as JSON |
//! | `blueprint_volume_report` | what a map's carve list made: an `Add` a later `Carve` cut, the voids, and whether a point is air |
//! | `blueprint_capture_still` | one picture of the editor, plus the state |
//! | `blueprint_capture_sequence` | frames at 4 fps by default (2 to 15 a second), with an optional operation after frame 0 |
//! | `blueprint_snip` | one rectangle of the picture, cut out and written as a crop with its metadata |
//! | `blueprint_decide` | ask a named check or typed questions about pictures and facts; a probability for every answer |
//! | `blueprint_present` | put a result in front of the person and take their Keep or Undo all |
//! | `blueprint_wish` | after the hand-back, name the one tool that would most have helped |
//! | `blueprint_lesson` | after the hand-back, propose one line for the project's guide |
//! | `blueprint_schema` | one kind of project file's JSON Schema, generated from the type the reader reads |
//! | `blueprint_list_pieces` | the library's pieces, a page at a time, with their label provenance |
//! | `blueprint_list_snips` | the snips the person took, newest first, with the newest picture |
//! | `blueprint_feedback_list` | the reports the person filed about DarkFactory itself, newest first |
//! | `blueprint_project_feedback_list` | the reports the person filed about the open project, newest first |
//! | `blueprint_feedback_file_issue` | one report as the GitHub issue the loop reads: title, seven-section body, picture URLs |
//! | `blueprint_style` | the style of one place, game to area, behind the person's trust, with the pictures it cites |
//! | `blueprint_style_propose` | propose a change to the game's or a region's style page, which the person keeps or discards |
//! | `blueprint_list_rules` | the block rules a layout is generated from, as the project states them |
//! | `blueprint_run_command` | run a registered editor command by its id, as its shortcut would |
//! | `blueprint_jobs_stop` | ask a background job, such as the palette's thumbnail bake, to stop |
//! | `blueprint_read_log` | the editor's own log file, read from disk, so it answers when the editor does not |
//! | `blueprint_open_project` | open a project folder in the running editor, from any screen including the Projects home |
//! | `blueprint_self_test` | the power-on self test an agent runs when DarkFactory's terminal starts it |
//!
//! **Thirty tools, and the set is small on purpose.** An agent's context is spent on the tools it
//! is offered, and a catalogue of ninety-six verbs is a catalogue of ninety-six things to choose
//! badly from. **The operation tools are a discovery surface**: the first names what exists, and
//! the others preview, run and take back what it names. Later iterations add one tool per
//! operation as the registry grows, and the discovery tool goes away when it is not needed.
//!
//! # The transport
//!
//! **Stdio, and only the MCP specification.** The server speaks JSON-RPC to its client over stdin
//! and stdout, and it reaches the editor over BRP — which is the editor's own transport, and the
//! editor is a separate process that owns the world.
//!
//! **With `--project`, every path is confined to the project directory.** The server refuses a
//! change made while the editor has another project open, and the editor writes the project's files.
//! **Without it the server follows the editor**: whatever it has open, including none (the Projects
//! home). Captures then go to the per-user data folder (`blueprint_model::user_dirs`), and the tools
//! that read project files from disk (`blueprint_decide`, `blueprint_list_rules`, `blueprint_list_snips`)
//! refuse and ask for `--project`.

use std::path::PathBuf;

use rmcp::handler::server::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ServerCapabilities, ServerInfo};
use rmcp::service::ServiceExt;
use rmcp::{ErrorData, tool, tool_handler, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use dark_factory_mcp::{brp, docs, sequence};

use brp::Brp;

/// **The parameters of `blueprint_list_operations`** (#141).
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListParams {
    /// **Words to search for**, such as `carve` or `note`. With them, only the operations whose
    /// name or parameters match, best first, and `total`, the number of operations there are.
    /// Without them, every operation.
    #[serde(default)]
    pub query: Option<String>,
    /// **One operation's exact name**, such as `data_set` (#444). With it, only that operation: each parameter with its
    /// description, and one example call, a few hundred bytes where the whole list is tens of kilobytes. An unknown name is
    /// refused naming the closest operations. Ask with `query` or with `operation`, not both.
    #[serde(default)]
    pub operation: Option<String>,
}

/// **The parameters of `blueprint_schema`.**
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SchemaParams {
    /// **The kind of file's schema**, a [`blueprint_model::reload::FileKind::id`]: `map`, `kits`,
    /// `library`, `vocab`, `compositions`, `rigs`, `rules`, `standards`, `notes`, `relations`,
    /// `terrain`, `volume`, `agent`, `play`, `destructible_marks`, and the rest of `schema::KINDS`.
    pub kind: String,
}

/// **The parameters of `blueprint_self_test`**: none. The test is the same every time.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SelfTestParams {}

/// **The parameters of `blueprint_preview_operation` and `blueprint_apply_operation`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OperationParams {
    /// **The operation's name**, as `blueprint_list_operations` gives it. For example
    /// `map_set_settings`.
    pub operation: String,
    /// **The operation's own parameters**, as that operation's schema describes them. For example
    /// `{"bounds": [64.0, 8.0, 64.0]}`.
    #[serde(default)]
    pub parameters: Value,
}

/// **The parameters of `blueprint_apply_operation`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyParams {
    /// **The operation's name**, as `blueprint_list_operations` gives it.
    pub operation: String,
    /// **The operation's own parameters**, as that operation's schema describes them.
    #[serde(default)]
    pub parameters: Value,
    /// **Whether to preview instead of applying.** `true` changes nothing: the map, the file and
    /// the log are all left alone, and the answer says what would have changed.
    #[serde(default)]
    pub dry_run: bool,
    /// **Your name as the holder of a note you claimed.** While you hold one, an operation that
    /// reaches outside the note's area plus one cell is refused, and so is one that needs a
    /// question answered first (removing a piece a person placed, changing more than 10 pieces,
    /// touching the margin): ask with note_ask, and go on once the person answers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

/// **The parameters of `blueprint_undo_operation`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UndoParams {
    /// **The id of the change to take back**, as `blueprint_apply_operation` returned it or as
    /// `blueprint_read_oplog` lists it. Omitted: the newest change that has not been taken back.
    #[serde(default)]
    pub operation_id: Option<String>,
    /// **Whether to preview instead of undoing.** `true` changes nothing and says what the undo
    /// would set.
    #[serde(default)]
    pub dry_run: bool,
    /// **Take back the whole presented turn (E-084)**, newest change first, instead of one logged
    /// change.
    ///
    /// The person's Cmd+Z has always meant "the turn"; without this the agent's tool took one
    /// operation of it, so the same turn came back in different amounts depending on who asked.
    /// Refused when nothing has been presented, because a turn is a thing the agent declared.
    #[serde(default)]
    pub turn: bool,
}

/// **The parameters of `blueprint_run_command`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunCommandParams {
    /// **The command's id**, `<category>.<verb>`, such as `edit.undo` or `edit.redo`.
    pub id: String,
}

/// **The parameters of `blueprint_frame_view`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FrameParams {
    /// **What to frame.** `"map"`, `"selection"`, `{"placements": [ids]}` or `{"rect": [x0, z0, x1, z1]}`.
    pub target: FrameTargetArg,
}

/// **A frame target, as the caller writes it.** The editor judges every id and every number; this
/// type only shapes the call.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum FrameTargetArg {
    Named(NamedTarget),
    Placements(PlacementsArg),
    Rect(RectArg),
}

/// **A target with no arguments.** The whole map, or the locked placement.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum NamedTarget {
    Map,
    Selection,
}

/// `{"placements": [ids]}` — the placements to frame, as the map lists them.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlacementsArg {
    pub placements: Vec<String>,
}

/// `{"rect": [x0, z0, x1, z1]}` — a rectangle in map metres, with x1 > x0 and z1 > z0.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RectArg {
    pub rect: [f32; 4],
}

/// **The parameters of `blueprint_capture_still`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StillParams {
    /// **A name for the capture**, in snake_case. It names the file it is written under.
    pub label: String,
    /// **What to capture**: `surface` (the default), the image the editor draws into, or `window`,
    /// what the window itself shows (its swapchain). On macOS an unfocused window reads as one flat
    /// colour.
    #[serde(default)]
    pub source: Option<String>,
    /// **The map's own surfaces to hide for this picture** (#472): any of `terrain`, `volume`, `water`.
    ///
    /// **The one thing that makes an underground map visible.** A map whose rooms are `volume_edit`
    /// carves draws the rock they are cut into, so every still from outside shows the hill's skin and
    /// not one of its rooms. `["terrain"]` cuts the solid, so what is drawn is its interior: the floor,
    /// the walls, the vault, and the sides of the columns standing in it, from the same camera.
    ///
    /// **The camera is not touched and the map is not touched**: it is a view, put back by the next call
    /// with no `hide`.
    #[serde(default)]
    pub hide: Option<Vec<String>>,
    /// **Whether the view is put back after the picture.** `true` (the default) hides what `hide` names
    /// for the one capture and restores what was hidden before it, so a still never leaves the person's
    /// screen changed; `false` leaves the view as the call set it, for a run of stills that all want it.
    #[serde(default = "yes")]
    pub restore: bool,
    /// **A section plane: everything nearer the camera than this many metres is removed from the
    /// picture** (#472), so a room's floor can be seen from outside. The caller places it: it framed
    /// the room, so it knows how deep in the map the room's near wall stands. `hide: ["terrain"]`
    /// alone draws the interior's near surface, which is the vault; `cut_m` deletes the near half of
    /// that interior and what the picture then shows is the floor and the columns standing in it.
    #[serde(default)]
    pub cut_m: Option<f32>,
}

/// **The parameters of `blueprint_read_log`.**
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditorLogParams {
    /// **How many of the newest lines**, from 1 to 2000. The default is 200.
    #[serde(default)]
    pub lines: Option<u32>,
}

/// **The newest lines `blueprint_read_log` returns when not told.**
const LOG_LINES_DEFAULT: u32 = 200;
/// **The most lines `blueprint_read_log` returns.** A larger ask is clamped to this.
const LOG_LINES_MAX: u32 = 2000;

/// **The parameters of `blueprint_open_project`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OpenProjectParams {
    /// **The project folder**, one with an `assets/blueprint` folder, as a path on the editor's machine.
    pub root: String,
    /// **A map to open it on.** Without one, the project's own first screen.
    #[serde(default)]
    pub map: Option<String>,
}

/// **The parameters of `blueprint_snip`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SnipParams {
    /// **A name for the snip**, in snake_case. It names the directory the crop is written under.
    pub label: String,
    /// **The rectangle, in window pixels: `[x0, y0, x1, y1]`**, origin at the top left, `x1` and
    /// `y1` exclusive.
    pub rect: [i64; 4],
}

/// **The parameters of `blueprint_list_snips`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ListSnipsParams {
    /// **How many snips to list, newest first**, from 1 to 20. The default is 5, a screenful.
    #[serde(default = "five")]
    pub limit: usize,
}

/// **Five snips, a screenful of metadata.**
fn five() -> usize {
    5
}

/// **The parameters of `blueprint_feedback_file_issue`** (#495): the report's stem.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FeedbackIssueParams {
    /// **The report's stem**, its folder name: `feedback-1794921159460`.
    pub stem: String,
}

/// **The parameters of `blueprint_feedback_list` and `blueprint_project_feedback_list`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FeedbackListParams {
    /// **How many reports to list, newest first**, from 1 to 20. The default is 5, a screenful.
    #[serde(default = "five")]
    pub limit: usize,
}

/// **The parameters of `blueprint_style`** (SG-02, #401).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StyleParams {
    /// **A map of the project**, by name. With none, the answer is the game's files alone.
    #[serde(default)]
    pub map: Option<String>,
    /// **A point on that map**, `[x, z]` in map metres: the area that holds it closes the chain. It must be
    /// inside the map, and it needs `map`.
    #[serde(default)]
    pub at: Option<[f32; 2]>,
    /// **A topic the game states**, such as `dialogue`: each place that has a file for it adds that file.
    #[serde(default)]
    pub topic: Option<String>,
}

/// **One style line a hand-back cites**: the page, relative to the project root, and the line, counted from 1.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FollowedParam {
    /// The page, as `blueprint_style` headed it.
    pub path: String,
    /// The line, counted from 1 as an editor shows it.
    pub line: u32,
}

/// **One small edit of a style page**: the exact span and what replaces it.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StyleEditParam {
    /// **The exact text to replace**, which must be in the page exactly once.
    pub find: String,
    /// What takes its place; empty takes the span out.
    pub replace: String,
}

/// **The parameters of `blueprint_style_propose`** (SG-07, #406).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StyleProposeParams {
    /// **The page**, relative to the project root: `design/style.md`.
    pub path: String,
    /// **Small edits, in order**, each `find` exactly once in the page as the edit before it left it.
    pub edits: Vec<StyleEditParam>,
    /// **Why the page should say this**, one sentence.
    pub why: String,
}

/// **One report, as an agent reads it**: the record, both picture paths, and the log tail's path.
///
/// **Both readers answer with the same shape** — the developers' channel and the project's differ in
/// where the files are, not in what a report is — so an agent that has read one has read both.
fn list_reports(dir: &std::path::Path, limit: usize) -> CallToolResult {
    if !(1..=20).contains(&limit) {
        return refusal(&format!(
            "limit is {limit}, and it must be from 1 to 20."
        ));
    }
    let (reports, mut problems) = blueprint_model::feedback::read_reports(dir);
    let mut content = Vec::new();
    if let Some(newest) = reports.first() {
        let path = dir.join(&newest.picture.file);
        match std::fs::read(&path) {
            Ok(bytes) => content.push(rmcp::model::ContentBlock::image(
                base64_encode(&bytes),
                "image/png",
            )),
            // **A picture that cannot be read is named, and the list still answers** — the same rule
            // `blueprint_list_snips` follows.
            Err(e) => problems.push(format!(
                "{}: the picture could not be read: {e}",
                path.display()
            )),
        }
    }
    let shown: Vec<Value> = reports
        .iter()
        .take(limit)
        .map(|report| {
            let mut entry = serde_json::to_value(report).unwrap_or(Value::Null);
            if let Value::Object(map) = &mut entry {
                map.insert(
                    "picture_path".to_string(),
                    json!(dir.join(&report.picture.file).to_string_lossy()),
                );
                if let Some(picks) = &report.picks_picture {
                    map.insert(
                        "picks_picture_path".to_string(),
                        json!(dir.join(&picks.file).to_string_lossy()),
                    );
                }
                if let Some(log) = &report.log {
                    map.insert(
                        "log_path".to_string(),
                        json!(dir.join(log).to_string_lossy()),
                    );
                }
            }
            entry
        })
        .collect();
    let answer = json!({
        "dir": dir.to_string_lossy(),
        "total": reports.len(),
        "shown": shown.len(),
        "reports": shown,
        "not_reports": problems,
    });
    content.push(rmcp::model::ContentBlock::text(
        serde_json::to_string_pretty(&answer).unwrap_or_default(),
    ));
    CallToolResult::success(content)
}

/// **The chain the editor composed, as content**: its text, then each picture it named as an image after a line holding
/// the picture's path and its caption (SG-02, #401).
///
/// **The editor chose the pictures and resolved each to a file inside the project**; this reads those files and
/// nothing else. A picture it cannot read is named in a last block with the reason, and the answer still arrives:
/// the words of the style are worth more than the picture that failed.
fn style_answer(answer: &Value) -> CallToolResult {
    let text = answer.get("text").and_then(Value::as_str).unwrap_or_default();
    let mut content = vec![rmcp::model::ContentBlock::text(text.to_owned())];
    let mut unreadable: Vec<String> = Vec::new();
    for picture in answer.get("pictures").and_then(Value::as_array).into_iter().flatten() {
        let field = |name: &str| picture.get(name).and_then(Value::as_str).unwrap_or_default();
        let (file, caption, path, mime) = (field("file"), field("caption"), field("path"), field("mime"));
        let caption = if caption.is_empty() { "no caption" } else { caption };
        match std::fs::read(path) {
            Ok(bytes) => {
                content.push(rmcp::model::ContentBlock::text(format!("Picture `{file}`: {caption}")));
                content.push(rmcp::model::ContentBlock::image(base64_encode(&bytes), mime.to_owned()));
            }
            Err(e) => unreadable.push(format!("- `{file}`, \"{caption}\": the picture could not be read: {e}")),
        }
    }
    if !unreadable.is_empty() {
        content.push(rmcp::model::ContentBlock::text(format!("## Pictures not attached\n\n{}\n", unreadable.join("\n"))));
    }
    CallToolResult::success(content)
}

/// **The parameters of `blueprint_capture_sequence`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SequenceParams {
    /// **A name for the sequence**, in snake_case. It names the directory the frames land in.
    pub label: String,
    /// **How long the sequence runs, in seconds.** Between a moment and ten.
    #[serde(default = "two_seconds")]
    pub seconds: f32,
    /// **How many frames a second.** Four by default; a whole rate from two to fifteen. A higher rate costs frame time while recording.
    #[serde(default = "four_fps")]
    pub fps: u32,
    /// **The operation to apply, once frame 0 is in.** Without it the sequence is the settle
    /// alone, which is what a reviewer wants when they are checking that nothing moved.
    #[serde(default)]
    pub operation: Option<SequenceOperation>,
}

/// **The operation a sequence applies.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SequenceOperation {
    /// **The operation's name**, as `blueprint_list_operations` gives it.
    pub name: String,
    /// **Its parameters.**
    #[serde(default)]
    pub parameters: Value,
}

/// **Two seconds, which is one contact sheet at four frames a second.**
fn two_seconds() -> f32 {
    2.0
}

/// **Four frames a second, the brief's default.**
fn four_fps() -> u32 {
    4
}

/// **The parameters of `blueprint_read_oplog`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogParams {
    /// **Read from this sequence number on.** `0` is the beginning of the log.
    #[serde(default)]
    pub since_seq: u64,
    /// **How many entries at most**, 1 to 200. The default is 50, which is a screenful.
    #[serde(default = "fifty")]
    pub limit: usize,
}

/// **The parameters of `blueprint_jobs_stop`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct JobsStopParams {
    /// **Which job to stop**: its own name. `thumbs` is the palette's thumbnail bake, and
    /// `blueprint_state.jobs.items` lists every key that is running.
    pub key: String,
}

/// **The parameters of `blueprint_state`.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StateParams {
    /// **Where the pinned list starts**, counted in map order. `0` is the first pin. When there are
    /// more than 50 pins, the answer's `pinned_next` names the value to pass for the next page.
    #[serde(default)]
    pub pinned_from: usize,
    /// **A sent note's id, to read everything needed to work it** (the answer's `notes.bundle`): its
    /// text, its area and margin, the pieces near it, the kit's palette, the camera the person had,
    /// how to get a picture of the area, and the earlier notes and how they turned out. Claim the note
    /// first with `note_claim`; `notes.sent` lists what can be claimed.
    #[serde(default)]
    pub note: Option<String>,
}

/// **The parameters of `blueprint_volume_report`** (#473).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VolumeReportParams {
    /// **The map to report on.** Absent is the open map.
    #[serde(default)]
    pub map: Option<String>,
    /// **Points to answer "is this floor air?" for**, each `[x, y, z]` in map metres. The answer's `at`
    /// lists, per point, the ordered entries that contain it and whether the result is solid. Empty
    /// answers none.
    #[serde(default)]
    pub at: Vec<[f32; 3]>,
}

/// **The parameters of `blueprint_present`.** A result, handed back for review.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PresentParams {
    /// What the result is called. Required: a presentation without a title is a mystery.
    pub title: String,
    /// One paragraph saying what was made.
    pub summary: String,
    /// **What the person asked for**, quoted back to them.
    #[serde(default)]
    pub specified: Vec<String>,
    /// **What you decided on your own.** The brief asks for this distinction by name: a person who
    /// cannot see it cannot correct the right half of the result.
    #[serde(default)]
    pub inferred: Vec<String>,
    /// **Up to three places worth checking.** More than three is not a short list; the editor
    /// refuses a fourth.
    #[serde(default)]
    pub check: Vec<CheckItemParam>,
    /// **The operations this covers**, as `blueprint_apply_operation` returned them. Undo all walks
    /// them newest first, so an id the log does not hold is refused.
    #[serde(default)]
    pub operation_ids: Vec<String>,
    /// **The style lines the work followed** (SG-09, #408): `[{path, line}]`, taken from a chain `blueprint_style` returned in this session. Each must be a
    /// non-empty line a page still says; one that is not is refused with the entry and the reason. Optional: a hand-back with no style involved cites nothing.
    #[serde(default)]
    pub followed: Vec<FollowedParam>,
}

/// **One place worth a look**, as the agent states it.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckItemParam {
    /// What to look at, in your own words.
    pub what: String,
    /// Placement ids to frame in the viewport first, if any.
    #[serde(default)]
    pub placements: Vec<String>,
}

/// **The parameters of `blueprint_wish`** (#353). The one tool that would most have helped.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WishParams {
    /// **The hand-back this wish follows**: a presentation's id (`present-1`) or a note the agent
    /// resolved (`n3`). **One wish per hand-back**, so a second citing the same one is refused.
    pub after: AfterParam,
    /// What the task was.
    pub task: String,
    /// What cost steps: where the work stalled, repeated itself, or needed a second call to learn
    /// what one call should have said.
    pub hardship: String,
    /// **The single tool that would most have helped**, named and described in one sentence. Its
    /// limit is [`blueprint_model::wish::WISH_TOOL_MAX_CHARS`], stated in the tool's schema.
    pub tool: String,
    /// A call to that tool, as you would like to write it. Its limit is
    /// [`blueprint_model::wish::WISH_FIELD_MAX_CHARS`], stated in the tool's schema.
    pub example_call: String,
}

/// **Which hand-back a wish follows**, as the tool takes it: exactly one of the two.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AfterParam {
    /// A presentation's id, as `blueprint_present` returned it.
    #[serde(default)]
    pub presentation: Option<String>,
    /// A note's id, as the agent resolved it.
    #[serde(default)]
    pub note: Option<String>,
}

/// **The parameters of `blueprint_lesson`** (AG-06, #351). One line for the project's guide, learned
/// from the task just handed back.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LessonParams {
    /// **The hand-back this lesson follows**: a presentation's id (`present-1`) or a note the agent
    /// resolved (`n3`). **One lesson per hand-back**, so a second citing the same one is refused.
    pub after: AfterParam,
    /// **The one line the guide gains**, within
    /// [`blueprint_model::lesson::LESSON_MAX_CHARS`] and with no newline: a lesson is one line.
    pub lesson: String,
    /// Why it is worth the line, within [`blueprint_model::lesson::WHY_MAX_CHARS`].
    pub why: String,
}

/// **The parameters of `blueprint_list_pieces`.** Every filter is optional and they combine.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PiecesParams {
    /// **Where the page starts**, counted in the filtered list. `0` is the first piece. When there
    /// are more than 50 pieces, the answer's `pieces_next` names the value to pass for the next page.
    #[serde(default)]
    pub pieces_from: usize,
    /// **`true` leaves out the pieces that name no mesh** (the flat footprint markers such as roads
    /// and fields), so the list is only the pieces that are drawn with a model.
    #[serde(default)]
    pub with_mesh: bool,
    /// **`true` keeps only the pieces no one has labelled**: no label record, or a record that
    /// labels none of its axes.
    #[serde(default)]
    pub unlabelled: bool,
    /// **`true` keeps only the pieces with an axis a model or an agent wrote and no person has confirmed.** Also spelled `unconfirmed`: it matches agent-written axes too, not only a model's.
    #[serde(default, alias = "unconfirmed")]
    pub by_model: bool,
    /// **Keeps only the pieces whose label record says this confidence**: `high`, `medium` or `low`.
    /// Any other word is refused.
    #[serde(default)]
    pub confidence: Option<String>,
    /// **Keeps only the pieces whose id contains this text.**
    #[serde(default)]
    pub id_contains: Option<String>,
}

/// **Fifty entries, which is a screenful.**
fn fifty() -> usize {
    50
}

/// **The server.**
#[derive(Clone)]
pub struct Blueprint {
    /// **The editor's BRP endpoint.**
    editor: Brp,
    /// **The project directory, and the only place the server may point at**, when one was named.
    /// `None` follows whatever the editor has open.
    project: Option<PathBuf>,
    /// **Tools this server does not serve at all** (`--hide-tools`), so they are absent from an agent's
    /// tool list rather than refused when it calls one.
    ///
    /// A benchmark arm that hides `blueprint_frame_view` is asking what an agent does *without* that tool,
    /// and a tool that is in the list and refuses every call answers a different question — the agent
    /// reads the name, decides to try it, and burns a call learning it cannot. So this is the list the
    /// `tools/list` answer leaves out, and `call_tool` refuses a hidden name as it refuses an unknown one.
    hidden: Vec<String>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Blueprint {
    /// **A server for the editor at `editor`, rooted at `project`, or following the editor when `None`.**
    pub fn new(editor: Brp, project: Option<PathBuf>) -> Self {
        Self::with_hidden(editor, project, Vec::new())
    }

    /// **The same server, serving every tool but `hidden`.**
    ///
    /// A name in `hidden` that this build has no tool for is **refused at start** by [`checked_hidden`],
    /// not silently ignored: a benchmark arm naming a tool that does not exist would otherwise measure
    /// the full tool set while its report said one was missing.
    pub fn with_hidden(editor: Brp, project: Option<PathBuf>, hidden: Vec<String>) -> Self {
        let tool_router = Self::tool_router();
        Blueprint {
            editor,
            project,
            hidden,
            tool_router,
        }
    }

    /// **Every tool this build has**, whatever is hidden, so a start can check a `--hide-tools` name.
    pub fn tool_names(&self) -> Vec<String> {
        self.tool_router
            .list_all()
            .iter()
            .map(|t| t.name.as_ref().to_owned())
            .collect()
    }

    /// **Whether this server serves `name`.**
    fn serves(&self, name: &str) -> bool {
        self.tool_router.get(name).is_some() && !self.hidden.iter().any(|h| h == name)
    }

    /// **The project's root, for a tool that names a file**, when the server was given one.
    pub fn project(&self) -> Option<&std::path::Path> {
        self.project.as_deref()
    }

    /// **Grab a picture of the editor, as base64 PNG**, through `bevy_fairy/screenshot` and
    /// `bevy_fairy/capture` (`Brp::screenshot`). `source` is bevy_fairy's: `offscreen`, the surface
    /// the editor draws into, or `window`, the window's swapchain.
    async fn grab(&self, source: &str) -> Result<String, String> {
        self.editor
            .screenshot(json!({"source": source}))
            .await
            .map_err(|e| format!("the editor's {e}"))
    }

    /// **Hide the surfaces a capture asks for**, and answer what was hidden before it, so the caller can put
    /// the view back. `None` when the call names none: the editor is not asked, so a still with no `hide`
    /// costs no BRP round trip and cannot change the view.
    /// **The section plane for one picture, and the camera's own range back** (#472): \`None\` asks
    /// the editor for no cut, and the same call with \`null\` puts the camera's near distance back.
    async fn set_cut(&self, cut_m: Option<f32>) -> Result<(), String> {
        self.editor
            .call("blueprint/section_cut", json!({"cut_m": cut_m}))
            .await
            .map(|_| ())
            .map_err(|e| format!("the section plane could not be set: {e}"))
    }

    async fn set_hidden(&self, hide: Option<&[String]>) -> Result<Option<Vec<String>>, String> {
        let Some(hide) = hide else { return Ok(None) };
        let answer = self
            .editor
            .call("blueprint/surface_hide", json!({"hide": hide}))
            .await?;
        let was = answer.get("was").and_then(|w| w.as_array()).map(|w| w.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect());
        Ok(Some(was.unwrap_or_default()))
    }

    /// **Where a capture lands.**
    ///
    /// **Under the project, in a directory the brief calls working files.** The brief says raw
    /// captures go under `target/loop/<date>-<sha>/`, and **the project directory is where the
    /// editor's own data lives** — so a capture is written beside the project, under `target`,
    /// and never into the project's own data. **Without `--project`**, under the per-user data
    /// folder's `captures` (`blueprint_model::user_dirs::captures_dir`).
    ///
    /// **The label is checked first, because it is joined into the path.** A label with `..`, a
    /// leading `/`, or an empty part would put the picture outside `captures`, or replace the
    /// base path with an absolute one. A refused label is answered before the editor is asked.
    fn capture_dir(&self, label: &str) -> Result<std::path::PathBuf, String> {
        if !is_capture_label(label) {
            return Err(format!(
                "`{label}` is not a capture label. Use letters, digits, `_` and `-`, with `/` \
                 between the parts; no empty part, `.` or `..`."
            ));
        }
        let base = match &self.project {
            Some(project) => project.join("target").join("loop").join("captures"),
            None => blueprint_model::user_dirs::captures_dir().ok_or(
                "this system has no data directory, so a capture has nowhere to go; pass --project",
            )?,
        };
        Ok(base.join(label))
    }

    /// **A name for a capture, unique enough that two do not collide.**
    fn stamp(&self) -> String {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        format!("{now}")
    }

    /// **Is the editor on the project this server was told about?**
    ///
    /// **The one thing the two processes have to agree on**, and the server cannot know it by
    /// itself: the editor is the one that knows which project it has open. **So the server asks
    /// the editor for its project root and compares**, and a mismatch is a refusal with a sentence
    /// rather than a change to somebody else's map. **A server told no project follows the editor**,
    /// so there is nothing to compare.
    pub async fn check_project(&self) -> Result<(), String> {
        let Some(project) = &self.project else {
            return Ok(());
        };
        let told = project.canonicalize().unwrap_or_else(|_| project.clone());
        let answer = self.editor.call("blueprint/project_root", json!({})).await?;
        let editor_root = answer
            .get("project_root")
            .and_then(|v| v.as_str())
            .ok_or("the editor did not say which project it has open.")?;
        let editor_root = std::path::PathBuf::from(editor_root);
        let editor_root = editor_root.canonicalize().unwrap_or(editor_root);
        if editor_root != told {
            return Err(format!(
                "the editor has `{}` open and this server was told `{}`. Point both at the same \
                 project, or the changes would land in the wrong one.",
                editor_root.display(),
                told.display()
            ));
        }
        Ok(())
    }

    /// **`blueprint_guide` — how to work in DarkFactory, in the person's and the project's own words**
    /// (#346, AG-01; #347, AG-02).
    ///
    /// **Three layers, in order.** The shipped guide is `guide.md`, compiled into the binary, so this
    /// answers with no editor running and no project open — which matters, because an agent reads it
    /// before it has done anything. Then the person's `<config_dir>/Blueprint/AGENTS.md` and the open
    /// project's `.dark_factory/AGENTS.md`, both **read by the editor** and never by this server: the
    /// editor holds the open project and the person's directories, so it is the one place that knows
    /// where they are. **A later layer adds to the earlier ones; nothing is merged or overridden key by
    /// key.**
    ///
    /// **A missing file is an absent layer, and a file that cannot be served is a problem.** Each layer
    /// arrives under a heading naming its source and path, and a file that cannot be read, is not UTF-8
    /// or is too large is left out with the refusal said in one line — never truncated into a guide an
    /// agent would half-follow.
    ///
    /// **With no editor, the shipped layer is the whole guide.** A server an agent starts before the
    /// editor does still answers with something true, and says the two other layers were not read.
    ///
    /// **One text, two readers.** The shipped string is what [`docs::render`] writes into
    /// `docs/mcp-tools.md`, so the guide an agent is given and the guide a person reads cannot
    /// disagree.
    #[tool(
        name = "blueprint_guide",
        description = "How to work in DarkFactory, in the order a task goes: orient, change, what to do \
                       when a person pauses the agent's writes, how to show your work, and how to \
                       hand a result back. Call this before any other tool on your first task in a \
                       project: the other tools say what one tool does, and this says which of them \
                       in what order, and what the person on the other end expects. It returns the \
                       shipped guide, then the person's own guide and the open project's, each under \
                       a heading that names its source and path. A project's guide is served only once \
                       the person has allowed that exact content (or trusted the project), so an \
                       unread one arrives as one line saying they have not allowed it, never as its \
                       text. It takes no parameters and changes nothing.",
        annotations(
            title = "Read the guide to working in DarkFactory",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn guide(&self) -> Result<CallToolResult, ErrorData> {
        let mut text = docs::GUIDE.to_owned();
        match self.editor.call("blueprint/agent_guide", json!({})).await {
            Ok(answer) => {
                let layers = answer
                    .get("layers")
                    .and_then(|l| l.as_array())
                    .cloned()
                    .unwrap_or_default();
                for layer in &layers {
                    // **The layer exactly as the editor rendered it** — heading, source and path — so the
                    // one renderer is `blueprint_model::agent_guide::Layer::rendered` and this server
                    // cannot spell a heading differently or print a path the agent does not need.
                    let Some(rendered) = layer.get("rendered").and_then(|r| r.as_str()) else { continue };
                    text.push('\n');
                    text.push_str(rendered.trim_end());
                    text.push('\n');
                }
                // **A withheld layer is one line where the layer would have been**, and the text it
                // withheld is never sent: the editor never puts it in the answer, and this reads only what
                // the editor sent.
                if let Some(withheld) = answer.get("withheld").and_then(|w| w.as_str()) {
                    text.push('\n');
                    text.push_str(&format!("**{withheld}**\n"));
                }
                if !layers.is_empty() || answer.get("withheld").and_then(|w| w.as_str()).is_some() {
                    text.push('\n');
                }
                if let Some(trouble) = answer.get("trouble").and_then(|t| t.as_str()) {
                    text.push_str(&format!("\n**No answer can be recorded about it: {trouble}**\n"));
                }
                for problem in answer.get("problems").and_then(|p| p.as_array()).cloned().unwrap_or_default() {
                    // **Every part the problem has**, because the reason is what an agent acts on: a path
                    // with no `why` is a refusal a reader has to guess the cause of.
                    let said = problem.get("what").and_then(|w| w.as_str()).unwrap_or("a guide was left out");
                    let at = problem
                        .get("where")
                        .and_then(|w| w.get("file"))
                        .and_then(|f| f.as_str())
                        .unwrap_or("no file was named");
                    let why = problem.get("why").and_then(|w| w.as_str());
                    let fix = problem.get("fix").and_then(|w| w.as_str());
                    let mut line = format!("\n**A guide was not served: {said} — {at}");
                    if let Some(why) = why {
                        line.push_str(&format!(": {why}"));
                    }
                    line.push('.');
                    if let Some(fix) = fix {
                        line.push_str(&format!(" {fix}."));
                    }
                    line.push_str("**\n");
                    text.push_str(&line);
                }
            }
            // **No editor, or an older one that has no such method.** The shipped layer is the whole
            // guide and says so, because an agent that is told nothing about the two files cannot know
            // whether they are empty or unread.
            Err(e) => {
                // **The project's guide is named as withheld, not as a file to go and read.** Its whole
                // point is that an agent cannot serve itself a project's rules: the editor reads it only
                // once a person has allowed that content, so an agent told to `cat` it would be a route
                // around the check this server exists to hold.
                text.push_str(&format!(
                    "\n**Neither the person's guide nor this project's guide was read: {e}** The \
                     person's is `AGENTS.md` in the editor's config folder, and reading it is safe. \
                     This project's guide (`.dark_factory/AGENTS.md`) is served only by the editor, \
                     and only once the person has allowed that exact content there — it is not a file \
                     for you to read around this tool.\n"
                ));
            }
        }
        Ok(CallToolResult::success(vec![
            rmcp::model::ContentBlock::text(text),
        ]))
    }

    /// **`blueprint_list_operations` — every operation, its schema, and how it is undone.**
    ///
    /// **A read.** It asks the running editor, so the list is the editor's own and cannot be out
    /// of date.
    #[tool(
        name = "blueprint_list_operations",
        description = "Every operation the running bevy_blueprint editor knows, with each one's \
                       parameters' JSON schema and how it is undone. Call this before \
                       blueprint_apply_operation so the parameters are the ones the operation \
                       actually takes. Returns the list; it does not change anything. The undo \
                       column names the strategy an operation declares; blueprint_undo_operation \
                       performs it. Pass query (words such as \"carve\" or \"note\") to get only \
                       the operations whose name or parameters match, best first, and total, the \
                       number of operations there are. Pass operation (an operation's exact name, \
                       such as data_set) to get only that one operation: each parameter with its \
                       description, and one example call, in a few hundred bytes rather than the \
                       whole list. Ask with query or with operation, not both.",
        annotations(
            title = "List the editor's operations",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_operations(
        &self,
        Parameters(p): Parameters<ListParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.editor.call("blueprint/operation_list", json!({"query": p.query, "operation": p.operation})).await {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_schema` — the JSON Schema of one kind of project file.**
    ///
    /// **A read, and the answer to "what may this file say?"** The schema is generated from the Rust
    /// type the editor's reader reads, so it cannot describe a file the reader refuses; a field's doc
    /// comment is its `description`, and a field that names another file's value carries
    /// `x-dark-factory-ref: <kind>` so a caller can follow it. The same file is written into the
    /// project at `.dark_factory/schema/<kind>.schema.json` for an outside editor's YAML extension.
    #[tool(
        name = "blueprint_schema",
        description = "The JSON Schema of one kind of project file, generated from the Rust type \
                       the editor reads, so it describes exactly what the reader accepts. Its \
                       properties carry the field's doc comment as description, and a field that \
                       names another file's value carries x-dark-factory-ref (the kind it names). \
                       kind is a file kind's id: map, kits, library, vocab, compositions, rigs, \
                       rules, standards, notes, relations, terrain, volume, agent, play, \
                       destructible_marks and the rest. Returns the schema as JSON; it changes \
                       nothing. The same schema is on disk at \
                       .dark_factory/schema/<kind>.schema.json.",
        annotations(
            title = "One file kind's JSON Schema",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn schema(&self, Parameters(p): Parameters<SchemaParams>) -> Result<CallToolResult, ErrorData> {
        // **Generated here, not asked of the editor**: the type is this crate's own dependency, so a
        // schema answers with no editor running and no project open.
        match blueprint_model::schema::for_kind(&p.kind) {
            Ok(schema) => Ok(CallToolResult::structured(schema)),
            Err(problem) => Ok(refusal(&problem.line())),
        }
    }

    /// **`blueprint_preview_operation` — what an operation would change, and nothing changed.**
    ///
    /// **A read.** The editor's `plan` runs and the answer comes back; the map, the file and the
    /// log are untouched.
    #[tool(
        name = "blueprint_preview_operation",
        description = "What an operation would change, and nothing changed: the map, the data file \
                       and the operation log are all left alone. Use this to check a proposal \
                       before applying it. Returns what would change and shown, where the change \
                       would be seen (the place the apply will name), or the refusal.",
        annotations(
            title = "Preview an editor operation",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn preview_operation(
        &self,
        Parameters(p): Parameters<OperationParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call(
                "blueprint/operation_preview",
                json!({"operation": p.operation, "parameters": p.parameters}),
            )
            .await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_apply_operation` — run one operation.**
    ///
    /// **Not a read.** It changes the map, writes the data file, and appends a line to the
    /// operation log. **`dry_run` previews instead**, and nothing is written.
    #[tool(
        name = "blueprint_apply_operation",
        description = "Run one of the editor's operations. Changes the editor (most operations \
                       change the open map and write its file; brush_arm only arms a piece) and \
                       appends a line to the operation log. Set dry_run to true to see \
                       what would change without changing it. Returns the operation's id, which \
                       can be cited in a report to the developer, and the log's own record of it. \
                       When the operation changes a document and a map is open, the answer's \
                       checks field names the edge faults and overlapping placements the change \
                       introduced (new, at most 20 of each) with new_total, and each kind's total \
                       on the whole map; it is null otherwise. A rules_set whose weights name a \
                       prefix no piece in the library starts answers tunes_nothing (in \
                       would_change on a preview, and in the summary on an apply), naming the \
                       prefix and the pieces it does not tune: the change is applied, because \
                       removing a weight list is how an undo restores one, so read that field \
                       rather than assuming a weight did something. \
                       The operation replaces the values it names rather than adding to them, \
                       so it is marked destructive. The answer's shown field says where the \
                       change is seen — a box on the map, a library piece, a composition, a rig, \
                       a note, a project file, or nowhere — which is what the person's view \
                       follows. Take a change back with blueprint_undo_operation and the returned id.",
        annotations(
            title = "Apply an editor operation",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn apply_operation(
        &self,
        Parameters(p): Parameters<ApplyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The project is checked before anything is changed.** A change to the wrong project is
        // worse than no change, and the server cannot know which project the editor has open
        // except by asking.
        if let Err(why) = self.check_project().await {
            return Ok(refusal(&why));
        }
        match self
            .editor
            .call(
                "blueprint/operation_apply",
                json!({
                    "operation": p.operation,
                    "parameters": p.parameters,
                    "dry_run": p.dry_run,
                    "agent": p.agent,
                }),
            )
            .await
        {
            Ok(v) => {
                // **A refusal is an error, and the tool says so.** The editor answers a refusal
                // with `outcome: "refused"` and a reason, and the agent must be able to tell that
                // apart from a success without reading prose.
                let refused = v.get("outcome").and_then(|o| o.as_str()) == Some("refused");
                if refused {
                    Ok(CallToolResult::structured_error(v))
                } else {
                    Ok(CallToolResult::structured(v))
                }
            }
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_state` — what the editor is holding right now.**
    ///
    /// **A read, and the answer to "what does the editor look like?"** — the question an agent
    /// asks before it changes anything, and the question it asks afterwards to check that the
    /// change landed.
    ///
    /// **It is a summary, not a dump.** The editor holds hundreds of descriptors and thousands of
    /// placements; **none of that is what an agent needs to know.** What it needs is the *shape* of
    /// the current state — which map, how big, how many pieces, what is armed, what is selected,
    /// which door and tab are open, and what the editor last said.
    #[tool(
        name = "blueprint_state",
        description = "What the editor is holding right now: the open map and its bounds, how many \
                       pieces it has, what is armed, what is selected, which door and tab are open, \
                       and what the editor last said and what has gone wrong. The editor section also \
                       names the next undo and redo (undo_next, redo_next) and how many are stacked \
                       (undo_depth, redo_depth). The map section names up to 50 pinned placements \
                       by id and reason (pinned). pinned_total is the full count; past 50, pass \
                       pinned_from (from pinned_next) to read the rest. With --live, game is the running \
                       game's verdict on the last reload: null (none), \"pending\" (it has not \
                       answered the latest write) or {ok, message, at_unix_ms, seq, kinds}; kinds has \
                       one entry for every file the game read, {kind, file, outcome (applied or \
                       refused), field, reason}, so a refusal names its file and field. reloads is the \
                       editor's own verdict on the files it draws from (terrain, water_field, volume, \
                       forest, trees, notes, family, rigs): {ok, seq, kinds} with the same entries, where \
                       an outcome is applied, refused (the file did not read: it names the field, and \
                       the editor keeps what it had) or needs_restart (the file reads but the editor \
                       holds that kind from when it started, as it does rigs.yaml, so the edit is on \
                       disk and not in use); ok is false while any file is not applied, and seq counts \
                       decisions, one per edit. reload_files is what became of the latest edit of \
                       every file of the open project, as a running game says it: {now_unix_ms, game \
                       {connected, watching (this editor runs with --live, the only way it reads a \
                       game's verdict), at_unix_ms, seq}, files}, one entry per file {kind, file, state, \
                       field, reason, at_unix_ms (when the game decided), modified_unix_ms (when the \
                       file was written), editor (the editor's own verdict on it, or null)}; state is \
                       no_game_connected (no game verdict has reached the editor, so nothing answered), pending \
                       (written after the game's last decision, so its answer is about earlier text), \
                       game_applied, game_refused (with field and reason), needs_restart or not_read \
                       (the game's last decision did not read the file). frame is the editor's own \
                       clock: frame_ms (wall time between frames), busy_ms (work done inside one \
                       frame) and fps; a high frame_ms with a low busy_ms is the machine, not the \
                       editor. The notes section lists the sent and claimed notes (id, \
                       holder, lease_until); pass note=<id> for that note's bundle: text, area and \
                       margin, nearby pieces, the palette, the camera, a picture recipe and earlier notes. \
                       repetition names up to 10 visible repeats (the same piece at one yaw and tip \
                       within 6 m; any yaw for a round kind), the one a player notices first first: \
                       descriptor, patched, role (clutter or structure), pieces, pairs, score; total \
                       counts them all; the radius, the structure kinds, their weight and the round \
                       kinds come from the project's repetition.yaml, \
                       re-read on every call, and a bad file is named in error. \
                       relations checks the <map>.relations.yaml beside the open map, re-read on \
                       every call: each relation (of, kind) with outcome met, unmet or missing, \
                       measured (metres, or degrees off for Faces), by (the piece that breaks it) \
                       and missing_id; unmet and missing count them; error names a file that does not parse. \
                       standards checks the project's standards.yaml (beside kits.yaml), re-read on \
                       every call: breaches (piece, rule, dim, found metres, why below_min, above_max, \
                       not_multiple_of or unmeasured, bound), idle, the rules no piece is covered by, \
                       and unnested, the pairs of rules whose multiple_of modules do not nest (rules, \
                       dims, modules, meet metres where they share a grid line again or null, message); \
                       error names a file that does not parse. \
                       dead_ends is the kit stress test: every declared face (piece, dir N/E/S/W, \
                       face tokens, closest {piece, turn, misses [{at, want, found}]} the piece \
                       nearest to meeting it or null, message) that no piece of the kit meets at any quarter turn, \
                       up to 20, with total counting them all. \
                       stress is the rest of the kit stress test, from the library on every call: \
                       ring, the loop the kit closes, four {piece, turn in degrees} north-west, \
                       north-east, south-west, south-east; room, the smallest room with a middle, \
                       three rows of three {piece, turn} north to south and west to east, the middle \
                       at [1][1]; each null when the kit cannot build it; declared counts the pieces \
                       with an edge token, and at 0 the nulls say the kit is unlabelled. \
                       Returns that summary \
                       as JSON, and changes nothing. \
                       Call it before a change to know what you are changing, and after one to check \
                       that it landed.",
        annotations(
            title = "Read the editor's state",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn state(
        &self,
        Parameters(p): Parameters<StateParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call("blueprint/state", json!({"pinned_from": p.pinned_from, "note": p.note}))
            .await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_volume_report` — what a map's carve list actually made** (#473).
    ///
    /// **The one question a carve raises and nothing else answered.** `volume_edit` takes one entry per
    /// call and answers `edited`, and `blueprint_preview_operation` previews one edit against the map —
    /// so every defect the issue names exists only in the *sequence* and no per-call preview can show
    /// one. Three were found in one session by hand, with a Python voxel sampler over the YAML.
    ///
    /// **A read**: no file is written, the map is not marked dirty, no operation log line is made.
    #[tool(
        name = "blueprint_volume_report",
        description = "What a map's carve list actually made, and nothing changed: every entry \
                       (index, kind, mode, world bounds); carves_that_cut_a_later_solid, an Add that \
                       a Carve after it cuts through, with overlapping_m how deep it reaches (order is \
                       load-bearing and nothing else says so); components, the void split into \
                       connected regions so a sealed room is one list of one; isolated, carves reaching \
                       no region at all (one a later Add filled); and, for each point in `at`, the \
                       ordered entries containing it and whether the result is solid, which is the \
                       ground truth for \"is this floor air?\". A crown vault reports as a component of \
                       one and is not an error: it is a crypt's ceiling void, sealed on purpose. \
                       Call it after carving a sequence, before claiming the map is right.",
        annotations(
            title = "Report on a map's carved volume",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn volume_report(
        &self,
        Parameters(p): Parameters<VolumeReportParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call("blueprint/volume_report", json!({"map": p.map, "at": p.at}))
            .await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_present` — hand the result back for review (S1.7).**
    ///
    /// **The brief's fourth step, in one call.** A tool that only reports leaves the person to work
    /// out what changed; this puts the result in front of them as a Keep / Undo all question, with
    /// what they asked for beside what was assumed, and the operations it covers.
    ///
    /// **It returns at once.** An MCP call that blocked on a person would deadlock with the editor's
    /// own frame, so the answer is read later from `blueprint_state`'s `presentation.answer`.
    #[tool(
        name = "blueprint_present",
        description = "Put a result in front of the person: a title, a one-paragraph summary, what \
                       they specified, what you inferred, up to three places worth checking, and the \
                       operation ids it covers. The editor shows it as a question: Keep, or Undo all \
                       (which undoes every listed operation, newest first). This call returns at \
                       once; read the person's answer from blueprint_state's presentation.answer \
                       (\"kept\", \"undone\", or why the undo stopped). It does not change the map by \
                       itself. If you called blueprint_style, also cite the style lines you worked to: \
                       followed, a list of {path, line} taken from a chain blueprint_style returned in \
                       this session. The person's card lists each line's text with a link that opens it; \
                       a line from a page you were never served, past a page's end, blank, or on a page \
                       that has changed since is refused with the entry and the reason. A hand-back with \
                       no style involved cites nothing.",
        annotations(
            title = "Present a result for review",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn present(
        &self,
        Parameters(p): Parameters<PresentParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The first place to check is framed before the question goes up**, and the editor does it
        // (AF-02, #358): `blueprint/present` raises a *follow* with the check's placements, so the
        // hand-back and an agent's change share one camera, one ease and one crossing. A second
        // `blueprint/frame_view` call here would be a jump that fought the follow already in flight.
        let body = json!({
            "title": p.title,
            "summary": p.summary,
            "specified": p.specified,
            "inferred": p.inferred,
            "check": p.check.iter().map(|c| json!({"what": c.what, "placements": c.placements})).collect::<Vec<_>>(),
            "operation_ids": p.operation_ids,
            "followed": p.followed.iter().map(|f| json!({"path": f.path, "line": f.line})).collect::<Vec<_>>(),
        });
        match self.editor.call("blueprint/present", body).await {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_wish` — the one tool that would most have helped** (#353).
    ///
    /// **After the hand-back, not instead of it.** `blueprint_present` says what was done; this says
    /// what it cost, and names the single tool that would have removed the cost. It is the one thing
    /// a person reading a finished result cannot see.
    ///
    /// **Queued in the editor and answered at once**, like `blueprint_present`: what became of the
    /// wish is read from `blueprint_state.wishes`, because a wish waits to be reviewed by a person
    /// and an MCP call that blocked on one would deadlock with the editor's frame.
    ///
    /// **One wish per hand-back.** A second citing the same presentation or note is refused, and so
    /// is one citing an id this editor never handed back.
    #[tool(
        name = "blueprint_wish",
        description = "After a blueprint_present hand-back (or a note you resolved), name the one \
                       tool that would most have helped with that task: after, the hand-back it \
                       follows ({presentation: \"present-1\"} or {note: \"n3\"}); task, what the \
                       task was; hardship, what cost steps; tool, the single tool, named and \
                       described in one sentence, inside the limit its schema states; example_call, \
                       a call to it as you would like to write it, inside its own. The editor queues the wish \
                       and the editor adds the context it already has — the hand-back's title, the \
                       operations the task applied, its version and your client's name and version \
                       — so do not restate those. It returns at once with the wish's id and \
                       \"waiting\"; read blueprint_state's wishes for what became of it. One wish per \
                       hand-back: a second one citing the same hand-back, or one citing an id this \
                       editor never handed back, is refused with the reason. An empty field, or one \
                       past its limit, is refused naming the field — a wish is never trimmed. It \
                       writes nothing outside the editor's own queue.",
        annotations(
            title = "Wish for the tool that would have helped",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn wish(
        &self,
        Parameters(p): Parameters<WishParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **Exactly one hand-back is named.** Two is a wish citing two tasks; none is a wish citing
        // nothing, and the editor cannot tell either what was handed back.
        let after = match (p.after.presentation, p.after.note) {
            (Some(id), None) => json!({"presentation": id}),
            (None, Some(id)) => json!({"note": id}),
            (Some(_), Some(_)) => {
                return Ok(refusal(
                    "a wish follows one hand-back: pass `presentation` or `note`, not both",
                ));
            }
            (None, None) => {
                return Ok(refusal(
                    "a wish follows a hand-back: pass `presentation` (an id blueprint_present \
                     returned) or `note` (an id you resolved)",
                ));
            }
        };
        match self.editor.call("blueprint/wish", json!({
            "after": after,
            "task": p.task,
            "hardship": p.hardship,
            "tool": p.tool,
            "example_call": p.example_call,
        })).await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_lesson` — one line for the project's guide, learned from the task** (AG-06, #351).
    ///
    /// **After the hand-back, like a wish, and about the work rather than the tools.** A wish names
    /// what the editor lacked; a lesson names what the *task* taught — the thing the next agent in
    /// this project should know. It lands in the project's own guide, under its Lessons heading,
    /// which is the file an agent is already given.
    ///
    /// **Queued in the editor and answered at once.** The call writes nothing: a proposal waits as a
    /// row in the AGENT block, and only a person's **Keep** appends the line. What became of it is
    /// read from `blueprint_state.lessons`, because a call that blocked on a person would deadlock
    /// with the editor's frame.
    ///
    /// **One lesson per hand-back.** A second citing the same presentation or note is refused, and so
    /// is one citing an id this editor never handed back.
    #[tool(
        name = "blueprint_lesson",
        description = "After a blueprint_present hand-back (or a note you resolved), propose one line \
                       for this project's agent guide: what the task taught that the next agent \
                       should know. Fields: after, the hand-back it follows ({presentation: \
                       \"present-1\"} or {note: \"n3\"}); lesson, the one line the guide would \
                       gain, inside the limit its schema states and with no newline; why, why it is \
                       worth the line, also one line, inside its own. The editor queues the proposal \
                       in the AGENT block and the person keeps or discards it; only their Keep \
                       writes the line to .dark_factory/AGENTS.md, so this call writes nothing. It \
                       returns at once with the lesson's id, the line it would add, and \"waiting\"; \
                       read blueprint_state's lessons for what became of it. One lesson per \
                       hand-back: a second citing the same hand-back, or one citing an id this \
                       editor never handed back, is refused with the reason, as is a newline, an \
                       empty field or one past its limit — a lesson is never trimmed. A project \
                       with no agent guide is refused, and the person creates one on the project \
                       page. A lesson is optional: one worth keeping beats one per task.",
        annotations(
            title = "Propose a lesson for the project's guide",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn lesson(
        &self,
        Parameters(p): Parameters<LessonParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **Exactly one hand-back is named**, as a wish's is: two is a lesson learned from two tasks
        // and none is a lesson with no task behind it.
        let after = match (p.after.presentation, p.after.note) {
            (Some(id), None) => json!({"presentation": id}),
            (None, Some(id)) => json!({"note": id}),
            (Some(_), Some(_)) => {
                return Ok(refusal(
                    "a lesson follows one hand-back: pass `presentation` or `note`, not both",
                ));
            }
            (None, None) => {
                return Ok(refusal(
                    "a lesson follows a hand-back: pass `presentation` (an id blueprint_present \
                     returned) or `note` (an id you resolved)",
                ));
            }
        };
        match self.editor.call("blueprint/lesson", json!({
            "after": after,
            "lesson": p.lesson,
            "why": p.why,
        })).await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_list_pieces` — the library's pieces, a page at a time.**
    ///
    /// **A read.** `blueprint_state` names the first page of ids; this is the list itself, with the
    /// mesh each piece is drawn with and its kinds, effects and looks, so an agent can choose a piece
    /// by what it is and what it looks like, not by its id alone.
    #[tool(
        name = "blueprint_list_pieces",
        description = "The library's pieces, a page at a time. Each row carries the piece's id, the \
                       mesh it is drawn with (null for a flat footprint marker), its kind, effects \
                       and looks, and its labels: who labelled each axis (model, agent or human), the \
                       record's model, date and confidence, which report what the record says, not whether anyone has checked it; by_model finds labels no human has confirmed. Filters combine: with_mesh, unlabelled, \
                       by_model, confidence (high, medium or low), id_contains. total is the true \
                       count after the filters. A page holds 50 rows; past 50, pass pieces_from \
                       (from pieces_next) to read the rest, and how_to_narrow names the filters \
                       when a filtered page is still capped. Refuses when no project is open, \
                       rather than answering an empty list. Returns that page as JSON, and changes \
                       nothing.",
        annotations(
            title = "List the library's pieces",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_pieces(
        &self,
        Parameters(p): Parameters<PiecesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call(
                "blueprint/pieces",
                json!({
                    "pieces_from": p.pieces_from,
                    "with_mesh": p.with_mesh,
                    "unlabelled": p.unlabelled,
                    "by_model": p.by_model,
                    "confidence": p.confidence,
                    "id_contains": p.id_contains,
                }),
            )
            .await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_capture_still` — a picture of the editor as it is now.**
    ///
    /// **A read of the screen, and a write of a file.** The picture comes from the editor's own
    /// surface — the world *and* the interface, because the editor draws both into one image —
    /// and the state beside it is the same summary `blueprint_state` returns, so a picture is never
    /// evidence on its own.
    ///
    /// **The file is written here and not by the editor.** The editor's own capture path writes
    /// one file in the working directory and overwrites it; **a tool that overwrote the last
    /// capture would make two runs indistinguishable.**
    #[tool(
        name = "blueprint_capture_still",
        description = "A picture of the editor as it is now -- the world and the interface, because \
                      the editor draws both into one image -- plus a state summary of the same \
                      moment. The picture is returned as an image and written to a file whose path \
                      is in the answer. The answer also carries a fingerprint of the picture's \
                      bytes: two stills with the same fingerprint are the same picture, so compare \
                      fingerprints rather than file paths to tell whether the screen changed. Use \
                      this after a change to see what it did, or before one to see what it was \
                      like. Pass source \"window\" to capture what the window itself shows (its \
                      swapchain) instead of what the editor drew; on macOS an unfocused window \
                      reads as one flat colour. Pass hide to take the map's own surfaces out of the \
                      picture for it: [\"terrain\"] cuts the ground's outer skin away, so what the \
                      picture shows is the interior's NEAR surface -- the vault and the far walls, lit \
                      from the side the camera is on rather than left as shadows -- instead of the rock \
                      they are cut into. That is the shape of the void, not the room: the FLOOR is \
                      behind the vault along every ray from above and is NOT drawn, and neither is a \
                      column's cap. To see a room's floor, add cut_m: a section plane that removes \
                      everything nearer the camera than that many metres, so the near half of the \
                      interior goes and the floor is what the picture shows. cut_m is the caller's \
                      deliberate section: it knows the value because it framed the room and knows its \
                      size. The camera does not move, a view only: no file is written, the map is not \
                      marked dirty, no operation log line is made, and what was hidden and the near \
                      plane are both put back after the picture unless restore is false. The answer's \
                      hid field says what the picture was taken with.",
        annotations(
            title = "Capture a still of the editor",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn capture_still(
        &self,
        Parameters(p): Parameters<StillParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The source first, then the label**: a refused argument is answered before the editor is
        // asked anything.
        let (source, fairy_source) = match p.source.as_deref() {
            None | Some("surface") => ("surface", "offscreen"),
            Some("window") => ("window", "window"),
            Some(_) => return Ok(refusal("source is surface or window")),
        };
        let dir = match self.capture_dir(&p.label) {
            Ok(dir) => dir,
            Err(e) => return Ok(refusal(&e)),
        };
        // **The state first**, so the picture and the state are the same moment as near as the
        // editor can make them.
        let state = self
            .editor
            .call("blueprint/state", json!({}))
            .await
            .unwrap_or(Value::Null);
        // **The view the picture is taken through** (#472): `hide` is applied before the grab, so an
        // underground map's rooms are what the picture shows rather than the rock they are cut into.
        // **The plane goes on after the hiding and comes off with it** (#472): a cut on its own shows
        // the rock in front of a room that is still buried.
        if let Err(e) = self.set_cut(p.cut_m).await {
            return Ok(refusal(&e));
        }
        let was = match self.set_hidden(p.hide.as_deref()).await {
            Ok(was) => was,
            Err(e) => return Ok(refusal(&e)),
        };
        let grabbed = self.grab(fairy_source).await;
        // **The person's screen goes back as it was**, whether the picture landed or not: a still is a
        // look, not a change to what they are looking at.
        if p.restore
            && let Err(e) = self.set_cut(None).await
        {
            return Ok(refusal(&format!("the camera's own range could not be put back: {e}")));
        }
        if p.restore && was.is_some()
            && let Err(e) = self.set_hidden(Some(&was.unwrap_or_default())).await
        {
            return Ok(refusal(&format!("the view could not be put back after the capture: {e}")));
        }
        let png = match grabbed {
            Ok(png) => png,
            Err(e) => return Ok(refusal(&e)),
        };
        // **The file, written here.** A directory per label, and a name that says when it was
        // taken, so two captures never collide.
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Ok(refusal(&format!("{}: {e}", dir.display())));
        }
        let path = dir.join(format!("still-{}.png", self.stamp()));
        let bytes = match base64_decode(&png) {
            Some(b) => b,
            None => return Ok(refusal("the editor's picture was not base64.")),
        };
        if let Err(e) = std::fs::write(&path, &bytes) {
            return Ok(refusal(&format!("{}: {e}", path.display())));
        }
        let answer = json!({
            "path": path.to_string_lossy(),
            "source": source,
            "bytes": bytes.len(),
            // **What the picture IS, so two stills can be compared without opening them (E-062).**
            //
            // A caller that compares paths, or timestamps, or file sizes cannot tell "the screen did
            // not change" from "the file did not change" — and on 2026-10-24 that cost this loop a
            // whole iteration: two captures came back with identical MD5, which was read as a stale
            // screen and was in fact a repeated file. A fingerprint of the bytes is the fact the
            // question actually needs.
            "fingerprint": dark_factory_mcp::fingerprint(&bytes),
            // **What the picture was taken with hidden** (#472), so a still that shows an interior says
            // so in its own answer and cannot be mistaken for one of the outside.
            "hid": p.hide.clone().unwrap_or_default(),
            "state": state,
        });
        Ok(CallToolResult::success(vec![
            rmcp::model::ContentBlock::image(png, "image/png"),
            rmcp::model::ContentBlock::text(
                serde_json::to_string_pretty(&answer).unwrap_or_default(),
            ),
        ]))
    }

    /// **`blueprint_snip` -- one rectangle of the picture, and nothing else.**
    ///
    /// **The same grab as a still**, so a crop and a still taken at the same moment are the same
    /// pixels. **The rectangle is checked before anything is written**, so a refusal leaves no file
    /// behind.
    #[tool(
        name = "blueprint_snip",
        description = "A rectangle of the editor's picture, cut out and returned as an image. rect \
                       is [x0, y0, x1, y1] in window pixels, origin at the top left, x1 and y1 \
                       exclusive, so the same pixels a still is in. The crop is written to a file \
                       whose path is in the answer, with the state of the same moment. It writes \
                       one crop file under target/loop and changes no map data: it is not an \
                       operation, not logged and not undoable. Refuses, and writes nothing, on a \
                       rectangle that is empty, reversed, negative or outside the picture, and says \
                       the picture's size. Use it to look closer at one part of a still from \
                       blueprint_capture_still.",
        annotations(
            title = "Snip one rectangle of the editor",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn snip(
        &self,
        Parameters(p): Parameters<SnipParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The label first**: a refused label is answered before the editor is asked anything.
        let dir = match self.capture_dir(&p.label) {
            Ok(dir) => dir,
            Err(e) => return Ok(refusal(&e)),
        };
        let state = self
            .editor
            .call("blueprint/state", json!({}))
            .await
            .unwrap_or(Value::Null);
        let png = match self.grab("offscreen").await {
            Ok(png) => png,
            Err(e) => return Ok(refusal(&e)),
        };
        let bytes = match base64_decode(&png) {
            Some(b) => b,
            None => return Ok(refusal("the editor's picture was not base64.")),
        };
        let crop = match dark_factory_snip::crop_png(&bytes, p.rect) {
            Ok(crop) => crop,
            Err(e) => return Ok(refusal(&e)),
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Ok(refusal(&format!("{}: {e}", dir.display())));
        }
        let path = dir.join(format!("snip-{}.png", self.stamp()));
        if let Err(e) = std::fs::write(&path, &crop.png) {
            return Ok(refusal(&format!("{}: {e}", path.display())));
        }
        let answer = json!({
            "path": path.to_string_lossy(),
            "bytes": crop.png.len(),
            "rect": p.rect,
            "size": crop.size,
            "picture": crop.picture,
            "state": state,
        });
        let encoded = base64_encode(&crop.png);
        Ok(CallToolResult::success(vec![
            rmcp::model::ContentBlock::image(encoded, "image/png"),
            rmcp::model::ContentBlock::text(
                serde_json::to_string_pretty(&answer).unwrap_or_default(),
            ),
        ]))
    }

    /// **`blueprint_decide` -- ask a decision model, and get a probability back.**
    ///
    /// **Advisory and authoring-time**: it changes no map data, and a finding never blocks or undoes
    /// anything. Which model answers is the person's `decision_model` setting, read from the settings layers.
    #[tool(
        name = "blueprint_decide",
        description = concat!(
                       "The fastest way to check how your work looks: one still and up to eight questions, answered ",
                       dark_factory_mcp::decide_latency!(),
                       " with a probability each. Call it after every change the person will see, before you hand ba",
                       "ck: pass view: true to take the still yourself, or give a still from blueprint_capture_still",
                       " in images (its path or its fingerprint), and ask only the questions your next operation cou",
                       "ld change. The answer names the model that answered (and model_source, reported by the endpo",
                       "int or configured), where it ran (local, or remote when pictures may have left the LAN) and ",
                       "latency_ms. Ask a decision model a typed question about the editor's pictures and facts, and",
                       " get a probability for every allowed answer, with nothing to parse. Two forms. check: a name",
                       "d check such as floating, clipping, outside_area, blank_frame or toast_visible (the editor's",
                       " own, with the project's assets/blueprint/checks.yaml over them, read on every call); it ans",
                       "wers a finding with the check's P(yes), its threshold and whether it was found, and takes th",
                       "e editor's current picture itself when it needs view. questions: ad-hoc typed questions by i",
                       "d, each noul (yes or no, P(yes)), choice (named options, each one's probability) or score (l",
                       "evels, a weighted score), with state in words, images (stills from blueprint_capture_still, ",
                       "at most four, each at most 8 MiB) and facts (JSON). A check that needs a picture or facts it",
                       " was not given is refused and names what is missing; it never answers from less. hide: the m",
                       "ap's own surfaces to take out of the picture this call takes for view, in blueprint_capture_",
                       "still's own words: [\"terrain\"] cuts a carved ground's outer skin away, so floating and cli",
                       "pping are asked about the interior's NEAR surface -- the vault and the far walls, lit from t",
                       "he side the camera is on -- instead of the hill it is buried in. That is the shape of the vo",
                       "id, not the room: the FLOOR is behind the vault along every ray from above and is NOT drawn.",
                       " Add cut_m (a section plane removing everything nearer the camera than that many metres) to ",
                       "see a room's floor, and pass the same cut_m the still you compared against used. No camera m",
                       "ove, no file written, the map not marked dirty; what was hidden and the near plane are both ",
                       "put back before the model is asked, so the person's screen is unchanged by the question. Ign",
                       "ored when no picture is taken. game: also look at the running game that play started: a fram",
                       "e from its bevy_fairy, named game, on the port in play.yaml or game.port, and one BRP call's",
                       " answer (game.state) as facts.game; a check that asks for the image game takes it itself, an",
                       "d a game's own checks live in the project's assets/blueprint/checks.yaml. Advisory: it chang",
                       "es no map data and a finding never blocks or undoes an operation. Which model answers is the",
                       " person's decision_model setting: Local (d3-flash on the LAN, the default) or Remote (Perple",
                       "xity's, through OpenRouter, which sends pictures off the LAN and is the person's own choice)",
                       ". It is one per call with no fallback: Remote with no OPENROUTER_API_KEY is refused, never a",
                       "sked of the local model.",
        ),
        annotations(
            title = "Ask a decision model",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = true
        )
    )]
    async fn decide(
        &self,
        Parameters(p): Parameters<dark_factory_mcp::decide::DecideParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let Some(project) = self.project.clone() else {
            return Ok(refusal("blueprint_decide needs --project: its checks live in the project"));
        };
        let checks = match dark_factory_mcp::decide::load_checks(&project) {
            Ok(c) => c,
            Err(e) => return Ok(refusal(&e)),
        };
        let view = if dark_factory_mcp::decide::needs_view(&p, &checks) {
            match self.grab("offscreen").await {
                Ok(png) => Some(png),
                Err(e) => return Ok(refusal(&e)),
            }
        } else {
            None
        };
        let game = if dark_factory_mcp::decide::needs_game(&p, &checks) {
            let asked = p.game.clone().unwrap_or_default();
            match dark_factory_mcp::decide::fetch_game(&project, &asked).await {
                Ok(g) => Some(g),
                Err(e) => return Ok(refusal(&e)),
            }
        } else {
            None
        };
        // **What the editor is told about this call** (DL-01, #460), taken before `p` moves into the thread that asks the model.
        let asked = p.clone();
        // **Which model this call asks**, resolved as `decide_asked` resolves it (the person's choice, one endpoint, no fallback); a choice that does not
        // resolve is refused by that call, and the report then names no model.
        let (model, endpoint) = bevy_oracle::Endpoints::from_env()
            .resolve(bevy_oracle::user_file().as_deref(), Some(&bevy_oracle::project_file(&project)))
            .map(|resolved| (resolved.config.model, resolved.config.endpoint))
            .unwrap_or_default();
        let started = std::time::Instant::now();
        let answer = tokio::task::spawn_blocking(move || {
            dark_factory_mcp::decide::decide_asked(
                &p,
                &project,
                view.as_deref(),
                game.as_ref(),
                bevy_oracle::user_file().as_deref(),
                &bevy_oracle::Endpoints::from_env(),
            )
        })
        .await
        .map_err(|e| ErrorData::internal_error(e.to_string(), None))?;
        // **The editor is the one writer of the turn ledger**, so this server reports the call and keeps nothing. It is sent on its own task:
        // an editor that is busy or not running must never delay the answer the agent is waiting for.
        let report = dark_factory_mcp::decide::report(
            &asked,
            &answer,
            model.as_deref(),
            &endpoint,
            u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        );
        let editor = self.editor.clone();
        tokio::spawn(async move {
            if let Err(why) = editor.call("blueprint/decide_report", report).await {
                eprintln!("dark_factory_mcp: could not tell the editor about a decision: {why}");
            }
        });
        Ok(match answer {
            Ok(v) => CallToolResult::structured(v),
            Err(e) => refusal(&e),
        })
    }

    /// **`blueprint_list_snips` -- the snips the person took, newest first.**
    ///
    /// **A read of files the editor wrote.** It needs no editor running and changes nothing: the
    /// records are the editor's own, read back in the shape the editor wrote them.
    #[tool(
        name = "blueprint_list_snips",
        description = "The snips the person took in the editor, newest first: each one's name, when \
                       it was taken, the map and the active tool, the note the person typed, the \
                       rectangle in window and picture pixels, the camera view, the cursor's world \
                       position and the placements inside the rectangle. Each entry names its \
                       picture file, and the newest picture comes back as an image. It reads files \
                       the editor wrote, so it needs no editor running and changes nothing. Returns \
                       how many snips there are, how many are shown, the snips still being saved \
                       (pending: confirmed in the editor, picture not yet written, so not yet in \
                       the snips list) and each .json file in the snips folder that is not a snip \
                       record or has no picture file, with the reason.",
        annotations(
            title = "List the snips the person took",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_snips(
        &self,
        Parameters(p): Parameters<ListSnipsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if !(1..=20).contains(&p.limit) {
            return Ok(refusal(&format!(
                "limit is {}, and it must be from 1 to 20.",
                p.limit
            )));
        }
        if self.project.is_none() {
            return Ok(refusal(
                "blueprint_list_snips needs --project: the snips are files the editor writes in the project",
            ));
        }
        let dir = match self.capture_dir("snips") {
            Ok(dir) => dir,
            Err(e) => return Ok(refusal(&e)),
        };
        let (records, mut not_snips) = dark_factory_snip::read_snips(&dir);
        let (open, pending_problems) = dark_factory_snip::read_pending(&dir);
        not_snips.extend(pending_problems);
        // **A snip that already has its picture is finished, not pending.** Between the picture's
        // write and the pending record's removal, one stem is in both lists.
        let open: Vec<_> = open
            .into_iter()
            .filter(|r| !records.iter().any(|done| done.stem == r.stem))
            .collect();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let pending_total = open.len();
        let pending: Vec<Value> = open
            .iter()
            .take(p.limit)
            .map(|r| {
                let age_ms = now.saturating_sub(r.taken_at_ms);
                let status = if age_ms > dark_factory_snip::PENDING_ABANDONED_AFTER_MS {
                    format!(
                        "abandoned: no picture after {} s, so it is not in snips. The editor quit or the capture was lost; take the snip again.",
                        age_ms / 1000
                    )
                } else {
                    "being saved: the picture has not landed yet, so it is not in snips".to_string()
                };
                let mut entry = serde_json::to_value(r).unwrap_or(Value::Null);
                if let Value::Object(map) = &mut entry {
                    map.insert("status".to_string(), json!(status));
                    map.insert("age_ms".to_string(), json!(age_ms));
                }
                entry
            })
            .collect();
        let mut content = Vec::new();
        if let Some(newest) = records.first() {
            let path = dir.join(format!("{}.png", newest.stem));
            match std::fs::read(&path) {
                Ok(bytes) => content.push(rmcp::model::ContentBlock::image(
                    base64_encode(&bytes),
                    "image/png",
                )),
                // **A picture that cannot be read is named, and the list still answers.**
                Err(e) => not_snips.push(format!(
                    "{}: the picture could not be read: {e}",
                    path.display()
                )),
            }
        }
        let shown: Vec<Value> = records
            .iter()
            .take(p.limit)
            .map(|r| {
                let mut entry = serde_json::to_value(r).unwrap_or(Value::Null);
                if let Value::Object(map) = &mut entry {
                    map.insert(
                        "picture_path".to_string(),
                        json!(dir.join(format!("{}.png", r.stem)).to_string_lossy()),
                    );
                }
                entry
            })
            .collect();
        let answer = json!({
            "dir": dir.to_string_lossy(),
            "total": records.len(),
            "shown": shown.len(),
            "snips": shown,
            "pending": pending,
            "pending_total": pending_total,
            "not_snips": not_snips,
        });
        content.push(rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&answer).unwrap_or_default(),
        ));
        Ok(CallToolResult::success(content))
    }

    /// **`blueprint_feedback_list` -- the reports the person filed about DarkFactory itself.**
    ///
    /// **A read of files the editor wrote**, in the per-user data folder, so it needs **no
    /// `--project`**: a report about the editor is not about the project that happened to be open
    /// when the mistake was noticed, and the Projects home — where no project is open — must be able
    /// to file one.
    #[tool(
        name = "blueprint_feedback_list",
        description = "The reports the person filed about DarkFactory itself, newest first: each \
                       one's name, when it was taken, which screen and which project were up, \
                       the window's shape, each thing that was pointed at (a node of the \
                       interface with its chain of roles and text, a dragged area, the whole \
                       screen, or a point in the map with its cell and the piece there), the \
                       relation between every two of them worked out from their rectangles \
                       (contains, overlaps, touches or apart, the overlap in square pixels, and \
                       which is drawn in front), the note the person typed, and the editor's log \
                       tail. Each entry names its whole picture and the same picture with every \
                       pick outlined and every overlap washed, and the newest picture comes back as \
                       an image. It reads files the editor wrote, so it needs no editor running and \
                       changes nothing. Returns how many reports there are, how many are shown, and \
                       each .json file in the folder that is not a report or has no picture, with \
                       the reason.",
        annotations(
            title = "List the reports the person filed about DarkFactory",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn feedback_list(
        &self,
        Parameters(p): Parameters<FeedbackListParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let dir = match blueprint_model::user_dirs::feedback_dir() {
            Some(dir) => dir,
            None => {
                return Ok(refusal(
                    "this system has no data directory, so the editor has nowhere to write a report about itself.",
                ));
            }
        };
        Ok(list_reports(&dir, p.limit))
    }

    /// **`blueprint_feedback_file_issue` -- file one capture as a GitHub issue** (#495).
    ///
    /// **The same body builder the editor's own filing uses** (`blueprint_model::feedback::issue`), so
    /// an agent that read a capture files it exactly as the editor would. It answers the body and the
    /// title, and files nothing: the copy, the commit, the push and the `gh` call are the editor's
    /// (`report_issue`), because they need the checkout and the person's own hands on the settings.
    /// An agent that wants the issue opened hands this body to `gh` itself.
    #[tool(
        name = "blueprint_feedback_file_issue",
        description = "Turn one report a person filed about the editor into the GitHub issue the \
                       loop reads: the seven sections in the order docs/loop/captures/ISSUE-FORMAT.md \
                       states them — the provenance read from the record, both pictures embedded on \
                       the repository's raw URLs, the note verbatim in a blockquote, the measured \
                       facts of every pick, then Current behaviour, Wanted and Acceptance — and the \
                       title. Give the report's stem (its folder name). Writes nothing and opens no \
                       issue: it answers the title and the body, so an agent can read it before \
                       anything is filed. A stem the folder does not hold is refused, naming it.",
        annotations(
            title = "Turn a captured report into a GitHub issue body",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn feedback_file_issue(
        &self,
        Parameters(p): Parameters<FeedbackIssueParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let dir = match blueprint_model::user_dirs::feedback_dir() {
            Some(dir) => dir,
            None => {
                return Ok(refusal(
                    "this system has no data directory, so there are no reports to file.",
                ));
            }
        };
        let (reports, problems) = blueprint_model::feedback::read_reports(&dir);
        let Some(report) = reports.into_iter().find(|r| r.stem == p.stem) else {
            return Ok(refusal(&format!(
                "`{}` is not a report in {}{}",
                p.stem,
                dir.display(),
                if problems.is_empty() { String::new() } else { format!(" (unreadable: {})", problems.join("; ")) },
            )));
        };
        Ok(CallToolResult::success(vec![rmcp::model::ContentBlock::text(
            serde_json::json!({
                "stem": report.stem,
                "title": blueprint_model::feedback::issue::title(&report),
                "body": blueprint_model::feedback::issue::body(&report),
                "repo": blueprint_model::feedback::issue::REPO,
                "branch": format!("capture/{}", report.stem),
                "pictures": [
                    blueprint_model::feedback::issue::raw_url("main", &report.stem, &blueprint_model::feedback::picks_picture_file(&report.stem)),
                    blueprint_model::feedback::issue::raw_url("main", &report.stem, &blueprint_model::feedback::picture_file(&report.stem)),
                ],
            })
            .to_string(),
        )]))
    }

    /// **`blueprint_project_feedback_list` -- the reports the person filed about the open project.**
    ///
    /// **A read of files the editor wrote inside the project**, so it needs `--project`: a project's
    /// reports are for whoever works that project, and reading another project's would be reading
    /// somebody else's notes.
    #[tool(
        name = "blueprint_project_feedback_list",
        description = "The reports the person filed about this project, newest first: each one's \
                       name, when it was taken, which screen and which project were up, the \
                       window's shape, each thing that was pointed at (a node of the interface \
                       with its chain of roles and text, a dragged area, the whole screen, or a \
                       point in the map with its cell and the piece there), the relation between \
                       every two of them worked out from their rectangles (contains, overlaps, \
                       touches or apart, the overlap in square pixels, and which is drawn in \
                       front), the note the person typed, and the editor's log tail. The files are \
                       in the project's target/feedback. Each entry names its whole picture and \
                       the same picture with every pick outlined and every overlap washed, and the \
                       newest picture comes back as an image. It reads files the editor wrote, so \
                       it needs no editor running and changes nothing. Returns how many reports \
                       there are, how many are shown, and each .json file in the folder that is not \
                       a report or has no picture, with the reason.",
        annotations(
            title = "List the reports the person filed about this project",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn project_feedback_list(
        &self,
        Parameters(p): Parameters<FeedbackListParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let Some(project) = &self.project else {
            return Ok(refusal(
                "blueprint_project_feedback_list needs --project: a project's reports are files the editor writes in the project",
            ));
        };
        let dir = project.join(blueprint_model::feedback::PROJECT_DIR);
        Ok(list_reports(&dir, p.limit))
    }

    /// **`blueprint_style` -- the style of one place, as an agent should read it** (SG-02, #401).
    ///
    /// **The editor composes the chain and holds the person's trust; this server forwards the request and
    /// attaches the pictures.** A style file is prompt text fed to an agent that has a shell, so the server
    /// reads none itself: it asks the editor (`blueprint/style`), which withholds every file the person has
    /// not allowed. Without an editor the tool refuses and says why, rather than reading the files and
    /// serving what the person never allowed.
    ///
    /// **The text comes first, then each picture the editor named, after a line holding its path and its
    /// caption**, so an agent that cannot see still has the words. The editor chose the pictures (nearest
    /// place first, at most `STYLE_IMAGES_MAX`, each at most `STYLE_IMAGE_MAX_BYTES`) and resolved each to a
    /// file inside the project; this server only reads those files. One it cannot read is named in a last
    /// block, and the answer still arrives.
    #[tool(
        name = "blueprint_style",
        description = "The style of one place, as an agent should read it before shaping that place: the \
                       game's style files first, then the region's, the level's and the area's that holds \
                       `at`, so the most specific place comes last. Each place gives its style.md and, \
                       when `topic` is asked for, its file for that topic; each file arrives under a \
                       heading naming its path. With no `map` you get the game's files alone. A style \
                       file is served only once the person has allowed that exact content (or trusted \
                       the project): one that has not been allowed is withheld and arrives as one line \
                       saying it is waiting for them, never as its text. The pictures the served text \
                       cites come back as images, each after a line with its path and its caption, at \
                       most 8 of at most 1 MiB each; one left out is named with its path, its caption \
                       and the reason. `map` must be a map of the project, `at` ([x, z] in map metres) \
                       must be inside it, and `topic` must be one the game has: anything else is refused \
                       with the reason. It changes nothing.",
        annotations(
            title = "Read the style of a place",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn style(
        &self,
        Parameters(p): Parameters<StyleParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let answer = match self
            .editor
            .call(
                "blueprint/style",
                json!({
                    "map": p.map,
                    "at": p.at,
                    "topic": p.topic,
                }),
            )
            .await
        {
            Ok(answer) => answer,
            // **The editor's own refusal says its reason and nothing is added to it**; every failure of the
            // transport begins "the editor", and is no editor to answer: the tool says why it will not read the
            // files itself.
            Err(e) if !e.starts_with("the editor") => return Ok(refusal(&e)),
            Err(e) => {
                return Ok(refusal(&format!(
                    "{e} A style file is prompt text that needs the person's trust, so only the editor, which holds what they have allowed, serves it: this server reads none itself."
                )));
            }
        };
        Ok(style_answer(&answer))
    }

    /// **`blueprint_style_propose` -- a change to a style page an agent may not write itself** (SG-07, #406).
    ///
    /// **A level's or an area's page is changed with the operation `style_edit`; the game's and a region's by this proposal.** Those pages hold the
    /// whole game together, so the change is queued in the editor as a row in the AGENT block beside the wishes and the lessons, shown as the change
    /// to the page, and only a person's **Keep** writes it. **The call returns at once**, because a call that blocked on a person would deadlock
    /// with the editor's own frame; what became of it is read from `blueprint_state.style_proposals`. The editor judges the change by the rule
    /// `style_edit` uses, so a proposal that could never apply is refused at the door, with the reason.
    #[tool(
        name = "blueprint_style_propose",
        description = "Propose a change to a style page you may not write yourself: the game's \
                       (design/style.md and the game's topic files) or a region's. Fields: path, the \
                       page relative to the project root; edits, small exact edits in order, each \
                       {find, replace} with find exactly once in the page as the edit before it left \
                       it; why, one sentence on why the page should say it. The editor queues the \
                       proposal in the AGENT block, shown as the change to the page, and the person \
                       keeps or discards it: only their Keep writes, so this call writes nothing. It \
                       returns at once with the proposal's id and \"waiting\"; read blueprint_state's \
                       style_proposals for what became of it (\"kept\" or \"discarded\"). A find that \
                       is not exactly once, a page the person has not allowed (you cannot change what \
                       you have not been shown), a result over the style limits, a second proposal \
                       for a page that has one waiting, and a why past its limit are refused with the \
                       reason. A level's or an area's page is not proposed: change it with the \
                       operation style_edit.",
        annotations(
            title = "Propose a change to a style page",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn style_propose(
        &self,
        Parameters(p): Parameters<StyleProposeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call(
                "blueprint/style_propose",
                json!({
                    "path": p.path,
                    "edits": p.edits.iter().map(|e| json!({"find": e.find, "replace": e.replace})).collect::<Vec<_>>(),
                    "why": p.why,
                }),
            )
            .await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_list_rules` -- the rules a layout is generated from, as the project states them.**
    ///
    /// **A read of a project file.** `rules.yaml` sits beside `kits.yaml` under `assets/blueprint`, and
    /// the editor's own `Project::open` reads it there; this reads the same file, with the same
    /// parser and the same library-free validation ([`blueprint_model::rules::Rules::validate_numbers`]),
    /// so it needs no editor running and changes nothing.
    ///
    /// **It does not check descriptor ids against the library**, because the server has no library:
    /// `Project::open` loads the kits and then calls `validate_against`, and this tool cannot. So a
    /// block naming a piece the project does not have is answered here and refused by the editor, and
    /// the description says so rather than implying the two agree in every case. The loop's Inspect step asks an agent to say how
    /// varied a generated layout is and which constraint shaped it, and neither question can be
    /// answered by an agent that cannot read what the rules say.
    #[tool(
        name = "blueprint_list_rules",
        description = "The block rules the project states for generating layouts, read from \
                       assets/blueprint/rules.yaml: for each block, the example map its grammar is learned from, the \
                       region descriptors that ask for a block, the cell size and block size in cells, the border \
                       piece, no_empty, the weight multipliers, the footprint rules, attempts and seed. Returns the \
                       blocks and the path of the file read. A project with no rules.yaml answers with no blocks, \
                       because an absent file means no rules -- not a broken project. A file that is present but \
                       does not parse, or states a version this program does not read or a number it cannot \
                       generate from (no attempts, a cell that is not a usable step, a footprint under 2x2, a yaw \
                       other than 0 or 180), is refused with its path and the reason, and nothing is written. It \
                       parses the file but does not check that the descriptor ids it names exist in the project's \
                       library -- the editor's own open does that, so a block naming a missing piece is answered \
                       here and refused there. It reads a file, so it needs no editor running and changes nothing.",
        annotations(
            title = "List the block rules a layout is generated from",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_rules(&self) -> Result<CallToolResult, ErrorData> {
        // **The file the editor's own `Project::open` reads**, computed the same way, so the two
        // cannot disagree about where rules live.
        let Some(project) = &self.project else {
            return Ok(refusal(
                "blueprint_list_rules needs --project: it reads the project's rules.yaml from disk",
            ));
        };
        let path = project
            .join("assets")
            .join("blueprint")
            .join(blueprint_model::rules::FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            // **An absent file is no rules, not a failure.** `Project::open` treats it the same way:
            // a project without rules behaves as it always did, and an agent that asks must not get
            // a refusal that reads like a broken project. **A file that is present and blank is a
            // different case** and falls through to the parser below, which refuses it — the editor
            // refuses to open such a project, so answering "no rules" would call it clean.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let answer = json!({
                    "path": path.to_string_lossy(),
                    "version": blueprint_model::rules::RULES_VERSION,
                    "blocks": [],
                });
                return Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(
                        serde_json::to_string_pretty(&answer).unwrap_or_default(),
                    ),
                ]));
            }
            Err(e) => {
                return Ok(refusal(&format!(
                    "{} could not be read: {e}",
                    path.display()
                )));
            }
        };
        // **A blank file is refused, not read as "no rules".** `Project::open` parses whatever is
        // there, so a blank `rules.yaml` fails the editor's open — and a reader that answered "no
        // blocks" would tell an agent the project is clean when the editor cannot start on it.
        let rules = match blueprint_model::rules::Rules::parse(&text) {
            Ok(rules) => rules,
            // **Refused by name, with the parser's reason.** An agent told "no rules" when the file
            // is broken would generate against nothing and report a result that had nothing to do
            // with what the person wrote.
            Err(e) => return Ok(refusal(&format!("{}: {e}", path.display()))),
        };
        // **And every number the editor would refuse it for.** `Rules::parse` reads the file's shape and
        // nothing else, so a file can parse and still be one `Project::open` rejects: a version this program
        // does not read, `attempts: 0`, a `cell` that is not a usable step, a footprint size under 2x2, or a
        // yaw that is not 0/180. Answering those as valid would hand an agent numbers the editor will not
        // generate from.
        if let Err(e) = rules.validate_numbers() {
            return Ok(refusal(&format!("{}: {e}", path.display())));
        }
        let answer = json!({
            "path": path.to_string_lossy(),
            "version": rules.version,
            "blocks": serde_json::to_value(&rules.blocks).unwrap_or(Value::Null),
        });
        Ok(CallToolResult::success(vec![
            rmcp::model::ContentBlock::text(
                serde_json::to_string_pretty(&answer).unwrap_or_default(),
            ),
        ]))
    }

    /// **`blueprint_capture_sequence` — a sequence of frames, with an action in the middle.**
    ///
    /// **The editor takes the frames, and that is the point.** The brief's rule is that frame 0 is
    /// the state *before* the action, and only the editor can promise that: the picture is of a
    /// rendered frame, so an action applied before frame 0 was read back would be *in* frame 0.
    /// **So the editor times the frames and applies the operation after frame 0**, and the delay is
    /// in the answer.
    #[tool(
        name = "blueprint_capture_sequence",
        description = "A sequence of frames of the editor, with an optional operation applied \
                       once the first frame is in -- so frame 0 is the state before the action and \
                       the later frames are what it did. The frames are written to a directory and \
                       their paths are in the answer. The editor takes the frames, so the timing \
                       does not depend on this server's round trips. The rate is two to fifteen frames a second. A frame is asked for only once the one before it is written, so a stalled \
                       editor delays the frames after it rather than losing them; a frame that does not land within five seconds ends the run with a reason, and the status is done only when every frame is written. The call waits for the run to end, for at most the sequence's length plus five seconds a frame and a margin.",
        annotations(
            title = "Capture a sequence of the editor",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn capture_sequence(
        &self,
        Parameters(p): Parameters<SequenceParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let dir = match self.capture_dir(&p.label) {
            Ok(dir) => dir,
            Err(e) => return Ok(refusal(&e)),
        };
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Ok(refusal(&format!("{}: {e}", dir.display())));
        }
        let operation = p
            .operation
            .as_ref()
            .map(|o| json!({"name": o.name, "parameters": o.parameters}));
        let started = match self
            .editor
            .call(
                "blueprint/capture_sequence_start",
                json!({
                    "label": p.label,
                    "out_dir": dir.to_string_lossy(),
                    "seconds": p.seconds,
                    "fps": p.fps,
                    "operation": operation,
                }),
            )
            .await
        {
            Ok(v) => v,
            Err(e) => return Ok(refusal(&e)),
        };
        let id = started.get("id").and_then(|i| i.as_u64()).unwrap_or(0);
        // **Poll for the end.** The editor answers when the frames are in; the wait covers every
        // frame, not only the sequence's length.
        let expected_frames = (p.seconds * p.fps as f32) as u32;
        let deadline = std::time::Instant::now() + sequence_wait(p.seconds, expected_frames);
        let status = match sequence::wait_for_sequence(&self.editor, id, deadline).await {
            Ok(v) => v,
            Err(e) => return Ok(refusal(&e)),
        };
        // **The frames are read back and returned as images**, so the caller sees the sequence
        // without opening the files.
        let mut content = vec![];
        if let Some(frames) = status.get("frames").and_then(|f| f.as_array()) {
            for frame in frames {
                let Some(path) = frame.get("path").and_then(|p| p.as_str()) else {
                    continue;
                };
                let Ok(bytes) = std::fs::read(path) else {
                    continue;
                };
                content.push(rmcp::model::ContentBlock::image(
                    base64_encode(&bytes),
                    "image/png",
                ));
            }
        }
        content.push(rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&status).unwrap_or_default(),
        ));
        Ok(CallToolResult::success(content))
    }

    /// **`blueprint_undo_operation` — take one change back, by its id.**
    ///
    /// **The undo is an operation like any other.** The editor looks the change up in the
    /// operation log, dispatches the inverse it recorded through the same registry the change went
    /// through, and appends a new line saying which line it undoes. **Nothing is deleted from the
    /// log.** A change that a newer change has built on is refused, with the newer change named,
    /// because putting the old values back would overwrite it.
    #[tool(
        name = "blueprint_undo_operation",
        description = "Take back one change made through blueprint_apply_operation. Pass \
                       operation_id, the id that call returned; omit it to take back the newest \
                       change that has not been taken back. The editor applies the inverse the change recorded, writes \
                       the data file, and appends a new log line naming the line it undoes; the \
                       log is never edited. Refuses, and changes nothing, when the change was \
                       already undone, when a newer change exists (the answer names it; undo that \
                       one first), when the map was changed outside the log since (a hand-edited \
                       file, the menu, or another map open; the answer names what differs), or \
                       when the change recorded no inverse. Undoing an undo puts the change back, \
                       and the change's own id takes it back again. Set dry_run to true to see what the undo would set. \
                       Set turn to true to take back the whole result you last presented with \
                       blueprint_present, newest change first, in one call — the same amount one Cmd+Z \
                       takes back — and the answer says how many changes it took with took_back. \
                       turn refuses when nothing has been presented, or when the presentation named \
                       no operations. \
                       Returns the undo's own operation id, what it set, and shown: where the \
                       inverse's change is seen, which for a put-back is where the piece stands again.",
        annotations(
            title = "Undo an editor operation",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn undo_operation(
        &self,
        Parameters(p): Parameters<UndoParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The project is checked first**, for the reason `apply_operation` gives: an undo
        // against the wrong project would put the wrong file back.
        if let Err(why) = self.check_project().await {
            return Ok(refusal(&why));
        }
        match self
            .editor
            .call(
                "blueprint/operation_undo",
                // **`turn` travels too (E-084).** The body is built by hand, so a field the
                // editor's params grew is a field this line has to name — measured live: `turn:
                // true` reached the editor as "unknown field `turn`", was refused, and the answer
                // read like a one-change undo.
                json!({"operation_id": p.operation_id, "dry_run": p.dry_run, "turn": p.turn}),
            )
            .await
        {
            Ok(v) => {
                let refused = v.get("outcome").and_then(|o| o.as_str()) == Some("refused");
                if refused {
                    Ok(CallToolResult::structured_error(v))
                } else {
                    Ok(CallToolResult::structured(v))
                }
            }
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_read_oplog` — the operation log.**
    ///
    /// **A read.** Every operation writes one line, from a person or an agent, and an undo is a
    /// new line rather than a deletion — so the log is the whole history.
    #[tool(
        name = "blueprint_read_oplog",
        description = "The editor's operation log, newest last. Every operation appends one line, \
                       whether a person or an agent did it. Read it to see what has been done to \
                       this project. A refusal is not logged, so a line here is a line about a \
                       change that happened. An undo is a new line naming the line it undoes.",
        annotations(
            title = "Read the operation log",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn read_oplog(
        &self,
        Parameters(p): Parameters<LogParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call(
                "blueprint/oplog_read",
                json!({"since_seq": p.since_seq, "limit": p.limit}),
            )
            .await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_jobs_stop` — ask a background job to stop** (UI-22, #223).
    ///
    /// **The same request the status band's popover makes.** An agent that sees a long bake in
    /// `blueprint_state.jobs` can stop it with the same flag a person's click sets, so a job is never
    /// stoppable by one front end and not the other (P1, "one command, many doors").
    #[tool(
        name = "blueprint_jobs_stop",
        description = "Ask a background job to stop, the same request the status band's popover \
                       makes: the producer stops at its own next seam, so the answer is `asked` and \
                       blueprint_state.jobs shows it go. `key` is the job's own name — `thumbs` is \
                       the palette's thumbnail bake — and blueprint_state.jobs.items lists every key \
                       that is running. Refuses, and stops nothing, on a key that is not running, \
                       naming what is. Nothing is lost when a bake is stopped: importing a piece, or \
                       editing the library at all, asks for the new pieces' portraits again.",
        annotations(
            title = "Stop a background job",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn jobs_stop(
        &self,
        Parameters(p): Parameters<JobsStopParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call("blueprint/jobs_stop", json!({"key": p.key}))
            .await
        {
            Ok(v) => {
                let refused = v.get("outcome").and_then(|o| o.as_str()) == Some("refused");
                if refused {
                    Ok(CallToolResult::structured_error(v))
                } else {
                    Ok(CallToolResult::structured(v))
                }
            }
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_frame_view` — frame the Map camera on a box.**
    ///
    /// **A camera move and nothing else.** The editor resolves the target, refuses it atomically
    /// if any name is wrong, and otherwise sets the view; the map, the operation log and the
    /// undo stack are untouched.
    #[tool(
        name = "blueprint_frame_view",
        description = "Frame the editor's Map camera on a box. target: \"map\" (the whole map), \
                       \"selection\" (the locked placement; refused when nothing is locked), \
                       {\"placements\": [ids]} (their union), or {\"rect\": [x0, z0, x1, z1]} (map \
                       metres). It only moves the camera: it never changes map data, is not logged \
                       or undoable, and is not an operation. Refuses, and moves nothing, on an \
                       unknown id, an empty selection or a flat rectangle. Returns what was framed, \
                       the focus and the view height in metres. \
                       Call blueprint_capture_still afterwards to see the result.",
        annotations(
            title = "Frame the editor's view",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn frame_view(
        &self,
        Parameters(p): Parameters<FrameParams>,
    ) -> Result<CallToolResult, ErrorData> {
        match self
            .editor
            .call("blueprint/frame_view", json!({"target": p.target}))
            .await
        {
            Ok(v) => {
                let refused = v.get("outcome").and_then(|o| o.as_str()) == Some("refused");
                if refused {
                    Ok(CallToolResult::structured_error(v))
                } else {
                    Ok(CallToolResult::structured(v))
                }
            }
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_run_command` — run one of the editor's registered commands by id** (UI-90, #256).
    ///
    /// **The same command a key, a menu or the palette runs.** The editor presses the command's
    /// chord and its one dispatcher answers, so `edit.undo` undoes as Cmd+Z does, and a command the
    /// key would refuse in this state is refused here with the editor's own reason.
    #[tool(
        name = "blueprint_run_command",
        description = "Run one of the editor's registered commands by id, exactly as its keyboard \
                       shortcut would: `edit.undo` takes back what Cmd+Z takes back, `edit.redo` \
                       puts it back. An id is `<category>.<verb>`. Refuses, and changes nothing, \
                       when the command is not enabled now (the answer says why: the tab, what is \
                       in hand, or a text field holding the keys), when the launcher is open, or \
                       when the id is unknown (the answer names the nearest ids as did_you_mean). \
                       `ran` means the shortcut was queued; the change lands within a frame or two, \
                       so read blueprint_state for the result. blueprint_state.ui says which screen, \
                       workspace and panels are open and which region has the keys. A command \
                       that is also an operation (a placement, a resize) is better done with \
                       blueprint_apply_operation, which names its effect and logs it.",
        annotations(
            title = "Run an editor command",
            read_only_hint = false,
            destructive_hint = true,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn run_command(
        &self,
        Parameters(p): Parameters<RunCommandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // **The project is checked first**: a shortcut pressed in the wrong project's editor would
        // change the wrong map.
        if let Err(why) = self.check_project().await {
            return Ok(refusal(&why));
        }
        match self.editor.call("blueprint/run_command", json!({"id": p.id})).await {
            Ok(v) => {
                let refused = v.get("outcome").and_then(|o| o.as_str()) == Some("refused");
                if refused {
                    Ok(CallToolResult::structured_error(v))
                } else {
                    Ok(CallToolResult::structured(v))
                }
            }
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_read_log` — the editor's own log, read from the file it writes** (#306).
    ///
    /// **Read from disk, never asked of the editor**, so it answers when the editor is hung, dead or
    /// was never reachable: the log is how a black window or a crash on a machine with no terminal
    /// says what happened. The path is `blueprint_model::user_dirs::log_file`, the one the editor's
    /// `log_file` module writes.
    #[tool(
        name = "blueprint_read_log",
        description = "The running editor's own log, read from the file it writes on this machine: \
                       startup stages, the window self-check, warnings, errors and panics. Works even \
                       when the editor is not answering. Pass lines (default 200, at most 2000) for \
                       the newest lines.",
        annotations(
            title = "Read the editor's log",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn read_log(
        &self,
        Parameters(p): Parameters<EditorLogParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let Some(path) = blueprint_model::user_dirs::log_file() else {
            return Ok(refusal(
                "this system has no data directory, so the editor keeps no log on it",
            ));
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(refusal(&format!(
                    "no editor log at {}: this DarkFactory has not run as this user since it began \
                     logging (0.2.2)",
                    path.display()
                )));
            }
            Err(e) => return Ok(refusal(&format!("{}: {e}", path.display()))),
        };
        let text = String::from_utf8_lossy(&bytes);
        let all: Vec<&str> = text.lines().collect();
        let want = p.lines.unwrap_or(LOG_LINES_DEFAULT).clamp(1, LOG_LINES_MAX) as usize;
        let newest = &all[all.len().saturating_sub(want)..];
        let mut previous = path.as_os_str().to_owned();
        previous.push(".1");
        let previous = PathBuf::from(previous);
        Ok(CallToolResult::structured(json!({
            "path": path.to_string_lossy(),
            "lines": newest,
            "total_lines": all.len(),
            "previous": previous.exists().then(|| previous.to_string_lossy().into_owned()),
        })))
    }

    /// **`blueprint_open_project` — open a project in the running editor, from any screen** (#306).
    ///
    /// The editor checks the folder and opens it the way its command line and launcher do; the
    /// open itself runs on its next frame. A server started with `--project` opens only that one,
    /// because it would refuse every change made in another.
    #[tool(
        name = "blueprint_open_project",
        description = "Open a project folder in the running editor, from any screen including the \
                       Projects home, optionally on a named map. The open runs on the editor's next \
                       frame: call blueprint_state afterwards to see it.",
        annotations(
            title = "Open a project in the editor",
            read_only_hint = false,
            destructive_hint = false,
            idempotent_hint = false,
            open_world_hint = false
        )
    )]
    async fn open_project(
        &self,
        Parameters(p): Parameters<OpenProjectParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if let Some(told) = &self.project {
            let told = told.canonicalize().unwrap_or_else(|_| told.clone());
            let asked = PathBuf::from(&p.root);
            let asked = asked.canonicalize().unwrap_or(asked);
            if asked != told {
                return Ok(refusal(&format!(
                    "this server was started with --project `{}`, so it opens only that project, \
                     and `{}` is another one. Start it without --project to follow the editor.",
                    told.display(),
                    asked.display()
                )));
            }
        }
        match self
            .editor
            .call("blueprint/project_open", json!({"root": p.root, "map": p.map}))
            .await
        {
            Ok(v) => Ok(CallToolResult::structured(v)),
            Err(e) => Ok(refusal(&e)),
        }
    }

    /// **`blueprint_self_test` — DarkFactory's power-on self test.**
    ///
    /// An agent that DarkFactory's terminal started is asked to call this once. It reads the state,
    /// counts the operations and takes one picture, then tells the editor how each went so the
    /// terminal's self-test card can show it. **A failed check is part of the answer, not a
    /// refusal**: the agent then says what broke, and the card already shows it.
    #[tool(
        name = "blueprint_self_test",
        description = "DarkFactory's power-on self test, for an agent started in DarkFactory's terminal: \
                       reads the editor's state, counts its operations, takes one picture of the \
                       editor and asks the decision model one text-only question (which model \
                       answered, where it ran and how long it took, or why it did not answer), tells \
                       the editor how each went (it shows them to the person), and \
                       returns the picture and the results. Call it once when asked to run the self \
                       test; it changes nothing.",
        annotations(
            title = "Run DarkFactory's power-on self test",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn self_test(
        &self,
        Parameters(_): Parameters<SelfTestParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let state = match self.editor.call("blueprint/state", json!({})).await {
            Ok(answer) => (
                true,
                format!(
                    "the editor is on {}",
                    answer["screen"].as_str().unwrap_or("a screen it did not name")
                ),
            ),
            Err(e) => (false, e),
        };
        let operations = match self.editor.call("blueprint/operation_list", json!({})).await {
            Ok(answer) => match answer["operations"].as_array() {
                Some(list) => (true, format!("{} operations", list.len())),
                None => (false, "the editor's operation list carries no operations".to_owned()),
            },
            Err(e) => (false, e),
        };
        let mut png = None;
        let picture = match self.grab("offscreen").await {
            Ok(encoded) => {
                let size = base64_decode(&encoded).as_deref().and_then(png_size);
                match size {
                    Some((w, h)) => {
                        png = Some(encoded);
                        (true, format!("a {w}x{h} picture"))
                    }
                    None => (false, "the editor's picture is not a PNG".to_owned()),
                }
            }
            Err(e) => (false, e),
        };
        // **The decision model, asked once** (DM-03, #454): the one setting's model answers one text-only question, so an
        // agent learns at start-up whether its fastest check is available, which model it is and where it runs.
        let project = self.project.clone();
        let decision = tokio::task::spawn_blocking(move || {
            dark_factory_mcp::decide::self_test_check(
                project.as_deref(),
                bevy_oracle::user_file().as_deref(),
                &bevy_oracle::Endpoints::from_env(),
            )
        })
        .await
        .unwrap_or_else(|e| json!({"name": dark_factory_mcp::decide::SELF_TEST_CHECK, "ok": false, "detail": e.to_string()}));
        let checks = vec![
            self_test_check("state", state),
            self_test_check("operations", operations),
            self_test_check("picture", picture),
            decision,
        ];
        let told_the_editor = self
            .editor
            .call(
                "blueprint/self_test_result",
                json!({"token": agent_token(), "checks": checks}),
            )
            .await
            .is_ok();
        let answer = json!({
            "checks": checks,
            "told_the_editor": told_the_editor,
            "next": "Say in one short paragraph what you see in the picture and that you are ready.",
        });
        let mut blocks = Vec::new();
        if let Some(png) = png {
            blocks.push(rmcp::model::ContentBlock::image(png, "image/png"));
        }
        blocks.push(rmcp::model::ContentBlock::text(
            serde_json::to_string_pretty(&answer).unwrap_or_default(),
        ));
        Ok(CallToolResult::success(blocks))
    }
}

/// **A refusal, as a tool result an agent can read.**
///
/// **`isError: true`, and the reason in both places** — the text content and the structured
/// content. An agent reads one and a human reads the other, and both need the sentence.
fn refusal(why: &str) -> CallToolResult {
    CallToolResult::structured_error(json!({"refused": why}))
}

/// **Is this a capture label that can be a path under `captures`?** Letters, digits, `_` and `-`
/// in parts, with `/` between them. No part is empty, so there is no leading or trailing `/` and
/// no `//`; no part is `.` or `..`, because `.` is not a character a label may use. A `\` or an
/// absolute path fails for the same reason.
/// **The default of a parameter that is `true` unless it is said.** `Option<bool>`'s default is `None`, which
/// would read as `false` for `capture_still`'s `restore`: the person's view would stay changed after a still.
fn yes() -> bool {
    true
}

fn is_capture_label(label: &str) -> bool {
    !label.is_empty()
        && label.split('/').all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

/// **The token DarkFactory's terminal gave this server's agent**, from `DARK_FACTORY_AGENT_TOKEN`, or
/// `None` for a server nobody started from a terminal tab. The editor matches it to the tab whose
/// self-test card it belongs to.
fn agent_token() -> Option<String> {
    std::env::var("DARK_FACTORY_AGENT_TOKEN").ok().filter(|t| !t.is_empty())
}

/// **One self-test check, as the editor and the agent read it.**
fn self_test_check(name: &str, (ok, detail): (bool, String)) -> Value {
    json!({"name": name, "ok": ok, "detail": detail})
}

/// **A PNG's width and height**, from its IHDR chunk: the signature (8 bytes), the chunk's length
/// and type (8), then the width and the height as big-endian u32s. `None` for anything else.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || !bytes.starts_with(SIGNATURE) || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let word = |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Some((word(16), word(20)))
}

#[tool_handler]
impl rmcp::ServerHandler for Blueprint {
    /// **`tools/list` serves everything this server has, less the tools `--hide-tools` names.**
    ///
    /// **A hidden tool is absent, not refused.** An arm that hides `blueprint_frame_view` asks what an
    /// agent does without that tool; a tool that is listed and refuses every call answers a different
    /// question, because the agent reads the name, spends a call on it, and learns only then that it
    /// cannot be used. The generated handler lists the whole router, so this is the one call it needs
    /// replacing for.
    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<rmcp::model::ListToolsResult, rmcp::ErrorData> {
        let tools = self
            .tool_router
            .list_all()
            .into_iter()
            .filter(|t| self.serves(t.name.as_ref()))
            .collect();
        Ok(rmcp::model::ListToolsResult::with_all_items(tools))
    }

    /// **A tool that does not exist is refused with the list of the ones that do.**
    ///
    /// **The generated handler answers "tool not found"**, which tells an agent that it guessed
    /// wrong and nothing else — and an agent that has to guess again is an agent that guesses
    /// again. The brief's rule is *"Invalid calls are rejected atomically, with a message saying
    /// what was wrong and what to change."* **What to change is a list of names.**
    async fn call_tool(
        &self,
        request: rmcp::model::CallToolRequestParams,
        context: rmcp::service::RequestContext<rmcp::service::RoleServer>,
    ) -> Result<rmcp::model::CallToolResponse, rmcp::ErrorData> {
        let name = request.name.as_ref();
        if !self.serves(name) {
            let names: Vec<String> = self
                .tool_router
                .list_all()
                .iter()
                .map(|t| t.name.as_ref().to_owned())
                .filter(|n| self.serves(n))
                .collect();
            let result = CallToolResult::structured_error(json!({
                "refused": format!(
                    "`{name}` is not a tool this server has. The tools are: {}.",
                    names.join(", ")
                ),
            }));
            return Ok(result.into());
        }
        // **And the router, for a tool that does exist.** The `#[tool_handler]` macro generates
        // this call when the impl block does not carry its own — and this impl block does, so
        // the router is called here.
        let tcc = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
        self.tool_router.call(tcc).await
    }

    /// **Tell the editor an agent connected**, for the self-test card of the terminal tab that
    /// started it: the token that tab gave, the client's name and version, and how many tools it is
    /// offered. **A failed call never fails the session**: the server starts with no editor present,
    /// so this is said on stderr and the session goes on.
    async fn on_initialized(
        &self,
        context: rmcp::service::NotificationContext<rmcp::service::RoleServer>,
    ) {
        let client = context.peer.peer_info().map(|p| {
            json!({"name": p.client_info.name, "version": p.client_info.version})
        });
        let hello = json!({
            "token": agent_token(),
            "client": client,
            "tools": self.tool_router.list_all().len(),
        });
        if let Err(e) = self.editor.call("blueprint/agent_hello", hello).await {
            eprintln!("dark_factory_mcp: could not tell the editor an agent connected: {e}");
        }
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "This server drives the bevy_blueprint world editor. Call blueprint_guide first: it says \
             which tools come in what order, what a person expects back, and what to do when they \
             pause your writes. Then blueprint_list_operations to learn the operations and their \
             parameters, blueprint_preview_operation to check a change, and blueprint_apply_operation \
             to make it. Every applied operation is recorded in the editor's operation log. After \
             every change the person will see, take a still and check it with blueprint_decide: the \
             guide says how.",
        )
    }
}

/// **Serve on stdio, until the client goes away.**
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut project: Option<PathBuf> = None;
    let mut hidden: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--project" => project = args.next().map(PathBuf::from),
            // **A tool the server does not serve at all** (BM-02, #412), so an agent's tool list does not
            // hold it. Comma-separated, and each name is checked against the router at start: a name this
            // build has no tool for is refused, because an arm that silently hid nothing would measure the
            // full tool set while its report said one was missing.
            "--hide-tools" => {
                let given = args.next().unwrap_or_default();
                hidden.extend(given.split(',').map(str::trim).filter(|n| !n.is_empty()).map(str::to_owned));
            }
            "--emit-docs" => {
                // **The document is written to stdout, and the shell redirects it.** The server
                // never writes a file in the project — it does not own the project directory —
                // so the one write it can do is to a file the caller names.
                print!("{}", docs::render());
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "dark_factory_mcp — the bevy_blueprint editor over the Model Context Protocol.\n\n\
                     USAGE:\n    dark_factory_mcp [--project <dir>] [--hide-tools <a,b>]\n\n\
                     --project <dir>   the project the editor is working on; the server refuses a \
                     change made while the editor has another one open. Without it the server \
                     follows whatever the editor has open, including none (the Projects home).\n\
                     --hide-tools <a,b> tool names that are not served at all, so an agent's tool list \
                     does not hold them: a benchmark arm, not a permission.\n\n\
                     The editor's BRP endpoint comes from BEVY_BRP_PORT (default 15702), which is \
                     the same variable the editor reads."
                );
                return Ok(());
            }
            other => {
                eprintln!("dark_factory_mcp: `{other}` is not an argument this server takes.");
                eprintln!("Try `--help`.");
                std::process::exit(2);
            }
        }
    }
    // **A hidden name this build has no tool for is refused at start**, naming it and the tool that
    // would have matched: an arm that hid nothing would measure the whole tool set while its report said
    // one was missing, and the run would look like a result about a tool it never removed.
    let known = Blueprint::new(Brp::from_env(), project.clone()).tool_names();
    if let Some(unknown) = hidden.iter().find(|h| !known.iter().any(|k| k == *h)) {
        eprintln!(
            "dark_factory_mcp: `{unknown}` is in --hide-tools but this server has no tool by that name."
        );
        eprintln!("The tools are: {}.", known.join(", "));
        std::process::exit(2);
    }
    let server = Blueprint::with_hidden(Brp::from_env(), project, hidden);
    // **The project is checked when a tool is called, not when the server starts.**
    //
    // **A server that refuses to start is a server an agent cannot ask anything.** The editor may
    // be closed when the server starts and opened later — or the server may be started first on
    // purpose — and a server that exits because nothing is listening has told the agent
    // nothing except that the editor was not up. **So the check is per call**, and a call with
    // no editor answers with a sentence.
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

/// **Decode base64 into bytes.**
///
/// **Only the standard alphabet, and only the padded form** — which is what the editor's
/// capture path produces and what `base64::engine` writes.
fn base64_decode(text: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(text).ok()
}

/// **Encode bytes into base64.**
fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// **The landing timeout, per frame.** A copy of `CAPTURE_TIMEOUT_S` in `bevy_dark_factory`'s
/// `sequencer.rs`: the editor fails a run whose frame does not land within it.
const LANDING_TIMEOUT_S: f32 = 5.0;

/// **How long the caller waits for a sequence to finish.** The editor asks for at most one new frame
/// per update, up to four in flight, and fails the run on a frame that misses the landing timeout. A
/// run therefore takes at most one landing wait per frame, which still bounds it with frames overlapping.
/// The sequence's own length and a margin for the last readback are added on top.
fn sequence_wait(seconds: f32, expected_frames: u32) -> std::time::Duration {
    std::time::Duration::from_secs_f32(seconds + 10.0 + expected_frames as f32 * LANDING_TIMEOUT_S)
}

#[cfg(test)]
mod pieces_params_tests;

#[cfg(test)]
mod capture_label_tests {
    use super::is_capture_label;

    #[test]
    fn a_label_under_the_captures_directory_is_accepted() {
        for label in ["phase0-base", "town/house_cottage_plaster", "snip_check"] {
            assert!(
                is_capture_label(label),
                "`{label}` should be a capture label"
            );
        }
    }

    #[test]
    fn a_label_that_could_leave_the_directory_is_refused() {
        for label in [
            "",
            "../escape",
            "a/../b",
            "/tmp/escape",
            "a//b",
            "a/",
            "bad.label",
            "a\\b",
        ] {
            assert!(!is_capture_label(label), "`{label}` should be refused");
        }
    }
}

#[cfg(test)]
mod sequence_wait_tests {
    use super::sequence_wait;
    use std::time::Duration;

    /// **A slow run must not outlast its own wait.** Ten seconds at fifteen frames a second is
    /// 150 frames, and each may take up to a landing timeout of five seconds. The wait used to be the
    /// sequence's length plus ten seconds, 20 s here: a run still going at that point came back as
    /// "running" with no pictures, and the editor refused a new sequence until the run ended.
    #[test]
    fn the_wait_covers_every_frame_of_the_longest_run() {
        let wait = sequence_wait(10.0, 150);
        assert!(
            wait >= Duration::from_secs(150 * 5),
            "a 150-frame run can take a five-second landing per frame, but the caller waits only {wait:?}"
        );
        assert!(
            sequence_wait(10.0, 150) > sequence_wait(10.0, 40),
            "the wait must grow with the frames it has to cover"
        );
    }
}
