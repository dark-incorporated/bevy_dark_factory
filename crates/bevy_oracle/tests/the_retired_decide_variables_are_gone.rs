//! **The decision endpoint's environment variables are retired** (DM-03, #454): the decision model is the `decision_model` setting, read
//! from the settings layers, and no process environment or project `.env` names an endpoint any more.
//!
//! This walks every source file under `crates/` for the retired prefix. The only variable the environment still carries is
//! `OPENROUTER_API_KEY`, which is a key and not a setting. The one place the old names may appear is the changelog's
//! migration note, which is not under `crates/`.

use std::path::{Path, PathBuf};

/// The prefix, spelled in two pieces so that this file does not contain what it forbids.
fn retired_prefix() -> String {
    ["BLUEPRINT_", "DECIDE_"].concat()
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            if name != "target" && !name.starts_with('.') {
                sources(&path, out);
            }
        } else if matches!(path.extension().and_then(|e| e.to_str()), Some("rs" | "md" | "sh" | "py" | "yaml" | "toml" | "ron" | "js")) {
            out.push(path);
        }
    }
}

#[test]
fn no_file_under_crates_names_a_retired_decide_variable() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let needle = retired_prefix();
    let mut files = Vec::new();
    sources(&crates, &mut files);
    assert!(files.len() > 100, "the walk found {} files; it is looking in the wrong place", files.len());
    let found: Vec<String> = files
        .iter()
        .filter_map(|file| {
            let text = std::fs::read_to_string(file).ok()?;
            let line = text.lines().position(|l| l.contains(&needle))?;
            Some(format!("{}:{}", file.strip_prefix(&crates).unwrap_or(file).display(), line + 1))
        })
        .collect();
    assert!(found.is_empty(), "the decision model is the `decision_model` setting; these still name a retired variable: {found:#?}");
}
