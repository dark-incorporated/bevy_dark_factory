//! **What the map and the project hold, drawn as lists in the World and Sound workspaces** (NV-11, #392, #393, #394, #395).
//!
//! `docs/ui/navigation.md` measured four destinations that nothing drew: a project's **sound cues** (behind the `bevy_siren` feature),
//! a map's **lights** (behind `bevy_lights`), and a map's **water** and **forest** (operations, `water_place` and `forest_scatter`, written
//! through the World workspace's terrain line, with no panel). Each is a *list of data the project already has*: a cue is a name in
//! `family/bevy_siren.yaml`, a light is a placement's `bevy_lights` family value, the water plan and the forest are files the operations
//! write. None needs the family crate to be **read**; only to be edited or auditioned. So each list is drawn in every build, from the
//! file, and the family crate's own controls (the audition button, the pool budget) stay behind their feature.
//!
//! # Read-only on purpose
//!
//! The lists do not write. A second way to change what a list shows would be a second path to the same operation; what a list adds is the
//! thing nothing drew before, which is **what is there now**.
//!
//! # Filled when the file changes
//!
//! Each section is spawned with its workspace's panel (`workspace_panels`) and filled by a system that **subscribes to the project index**
//! (#501, [`crate::project_index`]): it reads its file when the index says the file's stamp moved (and once when the list first has an
//! index to ask), never on a clock, and rebuilds the rows only when what they say changed, as `problems_view` does for the PROBLEMS list.
//! A frame in which nothing moved costs an atomic load per list and no `stat`. A list built from the open map alone (the lights) is
//! rebuilt when the map changes, and the cue uses when the chosen cue or the finder's index changes: none of those reads a file.
//!
//! **What follows from reading the index and not the disk:** a file is seen up to the index's debounce and one sweep after it is written
//! ([`crate::project_index::SWEEP`], one second) unless the writer says it has finished ([`crate::project_index::note_written`]); a file the
//! index does not list (a link, a folder `.gitignore` names, `target`, `.git`) is **said, in the list's own row, not read**: the list
//! does not fall back on reading it from disk. A file that does not read is a row that names it, never a list that quietly says there
//! is nothing.

use std::cell::Cell;
use std::path::{Component as PathPart, Path, PathBuf};

use bevy::prelude::*;
use blueprint_model::map::Map;
use blueprint_model::water::WaterPlan;

use crate::chrome;
use crate::ops_forest::Forest;
use crate::project::{OpenMap, Project};
use crate::project_index::{FileStamp, ProjectIndex, Snapshot};
use crate::screen::Screen;
use crate::workspace::Workspace;

/// What the WATER list says when the map has none.
pub const NO_WATER: &str = "no water placed";

/// How the terrain line places water, in a line: the heading's one-line note. The line's own refusal spells the whole syntax, and a note that
/// wrapped over three lines pushed the headings below it off the panel's first screen.
pub const WATER_HINT: &str = "set on the TERRAIN line: water ...";

/// The WATER section's list, in the World workspace's panel.
#[derive(Component, Clone, Copy)]
pub struct WaterList;

/// What the FOREST list says when the map has none.
pub const NO_TREES: &str = "no trees planted";

/// How the terrain line plants a forest, in a line (see [`WATER_HINT`]).
pub const FOREST_HINT: &str = "set on the TERRAIN line: forest ...";

/// The FOREST section's list, in the World workspace's panel.
#[derive(Component, Clone, Copy)]
pub struct ForestList;

/// The most characters of a light's settings one row shows before it is cut with an ellipsis. A row is one line of the panel.
const ROW_CHARS: usize = 72;

/// The family key a placement's light fixture is saved under (`bevy_lights`), the same string `fixture::KEY` holds in a build that has the crate.
pub const LIGHTS_KEY: &str = "bevy_lights";

/// What the LIGHTS list says when the map has none.
pub const NO_LIGHTS: &str = "no light fixtures on this map";

/// Where a light is set: not here.
pub const LIGHTS_HINT: &str = "set per piece with family_set";

/// The LIGHTS section's list, in the World workspace's panel.
#[derive(Component, Clone, Copy)]
pub struct LightsList;

/// The crate's sound file under the project's `family/`, the same name `audio_panel::FILE` holds in a build that has the crate.
pub const SIREN_FILE: &str = "bevy_siren.yaml";

/// What the CUES list says when the project has none.
pub const NO_CUES: &str = "no cues in this project";

/// Where a cue is edited: not here.
pub const CUES_HINT: &str = "cues live in family/bevy_siren.yaml";

/// The CUES list, in the Sound workspace's panel.
#[derive(Component, Clone, Copy)]
pub struct CueList;

/// **The cue the list marks as selected** (NV-07, #384): the one a reference followed to, or a finder result chose. `Session`: it is
/// where the person is looking, and the Sound workspace's frame being rebuilt does not change what they chose. A cue the project no
/// longer holds is simply not marked.
#[derive(Resource, Default, Debug, Clone, PartialEq, Eq)]
pub struct SelectedCue(pub Option<String>);

/// **What the selected cue's row is marked with**, so a person reads which one is chosen.
pub const CUE_MARK: &str = "▸ ";

/// **The files that name the selected cue** (NV-07, #384), under the cue list.
#[derive(Component, Clone, Copy)]
pub struct CueUses;

pub struct WorldListsPlugin;

impl Plugin for WorldListsPlugin {
    fn build(&self, app: &mut App) {
        crate::project_index::ensure(app);
        app.init_resource::<SelectedCue>().add_systems(
            Update,
            (
                refresh_water.run_if(Workspace::world_is_open),
                refresh_forest.run_if(Workspace::world_is_open),
                refresh_lights.run_if(Workspace::world_is_open),
                refresh_cues.run_if(Workspace::sound_is_open),
                refresh_cue_uses.run_if(Workspace::sound_is_open),
            )
                .run_if(in_state(Screen::Editor)),
        );
    }
}

/// **The WATER section**: a heading, how to place some, and what the map's plan holds.
pub fn spawn_water_section(p: &mut ChildSpawnerCommands) {
    chrome::section(p, "WATER");
    chrome::body(p, WATER_HINT, chrome::ink().dim);
    p.spawn((WaterList, list_node()));
}

/// **The FOREST section**: a heading, how to plant one, and what the map's forest holds.
pub fn spawn_forest_section(p: &mut ChildSpawnerCommands) {
    chrome::section(p, "FOREST");
    chrome::body(p, FOREST_HINT, chrome::ink().dim);
    p.spawn((ForestList, list_node()));
}

/// **What reading a map's file found**: none, the value, or why it did not read.
enum Read<T> {
    Absent,
    Value(T),
    Broken(String),
}

/// **Read one YAML file a map's operation writes**, naming it (`water/m.yaml`) when it exists and does not parse.
///
/// The project is YAML since the cutover (GD-03, #365): this read a `.ron` and parsed with `ron::from_str`, so the lists were
/// empty for every project the editor can open.
fn read_file<T: serde::de::DeserializeOwned>(path: &Path, shown_as: &str) -> Read<T> {
    counted_read();
    match std::fs::read_to_string(path) {
        Ok(text) => match blueprint_model::yaml::from_str::<T>(&text, shown_as) {
            Ok(value) => Read::Value(value),
            Err(problem) => Read::Broken(problem.line()),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Read::Absent,
        Err(e) => Read::Broken(format!("{shown_as} cannot be read: {e}")),
    }
}

/// **The water plan in words**: a row per spring, sink and lake, in the order the plan lists them (springs, sinks, lakes).
pub fn water_rows(plan: &WaterPlan) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    for s in &plan.springs {
        rows.push(format!("spring at ({}, {}): {} m3/s, radius {} m", s.x, s.z, s.rate_m3_s, s.radius_m));
    }
    for s in &plan.sinks {
        rows.push(format!("sink at ({}, {}): {} m3/s, radius {} m", s.x, s.z, s.rate_m3_s, s.radius_m));
    }
    for l in &plan.lakes {
        rows.push(format!("lake at ({}, {}): radius {} m, level {} m", l.x, l.z, l.radius_m, l.level_m));
    }
    if rows.is_empty() {
        rows.push(NO_WATER.to_owned());
    }
    rows
}

/// **The forest in words**: how many trees, then a row per species with its count, in alphabetical order.
pub fn forest_rows(forest: &Forest) -> Vec<String> {
    if forest.trees.is_empty() {
        return vec![NO_TREES.to_owned()];
    }
    let mut counts: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for tree in &forest.trees {
        *counts.entry(tree.species.as_str()).or_default() += 1;
    }
    let mut rows = vec![format!("{} tree(s)", forest.trees.len())];
    rows.extend(counts.into_iter().map(|(species, n)| format!("{species}: {n}")));
    rows
}

/// **The fixtures list of the LIGHTS section**: the heading is the caller's (the family crate's section draws its own, with the pool
/// button), the rows are drawn in every build.
pub fn spawn_lights_list(p: &mut ChildSpawnerCommands) {
    chrome::body(p, LIGHTS_HINT, chrome::ink().dim);
    p.spawn((LightsList, list_node()));
}

/// **A map's lights in words**: a row per placement that carries `bevy_lights` settings, `<id>: <settings>`, the settings on one line and cut
/// at `ROW_CHARS`. A build without the crate reads the same value as text: it cannot tell a flicker from a fire, and does not need to.
pub fn light_rows(map: &Map) -> Vec<String> {
    let mut rows: Vec<String> = map
        .placements
        .iter()
        .filter_map(|p| p.family.get(LIGHTS_KEY).map(|settings| format!("{}: {}", p.id, one_line(settings))))
        .collect();
    if rows.is_empty() {
        rows.push(NO_LIGHTS.to_owned());
    }
    rows
}

/// A settings value on one line: whitespace collapsed, cut to [`ROW_CHARS`] with an ellipsis.
fn one_line(text: &str) -> String {
    let joined = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if joined.chars().count() <= ROW_CHARS {
        return joined;
    }
    let cut: String = joined.chars().take(ROW_CHARS.saturating_sub(1)).collect();
    format!("{cut}…")
}

/// **The cues list of the AUDIO section**, drawn in every build under the heading the caller draws.
pub fn spawn_cue_list(p: &mut ChildSpawnerCommands) {
    chrome::body(p, CUES_HINT, chrome::ink().dim);
    p.spawn((CueList, list_node()));
    p.spawn((CueUses, list_node()));
}

/// **Just the `used by` column**, for a build where the audio panel already names the cues (#600).
///
/// The panel draws the names and the choice Audition plays; this column answers the other question a following
/// reference asks — which file and line names this cue — and the two are one selection
/// (`world_lists::SelectedCue`), not two. Spawning `spawn_cue_list` beside the panel would draw the names twice.
pub fn spawn_cue_uses(p: &mut ChildSpawnerCommands) {
    p.spawn((CueUses, list_node()));
}

/// A list's column.
fn list_node() -> impl Bundle {
    (bevy_gooey::column(0.0), chrome::Gap::Tight.class())
}

/// **The cue names in `family/bevy_siren.yaml`**, read as a plain value rather than as the crate's `CueTable`, so a build without
/// `bevy_siren` lists them too. The file is a mapping of `settings:` (whatever the crate keeps) and `cues: {cues: [{name: ..}, ..]}`;
/// a file that does not read is an error naming what is wrong.
pub fn cue_names(text: &str) -> Result<Vec<String>, String> {
    // **The one YAML reader**, so this file is held to the rules every project file is, and a refusal names the field it stopped at.
    let file = blueprint_model::yaml::to_json(text, &siren_shown()).map_err(|p| p.line())?;
    let field = |value: &serde_json::Value, name: &str| -> Option<serde_json::Value> {
        value.as_object().and_then(|fields| fields.get(name)).cloned()
    };
    // A project with no cues table has none: the crate's own default is an empty table.
    let Some(table) = field(&file, "cues") else { return Ok(Vec::new()) };
    let Some(cues) = field(&table, "cues").and_then(|c| c.as_array().cloned()) else { return Ok(Vec::new()) };
    Ok(cues
        .iter()
        .filter_map(|cue| field(cue, "name").and_then(|n| n.as_str().map(str::to_owned)))
        .collect())
}

/// **The name a person reads for the sound file**, `family/bevy_siren.yaml`: one spelling, so the reader's refusal and the row that says
/// the file cannot be opened never disagree about which file it is.
fn siren_shown() -> String {
    format!("family/{SIREN_FILE}")
}

/// **The cue names of a project's sound file**: none when the project has no file, and why when the file does not read.
pub fn cue_names_of(project: &Project) -> Result<Vec<String>, String> {
    cue_names_at(&cue_file(project))
}

/// **Where a project's sound file is.**
fn cue_file(project: &Project) -> PathBuf {
    blueprint_model::family::settings_dir(&project.root).join(SIREN_FILE)
}

/// **The cue names of the sound file at `file`**: none when it is not there, and why when it does not read.
fn cue_names_at(file: &Path) -> Result<Vec<String>, String> {
    match std::fs::read_to_string(file) {
        Ok(text) => cue_names(&text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{} cannot be read: {e}", siren_shown())),
    }
}

/// **The sound cues of a project in words**: a row per cue (the selected one marked with [`CUE_MARK`]), or the line that says there
/// are none. A file that does not read is its own row.
pub fn cue_rows(project: &Project, selected: Option<&str>) -> Vec<String> {
    cue_rows_of(cue_names_of(project), selected)
}

/// **The cue rows for names already read** (or for the reason they were not), so a change of the chosen cue does not read the file again.
fn cue_rows_of(names: Result<Vec<String>, String>, selected: Option<&str>) -> Vec<String> {
    match names {
        Ok(names) if names.is_empty() => vec![NO_CUES.to_owned()],
        Ok(names) => names
            .into_iter()
            .map(|name| if selected == Some(name.as_str()) { format!("{CUE_MARK}{name}") } else { name })
            .collect(),
        Err(why) => vec![why],
    }
}

/// **What names the selected cue**, in words: one row per file and line, from the finder's own index (so a second scan cannot disagree
/// with it), or the sentence that nothing does. Nothing at all while no cue is selected.
pub fn cue_use_rows(selected: Option<&str>, index: &crate::goto::GotoIndex) -> Vec<String> {
    let Some(cue) = selected else { return Vec::new() };
    let uses: Vec<String> = index
        .used_by(crate::files_lang::CUE_KIND, cue)
        .map(|named| format!("used by {}:{}", named.file, named.line + 1))
        .collect();
    if uses.is_empty() { vec![format!("no file names `{cue}`")] } else { uses }
}

thread_local! {
    static READS: Cell<u64> = const { Cell::new(0) };
    static VERDICTS: Cell<u64> = const { Cell::new(0) };
}

/// **How many files the panels that follow the index through `Follow` have read on this thread**: the water plan, the forest and the
/// sound file of these lists, the relations file of the PROBLEMS list and the plan of the AGENT block's PLAN row. A test reads it across
/// frames to say that a change to a file none of them shows made none of them read anything. It counts only those reads: another module
/// that reads the same file (the finder reads the sound file for its rows) is not counted.
pub fn reads_on_this_thread() -> u64 {
    READS.with(Cell::get)
}

/// **Call this where a followed panel opens its file**, so [`reads_on_this_thread`] counts it.
pub(crate) fn counted_read() {
    READS.with(|c| c.set(c.get() + 1));
}

/// **How many times a followed file the index does not list was judged on this thread** (`files::project_path`, which reads the
/// project's `.gitignore` and canonicalises the root). A test reads it across an unrelated change to say that a file that is not
/// there is not judged again until the rules or the file moved.
pub fn verdict_lookups_on_this_thread() -> u64 {
    VERDICTS.with(Cell::get)
}

/// **One file followed through the project index** (#501): what a panel keeps in a `Local` so that it reads its file when the index says
/// the file moved, and at no other time. It holds the index generation it last compared at, and the file with what the index said of it
/// (its stamp, or why the index does not follow it) when it was last read.
///
/// **One atomic load per frame while nothing moved**: the generation says whether anything in the project moved, and only then is the
/// listing asked for this one file's stamp (a binary search of the sorted listing). A change to any other file moves the generation and
/// changes nothing here, so a file that did not move is not read again.
#[derive(Default)]
pub(crate) struct Follow {
    generation: u64,
    seen: Option<(PathBuf, Result<Option<FileStamp>, String>)>,
    /// The listing's `.gitignore` stamp `seen` was judged under; `None` when `seen` is not a verdict of a listing (the index had failed).
    rules: Option<Option<FileStamp>>,
}

impl Follow {
    /// **Whether the file moved since it was last read**, and where it is.
    ///
    /// - `None`: nothing to do: the file did not move, there is nothing to follow (no project, or `path` names no file), the index is
    ///   not following this project yet, or it has no listing yet. **A list that has nothing to say waits; it does not read the file on
    ///   its own.**
    /// - `Some(Ok(path))`: read the file at `path` now. This is also said the first time the index can answer, so a list is filled once
    ///   without waiting for a change.
    /// - `Some(Err(why))`: the index cannot answer for this file (it lies outside the project, the project's `.gitignore` or the
    ///   index's skipped folders hide it, or the index has no listing because the root cannot be read). Say `why`; do not read.
    ///
    /// `inputs_changed` says the project or whatever `path` is derived from changed this frame, so `path` is asked for; otherwise
    /// the path is not even built while the index's generation stands.
    pub(crate) fn moved(
        &mut self,
        index: &ProjectIndex,
        project: Option<&Project>,
        inputs_changed: bool,
        path: impl FnOnce(&Project) -> Option<PathBuf>,
    ) -> Option<Result<PathBuf, String>> {
        let Some(project) = project else {
            *self = Self::default();
            return None;
        };
        if index.root() != Some(project.root.as_path()) {
            return None;
        }
        let failure = index.failure();
        let generation = index.generation();
        if failure.is_none() && !inputs_changed && self.seen.is_some() && self.generation == generation {
            return None;
        }
        let Some(path) = path(project) else {
            *self = Self::default();
            return None;
        };
        let (stamp, rules) = match failure {
            Some(why) => (Err(format!("the project index cannot list {}: {why}", project.root.display())), None),
            None if generation == 0 => return None,
            None => {
                let snapshot = index.snapshot()?;
                let earlier = self.seen.as_ref().zip(self.rules).filter(|((seen, _), _)| *seen == path).map(|((_, last), rules)| (last, rules));
                (stamp_of(&snapshot, &project.root, &path, earlier), Some(rules_of(&snapshot)))
            }
        };
        self.generation = generation;
        self.rules = rules;
        if self.seen.as_ref().is_some_and(|(seen, last)| *seen == path && *last == stamp) {
            return None;
        }
        let said = match &stamp {
            Ok(_) => Ok(path.clone()),
            Err(why) => Err(why.clone()),
        };
        self.seen = Some((path, stamp));
        Some(said)
    }
}

/// **What the index said of one file**: its stamp, `None` when it lists none, or why it cannot answer.
pub(crate) type Verdict = Result<Option<FileStamp>, String>;

/// **The listing's stamp of the project's `.gitignore`**: what a verdict that a file is not followed was reached under. A rule that
/// changed moves it (the index lists the file), so a verdict is judged again then and not before.
pub(crate) fn rules_of(snapshot: &Snapshot) -> Option<FileStamp> {
    snapshot.files.binary_search_by(|(listed, _)| listed.as_str().cmp(".gitignore")).ok().map(|at| snapshot.files[at].1)
}

/// **What the index says of one file**: its stamp, `None` when the index lists no such file (it is not there), or why the index cannot
/// answer for it. A file the listing does not hold is "not there" only when the index would have listed it if it were; a file inside
/// a folder the project's `.gitignore` names, or inside `.git`, `target`, `.live` or `node_modules`, is not followed, and that is an
/// error to say, not an absence. (A link is never listed either; the listing is not asked about links, so a link reads as absent.)
///
/// **`earlier` is what the caller found for this same file and the [`rules_of`] it found it under.** Judging a file the listing does not
/// hold reads the project's `.gitignore` from disk, which a change to any other file must not make a panel do, so an earlier verdict of
/// "not there" or "not followed" stands while the rules' stamp is the same.
pub(crate) fn stamp_of(
    snapshot: &Snapshot,
    root: &Path,
    path: &Path,
    earlier: Option<(&Verdict, Option<FileStamp>)>,
) -> Verdict {
    let rel = rel_of(root, path)?;
    if let Ok(at) = snapshot.files.binary_search_by(|(listed, _)| listed.as_str().cmp(rel.as_str())) {
        return Ok(Some(snapshot.files[at].1));
    }
    if let Some((verdict, rules)) = earlier
        && !matches!(verdict, Ok(Some(_)))
        && rules == rules_of(snapshot)
    {
        return verdict.clone();
    }
    VERDICTS.with(|c| c.set(c.get() + 1));
    crate::files::project_path(root, &rel)
        .map(|_| None)
        .map_err(|why| format!("{why}; the project index does not list it, so it is not followed"))
}

/// **A path under the root as the index names it**, `/`-separated; an error when it is not under the root or not UTF-8.
fn rel_of(root: &Path, path: &Path) -> Result<String, String> {
    let outside = || format!("{} is outside the project at {}, and the project index follows only the project", path.display(), root.display());
    let under = path.strip_prefix(root).map_err(|_| outside())?;
    let mut parts: Vec<&str> = Vec::new();
    for component in under.components() {
        match component {
            PathPart::Normal(name) => parts.push(name.to_str().ok_or_else(|| format!("{} is not a UTF-8 path, so the project index does not list it", path.display()))?),
            _ => return Err(outside()),
        }
    }
    if parts.is_empty() {
        return Err(outside());
    }
    Ok(parts.join("/"))
}

/// **Keep one list equal to its rows.** `stale` says the rows may have changed (a file moved, or what the rows are made from did): then
/// `now` makes them, and they are rebuilt only when what they say changed. A list that a door change respawned has no children, and is
/// filled again from the rows held, with no read, even when nothing it says changed.
fn keep<L: Component>(
    commands: &mut Commands,
    held: &mut Option<Vec<String>>,
    lists: &Query<(Entity, Has<Children>), With<L>>,
    stale: bool,
    now: impl FnOnce() -> Option<Vec<String>>,
) {
    let mut changed = false;
    if stale {
        let Some(rows) = now() else { return };
        if held.as_ref() != Some(&rows) {
            *held = Some(rows);
            changed = true;
        }
    }
    let Some(rows) = held.as_ref() else { return };
    // **A list that is meant to be empty has nothing to be missing** (the cue uses, with no cue chosen), or it would be rebuilt every frame.
    let unfilled = !rows.is_empty() && lists.iter().any(|(_, filled)| !filled);
    if !changed && !unfilled {
        return;
    }
    for (list, _) in lists {
        commands.entity(list).despawn_related::<Children>();
        commands.entity(list).with_children(|p| {
            for row in rows {
                chrome::body(p, row.clone(), chrome::ink().text);
            }
        });
    }
}

/// What a list that follows a file keeps between frames: the rows it shows and the file it follows.
#[derive(Default)]
struct Held {
    rows: Option<Vec<String>>,
    file: Follow,
}

/// What the CUES list keeps: the rows, the file, and the names the file held when it was last read (so choosing another cue re-marks
/// the rows and reads nothing).
#[derive(Default)]
struct CuesHeld {
    rows: Option<Vec<String>>,
    file: Follow,
    names: Option<Result<Vec<String>, String>>,
}

fn changed<T: Resource>(resource: &Option<Res<T>>) -> bool {
    resource.as_ref().is_some_and(|r| r.is_changed())
}

fn refresh_cues(
    mut commands: Commands,
    index: Res<ProjectIndex>,
    project: Option<Res<Project>>,
    selected: Option<Res<SelectedCue>>,
    lists: Query<(Entity, Has<Children>), With<CueList>>,
    mut held: Local<CuesHeld>,
) {
    let CuesHeld { rows, file, names } = &mut *held;
    // **A choice is shown on the frame it is made.**
    let mut stale = changed(&selected) || rows.is_none();
    match file.moved(&index, project.as_deref(), changed(&project), |p| Some(cue_file(p))) {
        Some(Ok(path)) => {
            counted_read();
            let read = cue_names_at(&path);
            if let Err(why) = &read {
                warn!("{why}");
            }
            *names = Some(read);
            stale = true;
        }
        Some(Err(why)) => {
            warn!("{why}");
            *names = Some(Err(why));
            stale = true;
        }
        None => {}
    }
    let Some(names) = names.as_ref() else { return };
    let chosen = selected.as_deref().and_then(|s| s.0.as_deref());
    keep(&mut commands, rows, &lists, stale, || Some(cue_rows_of(names.clone(), chosen)));
}

fn refresh_cue_uses(
    mut commands: Commands,
    selected: Option<Res<SelectedCue>>,
    index: Res<crate::goto::GotoIndex>,
    lists: Query<(Entity, Has<Children>), With<CueUses>>,
    mut held: Local<Option<Vec<String>>>,
) {
    let stale = held.is_none() || changed(&selected) || index.is_changed();
    let chosen = selected.as_deref().and_then(|s| s.0.as_deref());
    keep(&mut commands, &mut held, &lists, stale, || Some(cue_use_rows(chosen, &index)));
}

fn refresh_lights(
    mut commands: Commands,
    open: Option<Res<OpenMap>>,
    lists: Query<(Entity, Has<Children>), With<LightsList>>,
    mut held: Local<Option<Vec<String>>>,
) {
    let stale = held.is_none() || changed(&open);
    keep(&mut commands, &mut held, &lists, stale, || Some(light_rows(&open.as_deref()?.map)));
}

fn refresh_water(
    mut commands: Commands,
    index: Res<ProjectIndex>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    lists: Query<(Entity, Has<Children>), With<WaterList>>,
    mut held: Local<Held>,
) {
    let Held { rows, file } = &mut *held;
    let moved = file.moved(&index, project.as_deref(), changed(&project) || changed(&open), |p| crate::ops_water::plan_path_of(p, open.as_deref()?));
    let now = moved.map(|moved| match moved {
        Ok(path) => {
            let shown_as = path.file_name().map_or_else(|| path.display().to_string(), |name| format!("water/{}", name.to_string_lossy()));
            match read_file::<WaterPlan>(&path, &shown_as) {
                Read::Value(plan) => water_rows(&plan),
                Read::Absent => vec![NO_WATER.to_owned()],
                Read::Broken(why) => {
                    warn!("{why}");
                    vec![why]
                }
            }
        }
        Err(why) => {
            warn!("{why}");
            vec![why]
        }
    });
    let stale = now.is_some();
    keep(&mut commands, rows, &lists, stale, || now);
}

fn refresh_forest(
    mut commands: Commands,
    index: Res<ProjectIndex>,
    project: Option<Res<Project>>,
    open: Option<Res<OpenMap>>,
    lists: Query<(Entity, Has<Children>), With<ForestList>>,
    mut held: Local<Held>,
) {
    let Held { rows, file } = &mut *held;
    let moved = file.moved(&index, project.as_deref(), changed(&project) || changed(&open), |p| {
        let name = &open.as_deref()?.map.name;
        Some(p.blueprint_dir.join("forest").join(format!("{name}.yaml")))
    });
    let now = moved.map(|moved| match moved {
        Ok(path) => {
            let shown_as = path.file_name().map_or_else(|| path.display().to_string(), |name| format!("forest/{}", name.to_string_lossy()));
            match read_file::<Forest>(&path, &shown_as) {
                Read::Value(forest) => forest_rows(&forest),
                Read::Absent => vec![NO_TREES.to_owned()],
                Read::Broken(why) => {
                    warn!("{why}");
                    vec![why]
                }
            }
        }
        Err(why) => {
            warn!("{why}");
            vec![why]
        }
    });
    let stale = now.is_some();
    keep(&mut commands, rows, &lists, stale, || now);
}
