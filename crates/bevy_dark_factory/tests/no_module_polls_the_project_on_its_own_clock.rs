//! **No module polls the project on its own clock** (#501): a ratchet that only turns one way.
//!
//! The project index (`project_index.rs`) is the one background watcher; a panel that wants to know "did this file change?" subscribes
//! to it. #501's checklist (`docs/guide/reload-contract.md`, "The one watcher") is retired down to the two `game_view` clocks;
//! every other module subscribes. This test **fails when a poll constant of the checklist's shapes exists in `src/` that is not on [`KEPT`]**, so no
//! new one can be added while the old ones are retired. It also **fails when an entry on [`KEPT`] names a constant the tree does not
//! declare**, because a list of retired names is a claim nobody checks — the shape this file was written to stop. The tally it prints
//! is counted from the constants it finds, so the number in a log line can never describe a past state.
//!
//! # What counts as a poll constant
//!
//! A `const` or `static` whose name has one of the checklist's shapes ([`is_a_poll_clock`]): `POLL_S` or `*_POLL_S`, `*POLL_SECS`,
//! `*RECHECK_SECS`, `*READ_SECS`, `*LOOK_EVERY*`. Every one of those is a **time in seconds** a module waits before it looks at a file again.
//!
//! # What is not flagged, and how it is told apart
//!
//! It is told apart **by name shape alone**, and nothing is excluded by file or folder: the terminal's PTY and exit timings are in milliseconds or a
//! `Duration` and are named for that (`QUIT_POLL_MS`, `HANGUP_POLL_MS`, `LOGIN_ASK_EXIT_POLL_MS`, `READY_POLL`, `watch.rs`'s `POLL`), the
//! caret blink and the collection readout are not `*_POLL_S`, `*READ_SECS` or `*LOOK_EVERY*` (`COLLECTION_READOUT_EVERY`,
//! `PROGRESS_EVERY`). The scan reads every `.rs` file under `src/`, `src/terminal/` included (it has no constant of these shapes today).
//! **It is a line scan**: a `const` after an attribute on the same line, or inside a `/* */` block, is not seen, and a constant in a
//! `mod tests` is flagged like any other. **A poll written in a different shape (a `Duration`, or `0.5` written in place) is not seen by
//! this test**: it is a ratchet on the checklist's shapes, not a proof that nothing polls.

use std::path::{Path, PathBuf};

/// **A name the checklist lists, the file it is in, and its item number** in `docs/guide/reload-contract.md`.
type Open = (&'static str, &'static str, &'static str);

/// **The poll constants that are deliberately kept**, and why each one is not a project file on a clock. Every other item on
/// #501's checklist was retired by moving its reader onto `project_index`, so this list holds what is left and nothing that has
/// gone: an entry naming a constant the tree does not declare would be a claim nobody can check, which is the whole class of
/// defect this file exists to stop. The tally the test prints is **counted from the constants it finds**, never written here.
const KEPT: &[Open] = &[
    ("game_view.rs", "LOG_POLL_S", "item 17: the game's log is outside the project root, so no watcher walks it"),
    ("game_view.rs", "STATE_POLL_S", "item 13: the game's state is asked over a socket, which no file says moved"),
];

/// **Whether a constant's name has one of the checklist's poll shapes.**
fn is_a_poll_clock(name: &str) -> bool {
    name == "POLL_S" || name.ends_with("_POLL_S") || name.ends_with("POLL_SECS") || name.ends_with("RECHECK_SECS") || name.ends_with("READ_SECS") || name.contains("LOOK_EVERY")
}

/// **The name a line declares as a `const` or `static`**, if it does: `pub const POLL_S: f32 = 1.0;` gives `POLL_S`.
fn declared_name(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    if rest.starts_with("//") {
        return None;
    }
    if let Some(after) = rest.strip_prefix("pub") {
        rest = match after.strip_prefix('(') {
            Some(scoped) => scoped.split_once(')')?.1,
            None => after,
        }
        .trim_start();
    }
    let rest = rest.strip_prefix("const ").or_else(|| rest.strip_prefix("static "))?;
    let name = rest.split(':').next()?.trim();
    (!name.is_empty() && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')).then_some(name)
}

/// Every `.rs` file under `dir`.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    for entry in entries {
        let path = entry.unwrap_or_else(|e| panic!("{}: {e}", dir.display())).path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// **Every poll constant in `files`**, as `(file name, constant, line)`, the line counted from 1.
fn poll_clocks(files: &[PathBuf]) -> Vec<(String, String, usize)> {
    let mut found = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let file = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
        for (n, line) in text.lines().enumerate() {
            if let Some(name) = declared_name(line).filter(|name| is_a_poll_clock(name)) {
                found.push((file.clone(), name.to_owned(), n + 1));
            }
        }
    }
    found
}

/// **The poll constants that are on neither list**, as `(file, name, line)`: what fails the ratchet.
fn unlisted(found: &[(String, String, usize)]) -> Vec<&(String, String, usize)> {
    let allowed = |file: &str, name: &str| KEPT.iter().any(|(f, n, _)| *f == file && *n == name);
    found.iter().filter(|(file, name, _)| !allowed(file, name)).collect()
}

/// **No new poll constant**: every one found is on the list.
#[test]
fn no_module_polls_the_project_on_its_own_clock() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let found = poll_clocks(&files);
    let new: Vec<String> = unlisted(&found).iter().map(|(file, name, line)| format!("  src/**/{file}:{line}: `{name}`")).collect();
    let kept = found.iter().filter(|(file, name, _)| KEPT.iter().any(|(f, n, _)| f == file && n == name)).count();
    // **Counted from the constants found**, never from a number written here: a hand-written total describes the day it was typed,
    // and this file exists because a count nobody checks is a count that is wrong once and stays wrong (#547).
    let tally = format!("{} poll constants of the checklist's shapes are declared in src/, and {kept} of them are kept on purpose", found.len());
    println!("{tally}");
    assert!(
        new.is_empty(),
        "{tally}.\n\nA poll constant that is not on the list of #501 was added:\n{}\n\nA module that wants to know a project file changed subscribes to \
         `project_index::ProjectFileChanged` (see `docs/guide/reload-contract.md`, \"The one watcher\"); it does not keep its own timer. \
         If this is not a poll of the project, name the constant so it does not have a poll's shape.",
        new.join("\n")
    );
    assert_eq!(kept, found.len(), "{tally}: a poll constant of these shapes is declared in src/ that `KEPT` does not name, so the exception list has fallen behind the tree");
}

/// **The matcher sees the shapes and only them**, so the ratchet cannot pass by matching nothing.
#[test]
fn the_matcher_sees_the_checklists_shapes_and_not_the_pty_timings() {
    for name in ["POLL_S", "USED_BY_POLL_S", "POLL_SECS", "RIGS_RECHECK_SECS", "RECHECK_SECS", "PLAN_READ_SECS", "LOOK_EVERY", "LOOK_EVERY_S"] {
        assert!(is_a_poll_clock(name), "{name} is a poll clock");
    }
    for name in ["QUIT_POLL_MS", "HANGUP_POLL_MS", "LOGIN_ASK_EXIT_POLL_MS", "READY_POLL", "POLL", "COLLECTION_READOUT_EVERY", "PROGRESS_EVERY", "SWEEP"] {
        assert!(!is_a_poll_clock(name), "{name} is not");
    }
    assert_eq!(declared_name("pub const FOO_POLL_S: f32 = 1.0;"), Some("FOO_POLL_S"));
    assert_eq!(declared_name("    pub(crate) const RECHECK_SECS: f32 = 1.0;"), Some("RECHECK_SECS"));
    assert_eq!(declared_name("static LOOK_EVERY: f32 = 1.0;"), Some("LOOK_EVERY"));
    assert_eq!(declared_name("/// const FOO_POLL_S: f32 = 1.0;"), None);
    assert_eq!(declared_name("let const_poll = 1;"), None);

    // **The scan finds a constant in a file it reads**, in a folder of its own, so the ratchet cannot pass by reading nothing.
    let dir = std::env::temp_dir().join(format!("no_module_polls_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("terminal")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(dir.join("a.rs"), "const FOO_POLL_S: f32 = 1.0;\nconst QUIT_POLL_MS: u64 = 25;\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(dir.join("terminal/b.rs"), "const BAR_POLL_S: f32 = 1.0;\n").unwrap_or_else(|e| panic!("{e}"));
    let mut files = Vec::new();
    sources(&dir, &mut files);
    let mut found = poll_clocks(&files);
    std::fs::remove_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    found.sort();
    assert_eq!(
        found,
        vec![("a.rs".to_owned(), "FOO_POLL_S".to_owned(), 1), ("b.rs".to_owned(), "BAR_POLL_S".to_owned(), 1)],
        "the scan reads both files, a folder named `terminal` included, and finds the poll clocks and not the millisecond one"
    );
    // **The main assertion fails on a new constant and not on a kept one**: `unlisted` is what it asserts is empty.
    let fabricated = vec![
        ("zz_new_module.rs".to_owned(), "FOO_POLL_S".to_owned(), 1),
        ("game_view.rs".to_owned(), "LOG_POLL_S".to_owned(), 2),
        ("data_view.rs".to_owned(), "RECHECK_SECS".to_owned(), 3),
    ];
    assert_eq!(unlisted(&fabricated), vec![&fabricated[0], &fabricated[2]], "a new constant and a retired one are unlisted; the kept `game_view` one is not");
    assert_eq!(KEPT.len(), 2, "the two deliberate `game_view` exceptions, and nothing else");
}
