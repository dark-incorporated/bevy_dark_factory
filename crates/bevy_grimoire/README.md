# bevy_grimoire

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

The runtime half of grimoire, for Bevy developers whose levels are authored in the Cauldron editor: load a library of pieces and a map from a project's YAML and put them in a Bevy world, with the game's own behaviours and scripts defined in data.

**Status:** usable · Bevy 0.20 · MSRV 1.97.1

**Why:** the editor shows a map and the game plays it, and they must agree down to the last degree of yaw. `spawn_descriptor` is the single shared spawner, so a map cannot look one way in the editor and another in the game. This is the only crate in the grimoire set that knows what a renderer is, on purpose: the schema and the checks live in the engine-free [`bevy_grimoire_core`](https://crates.io/crates/bevy_grimoire_core).

## Quick start

```sh
cargo add bevy_grimoire
```

A whole game made of data is one line of Rust. `GamePlugin` reads the project at `$GRIMOIRE_ROOT` (the working directory when that is not set): its behaviours, the entity files in `game/*.entities.yaml`, the Rhai scripts those entities name, and the project's first map, which it puts on the ground.

```rust,no_run
use bevy::prelude::*;
use bevy_grimoire::GamePlugin;

fn main() {
    App::new().add_plugins((DefaultPlugins, GamePlugin)).run();
}
```

The repository's [sample game](https://github.com/dark-incorporated/bevy_dark_factory/tree/main/games/sample) is exactly that, plus YAML and scripts. A map that is refused at load (an undefined piece, say) stops the game with the file's own sentence rather than half-loading.

## What you can do

- **Place a map** with `GrimoirePlugin` and a `GrimoireWorld` resource. `GrimoireWorld::new` validates a library and a map together and resolves masks, stacked heights, role masks and seat positions once at load, before anything spawns; each placement then becomes one entity carrying `Placement`, `OfDescriptor`, `Tags` and, where it stands on something, `MountedOn`.
- **Give entities behaviours from YAML.** `BehaviourPlugin` registers `Trigger`, `Switch`, `Door`, `Spawner`, `Pickup`, `Health`, `Damage`, `Countdown`, `PathFollow`, `StateMachine`, `Body`, `Collector` and `Walker` as reflected components, and a file in `<project>/game/` sets them. A behaviour writes a message where another system has to act, and none registers itself: the plugin is the opt-in, so a game's own `Health` stays unambiguous.
- **Reload while the game runs.** `GrimoireLivePlugin { dir, maps }` watches the project and, about a second after the last write, reloads it and redraws only the placements that changed; an unchanged placement keeps its `Entity`. A reload either loader refuses leaves the world as it was.
- **Script what fields cannot say.** `GamePlugin` runs the `.rhai` files an entity's `Script` names, through [`bevy_grimoire_rhai`](https://crates.io/crates/bevy_grimoire_rhai).
- **Ship a release that reads a bake.** With the `baked` feature the plugin reads the postcard files `bevy_cauldron bake` made, refuses to start naming the file if the YAML has changed since, and never falls back to the YAML.

```yaml
# <project>/game/room.entities.yaml
entities:
  latch: { Switch: { latch: north, on: false } }
  gate:  { Door: { link: north, open: false } }
```

## Try it

From a clone of the [repository](https://github.com/dark-incorporated/bevy_dark_factory):

```sh
cargo run -p bevy_grimoire --example spawn_headless
```

Builds a library and a map in code, shows a map naming an undefined piece being refused at load, then hands the valid one to `GrimoirePlugin` and counts the spawned entities. It runs with `WgpuSettings { backends: None }`, so it needs no GPU and no asset on disk: what it demonstrates is placement, not pixels.

## Learn more

- API reference: [docs.rs/bevy_grimoire](https://docs.rs/bevy_grimoire).
- [Set up your game](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/your-game.md), [behaviours](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/behaviours.md) (the table and the rules), [scripts](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/scripts.md) and [the reload contract](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/reload-contract.md).
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_grimoire.html) says how they fit together.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-APACHE).
