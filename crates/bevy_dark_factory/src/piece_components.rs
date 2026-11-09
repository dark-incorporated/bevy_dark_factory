//! **A piece's authoring components, on its entity in the viewport** (#103).
//!
//! A map's [`Placed::components`](blueprint_model::map::Placed::components) names, per piece, each component's
//! type path and its RON. The game's `bevy_blueprint` spawner inserts them by reflection; this module does the
//! same for the editor's own piece entities ([`crate::editor::Placement`]), through the same
//! [`bevy_blueprint::insert_piece_components`], so a torch flickers here because the crate's own `Flicker` is on
//! it, exactly as in the game. A component the build does not register, or a value its type does not read, is a
//! problem on the status line and the piece stands without it.
//!
//! Taking a component off takes off what putting it on added (a `Flicker` adds its `PointLight`): the
//! components the entity gained when it went on are remembered and removed with it.

use std::collections::{BTreeMap, HashMap};

use bevy::ecs::component::ComponentId;
use bevy::ecs::reflect::ReflectComponent;
use bevy::ecs::system::SystemState;
use bevy::prelude::*;

use crate::editor::{EditorState, Placement};
use crate::project::OpenMap;

/// What the editor last put on a piece entity from the map: each component path with its RON, and the
/// components the entity gained when it went on.
#[derive(Component, Default, Debug)]
pub struct AppliedComponents {
    applied: BTreeMap<String, (String, Vec<ComponentId>)>,
}

type Gate<'w, 's> = (Option<Res<'w, OpenMap>>, Query<'w, 's, (), Added<Placement>>);

/// The components each piece's entity is gained or taken off, when the map changed or a piece was spawned.
pub fn sync_piece_components(world: &mut World, mut gate: Local<Option<SystemState<Gate<'static, 'static>>>>) {
    let state = gate.get_or_insert_with(|| SystemState::new(world));
    let wanted: HashMap<String, BTreeMap<String, String>> = {
        let Ok((open, added)) = state.get(world) else { return };
        let Some(open) = open else { return };
        if !open.is_changed() && added.is_empty() {
            return;
        }
        open.map.placements.iter().map(|p| (p.id.clone(), p.components.clone())).collect()
    };
    let pieces: Vec<(Entity, String)> = world.query::<(Entity, &Placement)>().iter(world).map(|(e, p)| (e, p.0.clone())).collect();
    let mut problems = Vec::new();
    for (entity, id) in pieces {
        let want = wanted.get(&id).cloned().unwrap_or_default();
        let have: BTreeMap<String, (String, Vec<ComponentId>)> =
            world.get::<AppliedComponents>(entity).map(|a| a.applied.clone()).unwrap_or_default();
        if have.len() == want.len() && have.iter().all(|(k, (v, _))| want.get(k) == Some(v)) {
            continue;
        }
        let mut kept = BTreeMap::new();
        for (path, (text, gained)) in have {
            if want.get(&path) == Some(&text) {
                kept.insert(path, (text, gained));
            } else {
                take_off(world, entity, &path, &gained);
            }
        }
        for (path, text) in want {
            if kept.contains_key(&path) {
                continue;
            }
            let before = components_of(world, entity);
            let one = BTreeMap::from([(path.clone(), text.clone())]);
            let said = bevy_blueprint::insert_piece_components(world, entity, &id, &one);
            world.flush();
            if said.is_empty() {
                let after = components_of(world, entity);
                let gained = after.into_iter().filter(|c| !before.contains(c)).collect();
                kept.insert(path, (text, gained));
            }
            problems.extend(said);
        }
        if let Ok(mut e) = world.get_entity_mut(entity) {
            e.insert(AppliedComponents { applied: kept });
        }
    }
    if let Some(mut state) = world.get_resource_mut::<EditorState>() {
        for problem in problems {
            state.status.problem(problem);
        }
    }
}

fn components_of(world: &World, entity: Entity) -> Vec<ComponentId> {
    world.get_entity(entity).map(|e| e.archetype().components().to_vec()).unwrap_or_default()
}

/// Takes the component at `path` off `entity`, and every component it brought with it.
fn take_off(world: &mut World, entity: Entity, path: &str, gained: &[ComponentId]) {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let Ok(mut e) = world.get_entity_mut(entity) else { return };
    if let Some(component) = registry.get_with_type_path(path).and_then(|r| r.data::<ReflectComponent>()) {
        component.remove(&mut e);
    }
    for id in gained {
        e.remove_by_id(*id);
    }
}
