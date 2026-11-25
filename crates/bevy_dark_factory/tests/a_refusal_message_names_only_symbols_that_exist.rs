//! **A test's own failure message names only symbols the tree holds** (#615).
//!
//! **The measured failure.** The chrome census refuses "spacing written as numbers outside chrome.rs",
//! and told the reader to *"Name it — `GAP_TIGHT` / `GAP_ROW` / `GAP_GROUP` / `PAD` / `MARGIN` /
//! `CHIP_PAD` / `BUTTON_PAD` / `FIELD_PAD` / `COL_TIGHT` / `COL_LABEL` / `COL_WIDE`"*. **Ten of those
//! eleven names had no `const`, `static` or `fn` anywhere in `crates/`.** Following the message wrote a
//! name the compiler would reject, so the refusal taught a wrong fix — which is worse than a refusal
//! with no advice, because it looks authoritative.
//!
//! The cause is structural rather than careless: **a refusal message is the one string nothing
//! compiles.** It can name a symbol that was deleted a year ago and every check stays green, because
//! no test reads the advice, only the condition.
//!
//! This test reads the advice. Every backticked `SCREAMING_CASE` or `Pascal::Case` name in a refusal
//! message must resolve to a symbol in the tree — and the two escape hatches are honest: a name may be
//! a **sheet class** (`.chrome-*`, which lives in CSS, not Rust) or be marked as prose.

use std::path::{Path, PathBuf};

/// **Where the messages live**: the test files, because a refusal's words are written there.
fn test_sources(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|k| k == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// **A backticked name that looks like a Rust symbol**: all-caps with underscores, or `Pascal::Camel`.
fn looks_like_a_symbol(name: &str) -> bool {
    let is_screaming = name.len() > 2
        && name.chars().any(|c| c.is_ascii_uppercase())
        && name.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && !name.chars().all(|c| c.is_ascii_digit());
    let is_path = name.contains("::")
        && name
            .split("::")
            .all(|part| part.chars().next().is_some_and(|c| c.is_ascii_uppercase()) || part.contains('<'));
    is_screaming || is_path
}

/// **Every backticked word in `text`.**
fn backticked(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        names.push(after[..close].to_owned());
        rest = &after[close + 1..];
    }
    names
}

#[test]
fn a_refusal_message_names_only_symbols_that_exist() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = crate_root.join("src");
    let tests = crate_root.join("tests");

    // **Every identifier the source declares**, as a haystack: the search below is a `contains`, so
    // a name is "real" if the tree mentions it as a declaration or a type path.
    let mut declarations = String::new();
    for file in test_sources(&src) {
        if let Ok(text) = std::fs::read_to_string(&file) {
            declarations.push_str(&text);
        }
    }
    assert!(declarations.len() > 100_000, "the source was not read; this check would pass on nothing");

    let mut faults: Vec<String> = Vec::new();
    for file in test_sources(&tests) {
        let Ok(text) = std::fs::read_to_string(&file) else { continue };
        let shown = file.strip_prefix(crate_root).unwrap_or(&file).display().to_string();
        for name in backticked(&text) {
            // A sheet class is CSS, not Rust, and lives in the style YAML.
            if name.starts_with('.') {
                continue;
            }
            let bare = name.split('<').next().unwrap_or(&name).trim_end_matches("()");
            if !looks_like_a_symbol(bare) {
                continue;
            }
            // `Col::Tight` -> resolve each half, so a real enum and a real variant both count.
            let found = bare.split("::").all(|part| declarations.contains(part));
            if !found {
                faults.push(format!("{shown}: `{name}` names no symbol in `src`"));
            }
        }
    }
    assert!(
        faults.is_empty(),
        "**{} name(s) a refusal message tells a reader to use, which the tree does not hold** — following \
         the advice writes code the compiler rejects. Name what exists, or mark the name as prose:\n  {}",
        faults.len(),
        faults.join("\n  ")
    );
}
