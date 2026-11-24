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

/// **Is `entity` on screen**, by the same rule the panel census uses: visible, and laid out with a box.
fn is_drawn(world: &World, entity: Entity) -> bool {
    let Some(shown) = world.get::<InheritedVisibility>(entity) else { return false };
    let Some(node) = world.get::<ComputedNode>(entity) else { return false };
    shown.get() && node.size().x > 0.0 && node.size().y > 0.0
}

/// **The name a screen reader gets for `entity`**, as gooey published it.
///
/// `access.rs` writes one `AccessibilityNode` per control and fills its `name` from the arm that made it:
/// a field's is its row's `ControlText`, a code editor's is `CodeEditor::label()` — which
/// `bevy_gooey::code_editor(path, text)` built from the path, so it is never empty in this workspace. Reading
/// the published name is asking the same question a screen reader asks, rather than guessing which arm ran.
fn published_name(world: &World, entity: Entity) -> Option<String> {
    // **`AccessibilityNode` is a plain newtype over `accesskit::Node`** in Bevy 0.20 (`bevy_a11y-0.20.0`,
    // `pub struct AccessibilityNode(pub Node)`), so the node is read directly. `label()` is accesskit's own
    // accessor — `(Label, label, set_label, clear_label)` in its property table — and it is what gooey's
    // `wanted.name` becomes. Read through the crates rather than a second notion of "name".
    let node = world.get::<bevy::a11y::AccessibilityNode>(entity)?;
    node.0.label().map(str::to_owned).filter(|name| !name.trim().is_empty())
}

/// **The label gooey reads for a field**, which is its row's `ControlText` — `access.rs`'s
/// `Role::TextField` arm does `facts.field` then `look.control_text(row)`.
///
/// The row is the field's **ancestor**, not its descendant: `text_row_as` spawns the label, then the cell,
/// then the field inside that cell. A sweep that looked only downward would call every gooey field in the
/// app unlabelled — which is why this asks the crate's own question instead of inventing a second one.
fn row_label(world: &World, entity: Entity) -> Option<String> {
    let mut at = entity;
    while let Some(parent) = world.get::<ChildOf>(at).map(|p| p.parent()) {
        if let Some(label) = world
            .get::<Children>(parent)
            .map(|children| children.iter().collect::<Vec<Entity>>())
            .unwrap_or_default()
            .into_iter()
            .filter(|child| world.get::<bevy_gooey::Role>(*child) == Some(&bevy_gooey::Role::ControlText))
            .filter_map(|child| world.get::<Text>(child).map(|text| text.0.trim().to_owned()))
            .find(|words| !words.is_empty())
        {
            return Some(label);
        }
        at = parent;
    }
    None
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
                Role::TextInput
                    | Role::TextField
                    // (checked below: a control that is not laid out is not on screen, so it is not swept)
                    | Role::Button
                    | Role::Toggle
                    | Role::List
                    | Role::Table
                    | Role::Tab
                    // **The code editor is a named control of its own** (#599). It is not a `TextField`:
                    // `bevy_gooey::code_editor(label, text)` builds `CodeEditor::new(label.clone(), &text)` from
                    // the path `files.rs` passes, and `access.rs`'s `Role::CodeEditor` arm names it from that
                    // label — the crate's own accessor, so the guard must ask the crate's own question rather
                    // than look for a drawn sibling label. Checked in the crate's source before concluding the
                    // editor was unlabelled; it was not.
                    | Role::CodeEditor
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
        // **A control is named when a screen reader gets a name, or when it draws one.** A field's name is
        // its row's `ControlText` — drawn, so `drawn_words` finds it; a code editor's is the path its
        // `CodeEditor` was built from — published, so `published_name` finds it. One of the two is the
        // answer to "can a person tell what this is", and requiring the drawn one alone is what would make
        // this guard demand a second label on a control that already has one.
        // **Not drawn is not unlabelled**: a control the pane keeps for a file that is not open is
        // `Display::None`, and a person cannot tab into it. `a_panel_census` states the same rule.
        if !is_drawn(app.world(), *entity) {
            continue;
        }
        let named = published_name(app.world(), *entity).is_some()
            || row_label(app.world(), *entity).is_some()
            || !drawn_words(app.world(), *entity).is_empty();
        if !named {
            unlabelled.push((*entity, *role));
        }
    }
    if !unlabelled.is_empty() {
        // **Say which row of the pane each one is**, taken from the nearest ancestor that draws text, so a
        // failure names the field a person sees rather than an entity id alone.
        for (entity, role) in &unlabelled {
            let mut at = *entity;
            let mut owner = None;
            while let Some(parent) = app.world().get::<ChildOf>(at).map(|p| p.parent()) {
                let words = drawn_words(app.world(), parent);
                if !words.is_empty() {
                    owner = Some(words.join(" | "));
                    break;
                }
                at = parent;
            }
            eprintln!("unlabelled {role:?} {entity:?} sits under {owner:?}");
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
