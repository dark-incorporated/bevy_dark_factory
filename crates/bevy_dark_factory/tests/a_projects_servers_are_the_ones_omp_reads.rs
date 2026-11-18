//! **The project's servers a session has are the ones omp actually reads** (#484, `feedback-1794783878399`).
//!
//! The owner: *"Starting an agent should not automatically start all MCPs. The game editor should have its own MCP detection
//! and enablement."*
//!
//! # The gap this closes, measured
//!
//! The editor used to read one file, `<root>/.mcp.json`. omp v18.22.0 reads **four**, and a project that uses any of the
//! other three gets servers the session starts and the self-test card never names. Measured on omp 18.22.0 (macOS, each
//! server a `/bin/sh` that echoes its tag to a file, so a line proves it was spawned):
//!
//! | File, relative to the project root | Read by omp |
//! |---|---|
//! | `.omp/mcp.json` | yes, and it wins a name over the other three |
//! | `.omp/.mcp.json` | yes |
//! | `mcp.json` | yes |
//! | `.mcp.json` | yes |
//!
//! Precedence, measured pairwise on the same server name: `.omp/mcp.json` > `.omp/.mcp.json` > `mcp.json` > `.mcp.json`;
//! and a server in a project file still beats one of the same name in the `-e` folder omp was given.
//!
//! # What that means for the editor's promise
//!
//! The session's set is what omp will load from the project, so the editor reads the same four files in the same order, and
//! the extension folder's `.mcp.json` it writes carries the winning definition of each name — the editor's own server first
//! and never overridden. A name a project declares in two files is written once, from the file omp would have taken it from,
//! so the card's list and the session's config cannot disagree.

use std::path::Path;

use bevy_dark_factory::agent_servers::{self, SessionServers};
use bevy_dark_factory::settings::TerminalAgent;
use bevy_dark_factory::terminal_agent::{self, AgentSetup, MCP_SERVER_NAME};
use serde_json::Value;

/// The editor's server, as the config spells it.
const EDITOR: &str = MCP_SERVER_NAME;

/// **The four files this test expects the editor to read**, written here rather than imported so the expectation is the test's
/// own: if the library's list drifts from omp's, this is what says so. Measured on omp 18.22.0 (the table in the file header).
const OMP_PROJECT_MCP_FILES: [&str; 4] = [".omp/mcp.json", ".omp/.mcp.json", "mcp.json", ".mcp.json"];

fn setup(tag: &str) -> AgentSetup {
    let root = std::env::temp_dir().join(format!("project_servers_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
    let server = root.join("dark_factory_mcp");
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    AgentSetup { port: Some(15799), server, files: root.join("agents") }
}

/// A project root whose files declare the given servers, keyed by path under the root.
fn project(tag: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("project_servers_proj_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
    for (name, declared) in files {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|e| panic!("{}: {e}", parent.display()));
        }
        std::fs::write(&path, declared).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
    root
}

/// **Every file omp reads is read here too**, so a project that declares its server in `.omp/mcp.json` gets it in the session
/// *and* on the card — not silently started and unnamed.
#[test]
fn a_server_declared_in_any_of_omps_project_files_is_in_the_session_and_named_on_the_card() {
    for file in OMP_PROJECT_MCP_FILES {
        let setup = setup(&format!("each{}", file.replace(['/', '.'], "_")));
        let root = project(
            &format!("each{}", file.replace(['/', '.'], "_")),
            &[(file, r#"{"mcpServers": {"from_project": {"type": "stdio", "command": "/bin/from_project"}}}"#)],
        );
        let launch = terminal_agent::plan(TerminalAgent::OhMyPi, None, Some(&setup), Some(&root), 0.0);
        let text = std::fs::read_to_string(setup.files.join("omp").join(".mcp.json")).unwrap_or_else(|e| panic!("{e}"));
        let config: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
        let mut names: Vec<&str> = config["mcpServers"].as_object().expect("servers").keys().map(String::as_str).collect();
        names.sort();
        assert_eq!(names, ["dark_factory", "from_project"], "{file} declares a server, so the session carries it: {text}");
        assert_eq!(
            launch.post.servers,
            Some(SessionServers { editor: true, project: vec!["from_project".to_owned()] }),
            "{file}: the card's set is the config's: {:?}",
            launch.post.servers
        );
        let row = launch.post.servers_row().expect("the card has its row");
        assert!(row.contains("from_project"), "{file}: the card names it: {row}");
    }
}

/// **A name declared in two project files is one server, from the file omp would take it from**, so the config the session
/// gets is the server omp would have loaded rather than a silently different one.
#[test]
fn a_name_in_two_project_files_is_written_once_from_the_file_omp_prefers() {
    let setup = setup("precedence");
    let root = project(
        "precedence",
        &[
            (".mcp.json", r#"{"mcpServers": {"shared": {"type": "stdio", "command": "/bin/from_dot_mcp"}}}"#),
            (".omp/mcp.json", r#"{"mcpServers": {"shared": {"type": "stdio", "command": "/bin/from_omp_mcp"}}}"#),
        ],
    );
    let launch = terminal_agent::plan(TerminalAgent::OhMyPi, None, Some(&setup), Some(&root), 0.0);
    let text = std::fs::read_to_string(setup.files.join("omp").join(".mcp.json")).unwrap_or_else(|e| panic!("{e}"));
    let config: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
    assert_eq!(
        config["mcpServers"]["shared"]["command"], "/bin/from_omp_mcp",
        "`.omp/mcp.json` is the file omp takes the name from: {text}"
    );
    assert_eq!(config["mcpServers"].as_object().map(|s| s.len()), Some(2), "one server of that name and the editor's: {text}");
    let names = launch.post.servers.as_ref().expect("the set").names();
    assert_eq!(names, ["dark_factory", "shared"], "the card names it once");
}

/// **The editor's own server stands against all four files**: a project cannot shadow the one server the session cannot do
/// without, wherever it declares it.
#[test]
fn no_project_file_replaces_the_editors_server() {
    for file in OMP_PROJECT_MCP_FILES {
        let tag = format!("shadow{}", file.replace(['/', '.'], "_"));
        let setup = setup(&tag);
        let root = project(&tag, &[(file, r#"{"mcpServers": {"dark_factory": {"type": "stdio", "command": "/bin/impostor"}}}"#)]);
        let launch = terminal_agent::plan(TerminalAgent::OhMyPi, None, Some(&setup), Some(&root), 0.0);
        let text = std::fs::read_to_string(setup.files.join("omp").join(".mcp.json")).unwrap_or_else(|e| panic!("{e}"));
        let config: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
        assert_ne!(config["mcpServers"][EDITOR]["command"], "/bin/impostor", "{file} cannot take the editor's name: {text}");
        let row = launch.post.servers_row().expect("the card has its row");
        assert_eq!(row.matches(EDITOR).count(), 1, "{file}: the card names the editor's server once: {row}");
    }
}

/// **A machine-level config is not one of the files read**, and its names are not in the session's set: the editor reads the
/// project's four files and nothing else, so a `~/.omp/agent/mcp.json` entry can only reach the card by the editor inventing
/// it. Proved by a machine-shaped file *inside* the project (a `home` directory holding the machine's own config): it is not
/// one of the four paths, so it declares nothing.
#[test]
fn a_machine_level_config_is_not_read() {
    let setup = setup("machine");
    let root = project(
        "machine",
        &[
            (".mcp.json", r#"{"mcpServers": {"mine": {"type": "stdio", "command": "/bin/mine"}}}"#),
            (
                "home/.omp/agent/mcp.json",
                r#"{"mcpServers": {"imessage-search": {"type": "stdio", "command": "/bin/imessage"}}}"#,
            ),
        ],
    );
    let launch = terminal_agent::plan(TerminalAgent::OhMyPi, None, Some(&setup), Some(&root), 0.0);
    let text = std::fs::read_to_string(setup.files.join("omp").join(".mcp.json")).unwrap_or_else(|e| panic!("{e}"));
    let config: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"));
    let mut names: Vec<&str> = config["mcpServers"].as_object().expect("servers").keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, ["dark_factory", "mine"], "exactly the editor's server and the project's: {text}");
    let row = launch.post.servers_row().expect("the card has its row");
    assert!(!row.contains("imessage-search"), "a machine-level server is not on the card: {row}");
}

/// **A file that does not read names itself and does not stop the launch**, whichever of the four it is.
#[test]
fn a_project_file_that_does_not_read_is_named_and_the_launch_goes_on() {
    for file in OMP_PROJECT_MCP_FILES {
        let tag = format!("broken{}", file.replace(['/', '.'], "_"));
        let setup = setup(&tag);
        let root = project(&tag, &[(file, "{not json")]);
        let launch = terminal_agent::plan(TerminalAgent::OhMyPi, None, Some(&setup), Some(&root), 0.0);
        let row = launch.post.servers_row().expect("the card still has its row");
        assert!(row.contains(file), "{file} is named on the card: {row}");
        assert!(!String::from_utf8_lossy(&launch.line).is_empty(), "{file}: the launch still happens");
        assert_eq!(launch.post.servers, Some(SessionServers { editor: true, project: Vec::new() }));
    }
}

/// **The set and the config agree and repeat**, so a person reading the card and the agent reading the file cannot be told two
/// different things.
#[test]
fn the_set_and_the_config_agree_and_repeat() {
    let root = project("repeat", &[(".mcp.json", r#"{"mcpServers": {"b": {"command": "/bin/b"}, "a": {"command": "/bin/a"}}}"#)]);
    assert_eq!(agent_servers::project_server_names(&root), ["a", "b"], "sorted, so the card's order is stable");
    let mut editor = serde_json::Map::new();
    editor.insert(EDITOR.to_owned(), serde_json::json!({"type": "stdio", "command": "/bin/editor"}));
    let editor = serde_json::json!({ "mcpServers": serde_json::Value::Object(editor) });
    let config = agent_servers::session_config(&editor, Some(Path::new(&root)));
    let mut names: Vec<&str> = config["mcpServers"].as_object().expect("servers").keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, ["a", "b", EDITOR], "the config carries exactly the set the reader names: {config}");
}
