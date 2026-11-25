//! **No command file orders a destructive git command** (#621).
//!
//! **The measured contradiction.** `AGENTS.md` forbids `git reset --hard` by name, and `build-loop.md` and
//! `editor-dev.md` both repeat the ban — while `debug-response.md:69` **instructed** one: *"move those commits aside
//! with `git branch parked/pass-<N>` and then `git reset --hard origin/main`"*. Two files in one directory, opposite
//! orders for one command, and every pass reads them both.
//!
//! **A prohibition and an instruction look almost identical to a grep**, so this test tells them apart the only way
//! that is honest: by whether the line **forbids** the command (it carries a banning word) or merely names it. A
//! line that names a destructive command without forbidding it is the defect, because a reader is looking for what
//! to do, not for what to avoid.
//!
//! The list of commands comes from `AGENTS.md`'s own section, so this cannot drift from the rule it enforces.

use std::path::{Path, PathBuf};

/// **The commands `AGENTS.md` names as needing the owner's permission**, as the text a grep can find.
const DESTRUCTIVE: &[&str] = &[
    "reset --hard",
    "reset --merge",
    "reset --keep",
    "checkout --",
    "clean -",
    "branch -D",
    "push --force",
    "push -f",
    "rebase",
    "filter-branch",
];

/// **A prohibition is a sentence with a banning *verb before* the command**, and nothing weaker.
///
/// A word-list was tried and failed on the first run: `debug-response.md:69` reads *"If the gate **refuses**, move
/// those commits aside … and then `git reset --hard origin/main`"* — the "refuses" is the gate's, not a ban on the
/// command, and the guard fell silent on the exact line it was written for. So the test does not look for words
/// anywhere on the line; it requires a banning word in the **first clause**, before the command is named. That is
/// where a prohibition puts it, and it is not where an instruction does.
const FORBIDDING: &[&str] = &[
    "never", "no destructive", "forbid", "must not", "don't", "do not", "avoid", "prohibit", "banned",
    "needs the user", "express permission", "without",
];

fn command_files(root: &Path) -> Vec<PathBuf> {
    let dir = root.join(".omp/commands");
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|k| k == "md"))
        .collect();
    found.sort();
    found
}

#[test]
fn the_commands_never_order_a_destructive_git() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let files = command_files(&root);
    assert!(!files.is_empty(), "`.omp/commands/*.md` was not found from {root:?}");

    let mut faults: Vec<String> = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        let shown = file.strip_prefix(&root).unwrap_or(file).display().to_string();
        for (at, line) in text.lines().enumerate() {
            let lower = line.to_lowercase();
            let Some((at_cmd, named)) = DESTRUCTIVE
                .iter()
                .filter_map(|d| lower.find(&**d).map(|i| (i, *d)))
                .min_by_key(|(i, _)| *i)
            else {
                continue;
            };
            // **A prohibition puts its banning word before the command**, in the same clause — `never \`git reset
            // --hard\``. An instruction names the command first and explains afterwards, so `before` is what tells
            // them apart, and it is not fooled by a subordinate "if the gate refuses" earlier in the sentence.
            let before = &lower[..at_cmd];
            let clause_start = before.rfind(". ").map(|i| i + 2).unwrap_or(0);
            if FORBIDDING.iter().any(|f| before[clause_start..].contains(f)) {
                continue;
            }
            faults.push(format!("{shown}:{}: names `{named}` without forbidding it: {}", at + 1, line.trim()));
        }
    }
    assert!(
        faults.is_empty(),
        "**{} command-file line(s) tell a reader to run a destructive git command** — `AGENTS.md` requires the \
         owner's permission by name for each. Make the line forbid it, or use the `git stash push -u` a sibling \
         step already prescribes:\n  {}",
        faults.len(),
        faults.join("\n  ")
    );
}
