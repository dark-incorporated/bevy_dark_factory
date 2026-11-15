# bevy_grimoire_rhai

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

Rhai scripts on Bevy entities, reloaded live, for developers whose game's behaviour is too conditional for a component's fields to describe: the one Rhai host a `bevy_grimoire` game runs its scripts on.

[![crates.io](https://img.shields.io/crates/v/bevy_grimoire_rhai.svg)](https://crates.io/crates/bevy_grimoire_rhai) [![docs.rs](https://img.shields.io/docsrs/bevy_grimoire_rhai)](https://docs.rs/bevy_grimoire_rhai) [![CI](https://github.com/dark-incorporated/bevy_dark_factory/actions/workflows/ci.yml/badge.svg)](https://github.com/dark-incorporated/bevy_dark_factory/actions/workflows/ci.yml) [![Bevy 0.20](https://img.shields.io/badge/bevy-0.20-purple.svg)](https://bevyengine.org) [![licence](https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-blue.svg)](#who-and-licence)

**Status:** usable · Bevy 0.20 · MSRV 1.97.1

**Why:** a script is only worth editing while the game runs if saving it changes the running game and a mistake in it cannot end the game. Saving recompiles the file in place and keeps the state the script kept; a file that does not compile leaves the last good version running and raises a problem naming the file and the line.

## Quick start

```sh
cargo add bevy_grimoire_rhai
```

A `Script` is a component naming a `.rhai` file, relative to the project root you give `ScriptPlugin`. The file defines up to four hooks (functions the plugin calls), and the plugin calls the ones it finds:

```rhai
// scripts/door.rhai
fn on_start()             { this.state.open = false; }
fn on_update(dt)          { this.state.frames = (this.state.frames ?? 0) + 1; }
fn on_event(name, data)   { if name == "pressed" { this.Door.open = true; } }
fn on_trigger(other)      { print(other); }
```

```rust,no_run
use bevy::prelude::*;
use bevy_grimoire_rhai::{Script, ScriptPlugin};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, ScriptPlugin { root: ".".into() }))
        .add_systems(Startup, |mut commands: Commands| {
            commands.spawn((Name::new("door"), Script { file: "scripts/door.rhai".into() }));
        })
        .run();
}
```

With `bevy_grimoire`'s `GamePlugin` you write no Rust: an entity file does the same.

```yaml
# <project>/game/room.entities.yaml
entities:
  door:
    Script: { file: "scripts/door.rhai" }
```

## What a script can do

- **Read and write its own entity's components by name**: `this.Health.current -= 1`. `this` is built from the components the game registered that this entity actually carries, so a script touches exactly what a YAML file could set, and a component the game did not register cannot be named by a script any more than by a file.
- **Find another entity by its `Name`**: `find("guard")` answers its components as a value, `world_get("guard", "Health")` answers one, and `world_set("guard", "Health", data)` writes one back. It is two calls rather than one handle because a write through a map a native function returned lands on a copy.
- **Send and receive events**: `emit("door_open")` writes a `ScriptEvent` every scripted entity may see, and `on_event(name, data)` is called for each. A game's own code writes the same message to address one script by `Name`.
- **Start a timer**: `timer("slow", 2.5)` fires `on_event("timer", "slow")` once, 2.5 s later, for the entity that started it. The entity needs a `Name`, because that is how the message is addressed.

`on_trigger(other)` runs from a `ScriptEvent` named `trigger`; `bevy_grimoire`'s `GamePlugin` writes one with the other entity's `Name` when a body enters a `Trigger` volume.

## Saving a script changes the running game

Saving the `.rhai` file recompiles it. **The state a script keeps in `this.state` survives the reload**, so a counter keeps counting. A file that does not compile leaves the last good version running and raises a `ScriptProblem` naming the file and the line. A call that runs past `SCRIPT_STEP_LIMIT` operations (200,000) is stopped with a problem naming the file, and so is a string, array or map that outgrows its named cap, so a runaway script costs one call rather than the game. A problem is raised once per edit, not once a frame.

`import` resolves nothing: a script cannot load a file the game did not put in its project.

## Bevy compatibility

| `bevy_grimoire_rhai` | Bevy |
|---|---|
| 0.1 to 0.2 | 0.20 |

## Learn more

- API reference: [docs.rs/bevy_grimoire_rhai](https://docs.rs/bevy_grimoire_rhai).
- [Scripts](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/scripts.md) in the guide, and [behaviours](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/behaviours.md) for what a component's fields can already say.
- The tests that prove the reload and the limits: [`tests/`](https://github.com/dark-incorporated/bevy_dark_factory/tree/main/crates/bevy_grimoire_rhai/tests).
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_grimoire.html) says how they fit together.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-APACHE).
