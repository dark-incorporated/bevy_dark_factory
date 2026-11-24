//! **A comment, a document or a test that names a symbol, a module or a format the tree does not have is
//! a lie a reader has no way to check** (#591, #604, #605, #606).
//!
//! The 2026-11-24 review found five of them in one sweep (#591, #605, #606), and every one had the same
//! shape: the code moved and the prose did not. A writer was renamed a character class away, a module was
//! folded into each format's own `parse`, a fixed pixel column became a computed `slot_cell()`, a settings
//! field replaced a constant (#591), a placement system was renamed to `drive_place` (#605) and thirteen
//! comments called a YAML file RON (#604). None of these could fail a build: clippy does not read prose, and
//! rustdoc only follows `[intra-doc]` links, not a bare `backticked_name`.
//!
//! So this file is the build that reads them. Each test is a grep the review ran by hand, kept:
//!
//! 1. no dead name is anywhere under `crates/` (`src`, `tests` and the manifests);
//! 2. no `RON` in the editor's `src` is a claim about a live format — cutover history carries its citation;
//! 3. `keys.rs`'s focus sentence states the machinery the crate actually has, counted from `src/`;
//! 4. the activity bar draws every workspace `Workspace::ALL` declares, and `WAITING` is what decides.

use std::path::{Path, PathBuf};

/// **The repository's root**, from this crate's manifest.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// **Every `.rs`, `.md`, `.toml` and `.yaml` file under `dir`**, in a stable order.
///
/// **`no_comment_names_a_symbol_the_tree_does_not_have` skips this file and only this file**, because it is the
/// guard: the names it refuses are assembled here, and a walk would find them in this file's own list.
/// Nothing else is skipped — not `tests/`, which #605 found one of its six in.
fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            // `target/` is build output, and a stale copy of a file in it is not this tree's prose.
            if path.file_name().is_some_and(|n| n == "target" || n == ".git") {
                continue;
            }
            files_under(&path, out);
        } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| matches!(e, "rs" | "md" | "toml" | "yaml")) {
            out.push(path);
        }
    }
}

/// **The dead names, assembled rather than spelled.**
///
/// **A guard that greps for a word may not itself contain the word**, and this file lives under `crates/`,
/// so the acceptance grep would find the guard. Each name is therefore built from its pieces, and the comment
/// beside it says what it is.
fn dead_names() -> Vec<String> {
    vec![
        // The module that used to hold the persistence rule, folded into each format's own `parse`.
        ["persist", "rs"].join("."),
        // The 15-degree step, now `bevy_dark_factory::settings::ViewSettings::yaw_step_deg`.
        ["YAW", "STEP"].join("_"),
        // The fixed pixel column, now the computed `anim_tab::slot_cell()`.
        ["SLOT", "COL"].join("_"),
        // The placement system, which is `editor::drive_place`.
        ["place", "on", "click"].join("_"),
        // The map's writer, which is `Map::to_yaml`.
        ["to", "ron"].join("_"),
    ]
}

/// **No dead name survives anywhere under `crates/`** (#591, #605, #606).
///
/// The five names are a closed list, not a heuristic: each one was a real citation the review found, and each
/// has a replacement this test names in the failure. A new one belongs in [`dead_names`] with its own line.
#[test]
fn no_comment_names_a_symbol_the_tree_does_not_have() {
    let crates = root().join("crates");
    let mut files = Vec::new();
    files_under(&crates, &mut files);
    assert!(!files.is_empty(), "no files were walked under {}", crates.display());

    let names = dead_names();
    let me = std::path::Path::new(file!()).file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
    let mut found = Vec::new();
    for file in &files {
        // **The guard's own file, and only it**: see `files_under`'s note.
        if file.file_name().and_then(|n| n.to_str()).is_some_and(|n| n == me) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        let shown = file.strip_prefix(root()).unwrap_or(file).display().to_string();
        for (n, line) in text.lines().enumerate() {
            for name in &names {
                // **A name is matched whole**: the step's constant may not fire on `YAW_STEP_MAX_DEG`, which
                // exists and is a different thing (the largest turn a person may set, not the step).
                let mut from = 0;
                while let Some(at) = line[from..].find(name.as_str()) {
                    let start = from + at;
                    let after = line[start + name.len()..].chars().next();
                    let whole = !after.is_some_and(|c| c.is_alphanumeric() || c == '_');
                    if whole {
                        found.push(format!("{shown}:{}: `{name}`", n + 1));
                    }
                    from = start + name.len();
                }
            }
        }
    }
    assert!(
        found.is_empty(),
        "**{} line(s) name something the tree does not have**. Where each one really lives, in the order \
         `dead_names` lists them: `blueprint_model::format_version` and each format's own `parse`/`validate`; \
         `settings::ViewSettings::yaw_step_deg` (15.0 by default); `anim_tab::slot_cell()`; \
         `editor::drive_place`; `blueprint_model::map::Map::to_yaml` (#591, #605, #606):\n{}",
        found.len(),
        found.join("\n")
    );
}

/// **Every `RON` in the editor's `src` is either the syntax or cutover history that says so** (#604).
///
/// Thirteen comments called the family settings files RON while `ops_family.rs` writes and parses YAML, and the
/// two are one subsystem disagreeing with itself: a reader implementing `family_set` went looking for RON
/// quoting rules, `(`/`)` delimiters and a `ron` crate that has not been a dependency since GD-03 (#365). The
/// rule is the review's acceptance test, kept: a line that says RON says `GD-03` or `#365` on the same line,
/// unless it names the file's syntax (`` `.ron` ``, which a game may still keep) rather than a project file.
#[test]
fn no_ron_claim_in_the_editor_lacks_its_cutover_citation() {
    let src = root().join("crates/bevy_dark_factory/src");
    let mut files = Vec::new();
    files_under(&src, &mut files);

    let mut claims = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        let shown = file.strip_prefix(root()).unwrap_or(file).display().to_string();
        for (n, line) in text.lines().enumerate() {
            let mut from = 0;
            while let Some(at) = line[from..].find("RON") {
                let start = from + at;
                from = start + 3;
                // **A word, not a substring**: `RONS` is not a claim, and neither is a URL's path.
                let before = line[..start].chars().next_back();
                let after = line[start + 3..].chars().next();
                if before.is_some_and(|c| c.is_alphanumeric() || c == '_') || after.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                    continue;
                }
                if line.contains("GD-03") || line.contains("#365") {
                    continue;
                }
                // **The syntax, not a file this editor reads or writes**: a `.ron` beside the word is a game's
                // own file, which `files_lang::ron` colours and nothing in the project half parses.
                if line.contains(".ron") {
                    continue;
                }
                claims.push(format!("{shown}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    assert!(
        claims.is_empty(),
        "**{} line(s) in `crates/bevy_dark_factory/src` call something RON without saying it is cutover \
         history**. Since GD-03 (#365) every project file is YAML, and `ops_family.rs` is where the family \
         settings are read and written: name the format the code uses, or cite the cutover:\n{}",
        claims.len(),
        claims.join("\n")
    );
}

/// **`keys.rs`'s focus sentence states the machinery the crate has, counted from `src/`** (#602).
///
/// The module's argument is that routing is by context and not by focus, and it used to rest on *"`InputFocus`
/// /`TabIndex` appear nowhere in `src/`"* — which was false in nine places, six of them text-entry fields and
/// two of them `TabIndex(0)` on a focus region. A reader deciding whether to add focus handling was told the
/// ground was empty when it was occupied. The sentence now names what is there, and this counts it, so the
/// sentence cannot drift while the code moves.
#[test]
fn the_keys_doc_states_the_focus_machinery_the_crate_actually_has() {
    let src = root().join("crates/bevy_dark_factory/src");
    let mut files = Vec::new();
    files_under(&src, &mut files);

    let mut with_input_focus = Vec::new();
    let mut with_tab_index = Vec::new();
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        let shown = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // **Code only**: a comment that names a type is not a place the type is used, and `keys.rs`'s own doc is
        // the sentence under test.
        let code = text.lines().filter(|l| {
            let t = l.trim_start();
            !t.is_empty() && !t.starts_with("//")
        });
        let mut input = false;
        let mut tab = false;
        for line in code {
            input |= line.contains("InputFocus");
            tab |= line.contains("TabIndex");
        }
        if input {
            with_input_focus.push(shown.clone());
        }
        if tab {
            with_tab_index.push(shown);
        }
    }
    with_input_focus.sort();
    with_tab_index.sort();

    let keys = std::fs::read_to_string(src.join("keys.rs")).expect("keys.rs is in this crate");
    let word = |n: usize| match n {
        0 => "none",
        1 => "one",
        2 => "two",
        3 => "three",
        4 => "four",
        5 => "five",
        6 => "six",
        7 => "seven",
        8 => "eight",
        9 => "nine",
        _ => "too many",
    };
    let said_input = format!("`InputFocus` is in {} files", word(with_input_focus.len()));
    let said_tab = format!("`TabIndex` in {}", word(with_tab_index.len()));
    assert!(
        keys.contains(&said_input),
        "**`keys.rs`'s focus note does not say `{said_input}`**, and {} file(s) carry `InputFocus` in code \
         ({}). The note argues the module's design, so it has to state what is there (#602).",
        with_input_focus.len(),
        with_input_focus.join(", ")
    );
    assert!(
        keys.contains(&said_tab),
        "**`keys.rs`'s focus note does not say `{said_tab}`**, and {} file(s) carry `TabIndex` in code ({}). \
         `chrome`'s viewport region and `report`'s modal `TabGroup` are the two, and `report` records why it \
         reopened the decision (#602).",
        with_tab_index.len(),
        with_tab_index.join(", ")
    );
}

/// **The activity bar draws every workspace `Workspace::ALL` declares, and `workspace::WAITING` is what decides** (#603).
///
/// The module note used to say *"Agent and Files are not registered yet (NV-10 and GD-11), so their tabs ...
/// are not on the bar today"*, and `workspace.rs`'s heading said *"Nine"*, while `ALL` declared ten and
/// `WAITING` had been empty since NV-08 (#385). A reader looking for where a tab comes from was told two
/// workspaces were absent by the one file that could see them. `WAITING` is the single table: a workspace is
/// built exactly when it is not listed, and the bar filters on that — so the bar and the heading can only
/// agree, which is what this asserts.
#[test]
fn the_activity_bar_draws_every_workspace_the_registry_declares() {
    use bevy_dark_factory::activity_bar;
    use bevy_dark_factory::workspace::{Workspace, WAITING};

    let drawn = activity_bar::registered();
    let all = Workspace::ALL.to_vec();
    assert_eq!(
        drawn, all,
        "**the activity bar draws {drawn:?} and `Workspace::ALL` declares {all:?}**. The bar is \
         `Workspace::ALL` filtered by `is_built`/`WAITING`, so a difference means one of them changed \
         without the other (#603)."
    );
    // **The iff**: nothing is waiting exactly while every workspace is drawn.
    assert_eq!(
        drawn.len() == all.len(),
        WAITING.is_empty(),
        "`Workspace::WAITING` has {} entr(ies) and the bar draws {} of {} workspaces: a workspace that is \
         waiting must not be drawn, and one that is drawn must not be waiting (#603).",
        WAITING.len(),
        drawn.len(),
        all.len()
    );

    // **And the filter is what does it**, proved on a table that is not empty — the seam `registered_in`
    // exists for, so this test keeps meaning after a future workspace lands: one entry hides exactly one tab.
    let table: [(Workspace, &str); 1] = [(Workspace::Design, "a later issue")];
    let hidden = activity_bar::registered_in(&table);
    assert_eq!(hidden.len(), all.len() - 1, "one waiting workspace hides exactly one tab");
    assert!(!hidden.contains(&Workspace::Design), "the waiting workspace draws no tab, and no placeholder");
    for w in Workspace::ALL {
        assert_eq!(
            drawn.contains(&w),
            w.is_built(),
            "`{w:?}` is drawn by the bar iff `is_built()`, and the two disagree: the note in `activity_bar.rs` \
             and the table in `workspace.rs` are one fact (#603)"
        );
    }
}
