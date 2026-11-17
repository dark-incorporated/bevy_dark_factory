//! **Carving and adding volume, as registered operations** (#101): `volume_edit` and `volume_set`.
//!
//! The volume is `<blueprint_dir>/volume/<map>.yaml`, the engine-free `blueprint_model::volume::Volume`
//! (its edit list, not voxels) as YAML, saved beside the map.
//!
//! # What one call does
//!
//! **`volume_edit` changes one entry, or every entry goes.** `edit` appends it, `at: <index>` replaces
//! the entry standing there, `remove: <index>` takes that entry out, and `clear: true` empties the
//! list; each is one undo, whose inverse is the same operation carrying the file as it was.
//!
//! **`volume_set` replaces the whole list** (#474): `edits` is the list the map should hold, in order,
//! and it is one undo whose inverse is the file as it was. Correcting a 26-entry carve list had two
//! paths before it and both were wrong — append compensating entries (the file becomes a diary) or
//! rewrite the YAML by hand (which the log never saw, so *Undo all* left the broken geometry standing
//! while telling the person their work was reverted).
//!
//! **An index the list has not got is refused**, naming it and the list's length (#474): the old path
//! could only append, so a caller correcting entry 2 had no way to say so and a silent append would
//! have written the change somewhere it did not ask for. **A `volume_set` list with one entry the
//! single-edit path refuses is refused whole**, naming that entry's index.
//!
//! # What one call says
//!
//! **Which entry it changed** (#473): the index it stands at, how many entries the list holds now,
//! the shape's kind and whether it adds or carves, and — for a replace or a removal — what stood
//! there before. The index and the count travel in the log line's `after` too, so an oplog reader can
//! find the entry a change made without re-reading the list. `volume_set` says how many entries the
//! list holds now, and carries the text it wrote.

use blueprint_model::volume::{Edit, Volume};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Shown, Undo};

/// An undo's put-back: the file as it was (`None`: there was none).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutBack {
    pub text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Params {
    pub map: String,
    /// One edit: `{"shape": {"Sphere": {center, radius_m}}, "mode": "Carve", "smooth_m": 0.5}`; shapes are
    /// `Sphere {center, radius_m}`, `Box {center, half_m}` and `Capsule {a, b, radius_m}`; modes `Add` and `Carve`.
    /// Appends it, unless `at` names the entry it replaces.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit: Option<Edit>,
    /// **The entry the `edit` replaces**, counted from 0 in the order the file lists them (#474). An
    /// index the list has not got is refused, naming it, and nothing is written.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<usize>,
    /// **The entry to take out**, counted from 0 (#474). Nothing else may be asked for in the same
    /// call: one call changes one entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove: Option<usize>,
    /// Remove every edit of the map.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub clear: bool,
    /// An undo's put-back. Not a thing a caller sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

impl Params {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["map"],
            "properties": {
                "map": {"type": "string", "description": "The map whose volume is edited."},
                "edit": {"type": "object", "description": "One edit: {\"shape\": {\"Sphere\": {center:[x,y,z], radius_m}} | {\"Box\": {center:[x,y,z], half_m:[x,y,z]}} | {\"Capsule\": {a, b, radius_m}}, \"mode\": \"Add\"|\"Carve\", \"smooth_m\": blend metres, default 0}. Appended, or put in the place `at` names."},
                "at": {"type": "integer", "minimum": 0, "description": "The index of the entry `edit` replaces, counted from 0 in the order the file lists them. An index the list has not got is refused, naming it and the list's length; nothing is written. Without it the edit is appended."},
                "remove": {"type": "integer", "minimum": 0, "description": "The index of the entry to take out, counted from 0. Give it on its own: one call changes one entry, so `remove` with `edit`, `at` or `clear` is refused."},
                "clear": {"type": "boolean", "description": "Remove every edit of the map."},
                "put_back": {"type": "object", "description": "An undo's put-back. Only an undo sends this."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<Params, Refusal> {
        let p: Params = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!(
                "this is not a volume change: {e}. The fields are `map`, `edit`, `at`, `remove` and `clear`."
            ))
        })?;
        // **One call changes one entry.** Two ways of naming an entry in one call would make the
        // answer's `index` ambiguous and would let a caller clear a list and remove from it.
        let asked = [p.edit.is_some() || p.at.is_some(), p.remove.is_some(), p.clear].into_iter().filter(|a| *a).count();
        if p.put_back.is_none() && asked > 1 {
            return Err(Refusal::new(
                "a volume change changes one entry: give `edit` (to append), `at` (to replace what stands there), `remove` (to take one out) or `clear` (to empty the list), not more than one. Nothing was changed.",
            ));
        }
        if p.at.is_some() && p.edit.is_none() {
            return Err(Refusal::new("`at` says which entry `edit` replaces, and no `edit` was given. Nothing was changed."));
        }
        if p.edit.is_none() && p.at.is_none() && p.remove.is_none() && !p.clear && p.put_back.is_none() {
            return Err(Refusal::new("a volume change needs `edit`, `at`, `remove` or `clear`. Nothing was changed."));
        }
        Ok(p)
    }
}

fn path_of(ctx: &Ctx, map: &str) -> Result<std::path::PathBuf, Refusal> {
    let Some(project) = ctx.project.as_ref() else {
        return Err(Refusal::new("no project is open, so there is no volume to edit."));
    };
    Ok(project.blueprint_dir.join("volume").join(format!("{}.yaml", crate::ops::map_name(map)?)))
}

fn read(ctx: &Ctx, map: &str) -> Option<String> {
    std::fs::read_to_string(path_of(ctx, map).ok()?).ok()
}

/// **The saved volume of the open map**, or `None` when it has none.
pub fn volume_of(project: &crate::project::Project, open: &crate::project::OpenMap) -> Option<Volume> {
    let name = open.map_path.file_name()?.to_str()?.strip_suffix(".map.yaml")?;
    volume_named(project, name).ok().flatten()
}

/// **A map's saved volume by its name**, or `None` when it has none — and **a refusal for a name that
/// is not a map name**, so a caller naming a path is told so rather than answered with an empty list.
///
/// **The one reader of the `<blueprint_dir>/volume/<map>.yaml` path**, so a report and an edit cannot
/// read different files for one map name. The name passes [`crate::ops::map_name`], the same guard the
/// edit path uses: without it a report asked for `map: "../../some-file"` read a path outside `volume/`
/// and swallowed the parse failure, which reads as "this map carves nothing" instead of the refusal.
pub fn volume_named(project: &crate::project::Project, map: &str) -> Result<Option<Volume>, Refusal> {
    let name = crate::ops::map_name(map)?;
    Ok(std::fs::read_to_string(project.blueprint_dir.join("volume").join(format!("{name}.yaml"))).ok().and_then(|text| Volume::parse(&text).ok()))
}

/// **What one call did to the list** (#473): which entry it touched, and what stood there.
#[derive(Debug, Clone, PartialEq)]
struct Changed {
    /// The index the change stands at: an append's new last index, a replace's, a removal's.
    index: usize,
    /// How many entries the list holds **after** the change.
    entries: usize,
    /// The edit the change put in the list; `None` for a removal and a clear.
    edit: Option<Edit>,
    /// The edit that stood at `index` before a replace or a removal, so the answer can name what it
    /// corrected. `None` for an append.
    was: Option<Edit>,
    /// **Which kind of change it was**, in the word the answer uses.
    word: &'static str,
}

impl Changed {
    /// **The one line a receipt carries**: `volume m: index 3, Carve Box, 4 entries`; for a replace
    /// `volume m: index 2, Add Capsule replaced Add Box, 3 entries`; for a removal
    /// `volume m: index 1, removed Carve Box, 2 left`.
    ///
    /// **The shape and mode are the schema's own words** (`Sphere`/`Box`/`Capsule`, `Add`/`Carve`), so
    /// an answer naming what stood in an entry reads in the vocabulary the caller already writes.
    fn sentence(&self, map: &str) -> String {
        let of = |e: &Edit| format!("{} {}", e.mode_name(), e.shape_name());
        match (self.word, self.edit.as_ref(), self.was.as_ref()) {
            ("edit", Some(new), _) => format!("volume {map}: index {}, {}, {} entries", self.index, of(new), self.entries),
            ("replaced", Some(new), Some(was)) => {
                format!("volume {map}: index {}, {} replaced {}, {} entries", self.index, of(new), of(was), self.entries)
            }
            ("removed", None, Some(was)) => format!("volume {map}: index {}, removed {}, {} left", self.index, of(was), self.entries),
            // A shape the answer cannot name is a defect in this file, not in a caller's parameters.
            _ => format!("volume {map}: {}, {} entries", self.word, self.entries),
        }
    }

    fn to_json(&self, map: &str) -> serde_json::Value {
        let mut out = json!({"map": map, "index": self.index, "entries": self.entries, "change": self.word});
        if let Some(edit) = &self.edit {
            out["shape"] = json!(edit.shape_name());
            out["mode"] = json!(edit.mode_name());
        }
        if let Some(was) = &self.was {
            out["replaced"] = json!({"shape": was.shape_name(), "mode": was.mode_name()});
        }
        out
    }
}

/// **Add or carve one primitive, replace one, remove one, or clear the map's volume.**
pub struct VolumeEdit;

impl VolumeEdit {
    /// **The change the parameters ask for, applied to `volume` in memory.**
    ///
    /// **The one place the list is edited**, so `plan`, `commit` and a preview cannot disagree about
    /// which entry a call writes to — and so an index the list has not got is refused in one sentence,
    /// before anything is written (#474).
    fn changed(volume: &mut Volume, p: &Params) -> Result<Changed, Refusal> {
        // A put-back is not a change to one entry: the file it carries is the answer.
        if let Some(at) = p.remove {
            let was = volume.remove(at).map_err(|e| Refusal::new(format!("{e}. Nothing was changed.")))?;
            return Ok(Changed { index: at, entries: volume.edits.len(), edit: None, was: Some(was), word: "removed" });
        }
        let Some(edit) = p.edit else {
            // `clear`, or a put-back (handled by the caller): the list is what it is.
            return Ok(Changed { index: 0, entries: volume.edits.len(), edit: None, was: None, word: "cleared" });
        };
        match p.at {
            Some(at) => {
                let was = volume.replace(at, edit).map_err(|e| Refusal::new(format!("{e}. Nothing was changed.")))?;
                Ok(Changed { index: at, entries: volume.edits.len(), edit: Some(edit), was: Some(was), word: "replaced" })
            }
            None => {
                volume.push(edit).map_err(|e| Refusal::new(format!("{e}. Nothing was changed.")))?;
                Ok(Changed { index: volume.edits.len() - 1, entries: volume.edits.len(), edit: Some(edit), was: None, word: "edit" })
            }
        }
    }

    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p = Params::parse(params)?;
        let path = path_of(ctx, &p.map)?;
        let before = std::fs::read_to_string(&path).ok();
        let inverse = json!({"map": p.map, "put_back": {"text": before}});
        let (text, change) = if let Some(back) = &p.put_back {
            // **A put-back is the file**, and the answer is that file's own list: reading it here is
            // what lets a receipt say which entries an undo restored.
            let entries = back.text.as_deref().and_then(|t| Volume::parse(t).ok()).map_or(0, |v| v.edits.len());
            (back.text.clone(), Changed { index: entries.saturating_sub(1), entries, edit: None, was: None, word: "put back" })
        } else if p.clear {
            (None, Changed { index: 0, entries: 0, edit: None, was: None, word: "cleared" })
        } else {
            let mut volume: Volume = match &before {
                Some(t) => Volume::parse(t).map_err(|e| Refusal::new(format!("{} is not a volume: {e}", path.display())))?,
                None => Volume::default(),
            };
            let change = Self::changed(&mut volume, &p)?;
            let t = blueprint_model::yaml::to_string(&volume)
                .map_err(|e| Refusal::new(format!("the volume could not be written: {e}")))?;
            (Some(t), change)
        };
        if write {
            match &text {
                Some(t) => {
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir).map_err(|e| Refusal::new(format!("{} cannot be made: {e}", dir.display())))?;
                    }
                    blueprint_model::atomic_file::write(&path, t)
                        .map_err(|e| Refusal::new(format!("{} refused the change, so nothing was written: {e}", path.display())))?;
                }
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        Ok(Outcome {
            summary: change.sentence(&p.map),
            // **One entry placed, put in another's place, or taken out** is one thing changed; a clear
            // and a put-back are a whole file and count nothing.
            affected: usize::from(matches!(change.word, "edit" | "replaced" | "removed")),
            saved: write,
            inverse: Some(inverse),
            after: Some({
                let mut after = change.to_json(&p.map);
                after["text"] = json!(text);
                after
            }),
        })
    }
}

impl Operation for VolumeEdit {
    fn name(&self) -> &'static str {
        "volume_edit"
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        Shown::MapWhole
    }
    fn writes(&self) -> &'static [&'static str] {
        &["volume"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        Params::parse(params).map(|p| p.put_back.is_some()).unwrap_or(false)
    }
    fn schema(&self) -> serde_json::Value {
        Params::schema()
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let map = after.get("map")?.as_str()?;
        Some(json!({"map": map, "text": read(ctx, map)}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let o = Self::apply(ctx, params, false)?;
        // **The same fields the apply answers with**, read from the same change: a preview that said
        // a different index from its apply's would be a second opinion about where the edit lands.
        let after = o.after.clone().unwrap_or_default();
        let mut said = json!({"map": Params::parse(params)?.map, "edited": o.affected});
        for key in ["index", "entries", "change", "shape", "mode", "replaced"] {
            if let Some(v) = after.get(key) {
                said[key] = v.clone();
            }
        }
        Ok(said)
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::apply(ctx, params, true)
    }
}

/// **The parameters of `volume_set`** (#474): the map, and the whole list it should hold.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetParams {
    pub map: String,
    /// **The list the map's volume should hold**, in the order the file lists them, in `volume_edit`'s
    /// own `edit` objects. Absent only for an undo, whose `put_back` is the file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edits: Option<Vec<Edit>>,
    /// An undo's put-back. Not a thing a caller sends.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub put_back: Option<PutBack>,
}

impl SetParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["map", "edits"],
            "properties": {
                "map": {"type": "string", "description": "The map whose volume is replaced."},
                "edits": {
                    "type": "array",
                    "description": "The list the map's volume should hold, in order, in `volume_edit`'s own edit objects: {\"shape\": {\"Sphere\": {center, radius_m}} | {\"Box\": {center, half_m}} | {\"Capsule\": {a, b, radius_m}}, \"mode\": \"Add\"|\"Carve\", \"smooth_m\": blend metres}. An empty list empties the map. One entry the single-edit path refuses is refused whole, naming its index, and nothing is written.",
                    "items": {"type": "object"}
                },
                "put_back": {"type": "object", "description": "An undo's put-back. Only an undo sends this."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<SetParams, Refusal> {
        let p: SetParams = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!(
                "this is not a whole volume: {e}. The fields are `map`, `edits` and `put_back`."
            ))
        })?;
        if p.put_back.is_none() && p.edits.is_none() {
            return Err(Refusal::new(
                "a whole volume needs `edits`: the list the map should hold. Send an empty list to empty it. Nothing was changed.",
            ));
        }
        Ok(p)
    }
}

/// **The list a `volume_set` will write, or the refusal naming the entry that made it impossible.**
///
/// **The check is [`Volume::parse`]'s own**, walked here over a list that never came from a file: a
/// caller's list is held to exactly the rule the reader holds a hand-written file to, so a `volume_set`
/// cannot put in a file an entry the next read refuses. The index is the one the issue asks for, since
/// a refusal that said only "an entry" would leave an agent hunting through twenty-six of them.
fn checked(edits: Vec<Edit>) -> Result<Volume, Refusal> {
    let volume = Volume { version: blueprint_model::volume::VOLUME_VERSION, edits };
    volume.check().map_err(|why| Refusal::new(format!("volume: {why}. Nothing was changed.")))?;
    Ok(volume)
}

/// **Set a map's whole carve list, in one logged, undoable change** (#474).
///
/// # Why this is not `volume_edit` with a list
///
/// `volume_edit` is **one entry**: append, replace one, remove one, or clear. None of those can say
/// "the list is this now", and correcting a 26-entry carve list had one path left — **rewrite the YAML
/// by hand**, which the operation log never saw. The hand-back a person was shown offered *Undo all*,
/// and it would have left the broken geometry standing while telling them their work was reverted.
///
/// **So the whole list is the change**: one `volume_set` line, whose inverse is the file as it was.
/// The file is written by the same writer the single-edit path uses, so the same list written either
/// way is the same bytes — a `volume_set` is a second way to say a list, not a second file format.
///
/// # What it refuses
///
/// **A list containing one entry the single-edit path refuses is refused whole, naming the index**
/// ([`checked`]). Nothing is written: a `volume_set` that wrote the entries it could and dropped the
/// one it could not would be the silent append #474 exists to stop, at a larger scale.
pub struct VolumeSet;

impl VolumeSet {
    fn apply(ctx: &Ctx, params: &serde_json::Value, write: bool) -> Result<Outcome, Refusal> {
        let p = SetParams::parse(params)?;
        let path = path_of(ctx, &p.map)?;
        let before = std::fs::read_to_string(&path).ok();
        let (text, entries) = if let Some(back) = &p.put_back {
            // **A put-back is the file**, and the answer is that file's own list: reading it here is
            // what lets a receipt say how many entries an undo restored.
            let entries = back.text.as_deref().and_then(|t| Volume::parse(t).ok()).map_or(0, |v| v.edits.len());
            (back.text.clone(), entries)
        } else {
            let volume = checked(p.edits.clone().unwrap_or_default())?;
            let entries = volume.edits.len();
            let text = blueprint_model::yaml::to_string(&volume)
                .map_err(|e| Refusal::new(format!("the volume could not be written: {e}")))?;
            (Some(text), entries)
        };
        if write {
            match &text {
                Some(t) => {
                    if let Some(dir) = path.parent() {
                        std::fs::create_dir_all(dir).map_err(|e| Refusal::new(format!("{} cannot be made: {e}", dir.display())))?;
                    }
                    blueprint_model::atomic_file::write(&path, t)
                        .map_err(|e| Refusal::new(format!("{} refused the change, so nothing was written: {e}", path.display())))?;
                }
                None => {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
        Ok(Outcome {
            summary: format!("volume {}: set to {} entries", p.map, entries),
            // **The entries the list holds now**, the same count the answer says: a whole-list set
            // changes as many things as the list is long, and a receipt that said 1 would be a lie
            // about a 26-entry correction.
            affected: entries,
            saved: write,
            inverse: Some(json!({"map": p.map, "put_back": {"text": before}})),
            after: Some(json!({"map": p.map, "entries": entries, "text": text})),
        })
    }
}

impl Operation for VolumeSet {
    fn name(&self) -> &'static str {
        "volume_set"
    }
    fn shows(&self, _ctx: &Ctx, _params: &serde_json::Value) -> Shown {
        Shown::MapWhole
    }
    fn writes(&self) -> &'static [&'static str] {
        &["volume"]
    }
    fn undo(&self) -> Undo {
        Undo::Inverse
    }
    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        SetParams::parse(params).map(|p| p.put_back.is_some()).unwrap_or(false)
    }
    fn schema(&self) -> serde_json::Value {
        SetParams::schema()
    }
    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let map = after.get("map")?.as_str()?;
        Some(json!({"map": map, "text": read(ctx, map)}))
    }
    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let o = Self::apply(ctx, params, false)?;
        // **The same fields the apply answers with**, read from the same outcome.
        let after = o.after.clone().unwrap_or_default();
        Ok(json!({"map": SetParams::parse(params)?.map, "entries": o.affected, "would_write": after.get("text").cloned()}))
    }
    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        Self::apply(ctx, params, true)
    }
}
