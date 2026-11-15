# bevy_grimoire_anim

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

Clip-weight blending on a skinned rig's `AnimationPlayer`, for Bevy developers who want a walk to turn into a run without the feet sliding: no transitions, and one shared gait phase.

[![crates.io](https://img.shields.io/crates/v/bevy_grimoire_anim.svg)](https://crates.io/crates/bevy_grimoire_anim) [![docs.rs](https://img.shields.io/docsrs/bevy_grimoire_anim)](https://docs.rs/bevy_grimoire_anim) [![CI](https://github.com/dark-incorporated/bevy_dark_factory/actions/workflows/ci.yml/badge.svg)](https://github.com/dark-incorporated/bevy_dark_factory/actions/workflows/ci.yml) [![Bevy 0.20](https://img.shields.io/badge/bevy-0.20-purple.svg)](https://bevyengine.org) [![licence](https://img.shields.io/badge/licence-MIT%20OR%20Apache--2.0-blue.svg)](#who-and-licence)

**Status:** usable · Bevy 0.20 · MSRV 1.97.1

**Why:** Bevy's `AnimationTransitions::play` ends by rewinding the incoming clip to time zero, so cross-fading a mid-stride walk against a run that starts from scratch makes the legs scissor. Here no clip is ever restarted, so the planted foot stays planted through the change.

## Quick start

```sh
cargo add bevy_grimoire_anim
```

Add `PoseBlendPlugin`, give the model that owns the scene a `BlendSource` (the rig's animation graph and slot table, built by `bevy_grimoire_anim::rigs::build` from an entry of `rigs.yaml`), and write target weights each frame. This sketch assumes a rig whose slots are exactly the eight locomotion slots; a rig with more slots needs a weight for each.

```rust,no_run
use bevy::prelude::*;
use bevy_grimoire_anim::blend::locomotion_weights;
use bevy_grimoire_anim::{PoseBlendPlugin, PoseBlendSet, PoseBlender};

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, PoseBlendPlugin))
        .add_systems(Update, walk_forward.before(PoseBlendSet))
        .run();
}

// Ordered before `PoseBlendSet`, the pass that eases the weights and writes them to the `AnimationPlayer`.
fn walk_forward(mut blenders: Query<&mut PoseBlender>) {
    for mut blender in &mut blenders {
        // 1.2 world units per second straight ahead, not aiming: the walk, with the run not yet faded in.
        let weights = locomotion_weights(1.2, 0.0, false);
        if let Err(e) = blender.set_targets(&weights) {
            error!("{e}");
        }
        blender.set_ground_speed(1.2); // advances the shared gait phase
    }
}
```

## The one idea: no transitions

Every clip in a rig's blend set stays resident on the `AnimationPlayer` for the model's whole life and is never restarted. Each frame only two things move: the eased clip weights, and one shared normalised gait phase. Every gait clip is seeked from that phase, so the left foot lands at the same instant in the walk and the run however the weights are mixed.

That also means you must never add `AnimationTransitions` to anything this drives: its `PostUpdate` pass would overwrite the weights.

`blend::locomotion_weights(speed, theta, aiming)` turns a speed and a direction of travel in the character's own frame into one weight per locomotion slot (idle, idle-alert, walk, run, walk and run backwards, strafe left and right). The weights always sum to 1, because every branch multiplies shares that already do.

## Cosmetic by construction

This layer runs on `Update`, never `FixedUpdate`, and writes only the `AnimationPlayer` and its own `PoseBlender` component. A game that hashes its simulation state for replays or search never sees it. Keep the `PoseBlender` on the model's child entity, not on the simulation entity.

## Try it

From a clone of the [repository](https://github.com/dark-incorporated/bevy_dark_factory):

```sh
cargo run -p bevy_grimoire_anim --example blend_weights
```

Prints the whole locomotion weight vector across the speed and heading domain, with no rig, no asset and no window. Watch idle, walk and run hand off while the weights sum to exactly 1.000 the entire way.

## Bevy compatibility

| `bevy_grimoire_anim` | Bevy |
|---|---|
| 0.1 to 0.3 | 0.20 |

## Learn more

- API reference: [docs.rs/bevy_grimoire_anim](https://docs.rs/bevy_grimoire_anim).
- How rigs and their clips are described: `rigs.yaml` in [data files](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/data-files.md); the editor's Animation workspace stages a rig through this same builder and blender.
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_grimoire.html) says how they fit together.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-APACHE).
