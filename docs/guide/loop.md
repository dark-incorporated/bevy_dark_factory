# The development loop

The Dark Factory is built in passes. A pass takes one bounded piece of work (a cluster of related issues, or one enhancement), proves it with a test that failed before and passes after, ships it, and writes down what it did. This page describes what a pass leaves behind, so you can find it.

The brief a pass follows is `.omp/commands/build-loop.md`. A **Wiggum loop** is a dumb driver (`tools/loop/wiggum.sh`) that runs one fresh `omp` process per pass over that brief, with the repository as its only memory; [`docs/loop/RUNNING-A-LOOP.md`](../loop/RUNNING-A-LOOP.md) says how to start one and the facts that each cost a cycle, and `tools/loop/README-wiggum.md` how to read one that is running. A **fleet** is a manager session that runs one worker per machine with `tools/loop/fleet/`, each taking a cluster of related issues; its tools are described in `tools/loop/fleet/README.md`.

How the documentation is written is in [`docs/loop/doc-guidance.md`](../loop/doc-guidance.md) and in the family's [documentation guide](https://github.com/dark-incorporated/.github/blob/main/DOCUMENTATION.md).

## One pass

A pass runs these phases in order, and marks each with `python3 tools/loop/steps.py mark <phase>` so the driver can time it:

1. **Orient.** `python3 tools/loop/steps.py begin` numbers the pass. Read the last log lines and the filed issues, and check the repository is in the state the last pass left it.
2. **Bugs.** Open `bug` issues come first, then `consumer` issues (a game asks for what it needs with the `consumer` label), then the owner's focus.
3. **Choose.** The next item is whichever the order above names first that the pass can prove with a check that fails before and passes after; otherwise the pass ships nothing.
4. **Build and prove.** The failing test is written first and its failing message is quoted in the report. No existing check may be weakened.
5. **Ship.** Commit, then push to `main` through the gate (`tools/loop/hooks/pre-push`: `python3 tools/loop/backlog.py check`, `python3 tools/changelog/changelog.py check` and `python3 tools/loop/version.py check`, then `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`, `cargo test --workspace --doc` and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`; the last of these is the step that reads a doc comment). Add a changelog line (with a clip when someone can see the change), a line in `docs/loop/log.md` and a report.
6. **Release.** Only when the pass changed what a package ships. The editor's version comes from `python3 tools/loop/version.py next`, a bump is its own `release: <version>` commit. The release workflow prints a benchmark comparison but is **not** stopped by it (owner, 2026-11-16: `bash tools/release/bench_advisory.sh` exits 0 whatever the verdict says); [`docs/loop/RUNNING-A-LOOP.md`](../loop/RUNNING-A-LOOP.md#a-release-prints-its-benchmark-comparison-it-is-not-stopped-by-it-owner-2026-11-16) is the one place that split is stated. Then `python3 tools/loop/steps.py end`.

## State files

| File | What it holds |
|---|---|
| `docs/loop/focus.md` | What the owner wants worked on next, in order, with the owner's own words. |
| `docs/loop/backlog.json` | The working list of bugs and enhancements, edited one item at a time with `tools/loop/backlog.py`. |
| `docs/loop/backlog-archive.json` | Fixed, rejected and stale items. |
| `docs/loop/log.md` | One line per pass, newest at the bottom. Append only. |
| `docs/loop/iterations/` | One report per pass or release, named `<date>-<area>-<sha>.md`. |
| `docs/loop/iterations/assets/` | Curated stills and frames a report cites. |
| `docs/loop/needs-decision/` | Questions the loop could not settle alone, each with a recommendation. |

Raw captures and check logs go under `target/loop/<date>-<sha>/`. They are not committed; a report links only the curated assets.

## What a pass may not change

Tests, golden files, fixtures and the scoring harness are read-only to a pass: a loop that can edit its own evaluator converges on passing the evaluator. A pass may add a test, a case or a field default, and it records any exception the owner gave. A change that removes, loosens or skips a check is logged as `blocked` and left for a person. The list is the "protected evaluator" section of `.omp/commands/build-loop.md`.
