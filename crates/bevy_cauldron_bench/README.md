# bevy_cauldron_bench

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

A benchmark for AI agents that drive the Cauldron editor, for the people who build the editor and want to know whether a change helped an agent: a task is a prompt, a starting project, a known answer and a list of checks, and `cauldron-bench` grades what a real agent leaves in a real editor, from the saved files.

**Status:** experimental · MSRV 1.88

**Why:** a task a run can only pass is measuring nothing, so every task proves itself with no model in the loop: an agent that connects and stops must fail at least one check, and the task's own solution applied through the editor must pass them all. The checks read the saved map, the library and the operation log, never the operation that made the change, so a bug in an operation cannot hide in the check that was meant to catch it.

## Quick start

`bevy_cauldron_bench` is not on crates.io. From a clone of the [repository](https://github.com/dark-incorporated/bevy_dark_factory), build the editor and the MCP server under test (the runner uses the ones beside its own binary), then run the two agents that need no model:

```sh
cargo build -p bevy_cauldron -p bevy_cauldron_mcp
cargo run -p bevy_cauldron_bench -- run --agent oracle    # applies each task's own solution: must pass every task
cargo run -p bevy_cauldron_bench -- run --agent nothing   # connects and stops: must pass none
```

If the oracle fails a task, or `nothing` passes one, that is a harness problem and not a result. A real agent is `--agent omp` (the default) or `--agent claude-code`:

```sh
cargo run -p bevy_cauldron_bench -- run --trials 5 --agent omp --model <provider/model>
```

## The commands

| command | what it does |
|---|---|
| `cauldron-bench run` | takes every task of a suite `--trials` times; each trial copies the fixture project, starts the editor on its own port (and, on Linux, its own `Xvfb` display), hands the agent the same MCP config a person's agent gets and the task's prompt, stops it at the task's wall-clock budget, and grades what is left. Options: `--suite <dir>` (default `bench/tasks`), `--project <root>`, `--agent`, `--model`, `--variant`, `--task <id>`, `--jobs <n>`, `--held-out <dir>`, `--label <name>` |
| `cauldron-bench report <run directory>` | builds the report (success with a 95% interval, `pass^k`, wall time, tool calls, refusals, undos, edits behind the editor's back, tokens and cost, per task and tag) from trials already run |
| `cauldron-bench compare <a> <b>` | says `better`, `worse` or `no measurable change` with a paired 95% bootstrap interval, and refuses a run with more than 10% harness errors |
| `cauldron-bench matrix --pairs <agent:model,...>` | one suite across several agent and model pairs on the same tasks and editor commit, with success against cost and the frontier marked |
| `cauldron-bench clean` | removes the credential directories (a trial's `home` and `claude-config`, a run's `preflight`) that older runs left under the state folder, those older than `--older-than <days>` (default 1); `--dry-run` lists them |

Each trial ends as `pass`, `fail` or `harness_error`, and a harness error (an editor that did not start, a transcript that could not be read) is never scored as either of the others. Trials and reports go under `~/.local/state/cauldron-bench/<run>/`, never the repository; `--label` also publishes a report to [`docs/bench/runs/`](https://github.com/dark-incorporated/bevy_dark_factory/tree/main/docs/bench).

## The tasks

The repository's 20 development tasks are in [`bench/tasks/`](https://github.com/dark-incorporated/bevy_dark_factory/tree/main/bench/tasks), one `task.yaml` each, against the project in [`bench/fixture/`](https://github.com/dark-incorporated/bevy_dark_factory/tree/main/bench/fixture). A task looks like this:

```yaml
# bench/tasks/bed-against-the-north-wall/task.yaml, abridged
id: bed-against-the-north-wall
title: The bed against the north wall
split: dev                              # dev | held_out
tags: [place, relations]
map: bench_bedroom
prompt: |
  Put a double bed flat against the north wall of the bedroom, centred on that wall.
  ...
setup: []                               # operations applied before the agent starts
solution:                               # a known answer, proved with no model
  - op: placement_add
    params: {piece: bed_double, at: [0.0, -1.39], yaw: 0.0}
  - op: grimoire_present                # the hand-back
    params: {title: ..., summary: ...}
checks:                                 # every one must hold on what the run left
  - placement_near_wall: {piece: bed_double, wall: north, max_gap_m: 0.05, centre_within_m: 0.2}
  - no_problem: [overlap]
  - oplog_has: {op: placement_add, min: 1}
budget: {wall_s: 600, tool_calls: 60}
```

## What is in the library

Nothing in `check`, `grade`, `suite`, `task` or `fixture` imports Bevy: the grader is a walk over files and the core crate's arithmetic, which is what lets one piece of code score both a headless proof in the test gate and a real agent's trial. `run` is the harness around it and the only module that owns a process or a socket; `claude` reads Claude Code's transcript; `report`, `compare`, `matrix` and `stats` build what a run publishes. The binary is `cauldron-bench`.

## Learn more

- [`bench/README.md`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/bench/README.md): how a trial runs, how each agent is isolated, what the agent inside a trial can read, and how an omp agent's calls are read.
- [`docs/bench/README.md`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/bench/README.md): every committed run, newest first.
- [`bench/tasks/README.md`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/bench/tasks/README.md): the task format and its checks.
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_grimoire.html) says how they fit together.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-APACHE).
