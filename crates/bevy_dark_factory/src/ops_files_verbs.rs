// ── file_create, file_rename, file_delete (#507) ─────────────────────────────
//
// **Every edit that is not a text change inside an existing file** used to be a trip out to the file manager: the Files
// workspace could edit a file and replace across files, and could not make, rename, duplicate or delete one. That made the
// agent's reach the same size as the person's — an agent could only `file_edit` a path that already existed.
//
// # Three operations, and the guards that make them safe
//
// `file_create` (a duplicate is one of these, with `copy_from`), `file_rename` and `file_delete`. Each is `Undo::Inverse`
// with the shape `map_create` uses: a create's undo is the file's removal, a delete's undo is the file's text put back
// verbatim (so a delete is undone byte for byte, line endings included), and a rename's undo is the rename back.
//
// The guards are why this is a feature rather than a footgun, and each is one rule with one test:
//
// | guard | refuses | so |
// |---|---|---|
// | the trust gate | a create, rename or delete of a command file or the guide, in [`crate::files_lang::READ_ONLY_NOTE`]'s words | the menu cannot do what `file_edit` forbids |
// | the reference rule | a rename or a delete of a file another file names, naming that file | a moved file leaves no dead name behind |
// | what a map places | a delete of a map that places pieces, naming them | an orphaned placement is a piece that silently fails to appear |
//
// **The reference rule is one function** ([`named_elsewhere`]) read by both verbs, so a rename and a delete cannot disagree
// about what "names this file" means; it reads the same walk the project's own scans use ([`crate::files::listed_files`]).
//
// # The tree follows the operation, not a timer
//
// [`crate::files::tree_files`] is what the tree draws, and a walk of it is the tree's own poll ([`crate::files::TREE_POLL_S`],
// 1.5 s). A create, rename or delete asks for the walk **now** ([`crate::files::note_tree_changed`]), so the row the change
// made is on screen in the next frame rather than a poll later.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::ops::{Ctx, Operation, Outcome, Refusal, Shown, Undo};

/// **The folder an operation's project is rooted at**, or the refusal every file verb gives without one.
fn root_of(ctx: &Ctx) -> Result<std::path::PathBuf, Refusal> {
    ctx.root_or_project().ok_or_else(|| Refusal::new("no project is open, so there is no file to change."))
}

/// The text of `rel` now, or `None` when the file is not there or is not text.
fn current(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

/// **What a replace that has no file to point at names**: the project's own folder, the tree the Files workspace shows.
const PROJECT_FOLDER: &str = ".";

/// **A duplicate's or a rename's new name, with the old extension kept.**
///
/// `a.map.yaml` asked for `b` is `b.map.yaml`; asked for `b` in a file called `readme.md` is `b.md`. A name that carries an
/// extension of its own (`b.txt`) is taken as given, because the person who typed one meant it. A folder has no extension to
/// keep.
///
/// **The extension is everything after the first dot of the name**, so `a.map.yaml` keeps `.map.yaml` rather than only
/// `.yaml`: the two-part suffix is what makes the editor open the file as a map, and dropping half of it turned a map into a
/// file the editor reads as plain YAML.
pub fn keep_extension(old: &str, new: &str, is_dir: bool) -> String {
    let new = new.trim();
    if is_dir || new.is_empty() {
        return new.to_owned();
    }
    let name = old.rsplit('/').next().unwrap_or(old);
    let Some((_, extension)) = name.split_once('.') else { return new.to_owned() };
    // **A name that already ends in the extension is not given it twice**, and a name with any extension of its own is
    // taken as given: `b.md` is the person's answer, not `b.md.md`.
    if new.rsplit('/').next().unwrap_or(new).contains('.') {
        return new.to_owned();
    }
    format!("{new}.{extension}")
}

/// **The project paths under `root` that name `rel`**, other than `rel` itself: what makes a rename or a delete of it a
/// dangling reference.
///
/// One reader for both verbs, so they cannot disagree. A file names another by its **path under the project**, written
/// anywhere in its text (`See notes/keep.yaml`), which is the only spelling a project file can carry that the editor can
/// follow without guessing. A file's own name is not a reference to itself.
pub fn named_elsewhere(root: &Path, rel: &str) -> Vec<String> {
    let mut out = Vec::new();
    for file in crate::files::listed_files(root) {
        if file == rel || crate::files::project_path(root, &file).is_err() {
            continue;
        }
        let full = root.join(&file);
        // **Only text the editor reads**: the same size limit the editor opens with, so a 200 MB binary is not scanned.
        let Ok(meta) = std::fs::metadata(&full) else { continue };
        if meta.len() > crate::files::MAX_FILE_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&full) else { continue };
        if text.contains(rel) {
            out.push(file);
        }
    }
    out
}

/// **The pieces a map's own file places**, read from the map so the refusal can name them: `Ok(None)` when `rel` is not a
/// map file, so a delete of anything else has nothing to ask.
fn pieces_placed_by(root: &Path, rel: &str) -> Option<Vec<String>> {
    if !rel.ends_with(".map.yaml") {
        return None;
    }
    let text = std::fs::read_to_string(root.join(rel)).ok()?;
    let map = blueprint_model::map::Map::parse(&text).ok()?;
    Some(map.placements.iter().map(|p| p.id.clone()).collect())
}

/// **The guards a create, a rename and a delete share**: the path is one an operation may write, and it is not a command
/// file. `Ok(full)` is the path on disk.
fn writable(root: &Path, rel: &str) -> Result<std::path::PathBuf, Refusal> {
    let full = crate::files::project_path(root, rel).map_err(Refusal::new)?;
    if crate::files_lang::is_command_file(rel) {
        return Err(Refusal::new(format!("{rel}: {}", crate::files_lang::READ_ONLY_NOTE)));
    }
    Ok(full)
}

/// **The parent folder of a project path**, made if it is not there: `notes/a.yaml` makes `notes/`.
fn make_parent(root: &Path, rel: &str) -> Result<(), Refusal> {
    let Some(parent) = Path::new(rel).parent().filter(|p| !p.as_os_str().is_empty()) else { return Ok(()) };
    let folder = root.join(parent);
    std::fs::create_dir_all(&folder).map_err(|e| Refusal::new(format!("{} was not made ({e}), so nothing was written.", parent.display())))
}

/// **What [`file_create`] takes.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateParams {
    /// The file's path under the project: `notes/new.yaml`.
    #[serde(default)]
    pub path: String,
    /// What it holds. Empty makes an empty file.
    #[serde(default)]
    pub text: String,
    /// **A duplicate**: another file of the project whose text the new one takes, byte for byte. `text` is then not read,
    /// so one call cannot say two things about what the file holds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub copy_from: Option<String>,
    /// **An undo's form: take this file away.** The inverse of a create; refused from an agent
    /// ([`Operation::only_as_undo`]) and refused while the file has unsaved edits in a tab.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remove: Option<String>,
}

impl CreateParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["path"],
            "properties": {
                "path": {"type": "string", "description": "The file to make, relative to the project's root, with its folder (`notes/new.yaml`). A path already there is refused: a create never overwrites."},
                "text": {"type": "string", "description": "What the new file holds. Default empty."},
                "copy_from": {"type": "string", "description": "Duplicate this file of the project: the new one holds its text byte for byte. `text` is not read then."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<CreateParams, Refusal> {
        let p: CreateParams = serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not a file to make: {e}. The fields are `path`, `text` and `copy_from`, and `path` is required."))
        })?;
        if p.remove.is_some() && !p.path.is_empty() {
            return Err(Refusal::new("`remove` is an undo's form and is never sent with `path`, so one call cannot both make and take away. Nothing was changed."));
        }
        Ok(p)
    }

    /// The path this call acts on: the file to make, or the undo's file to take away.
    fn target(&self) -> &str {
        self.remove.as_deref().unwrap_or(&self.path)
    }
}

/// **Make a file, or duplicate one**: `notes/new.yaml` and its folder, in one log line and one `Cmd+Z`.
///
/// **A create never overwrites** (`save_atomic_no_replace`): a path that is already there is a refusal naming it, so a
/// mistyped name cannot cost somebody a file.
pub struct FileCreate;

impl FileCreate {
    /// What would be written, or why not: every refusal found before anything is written.
    fn resolve(ctx: &Ctx, p: &CreateParams) -> Result<(std::path::PathBuf, String, String), Refusal> {
        let root = root_of(ctx)?;
        let rel = p.target();
        if rel.is_empty() {
            return Err(Refusal::new("no file to make: give `path`, the file's path under the project (`notes/new.yaml`). Nothing was changed."));
        }
        let full = writable(&root, rel)?;
        if p.remove.is_some() {
            if !full.exists() {
                return Err(Refusal::new(format!("{rel} is not there, so there is nothing to take away.")));
            }
            let old = current(&root, rel).unwrap_or_default();
            return Ok((root, old, String::new()));
        }
        if full.exists() {
            return Err(Refusal::new(format!("{rel} is already there; a create never overwrites it. Type another name, or open the one that is there. Nothing was changed.")));
        }
        let text = match &p.copy_from {
            Some(from) => {
                crate::files::project_path(&root, from).map_err(Refusal::new)?;
                std::fs::read_to_string(root.join(from))
                    .map_err(|e| Refusal::new(format!("{from} was not read ({e}), so nothing was made.")))?
            }
            None => p.text.clone(),
        };
        Ok((root, String::new(), text))
    }
}

impl Operation for FileCreate {
    fn name(&self) -> &'static str {
        "file_create"
    }

    fn undo(&self) -> Undo {
        Undo::Inverse
    }

    fn schema(&self) -> serde_json::Value {
        CreateParams::schema()
    }

    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = CreateParams::parse(params)?;
        let (_, _, text) = Self::resolve(ctx, &p)?;
        if p.remove.is_some() {
            return Ok(json!({"summary": format!("take away {}", p.target())}));
        }
        Ok(json!({
            "summary": match &p.copy_from {
                Some(from) => format!("duplicate {from} as {}", p.path),
                None => format!("make {}", p.path),
            },
            "path": p.path,
            "bytes": text.len(),
        }))
    }

    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = CreateParams::parse(params)?;
        let (root, old, text) = Self::resolve(ctx, &p)?;
        let rel = p.target();
        if p.remove.is_some() {
            std::fs::remove_file(root.join(rel)).map_err(|e| Refusal::new(format!("{rel} was not taken away ({e}); nothing was changed.")))?;
            crate::files::note_tree_changed();
            return Ok(Outcome {
                summary: format!("took away {rel}"),
                affected: 1,
                saved: true,
                inverse: Some(json!({"path": rel, "text": old})),
                after: Some(json!({"path": rel, "text": null})),
            });
        }
        make_parent(&root, rel)?;
        blueprint_model::save::save_atomic_no_replace(&root.join(rel), &text)
            .map_err(|e| Refusal::new(format!("{rel} was not written: {e}. Nothing was changed.")))?;
        crate::files::note_tree_changed();
        Ok(Outcome {
            summary: match &p.copy_from {
                Some(from) => format!("made {rel} as a copy of {from}"),
                None => format!("made {rel}"),
            },
            affected: 1,
            saved: true,
            // **The inverse of "this was not there" is taking the new file away.**
            inverse: Some(json!({"remove": rel})),
            after: Some(json!({"path": rel, "text": text})),
        })
    }

    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let root = ctx.root_or_project()?;
        let path = after.get("path")?.as_str()?;
        Some(json!({"path": path, "text": current(&root, path)}))
    }

    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        CreateParams::parse(params).is_ok_and(|p| p.remove.is_some())
    }

    fn writes(&self) -> &'static [&'static str] {
        &["game_data"]
    }

    fn shows(&self, _ctx: &Ctx, params: &serde_json::Value) -> Shown {
        let Ok(p) = CreateParams::parse(params) else { return Shown::File { path: PROJECT_FOLDER.to_owned(), lines: None } };
        Shown::File { path: p.target().to_owned(), lines: None }
    }
}

// ── file_rename ──────────────────────────────────────────────────────────────

/// **What [`file_rename`] takes.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameParams {
    /// The file as it is now, under the project.
    pub path: String,
    /// The new name. **Its extension is kept** unless it carries one of its own ([`keep_extension`]).
    pub to: String,
    /// **An undo's form**: rename `to` back to `path`, and only when it has not moved since.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub back: Option<bool>,
}

impl RenameParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["path", "to"],
            "properties": {
                "path": {"type": "string", "description": "The file as it is now, relative to the project's root."},
                "to": {"type": "string", "description": "The new name, as a path under the project. The old extension is kept when the new name carries none: `a.map.yaml` given `b` is `b.map.yaml`. A file another file of the project names is refused, naming it."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<RenameParams, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not a rename: {e}. The fields are `path` and `to`, and nothing else."))
        })
    }

    /// **The path the new name lands on**: the same folder as `path` when `to` names none, with the extension kept.
    fn destination(&self) -> String {
        let folder = Path::new(&self.path).parent().filter(|p| !p.as_os_str().is_empty());
        let named = self.to.trim();
        let in_folder = if named.contains('/') {
            named.to_owned()
        } else {
            match folder {
                Some(folder) => format!("{}/{}", folder.to_string_lossy().replace('\\', "/"), named),
                None => named.to_owned(),
            }
        };
        keep_extension(&self.path, &in_folder, false)
    }
}

/// **Rename a file, keeping its extension.**
pub struct FileRename;

impl FileRename {
    /// The old path, the new one and what the file holds, or why not.
    fn resolve(ctx: &Ctx, p: &RenameParams) -> Result<(std::path::PathBuf, String, String, String), Refusal> {
        let root = root_of(ctx)?;
        let was = if p.back == Some(true) { p.destination() } else { p.path.clone() };
        let now = if p.back == Some(true) { p.path.clone() } else { p.destination() };
        let old_full = writable(&root, &was)?;
        let new_full = writable(&root, &now)?;
        if !old_full.is_file() {
            return Err(Refusal::new(format!("{was} is not a file of this project, so there is nothing to rename.")));
        }
        if old_full == new_full {
            return Err(Refusal::new(format!("{was} would keep its own name, so nothing was renamed.")));
        }
        if new_full.exists() {
            return Err(Refusal::new(format!("{now} is already there; a rename never overwrites it. Type another name. Nothing was changed.")));
        }
        let text = std::fs::read_to_string(&old_full).map_err(|e| Refusal::new(format!("{was} was not read ({e}), so nothing was renamed.")))?;
        // **The reference rule**: a file another file names is refused, naming it, or the move leaves a dead name behind.
        let named = named_elsewhere(&root, &was);
        if !named.is_empty() {
            let list = named.join(", ");
            return Err(Refusal::new(format!("{was} is named by {list}, so renaming it would leave that name pointing at nothing. Change {list} first, or move the file and leave a copy under the old name. Nothing was changed.")));
        }
        Ok((root, was, now, text))
    }
}

impl Operation for FileRename {
    fn name(&self) -> &'static str {
        "file_rename"
    }

    fn undo(&self) -> Undo {
        Undo::Inverse
    }

    fn schema(&self) -> serde_json::Value {
        RenameParams::schema()
    }

    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = RenameParams::parse(params)?;
        let (_, was, now, text) = Self::resolve(ctx, &p)?;
        Ok(json!({"summary": format!("rename {was} to {now}"), "from": was, "to": now, "bytes": text.len()}))
    }

    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = RenameParams::parse(params)?;
        let (root, was, now, text) = Self::resolve(ctx, &p)?;
        make_parent(&root, &now)?;
        // **A new file and then the old one away**: a rename that stopped half way would leave two names for one text, and
        // a copy whose removal failed is the one state the editor can still put right (the undo removes the copy).
        let full = root.join(&now);
        blueprint_model::save::save_atomic_no_replace(&full, &text)
            .map_err(|e| Refusal::new(format!("{now} was not written ({e}), so nothing was renamed.")))?;
        if let Err(e) = std::fs::remove_file(root.join(&was)) {
            let _ = std::fs::remove_file(&full);
            return Err(Refusal::new(format!("{was} could not be taken away ({e}), so the rename was undone and nothing was changed.")));
        }
        crate::files::note_tree_changed();
        Ok(Outcome {
            summary: format!("renamed {was} to {now}"),
            affected: 1,
            saved: true,
            // **The rename back is this operation's own undo form, and only an undo sends it.**
            inverse: Some(json!({"path": now, "to": was, "back": true})),
            after: Some(json!({"path": now, "text": text})),
        })
    }

    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let root = ctx.root_or_project()?;
        let path = after.get("path")?.as_str()?;
        Some(json!({"path": path, "text": current(&root, path)}))
    }

    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        RenameParams::parse(params).is_ok_and(|p| p.back == Some(true))
    }

    fn writes(&self) -> &'static [&'static str] {
        &["game_data"]
    }

    fn shows(&self, ctx: &Ctx, params: &serde_json::Value) -> Shown {
        let Ok(p) = RenameParams::parse(params) else { return Shown::File { path: PROJECT_FOLDER.to_owned(), lines: None } };
        let path = if p.back == Some(true) { p.path.clone() } else { p.destination() };
        let _ = ctx;
        Shown::File { path, lines: None }
    }
}

// ── file_delete ──────────────────────────────────────────────────────────────

/// **What [`file_delete`] takes.**
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeleteParams {
    /// The file to take out, under the project.
    pub path: String,
    /// **The undo's own form**: write this text back, byte for byte. Only an undo sends it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub restore: Option<String>,
}

impl DeleteParams {
    pub fn schema() -> serde_json::Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["path"],
            "properties": {
                "path": {"type": "string", "description": "The file to take out, relative to the project's root. A command file is refused, so is a file another file of the project names (naming it), and so is a map that places pieces (naming them)."}
            }
        })
    }

    pub fn parse(raw: &serde_json::Value) -> Result<DeleteParams, Refusal> {
        serde_json::from_value(raw.clone()).map_err(|e| {
            Refusal::new(format!("these are not a file to delete: {e}. The one field is `path`."))
        })
    }
}

/// **Delete a file**, with the text kept in the log so one undo puts it back byte for byte.
pub struct FileDelete;

impl FileDelete {
    fn resolve(ctx: &Ctx, p: &DeleteParams) -> Result<(std::path::PathBuf, String), Refusal> {
        let root = root_of(ctx)?;
        if p.path.is_empty() {
            return Err(Refusal::new("no file to delete: give `path`, the file's path under the project. Nothing was changed."));
        }
        let full = writable(&root, &p.path)?;
        if p.restore.is_some() {
            if full.exists() {
                return Err(Refusal::new(format!("{} is there again, so the deleted file is not put back over it.", p.path)));
            }
            return Ok((root, String::new()));
        }
        if !full.is_file() {
            return Err(Refusal::new(format!("{} is not a file of this project, so there is nothing to delete.", p.path)));
        }
        // **A map that places pieces is refused naming them** (`piece_remove`'s own argument): an orphaned placement is a
        // piece that silently fails to appear. Asked first, because a map is the more specific answer to "why not this file".
        if let Some(pieces) = pieces_placed_by(&root, &p.path)
            && !pieces.is_empty()
        {
            return Err(Refusal::new(format!("{} is a map that places {}, so taking it away would orphan them. Remove them from the map first, or delete a map that places nothing. Nothing was changed.", p.path, pieces.join(", "))));
        }
        // **The reference rule, the rename's own**: one function, so the two verbs cannot disagree.
        let named = named_elsewhere(&root, &p.path);
        if !named.is_empty() {
            let list = named.join(", ");
            return Err(Refusal::new(format!("{} is named by {list}, so deleting it would leave that name pointing at nothing. Change {list} first. Nothing was changed.", p.path)));
        }
        let text = current(&root, &p.path).unwrap_or_default();
        Ok((root, text))
    }
}

impl Operation for FileDelete {
    fn name(&self) -> &'static str {
        "file_delete"
    }

    fn undo(&self) -> Undo {
        Undo::Inverse
    }

    fn schema(&self) -> serde_json::Value {
        DeleteParams::schema()
    }

    fn plan(&self, ctx: &Ctx, params: &serde_json::Value) -> Result<serde_json::Value, Refusal> {
        let p = DeleteParams::parse(params)?;
        let (_, text) = Self::resolve(ctx, &p)?;
        Ok(json!({"summary": format!("delete {}", p.path), "path": p.path, "bytes": text.len()}))
    }

    fn commit(&self, ctx: &mut Ctx, params: &serde_json::Value) -> Result<Outcome, Refusal> {
        let p = DeleteParams::parse(params)?;
        let (root, old) = Self::resolve(ctx, &p)?;
        let full = root.join(&p.path);
        if let Some(text) = &p.restore {
            make_parent(&root, &p.path)?;
            blueprint_model::save::save_atomic(&full, text)
                .map_err(|e| Refusal::new(format!("{} was not put back: {e}", p.path)))?;
            crate::files::note_tree_changed();
            return Ok(Outcome {
                summary: format!("put {} back", p.path),
                affected: 1,
                saved: true,
                inverse: Some(json!({"path": p.path})),
                after: Some(json!({"path": p.path, "text": text})),
            });
        }
        std::fs::remove_file(&full).map_err(|e| Refusal::new(format!("{} was not deleted ({e}); nothing was changed.", p.path)))?;
        crate::files::note_tree_changed();
        Ok(Outcome {
            summary: format!("deleted {}", p.path),
            affected: 1,
            saved: true,
            // **The inverse is the file's own text**, so a delete is undone byte for byte, line endings included.
            inverse: Some(json!({"path": p.path, "restore": old})),
            after: Some(json!({"path": p.path, "text": null})),
        })
    }

    fn state_of(&self, ctx: &Ctx, after: &serde_json::Value) -> Option<serde_json::Value> {
        let root = ctx.root_or_project()?;
        let path = after.get("path")?.as_str()?;
        Some(json!({"path": path, "text": current(&root, path)}))
    }

    fn only_as_undo(&self, params: &serde_json::Value) -> bool {
        DeleteParams::parse(params).is_ok_and(|p| p.restore.is_some())
    }

    fn writes(&self) -> &'static [&'static str] {
        &["game_data"]
    }

    fn shows(&self, _ctx: &Ctx, params: &serde_json::Value) -> Shown {
        let Ok(p) = DeleteParams::parse(params) else { return Shown::File { path: PROJECT_FOLDER.to_owned(), lines: None } };
        Shown::File { path: p.path, lines: None }
    }
}
