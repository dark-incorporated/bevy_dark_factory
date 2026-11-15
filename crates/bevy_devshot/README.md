# bevy_devshot

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

A frame on demand for Bevy apps, for developers whose screenshot comes from a script, a CI job or an agent rather than a person at the keyboard: `touch screenshot.request` and the next frame writes `screenshot.png`, rendered straight from the GPU.

**Status:** experimental · Bevy 0.20

**Why:** the thing that wants the screenshot is usually not a person at a key binding. A sentinel file (a file whose appearance is the signal) works over ssh, from a Makefile, inside CI and from an agent driving a window it does not own, with no operating-system screen capture, no accessibility permission prompt and no compositor.

## Quick start

`bevy_devshot` is not on crates.io; take it from the repository:

```sh
cargo add bevy_devshot --git https://github.com/dark-incorporated/bevy_dark_factory
```

```rust,no_run
use bevy::prelude::*;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);

    // A shipped game has no reason to watch the filesystem for a screenshot request.
    #[cfg(debug_assertions)]
    app.add_plugins(bevy_devshot::DevShotPlugin);

    app.run();
}
```

Then, from any other process, in the directory the app was started in:

```sh
touch screenshot.request     # -> screenshot.png in the working directory
```

That is the whole API. The plugin polls one `Path::exists` per frame and does nothing else; when the request file is there it deletes it, takes a screenshot of the primary window and writes `screenshot.png`, logging `devshot: wrote screenshot.png`.

## The `png` feature is load-bearing at runtime

This crate takes `bevy` with `default-features = false` and four features: `bevy_render`, `bevy_window`, `bevy_log` and `png`. Bevy's `save_to_disk` picks its encoder from the file extension through the `image` crate, so dropping `png` compiles and then fails with an unsupported-format error at the moment you wanted a screenshot. It is listed explicitly for that reason.

## Try it

From a clone of the [repository](https://github.com/dark-incorporated/bevy_dark_factory):

```sh
cargo run -p bevy_devshot --example capture
# then, from another terminal, in the same directory:
touch screenshot.request
```

Opens a window with a spinning cube and writes `screenshot.png` once the sentinel file appears. It is the only example here, and it needs a GPU and a display.

The Cauldron editor uses this plugin for its whole-frame capture; see [`bevy_cauldron`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/bevy_cauldron/README.md).

## Learn more

- The source is one file: [`src/lib.rs`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/bevy_devshot/src/lib.rs).
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_grimoire.html) says how they fit together.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/bevy_devshot/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/bevy_devshot/LICENSE-APACHE).
