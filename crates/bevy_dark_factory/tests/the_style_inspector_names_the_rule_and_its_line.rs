//! **The style inspector names the rule that styled a node, and the line that wrote it** (`crate::style_inspect`).
//!
//! An agent (or a person with `Cmd+Alt+I`) points at the editor's own chrome and is told the classes there, each
//! style sheet rule that matches in cascade order, and the file and line each was written on, so the edit that
//! changes the look goes to the right line. The line is checked against the sheet's own text, read here, not
//! against the inspector's bookkeeping.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_dark_factory::style_inspect::{StyleInspector, StyleInspectorRoot};
use serde_json::{Value, json};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (0.0, 0.0)).build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    for _ in 0..8 {
        app.update();
    }
    app
}

fn call(app: &mut App, method: &str, params: Value) -> Result<Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => panic!("no method `{method}` is registered"),
    };
    app.world_mut().run_system_with(id, Some(params)).unwrap_or_else(|e| panic!("{method}: {e:?}")).map_err(|e| format!("{e:?}"))
}

/// The line `selector`'s rule stands on in the shipped sheet, from 1, read from the sheet's own text.
fn line_of(selector: &str) -> usize {
    let quoted = format!("\"{selector}\":");
    include_str!("../src/style/editor.yaml")
        .lines()
        .position(|line| line.trim_start().starts_with(&quoted))
        .map(|at| at + 1)
        .unwrap_or_else(|| panic!("the shipped sheet has no `{selector}` rule"))
}

/// A point inside a classed panel's own padding, below its top edge, where none of its children stands: surface pixels.
fn a_point_on_a_panel(app: &mut App) -> Vec2 {
    let world = app.world_mut();
    let mut panels = world.query::<(&bevy_gooey::StyleClass, &ComputedNode, &UiGlobalTransform)>();
    let (node, at) = panels
        .iter(world)
        .find(|(class, node, _)| class.has("chrome-panel") && node.size().min_element() > 0.0)
        .map(|(_, node, at)| (*node, *at))
        .expect("the editor draws a panel");
    let top = at.translation.y - node.size().y / 2.0;
    // Past the border and well inside the padding: a quarter of the panel's padding down, which is never a child.
    Vec2::new(at.translation.x, top + node.border.min_inset.y + node.padding.min_inset.y / 4.0)
}

#[test]
fn a_point_on_a_panel_names_the_panel_rule_its_file_and_its_line() {
    let mut app = editor("inspect_panel");
    let at = a_point_on_a_panel(&mut app);
    let answer = call(&mut app, "blueprint/ui_style_inspect", json!({"x": at.x, "y": at.y})).unwrap_or_else(|e| panic!("{e}"));
    let classes: Vec<&str> = answer["classes"].as_array().expect("classes").iter().filter_map(Value::as_str).collect();
    assert!(classes.contains(&"chrome-panel"), "the node at the panel's padding is the panel: {answer}");
    let rule = answer["rules"]
        .as_array()
        .expect("rules")
        .iter()
        .find(|rule| rule["selector"] == ".chrome-panel")
        .unwrap_or_else(|| panic!("the panel's rule is named: {answer}"));
    assert_eq!(rule["line"], json!(line_of(".chrome-panel")), "the line the rule was written on: {rule}");
    assert!(rule["layer"].as_str().is_some_and(|layer| layer.ends_with("editor.yaml")), "the editor's own sheet wrote it: {rule}");
    // **The rule's inset, not its gap** (#650): `Role::Panel` supplies the panel's row gap and corner, so
    // the sheet states only its margin and its stated inset override. Asking about `padding` keeps this
    // an assertion that the rule the inspector names is the one that decides a drawn property.
    assert_eq!(rule["sets"]["padding"], answer["resolved"]["padding"], "and nothing later overrides its inset: {answer}");
}

#[test]
fn a_point_with_no_classed_node_is_refused_saying_so() {
    let mut app = editor("inspect_nothing");
    let refused = call(&mut app, "blueprint/ui_style_inspect", json!({"x": -50.0, "y": -50.0})).expect_err("nothing is there");
    assert!(refused.contains("no node with a style class"), "the refusal says why: {refused}");
    let half = call(&mut app, "blueprint/ui_style_inspect", json!({"x": 10.0})).expect_err("one coordinate is not a point");
    assert!(half.contains("both x and y"), "{half}");
}

#[test]
fn the_command_shows_the_overlay_and_shows_it_away() {
    let mut app = editor("inspect_toggle");
    let overlays = |app: &mut App| app.world_mut().query_filtered::<Entity, With<StyleInspectorRoot>>().iter(app.world()).count();
    assert_eq!(overlays(&mut app), 0, "the inspector starts hidden");
    call(&mut app, "blueprint/run_command", json!({"id": "view.inspect_style"})).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    assert!(app.world().resource::<StyleInspector>().on, "Cmd+Alt+I turned it on");
    assert_eq!(overlays(&mut app), 1, "and its overlay is drawn");
    call(&mut app, "blueprint/run_command", json!({"id": "view.inspect_style"})).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    assert_eq!(overlays(&mut app), 0, "the same command puts it away");
}
