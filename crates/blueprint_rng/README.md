# blueprint_rng

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

One deterministic random number generator for a whole simulation, for developers whose generators and searches must replay exactly: a seeded ChaCha8 stream plus the unbiased integer and float draws a generator needs. A run reproduces from a single `u64`, on any platform, at any thread count.

[![crates.io](https://img.shields.io/crates/v/blueprint_rng.svg)](https://crates.io/crates/blueprint_rng) [![docs.rs](https://img.shields.io/docsrs/blueprint_rng)](https://docs.rs/blueprint_rng) [![CI](https://github.com/dark-incorporated/bevy_dark_factory/actions/workflows/ci.yml/badge.svg)](https://github.com/dark-incorporated/bevy_dark_factory/actions/workflows/ci.yml) [![licence](https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-blue.svg)](#who-and-licence)

**Status:** usable · MSRV 1.97.1

**Why:** a project with two hand-written generators has two streams that must stay identical forever to keep its replays honest; this is the one place the stream is defined, with its first draws frozen in a test, so a change that moves the bits cannot land by accident.

## Quick start

```sh
cargo add blueprint_rng
```

```rust
use blueprint_rng::{DetRng, seeded};

let mut rng = seeded(0xD06F_00D);
let roll = rng.below(6);        // unbiased, in [0, 6)
let t = rng.unit();             // [0, 1)
let sub = rng.raw_u64();        // a seed for a nested generator

// Same seed, same everything: the property the crate exists for.
let mut again = seeded(0xD06F_00D);
assert_eq!(again.below(6), roll);
```

`seeded` returns a `rand_chacha::ChaCha8Rng`, and `DetRng` is a trait implemented for it with three methods: `raw_u64`, `below` and `unit`. The crate depends on `rand` and `rand_chacha` and on nothing else; a test (`tests/leaf.rs`) fails if the dependency list grows or a source file names an engine.

## Why this is a crate and not four lines in yours

"Four lines in yours" is how you end up with two of them.

The moment a second module needs a random integer, the temptation is to write a small generator next to it, and now the reproducibility claim covering your whole simulation depends on two generators staying identical forever. This crate exists because a quality-diversity search (a search that keeps an archive of varied good solutions) and a world generator in one project were about to do exactly that, and the search's value is that its archive replays bit for bit from its seed.

So there is one type in one place, and any consumer that needs a draw takes this.

## `below` is not `% n`

`raw_u64() % n` is biased toward low values whenever `n` does not divide the source range. From a 64-bit draw the bias is on the order of `n / 2^64`, which you will never measure: at a hundred thousand rolls of a six-sided die it is far below the sampling noise, so the example does not pretend to show it.

The reason to use the range sampler anyway is that it is correct by construction, for any source. The modulo version is only fine because of a property of the generator behind it, so the moment someone draws from something narrower (a 32-bit value, a byte, a cached partial draw) the reasoning has to be redone, and it will not be. `below` removes the question.

`below(0)` is a caller bug, since there is no valid result, so it fails loudly under `debug_assertions` rather than quietly returning `0`.

## What it deliberately does not do

**No entropy.** Nothing here reads the clock, the operating system's generator or a thread id. A generator that can seed itself can produce a run you cannot reproduce, and the point is the opposite.

**No global.** There is no ambient `rng()` to reach for. Draws come from a generator you own and pass, because a shared mutable stream makes the order of draws depend on execution order, which is what determinism must not depend on.

**No distributions beyond these three draws.** Gaussians, shuffles and weighted picks belong to whoever knows what they mean: build them on `raw_u64` and `unit`, and keep the shape of the stream visible at the call site.

## Determinism, and what it is worth

ChaCha8 is a specified algorithm, so `seeded(s)` produces the same bytes on any platform and at any thread count. What that buys you is not randomness quality but that a bug reproduces, a replay replays, and a search archive can be compared for equality rather than for approximate agreement.

The crate's own test `the_stream_is_frozen` (in `src/lib.rs`) pins the first draws of `raw_u64`, `unit` and `below` as literal numbers, and treats a change to those bits as a breaking change. If you adopt the crate, do the same in your project: the value is in the bits not moving.

## Try it

```sh
cargo run -p blueprint_rng --example reproducible
```

Prints two runs from one seed and compares them draw for draw, checks that a neighbouring seed does not agree, and rolls a hundred thousand dice to show `below` is uniform. Terminal only: no window, no GPU. Run it from a clone of the [repository](https://github.com/dark-incorporated/bevy_dark_factory).

## Bevy compatibility

None: this crate has no Bevy dependency, so every version works with any Bevy.

## Learn more

- API reference: [docs.rs/blueprint_rng](https://docs.rs/blueprint_rng).
- The example: [`examples/reproducible.rs`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/blueprint_rng/examples/reproducible.rs).
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_blueprint.html) says how they fit together. `blueprint_model` re-exports this crate as `blueprint_model::rng`.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/blueprint_rng/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/blueprint_rng/LICENSE-APACHE).
