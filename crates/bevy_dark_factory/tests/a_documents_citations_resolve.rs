//! **Every `symbol (path:line)` citation in the design pages lands on the symbol it names, every `Format version`
//! row states the constant the code compiles, every `Live reload` row says what the reload contract says, and every
//! file kind is either sectioned or delegated** (#586, #587, #588, #608, #609).
//!
//! # Why one resolver rather than a test per page
//!
//! Each of those issues is the same defect seen on one page: a document states a fact about the code — a line
//! number, a version number, whether a file is watched — and nothing reads the two together, so the page keeps
//! saying what was true when it was written. A page is prose, so the only general shape is "find the claim, find
//! the fact, compare". This is that comparison, over three pages at once, so a citation added tomorrow is read
//! without a new test.
//!
//! # What is resolved, and what is skipped
//!
//! Three claim shapes are read:
//!
//! 1. **`symbol (path:line)`** — the form `docs/design/layout-model.md` and `docs/design/text-roles.md` write. The
//!    line at `path:line` must name `symbol` (a `::` path is matched by its last segment, because a page cites
//!    `chrome::section` for a line whose declaration says `pub fn section`).
//! 2. **`Format version | N (`CONST`)`** rows of `docs/guide/data-files.md`, read against the `*_VERSION`
//!    constant the code compiles rather than against a copy of the number.
//! 3. **`Live reload | Yes/No`** rows of `docs/guide/data-files.md`, read against `docs/guide/reload-contract.md`'s
//!    own `class` column, so the two pages cannot disagree (#587).
//!
//! Skipped, and counted in the failure message so the count is stated rather than wondered at:
//!
//! - a citation into a file this repository does not hold (`list.rs` is `bevy_gooey`'s);
//! - a `path:line` inside a fenced code block (a tree diagram's comment is a picture, not a claim);
//! - a `path:line` with no symbol before it.
//!
//! # A symbol that became a sheet value
//!
//! `CONTROLS_W` and `LIST_W` were deleted into `metrics.layout.controls_w`/`list_w` (#608). A page that cites no
//! line for them is not wrong — there is no line — as long as it says so: the citation resolves when the page
//! names the `metrics.layout`/`metrics.type` key **and** the sheet holds that key with the number the page states.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use blueprint_model::reload::KINDS;

/// **The repository's root**, from this crate's manifest.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// **The text of a repository file**, or a panic naming it.
fn read(rel: &str) -> String {
    let path = root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// **Every `.rs` file in the workspace, by bare file name**, so a citation written `chrome.rs:1178` finds the one
/// `chrome.rs` the repository holds. A name two files share is an error rather than a guess.
fn sources_by_name() -> BTreeMap<String, Vec<PathBuf>> {
    let mut found: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    let mut stack = vec![root().join("crates")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
            {
                found.entry(name.to_owned()).or_default().push(path);
            }
        }
    }
    found
}

/// **One citation the page makes**: the symbol a sentence names, and where it says to look.
struct Citation {
    /// The page and line the citation is written on, for the failure message.
    written_at: String,
    symbol: String,
    file: String,
    line: usize,
}

/// **Every `symbol (path.rs:line)` citation in `doc`**, read the way a reader reads one: a backticked symbol
/// followed by a parenthesised `` `file.rs:N` ``. A `:N-M` range cites its first line.
fn citations(doc: &str, page: &str) -> Vec<Citation> {
    let mut out = Vec::new();
    let mut fenced = false;
    for (index, line) in doc.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let text = line;
        let mut from = 0usize;
        while let Some(found) = text[from..].find(".rs:") {
            let at = from + found;
            let digits: String = text[at + 4..].chars().take_while(char::is_ascii_digit).collect();
            if digits.is_empty() {
                from = at + 4;
                continue;
            }
            // **The file name**, walked back from `.rs` over the characters a path is made of. A citation into a
            // dependency (`list.rs:99`) is written with no folder, which is how the check knows it is not ours.
            let head = &text[..at + 3];
            let start = head
                .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '/' || c == '.'))
                .map_or(0, |n| n + 1);
            let file = &head[start..];
            // **The symbol**: the last backticked span before the `(` that opens the citation's parenthesis.
            let symbol = head.rfind('(').and_then(|open| {
                let before = &head[..open];
                let close = before.rfind('`')?;
                let open = before[..close].rfind('`')?;
                Some(before[open + 1..close].to_owned())
            });
            let Some(symbol) = symbol else {
                from = at + 4;
                continue;
            };
            // **A backticked path is the file itself**, not a symbol in it: `style/editor.yaml` in front of
            // `chrome.rs:1178` is a neighbouring citation, not this one's symbol.
            if symbol.contains('/') || symbol.contains('.') {
                from = at + 4;
                continue;
            }
            out.push(Citation {
                written_at: format!("{page}:{}", index + 1),
                symbol: symbol.trim_end_matches("()").to_owned(),
                file: file.to_owned(),
                line: digits.parse().expect("digits"),
            });
            from = at + 4 + digits.len();
        }
    }
    out
}

/// **Whether `symbol` names something at `line`**, matched whole so `body` does not answer for `body_line_h`.
///
/// A `::` path is matched by its last segment: a page cites `chrome::section` for a line whose own words are
/// `pub fn section`. Both a declaration and a **call site** count, because
/// `docs/design/text-roles.md` cites `chrome::body` at `world_lists.rs:128`, which is the call that draws the hint —
/// the citation is of the call, and the call is the fact.
fn names(line: &str, symbol: &str) -> bool {
    let leaf = symbol.rsplit("::").next().unwrap_or(symbol);
    let mut from = 0;
    while let Some(found) = line[from..].find(leaf) {
        let at = from + found;
        let before_ok = line[..at].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let after = &line[at + leaf.len()..];
        let after_ok = after.chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        if before_ok && after_ok {
            return true;
        }
        from = at + leaf.len().max(1);
    }
    false
}

/// **Every `Format version` row of `data-files.md` that names a `*_VERSION` constant**: the section, the constant,
/// the number the page states beside it, and the row for the failure message.
///
/// A row may name two constants (`area.yaml` and `board.yaml`), and each is read with the digits that end just
/// before its own `(`. A row that names no constant (`bevy_flora`'s, "None.", "the person's own bytes") is not a
/// compile-time number of this repo's and is not read here; the rule is stated in the test that uses this, so a
/// row that drops its constant is caught rather than skipped.
fn stated_versions() -> Vec<(String, String, u32, String)> {
    let doc = read("docs/guide/data-files.md");
    let mut out = Vec::new();
    let mut section = String::new();
    for line in doc.lines() {
        if let Some(head) = line.strip_prefix("## ") {
            section = head.trim().to_owned();
            continue;
        }
        let Some(row) = line.strip_prefix("| Format version | ") else { continue };
        let mut from = 0usize;
        while let Some(found) = row[from..].find("(`") {
            let at = from + found;
            let rest = &row[at + 2..];
            let Some(close) = rest.find("`)") else { break };
            let constant = rest[..close].to_owned();
            from = at + 2 + close + 2;
            if !constant.ends_with("_VERSION") {
                continue;
            }
            // **The number beside it**: the digits that end just before this `(`. A row whose number is not a digit
            // run (`bevy_flora`'s) fails here rather than silently reading the previous constant's.
            let head = row[..at].trim_end().trim_end_matches('*').trim_end();
            let digits: String = head.chars().rev().take_while(char::is_ascii_digit).collect();
            let digits: String = digits.chars().rev().collect();
            let number: u32 = digits.parse().unwrap_or_else(|_| {
                panic!("{section}: `{constant}` has no version number before it in `{}`", line.trim())
            });
            out.push((section.clone(), constant, number, line.trim().to_owned()));
        }
    }
    out
}

/// **The number a `pub const NAME: u32 = N` declares**, from the workspace source. One declaration, or a failure:
/// two constants of one name would make the page's row ambiguous.
fn constant_value(name: &str, sources: &BTreeMap<String, Vec<PathBuf>>) -> u32 {
    let needle = format!("const {name}: u32 = ");
    let mut found = Vec::new();
    for paths in sources.values() {
        for path in paths {
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            for line in text.lines() {
                let Some(at) = line.find(&needle) else { continue };
                let rest = &line[at + needle.len()..];
                let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
                if !digits.is_empty() {
                    found.push((path.clone(), digits.parse::<u32>().expect("digits")));
                }
            }
        }
    }
    assert_eq!(found.len(), 1, "`{name}` is declared {} time(s): {found:?}", found.len());
    found[0].1
}

/// **The kind each `## ` section of `data-files.md` is about**, by heading. A heading is a file name where the
/// kind's id is that file's stem, and an explicit row where it is not: `editor.yaml` is `editor_settings`,
/// `project.yaml` is `policy`, `maps/<name>.map.yaml` is `map`, and two prose headings carry two kinds each. A
/// heading with no kind (`Snip images`) says so with an empty list, so the table is complete over the page and a new
/// section is a decision rather than a silently-unchecked row.
///
/// **The table is written out because a heuristic is not**: guessing from the pattern read `maps/<name>.map.yaml`
/// as `map` by luck and `editor.yaml` as no kind at all, so half the page went unchecked while the test passed.
fn section_kinds() -> BTreeMap<&'static str, Vec<&'static str>> {
    BTreeMap::from([
        ("kits.yaml", vec!["kits"]),
        ("vocab.yaml", vec!["vocab"]),
        ("library.yaml", vec!["library"]),
        ("project.yaml", vec!["policy"]),
        ("compositions.yaml", vec!["compositions"]),
        ("rules.yaml", vec!["rules"]),
        ("standards.yaml", vec!["standards"]),
        ("repetition.yaml", vec!["repetition"]),
        ("rigs.yaml", vec!["rigs"]),
        ("maps/<name>.map.yaml", vec!["map"]),
        ("forest/<map>.yaml", vec!["forest"]),
        ("trees/<name>.tree.yaml", vec!["trees"]),
        ("family/bevy_elements.yaml", vec!["family"]),
        ("The design folder", vec!["design", "board"]),
        ("oplog.jsonl", vec!["oplog"]),
        ("A file the Files workspace makes, renames or deletes", vec![]),
        ("A draft of an unsaved buffer", vec![]),
        ("Snip images", vec![]),
        ("Feedback reports about the project", vec![]),
        ("Feedback reports about the editor", vec![]),
        ("recent.yaml", vec![]),
        ("editor.yaml and settings.yaml", vec!["editor_settings"]),
        ("The editor's style sheet", vec!["editor_style"]),
    ])
}

// ── #608: the design pages' citations ──────────────────────────────────────────────────────────

/// **Every `symbol (path:line)` citation in the two design pages resolves**, or the page says the symbol is a
/// **sheet value** a line number cannot name (#608).
///
/// The defects this reads for: `CONTROLS_W` and `LIST_W` cited at lines that are comments (they became
/// `metrics.layout.controls_w`/`list_w`), a `dock()` that never existed (`fit_docks_to_the_window` is the function),
/// and four symbols one line off — `fit_docks_to_the_window` at `:1335` for `:1178`, `size_the_shell_split` at
/// `:3743` for `:3807`, `the_left_panel_follows_its_pane` at `:3806` for `:3872`, and the window root at `:1660` for
/// the `spawn_frame` that builds it.
#[test]
fn the_design_pages_citations_land_on_their_symbols() {
    let sources = sources_by_name();
    let sheet = read("crates/bevy_dark_factory/src/style/editor.yaml");
    let mut resolved = 0usize;
    let mut skipped = 0usize;
    let mut wrong = Vec::new();
    for page in ["docs/design/layout-model.md", "docs/design/text-roles.md"] {
        let doc = read(page);
        for citation in citations(&doc, page) {
            // **A file this repository does not hold is a dependency's**, and a citation into one is not a claim
            // this test can check. Counted, never silently dropped.
            let Some(paths) = sources.get(&citation.file) else {
                skipped += 1;
                continue;
            };
            assert_eq!(
                paths.len(),
                1,
                "{}: `{}` names {} files of this repository ({paths:?}); a citation has to say which",
                citation.written_at,
                citation.file,
                paths.len()
            );
            let text = std::fs::read_to_string(&paths[0]).expect("a source file");
            let line = text.lines().nth(citation.line - 1).unwrap_or_else(|| {
                panic!("{}: `{}:{}` is past the end of the file", citation.written_at, citation.file, citation.line)
            });
            if names(line, &citation.symbol) {
                resolved += 1;
                continue;
            }
            // **A constant that became a sheet value**: no line to cite, so the page must say so and the sheet must
            // hold it.
            if is_sheet_value(&citation.symbol, &sheet) && says_sheet_value(&doc, &citation.symbol) {
                resolved += 1;
                continue;
            }
            wrong.push(format!(
                "{}: cites `{}` at `{}:{}`, which is `{}`",
                citation.written_at,
                citation.symbol,
                citation.file,
                citation.line,
                line.trim()
            ));
        }
    }
    assert!(
        wrong.is_empty(),
        "**{} citation(s) in the design pages land on something else (#608)**. A reader following one arrives \
         somewhere unrelated. Fix the page; the code is right:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    // **The number resolved is stated by a failure**, so a resolver that stopped reading cannot pass on an empty
    // list. 11 citations resolved when this was written; the floor is below that only for a citation legitimately
    // removed.
    assert!(
        resolved >= 9,
        "the design pages resolved only {resolved} citation(s) and skipped {skipped}; 11 were read when this test \
         was written, so the reader has stopped seeing some"
    );
    assert!(skipped <= 1, "{skipped} citations were skipped as another crate's; only `list.rs` should be");
}

// ── #685: the same citations, in test comments ────────────────────────────────────────────────

/// **The symbol a parenthetical group of citations belongs to**: the last backticked identifier before the
/// group's `(`, so `(`anim_tab.rs:1497`, `chrome.rs:3030`)` is the `FlexWrap::Wrap` its sentence names.
///
/// A group is read the way a reader reads it: one sentence, one symbol, the files it stands in. A citation
/// with no identifier before its `(` — a bare `(`chrome.rs:1178`` opening a sentence — is skipped, which is the
/// same rule the design pages get: a citation without a symbol is not a claim this test can check.
fn the_groups_symbol(line: &str) -> Option<(String, usize)> {
    let paren = line.find('(')?;
    let head = &line[..paren];
    let mut symbol = None;
    let mut from = 0usize;
    while let Some(open) = head[from..].find('`') {
        let at = from + open + 1;
        let close = head[at..].find('`').map(|n| at + n)?;
        let content = &head[at..close];
        let identifier = !content.is_empty()
            && content.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && content.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':');
        if identifier {
            symbol = Some(content.to_owned());
        }
        from = close + 1;
    }
    symbol.map(|symbol| (symbol, paren))
}

/// **Every `file.rs:N` citation in a `//` or `///` comment under any crate's `tests/` lands on the symbol its
/// sentence names** (#685), or says the symbol without a line.
///
/// Five such citations were stale in the same week that `chrome.rs`'s dock panels took `Role::Panel` (#650): the
/// design pages' citations moved with it and had this test, and the test comments' did not and had nothing. The
/// rule is the design pages' rule — the symbol beside the citation must be named on that line, matched whole so
/// `body` does not answer for `body_line_h` — and the failure names the file, the line, the citation and where
/// the symbol is now, because "somewhere else" is what a reader needs.
///
/// **A symbol with no line is the better fix and passes**: a comment that says `chrome::wrapping_row` cannot go
/// stale. This reads a citation; it does not demand one.
///
/// Skipped and counted, never silently: a file this repository does not hold (a dependency's `list.rs`), a
/// citation with no identifier before its `(` (a bare path), and a backticked path rather than a symbol.
#[test]
fn the_test_comments_citations_land_on_their_symbols() {
    let sources = sources_by_name();
    let mut resolved = 0usize;
    let mut skipped = 0usize;
    let mut wrong = Vec::new();
    let tests = root().join("crates");
    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![tests];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs")
                && path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()) == Some("tests")
            {
                files.push(path);
            }
        }
    }
    files.sort();
    assert!(files.len() > 100, "only {} test file(s) found; the walk has stopped reaching them", files.len());
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let page = file.strip_prefix(root()).unwrap_or(file).display().to_string();
        for (index, line) in text.lines().enumerate() {
            let stripped = line.trim_start();
            if !(stripped.starts_with("//") || stripped.starts_with('*')) {
                continue;
            }
            let Some((symbol, from)) = the_groups_symbol(line) else { continue };
            // **Every citation after the group's `(`**, on this line: a group names one symbol in one place.
            let mut at = from;
            while let Some(found) = line[at..].find(".rs:") {
                let start = at + found;
                let digits: String = line[start + 4..].chars().take_while(char::is_ascii_digit).collect();
                if digits.is_empty() {
                    at = start + 4;
                    continue;
                }
                let head = &line[..start + 3];
                let begin = head
                    .rfind(|c: char| !(c.is_alphanumeric() || c == '_' || c == '/' || c == '.'))
                    .map_or(0, |n| n + 1);
                let cited = &head[begin..];
                at = start + 4 + digits.len();
                let number: usize = digits.parse().expect("digits");
                let Some(paths) = sources.get(cited) else {
                    skipped += 1;
                    continue;
                };
                if paths.len() != 1 {
                    wrong.push(format!("{page}:{}: `{cited}` names {} files of this repository; a citation has to say which", index + 1, paths.len()));
                    continue;
                }
                let source = std::fs::read_to_string(&paths[0]).expect("a source file");
                let Some(named) = source.lines().nth(number - 1) else {
                    wrong.push(format!("{page}:{}: `{symbol}` at `{cited}:{number}` is past the end of the file", index + 1));
                    continue;
                };
                if names(named, &symbol) {
                    resolved += 1;
                    continue;
                }
                let now = source.lines().position(|l| names(l, &symbol)).map(|n| n + 1);
                wrong.push(match now {
                    Some(now) => format!(
                        "{page}:{}: cites `{symbol}` at `{cited}:{number}`, which is `{}`; the symbol is at line {now}",
                        index + 1,
                        named.trim()
                    ),
                    None => format!(
                        "{page}:{}: cites `{symbol}` at `{cited}:{number}`, but `{cited}` no longer names `{symbol}` at all",
                        index + 1
                    ),
                });
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "**{} test-comment citation(s) do not land on the symbol their sentence names (#685)**. Name the symbol \
         without a line (it cannot go stale), or fix the number:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    // **The number resolved is stated by a failure**, so a resolver that stopped reading cannot pass on an empty
    // list: eight citations resolve as this is written, and a later edit that drops one fails here.
    assert!(
        resolved >= 8,
        "only {resolved} test-comment citation(s) resolved and {skipped} were skipped; eight resolved when this \
         test was written, so the reader has stopped seeing some"
    );
}

/// **Whether a name is a `metrics` value of the sheet**, which a line-number citation cannot point at because it is
/// data rather than code.
///
/// The lower-case name of the deleted constant is the sheet's key: `CONTROLS_W` → `controls_w`.
fn is_sheet_value(symbol: &str, sheet: &str) -> bool {
    let key = symbol.to_lowercase();
    sheet.lines().any(|line| {
        let Some(head) = line.split(':').next() else { return false };
        head.trim() == key && line.contains(':')
    })
}

/// **Whether the page says, beside this symbol, that it is a sheet value** rather than a constant at a line.
///
/// The pages write it two ways: "sheet value" in prose, and `metrics.layout.<name>` in a table cell. Either is a
/// statement a reader can follow to the sheet.
fn says_sheet_value(doc: &str, symbol: &str) -> bool {
    let key = symbol.to_lowercase();
    doc.lines().any(|line| {
        line.contains(symbol)
            && (line.to_lowercase().contains("sheet value")
                || line.contains(&format!("metrics.layout.{key}"))
                || line.contains(&format!("metrics.type.{key}")))
    })
}

// ── #588: the four `Format version` rows ──────────────────────────────────────────────────────

/// **Every `Format version` row in `data-files.md` that names a constant states what the constant holds** (#588).
///
/// Four rows disagreed: `map.yaml` said 7 against `MAP_VERSION` 9, `vocab.yaml` and `forest.yaml` said "None."
/// against `VOCAB_VERSION` 1 and `FOREST_VERSION` 1, and `repetition.yaml` had no row at all against
/// `REPETITION_VERSION` 1. A row is now read against the constant by name, so a bump is a failure here rather than
/// a reader learning the wrong number.
#[test]
fn the_data_files_format_versions_are_the_constants() {
    let sources = sources_by_name();
    let rows = stated_versions();
    assert!(rows.len() >= 10, "docs/guide/data-files.md states only {} constant-bearing rows", rows.len());
    let mut wrong = Vec::new();
    let mut read: Vec<&str> = Vec::new();
    for (section, constant, said, line) in &rows {
        let actual = constant_value(constant, &sources);
        read.push(constant);
        if *said != actual {
            wrong.push(format!("{section}: says {said} ({constant}), which is {actual}   [{line}]"));
        }
    }
    assert!(
        wrong.is_empty(),
        "**docs/guide/data-files.md states a version the code does not compile** (#588). A person writing a \
         migration reads this page to learn what number to put in a file:\n{}",
        wrong.join("\n")
    );
    // **The four rows #588 was filed about are all read**, so a later edit that drops one fails here rather than
    // leaving the constant unguarded.
    for constant in ["MAP_VERSION", "VOCAB_VERSION", "FOREST_VERSION", "REPETITION_VERSION"] {
        assert!(
            read.contains(&constant),
            "docs/guide/data-files.md has no `Format version` row naming `{constant}` (#588)"
        );
    }
}

// ── #587: the `Live reload` column ────────────────────────────────────────────────────────────

/// **Every `Live reload` row in `data-files.md` agrees with the reload contract's own `class` column** (#587).
///
/// `rigs.yaml` and `oplog.jsonl` both said "No. The editor does not watch this file" while the contract said `Live`
/// and both are in fact watched (`anim_watch::sync_rigs_reload` registered at `anim_tab.rs:302`;
/// `oplog_view::sync_oplog_view` registered at `after_edit.rs:557`). The contract is rendered from `reload::KINDS`,
/// so it is the code's own answer; this reads the page against it.
///
/// The comparison is over the **class word**: a row starting `Yes` is a `Live` or `Respawn` kind, a row starting
/// `No` is a `Restart` kind, and a section that is not a project file (a draft, a snip, a feedback report) is in
/// [`section_kinds`] with no kinds and so is not compared.
#[test]
fn the_data_files_live_reload_rows_are_the_reload_contracts() {
    let contract = read("docs/guide/reload-contract.md");
    let mut classes: BTreeMap<&str, &str> = BTreeMap::new();
    for line in contract.lines() {
        if !line.starts_with("| `") {
            continue;
        }
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // `| kind | path | parses to | class | lifetime | written by | how |` is 9 pieces with the outer pipes.
        if cells.len() < 6 {
            continue;
        }
        classes.insert(cells[1].trim_matches('`'), cells[4]);
    }
    assert!(classes.len() >= 35, "the reload contract's table held {} kinds, which is short", classes.len());

    let by_kind = section_kinds();
    let doc = read("docs/guide/data-files.md");
    let mut section = String::new();
    let mut rows: BTreeMap<String, String> = BTreeMap::new();
    for line in doc.lines() {
        if let Some(head) = line.strip_prefix("## ") {
            section = head.trim().to_owned();
            continue;
        }
        let Some(row) = line.strip_prefix("| Live reload | ") else { continue };
        rows.insert(section.clone(), row.trim().to_owned());
    }
    assert!(rows.len() >= 15, "docs/guide/data-files.md has only {} `Live reload` rows", rows.len());

    let mut wrong = Vec::new();
    let mut read = 0usize;
    let mut checked_sections = 0usize;
    for (heading, line) in &rows {
        let kinds = by_kind.get(heading.as_str()).unwrap_or_else(|| {
            panic!(
                "docs/guide/data-files.md has a section `{heading}` that `section_kinds` does not name, so its \
                 `Live reload` row is unchecked"
            )
        });
        if kinds.is_empty() {
            continue;
        }
        checked_sections += 1;
        for kind in kinds {
            let Some(class) = classes.get(kind) else {
                panic!("`section_kinds` names `{kind}` for `{heading}`, which the reload contract does not hold")
            };
            read += 1;
            // **`Respawn` reads as `Yes`**: the editor does watch it, which is all the page's column claims.
            let watched = line.starts_with("Yes");
            let wanted = *class != "Restart";
            if watched != wanted {
                wrong.push(format!("{heading} (`{kind}`, the contract says `{class}`): {line}"));
            }
        }
    }
    assert!(checked_sections >= 15, "only {checked_sections} sections of data-files.md named a kind");
    assert!(read >= 16, "only {read} of data-files.md's rows resolved to a kind of the contract");
    assert!(
        wrong.is_empty(),
        "**docs/guide/data-files.md's `Live reload` column disagrees with the reload contract (#587)**, which is \
         rendered from `reload::KINDS`. The contract is the code's own answer:\n{}",
        wrong.join("\n")
    );
}

// ── #586: every kind is a section or delegated ────────────────────────────────────────────────

/// **Every kind `reload::KINDS` declares is one `data-files.md` has a section for, or the page delegates by name**
/// (#586).
///
/// The page used to open "This page lists every file in a bevy_blueprint project" over 23 of the 39 kinds. It now
/// says it lists the **authored** ones and names the rest, with a link to the reload contract. That sentence is
/// what makes the page honest, so this reads it: a kind added to `KINDS` with neither a section nor a place in the
/// sentence fails here.
#[test]
fn every_kind_is_listed_or_delegated() {
    let doc = read("docs/guide/data-files.md");
    const MARKER: &str = "<!-- data-files:delegated -->";
    let at = doc.find(MARKER).unwrap_or_else(|| panic!("docs/guide/data-files.md has no `{MARKER}` paragraph"));
    let rest = &doc[at + MARKER.len()..];
    let delegation = &rest[..rest.find("\n\n").unwrap_or(rest.len())];

    // **The kinds the page has a section for**, from the table the other two tests use, so the three cannot disagree
    // about which section carries which kind.
    let sectioned: Vec<&str> = section_kinds().values().flatten().copied().collect();
    let mut missing = Vec::new();
    for kind in KINDS {
        let listed = sectioned.contains(&kind.id);
        if !listed && !delegation.contains(&format!("`{}`", kind.id)) {
            missing.push(format!("`{}` ({}): no section and not in the delegation sentence", kind.id, kind.pattern));
        }
    }
    assert!(
        missing.is_empty(),
        "**docs/guide/data-files.md neither has a section for {} kind(s) nor names them as delegated** (#586). A \
         reader takes the page's own word, so a kind it does not carry has to be named where the page says which \
         page does:\n{}",
        missing.len(),
        missing.join("\n")
    );
    // **The sentence's count is the number of names it spells**, so "21 kinds" cannot survive a dropped name.
    let spelled = delegation.matches("`").count() / 2;
    assert!(
        spelled >= 20,
        "the delegation sentence spells only {spelled} backticked names; it said 21 kinds when this was written"
    );
    assert!(
        delegation.contains(&format!("{spelled} kinds")),
        "the delegation sentence spells {spelled} names and does not say `{spelled} kinds`: {delegation}"
    );
}
