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

/// **The dead names, assembled rather than spelled, each with the live name a line may carry instead.**
///
/// **A guard that greps for a word may not itself contain the word**, and this file lives under `crates/`,
/// so the acceptance grep would find the guard. Each name is therefore built from its pieces, and the comment
/// beside it says what it is.
///
/// **The second field is the `None`/`Some` split**: `None` is a name the tree must not carry at all, and
/// `Some(live)` is a name that may appear only on a line that also names `live` — the `(This was X; it is Y
/// now.)` shape `badges.rs` uses for `chrome::CensusRow`, which keeps the history and puts the reader within
/// one name of the thing that exists.
fn dead_names() -> Vec<(String, Option<&'static str>)> {
    vec![
        // The module that used to hold the persistence rule, folded into each format's own `parse`.
        (["persist", "rs"].join("."), None),
        // The 15-degree step, now `bevy_dark_factory::settings::ViewSettings::yaw_step_deg`.
        (["YAW", "STEP"].join("_"), None),
        // The fixed pixel column, now the computed `anim_tab::slot_cell()`.
        (["SLOT", "COL"].join("_"), None),
        // The placement system, which is `editor::drive_place`.
        (["place", "on", "click"].join("_"), None),
        // The map's writer, which is `Map::to_yaml`.
        (["to", "ron"].join("_"), None),
        // #652: `badges::ink()` collects UI rects; no world box is measured and dodged.
        (["sense", "world", "ink"].join("_"), None),
        // #652: the copy is `notice::harvest`.
        (["collect", "text"].join("_"), None),
        // #652: the advance is `compose::label_char_w()`, which reads `chrome::body_char_w()`.
        (["LABEL", "CHAR", "W"].join("_"), None),
        // #652: the containment rule is [`editor::brush_at`]'s; there is no `containment` module here.
        (["containment", "::", "cordon"].join(""), None),
        // #652: the ratchet is `keys::no_context_gains_a_key`.
        (["no_context", "carries_more_than_a_learnable_vocabulary"].join("_"), None),
        // #652: the system is `surface::fit_viewport_to_frame`. Matched whole, so that name does not fire.
        (["fit", "viewport"].join("_"), None),
        // #658: the hold-`K` overlay is `badges::rebuild_badges` (clusters) and `badges::place_badges`
        // (geometry); no system drives a shortcuts overlay of its own.
        (["drive", "shortcuts", "overlay"].join("_"), Some("rebuild_badges")),
        // #658: the key census is `keys::BINDINGS`/`keys::rows`, read by `badges::rebuild_badges`; there is no
        // `chrome` function that counts keys.
        (["key", "census"].join("_"), Some("BINDINGS")),
        // #658: the row-lighting is `badges::flash_live_badges`; the rows became badges.
        (["flash", "live", "rows"].join("_"), Some("flash_live_badges")),
    ]
}

/// **No dead name survives anywhere under `crates/`** (#591, #605, #606, #658).
///
/// The list is closed, not a heuristic: each one was a real citation the review found, and each has a
/// replacement this test names in the failure. A new one belongs in [`dead_names`] with its own line. A name
/// whose entry carries a live counterpart may stand on a line that names that counterpart, which is how a
/// comment keeps the history without sending a reader after a symbol that was never written.
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
            for (name, live) in &names {
                // **A name is matched whole**: the step's constant may not fire on `YAW_STEP_MAX_DEG`, which
                // exists and is a different thing (the largest turn a person may set, not the step).
                let mut from = 0;
                while let Some(at) = line[from..].find(name.as_str()) {
                    let start = from + at;
                    let after = line[start + name.len()..].chars().next();
                    let whole = !after.is_some_and(|c| c.is_alphanumeric() || c == '_');
                    // **The live name on the same line** is what makes the history a history: without it, a
                    // reader who follows the old name to `chrome::` finds nothing.
                    let paired = live.is_some_and(|live| line.contains(live));
                    if whole && !paired {
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
         `editor::drive_place`; `blueprint_model::map::Map::to_yaml` (#591, #605, #606); \
         `badges::ink()`; `notice::harvest`; `compose::label_char_w()`; `editor::brush_at`; \
         `keys::no_context_gains_a_key`; `surface::fit_viewport_to_frame` (#652); \
         `badges::rebuild_badges`/`badges::place_badges`, `keys::BINDINGS` and `badges::flash_live_badges` \
         (#658):\n{}",
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

/// **Every `docs/…md` a comment under the editor's `src` cites is a path the tree holds** (#660).
///
/// Ninety-seven of 135 such citations named a file this repository did not have, and `docs/ui.md` alone was
/// cited 52 times as the authority for section-numbered rules (§1.2, §3.5, §4.2 …) in a file no commit here
/// ever held. A dead symbol fails loudly when someone uses it; a dead **document citation** is the strongest
/// possible claim of checkability — "the rule is written down, here, at §3.5" — and it costs more to disprove
/// than to write, so nine comments can tell a reader the same rule is somewhere it is not.
///
/// **`CLAUDE.md` is not dead**, and this test is why the distinction is written down: each crate carries its
/// own `CLAUDE.md` beside its `Cargo.toml`, so `crates/bevy_dark_factory/CLAUDE.md` resolves and a citation to
/// it from `src/` is a path a reader can open. The check is therefore `fs::exists` against the crate and the
/// repository root, never a list of names.
#[test]
fn every_document_a_comment_cites_is_in_the_tree() {
    let repo = root();
    let src = repo.join("crates/bevy_dark_factory/src");
    let mut files = Vec::new();
    files_under(&src, &mut files);
    assert!(files.len() > 50, "the editor's src holds its modules; found {} .rs files", files.len());

    // **The two places a `docs/…` in a comment may resolve from**: the repository root, and the crate's own
    // directory (where its `CLAUDE.md` and `docs/` live). Both are paths a reader can actually open.
    let bases = [repo.clone(), repo.join("crates/bevy_dark_factory")];

    let mut found = Vec::new();
    let mut resolved = 0usize;
    for file in &files {
        let Ok(text) = std::fs::read_to_string(file) else { continue };
        let shown = file.strip_prefix(&repo).unwrap_or(file).display().to_string();
        for (n, line) in text.lines().enumerate() {
            for path in doc_paths(line) {
                if bases.iter().any(|base| base.join(&path).exists()) {
                    resolved += 1;
                } else {
                    found.push(format!("{shown}:{}: `{path}`", n + 1));
                }
            }
        }
    }
    assert!(resolved > 0, "no document citation was found at all under {}, so this test proves nothing", src.display());
    assert!(
        found.is_empty(),
        "**{} comment line(s) cite a document the tree does not hold**. Each is either the page that has the \
         rule now — `docs/ui/vision.md` holds the UI principles as `P1`…`P13`, so §4.2's mouse/keyboard parity \
         is `P11` — or nothing at all, in which case drop the citation and keep the sentence (#660):\n{}",
        found.len(),
        found.join("\n")
    );
}

/// **The `docs/…md`, `CLAUDE.md`, `TESTING.md` and `contextual_theming.md` paths on one line of source**, in
/// order.
///
/// The suffix set is the one #660 measured, not a guess: a `.md` path with no `docs/` in front of it is a bare
/// file name that could belong to any of the seven crates, and that ambiguity is what made #653's seven bare
/// `foo.rs` names uncheckable. `CLAUDE.md` is in the set because each crate carries one beside its
/// `Cargo.toml`, so it resolves against the crate root the same way a `docs/` path resolves against the
/// repository root. A path is returned as written so the failure quotes the comment's own words.
fn doc_paths(line: &str) -> Vec<String> {
    // **Only the doc extensions**: a `.rs` citation is #653's job, and a `.yaml` is the style sheet's.
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = line[from..].find(".md") {
        let end = from + at + 3;
        // **Walk back over the characters a path may be made of**, on `char` boundaries: a comment in this
        // crate is full of `—`, `§` and `·`, and stepping back a *byte* at a time would split one of those
        // and index the string off its own boundary. The reverse order is what lets the `break` stop at the
        // first character a path cannot hold.
        let head = from + at;
        let mut start = head;
        for (at, c) in line[..head].char_indices().rev() {
            if c.is_alphanumeric() || matches!(c, '/' | '_' | '-' | '.') {
                start = at;
            } else {
                break;
            }
        }
        let path = &line[start..end];
        // **A `docs/…` path, or one of the two names #660 counted at the repository root.** A bare `layout.md`
        // is skipped: it does not say which directory, and a checker that guessed would be the next lie.
        let named = path.starts_with("docs/")
            || path.ends_with("TESTING.md")
            || path.ends_with("contextual_theming.md")
            || path.ends_with("CLAUDE.md");
        if named && !path.contains("://") {
            out.push(path.to_string());
        }
        from = end;
    }
    out
}
