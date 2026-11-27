//! **The counts `docs/PROJECT.md` states are the code's own, not a copy that drifted** (#473 review).
//!
//! PROJECT.md tells a reader how big the editor is: how many operations `ops::registry()` builds, and
//! how many of those a default build has (the family crates' operations are behind cargo features).
//! Both had gone stale — the registry built 138 entries while the document said 137, and a default
//! build 131 while it said 130 — and a count nobody checks is a count that is wrong once and stays
//! wrong.
//!
//! **The default build's number is measured by calling the registry**, so it cannot drift: this test
//! compiles with the default features, and `ops::registry().len()` is exactly the count the document's
//! "in a default build" clause means. The total is the rows `registry()` builds, the derivation the
//! document names.
//!
//! **The other pages of the same shape are here too** (#561, #563, #564, #565, #566, #567, #570, #571): each
//! test reads a document the repository ships and the same fact out of the code, so a number that drifts
//! fails a build instead of a reader. `docs/PROJECT.md`'s tool count, the reload contract's extension set and
//! kind counts, the two design pages' sizes, the guide's keys page, `AGENTS.md`'s family table, and every
//! intra-repo markdown fragment under `docs/guide/`. The MCP crate's own pages (#562, #569) are tested in
//! `dark_factory_mcp`, because the editor crate does not depend on the server.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use blueprint_model::reload::{self, KINDS};

/// The document, as the repository holds it.
const PROJECT: &str = include_str!("../../../docs/PROJECT.md");

/// **The number immediately before `marker` in `text`**: the count a phrase states, read from the
/// digits that end just where the phrase begins.
fn count_before(text: &str, marker: &str) -> Option<usize> {
    let at = text.find(marker)?;
    let head: String = text[..at].chars().rev().take_while(char::is_ascii_digit).collect();
    head.chars().rev().collect::<String>().parse().ok()
}

/// **Every line count `docs/PROJECT.md` states names the commit it was counted at, and the editor's is spelled
/// one way** (#659).
///
/// The page said `294,204` in its crate table, `294,204 … (343,045 today)` in §6.1 and `343,000` in its closing
/// line — three numbers for one measurement, and the two "today" numbers were 4,361 lines stale at
/// `2fa6cce`. The banner at `:5` promised that "every other number is the snapshot's, counted at `198b82ae`
/// where the sentence says so", and nothing read the sentence for the "where it says so" half, so a number
/// presented as today's read as a measurement of the tree.
///
/// **The other numbers that change without a commit** are outside a test's reach: an `Open issues` count is a
/// GitHub query, not a repository fact, and the page now says so and links to the live list. That is why this
/// test reads line counts only — they are the ones the tree decides.
#[test]
fn the_project_pages_line_counts_name_their_commit() {
    let page = read("docs/PROJECT.md");
    let mut claims: Vec<(usize, String, String)> = Vec::new();
    let mut unsourced = Vec::new();
    let mut editor_counts: BTreeSet<String> = BTreeSet::new();
    for (n, line) in page.lines().enumerate() {
        let mut from = 0;
        while let Some(at) = line[from..].find(|c: char| c.is_ascii_digit()) {
            let start = from + at;
            // **The digits with their thousands separators**, then the word that makes them a line count.
            let digits: String = line[start..].chars().take_while(|c| c.is_ascii_digit() || *c == ',').collect();
            let rest = &line[start + digits.len()..];
            let unit = if rest.starts_with("-line") {
                Some("-line")
            } else if rest.starts_with(" lines") || rest.starts_with(" line") {
                Some("lines")
            } else {
                None
            };
            let Some(unit) = unit else {
                from = start + digits.len();
                continue;
            };
            from = start + digits.len() + unit.len();
            let Some(value) = digits.replace(',', "").parse::<usize>().ok() else { continue };
            // **A count under ten thousand is not a line count of this repository** (`4 lines`, `9 lines` in
            // prose about something else), so only the six-figure ones are read.
            if value < 10_000 {
                continue;
            }
            claims.push((n + 1, value.to_string(), line.to_owned()));
            // **The commit, in the same sentence** — or, for a table row, in the header that columns it. A
            // crate table states its commit once in the header (`| Crate | Lines (at `198b82ae`) |`) and every
            // row below inherits it, which is the same claim; a row that named a commit of its own would be a
            // different number in the same column.
            let sentence_start = line[..start].rfind(". ").map(|i| i + 2).unwrap_or(0);
            let mut cited = cites_a_commit(&line[sentence_start..]);
            if !cited && line.starts_with('|') {
                cited = table_header_before(&page, n).is_some_and(|header| cites_a_commit(&header));
            }
            if !cited {
                unsourced.push(format!(":{}: `{digits}{unit}` names no commit", n + 1));
            }
            // **The editor's own count**, whose three spellings disagreed.
            if line.contains("bevy_dark_factory` is") || line.contains("editor at its centre") || line.contains("| `bevy_dark_factory` |") {
                editor_counts.insert(value.to_string());
            }
        }
    }
    assert!(claims.len() >= 3, "only {} line count(s) were read from docs/PROJECT.md: the page's shape has changed", claims.len());
    assert!(
        unsourced.is_empty(),
        "**{} line count(s) in docs/PROJECT.md name no commit**: {}. A count with no commit reads as a \
         measurement of the tree, and today's tree is a different number (#659).",
        unsourced.len(),
        unsourced.join(", ")
    );
    assert_eq!(
        editor_counts.len(),
        1,
        "**docs/PROJECT.md states the editor's line count as {editor_counts:?}**, more than one number for one \
         measurement. The crate table, §6.1 and the closing line say the same quantity, so they say the same \
         digits (#659)."
    );
}

/// **Whether a fragment of the page names a commit**: a backticked short sha (seven or more hex characters, at
/// least one a digit, so a word of a-f letters is not read as one).
fn cites_a_commit(text: &str) -> bool {
    text.split('`').skip(1).step_by(2).any(|span| {
        span.len() >= 7 && span.chars().all(|c| c.is_ascii_hexdigit()) && span.chars().any(|c| c.is_ascii_digit())
    })
}

/// **The header row above a markdown table row**, walking back to the line that opens with `| ---`, then to
/// the header above it. `None` when the row is not inside a table.
fn table_header_before(page: &str, row: usize) -> Option<String> {
    let lines: Vec<&str> = page.lines().collect();
    // `row` is the 1-based line number of the data row, so `lines[row - 1]` is it.
    let mut at = row.checked_sub(1)?;
    while at > 0 {
        at -= 1;
        let line = lines.get(at)?.trim();
        if !line.starts_with('|') {
            return None;
        }
        // **The `| --- | --- |` separator**: the line above it is the header.
        if line.chars().filter(|c| *c == '-').count() >= 3 && line.chars().all(|c| c == '|' || c == '-' || c == ':' || c == ' ') {
            return lines.get(at.checked_sub(1)?).map(|header| (*header).to_owned());
        }
    }
    None
}

/// **Every row `ops::registry()` builds**, counted from its own source: the total the document's
/// "`ops::registry()`" clause names, feature gates included.
fn registry_rows() -> usize {
    let source = include_str!("../src/ops.rs");
    let start = source.find("pub fn registry() -> Vec<Box<dyn Operation>> {").expect("ops.rs declares registry()");
    let body = &source[start..];
    let end = body.find("\n}\n").expect("registry() has an end");
    body[..end].lines().filter(|line| line.trim_start().starts_with("Box::new(")).count()
}

#[test]
fn the_prose_states_the_registrys_own_row_count() {
    let rows = registry_rows();
    let said = count_before(PROJECT, " entries in the operation registry");
    assert_eq!(
        said,
        Some(rows),
        "**`docs/PROJECT.md` says {said:?} entries in the operation registry and `ops::registry()` builds {rows}**. \
         Fix the document, not this test."
    );
    let table = count_before(PROJECT, " in the registry, ");
    assert_eq!(table, Some(rows), "the summary table says {table:?} operations in the registry and it builds {rows}");
}

#[test]
fn the_prose_states_what_this_default_build_registers() {
    let built = bevy_dark_factory::ops::registry().len();
    // **A `family_all` build is the one that registers every row** (the gate builds it, #687), so it is held to
    // the registry's own total; a default build is held to the document's default-build number. Each build checks
    // the number that describes it, measured by calling the registry rather than derived from the source.
    if cfg!(feature = "family_all") {
        let rows = registry_rows();
        assert_eq!(built, rows, "a family_all build registers {built} operations and `ops::registry()` has {rows} rows: every row is compiled in");
        return;
    }
    let said = count_before(PROJECT, " in a default build");
    assert_eq!(
        said,
        Some(built),
        "**`docs/PROJECT.md` says {said:?} entries in a default build and this build registers {built}**. \
         Fix the document, not this test."
    );
    let table = count_before(PROJECT, " in a default build |");
    assert_eq!(table, Some(built), "the summary table says {table:?} in a default build and this build registers {built}");
}

/// **The repository's root**, from this crate's manifest.
fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// **The text of a repository file**, or a panic naming it.
fn read(rel: &str) -> String {
    let path = root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// **The digits that follow `lead` and end at `suffix` in `text`**, as a number.
///
/// The same reader `dark_factory_mcp`'s tool-count test uses, because the two are asking the same
/// question of the same prose: "what number does this sentence state?".
fn count_between(text: &str, lead: &str, suffix: &str) -> Option<usize> {
    let at = text.find(lead)? + lead.len();
    let digits: String = text[at..].chars().take_while(char::is_ascii_digit).collect();
    text[at + digits.len()..].starts_with(suffix).then(|| digits.parse().ok()).flatten()
}

/// **How many kinds core's own checked-in list names**: `blueprint_model::reload_check::unchecked()`, the same read the
/// other tests make, so the page's "the other N" is measured against the list that decides it rather than against a copy.
fn unchecked_count() -> usize {
    blueprint_model::reload_check::unchecked().count()
}

/// **Every `kind("id", …)` row of `blueprint_model::reload::KINDS`**, by id, for a document to be read
/// against.
fn kind_ids() -> BTreeSet<&'static str> {
    KINDS.iter().map(|k| k.id).collect()
}

// ── #563 and #564: the reload contract's sentence about the watcher and the clock ────────────────

/// **The reload contract names the extension set the game's watcher listens for, and the watcher's own
/// `FileWatch::new` calls are that set.**
///
/// The page said `{ron}`, a leftover from before the RON-to-YAML cutover (#365, GD-03), in the one
/// paragraph that tells a reader what a running game notices. It is generated prose about code, so the two
/// are read together: a sentence naming an extension set the code does not pass to `FileWatch::new` fails.
#[test]
fn the_reload_contract_names_the_extension_set_the_watchers_listen_for() {
    let guide = read("docs/guide/reload-contract.md");
    // **The sentence**: "the extension set `{…}`", as the audit's first bullet words it.
    const LEAD: &str = "the extension set `{";
    let at = guide.find(LEAD).unwrap_or_else(|| {
        panic!(
            "docs/guide/reload-contract.md no longer has a sentence `{LEAD}…}}`; the audit bullet that \
             says what the game's watcher notices was the thing #563 fixed. If it moved, move this test."
        )
    });
    let rest = &guide[at + LEAD.len()..];
    let said: BTreeSet<String> = rest[..rest.find('}').expect("the set closes")].split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty()).collect();

    // **What the watchers are given**, from the two `FileWatch::new` calls in the game's runtime crate.
    let mut listened = BTreeSet::new();
    for (file, text) in [
        ("crates/bevy_blueprint/src/live.rs", read("crates/bevy_blueprint/src/live.rs")),
        ("crates/bevy_blueprint/src/entities.rs", read("crates/bevy_blueprint/src/entities.rs")),
    ] {
        let mut seen = 0;
        let mut from = 0;
        while let Some(found) = text[from..].find("FileWatch::new(") {
            let call = from + found;
            from = call + "FileWatch::new(".len();
            let close = closing_paren(&text, call);
            let args = &text[call..close];
            // **The last `&[` in the call is the extension list**; the first argument is a path.
            if let Some(open) = args.rfind("&[") {
                let list = &args[open + 2..];
                let list = &list[..list.find(']').expect("the list closes")];
                for ext in list.split(',') {
                    let ext = ext.trim().trim_matches('"');
                    if !ext.is_empty() {
                        listened.insert(ext.to_owned());
                    }
                }
                seen += 1;
            }
        }
        assert!(seen >= 1, "{file} no longer calls `FileWatch::new`; this test reads that call");
    }
    assert!(!listened.is_empty(), "no `FileWatch::new(…, &[…])` extension list was found");
    assert_eq!(
        said, listened,
        "**docs/guide/reload-contract.md says the game's watcher listens for `{{{}}}`, and the two \
         `FileWatch::new` calls pass `{{{}}}`**. The page's sentence is about the code, so the code is \
         right: fix the sentence (#563).",
        said.iter().map(String::as_str).collect::<Vec<_>>().join(","),
        listened.iter().map(String::as_str).collect::<Vec<_>>().join(",")
    );
}

/// **No sentence in the reload contract says family settings are polled**, and no `.ron` survives outside
/// the cutover's history.
///
/// #538 retired `SETTINGS_POLL_SECS`; `family_host::reload_settings` has no `Timer`, no `Local<f32>` and no
/// delta, and `docs/guide/reload-contract.md`'s own checklist item 23 records the retirement nine lines
/// below the table row that still claimed it (#564).
#[test]
fn the_reload_contract_says_no_family_poll_and_no_ron() {
    let guide = read("docs/guide/reload-contract.md");
    let mut wrong = Vec::new();
    for line in guide.lines() {
        let lower = line.to_lowercase();
        if lower.contains("polled every 0.1 s") || lower.contains("every 0.1 s") && lower.contains("family") {
            wrong.push(format!("says family settings are polled: {}", line.trim()));
        }
    }
    assert!(wrong.is_empty(), "docs/guide/reload-contract.md still names a clock on the family settings:\n{}", wrong.join("\n"));

    // **`.ron` is cutover history.** The page may say RON where it is describing the migration it did
    // (`RON to YAML`, "a project written before the YAML cutover", `serde_path_to_error` on RON text) and
    // may not name a `.ron` extension as something a program reads or writes today.
    let mut ron = Vec::new();
    for line in guide.lines() {
        let lower = line.to_lowercase();
        let names_a_file = lower.contains(".ron") || lower.contains("`ron`") || lower.contains("{ron}");
        let history = lower.contains("cutover") || lower.contains("migration") || lower.contains("migrate") || lower.contains("before the yaml");
        if names_a_file && !history {
            ron.push(line.trim().to_owned());
        }
    }
    assert!(ron.is_empty(), "docs/guide/reload-contract.md names RON as a live format:\n{}", ron.join("\n"));
}

/// **The `family` kind's own note is the same story the code tells**, and it lives in the generator so the
/// next render cannot put the clock back.
///
/// The row is rendered from `blueprint_model::reload::KINDS`, so the markdown was never the place to fix it
/// (#564).
#[test]
fn the_family_kinds_note_names_the_project_index_not_a_clock() {
    let family = reload::kind_by_id("family").expect("`family` is a kind of the registry");
    let note = family.note.to_lowercase();
    assert!(
        !note.contains("polled") && !note.contains("0.1 s"),
        "`blueprint_model::reload::KINDS`'s `family` row says a clock still reads the file: `{}`. \
         `family_host::reload_settings` has no timer (#538): the project index publishes the change.",
        family.note
    );
    assert!(
        note.contains("project index") || note.contains("index"),
        "`family`'s note should say what does read the file now — the project index: `{}`",
        family.note
    );
}

// ── #565: the reload contract's kind counts ──────────────────────────────────────────────────────

/// **Every count the reload contract states in its two version sections is the length of the list it
/// describes.**
///
/// The page said "24 of the 33 kinds" against a table of 39 rows and a registry of 39 kinds, and "the other
/// 9" against a checked-in list of 15. A reader adding "24 + 9 + 4 owned elsewhere" got 37 and could not
/// find the missing two.
#[test]
fn the_reload_contracts_kind_counts_are_the_registrys() {
    let guide = read("docs/guide/reload-contract.md");
    let registry = kind_ids();
    let total = KINDS.len();

    // **"`check` covers N of the M kinds"**, the sentence #565 is about.
    const COVERS: &str = "`check` covers ";
    let checked = count_between(&guide, COVERS, " of the ")
        .unwrap_or_else(|| panic!("docs/guide/reload-contract.md no longer says `check` covers N of the M kinds"));
    // **The `M` is read from the same sentence**, so a second `of the … kinds` elsewhere on the page cannot
    // answer for it.
    let after = &guide[guide.find(COVERS).expect("the sentence")..];
    let named_total = count_between(after, " of the ", " kinds")
        .unwrap_or_else(|| panic!("the sentence's `of the M kinds` no longer reads as a number"));
    assert_eq!(named_total, total, "the page says the registry has {named_total} kinds and `KINDS` has {total}");
    // **The other list** is the one the checked-in `UNCHECKED` in `reload_verdicts.rs` holds; its length is
    // that file's `UNCHECKED_MAX`, which must equal its own list.
    let other = count_between(&guide, "The other ", " are on a checked-in list")
        .unwrap_or_else(|| panic!("the page no longer says how many kinds are unchecked"));
    let unchecked = unchecked_count();
    assert_eq!(
        other, unchecked,
        "the page says {other} kinds are on the checked-in list and `reload_verdicts.rs`'s `UNCHECKED` has \
         {unchecked}"
    );
    assert_eq!(
        checked + unchecked,
        total,
        "`check` covers {checked} and {unchecked} are unchecked, which is not the registry's {total}: the \
         page's two numbers must add up"
    );

    // **The version section's bullets**, each of whose stated count must equal both the list it names in
    // the version test and the number of names the bullet itself spells. Both, because a number and a list
    // can agree while both are wrong.
    let version_test = read("crates/blueprint_model/tests/every_format_carries_a_version.rs");
    let bullets: [(&str, &str, usize); 6] = [
        // (lead, the word-after-the-number, what the version test holds)
        ("- **Versioned** (", " kinds", sample_count(&version_test)),
        (
            "- **Gained a version in slice 2** (",
            " of those kinds",
            6,
        ),
        ("- **Versioned above this crate** (", " kinds", tuple_count(&version_test, "VERSIONED_ABOVE") - 1),
        ("- **Versioned above this crate, the log (slice 4)** (", " kind", 1),
        ("- **Unversioned** (", " kinds", tuple_count(&version_test, "UNVERSIONED")),
        ("- **Owned elsewhere** (", " kinds", tuple_count(&version_test, "OWNED_ELSEWHERE")),
    ];
    for (lead, suffix, actual) in bullets {
        let said = count_between(&guide, lead, suffix)
            .unwrap_or_else(|| panic!("the guide's bullet `{lead}…` no longer states a count"));
        assert_eq!(
            said,
            actual,
            "**docs/guide/reload-contract.md's bullet `{lead}` says {said} and the version test places \
             {actual}**. The bullet's number and the test's list move together."
        );
        // **And the names the bullet spells are that many**, and every one is a kind of the registry. A
        // bullet that spells none (`Unversioned`, whose list is empty, and the log's single kind) is counted
        // by the version test alone.
        let named = spelled_names(&guide, lead);
        if !named.is_empty() {
            assert_eq!(
                said,
                named.len(),
                "the bullet `{lead}` says {said} and spells {} name(s): {named:?}",
                named.len()
            );
        }
        for name in &named {
            assert!(registry.contains(name.as_str()), "the bullet `{lead}` names `{name}`, which is no kind of `KINDS`");
        }
    }

    // **The slice-2 bullet's six names are six of the versioned kinds**, so a kind that gained its version
    // there cannot be dropped from the sentence while the number stays.
    let gained = spelled_names(&guide, "- **Gained a version in slice 2** (");
    let sampled = sample_ids(&version_test);
    for name in &gained {
        assert!(sampled.contains(name.as_str()), "the slice-2 bullet names `{name}`, which `versioned()` does not sample");
    }
}

/// **The reload contract's second census — the one that restates the first as three clauses and a sum —
/// names every kind the code's three lists hold** (#654).
///
/// Line 119 already named all 15 the editor's layer covers; line 149 said the same thing in different words,
/// named 5 of the 8 watchers and called the result exhaustive — 33 accounted for in a 39-kind registry. The
/// page's own tests read only the "`check` covers N of the M kinds" sentence, so a correct census and a stale
/// one sat beside each other with every test green.
///
/// **Three assertions, each a way the sentence can be wrong**: the counts it gives are the lengths of the three
/// lists; the sum it states is the registry's; and each clause spells exactly the ids its list holds, so a
/// count and a list cannot agree while both are wrong.
#[test]
fn the_reload_contracts_second_census_names_the_three_lists() {
    let guide = read("docs/guide/reload-contract.md");
    let sentence = guide
        .split("With these two, every kind in the registry has a reader")
        .nth(1)
        .unwrap_or_else(|| panic!("docs/guide/reload-contract.md no longer has the second census sentence"));
    let sentence = sentence.split("\n\n").next().unwrap_or(sentence);

    let checked = blueprint_model::reload::KINDS.len() - unchecked_count();
    let read_here: BTreeSet<&str> = bevy_dark_factory::map_files::READ_HERE.iter().copied().collect();
    let watched: BTreeSet<&str> = bevy_dark_factory::map_files::WATCHED_HERE.iter().copied().collect();
    let still = still_unchecked();
    assert_eq!(
        checked + read_here.len() + watched.len() + still.len(),
        blueprint_model::reload::KINDS.len(),
        "the four lists do not partition the registry: {checked} + {} + {} + {} != {}",
        read_here.len(),
        watched.len(),
        still.len(),
        blueprint_model::reload::KINDS.len()
    );

    // **The three numbers the sentence states**, each a fact about a list.
    let said_checked = count_between(sentence, "`reload_check::check` for ", ",")
        .unwrap_or_else(|| panic!("the second census no longer says how many kinds `check` covers"));
    assert_eq!(said_checked, checked, "the second census says `check` covers {said_checked} and it covers {checked}");
    // **The sum the sentence spells**, which is the registry's whole list.
    let sum_at = sentence
        .find("That is ")
        .unwrap_or_else(|| panic!("the second census no longer states its sum (`That is 24 + 4 + 8 + 3 = 39`)"));
    let sum_text: String = sentence[sum_at + "That is ".len()..].chars().take_while(|c| *c != ',' && *c != '.').collect();
    let parts: Vec<usize> = sum_text
        .split('=')
        .next()
        .unwrap_or_default()
        .split('+')
        .filter_map(|part| part.trim().parse().ok())
        .collect();
    let stated_total: Option<usize> = sum_text.split('=').nth(1).and_then(|n| n.trim().parse().ok());
    assert_eq!(parts.len(), 4, "the sum `That is {sum_text}` no longer adds four numbers: {parts:?}");
    assert_eq!(
        parts,
        vec![checked, read_here.len(), watched.len(), still.len()],
        "the second census adds {parts:?} and the four lists are [check {checked}, READ_HERE {}, WATCHED_HERE {}, \
         STILL_UNCHECKED {}]",
        read_here.len(),
        watched.len(),
        still.len()
    );
    assert_eq!(
        stated_total,
        Some(blueprint_model::reload::KINDS.len()),
        "the second census says the sum is {stated_total:?} and the registry has {} kinds",
        blueprint_model::reload::KINDS.len()
    );

    // **Each clause spells the ids its list holds.** The first clause's `24` is a number alone (the 24 are
    // core's, and the sentence has no room for them), so only the two named clauses are read here, and the
    // ratchet list's three.
    let readers = clause_names(sentence, "the editor's readers for ", ", its watchers");
    assert_eq!(readers, read_here, "the readers clause spells {readers:?} and `READ_HERE` is {read_here:?}");
    let named_watchers = clause_names(sentence, "its watchers for ", ", and the mapper's ratchet");
    assert_eq!(named_watchers, watched, "the watchers clause spells {named_watchers:?} and `WATCHED_HERE` is {watched:?}");
    let named_still = clause_names(sentence, "(`STILL_UNCHECKED`) for ", ", which cannot name a field");
    assert_eq!(named_still, still, "the ratchet clause spells {named_still:?} and `STILL_UNCHECKED` is {still:?}");
}

/// **The backticked ids a census clause spells** between `lead` and `tail`, as a set, borrowed from `sentence`.
fn clause_names<'a>(sentence: &'a str, lead: &str, tail: &str) -> BTreeSet<&'a str> {
    let at = sentence.find(lead).unwrap_or_else(|| panic!("no clause `{lead}`")) + lead.len();
    let end = sentence[at..].find(tail).unwrap_or_else(|| panic!("no clause tail `{tail}`")) + at;
    sentence[at..end].split('`').skip(1).step_by(2).collect()
}

/// **The kinds on the editor's own ratchet of kinds nothing reads**, read from the test that holds the list:
/// `a_file_that_does_not_read_is_never_overwritten.rs`'s `STILL_UNCHECKED`, which is checked in and ratcheted.
fn still_unchecked() -> BTreeSet<&'static str> {
    const SOURCE: &str = include_str!("a_file_that_does_not_read_is_never_overwritten.rs");
    let at = SOURCE.find("const STILL_UNCHECKED: &[&str] = &[").expect("the ratchet declares `STILL_UNCHECKED`");
    let body = &SOURCE[at..];
    let end = body.find("];").expect("the list closes");
    // **The ids are the quoted spans on their own lines**, and the reasons above them are comments: one id per
    // line, a `//` line skipped.
    let mut ids: BTreeSet<&str> = BTreeSet::new();
    for line in body[..end].lines().skip(1) {
        let trimmed = line.trim();
        if trimmed.starts_with("//") || trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('"')
            && let Some(id) = rest.split('"').next()
        {
            ids.insert(id);
        }
    }
    assert!(!ids.is_empty(), "no ids were read from `STILL_UNCHECKED`");
    ids
}

/// **The names a version-section bullet spells** after its colon, as `"id"`s: `` `forest`, `people` `` reads
/// as two. The clause after the last name (`; slice 3`) and a `from slice 3` note are not names.
fn spelled_names(guide: &str, lead: &str) -> Vec<String> {
    let at = guide.find(lead).unwrap_or_else(|| panic!("no bullet `{lead}`")) + lead.len();
    let open = guide[..at].rfind('(').expect("the bullet opens a parenthesis");
    let close = guide[open..].find(')').expect("the bullet closes") + open;
    let body = &guide[at..close];
    let Some((_, names)) = body.split_once(':') else { return Vec::new() };
    // **A list a writer punctuates** is split on commas and on the `and` before a last item, because prose
    // does both (`forest`, `people`, ... and `editor_style`), and a split on commas alone read five names from
    // a bullet that spells six (the 2026-11-23 gate).
    names
        .split(',')
        .map(|part| part.trim())
        .map(|part| part.strip_prefix("and ").unwrap_or(part))
        .filter(|part| part.starts_with('`'))
        .filter_map(|part| part.split('`').nth(1))
        .map(str::to_owned)
        .collect()
}

/// **The `)` that closes the `(` of a call**, by counting depth: an argument may hold a call of its own
/// (`&self.root.join(ENTITIES_DIR)`), and reading to the first `)` stopped inside it, so the extension list
/// after it was never seen (the 2026-11-23 gate).
fn closing_paren(text: &str, call_at: usize) -> usize {
    let open = text[call_at..].find('(').expect("the call opens a parenthesis") + call_at;
    let mut depth = 0usize;
    for (offset, ch) in text[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return open + offset;
                }
            }
            _ => {}
        }
    }
    panic!("the call at {call_at} never closes");
}

/// **The `s("id", …)` sample ids `fn versioned()` builds**, which the version test asserts are one per kind
/// it can parse.
fn sample_ids(source: &str) -> BTreeSet<&str> {
    let body = &source[source.find("fn versioned()").expect("versioned()")..];
    let body = &body[..body.find("\n}").expect("the function ends")];
    // **One row starts with `s(`**, whether its id is on that line or the next: the long samples are written
    // across lines, so the id is the first quoted word after the `s(`.
    body.split("s(")
        .skip(1)
        .filter_map(|rest| rest.trim_start().strip_prefix('"'))
        .filter_map(|rest| rest.split('"').next())
        .collect()
}

/// **The `("id", …)` tuples a `const NAME: &[(&str, …)]` holds** in a source file: `VERSIONED_ABOVE`,
/// `UNVERSIONED`, `OWNED_ELSEWHERE`.
fn tuple_count(source: &str, name: &str) -> usize {
    let at = source.find(&format!("const {name}:")).unwrap_or_else(|| panic!("the version test declares `{name}`"));
    let body = &source[at..];
    let end = body.find("];").expect("the list closes");
    body[..end].matches("(\"").count()
}

/// **How many sample rows `fn versioned()` builds.**
fn sample_count(source: &str) -> usize {
    sample_ids(source).len()
}

// ── #566: the type roles' sizes are the sheet's ──────────────────────────────────────────────────

/// **`docs/design/text-roles.md`'s role table states each role's size as `metrics.type` has it.**
///
/// The page said `HEADING` was 10 three times over; the sheet says 11, and `chrome.rs` records the owner's
/// #518 decision (a heading outranks the label it heads) that put it there. The page's §1 capture numbers
/// are the snapshot's and are not read here — only the table, which the page itself says is the sheet's.
#[test]
fn the_text_roles_table_states_the_sheets_sizes() {
    let doc = read("docs/design/text-roles.md");
    let metrics = bevy_dark_factory::style::compiled_default().metrics.r#type;
    for (role, px) in [
        ("TITLE", metrics.title),
        ("TAB", metrics.tab),
        ("BODY", metrics.body),
        ("HEADING", metrics.heading),
        ("LABEL", metrics.label),
        ("CONTROL", metrics.control),
        ("HINT", metrics.hint),
    ] {
        // **The row of §2's table**: `| \`ROLE\` | **N** | …`.
        let lead = format!("| `{role}` | **");
        let at = doc.find(&lead).unwrap_or_else(|| panic!("docs/design/text-roles.md has no `{role}` row"));
        let said: String = doc[at + lead.len()..].chars().take_while(|c| c.is_ascii_digit()).collect();
        let said: f32 = said.parse().unwrap_or_else(|_| panic!("the `{role}` row states no size"));
        assert!(
            (said - px).abs() < f32::EPSILON,
            "**docs/design/text-roles.md says `{role}` is {said} px and `metrics.type` has {px}**. The page's \
             own banner says these sizes come from the sheet, so the sheet is right (#566)."
        );
    }
    // **No line of the page says HEADING is anything but the sheet's number.**
    let px = metrics.heading;
    for line in doc.lines() {
        if line.contains("HEADING") && (line.contains(" px") || line.contains("**10**")) {
            assert!(
                line.contains(&format!("{px}")) || line.contains(&format!("**{px}**")),
                "docs/design/text-roles.md states a `HEADING` size that is not the sheet's {px}: {}",
                line.trim()
            );
        }
    }
}

/// **The symbol citations in `docs/design/text-roles.md` land on the symbol they name.**
///
/// `chrome::section` was cited at `chrome.rs:991` and sat at `:827`; the role module was cited at `:690` and sat at
/// `:531` (#566). The line number is read from the page, not pinned here: pinning it made this test and the citation
/// resolver demand different numbers whenever `chrome.rs` moved (#680). The page must still cite each symbol, and
/// the line it gives must be that symbol's declaration.
#[test]
fn the_text_roles_citations_land_on_their_symbols() {
    let doc = read("docs/design/text-roles.md");
    let chrome = read("crates/bevy_dark_factory/src/chrome.rs");
    let lines: Vec<&str> = chrome.lines().collect();
    for (symbol, needle) in [("`chrome::section` (`chrome.rs:", "pub fn section"), ("`chrome::text` is seven roles over five sizes (`chrome.rs:", "pub mod text")] {
        let at = doc.find(symbol).unwrap_or_else(|| panic!("docs/design/text-roles.md no longer cites {symbol}…`"));
        let rest = &doc[at + symbol.len()..];
        let number: String = rest.chars().take_while(char::is_ascii_digit).collect();
        let line_no: usize = number.parse().unwrap_or_else(|_| panic!("{symbol}…` carries no line number"));
        let line = lines.get(line_no - 1).unwrap_or_else(|| panic!("chrome.rs:{line_no} is past the end of chrome.rs"));
        assert!(line.contains(needle), "docs/design/text-roles.md cites `chrome.rs:{line_no}` for `{needle}` and that line is `{}`", line.trim());
    }
}

// ── #567: the layout model's fixed list is the sheet's ────────────────────────────────────────────

/// **§2.2's fixed list states the divider the editor sets, and names no private gooey constant as this
/// editor's measure.**
///
/// The page said the divider was "gooey's divider target 24" and named `SPLIT_GRIP_THICKNESS`, which is
/// private to `bevy_gooey`. The editor's own number is `metrics.layout.divider_w`, 12 since #476.
#[test]
fn the_layout_models_fixed_list_states_the_sheets_divider() {
    let doc = read("docs/design/layout-model.md");
    let layout = bevy_dark_factory::style::compiled_default().metrics.layout;
    let divider = layout.divider_w;
    assert!(divider > 0.0, "the sheet's `divider_w` is {divider}");

    // **The container tree's divider line**, which names `divider_w` at the sheet's value.
    assert!(
        doc.contains(&format!("`metrics.layout.divider_w` ({divider} px)")),
        "docs/design/layout-model.md's container tree no longer states `metrics.layout.divider_w` at the \
         sheet's {divider} px (#567)"
    );
    // **The fixed list**, whose divider clause names 12 in bold.
    assert!(
        doc.contains(&format!("**`divider_w` {divider}**")),
        "docs/design/layout-model.md's §2.2 fixed list no longer states `divider_w` at the sheet's \
         {divider} (#567)"
    );
    // **No line claims a divider thickness that is not the sheet's.** A snapshot observation may name the
    // target the capture's build drew, but it has to say it is the capture's.
    for line in doc.lines() {
        if line.contains("divider") && line.contains("24") {
            let snapshot = line.contains("this build") || line.contains("that build") || line.contains("capture") || line.contains("MIN_TARGET_PX");
            assert!(
                snapshot,
                "docs/design/layout-model.md states a 24 px divider without saying it is the capture's or \
                 gooey's default: {}",
                line.trim()
            );
        }
    }
    // **`SPLIT_GRIP_THICKNESS` is named once, as gooey's private grip**, never as a `metrics.layout` value.
    if let Some(at) = doc.find("SPLIT_GRIP_THICKNESS") {
        let line = doc[..at].rfind('\n').map_or(0, |n| n + 1);
        let stop = doc[at..].find('\n').map_or(doc.len(), |n| at + n);
        let line = &doc[line..stop];
        assert!(
            line.contains("private") || line.contains("gooey"),
            "docs/design/layout-model.md names `SPLIT_GRIP_THICKNESS` as this editor's measure; it is \
             `bevy_gooey`'s private grip: {}",
            line.trim()
        );
    }
}

// ── #570: keys.md names every fixed chord ─────────────────────────────────────────────────────────

/// **The chord as the page spells it.**
///
/// `chords::primary` is `Modifiers::PRIMARY`, which resolves to the HOST at compile time — `Ctrl` on the gate's Linux
/// runner, `Command` on a mac — and the page is written for the mac. A chord that names `Modifiers::CTRL` itself is
/// left alone: `Ctrl+Tab` is Ctrl on every platform on purpose (`Cmd+Tab` is the system's window switcher), so a
/// blanket `ctrl`→`meta` rewrite would change the one chord the page gets right (this is what it did on 2026-11-23,
/// reporting `Shift+Cmd+Tab` missing for a `session.previous_tab` the code builds as `CTRL | SHIFT`).
fn as_the_page_spells_it(chord: bevy_gooey::Shortcut, built_from_the_primary: bool) -> bevy_gooey::Shortcut {
    use bevy_gooey::Platform;
    // **Whether a chord is the platform's primary is a fact about how it was BUILT, and it cannot be read back off
    // the value**: on the gate's Linux host `Modifiers::PRIMARY` IS `Modifiers::CTRL`, so `session.previous_tab`
    // (declared `CTRL | SHIFT` because it is Ctrl on every platform) equals a `primary_shift` chord. The caller
    // reads the declaration from `chords.rs`'s source instead (see `built_from_the_primary`).
    if built_from_the_primary && Platform::current() != Platform::Mac {
        // A primary chord keeps its other modifiers (Alt on the editor-dev agent) and changes only the primary one.
        let mut for_the_mac = chord;
        for_the_mac.modifiers.ctrl = false;
        for_the_mac.modifiers.meta = true;
        return for_the_mac;
    }
    chord
}

/// **Whether the entry's `default` builds the platform's primary** (it names `primary` or `Modifiers::PRIMARY`), read from
/// `chords.rs`'s own source at `id`, because the value cannot say (see [`as_the_page_spells_it`]).
fn built_from_the_primary(id: &str, source: &str) -> bool {
    let at = source.find(&format!("id: \"{id}\"")).unwrap_or_else(|| panic!("no chord `{id}` in chords.rs"));
    let rest = &source[at..];
    let end = rest.find('}').map(|i| i + 1).unwrap_or(rest.len());
    let entry = &rest[..end];
    // Any `default` that names `primary` — `primary(..)`, `primary_shift(..)`, or `Modifiers::PRIMARY | ALT` — is the
    // host's primary modifier; one that names `Modifiers::CTRL` explicitly is not (the tab chords).
    entry.contains("|| primary") || entry.contains("Modifiers::PRIMARY")
}
/// **Every fixed chord a person can press is on `docs/guide/keys.md`, spelled the way the editor spells
/// it.**
///
/// The page says it lists "the keys the editor answers to" and that Help ▸ Keyboard shortcuts is complete;
/// itself was short by `Cmd+Q`, `Cmd+D`, `Shift+Cmd+I`, `Ctrl+Tab`/`Ctrl+Shift+Tab` and the terminal's
/// close chord. The comparison is against `chords::CHORDS` rendered by gooey's own `Shortcut::label_with`,
/// which is what the editor draws, so a spelling drift fails too (#570).
#[test]
fn the_keys_page_names_every_fixed_chord() {
    use bevy_dark_factory::chords;
    use bevy_gooey::{ChordGlyphs, Platform};

    let keys = read("docs/guide/keys.md");
    let chords_rs = read("crates/bevy_dark_factory/src/chords.rs");
    // **The chords a person cannot press from the page's audience.** The page is the editor's own key list,
    // and there is one chord whose action is not "press this to do a thing": none today. A chord that must
    // be left out is listed here with its reason.
    const NOT_A_PERSON_KEY: &[(&str, &str)] = &[];

    let mut missing = Vec::new();
    for named in chords::CHORDS {
        if NOT_A_PERSON_KEY.iter().any(|(id, _)| *id == named.id) {
            continue;
        }
        let chord = (named.default)();
        // **The chord is spelled for the mac, and its modifiers are the mac's too.** `chords::primary` builds
        // `Modifiers::PRIMARY`, which is `primary_on(Platform::current())` — `ctrl` when the gate compiles on
        // Linux (the 2026-11-23 gate read `Ctrl+Q` for a page that says `Cmd+Q`). So the modifiers are rebuilt
        // for the mac here rather than the host, which is the platform the page is written for.
        let chord = as_the_page_spells_it(chord, built_from_the_primary(named.id, &chords_rs));
        // **The spelling the page uses** is gooey's `Words` on a mac, the one `ui.rs` sets as the host's
        // `ChordGlyphs`.
        let label = chord.label_with(Platform::Mac, ChordGlyphs::Words);
        // **A chord is on the page when every one of its parts is in one backticked span**, so `Cmd+=` and
        // `Shift+Cmd+=` are different rows.
        let wanted = chord_parts(&label);
        let found = keys.split('`').skip(1).step_by(2).any(|span| chord_parts(span) == wanted);
        if !found {
            missing.push(format!("`{label}` ({}, \"{}\")", named.id, named.title));
        }
    }
    assert!(
        missing.is_empty(),
        "**docs/guide/keys.md does not name {} fixed chord(s)**: {}. The page is the guide's every-key page \
         and `chords::CHORDS` is what the editor answers to (#570).",
        missing.len(),
        missing.join(", ")
    );
}

/// **Every census binding with a modifier is on the page too** (#590).
///
/// `chords::CHORDS` is the *fixed* chords — the ones read by their own systems, outside `crate::keys` — and
/// the page's census above covers them. But the keymap's own rows can carry the platform modifier as well, and
/// one did it invisibly: `Action::ToggleRigs` was bound to `Cmd+R` (#522) and `keys.md`'s Anim table never
/// named it, because no census read `keys::BINDINGS` for a chord. A chord a person presses that the census
/// cannot see is a chord that can go missing unnoticed, which is exactly what happened.
///
/// **The other side of the choice in #590.** The issue allowed either moving `ToggleRigs` into
/// `chords::CHORDS` or teaching the census to read the keymap; this is the second, because `ToggleRigs` *is* a
/// census row — it has a context, a stance and a badge home — and moving it would have taken those away to
/// make a test pass.
#[test]
fn the_keys_page_names_every_chord_with_a_modifier() {
    use bevy_dark_factory::keys::BINDINGS;

    let page = read("docs/guide/keys.md");
    let mut spans: Vec<BTreeSet<String>> = page.split('`').skip(1).step_by(2).map(chord_parts).collect();
    // **A range the page writes compactly is every chord in it**: the workspace row says `` `Cmd+1` to
    // `Cmd+9` `` and that names all nine. Expanding it here is what lets the row stay one line instead of
    // becoming nine, which is how a key page stays readable.
    for (a, b) in page.split('`').skip(1).step_by(2).zip(page.split('`').skip(3).step_by(2)) {
        let (Some((a_prefix, a_digit)), Some((b_prefix, b_digit))) = (split_digit_chord(a), split_digit_chord(b)) else {
            continue;
        };
        if a_prefix != b_prefix || a_digit >= b_digit {
            // Not a range: the text between the two spans must be ` to `, which the caller of this loop
            // cannot see, so an out-of-order or different-prefix pair is simply not expanded.
            continue;
        }
        for digit in a_digit..=b_digit {
            let chord = format!("{a_prefix}{digit}");
            spans.push(chord_parts(&chord));
        }
    }

    let mut missing = Vec::new();
    for b in BINDINGS {
        if !b.needs_mod {
            continue;
        }
        // **The page is written for the mac**, so a modifier row is spelled the way that platform spells it,
        // whatever host compiled this test: `Cmd`, `Option`, and only a *required* Shift (the rule
        // `chord_text` states). `Binding::chord` is the bare key, never the modifier.
        let mut label = String::from("Cmd+");
        if b.needs_alt {
            label.push_str("Option+");
        }
        // **The page is written for the mac, and one chord's *name* is platform-dependent too**:
        // `keys::REMOVE_NAME` is `Delete` there and `Del` on the gate's Linux host, the same split
        // `MOD_NAME` has. So the mac spelling is what is compared, on the same rule as the modifiers.
        let spelled: String = if b.chord == "Del" { "Delete".to_owned() } else { b.chord.to_owned() };
        label.push_str(&spelled);
        if b.needs_shift == Some(true) {
            label.insert_str(0, "Shift+");
        }
        let wanted = chord_parts(&label);
        if !spans.contains(&wanted) {
            missing.push(format!("`{label}` ({:?}, \"{}\")", b.action, b.does));
        }
    }
    assert!(
        missing.is_empty(),
        "**docs/guide/keys.md does not name {} keymap chord(s) that carry a modifier**: {}. The page is the \
         guide's every-key page; add the row, and if the chord is not a person's, say why in `NOT_A_PERSON_KEY` \
         for the fixed chords or here (#590).",
        missing.len(),
        missing.join(", ")
    );
}

/// **Every distinct "Works when" value on `docs/guide/keys.md` is a word its own legend defines** (#661).
///
/// The page's legend at the top of the page offers six words for the column, and nothing read the column: a
/// row could invent a state word and every ratchet stayed green. Two had, and both read as near-misses of a
/// word that *is* defined — `Not browsing tiles` for `While browsing tiles`, `No piece in hand` for
/// `Nothing in hand` — which is worse than a typo, because the requirement is the opposite of the word it
/// resembles. The eight `Not browsing tiles` rows are `Stances::AnyBut(Stance::Browsing)` and the one
/// `No piece in hand` row is `AnyBut(Stance::Holding)`; `keys.rs` says outright that what those rows share
/// "is not a stance", so a legend of five stances could never have described them.
///
/// **A value passes one of two ways**, which is the shape the acceptance asks for: it is a word the legend
/// defines, or it is one of those words with an extra condition after a comma. An extra condition that says
/// *which* stance a row excludes — `Any time, but not while browsing tiles` — must name a stance the legend
/// defines, so a reader can look it up; any other must begin `when ` or `with ` and say what it adds.
#[test]
fn the_keys_pages_works_when_words_are_the_legends() {
    let page = read("docs/guide/keys.md");
    let legend = keys_legend_words(&page);
    assert!(
        legend.len() >= 6,
        "only {} word(s) were read from `docs/guide/keys.md`'s legend ({legend:?}): the bullets' shape has changed",
        legend.len()
    );

    let mut undefined = Vec::new();
    for (index, line) in page.lines().enumerate() {
        if !line.starts_with("| `") {
            continue;
        }
        let cells: Vec<&str> = line.split('|').collect();
        if cells.len() < 4 {
            continue;
        }
        let when = cells[2].trim();
        if when.is_empty() || legend.iter().any(|word| word == when) {
            continue;
        }
        let Some((head, tail)) = when.split_once(", ") else {
            undefined.push(format!("line {}: `{when}`", index + 1));
            continue;
        };
        if !legend.iter().any(|word| word == head) {
            undefined.push(format!("line {}: `{when}` (its first word `{head}` is not defined)", index + 1));
            continue;
        }
        // **An exclusion has to name a stance the reader can look up**: the row says *any time but* this one,
        // and `keys.rs`'s own note is that the requirement is not a stance at all but the page still needs a
        // word for what it is not.
        if let Some(excluded) = tail.strip_prefix("but not while ") {
            let as_a_legend_word = format!("While {excluded}");
            if !legend.contains(&as_a_legend_word) {
                undefined.push(format!(
                    "line {}: `{when}` excludes `{excluded}`, and the legend defines no `{as_a_legend_word}`",
                    index + 1
                ));
            }
            continue;
        }
        // **Anything else is a stated extra condition**, and it has to read as one rather than as a new state.
        if !(tail.starts_with("when ") || tail.starts_with("with ")) {
            undefined.push(format!("line {}: `{when}` (the extra condition `{tail}` states no condition)", index + 1));
        }
    }

    assert!(
        undefined.is_empty(),
        "**docs/guide/keys.md uses {} \"Works when\" value(s) its legend does not define**: {}. The column is \
         the page's statement of the state a key needs, and a reader has only the legend to decode it — a word \
         that is not there, or an exclusion that does not name a stance, reads as the word it resembles (#661).",
        undefined.len(),
        undefined.join(", ")
    );
}

/// **The words `docs/guide/keys.md`'s legend defines**, read from the bullets under the sentence that
/// introduces the column. Bounded to that paragraph on purpose: later sections of the page carry bullets of
/// their own (`- **The shell holds its keys.**`) that are not state words.
fn keys_legend_words(page: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut started = false;
    for line in page.lines() {
        if line.starts_with("The \"Works when\" column names the state") {
            started = true;
            continue;
        }
        if !started {
            continue;
        }
        let Some(rest) = line.strip_prefix("- **") else {
            // The bullets end at the first line that is not one, and the paragraph after them is prose.
            if words.is_empty() {
                continue;
            }
            break;
        };
        if let Some(end) = rest.find("**") {
            words.push(rest[..end].to_owned());
        }
    }
    words
}

/// **The commands the push gate runs on the tree**, each with the text the documents have to carry (#657).
///
/// The hook's own steps are its `timed <label> <command>` lines; `clippy-merge` and `rustdoc-merge` are the
/// clean-merge path's versions of the same two checks, so they map onto the same commands. Everything else in
/// the hook is a guard about the caller's state (uncommitted files, conflict markers, a ref that would be
/// refused) rather than a check of the tree, and no document enumerates those.
const GATE_STEPS: &[(&str, &str)] = &[
    ("clippy", "cargo clippy --workspace --all-targets"),
    ("clippy-merge", "cargo clippy --workspace --all-targets"),
    // **The default build still compiles** (#687): the lint, test and doc steps build `family_all`, what a release
    // ships, and this check keeps the build with no family crate compiling.
    ("check-default", "cargo check --workspace --all-targets"),
    ("check-default-merge", "cargo check --workspace --all-targets"),
    ("test", "cargo nextest run --workspace"),
    ("doc", "cargo test --workspace --doc"),
    ("rustdoc", "cargo doc --workspace --no-deps"),
    ("rustdoc-merge", "cargo doc --workspace --no-deps"),
    // **The four pre-checks, timed since #675.** They ran before `clippy` and landed no line at all,
    // so a document that listed the gate could not have named them; the hook now reports each.
    ("backlog", "python3 tools/loop/backlog.py check"),
    ("changelog", "python3 tools/changelog/changelog.py check"),
    ("version", "python3 tools/loop/version.py check"),
    ("wall-clock-filter", "python3 tools/loop/wall_clock_filter.py"),
];

/// **Every document that enumerates the push gate names every step the hook runs** (#657).
///
/// Three pages listed the gate as "clippy, nextest, doctests" and stopped there after `f7e85e7` added
/// `RUSTDOCFLAGS="-D warnings" cargo doc` as its own step (#549). The omission was not cosmetic: `AGENTS.md`
/// uses the list to argue that a hand-run is redundant, and clippy plus a crate's tests together cannot see a
/// doc comment — which is the one class of defect the gate catches and no crate-scoped run does.
///
/// **The ratchet is two-sided.** It reads the hook for its `timed` labels, so a step added to the gate fails
/// here until this table and the four documents name it; and it reads each document for the four commands, so
/// a document that drops one fails.
#[test]
fn the_documents_that_list_the_gate_list_every_timed_step() {
    let hook = read("tools/loop/hooks/pre-push");
    // **The labels the hook times**, which is its own statement of which steps are the gate's.
    let mut labels: BTreeSet<&str> = BTreeSet::new();
    for line in hook.lines() {
        let Some(rest) = line.trim().strip_prefix("timed ") else {
            continue;
        };
        let label = rest.split_whitespace().next().unwrap_or_default();
        if !label.is_empty() {
            labels.insert(label);
        }
    }
    let covered: BTreeSet<&str> = GATE_STEPS.iter().map(|(label, _)| *label).collect();
    let mut unknown: Vec<&str> = labels.difference(&covered).copied().collect();
    unknown.sort_unstable();
    assert!(
        unknown.is_empty(),
        "**`tools/loop/hooks/pre-push` times step(s) this test does not know**: {unknown:?}. Add each to \
         `GATE_STEPS` and name its command in AGENTS.md, `docs/guide/loop.md`, `.omp/commands/build-loop.md` \
         and `.omp/commands/debug-response.md` (#657)."
    );
    // **The command beside each label**, so a renamed step fails here rather than in a reader's head.
    for (label, command) in GATE_STEPS {
        assert!(
            hook.lines().any(|line| {
                let trimmed = line.trim();
                trimmed.strip_prefix("timed ").is_some_and(|rest| rest.starts_with(label)
                    && rest.contains(command))
            }),
            "**`tools/loop/hooks/pre-push` no longer runs `{command}` in its `{label}` step**. This test reads \
             the gate's steps and the four documents that list them, so both sides move together (#657)."
        );
    }

    // **The pages an agent reads before deciding what it may skip by hand.**
    let documents = [
        "AGENTS.md",
        "docs/guide/loop.md",
        ".omp/commands/build-loop.md",
        ".omp/commands/debug-response.md",
    ];
    let mut omissions = Vec::new();
    for document in documents {
        let text = read(document);
        for command in ["cargo clippy --workspace --all-targets", "cargo nextest run --workspace", "cargo test --workspace --doc", "cargo doc --workspace --no-deps"] {
            if !text.contains(command) {
                omissions.push(format!("{document} does not name `{command}`"));
            }
        }
        // **The rustdoc step's whole point is the flags**, so the page that names the command names them.
        if !text.contains("RUSTDOCFLAGS=\"-D warnings\"") {
            omissions.push(format!("{document} does not name `RUSTDOCFLAGS=\"-D warnings\"`"));
        }
        // **The three pre-flight checks**, by the script each one runs: `backlog.py`, `changelog.py` and
        // `version.py`. They read the commit rather than the tree and the hook refuses on each.
        for check in ["backlog", "changelog", "version", "wall-clock-filter"] {
            if !text.to_lowercase().contains(check) {
                omissions.push(format!("{document} does not name the `{check}` check"));
            }
        }
    }
    assert!(
        omissions.is_empty(),
        "**{} gate step(s) are missing from the documents that enumerate the push gate**: {}. The gate is four \
         checks plus three pre-flight scripts, and a page that lists three of the four tells an agent a \
         crate-scoped run covers everything (#657).",
        omissions.len(),
        omissions.join(", ")
    );
}

/// **A chord split into its prefix and its last digit**, when it is `…+<digit>`: the shape a page writes a
/// range in (`` `Cmd+1` to `Cmd+9` ``). `None` for any chord that is not one.
fn split_digit_chord(chord: &str) -> Option<(String, char)> {
    let last = chord.chars().next_back()?;
    if !last.is_ascii_digit() || chord.len() < 2 {
        return None;
    }
    let prefix = chord[..chord.len() - 1].to_owned();
    // **The prefix must end in a modifier separator**: `Cmd+` is, a bare `1` is not, so a lone digit is not
    // read as a one-member range.
    prefix.ends_with('+').then_some((prefix, last))
}

/// **A chord's parts, normalised to one spelling**, so the page and gooey can word the same chord differently
/// and still be compared.
///
/// **`Opt` is `Option`**: `gooey`'s `ChordGlyphs::Words` spells the alt key `Opt` on a mac and the page spells
/// it `Option`, the name the key carries. **`Backquote` is the backtick**: a Markdown span cannot hold a lone
/// backtick, so the page names the key, and gooey draws the glyph.
fn chord_parts(chord: &str) -> BTreeSet<String> {
    chord
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            match part {
                "Opt" => "Option",
                "Backquote" => "`",
                other => other,
            }
            .to_owned()
        })
        .collect()
}

// ── #656: every repository path a loop command names exists ──────────────────────────────────────

/// **A repository path a loop command names that the tree does not hold, and why it is allowed.**
///
/// The rule is #656's: a command that tells an agent to read a report directory under `tools/loop/` which
/// nothing creates sends it to `ls: No such file or directory`. The two ways a path is allowed to be absent
/// are both here, with the reason, rather than as a silent exception in the code.
///
/// **The dead path is spelled in pieces below**, on the same rule the dead-name guard states: this file lives
/// under `crates/`, so a grep for the path would find this test rather than only a document that points at it.
const PATHS_THAT_ARE_NOT_HERE: &[(&str, &str)] = &[
    // `.omp/commands/build-loop.md`'s rename record: the paragraph says in its own words that these three
    // crates "exist only as history", so naming them is the point of it.
    ("crates/bevy_blueprint_mapper", "the rename record, which says the crate is history"),
    ("crates/bevy_blueprint_mapper/tests/all.rs", "the rename record, which says the crate is history"),
    ("crates/bevy_blueprint_mcp", "the rename record, which says the crate is history"),
    ("crates/bevy_blueprint_decide", "the rename record, which says the crate is history"),
    // A consumer's command file, in Sanctuary's clone, not this repository's.
    (".omp/commands/emerge-loop.md", "Sanctuary's command file, named as a callsite in that game's clone"),
];

/// **Every repository path ` .omp/commands/*.md` names in backticks exists** (#656).
///
/// `review.md` sent the review agent to a report directory under `tools/loop/` for "the fleet's own reports"
/// and, 294 lines later, told it to write its own report to `docs/loop/iterations/`. That `tools/` path has
/// never existed — it was named once in the whole repository, on that line — and the reports are in
/// `docs/loop/iterations/`, which is where `docs/guide/loop.md` also says they go. An agent following the
/// first line found nothing and either skipped the duplicate-suppression step or filed the missing directory
/// as a finding.
///
/// **A path is a backticked span that begins with one of the repository's own top-level directories** and is a
/// literal: a span with a glob, a placeholder, a space (a script plus its arguments) or a `:line` suffix is not
/// a path a reader would `ls`. The test is the same shape as [`every_guide_markdown_fragment_resolves`], which
/// holds the guide's links to the files they name.
#[test]
fn every_path_a_loop_command_names_exists() {
    let root = root();
    // **The repository's own top-level directories**, read from the tree so a new one is covered without an
    // edit here. `src/` and `tests/` are deliberately absent: a command names those relative to a crate.
    let mut tops: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display())) {
        let path = entry.expect("a directory entry").path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        // A dotted directory is the tree's (`docs/`, `.github/`) but `.git/` is not a path a document sends
        // anyone to, and `..` would read as a top-level directory.
        if path.is_dir() && name != ".git" {
            tops.push(format!("{name}/"));
        }
    }
    assert!(tops.len() >= 8, "only {tops:?} were read as the repository's top-level directories");

    let allow: BTreeSet<&str> = PATHS_THAT_ARE_NOT_HERE.iter().map(|(path, _)| *path).collect();
    let commands = root.join(".omp/commands");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&commands)
        .unwrap_or_else(|e| panic!("{}: {e}", commands.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|path| path.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no command files were read under {}", commands.display());

    let mut missing = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let shown = file.strip_prefix(&root).unwrap_or(file).display().to_string();
        let mut fenced = false;
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            // **The backticked spans**, in pairs, which is the same walk the chord ratchets do.
            for span in line.split('`').skip(1).step_by(2) {
                let span = span.trim();
                if !tops.iter().any(|top| span.starts_with(top.as_str())) {
                    continue;
                }
                // A placeholder, a glob or a shell line is not a path a reader would `ls`.
                if span.contains(['*', '<', '>', '{', '}', ' ', '$', '|', '#']) {
                    continue;
                }
                // **A `path:line` citation names the file**, and the line is checked by its own reader.
                let path = span.split_once(':').map(|(p, rest)| if rest.chars().all(|c| c.is_ascii_digit()) { p } else { span }).unwrap_or(span);
                if allow.contains(path) {
                    continue;
                }
                let on_disk = root.join(path);
                // **A trailing slash is a directory**, which `exists()` answers for either kind; a path with
                // one that names a file is the same defect the other way round, so the kind is compared too.
                if path.ends_with('/') && !on_disk.is_dir() {
                    missing.push(format!("{shown}:{}: `{path}` is not a directory", n + 1));
                } else if !path.ends_with('/') && !on_disk.exists() {
                    missing.push(format!("{shown}:{}: `{path}` does not exist", n + 1));
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "**{} repository path(s) a loop command names do not exist**:\n{}\nA command that sends an agent to a \
         path the tree does not hold is either skipped or filed as a finding of its own (#656). Fix the path, \
         or add it to `PATHS_THAT_ARE_NOT_HERE` with the reason it is absent.",
        missing.len(),
        missing.join("\n")
    );
}

// ── #568 and #570: every intra-repo markdown fragment resolves ───────────────────────────────────

/// **Every intra-repo link in `docs/guide/*.md` and `AGENTS.md` resolves its file and its fragment.**
///
/// Three links landed nowhere: two `agents.md#start-an-agent-in-dark_factorys-terminal` (GitHub's slugger
/// strips the apostrophe, so the anchor is `…dark-factorys-terminal`) and `loop.md`'s link to a
/// `RUNNING-A-LOOP.md` heading that the owner's 2026-11-16 ruling replaced. The dead anchor is why the
/// release rule above it kept saying the opposite of the ruling (#568, #570).
///
/// **`docs/loop/iterations/` and `docs/research/` are not walked**: they are dated snapshots of what a pass
/// found, and a link that was valid when the pass wrote it is a fact about that pass.
#[test]
fn every_guide_markdown_fragment_resolves() {
    let mut files: Vec<PathBuf> = Vec::new();
    let guide = root().join("docs/guide");
    for entry in std::fs::read_dir(&guide).unwrap_or_else(|e| panic!("{}: {e}", guide.display())) {
        let path = entry.expect("a directory entry").path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            files.push(path);
        }
    }
    files.push(root().join("AGENTS.md"));

    let mut broken = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
        let shown = file.strip_prefix(root()).unwrap_or(file).display().to_string();
        let mut fenced = false;
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
                continue;
            }
            if fenced {
                continue;
            }
            for link in links(line) {
                if link.starts_with("http://") || link.starts_with("https://") || link.starts_with("mailto:") {
                    continue;
                }
                let (path, fragment) = match link.split_once('#') {
                    Some((p, f)) => (p, Some(f)),
                    None => (link.as_str(), None),
                };
                let target = if path.is_empty() { file.clone() } else { file.parent().unwrap_or(Path::new(".")).join(path) };
                let target = target.canonicalize().unwrap_or(target.clone());
                if !target.exists() {
                    broken.push(format!("{shown}:{}: `{link}` names a file that is not there", n + 1));
                    continue;
                }
                let Some(fragment) = fragment else { continue };
                if target.extension().and_then(|e| e.to_str()) != Some("md") {
                    continue;
                }
                let Ok(target_text) = std::fs::read_to_string(&target) else { continue };
                let headings: BTreeSet<String> = headings(&target_text);
                if !headings.contains(fragment) {
                    broken.push(format!("{shown}:{}: `{link}` has no heading with that anchor", n + 1));
                }
            }
        }
    }
    assert!(
        broken.is_empty(),
        "{} markdown link(s) in docs/guide and AGENTS.md land nowhere:\n{}",
        broken.len(),
        broken.join("\n")
    );
}

/// **The `](…)` targets on one line**, ignoring an image's `![…]` and anything inside a code span.
fn links(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let bytes: Vec<char> = line.chars().collect();
    let mut at = 0;
    while at + 1 < bytes.len() {
        if bytes[at] == ']' && bytes[at + 1] == '(' {
            let start = at + 2;
            let mut end = start;
            while end < bytes.len() && bytes[end] != ')' {
                end += 1;
            }
            let target: String = bytes[start..end].iter().collect();
            // **A link with a title** (`(path "the title")`) keeps the path.
            let target = target.split_whitespace().next().unwrap_or_default().to_owned();
            // **An image is not a link to a page**, and its target is checked by the same rule anyway, so it
            // is kept: a picture that moved is a broken page.
            if !target.is_empty() && !target.starts_with('<') {
                out.push(target);
            }
            at = end;
            continue;
        }
        at += 1;
    }
    out
}

/// **Every heading's GitHub anchor** in a markdown document: lowercased, punctuation dropped, spaces to
/// hyphens. The rule GitHub's slugger follows, which is why `Dark Factory's` becomes `dark-factorys`.
fn headings(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut fenced = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let trimmed = line.trim_start();
        let hashes = trimmed.chars().take_while(|c| *c == '#').count();
        if hashes == 0 || hashes > 6 || !trimmed[hashes..].starts_with(' ') {
            continue;
        }
        let title = trimmed[hashes..].trim();
        out.insert(slug(title));
    }
    out
}

/// **A heading's anchor**, the way GitHub's slugger makes it.
fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            out.extend(c.to_lowercase());
        } else if c == ' ' {
            out.push('-');
        }
        // **Every other mark is dropped**, including the apostrophe that made `factory's` `factorys`.
    }
    out
}

// ── #571: AGENTS.md's family table is the Cargo.toml's ────────────────────────────────────────────

/// **The crate each family row names is a `[features]` key, a dependency, or a path member of the editor —
/// and the rule line under the table says which rows are which.**
///
/// The table called every row "an optional cargo feature"; `bevy_gooey` is always built, `bevy_devshot` is a
/// path member, `bevy_fairy` rides the `debugger` feature as `bevy_debugger_bevy`, and the row for `isomesh`
/// named `bevy_isomesh`, a different crate the editor never depends on (#571).
///
/// **The row's crate is its first backticked name**: the table's second column is `| need | crate |`, and the
/// words after the crate are prose (`(always built)`, `bevy_fairy`, imported as …) which may name things this
/// check has no business judging.
#[test]
fn the_agents_family_table_names_the_crates_the_editor_has() {
    let agents = read("AGENTS.md");
    let manifest = read("crates/bevy_dark_factory/Cargo.toml");

    // **The family table**: the rows between its header and the blank line that ends it.
    let start = agents.find("| Need | Crate |").expect("AGENTS.md has the family table");
    let body = &agents[start..];
    let end = body.find("\n\n").expect("the table ends");
    let features = feature_keys(&manifest);
    let dependencies: BTreeSet<String> = manifest
        .lines()
        .filter(|l| l.starts_with(|c: char| c.is_ascii_alphabetic()) && l.contains('='))
        .filter_map(|l| l.split('=').next().map(|n| n.trim().replace('-', "_")))
        .collect();

    let mut wrong = Vec::new();
    let mut rows = 0;
    for line in body[..end].lines() {
        let line = line.trim();
        if !line.starts_with('|') || line.starts_with("| Need") || line.starts_with("|--") {
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').collect();
        if cells.len() != 2 {
            continue;
        }
        rows += 1;
        // **The row's crate**: the first backticked name of the crate column.
        let crate_name = cells[1]
            .split('`')
            .skip(1)
            .step_by(2)
            .next()
            .unwrap_or_else(|| panic!("the family row `{}` names no crate", cells[0].trim()));
        let key = crate_name.replace('-', "_");
        if features.contains(&key) || dependencies.contains(&key) {
            continue;
        }
        // **`bevy_fairy` is the package, imported as `bevy_debugger_bevy`**, which is the dependency the
        // feature turns on; the row says so in its own cell.
        if crate_name == "bevy_fairy" && dependencies.contains("bevy_debugger_bevy") {
            continue;
        }
        wrong.push(format!("`{crate_name}` for {}", cells[0].trim()));
    }
    assert!(rows >= 10, "only {rows} family rows were read: the table has changed shape");
    assert!(
        wrong.is_empty(),
        "**AGENTS.md's family table names crates `crates/bevy_dark_factory/Cargo.toml` has no feature key or \
         dependency for**: {}. A pass reads this table to decide what it may use (#571).",
        wrong.join(", ")
    );

    // **`bevy_isomesh` is not the crate this editor depends on**, and the row says so in words rather than
    // naming it as the crate. The crate the row names is `isomesh`.
    assert!(
        agents.contains("| SDF to mesh, caves, overhangs, carving | `isomesh`"),
        "AGENTS.md's SDF row no longer names `isomesh`, the crate `crates/bevy_dark_factory/Cargo.toml` \
         depends on. `bevy_isomesh` is a sibling crate in the same repository and is not one this editor \
         uses."
    );

    // **The rule line says which rows are which**, rather than "each crate is an optional cargo feature". Each
    // claim is read from the table's own cells or the rule beneath it, and the row it is about must carry it.
    for (claim, row) in [("always built", "bevy_gooey` (always built"), ("path", "bevy_devshot` (a workspace member by path"), ("debugger", "behind the `debugger` feature")] {
        assert!(
            agents.contains(row),
            "AGENTS.md no longer says `{claim}` of the row it applies to, which is what makes that row true \
             (#571): expected `{row}`"
        );
    }
}

/// **The `[features]` keys of a manifest**, as a set.
fn feature_keys(manifest: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut inside = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            inside = trimmed == "[features]";
            continue;
        }
        if !inside || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((name, _)) = trimmed.split_once('=') {
            out.insert(name.trim().replace('-', "_"));
        }
    }
    out
}
