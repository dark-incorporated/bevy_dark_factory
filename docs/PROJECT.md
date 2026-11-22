# What the Dark Factory is, why it exists, and what is wrong with it

An outside-in description of the project, measured on 2026-11-15 at commit `198b82ae` (2,055 commits). Everything here is checkable against the repository: the numbers were counted with the commands named beside them, and where a sentence is a judgement and not a count it says so. It is the one page that lists what does not work.

The names: the editor is the Dark Factory (this repository, `bevy_dark_factory`). Its binary and crates are named for it in snake_case (`bevy_dark_factory`, `dark_factory_mcp`); its earlier name was the Cauldron. What it writes and a game loads is still a blueprint (`bevy_blueprint*`, `assets/blueprint/`).

---

## 1. The one-sentence version

**The Dark Factory is a world editor for Bevy games in which a person and an AI agent edit the same map through the same named operations, with one undo history between them.**

That is the design constraint everything else follows from, and what makes it different from a level editor with an AI chat box bolted on.

---

## 2. The problem it is trying to solve

A game developer building a world has two tools that do not talk to each other: the editor, where things get placed, and the agent, which can reason about what should be placed. Every existing bridge between them is lossy in the same way:

| The usual arrangement | What goes wrong |
|---|---|
| Agent writes a file, editor reloads it | The change has no undo entry, no author, and no validation until the reload |
| Agent writes a script that calls editor APIs | The script is not auditable, not replayable, and bypasses every rule the UI enforces |
| Agent describes a change in prose | A person retypes it, and the retyping is where it goes wrong |
| Agent uses a separate "AI" path | Two implementations of "place a crate" that drift, and the AI path is always the worse one |

The answer is structural: **there is one way to change anything**, a typed operation in a single registry, and the keyboard, the menus and the agent all dispatch it. Undo, the log, the validators and the parity test follow from that one decision.

---

## 3. What it is made of

About 405,000 lines of Rust (`find crates -name '*.rs' | xargs cat | wc -l` per crate: source, tests and examples) in 13 crates, 844 Rust files under `crates/*/tests/` (`ls crates/*/tests/*.rs | wc -l`), 29 MCP tools, and 143 entries in the operation registry (`ops::registry()`; 136 in a default build, because the family crates' operations are behind cargo features: `bevy_dark_factory --print-operations` lists the ones a build has).

| Crate | Lines | On crates.io | What it is |
|---|---:|---|---|
| `bevy_dark_factory` | 294,204 | no | The editor. Opens a project directory, writes `<root>/assets/blueprint`. |
| `blueprint_model` | 67,339 | [yes](https://crates.io/crates/blueprint_model) | The engine-free core: the project's file types and its one YAML reader and writer, the design tree, the grammar learner, WFC (wave function collapse), validators. No Bevy. |
| `bevy_dark_factory_bench` | 16,330 | no | The benchmark runner and grader, `dark_factory-bench`. |
| `dark_factory_mcp` | 8,337 | no | The MCP server: 30 tools over stdio, talking to the running editor over BRP (the Bevy Remote Protocol). |
| `bevy_blueprint` | 8,200 | [yes](https://crates.io/crates/bevy_blueprint) | The runtime: loads a project into a game (`GamePlugin`, `BlueprintPlugin`, `BlueprintLivePlugin`). |
| `bevy_blueprint_migrate` | 3,155 | no | Moves a project's files from RON to YAML. |
| `bevy_oracle` | 2,218 | no | Typed questions to a decision model, authoring time only. |
| `bevy_blueprint_rhai` | 1,877 | [yes](https://crates.io/crates/bevy_blueprint_rhai) | The Rhai script host a game's scripts run on. |
| `bevy_blueprint_anim` | 1,327 | [yes](https://crates.io/crates/bevy_blueprint_anim) | Clip-weight blending on a skinned rig. |
| `blueprint_solver` | 1,304 | [yes](https://crates.io/crates/blueprint_solver) | A deterministic SAT back-end. |
| `dark_factory_snip` | 581 | [yes](https://crates.io/crates/dark_factory_snip) | Cuts one rectangle out of a picture of the editor. |
| `blueprint_rng` | 368 | [yes](https://crates.io/crates/blueprint_rng) | One seeded ChaCha8 stream for the whole simulation. |
| `bevy_devshot` | 134 | no | A frame on demand, from the GPU. |

`games/sample/` is a small game made of YAML and Rhai whose Rust is one line; its tests play it.

**The editor-to-core split is the architecture in one number.** The editor is a Bevy app and is tested headless only through a harness (`bevy_dark_factory::harness::build_headless`); the core is engine-free and is tested directly. `blueprint_model/tests/engine_free.rs` fails the build if a Bevy dependency appears in it.

### The operation registry

An operation is a trait object (`ops::Operation`) that declares four things: its parameters (a JSON schema an agent reads, `schema()`), a **plan** that is pure and changes nothing (`plan()`, which is what a preview is), a **commit** that is the only code that writes (`commit()`), and an **undo strategy** (`Undo::None`, `Undo::Inverse` or `Undo::Snapshot`). An agent lists them with `blueprint_list_operations` (or one with `{"operation": "<name>"}`), previews with `blueprint_preview_operation` and applies with `blueprint_apply_operation`. Two tests keep the registry honest: `operations_have_two_front_ends` enumerates it and fails when an operation lacks an entry point on either side, and `every_operation_has_a_row_in_the_agent_guide` fails when an operation has no row in [`docs/guide/agents.md`](guide/agents.md).

---

## 4. What the editor does today

- **Ten workspaces**, one job each, in a fixed order on `Cmd/Ctrl+1` to `0`: Project, Map, World, Kit, Animation, Sound, Files, Game, Agent and Design. A workspace is what is drawn; the project, the open map and the selection stay put when you switch ([`docs/ui/vision.md`](ui/vision.md)).
- **Authoring a map**: a palette, a ghost that shows what a click will place, fill, remove, turn, tip and lift; layers that pass each other (a floor tile under a crate is not in the crate's way: `stack::same_layer`); flush is legal and inside is refused (`OVERLAP_EPS`); and a continuation of a layout you started with WFC, drawn as a proposal before you keep it ([building maps](guide/building-maps.md)).
- **Kits and labels**: import a `.glb`, measure it, admit or refuse it; a vision model may propose labels, every proposal is marked unchecked and a person toggles each one ([kits and labels](guide/kits-and-labels.md)).
- **The game's style**: the Design workspace shows `design/` as a tree of places (the game, regions, levels, areas drawn on a map), each with Markdown style pages and a dream board of pictures ([agents](guide/agents.md#the-games-style)).
- **Files and the game**: every project file is YAML and the Files workspace edits them; the Game workspace plays and stops the game and shows its log and its state; `bevy_dark_factory bake` turns the data into a release, proved against the YAML ([data files](guide/data-files.md), [your game](guide/your-game.md)).
- **A terminal on every screen**, which starts collapsed (`Cmd/Ctrl+J`), and starts a connected agent when you ask; **three themes** (Tool dark, Parchment, High contrast) that recolour everything live; **a notification centre** on `Cmd/Ctrl+E` where each step and refusal is a row and the newest step has an Undo ([keys](guide/keys.md)).

### What an agent sees

- **30 tools**, `blueprint_`-prefixed and domain-worded (`blueprint_apply_operation`, not "run tool 7"), listed in [the MCP reference](mcp-tools.md), which is generated from the server and checked by a test (`docs_are_current`). `blueprint_guide` returns the shipped guide first; the server's instructions say to call it.
- **Every mutating answer carries an operation id** the agent cites and a person reverts by, and a change comes back with what it broke.
- **`dry_run` previews** anything; the preview is the operation's own pure `plan`, so it cannot half-apply.
- **`placement_add` refuses to land on a piece** unless told `replace: true`, and the answer to a replacing add names what it replaced.
- **A hand-back is the end of every task** (`blueprint_present`): a card the person keeps or undoes in one click, listing the style lines the agent followed. After it the agent may make one wish for a tool it lacked (`blueprint_wish`) and propose one line for the project's guide (`blueprint_lesson`), which the person keeps or discards.
- **One undo history.** The agent's change joins yours, and `Cmd+Z` takes back the newest change still in effect, whoever made it.

---

## 5. How it is measured

`dark_factory-bench` runs a real agent against a real editor: each trial copies a fixture project, starts the editor on its own port and display, hands the agent the MCP config the editor itself gives a person's agent, and grades the files left behind with no model in the loop. There are 20 development tasks in `bench/tasks/` and eight held-out tasks outside every repository. `oracle` (the task's own solution) must pass every task and `nothing` must pass none, which is what proves the pipe. `compare` says `better`, `worse` or `no measurable change` with a paired 95% bootstrap interval, and a release needs a committed report for its code that is not worse than the previous release's (`tools/release/bench_gate.py`). See [`bench/README.md`](../bench/README.md), [`docs/bench/README.md`](bench/README.md) for the committed runs, and the pre-registered study of whether the style helps an agent ([`docs/research/`](research/README.md)): on the sample game an agent with `blueprint_style` satisfied 24.0 of 24 style lines and one without it 10.4 (30 scored runs, one model, the report lists the limits).

---

## 6. What is wrong with it

This is the section to read if you are reading someone else's project.

### Structural problems

**1. The editor crate is enormous and unaudited.** `bevy_dark_factory` is 294,204 lines of Rust with its tests, written by AI agents working from a human's direction (the Vibe Coded banner on the README says so). It is covered by tests, and the tests are good, but a test suite proves the properties someone thought to assert, not that the code is comprehensible to the next person.

**2. Two copies of each tool's parameters.** The MCP server cannot depend on the editor crate (it is the bridge's client), so each tool's parameter struct in `dark_factory_mcp` and the editor's own in `bevy_dark_factory::ops_brp` are separate and kept in step by a hand-written `json!` body. They drifted once (B-052, a `turn` field accepted by the server and never sent) and the fix is a source-text ratchet, `a_tools_body_names_every_field`, which catches the drift and does not remove the duplication. A shared parameters crate would.

### Things that are limited by design

**3. Overlaps are only checked within a layer.** `stack::same_layer` is a table: a 2.2 m cabinet and a sconce at 1.8 m on the wall behind it can interpenetrate, and a door and a crate in its doorway do not conflict. Whether to refuse them is a decision about what an author wants refused.

**4. A style is advice, and what an agent does with it is measured on one game.** The style study is one sample game, one model and three tasks, and its with-style arm is told by the shipped guide to read the style and cite it, so a ceiling of 24 of 24 is the guide at work as much as a finding. Whether the style helps a person's game is not measured.

**5. The model's tool-name habit is not fixed.** omp mounts the editor's tools as `xd://` routes, and the model sometimes writes the route as a function name (`xd__mcp__dark_factory_blueprint_guide`), which omp refuses by name. The benchmark counts it as an agent error and scores the trial; nothing in the editor changes what the model writes ([`bench/README.md`](../bench/README.md), BM-11).

### Process

**6. Reviews are by model.** The loop's contract is an independent review before a change merges, and the reviewer is another model reading the diff. That finds defects, and it is not a human audit.

---

## 7. What is right with it

- **The one-operation rule is enforced by tests, not by convention** (`operations_have_two_front_ends`, `every_operation_has_a_row_in_the_agent_guide`, the parity of MCP and UI).
- **The repository writes down why.** Module docs argue rather than describe: `stack.rs` states the bounded claim of its overlap rule, and the changelog says for every change whether someone can see it and what proves it.
- **Refusal over degradation.** A file that does not read is refused with its file, field and line and the last good state stays loaded; a baked release refuses stale data naming the file; a blank `rules.yaml` is refused instead of read as "no rules".
- **The data is the project, and it hot-reloads.** The editor checks the project every quarter of a second and re-reads what changed; a running game can follow ([the reload contract](guide/reload-contract.md)).
- **Failing-before discipline.** A change's test is written first and its failing text is quoted in the report; a release is stopped by the benchmark.

---

## 8. Where it stands

| | |
|---|---|
| Workspaces | 10 |
| Operations | 143 in the registry, 136 in a default build |
| MCP tools | 30 |
| Test files | 844 under `crates/*/tests/` |
| Published | `bevy_blueprint`, `_core`, `_solver`, `_rng`, `_anim`, `_rhai` and `_snip` on crates.io; what `blueprint_model` 0.12 removed (the `references` module and `trust::GuideGrant`) is under "Upgrading from 0.11" in [its README](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/blueprint_model/README.md) |
| Editor release | the macOS app, on the [releases](https://github.com/dark-incorporated/bevy_dark_factory/releases) page |
| Open issues | 7 |

It runs, a game can load what it writes, and an agent can drive it end to end from the shipped guide alone. It also has an unaudited 294,000-line editor at its centre and a review process that is a model reading a diff. Both are true at once, and a document that said only one of them would be lying.
