//! **Which agent session a project had running when Dark Factory was last open**, so the next launch can resume it
//! (`feedback-1794148425925`: *"If there was an agent in process, Dark Factory should remember it and restart there."*).
//!
//! One file for the person, `<data_dir>/agents/resume.json` ([`RESUME_FILE`] under [`crate::terminal_agent::AgentSetup::files`]),
//! holding at most one entry per project **and kind of agent** ([`AgentKind`]: the project's own, and the editor
//! development agent's): the harness that was running and the session it said it was in. An entry is
//! written when an agent tab reports its session, struck out when the person ends the agent (closes its tab, or leaves
//! the harness), and **left alone when the editor quits**, which is what makes it a record of what was running when
//! the editor closed.
//!
//! Only omp is remembered: it is the one harness whose sessions Dark Factory can name (`omp --resume <id>`); a custom command
//! has no resume syntax the editor knows.
//!
//! **Version 2 keys an entry by `(project, kind)`.** A version 1 file (one entry per project, no kind) is resume state, not the
//! person's work: it is dropped with one line in the log and the next record writes version 2.
//!
//! Every function takes the folder, so a test owns its own and nothing here reads the real data folder.

use std::path::{Path, PathBuf};

use bevy::log::info;
use serde::{Deserialize, Serialize};

use crate::terminal_agent::AgentKind;

/// The file under the agents folder.
pub const RESUME_FILE: &str = "resume.json";
/// The file's own version; one this build does not know is refused rather than guessed at.
const VERSION: u32 = 2;
/// The version before agents had a kind: dropped, not read ([`read`]).
const VERSION_WITHOUT_KINDS: u32 = 1;
/// The longest session id taken. omp mints UUIDs (36 characters).
const SESSION_ID_MAX: usize = 64;

/// **One project's remembered agent.**
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The project's root, as the editor held it when the agent was started.
    pub project: PathBuf,
    /// Which agent: the project's, or the editor development agent.
    pub kind: AgentKind,
    /// The harness that was running: `omp`.
    pub harness: String,
    /// The session the harness said it was in.
    pub session: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    version: u32,
    agents: Vec<Entry>,
}

/// Only a file's version, read before the rest so a file of another shape is judged by it.
#[derive(Deserialize)]
struct Versioned {
    version: u32,
}

/// **Is `id` a session id the editor will type into a shell?** Letters, digits and `-`, as omp's UUIDs are: the id
/// is typed at a prompt, so anything else is refused, not quoted around.
pub fn is_session_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= SESSION_ID_MAX && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn read(files: &Path) -> Result<File, String> {
    let path = files.join(RESUME_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(File { version: VERSION, agents: Vec::new() }),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let versioned: Versioned = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if versioned.version == VERSION_WITHOUT_KINDS {
        info!("{}: a version {VERSION_WITHOUT_KINDS} resume record has no kinds of agent, so it is dropped; the next record writes version {VERSION}", path.display());
        return Ok(File { version: VERSION, agents: Vec::new() });
    }
    if versioned.version != VERSION {
        return Err(format!("{}: version {} is not one this build reads ({VERSION})", path.display(), versioned.version));
    }
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn write(files: &Path, file: &File) -> Result<(), String> {
    let path = files.join(RESUME_FILE);
    let text = serde_json::to_string_pretty(file).map_err(|e| format!("{}: {e}", path.display()))? + "\n";
    std::fs::create_dir_all(files).map_err(|e| format!("{}: {e}", files.display()))?;
    blueprint_model::atomic_file::write(&path, &text).map_err(|e| format!("{}: {e}", path.display()))
}

/// **Write down that `harness` was running in `project` as a `kind` agent, in `session`.** Replaces that entry; every other
/// project's and kind's is kept. A malformed file is refused, not overwritten: it is the person's.
pub fn remember(files: &Path, project: &Path, kind: AgentKind, harness: &str, session: &str) -> Result<(), String> {
    if !is_session_id(session) {
        return Err(format!("{session:?} is not a session id"));
    }
    let mut file = read(files)?;
    let entry = Entry { project: project.to_path_buf(), kind, harness: harness.to_owned(), session: session.to_owned() };
    match file.agents.iter_mut().find(|e| e.project == project && e.kind == kind) {
        Some(held) if *held == entry => return Ok(()),
        Some(held) => *held = entry,
        None => file.agents.push(entry),
    }
    file.version = VERSION;
    write(files, &file)
}

/// **Strike out `project`'s `kind` entry**, if it has one.
pub fn forget(files: &Path, project: &Path, kind: AgentKind) -> Result<(), String> {
    let mut file = read(files)?;
    let before = file.agents.len();
    file.agents.retain(|e| !(e.project == project && e.kind == kind));
    if file.agents.len() == before {
        return Ok(());
    }
    write(files, &file)
}

/// **What `project` had running as a `kind` agent**, if anything.
pub fn recall(files: &Path, project: &Path, kind: AgentKind) -> Result<Option<Entry>, String> {
    Ok(read(files)?.agents.into_iter().find(|e| e.project == project && e.kind == kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("agent_resume_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_project_gets_back_the_session_it_had_and_another_project_is_untouched() {
        let files = folder("round");
        let (a, b) = (Path::new("/p/a"), Path::new("/p/b"));
        let project = AgentKind::Project;
        assert_eq!(recall(&files, a, project), Ok(None), "nothing is remembered before anything is written");
        remember(&files, a, project, "omp", "0197-aaa").unwrap_or_else(|e| panic!("{e}"));
        remember(&files, b, project, "omp", "0197-bbb").unwrap_or_else(|e| panic!("{e}"));
        remember(&files, a, project, "omp", "0197-ccc").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(recall(&files, a, project).map(|e| e.map(|e| e.session)), Ok(Some("0197-ccc".to_owned())), "a newer session replaces the project's");
        assert_eq!(recall(&files, b, project).map(|e| e.map(|e| e.session)), Ok(Some("0197-bbb".to_owned())));
        forget(&files, a, project).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(recall(&files, a, project), Ok(None));
        assert!(recall(&files, b, project).is_ok_and(|e| e.is_some()), "striking one project out keeps the other");
        let _ = std::fs::remove_dir_all(&files);
    }

    /// **The project's agent and the editor development agent of one project are two records**: neither replaces nor
    /// strikes out the other.
    #[test]
    fn the_two_kinds_of_agent_in_one_project_keep_their_own_sessions() {
        let files = folder("kinds");
        let p = Path::new("/p/a");
        remember(&files, p, AgentKind::Project, "omp", "0197-aaa").unwrap_or_else(|e| panic!("{e}"));
        remember(&files, p, AgentKind::EditorDev, "omp", "0197-ddd").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(recall(&files, p, AgentKind::Project).map(|e| e.map(|e| e.session)), Ok(Some("0197-aaa".to_owned())));
        assert_eq!(recall(&files, p, AgentKind::EditorDev).map(|e| e.map(|e| e.session)), Ok(Some("0197-ddd".to_owned())));
        forget(&files, p, AgentKind::EditorDev).unwrap_or_else(|e| panic!("{e}"));
        assert!(recall(&files, p, AgentKind::Project).is_ok_and(|e| e.is_some()), "striking one kind out keeps the other");
        let _ = std::fs::remove_dir_all(&files);
    }

    /// **A version 1 record is dropped, not refused**: it is resume state from before agents had kinds, and the next record
    /// writes version 2 over it.
    #[test]
    fn a_version_1_record_is_dropped_and_written_again_as_version_2() {
        let files = folder("v1");
        std::fs::create_dir_all(&files).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(files.join(RESUME_FILE), r#"{"version": 1, "agents": [{"project": "/p", "harness": "omp", "session": "0197-old"}]}"#)
            .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(recall(&files, Path::new("/p"), AgentKind::Project), Ok(None), "a version 1 entry is not read");
        remember(&files, Path::new("/p"), AgentKind::Project, "omp", "0197-new").unwrap_or_else(|e| panic!("{e}"));
        let text = std::fs::read_to_string(files.join(RESUME_FILE)).unwrap_or_default();
        assert!(text.contains("\"version\": 2") && !text.contains("0197-old"), "the record is version 2 now: {text}");
        let _ = std::fs::remove_dir_all(&files);
    }

    #[test]
    fn an_id_that_is_not_a_session_id_is_refused_and_a_damaged_file_is_not_overwritten() {
        let files = folder("refuse");
        let project = AgentKind::Project;
        for bad in ["", "a b", "x;rm", "it's", &"a".repeat(SESSION_ID_MAX + 1)] {
            assert!(remember(&files, Path::new("/p"), project, "omp", bad).is_err(), "{bad:?} is typed into a shell, so it is refused");
        }
        assert!(!files.join(RESUME_FILE).exists(), "nothing was written for a refused id");
        std::fs::create_dir_all(&files).unwrap_or_else(|e| panic!("{e}"));
        std::fs::write(files.join(RESUME_FILE), "{ not json").unwrap_or_else(|e| panic!("{e}"));
        assert!(recall(&files, Path::new("/p"), project).is_err(), "a damaged file is said so, not read as empty");
        assert!(remember(&files, Path::new("/p"), project, "omp", "0197-aaa").is_err());
        assert_eq!(std::fs::read_to_string(files.join(RESUME_FILE)).unwrap_or_default(), "{ not json", "and it is left as it was");
        let _ = std::fs::remove_dir_all(&files);
    }
}
