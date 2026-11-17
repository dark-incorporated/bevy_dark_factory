//! **An arm's guide is one argument, and the arm that names a section the guide has not is refused** (#457,
//! DM-06).
//!
//! The issue's decision is that the without-the-section arm is **not a second copy of the guide**: the two
//! copies drift, and an arm measured on a copy is not the arm a person is handed. So `variants.yaml` gained
//! `omit_guide_sections`, the run passes it to the MCP server as `--omit-guide-sections`, and the omission
//! itself happens in the server (`crates/bevy_cauldron_mcp/tests/the_guide_has_one_arm_switch.rs` proves the
//! served text).
//!
//! **What is proved here, with no editor and no model:**
//!
//! - the suite's two ask arms read out of `bench/tasks/variants.yaml`, with exactly one line between them;
//! - a section name the shipped guide has not is **refused before any trial runs**, naming the headings that
//!   would have been accepted — never a run of 60 harness errors whose reason reads as the editor's fault;
//! - the section the arms differ by is one the shipped guide actually has, so the arms are two arms of this
//!   guide and not of one that was renamed under them.

use std::path::{Path, PathBuf};

use bevy_cauldron_bench::run::{Agent, Options};
use bevy_cauldron_bench::Suite;

/// **The repository this test runs in**, which is where `guide.md` and `bench/tasks` are.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("the repository")
}

/// **The section the two arms differ by**, as `guide.md` spells its heading.
const SECTION: &str = "## Check your work by asking";

/// **A scratch directory this test owns**, named with the test and the process id so a rerun and a parallel test
/// never share one.
fn scratch(test: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "grimoire-fixture-{}-arm-{test}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory is made");
    dir
}

/// **`Options` pointing at this repository, with the binaries this test can name**.
fn options(repo: &Path, variant: Option<&str>) -> Options {
    Options {
        root: repo.to_path_buf(),
        tasks: repo.join("bench/tasks"),
        agent: Agent::Nothing,
        trials: 1,
        jobs: 1,
        model: String::new(),
        variant: variant.map(str::to_owned),
        out: std::env::temp_dir(),
        editor: PathBuf::new(),
        server: PathBuf::new(),
        repo: repo.to_path_buf(),
        label: "arm".to_owned(),
        publish: None,
        only: Vec::new(),
    }
}

#[test]
fn the_suites_two_ask_arms_differ_by_one_section_and_nothing_else() {
    let variants = Suite::variants(&repo()).expect("the suite's arms read");
    let with = variants.iter().find(|v| v.name == "ask-with-guide").unwrap_or_else(|| {
        panic!("no `ask-with-guide` arm; the arms are: {:?}", variants.iter().map(|v| &v.name).collect::<Vec<_>>())
    });
    let without = variants.iter().find(|v| v.name == "ask-without-guide").unwrap_or_else(|| {
        panic!("no `ask-without-guide` arm; the arms are: {:?}", variants.iter().map(|v| &v.name).collect::<Vec<_>>())
    });

    assert!(with.omit_guide_sections.is_empty(), "the with arm omits a section: {with:?}");
    assert_eq!(without.omit_guide_sections, vec![SECTION.trim_start_matches("## ").to_owned()],
        "the without arm does not omit exactly the look-then-ask section: {without:?}");

    // **Nothing else differs**, which is what makes the two runs two arms of one question: the tool list and the
    // files laid over the trial are the same in both, so the only line that does differ is the guide's.
    assert_eq!(
        with.hide_tools, without.hide_tools,
        "the arms hide different tools, so a difference between them would not be the guide's"
    );
    assert_eq!(with.overlay, without.overlay, "the arms lay different files over the trial");
    assert_ne!(with.name, without.name, "the arms are two named arms");
    // **And the difference is one field**: everything but the name and the omission is equal.
    assert_eq!(
        (&with.name, &with.hide_tools, &with.overlay, &with.omit_guide_sections),
        (&with.name, &without.hide_tools, &without.overlay, &Vec::<String>::new()),
        "the with arm is not the without arm with its section kept"
    );
}

#[test]
fn the_section_the_arms_differ_by_is_one_the_shipped_guide_has() {
    // **A heading the guide renames must fail here**, not in a run: both arms would then be the same guide and
    // the benchmark would report a difference of nothing as a measurement.
    let headings = bevy_cauldron_bench::probe::guide_headings(&repo()).expect("the shipped guide is readable");
    assert!(
        headings.iter().any(|h| h == SECTION),
        "the shipped guide has no `{SECTION}`: the arms name a section it does not have. Its sections are: {headings:?}"
    );
    // **And the server has not been asked to print a list this test cannot read**: the headings are the guide's
    // own `##` lines, which is what `--omit-guide-sections` accepts.
    for heading in &headings {
        assert!(heading.starts_with("## "), "a heading is not a `##` line: {heading}");
    }
}

#[test]
fn an_arm_that_names_a_section_the_guide_has_not_is_refused_before_any_trial() {
    // **The refusal is the runner's, at its own door.** The editor and the server it would refuse are absent,
    // and this test depends on that: the arm's section is checked *before* the binaries are, so a run that would
    // have failed 60 trials for a heading says so in one sentence and never starts a display.
    let dir = scratch("bad-section");
    let root = dir.join("project");
    std::fs::create_dir_all(root.join("bench/tasks")).expect("the tasks directory is made");
    // **A copy of the arms with one heading the guide has not**, and the guide beside it, so the check has the
    // real `guide.md` to read and a real arm to refuse.
    let said = "version: 1\nvariants:\n  - name: ask-renamed\n    omit_guide_sections:\n      - Check your work by asking please\n";
    std::fs::write(root.join("bench/tasks/variants.yaml"), said).expect("the arms file is written");
    let guide = repo().join(bevy_cauldron_bench::probe::GUIDE_PATH);
    let guide_at = root.join(bevy_cauldron_bench::probe::GUIDE_PATH);
    std::fs::create_dir_all(guide_at.parent().expect("the guide's directory")).expect("the guide's directory is made");
    std::fs::copy(&guide, &guide_at).expect("the shipped guide is copied");

    let err = bevy_cauldron_bench::run::run(&options(&root, Some("ask-renamed")), None)
        .expect_err("an arm naming a section the guide has not is refused");
    assert!(
        err.contains("Check your work by asking please"),
        "the refusal does not name the heading it refused: {err}"
    );
    assert!(err.contains(SECTION), "the refusal does not name a heading the guide does have: {err}");
    let _ = std::fs::remove_dir_all(&dir);
}
