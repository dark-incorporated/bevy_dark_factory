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
