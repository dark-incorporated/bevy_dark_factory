//! **No module polls the project on its own clock** (#501): a ratchet that only turns one way.
//!
//! The project index (`project_index.rs`) is the one background watcher; a panel that wants to know "did this file change?" subscribes
//! to it. #501's checklist (`docs/guide/reload-contract.md`, "The one watcher") lists the modules that still `stat` their own files on
//! their own timer. This test **fails when a poll constant of the checklist's shapes exists in `src/` that is not on [`STILL_OPEN`]**, so
//! no new one can be added while the old ones are retired. It does not fail when a name on the list disappears: that is a module
//! migrating, and the workers who migrate them must not have to edit this file (the entry is then just stale; delete it when convenient).
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

/// **How many poll constants the checklist of #501 listed when this ratchet was written** (the 22 of the issue, less the four retired by
/// the finder's scan, the Files tree, the `Used by` line and an open file's reload).
const CHECKLIST_COUNT: usize = 18;

/// **A name the checklist lists, the file it is in, and its item number** in `docs/guide/reload-contract.md`.
type Open = (&'static str, &'static str, &'static str);

/// **The poll constants that are still open.** Retiring one is deleting it from its module; its entry here may stay (the ratchet is one-way).
const STILL_OPEN: &[Open] = &[
    ("data_view.rs", "RECHECK_SECS", "item 5"),
    ("notes_view.rs", "RECHECK_SECS", "item 6"),
    ("oplog_view.rs", "RECHECK_SECS", "item 7"),
    ("checks_view.rs", "RECHECK_SECS", "item 8"),
    ("problems_view.rs", "READ_SECS", "item 9"),
    ("world_lists.rs", "READ_SECS", "item 10"),
    ("reference_view.rs", "RECHECK_SECS", "item 11"),
    ("forest_view.rs", "POLL_S", "item 12"),
    ("game_view.rs", "FIELD_FILES_POLL_S", "item 13"),
    ("game_view.rs", "STATE_POLL_S", "item 13"),
    ("night_shift.rs", "PLAN_READ_SECS", "item 14"),
    ("anim_watch.rs", "POLL_SECS", "item 15"),
    ("anim_watch.rs", "RIGS_RECHECK_SECS", "item 15"),
    ("audit_view.rs", "LOOK_EVERY_S", "item 16"),
    ("game_view.rs", "LOG_POLL_S", "item 17"),
    ("settings.rs", "RECHECK_SECS", "item 18"),
    ("settings_screen.rs", "LOOK_EVERY", "item 18"),
    ("keymap_screen.rs", "LOOK_EVERY", "item 18"),
];

/// **A poll the checklist does not list**, found by this ratchet when it was written: the Project page re-reads the agent guide on a half
/// second clock. It is allowed so the test passes today, and it is a finding for the manager (it is not one of the 22 of #501).
const NOT_ON_THE_CHECKLIST: &[Open] = &[("project_page.rs", "GUIDE_READ_SECS", "found by this ratchet, not in #501's list")];

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
    let allowed = |file: &str, name: &str| STILL_OPEN.iter().chain(NOT_ON_THE_CHECKLIST).any(|(f, n, _)| *f == file && *n == name);
    found.iter().filter(|(file, name, _)| !allowed(file, name)).collect()
}

/// **No new poll constant**: every one found is on the list.
#[test]
fn no_module_polls_the_project_on_its_own_clock() {
    let mut files = Vec::new();
    sources(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
    let found = poll_clocks(&files);
    let new: Vec<String> = unlisted(&found).iter().map(|(file, name, line)| format!("  src/**/{file}:{line}: `{name}`")).collect();
    let remaining = found.iter().filter(|(file, name, _)| STILL_OPEN.iter().any(|(f, n, _)| f == file && n == name)).count();
    let unlisted = found.iter().filter(|(file, name, _)| NOT_ON_THE_CHECKLIST.iter().any(|(f, n, _)| f == file && n == name)).count();
    let tally = format!("{remaining} of {CHECKLIST_COUNT} poll constants remain ({unlisted} more are not on the checklist)");
    println!("{tally}");
    assert!(
        new.is_empty(),
        "{tally}.\n\nA poll constant that is not on the list of #501 was added:\n{}\n\nA module that wants to know a project file changed subscribes to \
         `project_index::ProjectFileChanged` (see `docs/guide/reload-contract.md`, \"The one watcher\"); it does not keep its own timer. \
         If this is not a poll of the project, name the constant so it does not have a poll's shape.",
        new.join("\n")
    );
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
    // **The main assertion fails on a new constant and not on a listed one**: `unlisted` is what it asserts is empty.
    let fabricated = vec![
        ("zz_new_module.rs".to_owned(), "FOO_POLL_S".to_owned(), 1),
        ("data_view.rs".to_owned(), "RECHECK_SECS".to_owned(), 2),
        ("project_page.rs".to_owned(), "GUIDE_READ_SECS".to_owned(), 3),
    ];
    assert_eq!(unlisted(&fabricated), vec![&fabricated[0]], "a new constant is unlisted, a listed one and the unchecklisted one are not");
    assert!(STILL_OPEN.len() <= CHECKLIST_COUNT, "the list never holds more than the checklist's {CHECKLIST_COUNT}");
}
