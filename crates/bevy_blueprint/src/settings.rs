//! **A project's settings files, applied to a family crate's own resources by reflection.**
//!
//! A family crate that meets the machine plate (`dark-incorporated/.github`, `docs/STYLE.md`)
//! registers each settings resource with `ReflectResource`, `ReflectDefault`, `ReflectSerialize` and
//! `ReflectDeserialize`. A project saves one file per such resource,
//! `<project>/family/<crate>/<TypeIdent>.ron` ([`blueprint_model::family::settings_file`]), and this
//! module is the one path from that text to the resource: the game's live reload
//! ([`crate::live::BlueprintLivePlugin`]) and the Dark Factory both call it, so a file reads the same
//! in both. Nothing here knows a crate: the type registry says which types are settings.

use bevy::ecs::reflect::{ReflectComponent, ReflectResource};
use bevy::prelude::*;
use bevy::reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
use bevy::reflect::{PartialReflect, ReflectDeserialize, ReflectSerialize, TypeRegistration, TypeRegistry};
use serde::de::DeserializeSeed;

/// **The entity a resource lives on.** In Bevy 0.20 a resource is a component on a singleton entity, so
/// a reflected resource is read and written through its `ReflectComponent` on that entity.
fn resource_entity(world: &World, registration: &TypeRegistration) -> Option<Entity> {
    let id = world.components().get_id(registration.type_id())?;
    world.resource_entities().get(id)
}

/// Writes `value` into the resource, or inserts it when the crate's plugin made none.
fn write_resource(world: &mut World, registration: &TypeRegistration, value: &dyn PartialReflect, registry: &TypeRegistry) -> Result<(), String> {
    let Some(component) = registration.data::<ReflectComponent>() else {
        return Err(format!("{} is not a resource", ident_of(registration)));
    };
    // **Insert, never `apply`**: a reflected `apply` merges, so a list the file shortens keeps the
    // running resource's extra elements. The file is the whole value.
    match resource_entity(world, registration) {
        Some(entity) => component.insert(&mut world.entity_mut(entity), value, registry),
        None => component.insert(&mut world.spawn_empty(), value, registry),
    }
    Ok(())
}

/// The settings types `krate` registered in this build: every registration whose crate is `krate` and
/// that has `ReflectResource`, `ReflectDefault`, `ReflectSerialize` and `ReflectDeserialize`, sorted by
/// type name. Two settings types of one crate with the same short name would share one file, so that is
/// refused with the crate and the name.
pub fn settings_types<'r>(registry: &'r TypeRegistry, krate: &str) -> Result<Vec<&'r TypeRegistration>, String> {
    let mut out: Vec<&TypeRegistration> = registry
        .iter()
        .filter(|r| r.type_info().type_path_table().crate_name() == Some(krate))
        .filter(|r| is_settings(r))
        .collect();
    out.sort_by_key(|r| r.type_info().type_path_table().ident().unwrap_or_default());
    for pair in out.windows(2) {
        let (a, b) = (ident_of(pair[0]), ident_of(pair[1]));
        if a == b {
            return Err(format!("{krate} registers two settings types named {a}"));
        }
    }
    Ok(out)
}

/// Whether a registration is a settings resource: a resource with a default and serde.
pub fn is_settings(r: &TypeRegistration) -> bool {
    r.data::<ReflectResource>().is_some() && r.data::<ReflectDefault>().is_some() && r.data::<ReflectSerialize>().is_some() && r.data::<ReflectDeserialize>().is_some()
}

/// The short name a settings file is named for (`GooeySkin`).
pub fn ident_of(r: &TypeRegistration) -> &'static str {
    r.type_info().type_path_table().ident().unwrap_or("")
}

/// The settings type `krate` registered as `ident`, or the refusal that names both.
pub fn settings_type<'r>(registry: &'r TypeRegistry, krate: &str, ident: &str) -> Result<&'r TypeRegistration, String> {
    settings_types(registry, krate)?
        .into_iter()
        .find(|r| ident_of(r) == ident)
        .ok_or_else(|| format!("{krate} has no settings type {ident} in this build"))
}

/// **Why a settings file did not read**: the field it is wrong at, when reading got that far, and RON's
/// own error with its line and column.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsError {
    /// `family/<crate>/<ident>.ron`.
    pub file: String,
    /// The path to the field the text is wrong at (`clock.date.month`), or `None`.
    pub field: Option<String>,
    pub reason: String,
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.field {
            Some(field) => write!(f, "{} ({field}): {}", self.file, self.reason),
            None => write!(f, "{}: {}", self.file, self.reason),
        }
    }
}

/// Reads `text` as `krate`'s settings type `ident`, without touching the world: what a caller checks a
/// file with before it applies it. The type's own serde reads it (`ReflectDeserialize`), so its
/// `#[serde(default)]` holds and a partial file is the default with those fields changed.
pub fn read_settings(registry: &TypeRegistry, krate: &str, ident: &str, text: &str) -> Result<Box<dyn PartialReflect>, SettingsError> {
    let file = blueprint_model::family::settings_file(krate, ident);
    let refuse = |field: Option<String>, reason: String| SettingsError { file: file.clone(), field, reason };
    let registration = settings_type(registry, krate, ident).map_err(|e| refuse(None, e))?;
    let mut de = ron::de::Deserializer::from_str(text).map_err(|e| refuse(None, e.to_string()))?;
    let mut track = serde_path_to_error::Track::new();
    let read = TypedReflectDeserializer::new(registration, registry).deserialize(serde_path_to_error::Deserializer::new(&mut de, &mut track));
    let value = match read {
        Ok(v) => v,
        Err(e) => {
            let path = track.path().to_string().replace(".?", "");
            let field = (path != "." && !path.is_empty()).then_some(path);
            return Err(refuse(field, e.to_string()));
        }
    };
    de.end().map_err(|e| refuse(None, e.to_string()))?;
    Ok(value)
}

/// **Applies a settings file's text to the running resource**, inserting the resource if the crate's
/// plugin did not. A file that does not read is refused and the resource is left as it was.
pub fn apply_settings_text(world: &mut World, krate: &str, ident: &str, text: &str) -> Result<(), String> {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let value = read_settings(&registry, krate, ident, text).map_err(|e| e.to_string())?;
    let registration = settings_type(&registry, krate, ident)?;
    write_resource(world, registration, value.as_ref(), &registry)
}

/// **Puts the resource back to its type's default**: what a deleted settings file means.
pub fn reset_settings(world: &mut World, krate: &str, ident: &str) -> Result<(), String> {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let registration = settings_type(&registry, krate, ident)?;
    let Some(default) = registration.data::<ReflectDefault>() else {
        return Err(format!("{krate}'s {ident} has no default"));
    };
    let value = default.default();
    write_resource(world, registration, value.as_partial_reflect(), &registry)
}

/// **The resource as a settings file's text**: pretty RON of what is running now, or of the type's
/// default when the resource is absent. What the Dark Factory writes when a crate is switched on.
pub fn settings_ron(world: &World, registration: &TypeRegistration) -> Result<String, String> {
    let registry = world.resource::<AppTypeRegistry>().read();
    let ident = ident_of(registration);
    let held;
    let running = resource_entity(world, registration).and_then(|e| registration.data::<ReflectComponent>()?.reflect(world.entity(e)));
    let value: &dyn PartialReflect = match running {
        Some(r) => r.as_partial_reflect(),
        None => {
            let Some(default) = registration.data::<ReflectDefault>() else { return Err(format!("{ident} has no default")) };
            held = default.default();
            held.as_partial_reflect()
        }
    };
    ron::ser::to_string_pretty(&TypedReflectSerializer::new(value, &registry), ron::ser::PrettyConfig::default()).map_err(|e| format!("{ident} does not write as RON: {e}"))
}
