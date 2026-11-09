//! **The authoring components a piece carries, inserted by reflection.**
//!
//! A map's [`blueprint_model::map::Placed::components`] holds, per piece, each component's full type path
//! and its RON. A family crate that meets the machine plate registers its authoring components with
//! `ReflectComponent` and serde, so this module needs no crate's types: the registry says what a path is.
//! The game's spawner and the Dark Factory both insert through [`insert_piece_components`], so a piece
//! looks the same in both.

use std::collections::BTreeMap;

use bevy::ecs::reflect::ReflectComponent;
use bevy::prelude::*;
use bevy::reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
use bevy::reflect::{PartialReflect, TypeRegistry};
use serde::de::DeserializeSeed;

/// **A component a piece names that could not be put on it.** The piece still spawns without it; the
/// text says which piece, which component and why, for a host to show where a person looks.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct SpawnProblem(pub String);

/// Whether `path` names a component this build registered with `ReflectComponent`.
pub fn is_registered_component(registry: &TypeRegistry, path: &str) -> bool {
    registry.get_with_type_path(path).is_some_and(|r| r.data::<ReflectComponent>().is_some())
}

/// Reads `text` as the component type at `path`. The refusal names the path and RON's error.
pub fn read_component(registry: &TypeRegistry, path: &str, text: &str) -> Result<Box<dyn PartialReflect>, String> {
    let Some(registration) = registry.get_with_type_path(path).filter(|r| r.data::<ReflectComponent>().is_some()) else {
        return Err(format!("no registered component {path}"));
    };
    let mut de = ron::de::Deserializer::from_str(text).map_err(|e| format!("{path}: {e}"))?;
    let value = TypedReflectDeserializer::new(registration, registry).deserialize(&mut de).map_err(|e| format!("{path}: {e}"))?;
    de.end().map_err(|e| format!("{path}: {e}"))?;
    Ok(value)
}

/// The RON a component of type `path` is saved as: its type's default when `value` is `None`.
pub fn component_ron(registry: &TypeRegistry, path: &str, value: Option<&dyn PartialReflect>) -> Result<String, String> {
    let Some(registration) = registry.get_with_type_path(path) else { return Err(format!("no registered component {path}")) };
    let held;
    let value = match value {
        Some(v) => v,
        None => {
            let Some(default) = registration.data::<ReflectDefault>() else { return Err(format!("{path} has no default")) };
            held = default.default();
            held.as_partial_reflect()
        }
    };
    ron::ser::to_string(&TypedReflectSerializer::new(value, registry)).map_err(|e| format!("{path} does not write as RON: {e}"))
}

/// **Puts each named component on `entity`.** A path this build does not register, or text its type does
/// not read, is skipped and returned as a problem naming `piece`; the rest are inserted. Each problem is
/// also logged and, when the app has the message, written as a [`SpawnProblem`].
pub fn insert_piece_components(world: &mut World, entity: Entity, piece: &str, components: &BTreeMap<String, String>) -> Vec<String> {
    if components.is_empty() {
        return Vec::new();
    }
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let mut problems = Vec::new();
    for (path, text) in components {
        let read = if is_registered_component(&registry, path) {
            read_component(&registry, path, text).map_err(|e| format!("piece {piece}: {e}"))
        } else {
            Err(format!("piece {piece}: component {path} is not registered in this build"))
        };
        match read {
            Ok(value) => {
                let Ok(mut target) = world.get_entity_mut(entity) else { return problems };
                if let Some(component) = registry.get_with_type_path(path).and_then(|r| r.data::<ReflectComponent>()) {
                    component.insert(&mut target, value.as_ref(), &registry);
                }
            }
            Err(problem) => problems.push(problem),
        }
    }
    drop(registry);
    for problem in &problems {
        warn!("blueprint: {problem}");
        if world.contains_resource::<Messages<SpawnProblem>>() {
            world.write_message(SpawnProblem(problem.clone()));
        }
    }
    problems
}

/// [`insert_piece_components`] as a command, for a spawner holding `Commands`.
pub fn queue_piece_components(commands: &mut Commands, entity: Entity, piece: String, components: BTreeMap<String, String>) {
    if components.is_empty() {
        return;
    }
    commands.queue(move |world: &mut World| {
        insert_piece_components(world, entity, &piece, &components);
    });
}
