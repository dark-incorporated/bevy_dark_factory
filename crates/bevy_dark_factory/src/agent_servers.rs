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
//! # What this file is
//!
//! The **set** the editor's session is meant to have, as data: the editor's server plus whatever the open project
//! declares, with a name for each in [`SessionServers`], which the self-test card prints so a person can read what their
//! session has. The project's half is read from **the files omp itself reads** ([`PROJECT_SERVERS_FILES`]), in omp's own
//! precedence, so the card cannot name a different set from the one the session starts. That was a real gap: the editor
//! used to read `<root>/.mcp.json` alone, and a project declaring its servers in `.omp/mcp.json` had them started anyway
//! and never named.
//!
//! # What it is not: stopping omp's machine-level discovery
//!
//! omp has no way to scope a session to a named set of servers, measured on **v18.22.0** (2026-11-18, each server a
//! `/bin/sh` that appends its tag to a file, so a line in the file proves the server was spawned). The earlier table was
//! taken on v18.16.0; every row was re-measured:
//!
//! | Attempt | Result |
//! |---|---|
//! | `enabledServers` in the user `mcp.json` (the `/mcp` toggle list) | not an allow-list: the machine's servers still start |
//! | `--no-extensions -e <dir>` | the folder's server started, **and every user-level server still started** |
//! | `--config` overlay with `disabledExtensions: ["mcp:*"]` | a glob is not matched; both servers started |
//! | `--config` overlay with `disabledExtensions: ["mcp:<name>"]` | **works**, one name at a time |
//! | `--config` overlay with `disabledProviders: [native, mcp-json, …]` | **works**, and drops project *and* user files |
//! | `--config` overlay with `mcp.enableProjectConfig: false` | drops the four project files, **keeps the user's** |
//! | `--profile <name>` | the profile's own `mcp.json` **and** the user-level one |
//! | `PI_CODING_AGENT_DIR=<dir>` | **scopes discovery** to `<dir>/mcp.json` — and takes the credential store with it, so a dir without credentials fails `No API key found` |
//! | `--only-mcp`, `--mcp-config`, `--mcp-servers`, `--mcp` | `unknown flag`: none of the flags the upstream request asks for exists |
//!
//! The two levers that work both need every name omp would otherwise discover, which means re-implementing omp's twelve
//! discovery providers (native, `mcp-json`, claude, cursor, vscode, gemini, opencode, codex, windsurf, plugins…) inside the
//! editor: that is the wrapper this issue refuses. The request is filed on omp's own repository
//! (**can1357/oh-my-pi#19573**), and the editor's own half is done and tested here regardless.
//!
//! # What the editor promises until then
//!
//! Whatever omp discovers, the editor's session **is** these servers: the card names them (so a person reads the
//! set rather than guessing), and the project's servers are written into the extension folder's
//! `.mcp.json` beside the editor's, so the project's declaration is what the session has rather than something
//! omp has to be trusted to find.
//!
//! A machine-level entry can still shadow the editor's own server: omp keeps the first definition of a name it finds, and a
//! `dark_factory` in `~/.omp/agent/mcp.json` beats the one in the `-e` folder (measured). Nothing the editor can write
//! prevents that, which is what the upstream request is for.

use std::path::Path;

use bevy::prelude::*;
use serde_json::{Map, Value, json};

use crate::terminal_agent;

/// **How many declared servers a session may add.** A project that names more than this is refused rather than
/// half-loaded: a cap keeps the card readable and a typo from writing a hundred processes, and is far above any
/// project that has ever needed one.
pub const PROJECT_SERVERS_MAX: usize = 16;

/// **A project's declared servers file, at the project's root**: the fourth of the four omp reads ([`PROJECT_SERVERS_FILES`]),
/// and the one this crate has read since #484's first pass.
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

    /// **The row's sentence**: the names written into this session's config, or why there is only one.
    ///
    /// **"written for this session", not "has"**: omp also loads the machine's own servers (measured on v18.22.0: every row of
    /// the table in this module's header), so a row claiming the session *has* only these would be false. What the editor can
    /// promise is what it wrote, and the difference is requested upstream (`can1357/oh-my-pi#19573`).
    pub fn words(&self) -> String {
        let names = self.names();
        if names.is_empty() {
            return "none written".to_owned();
        }
        let mut said = names.join(", ");
        if self.project.is_empty() {
            said.push_str(" (no project declares another)");
        }
        said
    }
}

/// **The files a project declares its own MCP servers in**, in the order omp takes a name from them: measured on omp
/// 18.22.0 by declaring the same server name in two files at once and reading which one spawned.
///
/// All four are read, and a name declared in more than one belongs to the earliest file here. A project that uses any of
/// them is a project whose servers the session starts, so every one is read here too: reading [`PROJECT_SERVERS_FILE`]
/// alone left a project's `.omp/mcp.json` servers started and never named on the card.
pub const PROJECT_SERVERS_FILES: [&str; 4] = [".omp/mcp.json", ".omp/.mcp.json", "mcp.json", PROJECT_SERVERS_FILE];

/// **The names a project declares**, sorted and de-duplicated, or nothing when it declares none or no file reads.
///
/// A file that does not read is **not** a refusal of the launch: a project's server list is a convenience, and a
/// malformed one must not stop a person's agent from starting. It is said on the card ([`project_file_problem`] names it),
/// so a typo is visible rather than silent. Past [`PROJECT_SERVERS_MAX`] the extra names are dropped for the same reason:
/// the launch goes on, the card says how many were read.
pub fn project_server_names(root: &Path) -> Vec<String> {
    let mut names = declared_names(root);
    names.truncate(PROJECT_SERVERS_MAX);
    names.sort();
    names
}

/// **Every server the project declares**, by omp's precedence: a name's first declaration wins, and the editor's own name is
/// never one of them (the editor's server is not a project's to define, and [`session_config`] refuses to replace it).
fn declared_names(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for file in PROJECT_SERVERS_FILES {
        let Some(servers) = declared_in(root, file) else {
            continue;
        };
        for name in servers.keys() {
            if name.as_str() != terminal_agent::MCP_SERVER_NAME && !names.iter().any(|seen| seen == name) {
                names.push(name.clone());
            }
        }
    }
    names
}

/// The `mcpServers` a project file declares, or `None` when the file does not exist or does not read.
fn declared_in(root: &Path, file: &str) -> Option<Map<String, Value>> {
    let text = std::fs::read_to_string(root.join(file)).ok()?;
    let config: Value = serde_json::from_str(&text).ok()?;
    config.get("mcpServers").and_then(Value::as_object).cloned()
}

/// **Why the project's files gave no servers**, when one exists but could not be read: what the card says instead of
/// pretending the project declared nothing. Every file that is a problem is named, because a person fixing a typo needs to
/// know which file has it.
pub fn project_file_problem(root: &Path) -> Option<String> {
    let mut problems: Vec<String> = Vec::new();
    for file in PROJECT_SERVERS_FILES {
        let path = root.join(file);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                problems.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(config) => {
                let declared = config.get("mcpServers").and_then(Value::as_object).map_or(0, Map::len);
                if declared > PROJECT_SERVERS_MAX {
                    problems.push(format!(
                        "{} declares {declared} servers; a session takes the first {PROJECT_SERVERS_MAX}",
                        path.display()
                    ));
                }
            }
            Err(e) => problems.push(format!("{}: {e}", path.display())),
        }
    }
    // **One problem a line** (#519): these were joined with a middle dot, so two broken files read as
    // one sentence with two caret-separated halves. They are two values — a file and what is wrong
    // with it — and the card that shows them draws one per line.
    (!problems.is_empty()).then(|| problems.join("\n"))
}

/// **The config an omp session is handed**: the editor's server, then each server the project declares, under its own
/// name, from the file omp would take it from.
///
/// **Written on every launch**, because the editor's server carries a token minted per launch ([`terminal_agent::plan`]):
/// a config file kept from an earlier launch would hand the agent a token no tab holds.
pub fn session_config(editor: &Value, project: Option<&Path>) -> Value {
    let mut servers = editor.get("mcpServers").and_then(Value::as_object).cloned().unwrap_or_default();
    if let Some(root) = project {
        let mut written = 0;
        for file in PROJECT_SERVERS_FILES {
            if written >= PROJECT_SERVERS_MAX {
                break;
            }
            let Some(servers_in) = declared_in(root, file) else {
                continue;
            };
            for (name, config) in &servers_in {
                if written >= PROJECT_SERVERS_MAX {
                    break;
                }
                // **The editor's own name is the editor's**: a project that declares a `dark_factory` server would otherwise
                // replace the one server the session cannot do without. An earlier file's name still stands over a later one.
                if name != terminal_agent::MCP_SERVER_NAME && !servers.contains_key(name) {
                    servers.insert(name.clone(), config.clone());
                    written += 1;
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn root(tag: &str) -> std::path::PathBuf {
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
        assert_eq!(servers.names(), ["dark_factory", "alpha", "zeta"], "the editor's server first, then the project's alphabetically");
        assert!(servers.words().starts_with("dark_factory, alpha, zeta"), "{}", servers.words());
        let alone = SessionServers::of(None);
        assert_eq!(alone.names(), ["dark_factory"], "a launch with no project has the editor's server alone");
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
        let editor = json!({"mcpServers": {"dark_factory": {"type":"stdio","command":"/bin/dark_factory","env":{"DARK_FACTORY_AGENT_TOKEN":"t"}}}});
        let config = session_config(&editor, Some(&dir));
        let servers = config["mcpServers"].as_object().expect("servers");
        let mut names: Vec<&str> = servers.keys().map(String::as_str).collect();
        names.sort();
        assert_eq!(names, ["dark_factory", "shared"], "exactly the editor's server and the project's: {config}");
        assert_eq!(servers["dark_factory"]["env"]["DARK_FACTORY_AGENT_TOKEN"], "t", "the editor's server keeps its token");
        assert_eq!(servers["shared"]["command"], "/bin/echo");
    }

    /// **A project cannot take the editor's own name**, or the one server the session cannot do without would be replaced.
    #[test]
    fn a_project_that_declares_the_editors_name_does_not_replace_it() {
        let dir = root("shadow");
        std::fs::write(
            dir.join(PROJECT_SERVERS_FILE),
            r#"{"mcpServers": {"dark_factory": {"type":"stdio","command":"/bin/impostor"}, "mine": {"type":"stdio","command":"/bin/mine"}}}"#,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let editor = json!({"mcpServers": {"dark_factory": {"type":"stdio","command":"/bin/dark_factory"}}});
        let config = session_config(&editor, Some(&dir));
        assert_eq!(config["mcpServers"]["dark_factory"]["command"], "/bin/dark_factory", "the editor's server stands: {config}");
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
        assert_eq!(servers.names(), ["dark_factory"], "the editor's server still starts");
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

    /// **The files read are omp's four, in omp's precedence**: measured on omp 18.22.0 by declaring one name in two files and
    /// reading which one spawned, so a change to omp's list is a change to this constant and to this test rather than a silent
    /// under-report on the card.
    #[test]
    fn the_files_read_are_omps_four_in_omps_precedence() {
        assert_eq!(PROJECT_SERVERS_FILES, [".omp/mcp.json", ".omp/.mcp.json", "mcp.json", ".mcp.json"]);
        let dir = root("precedence");
        for (file, command) in [
            (".mcp.json", "/bin/from_dot_mcp"),
            (".omp/mcp.json", "/bin/from_omp_mcp"),
        ] {
            let path = dir.join(file);
            std::fs::create_dir_all(path.parent().expect("a parent")).unwrap_or_else(|e| panic!("{e}"));
            std::fs::write(&path, json!({"mcpServers": {"shared": {"type": "stdio", "command": command}}}).to_string())
                .unwrap_or_else(|e| panic!("{e}"));
        }
        let editor = json!({"mcpServers": {"dark_factory": {"type": "stdio", "command": "/bin/dark_factory"}}});
        let config = session_config(&editor, Some(&dir));
        assert_eq!(
            config["mcpServers"]["shared"]["command"], "/bin/from_omp_mcp",
            "`.omp/mcp.json` is the file omp takes the name from: {config}"
        );
        assert_eq!(project_server_names(&dir), ["shared"], "one server of that name, named once");
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
