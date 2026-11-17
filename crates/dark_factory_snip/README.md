# dark_factory_snip

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

Cutting one rectangle out of a PNG of the DarkFactory editor and saving it with a record of what was in it, for the code that handles snips: the editor that saves one and the MCP server that hands one to an agent. A snip is a rectangle a person drags round part of the editor, with a one-line note, so an agent can see and read what they mean.

[![crates.io](https://img.shields.io/crates/v/dark_factory_snip.svg)](https://crates.io/crates/dark_factory_snip) [![docs.rs](https://img.shields.io/docsrs/dark_factory_snip)](https://docs.rs/dark_factory_snip) [![CI](https://github.com/dark-incorporated/bevy_dark_factory/actions/workflows/ci.yml/badge.svg)](https://github.com/dark-incorporated/bevy_dark_factory/actions/workflows/ci.yml) [![licence](https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-blue.svg)](#who-and-licence)

**Status:** usable · MSRV 1.88

**Why:** the editor's snip and an agent's own crop must refuse exactly the same rectangles, so the bounds rule lives in one place that both call. A rectangle that is empty, reversed, negative or outside the picture is refused with the picture's size in the sentence, never clamped, because a clamped crop is a picture of a different place than the one asked for.

## Quick start

```sh
cargo add dark_factory_snip
```

```rust,no_run
use dark_factory_snip::crop_png;

fn main() -> Result<(), String> {
    let still = std::fs::read("editor.png").map_err(|e| e.to_string())?;

    // `[x0, y0, x1, y1]` in window pixels, origin top left, `x1` and `y1` exclusive.
    let crop = crop_png(&still, [100, 60, 420, 300])?;
    println!(
        "cut {}x{} out of {}x{}",
        crop.size[0], crop.size[1], crop.picture[0], crop.picture[1],
    );

    std::fs::write("crop.png", &crop.png).map_err(|e| e.to_string())?;
    Ok(())
}
```

## What is in it

- **`crop_png(png, rect)`** cuts a `Rect` (`[i64; 4]`) out of PNG bytes and answers a `Crop`: the crop as PNG bytes, its size and the whole picture's size.
- **`write_snip(dir, stem, picture, rect, record)`** writes a snip as two files, `<stem>.png` (the crop) and `<stem>.json` (a `SnipRecord`: when it was taken, the open map, the armed tool, the note, the rectangle in window and in picture pixels, the camera, the ground point under the pointer and the placements inside the rectangle). It never overwrites an earlier snip, and a bad rectangle writes nothing. The DarkFactory keeps its snips in `<project>/target/loop/captures/snips/`.
- **`read_snips(dir)`** lists the snips in a folder, newest first, and names any file that is not a snip record instead of hiding the rest.
- **`write_pending`, `read_pending`, `clear_pending` and `clear_all_pending`** record a confirmed snip before its picture lands (the editor reads the picture back a frame or more after the confirm), so a read in between sees a snip being saved and not nothing.

The crate depends on `image`, `serde` and `serde_json`, and on no engine.

## Bevy compatibility

None: this crate has no Bevy dependency, so every version works with any Bevy.

## Learn more

- API reference: [docs.rs/dark_factory_snip](https://docs.rs/dark_factory_snip).
- How a person makes a snip and an agent reads it: [working with an agent](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/agents.md), and the MCP tools `blueprint_snip` and `blueprint_list_snips` in [the MCP tool reference](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/mcp-tools.md).
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_blueprint.html) says how they fit together.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-APACHE).
