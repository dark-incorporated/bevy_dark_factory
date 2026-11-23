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
    // **The default-build number is this build's own**, measured by calling the registry rather than
    // derived from the source, so the clause means what it says: what a default build has.
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
            let close = text[call..].find(')').expect("the call closes") + call;
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

/// **The names a version-section bullet spells** after its colon, as `"id"`s: `` `forest`, `people` `` reads
/// as two. The clause after the last name (`; slice 3`) and a `from slice 3` note are not names.
fn spelled_names(guide: &str, lead: &str) -> Vec<String> {
    let at = guide.find(lead).unwrap_or_else(|| panic!("no bullet `{lead}`")) + lead.len();
    let open = guide[..at].rfind('(').expect("the bullet opens a parenthesis");
    let close = guide[open..].find(')').expect("the bullet closes") + open;
    let body = &guide[at..close];
    let Some((_, names)) = body.split_once(':') else { return Vec::new() };
    names
        .split(',')
        .map(|part| part.trim())
        .filter(|part| part.starts_with('`'))
        .filter_map(|part| part.split('`').nth(1))
        .map(str::to_owned)
        .collect()
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
/// `chrome::section` was cited at `chrome.rs:991` and sits at `:827`; the role module was cited at `:690` and
/// sits at `:531`. A citation that points into the middle of something else is worse than none (#566).
#[test]
fn the_text_roles_citations_land_on_their_symbols() {
    let doc = read("docs/design/text-roles.md");
    let chrome = read("crates/bevy_dark_factory/src/chrome.rs");
    let lines: Vec<&str> = chrome.lines().collect();
    for (citation, needle) in [("chrome.rs:827", "pub fn section"), ("chrome.rs:531", "pub mod text")] {
        assert!(doc.contains(citation), "docs/design/text-roles.md no longer cites `{citation}`");
        let at: usize = citation.rsplit(':').next().expect("a line number").parse().expect("a number");
        let line = lines.get(at - 1).unwrap_or_else(|| panic!("{citation} is past the end of chrome.rs"));
        assert!(line.contains(needle), "docs/design/text-roles.md cites `{citation}` for `{needle}` and that line is `{}`", line.trim());
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
        // **The spelling the page uses** is gooey's `Words` on a mac and the platform's own elsewhere, the
        // one `ui.rs` sets as the host's `ChordGlyphs`. Read as the page writes it, which is the mac's.
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
