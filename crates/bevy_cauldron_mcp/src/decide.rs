//! **`grimoire_decide`'s logic, apart from the transport and the editor**, so a test drives it with a
//! recorded response and no model, no editor and no network.
//!
//! Two forms: a **named check** from `checks.yaml` (the editor's built-ins, merged with the project's own
//! `assets/grimoire/checks.yaml`, read on every call so an edit takes effect at once), and **ad-hoc typed
//! questions**. Images are files under the project; the one picture the server can take itself is `view`,
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

/// **The parameters of `grimoire_decide`.**
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
    /// **Pictures, files under the project**, each with the name a check asks for (`before`, `after`,
    /// `closeup`, ...). Ad-hoc questions send them in this order. At most four. A still from
    /// `grimoire_capture_still` is accepted as its `path` or as its `fingerprint` (see [`NamedImage::path`]),
    /// so a before-and-after pair is asked about without a second capture.
    #[serde(default)]
    pub images: Vec<NamedImage>,
    /// **Whether to take the editor's current picture too**, named `view`. A named check that needs `view`
    /// and is not given it takes it anyway.
    #[serde(default)]
    pub view: bool,
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
    if bevy_grimoire_core::play::path_for_project(project).exists() {
        return Ok(bevy_grimoire_core::play::PlayConfig::load(project)?.port);
    }
    Ok(bevy_grimoire_core::play::DEFAULT_PORT)
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

/// **A picture on disk, and what it is called.**
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NamedImage {
    /// The name a check asks for.
    pub name: String,
    /// **A PNG or JPEG file under the project**, relative to it or absolute, or **the fingerprint** (sixteen
    /// lowercase hex digits) that `grimoire_capture_still` returned for a still: the newest still under the
    /// project's `target/loop/captures` with those bytes is the picture.
    pub path: String,
}

/// The editor's checks with the project's own over them.
pub fn load_checks(project: &Path) -> Result<Checks, String> {
    Checks::for_project(project)
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

/// **Where `grimoire_capture_still` writes**, under the project: `<project>/target/loop/captures/<label>/still-<ms>.png`.
const CAPTURES: [&str; 3] = ["target", "loop", "captures"];

/// **Whether `s` is a fingerprint**: what [`crate::fingerprint`] returns, sixteen lowercase hex digits.
fn is_fingerprint(s: &str) -> bool {
    s.len() == 16 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// **Every still under `dir`**, as `(modified, path)`, found by walking `captures/<label>/...`.
fn stills_under(dir: &Path, out: &mut Vec<(std::time::SystemTime, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            stills_under(&path, out);
        } else if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("still-") && n.ends_with(".png"))
            && let Ok(modified) = entry.metadata().and_then(|m| m.modified())
        {
            out.push((modified, path));
        }
    }
}

/// **The newest still under the project's captures whose bytes have `fingerprint`.** Only the stills
/// `grimoire_capture_still` wrote are searched: a fingerprint never reaches an arbitrary file.
fn still_with_fingerprint(root: &Path, fingerprint: &str) -> Result<PathBuf, String> {
    let mut stills = Vec::new();
    stills_under(&CAPTURES.iter().fold(root.to_path_buf(), |p, part| p.join(part)), &mut stills);
    stills.sort_by_key(|a| std::cmp::Reverse(a.0));
    stills
        .into_iter()
        .map(|(_, path)| path)
        .find(|path| std::fs::read(path).is_ok_and(|bytes| crate::fingerprint(&bytes) == fingerprint))
        .ok_or_else(|| {
            format!(
                "image `{fingerprint}`: no still under the project's target/loop/captures has that fingerprint; \
                 take one with grimoire_capture_still and pass its path or its fingerprint."
            )
        })
}

/// A file under `project`, as a data URL. Refuses a path that leaves the project. `rel` is a path (relative
/// to the project, or absolute) or the fingerprint of a still from `grimoire_capture_still`.
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
    let mime = match full.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        _ => return Err(format!("image `{rel}` is not a .png or .jpg file.")),
    };
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

    /// **The answer's `model`, `where` and `latency_ms`**, to be merged into it.
    fn about(&self) -> Value {
        json!({
            "model": self.model.borrow().clone(),
            "where": where_of(&self.inner.endpoint()),
            "latency_ms": u64::try_from(self.spent.get().as_millis()).unwrap_or(u64::MAX),
        })
    }
}

impl Transport for Measured<'_> {
    fn endpoint(&self) -> String {
        self.inner.endpoint()
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

/// **`local` only for the editor's own default endpoint**; any other is `remote`, which says that the
/// pictures may have left the LAN. Reading it the other way round would let a mistyped endpoint pass as local.
pub const LOCAL: &str = "local";
/// What `where` says of any endpoint but the editor's own default.
pub const REMOTE: &str = "remote";

fn where_of(endpoint: &str) -> &'static str {
    if endpoint == bevy_oracle::Config::default().endpoint { LOCAL } else { REMOTE }
}

/// **Name the model the endpoint did not name**: a server that answers without a `model` is named by the
/// one the person configured (`GRIMOIRE_DECIDE_MODEL`), when there is one. A model the endpoint named is
/// never replaced.
pub fn name_the_model(answer: &mut Value, configured: Option<&str>) {
    if answer["model"].is_null()
        && let Some(name) = configured
    {
        answer["model"] = json!(name);
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
                "calibrated": check.calibration.is_some(),
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
