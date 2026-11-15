//! **The decision-model setting, read from the settings layers with no engine** (DM-03, #454).
//!
//! The setting is the one source of which decision model is asked: `decision_model: local` or `decision_model: remote` in
//! the person's `settings.yaml`, which the editor writes from Settings. The editor reads it through its own settings
//! layers; **this module is the other reader**, for everything that runs without the editor's `App`: the MCP server
//! (one process per project, which cannot link the editor), the labeller's worker threads, the after-edit checks and
//! `decide_check`. All of them call [`Endpoints::resolve`], so there is one answer to *which endpoint*.
//!
//! **The two layers, and what each may say.**
//!
//! - The person's file ([`user_file`]) chooses. No file, or no key, is `local`.
//! - The project's file ([`project_file`], `assets/grimoire/editor.yaml`) **cannot**: a project that could say `remote`
//!   would switch its own agents' pictures off the LAN for everyone who opens it. A project file that names the key
//!   at all is a [`SettingsProblem`] naming the file, and the person's choice stands.
//!
//! A user file that does not read (not YAML, an unknown value) is an error and nothing is asked: a half-read choice must
//! never fall back to either model. Every other key of the files is the editor's business and is not looked at here.
//!
//! `OPENROUTER_API_KEY` is not a setting and is not read here (see [`Endpoints::from_env`]).

use crate::{Config, DecisionModel, Endpoints, Error};
use bevy_grimoire_core::reload_check::{GRIMOIRE_DIR, PROJECT_SETTINGS_FILE};
use bevy_grimoire_core::user_dirs;
use serde::Deserialize;
use std::fmt;
use std::path::{Path, PathBuf};

/// The key of both settings files that holds the choice.
pub const SETTING_KEY: &str = "decision_model";

/// **What a settings file got wrong about the decision model**, naming the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsProblem {
    /// The file, with `/` separators.
    pub file: String,
    /// What is wrong, in a sentence.
    pub reason: String,
}

impl fmt::Display for SettingsProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {SETTING_KEY}: {}", self.file, self.reason)
    }
}

/// **The sentence for a project file that names the choice**, and the fix. The editor's verdict on the file and the
/// MCP server's answer both say it.
pub fn project_may_not_choose(chose: DecisionModel) -> String {
    format!(
        "this project's file sets {SETTING_KEY} to {chose}, but which model decides is the person's own choice, made in their own settings file: a project cannot choose it, least of all {remote}, which sends pictures off the LAN. It is ignored here; remove the key from this file",
        remote = DecisionModel::Remote,
    )
}

/// **The choice, and what the layers said wrong on the way to it.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    pub model: DecisionModel,
    /// Findings about files that did not decide, such as a project file that named the key. Never empty by accident:
    /// each one names its file.
    pub problems: Vec<SettingsProblem>,
}

/// One file's say about the key: only the key, so every other setting of the file is left to the editor.
#[derive(Deserialize, Default)]
struct Layer {
    #[serde(default)]
    decision_model: Option<DecisionModel>,
}

fn file_name(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// What `path` says about the key: `None` for no file or no key, the reason when it does not read.
fn read_layer(path: &Path) -> Result<Option<DecisionModel>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    let layer: Option<Layer> = serde_yaml_ng::from_str(&text).map_err(|e| format!("does not read: {e}"))?;
    Ok(layer.and_then(|l| l.decision_model))
}

/// **The person's settings file** (`<config_dir>/Grimoire/settings.yaml`), or `None` on a platform with no config directory.
pub fn user_file() -> Option<PathBuf> {
    user_dirs::settings_file()
}

/// **A project's settings file** (`<root>/assets/grimoire/editor.yaml`).
pub fn project_file(root: &Path) -> PathBuf {
    root.join(GRIMOIRE_DIR).join(PROJECT_SETTINGS_FILE)
}

/// **Which model decides**: the person's file, else [`DecisionModel::Local`]; a project file that names the key is a
/// problem and decides nothing. `Err` when the person's file does not read.
pub fn choose(user: Option<&Path>, project: Option<&Path>) -> Result<Chosen, SettingsProblem> {
    let model = match user {
        Some(path) => read_layer(path).map_err(|reason| SettingsProblem { file: file_name(path), reason })?.unwrap_or_default(),
        None => DecisionModel::default(),
    };
    let mut problems = Vec::new();
    // A project file that does not read is the editor's to report (its own watcher refuses it); it cannot decide anyway.
    if let Some(path) = project
        && let Ok(Some(chose)) = read_layer(path)
    {
        problems.push(SettingsProblem { file: file_name(path), reason: project_may_not_choose(chose) });
    }
    Ok(Chosen { model, problems })
}

/// **A choice made into the configuration that asks it**, with what the layers said on the way.
#[derive(Debug, Clone, PartialEq)]
pub struct Resolved {
    pub config: Config,
    pub chosen: Chosen,
}

impl Endpoints {
    /// **The one resolver** (DM-03, #454): read the layers named, choose, and build the configuration of that choice.
    ///
    /// `Err` when the person's file does not read ([`Error::Settings`]) or when Remote is chosen and
    /// [`Endpoints::key`] is unset ([`Error::RemoteWithoutKey`]). Neither is answered by the other model.
    pub fn resolve(&self, user: Option<&Path>, project: Option<&Path>) -> Result<Resolved, Error> {
        let chosen = choose(user, project).map_err(Error::Settings)?;
        let config = self.config(chosen.model)?;
        Ok(Resolved { config, chosen })
    }

    /// [`Endpoints::resolve`] for the project rooted at `root`, with the person's real settings file.
    pub fn resolve_for_project(&self, root: &Path) -> Result<Resolved, Error> {
        self.resolve(user_file().as_deref(), Some(&project_file(root)))
    }
}

/// **What one text-only question to the chosen model found out**: which model answered, where it ran and how long it took.
#[derive(Debug, Clone, PartialEq)]
pub struct Answered {
    /// The model the answer named, else the chosen model's own name ([`DecisionModel::name`]).
    pub model: String,
    pub site: DecisionModel,
    pub latency: std::time::Duration,
}

/// The state of the probe's one question: no picture, nothing of the project's.
const PROBE_STATE: &str = "ready";
/// The probe's id and instruction: the smallest request an endpoint accepts.
const PROBE_ID: &str = "ready";
const PROBE_QUESTION: &str = "Answer yes.";

/// **Ask the model one text-only `noul` question** and report who answered and how fast: the editor's status LED and the
/// agent's self test both ask it, so an agent learns at start-up whether its fastest check is there. Nothing of the project goes
/// with it, which is also why it is safe to send to Remote.
pub fn probe(transport: &dyn crate::Transport) -> Result<Answered, Error> {
    let request = crate::Request::new(PROBE_STATE).with_question(PROBE_ID, crate::Question::noul(PROBE_QUESTION));
    let started = std::time::Instant::now();
    let response = crate::ask(transport, &request)?;
    let latency = started.elapsed();
    let site = transport.site();
    Ok(Answered { model: response.model.unwrap_or_else(|| site.name().to_owned()), site, latency })
}
