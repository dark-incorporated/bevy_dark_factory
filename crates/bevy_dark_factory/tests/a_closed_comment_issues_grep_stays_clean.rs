//! **A closed comment-cleanup issue's own grep still returns nothing** (#618).
//!
//! Four issues closed claiming a comment sweep was complete, and each one's acceptance was a `grep` that should return
//! nothing. All four still returned hits on 2026-11-25 (#614–#617): the fixes landed, a later commit edited the same
//! files, and nothing re-ran the check. A close comment is not a check; this is.
//!
//! **Each pattern is the issue's own acceptance grep**, not a new list — so this test cannot drift from what the
//! issue asked. When one of these fails, the failure names the issue, the file and the line.

use std::path::{Path, PathBuf};

/// **One closed issue's acceptance grep**: the issue, what its close claimed, and the pattern it should leave dark.
struct Closed {
    issue: u32,
    claimed: &'static str,
    /// A path fragment every hit must contain, or empty when the pattern alone is the defect.
    pattern: &'static str,
    /// Hits under these files are deliberately historical (a migration note naming what moved) and are exempt.
    exempt: &'static [&'static str],
}

/// **The four closes whose greps still returned hits on 2026-11-25**, with the pattern each issue named.
const CLOSED: &[Closed] = &[
    Closed {
        issue: 544,
        claimed: "`rg '\\b0\\.19\\b' crates/ -g '*.rs'` -> 0 comment mentions",
        pattern: "0.19",
        // A migration note may name the version it moved from, so long as it says it did.
        exempt: &["was 0.19", "from 0.19", "moved from"],
    },
    Closed {
        issue: 543,
        claimed: "the 48 deleted constants no longer appear as live names",
        pattern: "MIN_FIELD_H",
        exempt: &[],
    },
    Closed {
        issue: 548,
        claimed: "the names its table listed were replaced",
        pattern: "BODY_CHAR_W",
        exempt: &[],
    },
    Closed {
        issue: 546,
        claimed: "the two counts are gone in favour of the mechanism",
        pattern: "twenty-four global",
        exempt: &[],
    },
];

/// **Every `.rs` file under the editor's `src`**, as paths a failure can name.
fn sources(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|kind| kind == "rs") {
                found.push(path);
            }
        }
    }
    found
}

#[test]
fn a_closed_comment_issues_grep_stays_clean() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let files = sources(&root);
    assert!(files.len() > 50, "the editor's src holds its modules; found {} .rs files", files.len());

    let mut failures: Vec<String> = Vec::new();
    for closed in CLOSED {
        for file in &files {
            let Ok(text) = std::fs::read_to_string(file) else { continue };
            for (at, line) in text.lines().enumerate() {
                if !line.contains(closed.pattern) {
                    continue;
                }
                if closed.exempt.iter().any(|said| line.contains(said)) {
                    continue;
                }
                let shown = file.strip_prefix(&root).unwrap_or(file).display();
                failures.push(format!(
                    "#{} (`{}`) at src/{shown}:{}: {}",
                    closed.issue,
                    closed.claimed,
                    at + 1,
                    line.trim()
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "**{} hit(s) from a closed issue's own grep** — the fix landed and a later edit brought the name back. \
         Fix the comment; if the name is genuinely historical, make the line say which version it moved from:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}
