//! **A family crate's settings, and a piece's components, as registered operations** (#96, #103):
//! `family_set` and `placement_set_component`.
//!
//! A crate's settings are its own reflected resources, one RON file each in
//! `<project>/family/<crate>/<Type>.ron`. The editor holds no schema for them: `family_set` rewrites one
//! dot-pathed field's value text (`blueprint_model::family::set_field`), keeps the rest of the file as
//! the author left it, and reads the result as the crate's own type through the type registry before it
//! writes. Its inverse is the same operation with the old value, so a person's Enter and an agent's
//! `apply` are one change with one log line and one undo. `placement_set_component` puts one of a crate's
//! registered authoring components on a piece of the map (`Placed::components`) the same way.

use blueprint_model::family;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Undo};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    /// The family crate whose settings file is written, as named in `family.ron`.
    #[serde(rename = "crate")]
    pub krate: String,
    /// The settings type, its short name: the file is `family/<crate>/<type>.ron`.
    #[serde(rename = "type")]
    pub ty: String,
    /// Dot-separated field names inside the file, `date.month`; empty for the whole value.
    pub field: String,
    /// One RON value: `6.5`, `"dusk"`, `(1.0, 2.0)`.
    pub value: String,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["crate", "type", "field", "value"],
            "properties": {
                "crate": {"type": "string", "description": "The family crate whose settings are written, as listed in family.ron."},
                "type": {"type": "string", "description": "The settings type's short name; the file is family/<crate>/<type>.ron, e.g. WorldClock."},
                "field": {"type": "string", "description": "Dot-separated field names inside that file, e.g. date.month; empty to replace the whole value."},
                "value": {"type": "string", "description": "The new value as one RON value, e.g. 6.5 or \"dusk\"."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not a family setting: {e}. The fields are `crate`, `type`, `field` and `value`, and nothing else."))
        })
    }
}

/// `<root>/family/<krate>/<ident>.ron`, when the project lists `krate` and both names are one path
/// segment each.
pub(crate) fn settings_path(ctx: &Ctx, krate: &str, ident: &str) -> Result<std::path::PathBuf, Refusal> {
    let Some(root) = ctx.root_or_project() else {
        return Err(Refusal::new("no project is open, so there are no family settings to change."));
    };
    // A crate or a type name is one path segment: a file name, never a route out of `family/`.
    for name in [krate, ident] {
        if name.is_empty() || name.contains(['/', '\\', '.']) {
            return Err(Refusal::new(format!("`{name}` is not a family crate or type name. Nothing was written.")));
        }
    }
    let listed = family::Family::load(&root).map_err(Refusal::new)?;
    if !listed.crates.iter().any(|c| c == krate) {
        return Err(Refusal::new(format!("{krate} is not listed in family.ron, so its settings were not changed.")));
    }
    Ok(family::settings_path(&root, krate, ident))
}

fn read(ctx: &Ctx, krate: &str, ident: &str) -> Option<String> {
    std::fs::read_to_string(settings_path(ctx, krate, ident).ok()?).ok()
}

/// **Whether `text` is a value of `krate`'s type `ident`**: a settings type this build registers is read
/// with it by reflection, so a typo in a field name is refused here and not discovered by the game; the
/// crates' data files keep their own checks; any other file is held to being well-formed RON.
fn reads_as(krate: &str, ident: &str, text: &str) -> Result<(), Refusal> {
    if let Some(registry) = crate::type_registry::get() {
        let registry = registry.read();
        if bevy_blueprint::settings::settings_type(&registry, krate, ident).is_ok() {
            return bevy_blueprint::settings::read_settings(&registry, krate, ident, text)
                .map(drop)
                .map_err(|e| Refusal::new(format!("{e}. Nothing was written.")));
        }
    }
    check_script(krate, ident, text)?;
    check_society(krate, ident, text)?;
    ron::from_str::<ron::Value>(text).map(drop).map_err(|e| Refusal::new(format!("that is not RON, so nothing was written: {e}")))
}

/// **Set one field of a family crate's settings file.**
pub struct FamilySet;

impl FamilySet {
    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Option<Outcome>, Refusal> {
        let p = Params::parse(params)?;
        let path = settings_path(ctx, &p.krate, &p.ty)?;
        let file = family::settings_file(&p.krate, &p.ty);
        let text = std::fs::read_to_string(&path).map_err(|_| {
            Refusal::new(format!("{file} does not exist; switch {} on in the Plugins door to write its defaults", p.krate))
        })?;
        let (new, old) = if p.field.is_empty() {
            (format!("{}\n", p.value.trim()), text.clone())
        } else {
            family::set_field(&text, &p.field, &p.value).map_err(Refusal::new)?
        };
        reads_as(&p.krate, &p.ty, &new)?;
        if !write {
            return Ok(None);
        }
        blueprint_model::atomic_file::write(&path, &new)
            .map_err(|e| Refusal::new(format!("{} refused the change, so nothing was written: {e}", path.display())))?;
        let shown = if p.field.is_empty() { p.ty.clone() } else { format!("{}.{}", p.ty, p.field) };
        Ok(Some(Outcome {
            summary: format!("{}: {shown} = {}", p.krate, p.value.trim()),
            affected: 1,
            saved: true,
            inverse: Some(json!({"crate": p.krate, "type": p.ty, "field": p.field, "value": old})),
            after: Some(json!({"crate": p.krate, "type": p.ty, "file": new})),
        }))
    }
}

impl Operation for FamilySet {
    fn name(&self) -> &'static str {
        "family_set"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["family"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn schema(&self) -> serde_json::Value {
        Params::schema()
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let krate = after.get("crate")?.as_str()?;
        let ty = after.get("type")?.as_str()?;
        Some(json!({"crate": krate, "type": ty, "file": read(ctx, krate, ty)?}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        Self::apply(ctx, params, false)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::apply(ctx, params, true)?.ok_or_else(|| Refusal::new("the change was not written."))
    }
}

// ---------------------------------------------------------------- placement_set_component

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PieceParams {
    /// The placement's id, as the map file names it.
    pub id: String,
    /// The component's full type path, `bevy_lights::flicker::Flicker`.
    pub component: String,
    /// The component as one RON value, `(base: 900.0, phase: 1.3, depth: 0.15)`. Absent takes it off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl PieceParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["id", "component"],
            "properties": {
                "id": {"type": "string", "description": "The placement's id, as the map file names it."},
                "component": {"type": "string", "description": "The component's full type path, as factory_state's plugins list it, e.g. bevy_lights::flicker::Flicker."},
                "value": {"type": "string", "description": "The component as one RON value, e.g. (base: 900.0, phase: 1.3, depth: 0.15). Leave it out to take the component off the piece."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<PieceParams, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not a piece's component: {e}. The fields are `id`, `component` and `value`, and nothing else."))
        })
    }
}

fn component_of(open: &crate::project::OpenMap, id: &str, component: &str) -> serde_json::Value {
    let value = open.map.placements.iter().find(|p| p.id == id).and_then(|p| p.components.get(component));
    json!({"map": open.map.name, "id": id, "component": component, "value": value})
}

/// **Put one registered component on a placement, or take it off.**
pub struct PlacementSetComponent;

impl PlacementSetComponent {
    fn check(ctx: &Ctx, p: &PieceParams) -> Result<(), Refusal> {
        let Some(open) = ctx.open.as_ref() else {
            return Err(Refusal::new("no map is open, so there are no placements to set."));
        };
        let Some(piece) = open.map.placements.iter().find(|q| q.id == p.id) else {
            return Err(Refusal::new(format!("no placement is called `{}` on this map, so nothing was set.", p.id)));
        };
        let Some(v) = &p.value else {
            // Taking off a component the piece carries is always allowed, registered or not, so a stale
            // entry from a build with another crate can be cleaned up.
            if piece.components.contains_key(&p.component) {
                return Ok(());
            }
            return Err(Refusal::new(format!("{} carries no {}, so nothing was taken off.", p.id, p.component)));
        };
        let Some(registry) = crate::type_registry::get() else {
            return Err(Refusal::new("the editor's type registry is not ready, so no component can be read."));
        };
        let registry = registry.read();
        if !bevy_blueprint::components::is_registered_component(&registry, &p.component) {
            return Err(Refusal::new(format!("no registered component {}", p.component)));
        }
        bevy_blueprint::components::read_component(&registry, &p.component, v).map_err(|e| Refusal::new(format!("{e}. Nothing was set.")))?;
        check_conversation(ctx, p)
    }
}

/// **A piece that starts a conversation names one the project's `DialogueScript` defines** (#106).
#[cfg(feature = "bevy_speech_bubbles")]
fn check_conversation(ctx: &Ctx, p: &PieceParams) -> Result<(), Refusal> {
    use bevy::reflect::TypePath;
    let (true, Some(v)) = (p.component == bevy_speech_bubbles::StartsConversation::type_path(), p.value.as_deref()) else {
        return Ok(());
    };
    let bevy_speech_bubbles::StartsConversation(name) =
        ron::from_str(v).map_err(|e| Refusal::new(format!("`{v}` is not a conversation (\"hello\") : {e}. Nothing was set.")))?;
    let script = settings_path(ctx, "bevy_speech_bubbles", "DialogueScript")?;
    let text = std::fs::read_to_string(&script)
        .map_err(|e| Refusal::new(format!("{} cannot be read, so no conversation can be attached: {e}", script.display())))?;
    let defined: bevy_speech_bubbles::DialogueScript =
        ron::from_str(&text).map_err(|e| Refusal::new(format!("the project's dialogue script is not readable: {e}")))?;
    if defined.conversations.contains_key(&name) {
        return Ok(());
    }
    let known: Vec<&str> = defined.conversations.keys().map(String::as_str).collect();
    Err(Refusal::new(format!("the dialogue script has no conversation `{name}` (it has: {}). Nothing was set.", known.join(", "))))
}

#[cfg(not(feature = "bevy_speech_bubbles"))]
fn check_conversation(_ctx: &Ctx, _p: &PieceParams) -> Result<(), Refusal> {
    Ok(())
}

impl Operation for PlacementSetComponent {
    fn name(&self) -> &'static str {
        "placement_set_component"
    }
    fn writes(&self) -> &'static [&'static str] {
        &["map"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn schema(&self) -> serde_json::Value {
        PieceParams::schema()
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let id = after.get("id")?.as_str()?;
        let component = after.get("component")?.as_str()?;
        Some(component_of(ctx.open.as_ref()?, id, component))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = PieceParams::parse(params)?;
        Self::check(ctx, &p)?;
        Ok(params.clone())
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = PieceParams::parse(params)?;
        Self::check(ctx, &p)?;
        let Some(mut open) = ctx.open.take() else { return Err(Refusal::new("no map is open.")) };
        let Some(index) = open.map.placements.iter().position(|q| q.id == p.id) else {
            ctx.open = Some(open);
            return Err(Refusal::new(format!("no placement is called `{}` on this map, so nothing was set.", p.id)));
        };
        let old = open.map.placements[index].components.get(&p.component).cloned();
        let put = |open: &mut crate::project::OpenMap, v: &Option<String>| match v {
            Some(v) => {
                open.map.placements[index].components.insert(p.component.clone(), v.clone());
            }
            None => {
                open.map.placements[index].components.remove(&p.component);
            }
        };
        put(&mut open, &p.value);
        let Some(project) = ctx.project.clone() else {
            put(&mut open, &old);
            ctx.open = Some(open);
            return Err(Refusal::new("no project is open."));
        };
        if let Err(e) = open.save(&project) {
            put(&mut open, &old);
            ctx.open = Some(open);
            return Err(Refusal::new(format!("the file refused the change, so nothing was written: {e}")));
        }
        let short = p.component.rsplit("::").next().unwrap_or(&p.component).to_owned();
        let summary = match &p.value {
            Some(v) => format!("{}: {short} = {}", p.id, v.trim()),
            None => format!("{}: took {short} off", p.id),
        };
        let after = Some(component_of(&open, &p.id, &p.component));
        ctx.open = Some(open);
        Ok(Outcome {
            summary,
            affected: 1,
            saved: true,
            inverse: Some(json!({"id": p.id, "component": p.component, "value": old})),
            after,
        })
    }
}

/// **A society definition that `SocietyDefs` refuses to compile is never written** (#104): the saved file is
/// the crate's own `DefsFile`, so the crate's own compile step decides, and its message is the refusal.
#[cfg(feature = "bevy_society")]
fn check_society(krate: &str, ident: &str, text: &str) -> Result<(), Refusal> {
    if krate != "bevy_society" || ident != "DefsFile" {
        return Ok(());
    }
    bevy_society::SocietyDefs::from_ron(text).map(|_| ()).map_err(|e| Refusal::new(format!("{e}. Nothing was written.")))
}

#[cfg(not(feature = "bevy_society"))]
fn check_society(_krate: &str, _ident: &str, _text: &str) -> Result<(), Refusal> {
    Ok(())
}

/// **A dialogue script that `validate_script` refuses is never written** (#106): the saved file is the
/// crate's own `DialogueScript`, so the crate's own validation decides, and its message is the refusal.
#[cfg(feature = "bevy_speech_bubbles")]
fn check_script(krate: &str, ident: &str, text: &str) -> Result<(), Refusal> {
    if krate != "bevy_speech_bubbles" || ident != "DialogueScript" {
        return Ok(());
    }
    let script: bevy_speech_bubbles::DialogueScript =
        ron::from_str(text).map_err(|e| Refusal::new(format!("that is not a dialogue script, so nothing was written: {e}")))?;
    bevy_speech_bubbles::validate_script(&script).map_err(|e| Refusal::new(format!("{e}. Nothing was written.")))
}

#[cfg(not(feature = "bevy_speech_bubbles"))]
fn check_script(_krate: &str, _ident: &str, _text: &str) -> Result<(), Refusal> {
    Ok(())
}
