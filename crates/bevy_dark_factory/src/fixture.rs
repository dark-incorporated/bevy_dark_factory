//! **What a `bevy_lights` piece carries** (#103): the crate's own `Flicker` (a bare light that breathes) or its
//! own `Fire` (a whole fire: flame, pooled light, sparks, smoke), each a component a piece carries under its type
//! path in [`Placed::components`](blueprint_model::map::Placed::components). The editor never defines a second
//! schema: [`Fixture::of`] reads the component the piece names with the crate's own serde.

use bevy::reflect::TypePath;
use bevy_lights::{FIRE_EDGE_LUX, Fire, Flicker, flame_light_offset};
use blueprint_model::map::Placed;

/// A piece's light: a bare breathing light or a whole fire.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fixture {
    Flicker(Flicker),
    Fire(Fire),
}

impl Fixture {
    /// The fixture `piece` carries, `None` for a piece with neither component, or why its value does not read.
    /// A `Fire` wins over a `Flicker` on the same piece, because a fire hangs its own light.
    pub fn of(piece: &Placed) -> Option<Result<Fixture, String>> {
        if let Some(text) = piece.components.get(Fire::type_path()) {
            return Some(ron::from_str::<Fire>(text).map(Fixture::Fire).map_err(|e| format!("`{text}` is not a bevy_lights Fire: {e}")));
        }
        let text = piece.components.get(Flicker::type_path())?;
        Some(ron::from_str::<Flicker>(text).map(Fixture::Flicker).map_err(|e| format!("`{text}` is not a bevy_lights Flicker: {e}")))
    }

    /// The light's mean lumens, the number the light map and the pool budget read: a `Flicker`'s `base`, and for a `Fire` its
    /// own `lumens` or, with none, the crate's size for its kind (the lumens that put [`FIRE_EDGE_LUX`] on the ground at the
    /// edge of the kind's reach, with the light at the flame's middle), the same figure `FirePlugin` hands the pool.
    pub fn lumens(&self) -> f32 {
        match self {
            Fixture::Flicker(f) => f.base,
            Fixture::Fire(fire) => fire.lumens.unwrap_or_else(|| fire.kind.lumens(FIRE_EDGE_LUX, flame_light_offset(fire.kind.height())[1])),
        }
    }
}
