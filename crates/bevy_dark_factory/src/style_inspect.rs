//! **The style inspector**: the editor's devtools for its own chrome.
//!
//! [`Action::InspectStyle`](crate::keys::Action::InspectStyle) (`view.inspect_style`) toggles an overlay that
//! names the topmost classed node under the pointer ([`crate::view::Pointer`], never the window's cursor, so
//! an agent's pointer is inspected the same way): its classes, each style sheet rule that matches it in
//! cascade order with the layer and the line that wrote it, and what the rules add up to.
//!
//! A click on a rule opens its file at that line in the Files workspace when the file is the project's
//! (`editor_style.yaml`); a rule in another layer (the checkout's `src/style/editor.yaml`, a theme, the
//! person's `style.yaml`) is outside the project the file editor holds, so its `path:line` goes to the
//! clipboard instead. `blueprint/ui_style_inspect` answers the same question for an agent, at a point it
//! names.
//!
//! While the pointer is over the overlay itself, the overlay keeps showing what it showed, so a rule can be
//! reached and clicked.

use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use serde::Serialize;

use crate::chrome;
use crate::style::StyleLines;

/// **Whether the inspector is up**, and what it last drew.
#[derive(Resource, Default)]
pub struct StyleInspector {
    /// Shown or not; the action flips it.
    pub on: bool,
    /// What the overlay shows now, so it is rebuilt only when the answer changes.
    shown: Option<Result<Inspection, String>>,
}

/// The overlay's root.
#[derive(Component)]
pub struct StyleInspectorRoot;

/// One rule row of the overlay: a click opens or copies where the rule was written.
#[derive(Component, Clone, Debug)]
pub struct InspectorRuleRow(pub InspectedRule);

/// **One rule that styles the node**, as the inspector and `blueprint/ui_style_inspect` report it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InspectedRule {
    /// The selector as written, `.chrome-panel`.
    pub selector: String,
    /// The layer it was read from: `style/editor.yaml`, a theme's file, the person's `style.yaml`, the project's.
    pub layer: String,
    /// The layer's file on disk, when it has one (a compiled layer has none).
    pub path: Option<PathBuf>,
    /// The line its selector stands on, from 1.
    pub line: Option<usize>,
    /// The specificity it matched with: a higher one wins a property, a tie goes to the later rule.
    pub specificity: u32,
    /// What it declares, as the sheet's own keys.
    pub sets: serde_json::Value,
}

/// **Why a node looks the way the style sheet makes it.**
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Inspection {
    /// The entity, `123v4`.
    pub entity: String,
    /// Its `Name`, when it has one.
    pub name: Option<String>,
    /// Its classes.
    pub classes: Vec<String>,
    /// Its laid-out size, logical px.
    pub size_px: [f32; 2],
    /// The rules that match it, lowest specificity first: the last one that sets a property wins it.
    pub rules: Vec<InspectedRule>,
    /// Every matching declaration laid over the last.
    pub resolved: serde_json::Value,
}

/// **The topmost classed node under `point`** (surface pixels, [`crate::view::Pointer`]'s), the inspector's own
/// overlay excepted. Topmost is the one drawn last: the later in Bevy's back-to-front [`bevy::ui::UiStack`].
pub fn classed_node_at(world: &mut World, point: Vec2) -> Option<Entity> {
    let mut nodes = world.query_filtered::<(Entity, &ComputedNode, &UiGlobalTransform, &InheritedVisibility), (
        With<bevy_gooey::StyleClass>,
        Without<StyleInspectorRoot>,
    )>();
    let under: Vec<Entity> =
        nodes.iter(world).filter(|(_, node, at, shown)| shown.get() && node.contains_point(**at, point)).map(|(entity, ..)| entity).collect();
    let stack = world.get_resource::<bevy::ui::UiStack>()?;
    stack.uinodes.iter().rev().find(|entity| under.contains(entity)).copied()
}

/// **What the style sheet does to `entity`**, with the line each rule was written on; refused with the reason
/// when it has no class or there is no sheet.
pub fn inspect(world: &World, entity: Entity) -> Result<Inspection, String> {
    let explained = bevy_gooey::explain(world, entity).ok_or_else(|| format!("{entity} has no style class, or no style sheet is running"))?;
    let lines = world.get_resource::<StyleLines>();
    let mut rules = Vec::with_capacity(explained.matched.len());
    for rule in explained.matched {
        let line = lines.and_then(|l| l.lines.get(&(rule.source.clone(), rule.selector.clone())).copied());
        let path = lines.and_then(|l| l.paths.get(&rule.source).cloned());
        rules.push(InspectedRule {
            sets: declared(&rule.decl)?,
            selector: rule.selector,
            layer: rule.source,
            path,
            line,
            specificity: rule.specificity,
        });
    }
    let size_px = world.get::<ComputedNode>(entity).map_or([0.0, 0.0], |node| (node.size() * node.inverse_scale_factor()).to_array());
    Ok(Inspection {
        entity: entity.to_string(),
        name: world.get::<Name>(entity).map(|name| name.as_str().to_owned()),
        classes: explained.classes,
        size_px,
        rules,
        resolved: declared(&explained.resolved)?,
    })
}

/// **What a pointer at `point` inspects**: the topmost classed node there, or why there is nothing.
pub fn inspect_at(world: &mut World, point: Vec2) -> Result<Inspection, String> {
    let entity = classed_node_at(world, point).ok_or_else(|| format!("no node with a style class is under ({:.0}, {:.0})", point.x, point.y))?;
    inspect(world, entity)
}

/// A declaration as the sheet's own keys, the way a person would write it back.
fn declared(decl: &bevy_gooey::StyleDecl) -> Result<serde_json::Value, String> {
    serde_json::to_value(decl).map_err(|e| format!("a declaration did not serialise: {e}"))
}

/// **Flip the inspector** — the action's whole effect.
pub fn toggle_the_inspector(keyboard: Res<ButtonInput<KeyCode>>, live: Res<crate::keys::Live>, mut inspector: ResMut<StyleInspector>) {
    if !crate::keys::just_pressed(&keyboard, *live, crate::keys::Action::InspectStyle) {
        return;
    }
    // **The key dispatches the operation an agent calls** (E-002, #129), the shape `notice::toggle_journal` set: up or away is
    // the operation's answer, read from the inspector and written back after it.
    let mut ctx = crate::ops::Ctx {
        source: crate::ops::Source::Human,
        session: crate::ops::session_id(),
        view: Some(crate::ops_view::View { style_inspector_open: inspector.on, ..Default::default() }),
        ..crate::ops::Ctx::default()
    };
    let outcome = crate::ops::dispatch(&mut ctx, "view_toggle_style_inspector", &serde_json::json!({}));
    let _ = outcome;
    let on = ctx.view.map_or(inspector.on, |view| view.style_inspector_open);
    if inspector.on != on {
        inspector.on = on;
    }
}

/// **Keep the overlay showing the node under the pointer**, rebuilt only when the answer changes. The pointer over
/// the overlay keeps the last answer, so a rule row can be clicked.
pub fn draw_the_inspector(world: &mut World) {
    let on = world.get_resource::<StyleInspector>().is_some_and(|inspector| inspector.on);
    let root = world.query_filtered::<Entity, With<StyleInspectorRoot>>().iter(world).next();
    if !on {
        if let Some(root) = root {
            world.entity_mut(root).despawn();
        }
        if let Some(mut inspector) = world.get_resource_mut::<StyleInspector>() {
            inspector.shown = None;
        }
        return;
    }
    let pointer = world.get_resource::<crate::view::Pointer>().and_then(|pointer| pointer.0);
    let over_overlay = match (root, pointer) {
        (Some(root), Some(at)) => {
            let laid = world.get::<ComputedNode>(root).zip(world.get::<UiGlobalTransform>(root));
            laid.is_some_and(|(node, transform)| node.contains_point(*transform, at))
        }
        _ => false,
    };
    if over_overlay && root.is_some() {
        return;
    }
    let answer = match pointer {
        Some(at) => inspect_at(world, at),
        None => Err("the pointer is not over the window".to_owned()),
    };
    let unchanged = world.get_resource::<StyleInspector>().is_some_and(|inspector| inspector.shown.as_ref() == Some(&answer));
    if unchanged && root.is_some() {
        return;
    }
    if let Some(root) = root {
        world.entity_mut(root).despawn();
    }
    spawn_the_overlay(world, &answer);
    if let Some(mut inspector) = world.get_resource_mut::<StyleInspector>() {
        inspector.shown = Some(answer);
    }
}

/// The overlay for one answer: a heading, the node's classes and size, a row per rule, and what they add up to.
fn spawn_the_overlay(world: &mut World, answer: &Result<Inspection, String>) {
    let ink = chrome::ink();
    let inset = chrome::space(chrome::STEP_PAD);
    let line = |text: String, colour: Color| (Text::new(text), TextColor(colour), chrome::typed(chrome::text::HINT));
    let mut root = world.spawn((
        StyleInspectorRoot,
        Name::new("style inspector"),
        bevy_gooey::placed(UiRect { top: Val::Px(inset), right: Val::Px(inset), ..UiRect::all(Val::Auto) }),
        bevy_gooey::RowLayout { direction: Some(FlexDirection::Column), ..default() },
        bevy_gooey::StyleClass::of("style-inspect-panel"),
        GlobalZIndex(crate::style::metrics().z.report),
        BackgroundColor(ink.overlay_bg),
        bevy_gooey::InkFollow,
    ));
    root.with_children(|panel| {
        panel.spawn(line("STYLE".to_owned(), ink.label));
        let inspection = match answer {
            Ok(inspection) => inspection,
            Err(why) => {
                panel.spawn(line(why.clone(), ink.dim));
                return;
            }
        };
        let name = inspection.name.as_deref().map_or_else(String::new, |name| format!(" {name}"));
        panel.spawn(line(
            format!("{}{name}  .{}  {:.0} x {:.0} px", inspection.entity, inspection.classes.join("."), inspection.size_px[0], inspection.size_px[1]),
            ink.text,
        ));
        if inspection.rules.is_empty() {
            panel.spawn(line("no rule matches these classes".to_owned(), ink.dim));
        }
        for rule in &inspection.rules {
            let place = match (&rule.path, rule.line) {
                (Some(path), Some(line)) => format!("{}:{line}", path.display()),
                (None, Some(line)) => format!("{}:{line} (compiled)", rule.layer),
                (_, None) => rule.layer.clone(),
            };
            // **A quiet row, off the `Activate` bus** (`chrome::quiet_row_shape`): the inspector opens over every screen,
            // the launcher too, where the editor's global `Activate` observers have no project to read.
            let words = format!("{}  {}  specificity {}  {}", rule.selector, place, rule.specificity, rule.sets);
            panel
                .spawn((crate::chrome::quiet_row_shape(false), InspectorRuleRow(rule.clone()), Pickable::default()))
                .with_children(|row| {
                    row.spawn(line(words, ink.accent));
                })
                .observe(open_the_rule);
        }
        panel.spawn(line(format!("= {}", inspection.resolved), ink.text));
    });
}

/// **A click on a rule row**: open the project's layer at the rule in the Files workspace, or put another layer's
/// `path:line` on the clipboard, and say which in the status line.
fn open_the_rule(click: On<bevy::picking::events::PointerClick>, rows: Query<&InspectorRuleRow>, mut commands: Commands) {
    let Ok(row) = rows.get(click.entity) else { return };
    let rule = row.0.clone();
    commands.queue(move |world: &mut World| {
        let said = match project_relative(world, &rule) {
            Some(rel) => {
                let target = crate::files_panel::Target { rel, line: rule.line.map_or(0, |line| line.saturating_sub(1)), col: 0, len: 0 };
                crate::files_panel::open_at(world, &target);
                None
            }
            None => Some(copy_the_place(&rule)),
        };
        if let (Some(said), Some(mut state)) = (said, world.get_resource_mut::<crate::editor::EditorState>()) {
            match said {
                Ok(place) => state.status.note(format!("COPIED {place}: that layer is outside the project's files")),
                Err(why) => state.status.problem(format!("NOT COPIED: {why}")),
            }
        }
    });
}

/// The rule's file relative to the open project's root, when it is one of the project's.
fn project_relative(world: &World, rule: &InspectedRule) -> Option<String> {
    let root = world.get_resource::<crate::project::Project>().map(|project| project.root.clone())?;
    let path: &Path = rule.path.as_deref()?;
    path.strip_prefix(&root).ok().map(|rel| rel.to_string_lossy().replace('\\', "/"))
}

/// Put the rule's `path:line` on the clipboard, and return it.
fn copy_the_place(rule: &InspectedRule) -> Result<String, String> {
    let file = rule.path.as_ref().map_or_else(|| rule.layer.clone(), |path| path.display().to_string());
    let place = rule.line.map_or_else(|| file.clone(), |line| format!("{file}:{line}"));
    arboard::Clipboard::new().and_then(|mut clipboard| clipboard.set_text(place.clone())).map_err(|e| e.to_string())?;
    Ok(place)
}

/// The inspector's resource, its key and its overlay.
pub struct StyleInspectPlugin;

impl Plugin for StyleInspectPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StyleInspector>().add_systems(Update, (toggle_the_inspector, draw_the_inspector).chain());
    }
}
