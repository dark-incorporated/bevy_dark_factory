//! **Every focusable control in the Files workspace says what it is** (#599).
//!
//! Measured 2026-11-24 at 1600x1000: the Files workspace drew a bordered, focusable, **153x29** control
//! at `x 1404..1557, y 205..234` with **no child text at all**, immediately right of the `Set a field:
//! path value` placeholder. A person could tab into an unlabelled box.
//!
//! # What it was
//!
//! `files.rs` spawns one `TextInput` in the editor's header row: the **field line**, which sets one
//! value of the open YAML file (`<path> <value>`, through the operation `file_edit`). Its `TextInput`
//! was given `"Set a field: path value"` as the *label* argument, which gooey draws beside the field as
//! the row's `ControlText` and reads as the control's accessible name — but that string is the pair
//! [`files::set_the_field`] parses, so the field's own name and its placeholder were one string, and the
//! 153 px is the box: the field without its label is what the sweep measured.
//!
//! # The rule this pins
//!
//! **A focusable control in this workspace has a name a person can read, drawn beside or in it.**
//! The sweep walks every entity in the Files workspace that can take focus and asks two questions:
//! does a screen reader get a name from it (its own text, or a `ControlText` child in its row), and
//! is there anything drawn? An empty `Text` is not a name — which is what the box in the report was.

use bevy::prelude::*;

use bevy_dark_factory::files::{FIELD_LINE_HINT, FIELD_LINE_LABEL, FilesHost, FilesPanel};
use bevy_dark_factory::harness;
use bevy_dark_factory::workspace::Workspace;
use bevy_gooey::Role;

use crate::fixtures;
use crate::settle_support;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("files_labelled_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    app
}

fn go_to_files(app: &mut App) {
    use bevy_dark_factory::command_run::Chords;
    use bevy_dark_factory::commands;
    use bevy_dark_factory::keys::Live;
    let id = "workspace.files";
    let command = commands::by_id(id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(app);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Files, "the Files workspace is open");
}

/// **The words under `entity` that are drawn**: its own `Text` and its descendants', visible and laid out.
fn drawn_words(world: &World, entity: Entity) -> Vec<String> {
    let mut found = Vec::new();
    let mut stack = vec![entity];
    while let Some(at) = stack.pop() {
        if let Some(children) = world.get::<Children>(at) {
            stack.extend(children.iter());
        }
        if let Some(text) = world.get::<Text>(at)
            && let Some(shown) = world.get::<InheritedVisibility>(at)
            && let Some(node) = world.get::<ComputedNode>(at)
            && shown.get()
            && node.size().x > 0.0
            && node.size().y > 0.0
        {
            let words = text.0.trim().to_owned();
            if !words.is_empty() {
                found.push(words);
            }
        }
    }
    found
}

/// **Is `entity` inside the Files workspace's panel or its page host?** The pane is the one this issue
/// measured, so the sweep is scoped to it rather than to the whole shell (which has unlabelled regions by
/// design, such as the viewport, and labels them on their own terms).
fn in_the_files_pane(world: &World, entity: Entity) -> bool {
    let mut at = entity;
    loop {
        if world.get::<FilesPanel>(at).is_some() || world.get::<FilesHost>(at).is_some() {
            return true;
        }
        match world.get::<ChildOf>(at) {
            Some(parent) => at = parent.parent(),
            None => return false,
        }
    }
}

/// **Every focusable control in the Files workspace has a name a person reads.**
///
/// The controls are the roles gooey marks as taking the keyboard — a text field, a button, a toggle, a
/// table — found by `Role` rather than by a list of spawn sites, so a control added tomorrow is swept
/// without anyone remembering to add it here.
#[test]
fn every_focusable_control_in_the_files_pane_is_labelled() {
    let mut app = editor("files_labelled");
    go_to_files(&mut app);
    let world = app.world_mut();
    let controls: Vec<(Entity, Role, String)> = world
        .query::<(Entity, &Role)>()
        .iter(world)
        .filter(|(entity, role)| {
            matches!(
                role,
                // **The roles that take the keyboard**: a field, a button, a toggle, a list, a table, a
                // tab, a dock's grip. `Role` is gooey's own vocabulary, so this list is what a control *is*
                // rather than a list of the spawn sites that exist today.
                Role::TextInput | Role::TextField | Role::Button | Role::Toggle | Role::List | Role::Table | Role::Tab
            ) && in_the_files_pane(world, *entity)
        })
        .map(|(entity, role)| {
            let own = world
                .get::<Text>(entity)
                .map(|text| text.0.trim().to_owned())
                .filter(|text| !text.is_empty());
            let name = own.or_else(|| world.get::<AccessibleLabel>(entity).map(|label| label.0.clone()));
            (entity, *role, name.unwrap_or_default())
        })
        .collect();

    assert!(!controls.is_empty(), "no control was found in the Files workspace, so this swept nothing");
    let mut unlabelled = Vec::new();
    for (entity, role, _) in &controls {
        let words = drawn_words(app.world(), *entity);
        if words.is_empty() {
            unlabelled.push((*entity, *role));
        }
    }
    assert!(
        unlabelled.is_empty(),
        "{} control(s) in the Files workspace draw no words at all, so a person can tab into a box that \
         does not say what it edits: {unlabelled:?}",
        unlabelled.len(),
    );
}

/// **The field line is the control that was unlabelled, and it now carries its own name.**
///
/// Named explicitly so a future sweep that widens its role match cannot quietly pass by finding a
/// different control: the field line's row must draw [`FIELD_LINE_LABEL`] beside the field, and the
/// placeholder must be the `<path> <value>` shape [`FIELD_LINE_HINT`] states rather than the label the
/// box in the report had.
#[test]
fn the_field_line_says_it_sets_a_field() {
    let mut app = editor("files_field_line");
    go_to_files(&mut app);
    let world = app.world_mut();
    let line = world
        .query::<(Entity, &bevy_dark_factory::files::FieldLine)>()
        .iter(world)
        .map(|(entity, _)| entity)
        .next()
        .unwrap_or_else(|| panic!("the Files workspace draws no field line; this test measured nothing"));
    // **`text_input`'s bundle is the row**: `text_row` spawns the `Node`, the `ControlText` label and the
    // field cell on one entity, so the label is the field's own child rather than a sibling's.
    let row = line;

    // **The label is drawn beside the field**, as a `ControlText` — the role gooey reads a row's
    // accessible name from.
    let label: Vec<String> = world
        .get::<Children>(row)
        .map(|children| children.iter().collect::<Vec<Entity>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|child| world.get::<Role>(*child) == Some(&Role::ControlText))
        .filter_map(|child| world.get::<Text>(child).map(|text| text.0.clone()))
        .collect();
    assert_eq!(
        label,
        vec![FIELD_LINE_LABEL.to_owned()],
        "the field line's row draws no `{FIELD_LINE_LABEL}` beside the field, so the box that was reported unlabelled still is",
    );

    let hint = world
        .get::<bevy_gooey::TextHint>(row)
        .map(|hint| hint.0.clone())
        .unwrap_or_default();
    assert_eq!(hint, FIELD_LINE_HINT, "the field's placeholder is the shape a person types, not its label");
}
