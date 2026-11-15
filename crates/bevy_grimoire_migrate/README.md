# bevy_grimoire_migrate

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

Moves a Cauldron project's files from RON (Rusty Object Notation) to YAML without losing anything, for anyone whose project was written before the editor switched to YAML. Every file is converted in memory and read back before anything is written, comments travel with their keys, and the old files are archived rather than deleted.

**Status:** experimental · MSRV 1.97.1

**Why:** RON and YAML spell an `Option` and an enum differently, so a text substitution produces a file that looks right and holds a different value. This crate compares the values the types read, and when one differs it refuses the whole migration, names the file and writes nothing. It is also the only crate in the workspace that still reads RON, so `bevy_grimoire_core` does not depend on `ron`.

## Quick start

You rarely call this crate yourself: the editor's binary has a subcommand that runs it and exits, with no window, so it works over ssh on a machine with no GPU.

```sh
cargo run -p bevy_cauldron -- migrate <project folder>
```

`<project folder>` is the game's root, the folder that holds `assets/grimoire`. The command prints one line per converted file, in the form `<old path> -> <new path> (<kind>), N comment(s) carried`, then any comment that had no place in its YAML file, then where the originals went. A project with no RON project file prints `nothing to migrate`. The editor also refuses to open a project that still holds RON files, and its refusal names this command.

From Rust, the same conversion is one call. `bevy_grimoire_migrate` is not on crates.io, so depend on it from the repository:

```sh
cargo add bevy_grimoire_migrate --git https://github.com/dark-incorporated/bevy_dark_factory
```

```rust,no_run
use std::path::Path;

fn main() {
    match bevy_grimoire_migrate::convert_project(Path::new("my_game")) {
        Ok(report) => report.lines().iter().for_each(|line| println!("{line}")),
        Err(problem) => eprintln!("{}", problem.line()),
    }
}
```

## What it does

- **Converts every project file a file kind names**, plus the data files a game registers in its `data.yaml`: as the kind's own type where this crate has the type, and by value otherwise (`convert_value`, `read_ron`, `written_as_yaml`). A `.ron` file no kind names is not a project file and is left alone.
- **Is all or nothing.** Only when every conversion is lossless is a single file written; then the RON originals are moved into `.dark_factory/migrated-ron/` under the project root. A refusal leaves the project byte for byte as it was, and a success leaves no `.ron` project file behind.
- **Carries comments.** A comment moves to the YAML key it stood on. One that stood outside any key, such as a file header, has no node to hang from; it is listed in the report (`Report::unplaced`) rather than dropped.
- **Upgrades a person's own files** (`upgrade_user_files`): `settings.ron`, `recent.ron` and `trusted.ron` become YAML beside their old place, with the originals moved to a `migrated-ron/` folder next to them. A file that cannot be converted is named, with the reason, and the original stays where it is.
- **Moves a per-map references list to the level's dream board**, with the same captions, so the pictures a map pointed at are still there under the design tree.

`leftover(root, command)` is the refusal an editor's open raises when a project has not been converted, and `plan` lists what a migration would convert without converting it.

## Examples

The three programs in [`examples/`](https://github.com/dark-incorporated/bevy_dark_factory/tree/main/crates/bevy_grimoire_migrate/examples) are for the repository's own tooling and are not product paths. `same_value_as_the_ron_original` reads a list of RON files from a base commit on stdin and reports whether each converted fixture reads to the same value as its original; `fixtures_to_yaml` and `oracle_fixtures_to_yaml` converted test fixtures from RON to YAML.

## Learn more

- "Moving a project to YAML" in [set up your game](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/your-game.md) explains the move for a project's owner, and [data files](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/data-files.md) lists the files and their formats.
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_grimoire.html) says how they fit together.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-APACHE).
