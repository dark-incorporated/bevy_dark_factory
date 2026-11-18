//! **`blueprint_decide`'s logic, apart from the transport and the editor**, so a test drives it with a
//! recorded response and no model, no editor and no network.
//!
//! Two forms: a **named check** from `checks.yaml` (the editor's built-ins, merged with the project's own
//! `assets/blueprint/checks.yaml`, read on every call so an edit takes effect at once), and **ad-hoc typed
//! questions**. Images are stills `blueprint_capture_still` wrote under the project; the one picture the server can take itself is `view`,
//! the editor's current picture. A third, `game`, is a frame of the **running game** taken through its own
//! `bevy_fairy` (the port in the project's `play.yaml`), with one BRP call's answer as the facts, so the same
//! checks and questions work on the game as on the editor (#114).

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bevy_oracle::{Check, Checks, Error, Evidence, Question, Request, Response, Transport};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The name of the picture the server takes itself.
pub const VIEW: &str = "view";

/// The name of the picture of the running game, taken through its `bevy_fairy`.
pub const GAME: &str = "game";

/// **The parameters of `blueprint_decide`.**
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DecideParams {
    /// **A named check**, such as `floating`. Give this or `questions`, not both.
    #[serde(default)]
    pub check: Option<String>,
    /// **Ad-hoc typed questions**, by id: `{"type": "noul", "instructions": "..."}`, `{"type": "choice",
    /// "instructions": "...", "criteria": {"option": "what it means"}}` or `{"type": "score", "instructions":
    /// "...", "criteria": ["lowest level", "...", "highest level"]}`. One to 64.
    #[serde(default)]
    pub questions: Option<BTreeMap<String, Value>>,
    /// **What the ad-hoc questions are about**, in words.
    #[serde(default)]
    pub state: Option<String>,
    /// **Pictures, stills from `blueprint_capture_still`**, each with the name a check asks for (`before`,
    /// `after`, `closeup`, ...). Ad-hoc questions send them in this order. At most four. A still is given by
    /// its `path` or its `fingerprint` (see [`NamedImage::path`]), so a before-and-after pair is asked about
    /// without a second capture.
    #[serde(default)]
    pub images: Vec<NamedImage>,
    /// **Whether to take the editor's current picture too**, named `view`. A named check that needs `view`
    /// and is not given it takes it anyway.
    #[serde(default)]
    pub view: bool,
    /// **The map's own surfaces to hide for the picture this call takes** (#472): any of `terrain`,
    /// `volume`, `water`, in `grimoire_capture_still`'s own words.
    ///
    /// **The same parameter, the same names, the same default**, because it is the same view: the server
    /// hides these before the still and puts the previous view back after it, through the one
    /// `grab`/`set_hidden` pair `grimoire_capture_still` uses. `["terrain"]` cuts a carved map's solid, so
    /// the picture `floating` and `clipping` are asked about is a room rather than the hill it is buried in
    /// — which is the whole of this parameter's reason for existing: those checks were blind underground.
    ///
    /// **Ignored when no picture is taken** (`view: false` and no named check asking for `view`), because
    /// there is then nothing to hide anything from, and asking the editor would change a view nobody asked
    /// about. A name the editor does not draw is refused by the editor, in its own words.
    #[serde(default)]
    pub hide: Option<Vec<String>>,
    /// **A section plane for the picture this call takes** (#472): everything nearer the camera than this many
    /// metres is removed, so a room's floor is what the picture shows.
    ///
    /// **`hide` is not enough for a floor**, and saying so is the point: hiding the terrain draws the interior's
    /// near surface — the vault and the far walls — because front-face culling keeps the faces pointing away
    /// from the camera, and the floor is behind the vault along every ray from above. A plane removes geometry
    /// instead of choosing faces, and how much to remove is where the room's near wall stands, which the caller
    /// knows because it framed the room.
    ///
    /// **It is the still's own parameter**, in the still's own words, placed and put back through the same one
    /// path. Naming it without `hide` is refused: removing the air in front of a room that is still buried
    /// shows rock, so the two are one request.
    #[serde(default)]
    pub cut_m: Option<f32>,
    /// **The JSON facts**: the operations run, the pieces with names and positions, the camera.
    #[serde(default)]
    pub facts: Option<Value>,
    /// **Look at the running game, not only the editor** (#114): a frame from the game's `bevy_fairy`, named
    /// `game`, and optionally one BRP call's answer as `facts.game`. A named check that asks for the image
    /// `game` gets it without this being set.
    #[serde(default)]
    pub game: Option<GameParams>,
}

/// **Where the running game is, and what of its state to read.**
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GameParams {
    /// The port the game's `bevy_fairy` answers on. Default: `port` in the project's `play.yaml`, else 15702.
    #[serde(default)]
    pub port: Option<u16>,
    /// One BRP call to the game, whose answer becomes `facts.game`: `{"method": "world.query", "params": ...}`.
    #[serde(default)]
    pub state: Option<StateCall>,
}

/// **One BRP method and its parameters**, asked of the game.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StateCall {
    /// The BRP method, such as `world.query` or `bevy_fairy/status`.
    pub method: String,
    /// Its parameters, if it takes any.
    #[serde(default)]
    pub params: Option<Value>,
}

/// **What was read from the running game**: its frame and, when asked, its state.
#[derive(Debug, Clone)]
pub struct GameEvidence {
    /// The port it answered on.
    pub port: u16,
    /// The frame, as base64 PNG.
    pub png: String,
    /// The answer to [`GameParams::state`], when one was asked.
    pub state: Option<Value>,
}

/// **Whether the server must read the running game before deciding**: it was asked for, or a named check
/// asks for the image `game`.
pub fn needs_game(params: &DecideParams, checks: &Checks) -> bool {
    if params.images.iter().any(|i| i.name == GAME) {
        return false;
    }
    params.game.is_some()
        || params.check.as_deref().and_then(|n| checks.get(n)).is_some_and(|c| c.images.iter().any(|i| i == GAME))
}

/// **The port the game answers on**: the one asked for, else the project's `play.yaml`, else fairy's default.
/// A `play.yaml` that exists and does not read is an error, not a silent default.
pub fn game_port(project: &Path, asked: Option<u16>) -> Result<u16, String> {
    if let Some(p) = asked {
        return Ok(p);
    }
    if blueprint_model::play::path_for_project(project).exists() {
        return Ok(blueprint_model::play::PlayConfig::load(project)?.port);
    }
    Ok(blueprint_model::play::DEFAULT_PORT)
}

/// **Read the running game**: a frame through its `bevy_fairy`, and its state when asked. An error names the
/// game and its port, never the editor.
pub async fn fetch_game(project: &Path, game: &GameParams) -> Result<GameEvidence, String> {
    let port = game_port(project, game.port)?;
    let brp = crate::brp::Brp::new(format!("http://127.0.0.1:{port}/"));
    let said = |e: String| format!("the running game on port {port} (play.yaml's port, or `game.port`): {e}");
    let png = brp.screenshot(json!({})).await.map_err(said)?;
    let state = match &game.state {
        Some(s) => Some(brp.call(&s.method, s.params.clone().unwrap_or(Value::Null)).await.map_err(said)?),
        None => None,
    };
    Ok(GameEvidence { port, png, state })
}

/// **Take one picture of the editor through a view `hide` names, and put the view back after it** (#472).
///
/// **The one path `grimoire_capture_still` and `grimoire_decide` both take**, so a still and the picture a
/// check is asked about are the same picture taken the same way. A second implementation of hiding would be
/// a second answer to what "the terrain hidden" looks like, and the two would drift the first time one
/// changed.
///
/// `hide` is `grimoire_capture_still`'s own parameter — `["terrain"]`, `["volume"]`, `["water"]`, or a
/// combination — and `None` asks the editor nothing at all, so a call that names none costs no round trip and
/// cannot move the view. **A name the editor does not draw is refused by the editor**, in its own words and
/// naming the list that would have worked, so both tools refuse a typo identically.
///
/// **A refused picture still puts the view back.** The editor is the person's screen, and a capture that
/// failed is not a reason to leave their map cut open; the restore is attempted before the error is
/// returned, and an error putting it back is the one reported, because that is the failure that touches the
/// person rather than the agent.
pub async fn grab_through(
    brp: &crate::brp::Brp,
    source: &str,
    hide: Option<&[String]>,
    cut_m: Option<f32>,
    restore: bool,
) -> Result<(String, Option<Vec<String>>), String> {
    // **The section plane is placed after the hiding and before the grab**, so the picture is taken through
    // both; and it is placed only when named, so a call with neither costs the editor no round trip.
    let cut_was = match cut_m {
        None => None,
        Some(metres) => {
            let answer = brp.call("grimoire/section_cut", json!({"cut_m": metres})).await?;
            Some(answer.get("was").and_then(Value::as_f64).map(|w| w as f32))
        }
    };
    // **A refused hiding still puts the plane back.** The plane is placed first (it is what makes the interior
    // visible), so a `surface_hide` the editor refuses would otherwise leave the person's map cut open with the
    // surfaces as they were — the one state neither the caller nor the person asked for.
    let was = match hide {
        None => None,
        Some(names) => match brp.call("grimoire/surface_hide", json!({"hide": names})).await {
            Ok(answer) => Some(
                answer
                    .get("was")
                    .and_then(Value::as_array)
                    .map(|w| w.iter().filter_map(|s| s.as_str().map(str::to_owned)).collect())
                    .unwrap_or_default(),
            ),
            Err(e) => {
                if restore && cut_m.is_some() {
                    let _ = brp.call("grimoire/section_cut", json!({"cut_m": cut_was.flatten()})).await;
                }
                return Err(e);
            }
        },
    };
    let grabbed = brp.screenshot(json!({"source": source})).await;
    // **Both halves go back, and the plane goes back even when the picture did not land**: a cut that
    // outlived a refused capture would leave the person looking at the inside of a hill.
    if restore {
        let mut lost = Vec::new();
        if was.is_some()
            && let Some(back) = &was
            && let Err(e) = brp.call("grimoire/surface_hide", json!({"hide": back})).await
        {
            lost.push(format!("the surfaces: {e}"));
        }
        if cut_m.is_some()
            && let Err(e) = brp.call("grimoire/section_cut", json!({"cut_m": cut_was.flatten()})).await
        {
            lost.push(format!("the section plane: {e}"));
        }
        if !lost.is_empty() {
            return Err(format!("the view could not be put back after the capture: {}.", lost.join("; ")));
        }
    }
    Ok((grabbed?, was))
}

/// **A picture on disk, and what it is called.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NamedImage {
    /// The name a check asks for.
    pub name: String,
    /// **A still from `blueprint_capture_still`**: its `path` (relative to the project, or absolute) or **the
    /// fingerprint** (sixteen lowercase hex digits) it returned, which names the newest still with those
    /// bytes. Only files under the project's `target/loop/captures` are sent, each at most
    /// [`IMAGE_BYTES_MAX`] bytes; any other file is refused.
    pub path: String,
}

/// The editor's checks with the project's own over them.
pub fn load_checks(project: &Path) -> Result<Checks, String> {
    Checks::for_project(project)
}

/// **What the editor is told about one `blueprint_decide` call** (DL-01, #460), for the turn ledger's `decisions`: the call's parameters (its
/// questions, its check, its facts; pictures by name only), what came back or why it did not, the model, where it ran and how long it took.
///
/// **Pure and complete**: the editor files it as it is, so nothing the ledger needs is read from anywhere else.
pub fn report(params: &DecideParams, answer: &Result<Value, String>, model: Option<&str>, endpoint: &str, latency_ms: u64) -> Value {
    let (answer, error) = match answer {
        Ok(v) => (Some(v), None),
        Err(e) => (None, Some(capped(&scrub(e, endpoint)))),
    };
    json!({
        "params": params,
        "answer": answer,
        "error": error,
        "model": model,
        "endpoint": public_endpoint(endpoint),
        "latency_ms": latency_ms,
    })
}

/// **The longest `error` text a report carries**, in characters (review of #460): an error can quote a response body, and the ledger keeps what it is
/// told for good.
pub const ERROR_CAP_CHARS: usize = 500;

/// **`text` cut to [`ERROR_CAP_CHARS`] characters**, saying that it was cut.
fn capped(text: &str) -> String {
    match text.char_indices().nth(ERROR_CAP_CHARS) {
        Some((at, _)) => format!("{}... (cut at {ERROR_CAP_CHARS} characters)", &text[..at]),
        None => text.to_owned(),
    }
}

/// **`text` with every address in it made safe to keep** (review of #460): `bevy_oracle::Error`'s own words name the endpoint as it was asked, key and
/// all, so the endpoint is replaced by its public form, and so is any other address a response body quotes.
pub fn scrub(text: &str, endpoint: &str) -> String {
    let text = if endpoint.is_empty() { text.to_owned() } else { text.replace(endpoint, &public_endpoint(endpoint)) };
    text.split_inclusive(char::is_whitespace)
        .map(|piece| {
            let word = piece.trim_end();
            let spaces = &piece[word.len()..];
            let Some(at) = word.find("://") else { return piece.to_owned() };
            let core = word.trim_end_matches(['"', '\'', ')', ',', '.', ';']);
            let tail = &word[core.len()..];
            let start = word[..at].rfind(|c: char| !c.is_ascii_alphabetic()).map_or(0, |p| p + 1);
            format!("{}{}{tail}{spaces}", &word[..start], public_endpoint(&core[start..]))
        })
        .collect()
}

/// **An endpoint as it is safe to keep** (review of #460): the scheme, host and path, with any `user:password@` and any `?query` or `#fragment`
/// dropped, because a key can travel in either and the ledger is kept for good.
pub fn public_endpoint(url: &str) -> String {
    let (scheme, rest) = match url.split_once("://") {
        Some((scheme, rest)) => (format!("{scheme}://"), rest),
        None => (String::new(), url),
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    let host = authority.rsplit_once('@').map_or(authority, |(_, host)| host);
    let path = tail.split(['?', '#']).next().unwrap_or("");
    format!("{scheme}{host}{path}")
}

/// **The one rule `cut_m` follows** (#472), in one sentence so both tools refuse it identically: a section
/// plane with no `hide` is refused, because removing the air in front of a room that is still buried shows
/// rock — the plane deletes what stands between the camera and the interior, and `hide` is what makes the
/// interior the thing there. `Ok(())` when the call is coherent (both named, or neither).
pub fn check_cut(hide: Option<&[String]>, cut_m: Option<f32>) -> Result<(), String> {
    let Some(metres) = cut_m else { return Ok(()) };
    if hide.is_none_or(<[String]>::is_empty) {
        return Err(format!(
            "`cut_m: {metres}` needs `hide`. A section plane removes what stands between the camera and an \
             interior, so without hiding the terrain there is no interior to see and the picture is rock with a \
             hole in it. Say `hide: [\"terrain\"]` and `cut_m: {metres}`, or neither. Nothing was changed."
        ));
    }
    if !metres.is_finite() || metres <= 0.0 {
        return Err(format!(
            "`cut_m: {metres}` is not a distance a plane can stand at: it is metres from the camera along its \
             view and must be above zero. Nothing was changed."
        ));
    }
    Ok(())
}

/// **Whether the server must take the editor's picture before deciding.**
pub fn needs_view(params: &DecideParams, checks: &Checks) -> bool {
    if params.images.iter().any(|i| i.name == VIEW) {
        return false;
    }
    params.view
        || params
            .check
            .as_deref()
            .and_then(|n| checks.get(n))
            .is_some_and(|c| c.images.iter().any(|i| i == VIEW))
}

/// **Where `blueprint_capture_still` writes**, under the project: `<project>/target/loop/captures/<label>/still-<ms>.png`.
const CAPTURES: [&str; 3] = ["target", "loop", "captures"];

/// **The most bytes of one picture a file named in `images` may have** (8 MiB). A still of the editor's
/// surface is a PNG of a few hundred KiB to a few MiB; a file past this is not one, and a remote endpoint
/// would be sent it base64-encoded, a third larger.
pub const IMAGE_BYTES_MAX: u64 = 8 * 1024 * 1024;

/// **Whether `s` is a fingerprint**: what [`crate::fingerprint`] returns, sixteen lowercase hex digits.
fn is_fingerprint(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// **Every still under `dir`**, as `(modified, path)`, found by walking `captures/<label>/...`. **A link is
/// never followed**, neither a linked folder (so a cycle cannot trap the walk, and a folder elsewhere in the
/// project cannot be searched through the captures) nor a linked file: only what `blueprint_capture_still`
/// wrote itself is a still. A file over [`IMAGE_BYTES_MAX`] cannot be sent, so it is not a candidate and is
/// never read.
fn stills_under(dir: &Path, out: &mut Vec<(std::time::SystemTime, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        let path = entry.path();
        if kind.is_dir() {
            stills_under(&path, out);
        } else if kind.is_file()
            && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("still-") && n.ends_with(".png"))
            && let Ok(meta) = entry.metadata()
            && meta.len() <= IMAGE_BYTES_MAX
            && let Ok(modified) = meta.modified()
        {
            out.push((modified, path));
        }
    }
}

/// **The project's captures folder**, where `blueprint_capture_still` writes.
fn captures_dir(root: &Path) -> PathBuf {
    CAPTURES.iter().fold(root.to_path_buf(), |p, part| p.join(part))
}

/// **The newest still under the project's captures whose bytes have `fingerprint`**, hashing newest first and
/// stopping at the first match. Only the stills `blueprint_capture_still` wrote are searched: a fingerprint
/// never reaches an arbitrary file.
fn still_with_fingerprint(root: &Path, fingerprint: &str) -> Result<PathBuf, String> {
    let mut stills = Vec::new();
    stills_under(&captures_dir(root), &mut stills);
    stills.sort_by_key(|a| std::cmp::Reverse(a.0));
    stills
        .into_iter()
        .map(|(_, path)| path)
        .find(|path| std::fs::read(path).is_ok_and(|bytes| crate::fingerprint(&bytes) == fingerprint))
        .ok_or_else(|| {
            format!(
                "image `{fingerprint}`: no still under the project's target/loop/captures has that fingerprint; \
                 take one with blueprint_capture_still and pass its path or its fingerprint."
            )
        })
}

/// A still under `project`, as a data URL. `rel` is its path (relative to the project, or absolute, as
/// `blueprint_capture_still` answers it) or the fingerprint of a still.
///
/// **Only what `blueprint_capture_still` wrote is sent to a model** (the files under the project's
/// `target/loop/captures`, after links are resolved), and at most [`IMAGE_BYTES_MAX`] bytes of it: a
/// remote endpoint (DM-02) must never carry a file the agent merely named.
fn image_data_url(project: &Path, rel: &str) -> Result<String, String> {
    use base64::Engine as _;
    let root = project.canonicalize().map_err(|e| format!("{}: {e}", project.display()))?;
    let full: PathBuf = if is_fingerprint(rel) && !root.join(rel).exists() {
        still_with_fingerprint(&root, rel)?
    } else {
        root.join(rel)
    };
    let full = full.canonicalize().map_err(|e| format!("image `{rel}`: {e}"))?;
    if !full.starts_with(&root) {
        return Err(format!("image `{rel}` is outside the project."));
    }
    let captures = captures_dir(&root);
    if !captures.canonicalize().is_ok_and(|c| full.starts_with(c)) {
        return Err(format!(
            "image `{rel}` is not a still: only the pictures under the project's target/loop/captures, which \
             blueprint_capture_still writes, are sent to a model."
        ));
    }
    let mime = match full.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        _ => return Err(format!("image `{rel}` is not a .png or .jpg file.")),
    };
    let len = std::fs::metadata(&full).map_err(|e| format!("image `{rel}`: {e}"))?.len();
    if len > IMAGE_BYTES_MAX {
        return Err(format!("image `{rel}` is {len} bytes; a picture sent to a model is at most {IMAGE_BYTES_MAX} bytes."));
    }
    let bytes = std::fs::read(&full).map_err(|e| format!("image `{rel}`: {e}"))?;
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// **A transport that notes what the answer must say about itself**: which model answered, and how long the
/// endpoint took (the sum of its answers' round trips, so the pictures' encoding is not counted).
struct Measured<'a> {
    inner: &'a dyn Transport,
    model: RefCell<Option<String>>,
    spent: Cell<Duration>,
}

impl<'a> Measured<'a> {
    fn new(inner: &'a dyn Transport) -> Self {
        Measured { inner, model: RefCell::new(None), spent: Cell::new(Duration::ZERO) }
    }

    /// **The answer's `model`, `model_source`, `where` and `latency_ms`**, to be merged into it.
    fn about(&self) -> Value {
        let model = self.model.borrow().clone();
        json!({
            "model_source": model.as_ref().map(|_| REPORTED),
            "model": model,
            "where": self.inner.site().where_ran(),
            "latency_ms": u64::try_from(self.spent.get().as_millis()).unwrap_or(u64::MAX),
        })
    }
}

impl Transport for Measured<'_> {
    fn endpoint(&self) -> String {
        self.inner.endpoint()
    }
    fn site(&self) -> bevy_oracle::DecisionModel {
        self.inner.site()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        let started = Instant::now();
        let response = self.inner.send(request);
        self.spent.set(self.spent.get() + started.elapsed());
        if let Ok(r) = &response
            && let Some(name) = &r.model
        {
            self.model.borrow_mut().get_or_insert_with(|| name.clone());
        }
        response
    }
}

/// **`model_source` when the endpoint's own response named the model.**
pub const REPORTED: &str = "reported";
/// **`model_source` when the endpoint named none and the configuration did** (the remote model's name is one the
/// request itself carried).
pub const CONFIGURED: &str = "configured";

/// **Name the model the endpoint did not name**: a server that answers without a `model` is named by the
/// one the configuration sent, when there is one, and the answer's `model_source` says `configured` rather
/// than `reported`. A model the endpoint named is never replaced.
pub fn name_the_model(answer: &mut Value, configured: Option<&str>) {
    if answer["model"].is_null()
        && let Some(name) = configured
    {
        answer["model"] = json!(name);
        answer["model_source"] = json!(CONFIGURED);
    }
}

fn merge(answer: &mut Value, about: Value) {
    if let (Some(a), Value::Object(b)) = (answer.as_object_mut(), about) {
        a.extend(b);
    }
}

/// **Decide.** `view` is the editor's current picture as base64 PNG, when it was taken.
pub fn decide(
    params: &DecideParams,
    project: &Path,
    view: Option<&str>,
    transport: &dyn Transport,
) -> Result<Value, String> {
    decide_with(params, project, view, None, transport)
}

/// **Decide, with what was read from the running game** (#114): its frame joins the pictures as `game`, and
/// its state is the facts' `game` key.
pub fn decide_with(
    params: &DecideParams,
    project: &Path,
    view: Option<&str>,
    game: Option<&GameEvidence>,
    transport: &dyn Transport,
) -> Result<Value, String> {
    let measured = Measured::new(transport);
    let transport = &measured;
    let mut images: BTreeMap<String, String> = BTreeMap::new();
    let mut ordered: Vec<String> = Vec::new();
    for i in &params.images {
        let url = image_data_url(project, &i.path)?;
        ordered.push(url.clone());
        images.insert(i.name.clone(), url);
    }
    if let Some(png) = view {
        let url = format!("data:image/png;base64,{png}");
        if !images.contains_key(VIEW) {
            ordered.push(url.clone());
            images.insert(VIEW.to_owned(), url);
        }
    }
    let mut facts = params.facts.clone();
    if let Some(g) = game {
        let url = format!("data:image/png;base64,{}", g.png);
        if !images.contains_key(GAME) {
            ordered.push(url.clone());
            images.insert(GAME.to_owned(), url);
        }
        if let Some(state) = &g.state {
            facts = Some(match facts {
                None => json!({ GAME: state }),
                Some(Value::Object(mut o)) if !o.contains_key(GAME) => {
                    o.insert(GAME.to_owned(), state.clone());
                    Value::Object(o)
                }
                Some(_) => {
                    return Err("`game.state` puts the game's state under `facts.game`: give `facts` as an object without a `game` key.".into());
                }
            });
        }
    }
    match (&params.check, &params.questions) {
        (Some(_), Some(_)) => Err("give `check` or `questions`, not both.".into()),
        (None, None) => Err("give `check` (a named check) or `questions` (ad-hoc).".into()),
        (Some(name), None) => {
            let checks = load_checks(project)?;
            let check: &Check = checks.get(name).ok_or_else(|| {
                let known: Vec<&str> = checks.checks.iter().map(|c| c.name.as_str()).collect();
                format!("no check named `{name}`; the checks are: {}.", known.join(", "))
            })?;
            let evidence = Evidence { images, facts };
            let finding = check.run(transport, &evidence).map_err(|e| e.to_string())?;
            let mut answer = json!({
                "check": name,
                "finding": finding,
                "message": finding.message(),
                "calibrated": finding.is_calibrated(),
                "model": finding.model,
                "note": finding.uncalibrated_note(),
                "advisory": true,
                "endpoint": transport.endpoint(),
            });
            if let Some(g) = game {
                answer["game"] = json!({"port": g.port, "state": g.state.is_some()});
            }
            merge(&mut answer, measured.about());
            Ok(answer)
        }
        (None, Some(questions)) => {
            let mut state = params.state.clone().unwrap_or_default();
            if let Some(f) = &facts {
                state = format!("{state}\n\nFacts (JSON): {f}");
            }
            let mut req = Request::new(state);
            for (id, q) in questions {
                let q: Question = serde_json::from_value(q.clone()).map_err(|e| format!("question `{id}`: {e}"))?;
                req.questions.insert(id.clone(), q);
            }
            req.images = ordered;
            let response = bevy_oracle::ask(transport, &req).map_err(|e| e.to_string())?;
            let mut answer = json!({"answers": response.answers, "endpoint": transport.endpoint()});
            if let Some(g) = game {
                answer["game"] = json!({"port": g.port, "state": g.state.is_some()});
            }
            merge(&mut answer, measured.about());
            Ok(answer)
        }
    }
}

/// **Decide as the person's settings say** (DM-03, #454): read the layers (`user_settings` is the person's
/// `settings.yaml`, and the project's own `editor.yaml` is looked at too), build the endpoint they choose and ask it.
///
/// **One endpoint per call, no fallback.** Remote with no `OPENROUTER_API_KEY` is refused with that sentence and
/// the local model is not asked instead; a settings file that does not read is refused naming the file. A project file that
/// names the choice is ignored and said in the answer's `settings_problems`, naming the file.
pub fn decide_asked(
    params: &DecideParams,
    project: &Path,
    view: Option<&str>,
    game: Option<&GameEvidence>,
    user_settings: Option<&Path>,
    endpoints: &bevy_oracle::Endpoints,
) -> Result<Value, String> {
    let resolved = endpoints.resolve(user_settings, Some(&bevy_oracle::project_file(project))).map_err(|e| e.to_string())?;
    let configured = resolved.config.model.clone();
    let transport = bevy_oracle::Live::new(resolved.config);
    let mut answer = decide_with(params, project, view, game, &transport)?;
    name_the_model(&mut answer, configured.as_deref());
    if !resolved.chosen.problems.is_empty() {
        answer["settings_problems"] = json!(resolved.chosen.problems.iter().map(ToString::to_string).collect::<Vec<_>>());
    }
    Ok(answer)
}

/// **The name of the self test's decision-model check**: the editor's card gives it a row of its own.
pub const SELF_TEST_CHECK: &str = "decision_model";

/// **How long the self test waits for the model**, in seconds. The default wait is a minute, which would hold an agent's first
/// call that long; a model that has not answered a one-word question in this long is not available to a check that wants
/// to be fast.
pub const SELF_TEST_PATIENCE_SECS: u64 = 20;

/// **The decision model's check for `blueprint_self_test`**, as `{name, ok, detail}`: ask the chosen model one text-only `noul` and
/// say which model answered, where it ran (`local` or `remote`) and how long it took, or why it did not answer. An agent
/// learns at start-up whether its fastest check is there.
///
/// Resolved exactly as `blueprint_decide` is ([`decide_asked`]): one endpoint, no fallback, and Remote with no key fails
/// with that sentence rather than asking the local model. Nothing of the project goes in the question.
pub fn self_test_check(project: Option<&Path>, user_settings: Option<&Path>, endpoints: &bevy_oracle::Endpoints) -> Value {
    let project_file = project.map(bevy_oracle::project_file);
    let result = endpoints.resolve(user_settings, project_file.as_deref()).and_then(|resolved| {
        let mut config = resolved.config;
        config.timeout_secs = SELF_TEST_PATIENCE_SECS;
        bevy_oracle::probe(&bevy_oracle::Live::new(config))
    });
    match result {
        Ok(a) => json!({
            "name": SELF_TEST_CHECK,
            "ok": true,
            "detail": format!("{} · {} · {} ms", a.model, a.site, a.latency.as_millis()),
        }),
        Err(e) => json!({"name": SELF_TEST_CHECK, "ok": false, "detail": e.to_string()}),
    }
}
