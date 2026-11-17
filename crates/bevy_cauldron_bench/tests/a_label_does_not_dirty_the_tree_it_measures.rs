//! **A matrix with `--label` publishes only after every pair has run, so the label it writes cannot change what
//! a later pair reads** (#416).
//!
//! A matrix's rows are comparable only on one editor commit, and the commit is read at each pair's start. A
//! labelled pair used to publish its report into `docs/bench/` while the matrix was still running, which made the
//! working tree dirty: the second pair then read `<sha>+dirty` and the matrix refused a comparison the harness
//! itself had made unequal. The #416 run worked around it by dropping `--label`. These tests hold the fix in place,
//! and hold the refusal that is still wanted: a matrix whose pairs really did run at different commits is refused.
//!
//! **No test here runs a model or an editor.** [`bevy_cauldron_bench::matrix::run_with`] takes one pair's work as an
//! argument, so these tests hand it a stand-in over recorded trials; everything else — the order of the run, the
//! commit read, the refusal, the publication — is the matrix's own code.

use std::path::{Path, PathBuf};

use std::cell::RefCell;

use bevy_cauldron_bench::matrix::{self, Pair};
use bevy_cauldron_bench::report::{Header, REPORT_VERSION, RUN_FILE, Report, RunData};
use bevy_cauldron_bench::run::{Agent, Options};

// -------------------------------------------------------------------------------------------------
// A repository whose commit can be read, and made dirty by a tracked file.
// -------------------------------------------------------------------------------------------------

/// **A fresh scratch directory for one test.**
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bench-label-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **Run `git` in `repo`, answering its standard output.** A git that cannot be run is a broken host, not
/// something a test reads past.
fn git(repo: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output();
    let out = out.unwrap_or_else(|e| panic!("git {args:?} in {}: {e}", repo.display()));
    assert!(
        out.status.success(),
        "git {args:?} in {} failed ({}): {}",
        repo.display(),
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// **A repository at one commit**, with `docs/bench/README.md` tracked so that a publication into it shows up in
/// `git status`: a published report that is *new* is untracked, which [`bevy_cauldron_bench::probe::commit_of`]
/// ignores on purpose, and the README is what a second publication rewrites.
fn repo(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    git(dir, &["init", "--quiet"]);
    git(dir, &["config", "user.email", "bench@example.invalid"]);
    git(dir, &["config", "user.name", "bench"]);
    let bench = dir.join("docs/bench");
    std::fs::create_dir_all(&bench).unwrap_or_else(|e| panic!("{}: {e}", bench.display()));
    std::fs::write(bench.join("README.md"), "# The benchmark's runs\n")
        .unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(dir.join("tracked.txt"), "one\n").unwrap_or_else(|e| panic!("{e}"));
    git(dir, &["add", "-A"]);
    git(
        dir,
        &["commit", "--quiet", "-m", "the tree a pair measures"],
    );
}

/// **One pair's run, recorded**: four dev tasks, two trials each, all passed, in `dir`, answering its header.
fn record(dir: &Path, options: &Options) -> Header {
    let header = Header {
        version: REPORT_VERSION,
        label: options.label.clone(),
        commit: bevy_cauldron_bench::probe::commit_of(&options.repo),
        guide_sha256: "cd".repeat(32),
        agent: options.agent.name().to_owned(),
        agent_version: "1.0".to_owned(),
        model: options.agent.has_model().then(|| options.model.clone()),
        variant: None,
        suite: "bench/tasks".to_owned(),
        trials: options.trials,
        host: "big".to_owned(),
        date: "2026-11-13".to_owned(),
        price: None,
    };
    std::fs::create_dir_all(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    std::fs::write(
        dir.join(RUN_FILE),
        serde_json::to_string(&header).unwrap_or_else(|e| panic!("{e}")),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    for task in 0..4usize {
        for trial in 1..=2u32 {
            let d = dir
                .join(format!("task-{task}"))
                .join(format!("trial-{trial}"));
            std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("{}: {e}", d.display()));
            let trial = serde_json::json!({
                "task": format!("task-{task}"), "trial": trial, "title": "t", "split": "dev", "tags": ["place"],
                "outcome": "pass", "checks": [], "tool_calls": ["grimoire_state"], "tool_log": [],
                "refusals": 0, "refusals_by_tool": {}, "tokens_in": 1_000u64, "tokens_out": 0u64, "wall_s": 12.0,
            });
            std::fs::write(d.join("trial.json"), trial.to_string())
                .unwrap_or_else(|e| panic!("{e}"));
        }
    }
    header
}

/// **The matrix's own options**, as `cauldron-bench matrix --label` makes them from one run's.
fn matrix_options(tree: &Path, out: &Path) -> Options {
    Options {
        root: tree.to_owned(),
        tasks: tree.join("bench/tasks"),
        agent: Agent::Omp,
        trials: 2,
        jobs: 1,
        model: String::new(),
        variant: None,
        out: out.to_owned(),
        editor: tree.join("editor"),
        server: tree.join("server"),
        repo: tree.to_owned(),
        label: "matrix".to_owned(),
        publish: Some(tree.join("docs/bench")),
        only: Vec::new(),
    }
}

// -------------------------------------------------------------------------------------------------
// The fix (#416): the label publishes after every pair has run.
// -------------------------------------------------------------------------------------------------

#[test]
fn a_labelled_matrix_succeeds_and_the_second_pair_reads_the_commit_the_first_did() {
    let dir = scratch("labelled");
    let tree = dir.join("repo");
    repo(&tree);
    let head = bevy_cauldron_bench::probe::commit_of(&tree);
    assert!(
        !head.ends_with("+dirty"),
        "the scratch repository starts clean: {head}"
    );

    let pairs = Pair::parse_all("omp:m1,claude-code:sonnet").unwrap_or_else(|e| panic!("{e}"));
    let options = matrix_options(&tree, &dir.join("out"));

    // **A pair, as `matrix::run` runs it**: its commit is read when it starts, as `run` writes the run's header; its
    // report is built from what it wrote. The stand-in writes nothing outside its own `--out`.
    let commits: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let stand_in =
        |one: &Options, _held_out: Option<&Path>| -> Result<(Report, Vec<PathBuf>), String> {
            let header = record(&one.out, one);
            commits.borrow_mut().push(header.commit.clone());
            let data = RunData::read(&one.out)?;
            Ok((Report::build(&data), Vec::new()))
        };

    let (matrix, written) =
        matrix::run_with(&options, None, &pairs, &stand_in).unwrap_or_else(|e| {
            panic!("a labelled matrix of two pairs is a matrix, not a refusal: {e}")
        });

    let commits = commits.into_inner();
    assert_eq!(commits.len(), 2, "both pairs ran");
    assert_eq!(
        commits[1], head,
        "the second pair read the commit, not `{head}+dirty`"
    );
    assert_eq!(commits[0], commits[1], "and read what the first pair read");
    assert_eq!(matrix.rows.len(), 2, "a row per pair");
    assert_eq!(
        matrix.commit, head,
        "the matrix names the commit every pair ran at"
    );

    // **The label did publish**, after the run: a matrix and both runs are in `docs/bench`, and the README lists them.
    let matrices = tree.join("docs/bench/matrices");
    assert!(
        matrices.is_dir(),
        "the label published the matrix: {}",
        matrices.display()
    );
    for pair in &pairs {
        let stem = format!("2026-11-13-matrix-{}", pair.slug());
        assert!(
            tree.join("docs/bench/runs")
                .join(format!("{stem}.md"))
                .is_file(),
            "the label published `{stem}`"
        );
    }
    let readme = std::fs::read_to_string(tree.join("docs/bench/README.md"))
        .unwrap_or_else(|e| panic!("{e}"));
    assert!(
        readme.contains("## Matrices"),
        "the README lists the matrix:\n{readme}"
    );
    assert!(
        written
            .iter()
            .any(|p| p.starts_with(tree.join("docs/bench"))),
        "the publication is among the paths written"
    );
    // **The publication is real and it came last**: the tree it happened in is dirty now, and no pair read that.
    assert_ne!(
        bevy_cauldron_bench::probe::commit_of(&tree),
        head,
        "the label wrote tracked files"
    );
}

// -------------------------------------------------------------------------------------------------
// What the fix must not allow: pairs that really ran at different commits.
// -------------------------------------------------------------------------------------------------

#[test]
fn a_matrix_whose_pairs_really_ran_at_different_commits_is_still_refused() {
    let dir = scratch("mixed");
    let tree = dir.join("repo");
    repo(&tree);
    let pairs = Pair::parse_all("omp:m1,claude-code:sonnet").unwrap_or_else(|e| panic!("{e}"));
    let options = matrix_options(&tree, &dir.join("out"));

    // **A commit lands between the two pairs**, as it would if the tree moved while the matrix ran: the second pair
    // measures a different tree, and no ordering of the publication makes that not so.
    let commits: RefCell<Vec<String>> = RefCell::new(Vec::new());
    let stand_in =
        |one: &Options, _held_out: Option<&Path>| -> Result<(Report, Vec<PathBuf>), String> {
            if commits.borrow().len() == 1 {
                std::fs::write(one.repo.join("tracked.txt"), "two\n")
                    .map_err(|e| format!("{}: {e}", one.repo.display()))?;
                let moved = std::process::Command::new("git")
                    .arg("-C")
                    .arg(&one.repo)
                    .args(["commit", "--quiet", "-am", "a commit between two pairs"])
                    .output()
                    .map_err(|e| format!("git: {e}"))?;
                if !moved.status.success() {
                    return Err(format!("{}", String::from_utf8_lossy(&moved.stderr)));
                }
            }
            let header = record(&one.out, one);
            commits.borrow_mut().push(header.commit);
            let data = RunData::read(&one.out)?;
            Ok((Report::build(&data), Vec::new()))
        };

    let refused = match matrix::run_with(&options, None, &pairs, &stand_in) {
        Ok((matrix, _)) => panic!(
            "two editor commits are two things, and a matrix built anyway: {}",
            matrix.commit
        ),
        Err(said) => said,
    };
    assert!(refused.contains("editor commit"), "{refused}");
    assert!(
        refused.contains("same tasks, trials and editor commit"),
        "it says why: {refused}"
    );
    let commits = commits.into_inner();
    assert_eq!(commits.len(), 2, "both pairs ran");
    assert_ne!(
        commits[0], commits[1],
        "the two pairs really did run at different commits"
    );
    assert!(
        !tree.join("docs/bench/matrices").exists(),
        "a refused matrix publishes nothing, and least of all a matrix"
    );
}
