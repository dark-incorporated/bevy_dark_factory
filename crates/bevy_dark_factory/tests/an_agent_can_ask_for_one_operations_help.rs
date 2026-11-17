//! **An agent can ask for one operation's parameters and get nothing else** (#444, a wish from the style study).
//!
//! `blueprint_list_operations` answers with every operation and every schema, which is tens of kilobytes an agent
//! scans to learn what `data_set` takes. `{"operation": "data_set"}` answers with that one operation: each parameter
//! (name, whether it is required, its schema and description) and one example call, well under 2 KB.
//!
//! # What is proved
//!
//! 1. One operation's help is **under 2 KB** for `data_set` and holds **every parameter its schema has**, read from
//!    the registry's own list rather than restated here.
//! 2. It holds **one example** in the shape `blueprint_apply_operation` takes, with every required parameter present,
//!    and the operation accepts its shape (a preview of it is refused for the placeholder's value, not for a parameter
//!    the operation does not take).
//! 3. **Every operation has help**, and its example names each of its required parameters.
//! 4. An operation that is not there is refused naming the closest ones; `operation` with `query` is refused.

use bevy::prelude::*;
use bevy_dark_factory::harness;
use serde_json::{Value, json};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{name}_{}", std::process::id())).descriptor("crate", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
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
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut().run_system_with(id, Some(params)).map_err(|e| format!("{method}: {e:?}"))?.map_err(|e| format!("{method}: {e:?}"))
}

/// **The registry's own parameter names for one operation**, from the whole list: the source the help must agree with.
fn schema_of(whole: &Value, operation: &str) -> Value {
    whole["operations"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["operation"] == operation))
        .unwrap_or_else(|| panic!("the whole list has no `{operation}`"))["parameters"]
        .clone()
}

/// **1. `data_set` comes back under 2 KB with every parameter its schema has.**
#[test]
fn one_operations_help_is_small_and_names_every_parameter() {
    let mut app = editor("help_small");
    let whole = call(&mut app, "blueprint/operation_list", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let whole_bytes = whole.to_string().len();
    assert!(whole_bytes > 10_000, "the whole list is {whole_bytes} bytes; this test means little if it is small");

    let help = call(&mut app, "blueprint/operation_list", json!({"operation": "data_set"})).unwrap_or_else(|e| panic!("{e}"));
    let bytes = help.to_string().len();
    assert!(bytes < 2048, "data_set's help is {bytes} bytes, not under 2 KB: {help}");
    assert_eq!(help["operation"], "data_set", "{help}");
    assert_eq!(help["undo"], "inverse", "how it is undone stays in the help: {help}");

    let schema = schema_of(&whole, "data_set");
    let wanted: Vec<&str> = schema["properties"].as_object().expect("the schema has properties").keys().map(String::as_str).collect();
    assert!(wanted.len() >= 3, "{schema}");
    let parameters = help["parameters"].as_array().unwrap_or_else(|| panic!("the help has a parameter list: {help}"));
    let named: Vec<&str> = parameters.iter().filter_map(|p| p["name"].as_str()).collect();
    for name in &wanted {
        assert!(named.contains(name), "the help leaves out the parameter `{name}`: {help}");
    }
    assert_eq!(named.len(), wanted.len(), "the help names a parameter the schema does not have: {named:?} against {wanted:?}");
    for parameter in parameters {
        let required = schema["required"].as_array().is_some_and(|r| r.iter().any(|n| n == &parameter["name"]));
        assert_eq!(parameter["required"], json!(required), "`required` disagrees with the schema for {parameter}");
        let described = schema["properties"][parameter["name"].as_str().unwrap_or_default()]["description"].clone();
        assert_eq!(parameter["description"], described, "a parameter's description is the schema's own, word for word: {parameter}");
    }
}

/// **2. One example, in the shape `blueprint_apply_operation` takes, that the operation accepts the shape of.**
#[test]
fn the_example_is_one_call_with_every_required_parameter() {
    let mut app = editor("help_example");
    let help = call(&mut app, "blueprint/operation_list", json!({"operation": "data_set"})).unwrap_or_else(|e| panic!("{e}"));
    let example = &help["example"];
    assert_eq!(example["operation"], "data_set", "{help}");
    let parameters = example["parameters"].as_object().unwrap_or_else(|| panic!("the example has parameters: {help}"));
    for required in ["file", "path", "value"] {
        assert!(parameters.contains_key(required), "the example leaves out the required `{required}`: {help}");
    }

    // The shape is right: the operation reads the parameters and refuses the placeholder file, not the shape.
    let previewed = call(&mut app, "blueprint/operation_preview", json!({"operation": "data_set", "parameters": example["parameters"]})).unwrap_or_else(|e| panic!("{e}"));
    let why = previewed["refusal"].as_str().unwrap_or_default();
    assert!(!why.contains("these are not a data edit"), "the example is not a data edit's shape: {previewed}");
}

/// **3. Every operation has help whose example names each required parameter.**
#[test]
fn every_operation_has_help_with_an_example() {
    let mut app = editor("help_every");
    let whole = call(&mut app, "blueprint/operation_list", json!({})).unwrap_or_else(|e| panic!("{e}"));
    let rows = whole["operations"].as_array().expect("operations");
    assert!(rows.len() > 50, "{} operations", rows.len());
    for row in rows {
        let name = row["operation"].as_str().expect("a name");
        let help = call(&mut app, "blueprint/operation_list", json!({"operation": name})).unwrap_or_else(|e| panic!("`{name}` has no help: {e}"));
        assert_eq!(help["operation"], name, "{help}");
        let parameters = help["example"]["parameters"].as_object().unwrap_or_else(|| panic!("`{name}`'s example has no parameters object: {help}"));
        for required in row["parameters"]["required"].as_array().cloned().unwrap_or_default() {
            let required = required.as_str().expect("a name");
            assert!(parameters.contains_key(required), "`{name}`'s example leaves out the required `{required}`: {help}");
        }
        let properties = row["parameters"]["properties"].as_object().map_or(0, serde_json::Map::len);
        assert_eq!(help["parameters"].as_array().map_or(0, Vec::len), properties, "`{name}`'s help does not list every parameter");
    }
}

/// **4. An operation that is not there is refused, naming the closest; `operation` and `query` together are refused.**
#[test]
fn an_unknown_operation_is_refused_naming_the_closest_and_the_two_forms_do_not_mix() {
    let mut app = editor("help_unknown");
    let refused = call(&mut app, "blueprint/operation_list", json!({"operation": "dta_set"})).expect_err("no such operation");
    assert!(refused.contains("dta_set") && refused.contains("data_set"), "the refusal names neither the guess nor the closest operation: {refused}");
    assert!(refused.contains("query"), "the refusal does not point at the search: {refused}");

    let both = call(&mut app, "blueprint/operation_list", json!({"operation": "data_set", "query": "data"})).expect_err("one or the other");
    assert!(both.contains("operation") && both.contains("query"), "{both}");

    let blank = call(&mut app, "blueprint/operation_list", json!({"operation": "  "})).expect_err("a blank name");
    assert!(blank.contains("operation"), "{blank}");
}
