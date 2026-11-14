//! **One operation's help, and nothing else** (#444, a wish from the style study).
//!
//! `grimoire/operation_list` answers with every operation and every schema, which is tens of kilobytes. An agent that wants
//! to know what `data_set` takes had to scan all of it. `{"operation": "data_set"}` answers with [`help`] for that one
//! operation: each parameter and one example call, which for most operations is a few hundred bytes.
//!
//! # Where the words come from
//!
//! **The registry's own schema, and nothing written beside it.** A parameter's description, its type and its limits are the
//! schema's, word for word, so the help cannot say something the list does not. The example is **built from the schema's required
//! parameters** (a default, the first value of an enum, a placeholder in angle brackets for a name or a path): it shows the shape
//! the operation reads, in the form `grimoire_apply_operation` takes it, and its placeholders are the agent's to fill.

use serde_json::{Map, Value, json};

use crate::ops::{self, Operation};

/// **How deep an example descends** into nested objects and arrays before it stops with a placeholder.
const SAMPLE_DEPTH: usize = 4;

/// **The edit distance up to which an unknown name has a closest operation**: a typo or two, not another word.
const CLOSE: usize = 2;

/// **How many closest operations a refusal names.**
const CLOSEST_SHOWN: usize = 3;

/// **The help for one operation**: its name, how it is undone, each parameter (name, whether it is required, its description and
/// the rest of its schema) and one example call.
pub fn help(op: &dyn Operation) -> Value {
    let schema = op.schema();
    let required: Vec<&str> = schema
        .get("required")
        .and_then(Value::as_array)
        .map(|names| names.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    let mut parameters = Vec::new();
    let mut example = Map::new();
    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        for (name, property) in properties {
            let mut row = Map::new();
            row.insert("name".to_owned(), json!(name));
            row.insert("required".to_owned(), json!(required.contains(&name.as_str())));
            let mut rest = property.as_object().cloned().unwrap_or_default();
            if let Some(description) = rest.remove("description") {
                row.insert("description".to_owned(), description);
            }
            if !rest.is_empty() {
                row.insert("schema".to_owned(), Value::Object(rest));
            }
            parameters.push(Value::Object(row));
            if required.contains(&name.as_str()) {
                example.insert(name.clone(), sample(name, property, SAMPLE_DEPTH));
            }
        }
    }
    let mut out = Map::new();
    out.insert("operation".to_owned(), json!(op.name()));
    out.insert(
        "undo".to_owned(),
        json!(match op.undo() {
            ops::Undo::None => "none",
            ops::Undo::Inverse => "inverse",
            ops::Undo::Snapshot => "snapshot",
        }),
    );
    if let Some(description) = schema.get("description") {
        out.insert("description".to_owned(), description.clone());
    }
    out.insert("parameters".to_owned(), Value::Array(parameters));
    out.insert("example".to_owned(), json!({"operation": op.name(), "parameters": Value::Object(example)}));
    Value::Object(out)
}

/// **The refusal for a name that is not an operation**, naming the closest ones (a typo or two away) and the search.
pub fn unknown(name: &str) -> String {
    let mut near: Vec<(usize, &'static str)> = ops::registry()
        .iter()
        .map(|op| op.name())
        .map(|candidate| (distance(name, candidate), candidate))
        .filter(|(d, _)| *d <= CLOSE)
        .collect();
    near.sort();
    let closest = if near.is_empty() {
        String::new()
    } else {
        let named: Vec<String> = near.iter().take(CLOSEST_SHOWN).map(|(_, n)| format!("`{n}`")).collect();
        format!("; the closest is {}", named.join(", "))
    };
    format!("`{name}` is not an operation this editor knows{closest}. Ask with `query` to search the operations by words.")
}

/// **A value of the shape a schema describes**, for an example. A default, a constant or the first value of an enum when the schema
/// gives one; otherwise by type, with `<name>` standing for a string the caller must choose.
fn sample(name: &str, schema: &Value, depth: usize) -> Value {
    if let Some(constant) = schema.get("const") {
        return constant.clone();
    }
    if let Some(default) = schema.get("default").filter(|d| !d.is_null()) {
        return default.clone();
    }
    if let Some(first) = schema.get("enum").and_then(Value::as_array).and_then(|values| values.iter().find(|v| !v.is_null())) {
        return first.clone();
    }
    if let Some(first) = ["oneOf", "anyOf"].iter().find_map(|key| schema.get(key).and_then(Value::as_array).and_then(|alternatives| alternatives.first())) {
        return sample(name, first, depth);
    }
    let kind = match schema.get("type") {
        Some(Value::String(kind)) => Some(kind.as_str()),
        Some(Value::Array(kinds)) => kinds.iter().filter_map(Value::as_str).find(|k| *k != "null"),
        _ => None,
    };
    match kind {
        Some("integer" | "number") => {
            let low = schema.get("minimum").and_then(Value::as_i64).unwrap_or(1);
            json!(schema.get("maximum").and_then(Value::as_i64).map_or(low, |high| low.min(high)))
        }
        Some("boolean") => json!(false),
        Some("array") if depth > 0 => {
            let wanted = schema.get("minItems").and_then(Value::as_u64).map_or(1, |n| n.min(4)) as usize;
            let items = schema.get("items").cloned().unwrap_or_else(|| json!({}));
            Value::Array((0..wanted).map(|_| sample(name, &items, depth - 1)).collect())
        }
        Some("array") => json!([]),
        Some("object") if depth > 0 => {
            let required: Vec<&str> = schema
                .get("required")
                .and_then(Value::as_array)
                .map(|names| names.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            let mut fields = Map::new();
            if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
                for (field, property) in properties.iter().filter(|(field, _)| required.contains(&field.as_str())) {
                    fields.insert(field.clone(), sample(field, property, depth - 1));
                }
            }
            Value::Object(fields)
        }
        Some("object") => json!({}),
        _ => json!(format!("<{name}>")),
    }
}

/// **The edit distance between two names** (insert, delete or replace one character), which is what a typo is.
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let above = row[j + 1];
            row[j + 1] = (row[j + 1] + 1).min(row[j] + 1).min(diagonal + usize::from(ca != cb));
            diagonal = above;
        }
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_typo_is_one_or_two_edits_from_its_operation() {
        assert_eq!(distance("dta_set", "data_set"), 1);
        assert_eq!(distance("data_set", "data_set"), 0);
        assert_eq!(distance("", "ab"), 2);
        assert!(distance("placement", "data_set") > CLOSE);
    }

    #[test]
    fn an_example_is_built_from_the_required_parameters_only() {
        let schema = json!({
            "type": "object",
            "required": ["id", "at", "mode", "count", "on"],
            "properties": {
                "id": {"type": "string"},
                "at": {"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2},
                "mode": {"type": "string", "enum": ["a", "b"]},
                "count": {"type": "integer", "minimum": 3},
                "on": {"type": "boolean", "default": true},
                "other": {"type": "string"}
            }
        });
        let required = ["id", "at", "mode", "count", "on"];
        let built: Map<String, Value> = schema["properties"]
            .as_object()
            .map(|p| p.iter().filter(|(k, _)| required.contains(&k.as_str())).map(|(k, v)| (k.clone(), sample(k, v, SAMPLE_DEPTH))).collect())
            .unwrap_or_default();
        assert_eq!(Value::Object(built), json!({"id": "<id>", "at": [1, 1], "mode": "a", "count": 3, "on": true}));
    }
}
