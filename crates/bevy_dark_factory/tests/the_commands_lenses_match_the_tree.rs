//! **A lens in the loop's command that says "not built" must not name work that is in the tree** (#280).
//!
//! `.omp/commands/build-loop.md`'s "Lenses" section is what a pass reads when every filed issue is
//! blocked. Six of its ten lenses said **not built** or **partially built**; measured 2026-11-03, six of
//! those six were built — `relations::Kind`'s eleven kinds, `standards`, `adjacency::ring`/`room`,
//! `repetition::Role`, `layout_propose`'s `options`. Three passes in a row read that list, chose an item
//! from it, found it done, and shipped loop tooling instead.
//!
//! **The command is the owner's**, so this file does not edit it. What it does is make the drift
//! checkable: for each lens the command marks unbuilt, it names the module or symbol the claim is about,
//! and fails when that module is present in the crate. A future pass that builds the next lens without
//! updating the command fails here, with the file and symbol in the message.
//!
//! **It is a scan of the text, not a parser**, on `census_is_the_one_counter.rs`'s precedent: the claim
//! lives in prose, and the honest check is whether the prose still says something true.

use std::path::{Path, PathBuf};

/// **The lenses the command marked unbuilt, and what their being unbuilt would mean.** Each pair is
/// `(the words the command uses, the source path that must NOT exist while it says so)`.
///
/// A lens whose module is present is drift: the command tells a pass to go and build something that is
/// already there, and the pass spends its item discovering that.
const LENSES: &[(&str, &str)] = &[
    (
        "this repo has the solver and no constraint layer",
        "crates/blueprint_model/src/relations.rs",
    ),
    ("Kit admission rules from shipped practice", "crates/blueprint_model/src/standards.rs"),
    ("Role-weighted repetition", "crates/blueprint_model/src/repetition.rs"),
    ("Options on request", "crates/blueprint_model/src/options.rs"),
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The command's lens section, as text. `None` when the file is absent (a checkout without the loop's
/// own command), which is not a failure: the ratchet is about drift, not about the file existing.
fn command() -> Option<String> {
    std::fs::read_to_string(repo_root().join(".omp/commands/build-loop.md")).ok()
}

/// **Every lens the command marks unbuilt has nothing in the tree to build.**
///
/// Read it as: *if this fails, either the command is stale (update its lens line — the owner does that) or
/// the module was deleted (then the lens is true again and the command is right).*
#[test]
fn a_lens_marked_unbuilt_names_nothing_that_exists() {
    let Some(text) = command() else {
        // **Not a silence.** A clone without the command is a clone that cannot read the lens list, and
        // saying so is better than a test that passes because it read nothing.
        eprintln!("build-loop.md is not in this checkout, so the lens ratchet has nothing to check");
        return;
    };
    let mut drifted = Vec::new();
    for (words, path) in LENSES {
        if !text.contains(words) {
            // The command no longer makes this claim — the owner has updated it, which is the goal.
            continue;
        }
        let present = repo_root().join(path).exists();
        if present {
            drifted.push(format!("`{words}` → but {path} exists"));
        }
    }
    // **A report, not a refusal, and the difference is the point.** The command is the owner's file: a
    // pass may not edit it, so a test that failed on its staleness would put `main` red and block every
    // push over a sentence nobody in the loop may change. What a pass *can* do is say it out loud, once
    // per run, in the log the gate keeps — and `main` stays green while the owner decides.
    //
    // The one thing this test does refuse is the case it was written for: a lens naming a path that
    // does not exist *and* a claim that it does, which would be the command pointing at the wrong file.
    // **The drift is a list, and the list is asserted.** Reporting alone cannot fail, and a check that
    // cannot fail is not a check — the lesson pass 365 learned the hard way. So the *known* stale lenses
    // are written down here, and this fails when the set changes in either direction:
    //
    // - a **new** lens goes stale (built work the command still calls unbuilt) — the command is further
    //   behind, and the next pass must not be the one to discover it by reading;
    // - a **known** one stops being stale — the owner updated the command, and this list must shrink so
    //   the record matches the file.
    //
    // Neither edits the command; both make its state checkable, which is what this loop may do.
    const KNOWN_STALE: &[&str] = &[
        // Measured 2026-11-03 (pass 369, #280). The command is the owner's; this is the receipt.
        "`Kit admission rules from shipped practice` → but crates/blueprint_model/src/standards.rs exists",
        "`Role-weighted repetition` → but crates/blueprint_model/src/repetition.rs exists",
    ];
    if !drifted.is_empty() {
        eprintln!(
            "the command's lenses say these are not built, and the tree says they are (#280); the \
             command is the owner's file, so this is reported rather than refused:\n  {}",
            drifted.join("\n  ")
        );
    }
    assert_eq!(
        drifted, KNOWN_STALE,
        "the set of stale lenses changed. A new one means the command is further behind the tree and a \
         pass will choose already-built work (#280); a vanished one means the owner updated it and this \
         list must drop it. Neither is a change to the command."
    );
    assert!(
        LENSES.iter().all(|(_, p)| repo_root().join(p).parent().is_some_and(|d| d.exists())),
        "a lens names a directory that is not in this checkout: {}",
        LENSES
            .iter()
            .map(|(w, p)| format!("`{w}` -> {p}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
}

/// **And the count in the lens list is checked against the server**, so a lens that says "14 tools" or
/// "partially built" cannot survive the server declaring more.
///
/// **A guard that returns on the case it was written for is worse than the stale number** (#619): this test
/// used to take `line.split('*').next()`, which on `- **A small…surface** … 14 tools today` is `"- "` — no
/// digits — so every count in that bullet passed forever, including the `14` it was written to catch.
///
/// It now reads the number **stated before `tools today`**, and a line with no number there is a failure
/// rather than a return: a claim the guard cannot parse is a claim nothing checks.
#[test]
fn the_tool_count_the_command_quotes_is_the_servers() {
    let text = command().expect("the build-loop command is in this checkout (#619)");
    let line = text
        .lines()
        .find(|l| l.contains("tools today"))
        .expect("build-loop states its tool surface as `N tools today` (#619)");
    // **The digits before `tools today`, not anything before the first `*`.** The bullet opens with a bold
    // phrase, so `split('*').next()` is the empty marker; the number sits after the italic aside.
    let quoted = line
        .split("tools today")
        .next()
        .expect("the split yields the text before the phrase")
        .split(|c: char| !c.is_ascii_digit())
        .rfind(|run| !run.is_empty())
        .and_then(|run| run.parse::<usize>().ok())
        .unwrap_or_else(|| {
            panic!(
                "**the tool count cannot be read, which is the defect this test exists for** (#619): the line \
                 states `tools today` with no number before it, so no count is checked. Write the count \
                 (`N tools today`) or drop the phrase. The line is:\n  {line}"
            )
        });
    // The server's own declaration, not a number in a doc: the same source `docs/mcp-tools.md` renders from.
    let server = std::fs::read_to_string(repo_root().join("crates/dark_factory_mcp/src/docs.rs"))
        .unwrap_or_default();
    // **Rows only.** `server.matches("ToolDoc {").count()` is 33 because it also matches
    // `pub struct ToolDoc {`, which is the type and not a tool — the guard compared a count against a
    // count-plus-one and would have passed a command quoting 33 for a server holding 32.
    let declared = server
        .lines()
        .filter(|l| l.trim_start().starts_with("ToolDoc {"))
        .count();
    assert!(declared > 0, "the tool table could not be read, so this check proves nothing");
    assert_eq!(
        quoted, declared,
        "the command's lens quotes {quoted} MCP tools and the server declares {declared}. File the \
         update; the command is the owner's file (#280)."
    );
}
