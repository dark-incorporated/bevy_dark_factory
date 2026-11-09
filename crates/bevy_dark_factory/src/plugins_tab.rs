//! **The Plugins door: the family crates as Bevy plugins, read by reflection.**
//!
//! The right panel lists every crate the editor can host ([`crate::family_host::KNOWN`]), each with the
//! switch that lists it in the project's `family.ron`. Switching one on also writes
//! `family/<crate>/<Type>.ron` for each of its settings types that has no file yet, from the running
//! resource, so the files exist to edit. The left panel shows the selected crate in three tabs, all read
//! from the type registry and the schedules, so a crate that meets the machine plate needs no code here:
//!
//! - **Settings**: one section per settings resource the crate registers, gooey's reflect inspector over
//!   it. An edit is the operation `family_set` on the field's file, so it has an undo step and a log line;
//!   a refused edit puts the file's settings back on the resource.
//! - **Components**: every component the crate registers, with how many entities carry it now, and an
//!   *Add to selection* button for the authoring ones (a default and serde) that dispatches
//!   `placement_set_component` with the type's default.
//! - **Systems**: every system of the crate in every schedule, by short name, with the sets it is in.

use std::any::TypeId;

use bevy::ecs::reflect::ReflectComponent;
use bevy::ecs::schedule::NodeId;
use bevy::ecs::spawn::SpawnIter;
use bevy::prelude::*;
use bevy::reflect::serde::TypedReflectSerializer;
use bevy_gooey::{Activate, ValueChange, button, label, list, observe, section, tab_page, tabs, toggle};

use crate::editor::{EditorState, TargetLock};
use crate::ops::{self, Ctx, Source};
use crate::project::{OpenMap, Project};
use crate::tiles::{Mode, PluginsRoot};

/// How many crate rows the list shows before it scrolls.
const CRATE_ROWS: f32 = 10.5;

/// What the door shows: the selected crate, and what the panes were last built from.
#[derive(Resource, Default, Debug)]
pub struct PluginsDoor {
    /// The crate the left panel shows.
    pub selected: Option<String>,
    /// Bumped by a write the panes must show (a switch, an edit), so they are built again.
    pub writes: u32,
    built: Option<BuiltFrom>,
}

/// What the panes were built from: the selected crate, the write count, the project root, and the
/// project's `family.ron` as stat'd (modified time and length), so a hand edit rebuilds the list.
type BuiltFrom = (Option<String>, u32, Option<std::path::PathBuf>, Option<(std::time::SystemTime, u64)>);

/// The right panel's list column.
#[derive(Component)]
pub struct CrateList;

/// The left panel's column, built for the selected crate.
#[derive(Component)]
pub struct CrateDetail;

/// A crate's name button in the list.
#[derive(Component, Clone, Debug)]
pub struct CrateRow(pub String);

/// A settings inspector: which crate and which settings type it shows.
#[derive(Component, Clone, Debug)]
pub struct SettingsInspector {
    pub krate: String,
    pub ident: String,
}

/// An *Add to selection* button: the component's type path.
#[derive(Component, Clone, Debug)]
pub struct AddComponent(pub String);

pub struct PluginsTabPlugin;

impl Plugin for PluginsTabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PluginsDoor>()
            .add_systems(OnEnter(crate::screen::Screen::Editor), spawn_panels.after(crate::chrome::FrameSystems))
            .add_systems(Update, build_panes.run_if(in_state(crate::screen::Screen::Editor)))
            .add_observer(settings_edits_are_operations)
            .add_observer(add_to_selection);
    }
}

fn spawn_panels(mut commands: Commands, frame: Res<crate::chrome::Frame>) {
    crate::chrome::panel_root(&mut commands, &frame, crate::chrome::Side::Left, crate::chrome::CONTROLS_W, true, true, "PLUGIN")
        .insert(PluginsRoot)
        .with_children(|p| {
            crate::chrome::title(p, "INSPECTOR");
            p.spawn((Node { flex_direction: FlexDirection::Column, row_gap: Val::Px(crate::chrome::GAP_TIGHT), ..default() }, CrateDetail));
        });
    crate::chrome::panel_root(&mut commands, &frame, crate::chrome::Side::Right, crate::chrome::LIST_W, true, true, "PLUGINS")
        .insert(PluginsRoot)
        .with_children(|p| {
            crate::chrome::list_heading(p, "PLUGINS");
            p.spawn((Node { flex_direction: FlexDirection::Column, ..default() }, CrateList));
        });
}

/// One row of the Systems tab: a system's short name, its schedule, and the sets it is in.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SystemRow {
    pub schedule: String,
    pub name: String,
    pub sets: Vec<String>,
}

/// **Every system of `krate` in every initialized schedule**: the ones whose full name starts with
/// `<krate>::`, with the named sets each is directly in. A schedule that never ran has no systems yet
/// and is skipped.
pub fn systems_of(world: &World, krate: &str) -> Vec<SystemRow> {
    let prefix = format!("{krate}::");
    let mut out = Vec::new();
    let Some(schedules) = world.get_resource::<Schedules>() else { return out };
    for (label, schedule) in schedules.iter() {
        let Ok(systems) = schedule.systems() else { continue };
        let graph = schedule.graph();
        for (key, system) in systems {
            let name = system.name();
            if !name.to_string().starts_with(&prefix) {
                continue;
            }
            let mut sets: Vec<String> = graph
                .hierarchy()
                .graph()
                .neighbors_directed(NodeId::System(key), bevy::ecs::schedule::graph::Direction::Incoming)
                .filter_map(|node| match node {
                    NodeId::Set(set) => graph.system_sets.get(set).map(|s| format!("{s:?}")),
                    NodeId::System(_) => None,
                })
                .filter(|s| !s.starts_with("AnonymousSet"))
                .collect();
            sets.sort();
            out.push(SystemRow { schedule: format!("{label:?}"), name: name.shortname().to_string(), sets });
        }
    }
    out.sort();
    out
}

/// One row of the Components tab.
#[derive(Clone, Debug, PartialEq)]
pub struct ComponentRow {
    pub type_path: String,
    pub short: String,
    pub entities: usize,
    /// A default and serde: a piece can carry it.
    pub authoring: bool,
}

/// **Every component `krate` registers**, with how many live entities carry it.
pub fn components_of(world: &World, krate: &str) -> Vec<ComponentRow> {
    let registry = world.resource::<AppTypeRegistry>().read();
    let mut out: Vec<ComponentRow> = registry
        .iter()
        .filter(|r| r.type_info().type_path_table().crate_name() == Some(krate) && r.data::<ReflectComponent>().is_some())
        .filter(|r| r.data::<bevy::ecs::reflect::ReflectResource>().is_none())
        .map(|r| {
            let entities = world
                .components()
                .get_id(r.type_id())
                .map(|id| world.archetypes().iter().filter(|a| a.contains(id)).map(|a| a.len() as usize).sum())
                .unwrap_or(0);
            ComponentRow {
                type_path: r.type_info().type_path().to_owned(),
                short: r.type_info().type_path_table().short_path().to_owned(),
                entities,
                authoring: r.data::<ReflectDefault>().is_some() && r.data::<bevy::reflect::ReflectSerialize>().is_some(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.short.cmp(&b.short));
    out
}

/// **The panes, built again** when the door opens on them, the selected crate changes, a write was made,
/// or another project opened. Exclusive: the registry, the schedules and the archetypes are read here.
fn build_panes(world: &mut World) {
    if world.get_resource::<Mode>().is_none_or(|m| *m != Mode::Plugins) {
        return;
    }
    let root = world.get_resource::<Project>().map(|p| p.root.clone());
    let stat = root
        .as_ref()
        .and_then(|r| std::fs::metadata(blueprint_model::family::path_for_project(r)).ok())
        .and_then(|m| Some((m.modified().ok()?, m.len())));
    let key = {
        let door = world.resource::<PluginsDoor>();
        (door.selected.clone(), door.writes, root.clone(), stat)
    };
    if world.resource::<PluginsDoor>().built.as_ref() == Some(&key) {
        return;
    }
    world.resource_mut::<PluginsDoor>().built = Some(key.clone());
    let (selected, _, root, _) = key;
    let built = crate::family_host::built();
    let listed: Result<Vec<String>, String> = match &root {
        Some(root) => blueprint_model::family::Family::load(root).map(|f| f.crates),
        None => Err("no project is open".to_owned()),
    };
    // The list.
    let mut list_cols = world.query_filtered::<Entity, With<CrateList>>();
    let cols: Vec<Entity> = list_cols.iter(world).collect();
    for col in cols {
        world.entity_mut(col).despawn_children();
        // The switch is named by its crate, so a person and a screen reader hear which crate it lists;
        // the button beside it opens the crate's panes. A `family.ron` that does not read offers no
        // switch: nothing is written over it.
        let rows: Vec<_> = crate::family_host::KNOWN
            .iter()
            .filter(|_| listed.is_ok())
            .map(|krate| {
                let on = listed.as_ref().is_ok_and(|l| l.iter().any(|c| c == krate));
                let words = if built.contains(krate) { (*krate).to_owned() } else { format!("{krate} (not built here)") };
                let name = (*krate).to_owned();
                let pick = (*krate).to_owned();
                (
                    Node { flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween, ..default() },
                    children![
                        (toggle(words, on), observe(move |c: On<ValueChange<bool>>, mut commands: Commands| {
                            let (name, on) = (name.clone(), c.value);
                            commands.queue(move |world: &mut World| switch(world, &name, on));
                        })),
                        (
                            button("Open"),
                            bevy_gooey::Tooltip::new(format!("Show {pick}'s settings, components and systems")),
                            CrateRow(pick.clone()),
                            observe(move |_: On<Activate>, mut door: ResMut<PluginsDoor>| door.selected = Some(pick.clone())),
                        ),
                    ],
                )
            })
            .collect();
        world.entity_mut(col).with_children(|c| {
            match &listed {
                Err(why) => {
                    c.spawn(label(format!("family.ron: {why}. Fix the file by hand: nothing is written until it reads.")));
                }
                Ok(_) => {
                    c.spawn(list(CRATE_ROWS, Children::spawn(SpawnIter(rows.into_iter()))));
                }
            }
        });
    }
    // The selected crate.
    let mut detail_cols = world.query_filtered::<Entity, With<CrateDetail>>();
    let cols: Vec<Entity> = detail_cols.iter(world).collect();
    let Some(krate) = selected else {
        for col in cols {
            world.entity_mut(col).despawn_children().with_children(|c| {
                c.spawn(label("Pick a plugin in the list."));
            });
        }
        return;
    };
    let settings: Vec<(String, TypeId)> = {
        let registry = world.resource::<AppTypeRegistry>().read();
        bevy_blueprint::settings::settings_types(&registry, &krate)
            .map(|types| types.into_iter().map(|r| (bevy_blueprint::settings::ident_of(r).to_owned(), r.type_id())).collect())
            .unwrap_or_default()
    };
    let components = components_of(world, &krate);
    let systems = systems_of(world, &krate);
    let selection = selected_piece(world).is_some();
    for col in cols {
        world.entity_mut(col).despawn_children().with_children(|c| {
            c.spawn(label(krate.clone()));
            c.spawn(tabs(
                ["Settings", "Components", "Systems"],
                0,
                children![(tab_page(()), SettingsPage), (tab_page(()), ComponentsPage), (tab_page(()), SystemsPage)],
            ));
        });
    }
    let pages: Vec<(Entity, u8)> = world
        .query::<(Entity, Has<SettingsPage>, Has<ComponentsPage>, Has<SystemsPage>)>()
        .iter(world)
        .filter_map(|(e, s, c, y)| match (s, c, y) {
            (true, _, _) => Some((e, 0)),
            (_, true, _) => Some((e, 1)),
            (_, _, true) => Some((e, 2)),
            _ => None,
        })
        .collect();
    for (page, which) in pages {
        let mut page = world.entity_mut(page);
        match which {
            0 => {
                page.with_children(|p| {
                    if settings.is_empty() {
                        p.spawn(label("This plugin registers no settings."));
                    }
                    for (ident, type_id) in &settings {
                        p.spawn(section(
                            ident.clone(),
                            true,
                            children![(
                                bevy_gooey::inspector(bevy_gooey::InspectTarget::Resource { type_id: *type_id }),
                                SettingsInspector { krate: krate.clone(), ident: ident.clone() },
                            )],
                        ));
                    }
                });
            }
            1 => {
                page.with_children(|p| {
                    if components.is_empty() {
                        p.spawn(label("This plugin registers no components."));
                    }
                    for row in &components {
                        p.spawn(label(format!("{}: {} now", row.short, row.entities)));
                        if row.authoring {
                            let mut add = p.spawn((button("Add to selection"), AddComponent(row.type_path.clone())));
                            if !selection {
                                add.insert((bevy::ui::InteractionDisabled, bevy_gooey::Tooltip::new("Select a piece in the Map door")));
                            }
                        }
                    }
                });
            }
            _ => {
                page.with_children(|p| {
                    if systems.is_empty() {
                        p.spawn(label("This plugin adds no systems that have run yet."));
                    }
                    for row in &systems {
                        let sets = if row.sets.is_empty() { String::new() } else { format!(" in {}", row.sets.join(", ")) };
                        p.spawn(label(format!("{} ({}){sets}", row.name, row.schedule)));
                    }
                });
            }
        }
    }
}

/// The Settings tab's page.
#[derive(Component)]
struct SettingsPage;

/// The Components tab's page.
#[derive(Component)]
struct ComponentsPage;

/// The Systems tab's page.
#[derive(Component)]
struct SystemsPage;

/// **Lists or takes out a crate in `family.ron`**, and on switching it on writes a file for each of its
/// settings types that has none, from the running resource (or the type's default).
pub fn switch(world: &mut World, krate: &str, on: bool) {
    let Some(root) = world.get_resource::<Project>().map(|p| p.root.clone()) else { return };
    let outcome = crate::project_config::write_family(&root, krate, on);
    if let Some(mut reloads) = world.get_resource_mut::<crate::live::EditorReloads>() {
        reloads.decide(vec![crate::project_config::verdict_of(&root)]);
    }
    if let Err(e) = outcome {
        world.resource_mut::<EditorState>().status.problem(e);
        return;
    }
    if on {
        let types: Vec<bevy::reflect::TypeRegistration> = {
            let registry = world.resource::<AppTypeRegistry>().read();
            bevy_blueprint::settings::settings_types(&registry, krate).map(|t| t.into_iter().cloned().collect()).unwrap_or_default()
        };
        for registration in types {
            let ident = bevy_blueprint::settings::ident_of(&registration);
            let path = blueprint_model::family::settings_path(&root, krate, ident);
            if path.exists() {
                continue;
            }
            let written = bevy_blueprint::settings::settings_ron(world, &registration).and_then(|text| {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
                }
                blueprint_model::atomic_file::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
            });
            if let Err(e) = written {
                world.resource_mut::<EditorState>().status.problem(e);
            }
        }
    }
    world.resource_mut::<PluginsDoor>().writes += 1;
}

/// **A settings edit is `family_set` on its file.** The field gooey changed is read back from the
/// resource as RON and written through the operation, so it has an undo step and a log line; a refused
/// edit is said, and the file's settings are put back on the resource.
fn settings_edits_are_operations(change: On<bevy_gooey::InspectorChange>, inspectors: Query<&SettingsInspector>, mut commands: Commands) {
    let Ok(inspector) = inspectors.get(change.entity) else { return };
    let (krate, ident, path) = (inspector.krate.clone(), inspector.ident.clone(), change.path.clone());
    commands.queue(move |world: &mut World| {
        let value = {
            let registry = world.resource::<AppTypeRegistry>().clone();
            let registry = registry.read();
            let Ok(registration) = bevy_blueprint::settings::settings_type(&registry, &krate, &ident) else { return };
            let Some(component) = registration.data::<ReflectComponent>() else { return };
            let Some(id) = world.components().get_id(registration.type_id()) else { return };
            let Some(entity) = world.resource_entities().get(id) else { return };
            let Some(resource) = component.reflect(world.entity(entity)) else { return };
            let Ok(field) = resource.reflect_path(path.as_str()) else { return };
            ron::ser::to_string(&TypedReflectSerializer::new(field, &registry))
        };
        let Ok(value) = value else { return };
        let mut ctx = Ctx { project: world.get_resource::<Project>().cloned(), source: Source::Human, session: ops::session_id(), ..Ctx::default() };
        let outcome = ops::dispatch(&mut ctx, "family_set", &serde_json::json!({"crate": krate, "type": ident, "field": path, "value": value}));
        match outcome {
            Ok(ops::Applied::Done { outcome, operation_id, .. }) => {
                let mut state = world.resource_mut::<EditorState>();
                state.status.note(outcome.summary.clone());
                state.record_logged(operation_id, outcome.summary, false);
            }
            Ok(ops::Applied::Refused(e)) | Err(e) => {
                world.resource_mut::<EditorState>().status.raise(&format!("cannot set {ident}.{path}: "), e.problem());
                let file = world.get_resource::<Project>().map(|p| blueprint_model::family::settings_path(&p.root, &krate, &ident));
                if let Some(text) = file.and_then(|f| std::fs::read_to_string(f).ok()) {
                    let _ = bevy_blueprint::settings::apply_settings_text(world, &krate, &ident, &text);
                }
            }
        }
    });
}

/// The primary selected placement's id, the piece *Add to selection* puts a component on.
fn selected_piece(world: &World) -> Option<String> {
    world.get_resource::<TargetLock>()?.placement_id().map(str::to_owned)
}

/// **Add to selection**: `placement_set_component` with the type's default, on the selected piece.
fn add_to_selection(press: On<Activate>, buttons: Query<&AddComponent>, mut commands: Commands) {
    let Ok(AddComponent(path)) = buttons.get(press.entity) else { return };
    let path = path.clone();
    commands.queue(move |world: &mut World| {
        let Some(id) = selected_piece(world) else {
            world.resource_mut::<EditorState>().status.problem("Select a piece in the Map door first.".to_owned());
            return;
        };
        let value = {
            let registry = world.resource::<AppTypeRegistry>().read();
            bevy_blueprint::components::component_ron(&registry, &path, None)
        };
        let value = match value {
            Ok(v) => v,
            Err(e) => {
                world.resource_mut::<EditorState>().status.problem(e);
                return;
            }
        };
        let open = world.remove_resource::<OpenMap>();
        let mut ctx = Ctx { project: world.get_resource::<Project>().cloned(), open, source: Source::Human, session: ops::session_id(), ..Ctx::default() };
        let outcome = ops::dispatch(&mut ctx, "placement_set_component", &serde_json::json!({"id": id, "component": path, "value": value}));
        if let Some(open) = ctx.open.take() {
            world.insert_resource(open);
        }
        let mut state = world.resource_mut::<EditorState>();
        match outcome {
            Ok(ops::Applied::Done { outcome, operation_id, .. }) => {
                state.status.note(outcome.summary.clone());
                state.record_logged(operation_id, outcome.summary, false);
            }
            Ok(ops::Applied::Refused(e)) | Err(e) => state.status.raise("cannot add the component: ", e.problem()),
        }
    });
}

/// **The door's facts for `factory_state`**: per crate the editor can host, `{crate, enabled, settings,
/// components, systems}`.
pub fn plugins_json(world: &World) -> serde_json::Value {
    let listed: Vec<String> = world
        .get_resource::<Project>()
        .and_then(|p| blueprint_model::family::Family::load(&p.root).ok())
        .map(|f| f.crates)
        .unwrap_or_default();
    let rows: Vec<serde_json::Value> = crate::family_host::KNOWN
        .iter()
        .map(|krate| {
            let settings: Vec<String> = {
                let registry = world.resource::<AppTypeRegistry>().read();
                bevy_blueprint::settings::settings_types(&registry, krate)
                    .map(|t| t.into_iter().map(|r| bevy_blueprint::settings::ident_of(r).to_owned()).collect())
                    .unwrap_or_default()
            };
            serde_json::json!({
                "crate": krate,
                "enabled": listed.iter().any(|c| c == krate),
                "settings": settings,
                "components": components_of(world, krate).into_iter().map(|c| c.type_path).collect::<Vec<_>>(),
                "systems": systems_of(world, krate).len(),
            })
        })
        .collect();
    serde_json::Value::Array(rows)
}
