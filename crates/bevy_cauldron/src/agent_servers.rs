//! **The agent session's MCP servers are the editor's and the project's, and nothing else** (#484,
//! `feedback-1794783878399`).
//!
//! The owner, looking at the terminal pane: *"Starting an agent should not automatically start all MCPs. The game
//! editor should have its own MCP detection and enablement."*
//!
//! What the editor writes has always been one server (its own, in `.mcp.json` beside an omp extension). What
//! omp *also* does is read its own machine-level `~/.omp/agent/mcp.json` and every other agent tool's config,
//! so a session meant to have one server came up with the whole machine's list.
//!
//! # What this file is, and what it is not
//!
//! It is the **set** the editor's session is meant to have, as data: the editor's server plus whatever the open
//! project declares ([`crate::terminal_agent::project_mcp_servers`]), with a name for each in [`SessionServers`],
//! which the self-test card prints so a person can read what their session has.
//!
//! It is **not** a way to stop omp discovering the rest, because omp at v18.16.0 has none. Measured on
//! 2026-11-17 (each number from a run whose servers write a file when spawned):
//!
//! | Attempt | Result |
//! |---|---|
//! | `enabledServers` in the user `mcp.json` (the `/mcp` toggle list) | not an allow-list: both servers started |
//! | `--no-extensions -e <dir>` | the folder's server started, **and every user-level server still started** |
//! | `--config` overlay with `disabledExtensions: ["mcp:*"]` | a glob is not matched; the servers started |
//! | `--config` overlay with `["mcp:<name>", …]`, one per server | **works**, but needs every name omp would discover |
//! | `--profile <name>` | the profile's own `mcp.json` **and** the user-level one |
//! | `PI_CODING_AGENT_DIR=<dir>` | **scopes discovery** to `<dir>/mcp.json` — and takes the credential store with it, so a dir without credentials fails `No API key found` |
//!
//! So scoping a session to a named set is an omp feature that does not exist, and the editor does not re-implement
//! omp's discovery (eight config sources over per-platform paths) to name every server it would otherwise load:
//! that is the wrapper this issue refuses. The request is filed on omp's own repository
//! (**can1357/oh-my-pi#19573**, with the table above) and this work is blocked on it.
//!
//! # What the editor promises until then
//!
//! Whatever omp discovers, the editor's session **is** these servers: the card names them (so a person reads the
//! set rather than guessing), and a project's own `.mcp.json` servers are written into the extension folder's
//! `.mcp.json` beside the editor's, so the project's declaration is what the session has rather than something
//! omp has to be trusted to find.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use serde_json::{Map, Value, json};

use crate::terminal_agent;

/// **How many declared servers a session may add.** A project that names more than this is refused rather than
/// half-loaded: a cap keeps the card readable and a typo from writing a hundred processes, and is far above any
/// project that has ever needed one.
pub const PROJECT_SERVERS_MAX: usize = 16;

/// The project file a project declares its own MCP servers in, at the project's root.
pub const PROJECT_SERVERS_FILE: &str = ".mcp.json";

/// **The servers a session has**, in the order the card lists them: the editor's own first, then the project's, by
/// name. One value rather than two readers of the config, so "which servers does this session have" has one answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionServers {
    /// The editor's own server's name in every config written here ([`terminal_agent::MCP_SERVER_NAME`]).
    pub editor: bool,
    /// The project's declared servers, by name, sorted.
    pub project: Vec<String>,
}

impl SessionServers {
    /// **The set for a launch**: the editor's server, plus whatever the project at `root` declares.
    pub fn of(root: Option<&Path>) -> SessionServers {
        SessionServers { editor: true, project: root.map(project_server_names).unwrap_or_default() }
    }

    /// **Every server's name**, the editor's first, then the project's: what the card prints and what a person reads.
    pub fn names(&self) -> Vec<String> {
        let mut names = Vec::with_capacity(self.project.len() + 1);
        if self.editor {
            names.push(terminal_agent::MCP_SERVER_NAME.to_owned());
        }
        names.extend(self.project.iter().cloned());
        names
    }

    /// **The row's sentence**: the names, or why there is only one.
    pub fn words(&self) -> String {
        let names = self.names();
        if names.is_empty() {
            return "none".to_owned();
        }
        let mut said = names.join(", ");
        if self.project.is_empty() {
            said.push_str(" (no project declares another)");
        }
        said
    }
}

/// **The names a project declares in its own `.mcp.json`**, sorted and de-duplicated, or nothing when it declares none
/// or the file does not read.
///
/// A file that does not read is **not** a refusal of the launch: a project's server list is a convenience, and a
/// malformed one must not stop a person's agent from starting. It is said on the card ([`SessionServers::project`]
/// is empty and [`project_file_problem`] names it), so a typo is visible rather than silent. Past
/// [`PROJECT_SERVERS_MAX`] the extra names are dropped for the same reason — the launch goes on, the card says how
/// many were read.
pub fn project_server_names(root: &Path) -> Vec<String> {
    let path = root.join(PROJECT_SERVERS_FILE);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(config) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    server_names_in(&config)
}

/// **Why a project's `.mcp.json` gave no servers**, when it exists but could not be read: what the card says instead of
/// pretending the project declared nothing.
pub fn project_file_problem(root: &Path) -> Option<String> {
    let path = root.join(PROJECT_SERVERS_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => return Some(format!("{}: {e}", path.display())),
    };
    match serde_json::from_str::<Value>(&text) {
        Ok(config) => {
            let declared = server_count(&config);
            (declared > PROJECT_SERVERS_MAX).then(|| {
                format!("{} declares {declared} servers; a session takes the first {PROJECT_SERVERS_MAX}", path.display())
            })
        }
        Err(e) => Some(format!("{}: {e}", path.display())),
    }
}

/// How many servers a config declares, under `mcpServers`.
fn server_count(config: &Value) -> usize {
    config.get("mcpServers").and_then(Value::as_object).map_or(0, Map::len)
}

/// **The server names a config declares**, sorted, capped at [`PROJECT_SERVERS_MAX`].
fn server_names_in(config: &Value) -> Vec<String> {
    let Some(servers) = config.get("mcpServers").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut names: Vec<String> = servers.keys().take(PROJECT_SERVERS_MAX).cloned().collect();
    names.sort();
    names
}

/// **The config an omp session is handed**: the editor's server, then each server the project declares, under its own
/// name.
///
/// **Written on every launch**, because the editor's server carries a token minted per launch ([`terminal_agent::plan`]):
/// a config file kept from an earlier launch would hand the agent a token no tab holds.
pub fn session_config(editor: &Value, project: Option<&Path>) -> Value {
    let mut servers = editor.get("mcpServers").and_then(Value::as_object).cloned().unwrap_or_default();
    if let Some(root) = project
        && let Ok(text) = std::fs::read_to_string(root.join(PROJECT_SERVERS_FILE))
        && let Ok(declared) = serde_json::from_str::<Value>(&text)
        && let Some(servers_in) = declared.get("mcpServers").and_then(Value::as_object)
    {
        for (name, config) in servers_in.iter().take(PROJECT_SERVERS_MAX) {
            // **The editor's own name is the editor's**: a project that declares a `cauldron` server would otherwise
            // replace the one server the session cannot do without.
            if name != terminal_agent::MCP_SERVER_NAME {
                servers.insert(name.clone(), config.clone());
            }
        }
    }
    json!({ "mcpServers": servers })
}

/// **A project's declared servers for a root, as the set the card shows and the config carries.**
pub fn project_servers_for(root: Option<&Path>) -> (SessionServers, Option<String>) {
    let Some(root) = root else {
        return (SessionServers::of(None), None);
    };
    (SessionServers::of(Some(root)), project_file_problem(root))
}

/// The file under a project's root that declares its servers.
pub fn project_servers_path(root: &Path) -> PathBuf {
    root.join(PROJECT_SERVERS_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("session_servers_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
        root
    }

    /// **The set is the editor's plus the project's, by name**, and a project that declares none says so.
    #[test]
    fn the_sessions_servers_are_the_editors_then_the_projects_by_name() {
        let dir = root("set");
        std::fs::write(
            dir.join(PROJECT_SERVERS_FILE),
            r#"{"mcpServers": {"zeta": {"type":"stdio","command":"x"}, "alpha": {"type": "stdio", "command": "y"}}}"#,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let servers = SessionServers::of(Some(&dir));
        assert_eq!(servers.names(), ["cauldron", "alpha", "zeta"], "the editor's server first, then the project's alphabetically");
        assert!(servers.words().starts_with("cauldron, alpha, zeta"), "{}", servers.words());
        let alone = SessionServers::of(None);
        assert_eq!(alone.names(), ["cauldron"], "a launch with no project has the editor's server alone");
        assert!(alone.words().contains("no project declares another"), "{}", alone.words());
    }

    /// **A server the project declares is in the config the session gets, and one the machine has is not.**
    #[test]
    fn the_config_carries_the_editors_server_and_the_projects_and_nothing_else() {
        let dir = root("config");
        std::fs::write(
            dir.join(PROJECT_SERVERS_FILE),
            r#"{"mcpServers": {"shared": {"type":"stdio","command":"/bin/echo","args":["one"]}}}"#,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let editor = json!({"mcpServers": {"cauldron": {"type":"stdio","command":"/bin/cauldron","env":{"CAULDRON_AGENT_TOKEN":"t"}}}});
        let config = session_config(&editor, Some(&dir));
        let servers = config["mcpServers"].as_object().expect("servers");
        let mut names: Vec<&str> = servers.keys().map(String::as_str).collect();
        names.sort();
        assert_eq!(names, ["cauldron", "shared"], "exactly the editor's server and the project's: {config}");
        assert_eq!(servers["cauldron"]["env"]["CAULDRON_AGENT_TOKEN"], "t", "the editor's server keeps its token");
        assert_eq!(servers["shared"]["command"], "/bin/echo");
    }

    /// **A project cannot take the editor's own name**, or the one server the session cannot do without would be replaced.
    #[test]
    fn a_project_that_declares_the_editors_name_does_not_replace_it() {
        let dir = root("shadow");
        std::fs::write(
            dir.join(PROJECT_SERVERS_FILE),
            r#"{"mcpServers": {"cauldron": {"type":"stdio","command":"/bin/impostor"}, "mine": {"type":"stdio","command":"/bin/mine"}}}"#,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let editor = json!({"mcpServers": {"cauldron": {"type":"stdio","command":"/bin/cauldron"}}});
        let config = session_config(&editor, Some(&dir));
        assert_eq!(config["mcpServers"]["cauldron"]["command"], "/bin/cauldron", "the editor's server stands: {config}");
        assert_eq!(config["mcpServers"]["mine"]["command"], "/bin/mine");
    }

    /// **A `.mcp.json` that does not read declares no servers and is named**, so a typo is visible and a launch still happens.
    #[test]
    fn a_project_file_that_does_not_read_declares_nothing_and_is_named() {
        let dir = root("broken");
        std::fs::write(dir.join(PROJECT_SERVERS_FILE), "{not json").unwrap_or_else(|e| panic!("{e}"));
        assert!(project_server_names(&dir).is_empty());
        let problem = project_file_problem(&dir).expect("a file that does not read is said");
        assert!(problem.contains(".mcp.json"), "{problem}");
        let (servers, said) = project_servers_for(Some(&dir));
        assert_eq!(servers.names(), ["cauldron"], "the editor's server still starts");
        assert_eq!(said, Some(problem));
    }

    /// **A project with more servers than the cap takes the first [`PROJECT_SERVERS_MAX`] and says so**, rather than
    /// starting a hundred processes or refusing the launch.
    #[test]
    fn more_servers_than_the_cap_are_cut_and_said() {
        let dir = root("cap");
        let mut servers = Map::new();
        for n in 0..(PROJECT_SERVERS_MAX + 4) {
            servers.insert(format!("s{n:02}"), json!({"type": "stdio", "command": "/bin/true"}));
        }
        std::fs::write(dir.join(PROJECT_SERVERS_FILE), json!({ "mcpServers": servers }).to_string()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(project_server_names(&dir).len(), PROJECT_SERVERS_MAX);
        let said = project_file_problem(&dir).expect("the cap is said");
        assert!(said.contains(&format!("first {PROJECT_SERVERS_MAX}")), "{said}");
    }

    /// **A project that declares nothing is not a problem**, and one that is not there says nothing at all.
    #[test]
    fn a_project_with_no_file_or_no_servers_is_quiet() {
        let dir = root("quiet");
        assert_eq!(project_file_problem(&dir), None, "no file is no problem");
        assert!(project_server_names(&dir).is_empty());
        std::fs::write(dir.join(PROJECT_SERVERS_FILE), r#"{"mcpServers": {}}"#).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(project_file_problem(&dir), None, "an empty list is no problem");
        assert!(project_server_names(&dir).is_empty());
    }
}
