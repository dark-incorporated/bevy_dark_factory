# blueprint_model

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

The engine-free core of the DarkFactory's world building, for Rust developers who read, check, generate or tool a blueprint (the folder of YAML files the DarkFactory editor writes and a game loads) without linking a renderer: the file schemas, a project loader, a constraint solver front-end, wave function collapse and the checks that run over a map.

[![crates.io](https://img.shields.io/crates/v/blueprint_model.svg)](https://crates.io/crates/blueprint_model) [![docs.rs](https://img.shields.io/docsrs/blueprint_model)](https://docs.rs/blueprint_model) [![CI](https://github.com/dark-incorporated/bevy_dark_factory/actions/workflows/ci.yml/badge.svg)](https://github.com/dark-incorporated/bevy_dark_factory/actions/workflows/ci.yml) [![licence](https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-blue.svg)](#who-and-licence)

**Status:** usable · MSRV 1.97.1

**Why:** the game, the editor, the MCP server and headless tools all need the same schema and the same refusals, and none of them should have to agree on a renderer to share them. `tests/engine_free.rs` fails the build if a `bevy`, `avian`, `wgpu` or `winit` dependency appears in the manifest, so the boundary is a compiler fact and not a comment.

## Upgrading from 0.11

0.12 removes two things and adds one:

- **The `references` module is removed**, with its `References`, `Reference` and `REFERENCES_VERSION`. A map's per-map list of captioned pictures (`maps/<map>.references.*`) is gone. The **`board` module replaces it**: every place of the design tree has a dream board at `<place>/board/board.yaml` (read with `board::Board::of_place`, edited with `add`, `remove`, `recaption` and `move_to`), and pictures are cited by their path instead of an `r1` id. `bevy_dark_factory migrate` moves an old project's references file to the level's board with the same captions.
- **`trust::GuideGrant` is removed.** A grant is now one `trust::TextGrant` per file (it carries the file's `path`), so the project's guide and every style file share one store and one question. Use `TrustStore::text_decision`, `trust_text` and `revoke_text`; `guide_decision` and `trust_guide` remain as the guide's path. `TRUST_VERSION` is 3, and a version 1 or 2 store is still read: a version 2 guide grant becomes a text grant for the guide's path, so what a person already allowed stays allowed.
- **A new `style` module** answers the style of one place as an agent reads it, behind those text grants.

## Quick start

```sh
cargo add blueprint_model
```

Read a project the way a game does: every file the editor writes, in one call, cross-checked against each other. This reads the example project in this repository's [`assets/blueprint`](https://github.com/dark-incorporated/bevy_dark_factory/tree/main/assets/blueprint):

```rust,no_run
use std::path::Path;

use blueprint_model::project_data;

fn main() -> Result<(), String> {
    // `assets/blueprint` is the folder that holds `kits.yaml`; the names are the maps to load.
    let project = project_data::load(Path::new("assets/blueprint"), &["bedroom"])?;

    let map = &project.maps["bedroom"];
    println!(
        "`{}`: {} placements from a library of {} pieces",
        map.name,
        map.placements.len(),
        project.bound.library.descriptors.len(),
    );
    Ok(())
}
```

A file that does not read is refused at load, naming the file, and so is an unknown vocabulary token, a rule that names a piece the library lacks and a map that places a piece the library lacks.

## What is in it

Every module is public; the ones to start from:

- **The project's files**: `descriptor` and `library` (what a mesh is, where it may go, what it offers), `vocab`, `kits`, `map`, `composition`, `rules`, `standards`, `policy`, `relations`, `notes`, `plan`, `rigs`, `play`, `family`, `destructible`, `heightfield`, `volume`, `water` and `data_files`, with `project_data` to load them together.
- **YAML in and out**: `yaml` is the one place a project file is read or written, and its refusals name a file, a line and a field path; `yaml_surgery` edits one value and keeps every comment; `schema` generates one JSON Schema per file kind from the type, for an outside editor's completion; `save` and `atomic_file` write a file so a reader sees the old one or the new one, never a mix.
- **Design and trust**: `design` (the game's style as a tree of places), `board` (a dream board per place), `style`, `trust` (what a person allowed a project to run or an agent to read), `agent_guide`, `lesson` and `wish`.
- **Release and live reload**: `bake` turns every YAML file a game loads into postcard binaries proved against the YAML, `load` reads whichever the process opened, and `reload`, `reload_check` and `live` (with its `Verdict` types) say what a running game does when a file changes on disk.
- **Generation and measurement**: `constraints` (a constraint layer over Boolean variables, solved by [`blueprint_solver`](https://crates.io/crates/blueprint_solver)), `placement` (the constraint IR, its solver back-ends and the pass that scatters small props onto surfaces), `wfc` and `grammar` (wave function collapse, and learning a map's grammar to continue it), `geom` (Poisson-disk sampling and Delaunay triangulation), `run`, `range` and `repetition`.
- **Assets**: `glb` (a hand-written glTF reader that measures a mesh), `import` (propose a descriptor for a mesh), `clips` and `rig_check` (what a rig's animations contain) and `gait`.
- **`rng`**: a re-export of [`blueprint_rng`](https://crates.io/crates/blueprint_rng), the one seeded generator everything here draws through.

RON files from before the YAML move are converted by the separate `bevy_blueprint_migrate` crate (`bevy_dark_factory migrate <project folder>`); nothing in this crate reads RON.

## Try it

From a clone of the [repository](https://github.com/dark-incorporated/bevy_dark_factory); all of them print to the terminal and need no engine, assets or GPU except `import_dir`, which reads a folder of `.glb` files.

```sh
cargo run -p blueprint_model --example wfc_grid         # wave function collapse over a terrain alphabet you define
cargo run -p blueprint_model --example poisson          # Poisson-disk sites, Delaunay triangulation, a degree cap
cargo run -p blueprint_model --example det_rng          # the one seeded generator: reproducible, unbiased below(n)
cargo run -p blueprint_model --example faithful_port    # the constraint encoding agrees with the WFC tables
cargo run -p blueprint_model --example expressive_range # the composition grammar measured against thresholds set in advance
```

`empty_weight` re-reads the same measurements while sweeping one prototype's weight, and `import_dir` imports every `.glb` under a directory into a library file.

## Bevy compatibility

None: this crate has no Bevy dependency, so every version works with any Bevy.

## Learn more

- API reference: [docs.rs/blueprint_model](https://docs.rs/blueprint_model).
- The files a project keeps and their formats: [data files](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/data-files.md); what a running program does when one changes: [the reload contract](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/reload-contract.md); using the crates in a game: [your game](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/your-game.md).
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_blueprint.html) says how they fit together. [`bevy_blueprint`](https://crates.io/crates/bevy_blueprint) puts what this crate reads into a Bevy world.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-APACHE).
