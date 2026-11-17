//! **An agent session gets the editor's MCP server and the project's, and says which** (#484, `feedback-1794783878399`).
//!
//! The owner, looking at the terminal pane: *"Starting an agent should not automatically start all MCPs. The game editor
//! should have its own MCP detection and enablement."*
//!
//! # What the file the editor writes carries
//!
//! The omp extension folder's `.mcp.json` is the config omp reads for a session the editor started. It carries the editor's
//! own server plus each server the open project declares in its own `.mcp.json`, under the project's names, and nothing
//! else. A machine-level entry (the kind in `~/.omp/agent/mcp.json`) is not in it — the editor never writes one, and the
//! test proves a config with one in it does not reach the session.
//!
//! # What the card says
//!
//! The self-test card names the session's servers, so "which servers does this session have" is answered on screen rather
//! than guessed at from a file.
//!
//! # What is blocked, and where it is asked
//!
//! Stopping omp from *also* loading its machine-level servers needs a launch flag omp does not have at v18.16.0. That is
//! measured (see `docs`, and the table in `crates/bevy_cauldron/src/agent_servers.rs`) and the request is filed on omp's
//! own repository, **can1357/oh-my-pi#19573**. The editor does not re-implement omp's eight-source discovery to name every
//! server it would otherwise load: that is the wrapper this issue refuses.

use std::path::Path;

use bevy_cauldron::agent_servers::{self, PROJECT_SERVERS_FILE, SessionServers};
use bevy_cauldron::settings::TerminalAgent;
use bevy_cauldron::terminal_agent::{self, AgentSetup, MCP_SERVER_NAME};
use serde_json::Value;

/// The editor's server, as the config spells it.
const EDITOR: &str = MCP_SERVER_NAME;

/// A setup in a fresh temporary folder, with a file standing in for the server binary, named for this test and this process.
fn setup(tag: &str) -> AgentSetup {
    let root = std::env::temp_dir().join(format!("agent_servers_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
    let server = root.join("bevy_cauldron_mcp");
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    AgentSetup { port: Some(15799), server, files: root.join("agents") }
}

/// A project root holding the servers it declares.
fn project(tag: &str, declared: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("agent_servers_proj_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
    std::fs::write(root.join(PROJECT_SERVERS_FILE), declared).unwrap_or_else(|e| panic!("{e}"));
    root
}

/// **The `.mcp.json` an omp start writes carries exactly the editor's server and the project's, and a machine-level entry is
/// not in it.**
#[test]
fn the_written_session_config_carries_the_editors_and_the_projects_servers_and_no_machine_level_one() {
    let setup = setup("written");
    // A project that declares two servers of its own, and a project file carrying the machine-level names a person's
    // `~/.omp/agent/mcp.json` has, so this asserts on the text rather than on the editor's good intentions.
    let root = project(
        "declares",
        r#"{"mcpServers": {
            "alpha": {"type": "stdio", "command": "/bin/alpha", "args": ["--one"]},
            "zeta": {"type": "stdio", "command": "/bin/zeta"}
        }}"#,
    );
    let launch = terminal_agent::plan(TerminalAgent::OhMyPi, None, Some(&setup), Some(&root), 0.0);
    let omp = setup.files.join("omp");
    let text = std::fs::read_to_string(omp.join(".mcp.json")).unwrap_or_else(|e| panic!("{e}"));
    let config: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
    let mut names: Vec<&str> = config["mcpServers"].as_object().expect("servers").keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, ["alpha", EDITOR, "zeta"], "exactly the editor's server and the project's: {text}");
    // The editor's server keeps its own token, which is what makes the tab's work the tab's.
    assert_eq!(
        config["mcpServers"][EDITOR]["env"]["CAULDRON_AGENT_TOKEN"], launch.post.token.as_str(),
        "the editor's server carries the launch's token: {text}"
    );
    assert_eq!(config["mcpServers"]["alpha"]["command"], "/bin/alpha", "a project's server is written as the project declared it");
    // **Nothing else is in it**, which is the property, and it is checkable against a set computed here: the file's servers are
    // exactly the editor's own plus the project's declarations, so a server from anywhere else — the machine's `~/.omp/agent/mcp.json`,
    // another tool's config — could only appear by breaking that. (The editor never reads those files, so this asserts the universe
    // rather than names a file could not contain.)
    let mut expected = vec![EDITOR.to_owned(), "alpha".to_owned(), "zeta".to_owned()];
    expected.sort();
    let mut got: Vec<String> = names.iter().map(|name| (*name).to_owned()).collect();
    got.sort();
    assert_eq!(got, expected, "no server but the editor's and the project's: {text}");
    // And the plain harness config the same launch wrote is the editor's server alone, so nothing arrived through that door either.
    let plain = std::fs::read_to_string(setup.files.join("mcp.json")).unwrap_or_else(|e| panic!("{e}"));
    let plain: Value = serde_json::from_str(&plain).unwrap_or_else(|e| panic!("{e}: {plain}"));
    let plain_names: Vec<&str> = plain["mcpServers"].as_object().expect("servers").keys().map(String::as_str).collect();
    assert_eq!(plain_names, [EDITOR], "the config a custom command would read carries the editor's server alone: {plain}");
    assert_eq!(launch.post.servers, Some(SessionServers { editor: true, project: vec!["alpha".to_owned(), "zeta".to_owned()] }));
}

/// **A project that declares the editor's own name lists it once** (manager review of #484): `session_config` refuses to let a project
/// replace the editor's server, so the set the card prints must agree with the config rather than naming `cauldron` twice.
#[test]
fn a_project_that_declares_the_editors_name_lists_it_once() {
    let setup = setup("shadowed");
    let root = project("shadowed", r#"{"mcpServers": {"cauldron": {"type": "stdio", "command": "/bin/impostor"}, "mine": {"type": "stdio", "command": "/bin/mine"}}}"#);
    let launch = terminal_agent::plan(TerminalAgent::OhMyPi, None, Some(&setup), Some(&root), 0.0);
    assert_eq!(
        launch.post.servers,
        Some(SessionServers { editor: true, project: vec!["mine".to_owned()] }),
        "the project's own server is listed, its impostor of the editor's name is not"
    );
    let row = launch.post.servers_row().expect("the card has its row");
    assert_eq!(row.matches(EDITOR).count(), 1, "the card names the editor's server once: {row}");
    // And the config it wrote keeps the editor's own server, which is the fact the set follows.
    let text = std::fs::read_to_string(setup.files.join("omp").join(".mcp.json")).unwrap_or_else(|e| panic!("{e}"));
    let config: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
    assert_ne!(config["mcpServers"][EDITOR]["command"], "/bin/impostor", "the editor's server stands: {text}");
}

/// **A project's `.mcp.json` is what the session gets, and a launch with no project has the editor's server alone.**
#[test]
fn a_launch_with_no_project_has_the_editors_server_alone() {
    let setup = setup("alone");
    let launch = terminal_agent::plan(TerminalAgent::OhMyPi, None, Some(&setup), None, 0.0);
    let text = std::fs::read_to_string(setup.files.join("omp").join(".mcp.json")).unwrap_or_else(|e| panic!("{e}"));
    let config: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
    let names: Vec<&str> = config["mcpServers"].as_object().expect("servers").keys().map(String::as_str).collect();
    assert_eq!(names, [EDITOR], "one server: {text}");
    assert_eq!(launch.post.servers, Some(SessionServers { editor: true, project: Vec::new() }));
}

/// **The self-test card names the session's servers**, so a person reads them rather than guessing from a file.
#[test]
fn the_card_names_the_sessions_servers() {
    let setup = setup("card");
    let root = project("card", r#"{"mcpServers": {"shared": {"type": "stdio", "command": "/bin/shared"}}}"#);
    let launch = terminal_agent::plan(TerminalAgent::OhMyPi, None, Some(&setup), Some(&root), 0.0);
    let row = launch.post.servers_row().expect("the card has a row for the servers");
    assert!(row.starts_with(terminal_agent::AgentPost::SERVER_ROW_LABEL), "{row}");
    assert!(row.contains(EDITOR) && row.contains("shared"), "the row names both servers: {row}");
    assert!(!row.contains("imessage-search"), "and no machine-level server: {row}");
}

/// **A project file that does not read declares nothing, names itself on the card, and does not stop the launch.**
#[test]
fn a_project_file_that_does_not_read_is_named_and_the_editors_server_still_starts() {
    let setup = setup("broken");
    let root = project("broken", "{not json at all");
    let launch = terminal_agent::plan(TerminalAgent::OhMyPi, None, Some(&setup), Some(&root), 0.0);
    let row = launch.post.servers_row().expect("the card still has its row");
    assert!(row.contains(".mcp.json"), "the row names the file that did not read: {row}");
    assert!(row.contains(EDITOR), "and the editor's server, which starts anyway: {row}");
    let text = std::fs::read_to_string(setup.files.join("omp").join(".mcp.json")).unwrap_or_else(|e| panic!("{e}"));
    let config: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(config["mcpServers"].as_object().map(|s| s.len()), Some(1), "the editor's server alone: {text}");
    assert!(!String::from_utf8_lossy(&launch.line).is_empty(), "the launch still happens");
}

/// **The plain harness config is the editor's server alone**, because it is what a custom command is handed and that command's
/// own config is its business; the session set is the omp extension's file.
#[test]
fn a_custom_harness_gets_the_editors_server_config_and_not_the_projects() {
    let setup = setup("custom");
    let root = project("custom", r#"{"mcpServers": {"shared": {"type": "stdio", "command": "/bin/shared"}}}"#);
    terminal_agent::plan(TerminalAgent::Custom, Some("echo hi"), Some(&setup), Some(&root), 0.0);
    let text = std::fs::read_to_string(setup.files.join("mcp.json")).unwrap_or_else(|e| panic!("{e}"));
    let config: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
    let names: Vec<&str> = config["mcpServers"].as_object().expect("servers").keys().map(String::as_str).collect();
    assert_eq!(names, [EDITOR], "the plain config is the editor's server: {text}");
}

/// **The set is data, so the same project gives the same answer twice** — and the reader and the config do not disagree.
#[test]
fn the_set_and_the_config_agree_and_repeat() {
    let root = project("repeat", r#"{"mcpServers": {"b": {"type": "stdio", "command": "/bin/b"}, "a": {"type": "stdio", "command": "/bin/a"}}}"#);
    let first = agent_servers::project_server_names(&root);
    let second = agent_servers::project_server_names(&root);
    assert_eq!(first, second, "the same file gives the same set");
    assert_eq!(first, ["a", "b"], "sorted, so the card's order is stable");
    let editor = serde_json::json!({"mcpServers": {EDITOR: {"type": "stdio", "command": "/bin/editor"}}});
    let config = agent_servers::session_config(&editor, Some(Path::new(&root)));
    let mut names: Vec<&str> = config["mcpServers"].as_object().expect("servers").keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, ["a", "b", EDITOR], "the config carries exactly the set the reader names: {config}");
}
