//! **Negatives on purpose: one known defect on a scene the person kept, made with the editor's own operations** (DL-05, #464; series labels from
//! use, #459).
//!
//! Use produces ordinary successes and few failures, so a set built from use alone teaches a student to say yes. This module makes the other
//! half: it takes a **kept** turn's scene, applies **one** defect whose size is a named constant, and records the scene before (good) and after
//! (defective) with the defect as an **objective** label, because the engine can say whether it is there.
//!
//! # The defects, and what the engine measures for each
//!
//! | [`Defect`] | made with | the engine's fact, measured on the saved scene |
//! |---|---|---|
//! | `float` | `placement_set_lift` | the gap between the piece and what it should stand on: its resolved height with and without the lift |
//! | `clip` | `file_edit` on the map | the volume two pieces share: the plan rectangles' overlap area times the height they share |
//! | `outside_area` | `file_edit` on the map | whether the piece's centre is inside the design area it was in, and how far outside it is |
//! | `delete_named` | `placement_remove` | whether a piece the request named is still on the map |
//! | `turn_wrong` | `placement_set_yaw` | the angle between the piece's facing before and after |
//! | `thin_forest` | `file_edit` on the forest file | the trees before and after |
//! | `flatten_peak` | `terrain_stroke` | the height of the terrain's highest point before and after |
//!
//! A clip and an outside-area move use `file_edit` on the map's own file, because the editor has no operation that moves a piece and refuses
//! none that sets its `at`: `placement_add` refuses a footprint over another piece, which is exactly what a clip is. The thinned forest is a
//! `file_edit` for the same reason: the forest's own operation plants and has no inverse that is not an undo's.
//!
//! # Never on the person's map
//!
//! **Every perturbation runs on a scratch copy of the project** (`assets/` and `design/`, the folders the editor reads), made fresh for it and
//! removed after, as the benchmark does for a trial (BM-02). The person's project is read, never written: [`perturb`] opens nothing under its
//! root for writing, and `tests/negatives_on_purpose.rs` hashes every file of the project before and after a run.
//!
//! # A scene the turn did not leave is refused
//!
//! The ledger records a turn's after *picture* and the facts of its scene, not the map. A perturbation therefore starts from the project as it is
//! now, and only when it is still as the turn left it: the map has the placements the turn's facts counted, and every piece the turn touched is
//! where the facts put it. Anything else is refused, saying what differs, because a defect made on a scene that is not the one in the picture
//! would be a negative about a scene nobody saw.
//!
//! # A defect that cannot be made is refused, and says why
//!
//! No piece to float, nothing that can be pushed into another, no area, a request that names nothing on the map, no forest, no peak: each is a
//! [`Refused`] with the sentence. **A defect is recorded only when the engine measures it on the saved scene**, so an operation that applied
//! and changed nothing is a refusal too, never a no-op recorded as a defect.
//!
//! # Pictures
//!
//! The kept scene's picture is the turn's own after picture. The defective scene's picture comes from the caller's `render`, which is handed the
//! scratch project and its map: a headless run (a test) has nothing to draw with and says so in a [`Shot::missing`], and a run on a machine
//! with a window draws. **Assumed decision (owner may override):** the command that drives this over a ledger and renders through an editor is
//! DL-06's `cauldron-labels`; this module is the generator and the record it writes.

use std::path::{Path, PathBuf};

use bevy_grimoire_core::descriptor::{Descriptor, Mount, tipped_extents};
use bevy_grimoire_core::library::Library;
use bevy_grimoire_core::map::{Map, Placed};
use bevy_grimoire_core::{board, design, stack};
use serde_json::{Value, json};

use crate::ops::{self, Applied, Ctx};
use crate::project::{OpenMap, Project};
use crate::turn_store::{
    HandBackRef, ImageRef, LABEL_DEFECTIVE, OperationRecord, Perturbed, Provenance, Read, Request, Shot, Store, TIER_OBJECTIVE, Turn, scene_of, sha256_hex,
};

/// **How far a lifted piece floats, in metres, for a near-threshold defect.** Four centimetres is a gap a person sees against a table's edge
/// when they look for it and misses when they do not: the size where a student's yes and no are hardest to tell apart. A chosen size, not a
/// measured threshold; DL-08's report is where the student's own threshold is measured.
pub const FLOAT_NEAR_M: f32 = 0.04;
/// **How far a lifted piece floats for a clear defect**: thirty centimetres, a lamp in the air above its table.
pub const FLOAT_CLEAR_M: f32 = 0.30;

/// **How deep a clip is, in metres, for a near-threshold defect**: three times the engine's own overlap tolerance ([`stack::OVERLAP_EPS`]), the
/// shallowest interpenetration the problems list counts at all.
pub const CLIP_NEAR_M: f32 = 3.0 * stack::OVERLAP_EPS;
/// **How deep a clip is for a clear defect**: a quarter of a metre, a chair pushed into its desk.
pub const CLIP_CLEAR_M: f32 = 0.25;

/// **How far outside its area a piece's centre lands, in metres, for a near-threshold defect.** Ten centimetres is outside by the area test and
/// hardly outside to the eye, because most of the piece's footprint is still inside.
pub const OUTSIDE_NEAR_M: f32 = 0.10;
/// **How far outside its area a piece's centre lands for a clear defect**: a metre and a half, a piece clearly in the next room.
pub const OUTSIDE_CLEAR_M: f32 = 1.5;

/// **How far a piece is turned from its kept facing, in degrees, for a near-threshold defect.**
pub const TURN_NEAR_DEG: f32 = 15.0;
/// **How far a piece is turned for a clear defect**: the wrong way round.
pub const TURN_CLEAR_DEG: f32 = 180.0;

/// **The fraction of a forest's trees removed for a near-threshold defect**: one tree in five.
pub const THIN_NEAR_FRACTION: f32 = 0.2;
/// **The fraction of a forest's trees removed for a clear defect**: three in five.
pub const THIN_CLEAR_FRACTION: f32 = 0.6;

/// **The fraction of a peak's height above the lowest ground that is taken off, for a near-threshold defect.**
pub const FLATTEN_NEAR_FRACTION: f32 = 0.2;
/// **The fraction of a peak's height taken off for a clear defect**: the peak is more than halfway to the floor.
pub const FLATTEN_CLEAR_FRACTION: f32 = 0.7;

/// **The least a terrain's highest point must stand above its lowest for there to be a peak to flatten**, in metres.
pub const PEAK_MIN_M: f32 = 0.5;

/// **How many of the terrain's cells wide the flattening brush is**: the peak is one vertex, and a brush this wide takes it and its slopes.
pub const FLATTEN_BRUSH_CELLS: f32 = 1.5;

/// **How far a float, a clip, a move or a turn may differ from the size asked for, and still be the defect asked for**, in the unit of the
/// defect: a hair over a hundredth, since the engine's own tolerance is [`stack::OVERLAP_EPS`] and a file stores single-precision numbers.
const MEASURE_TOLERANCE: f64 = 0.015;

/// **How many steps the search for where a pushed piece first touches another takes.** Fifty halvings of a metre is far below a float's grain.
const CONTACT_STEPS: usize = 50;

/// **The folders of a project the editor reads**, copied for a scratch project: `assets/` holds the library, the meshes and the maps, `design/`
/// the areas. Anything else under a game's root (a build's `target/`, a repository's `.git`) is not the editor's and is not copied.
const PROJECT_FOLDERS: [&str; 2] = ["assets", "design"];

/// **One thing that can go wrong with a kept scene.**
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Defect {
    /// A piece lifted off what it stands on.
    Float,
    /// A piece pushed into another.
    Clip,
    /// A piece moved out of the design area it was in.
    OutsideArea,
    /// A piece the request named, deleted.
    DeleteNamed,
    /// A piece turned the wrong way.
    TurnWrong,
    /// A forest with trees taken out.
    ThinForest,
    /// The terrain's highest point brought down.
    FlattenPeak,
}

impl Defect {
    /// **Every defect**, in the order a run makes them.
    pub const ALL: [Defect; 7] =
        [Defect::Float, Defect::Clip, Defect::OutsideArea, Defect::DeleteNamed, Defect::TurnWrong, Defect::ThinForest, Defect::FlattenPeak];

    /// The word a record carries.
    pub fn name(self) -> &'static str {
        match self {
            Defect::Float => "float",
            Defect::Clip => "clip",
            Defect::OutsideArea => "outside_area",
            Defect::DeleteNamed => "delete_named",
            Defect::TurnWrong => "turn_wrong",
            Defect::ThinForest => "thin_forest",
            Defect::FlattenPeak => "flatten_peak",
        }
    }

    /// What [`Perturbed::magnitude`] counts.
    pub fn unit(self) -> &'static str {
        match self {
            Defect::Float | Defect::Clip | Defect::OutsideArea => "m",
            Defect::DeleteNamed => "placements",
            Defect::TurnWrong => "deg",
            Defect::ThinForest | Defect::FlattenPeak => "fraction",
        }
    }

    /// **The sizes this defect comes in.** A deletion has none: a piece is on the map or it is not.
    pub fn sizes(self) -> &'static [Size] {
        match self {
            Defect::DeleteNamed => &[Size::Clear],
            _ => &[Size::Near, Size::Clear],
        }
    }

    /// **The defect's size in its unit**, from the named constants above.
    pub fn magnitude(self, size: Size) -> f64 {
        let (near, clear) = match self {
            Defect::Float => (FLOAT_NEAR_M, FLOAT_CLEAR_M),
            Defect::Clip => (CLIP_NEAR_M, CLIP_CLEAR_M),
            Defect::OutsideArea => (OUTSIDE_NEAR_M, OUTSIDE_CLEAR_M),
            Defect::DeleteNamed => (1.0, 1.0),
            Defect::TurnWrong => (TURN_NEAR_DEG, TURN_CLEAR_DEG),
            Defect::ThinForest => (THIN_NEAR_FRACTION, THIN_CLEAR_FRACTION),
            Defect::FlattenPeak => (FLATTEN_NEAR_FRACTION, FLATTEN_CLEAR_FRACTION),
        };
        f64::from(match size {
            Size::Near => near,
            Size::Clear => clear,
        })
    }
}

/// **How big a defect is**: just over what a person notices, or plainly there. Easy negatives teach little, so both are made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Size {
    Near,
    Clear,
}

impl Size {
    /// The word a record carries.
    pub fn name(self) -> &'static str {
        match self {
            Size::Near => "near",
            Size::Clear => "clear",
        }
    }
}

/// **A defect that was not made, and why.** Never a record: nothing is written for it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refused {
    pub turn: String,
    pub defect: Defect,
    pub size: Size,
    /// The sentence a person reads.
    pub why: String,
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({}) on {}: {}", self.defect.name(), self.size.name(), self.turn, self.why)
    }
}

/// **A fresh copy of the project's editable folders**, removed when it goes out of scope.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn make(project: &Path, parent: &Path, name: &str) -> Result<Scratch, String> {
        let root = parent.join(name);
        if root.exists() {
            // Ours: a scratch name carries the turn, the defect and the size, and a previous run of this one died before it cleaned up.
            std::fs::remove_dir_all(&root).map_err(|e| format!("{}: {e}", root.display()))?;
        }
        let scratch = Scratch { root };
        for folder in PROJECT_FOLDERS {
            let from = project.join(folder);
            if from.is_dir() {
                copy_tree(&from, &scratch.root.join(folder))?;
            }
        }
        Ok(scratch)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// **Copy a folder's files**, following nothing that is a link: a project that links out of itself is not copied out of itself.
fn copy_tree(from: &Path, to: &Path) -> Result<(), String> {
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    for entry in std::fs::read_dir(from).map_err(|e| format!("{}: {e}", from.display()))?.flatten() {
        let kind = entry.file_type().map_err(|e| format!("{}: {e}", entry.path().display()))?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            // **Atomic, though it is a scratch copy**: the scan that keeps a project file from being written the plain way sees every write, and a copy
            // that dies half way must not leave a file that reads as whole.
            let bytes = std::fs::read(entry.path()).map_err(|e| format!("{}: {e}", entry.path().display()))?;
            bevy_grimoire_core::atomic_file::write_cache(&target, bytes).map_err(|e| format!("{}: {e}", target.display()))?;
        }
    }
    Ok(())
}

/// **The scratch project, open**: the project, the map as the scratch's disk has it, and a dispatch context over both.
struct Scene {
    root: PathBuf,
    map_name: String,
    project: Project,
    open: OpenMap,
    session: String,
}

impl Scene {
    fn open(root: &Path, map_name: &str, session: String) -> Result<Scene, String> {
        let project = Project::open(root, None)?;
        let open = OpenMap::open(&project, map_name)?;
        Ok(Scene { root: root.to_path_buf(), map_name: map_name.to_owned(), project, open, session })
    }

    /// **Read the project and its map again from the scratch's disk**, which is the only place a defect counts: what an operation says it did is
    /// not what is measured.
    fn reload(&mut self) -> Result<(), String> {
        self.project = Project::open(&self.root, None)?;
        self.open = OpenMap::open(&self.project, &self.map_name)?;
        Ok(())
    }

    /// **Run one of the editor's own operations** as an agent would, and reload from disk.
    fn dispatch(&mut self, operation: &str, params: &Value) -> Result<(), String> {
        let mut ctx = Ctx {
            project: Some(self.project.clone()),
            open: Some(self.open.clone()),
            source: ops::Source::Agent,
            session: self.session.clone(),
            ..Ctx::default()
        };
        match ops::dispatch(&mut ctx, operation, params) {
            Ok(Applied::Done { .. }) => {}
            Ok(Applied::Refused(why)) | Err(why) => return Err(format!("`{operation}` was refused: {why}")),
        }
        self.reload()
    }

    /// **A project file's path under the root**, for `file_edit`.
    fn relative(&self, path: &Path) -> Result<String, String> {
        // Both sides resolved, because a temp folder is often a link (`/var` is `/private/var` on a Mac) and the project may name it either way.
        let (root, path) = (std::fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone()), std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()));
        path.strip_prefix(&root).map(|p| p.to_string_lossy().replace('\\', "/")).map_err(|_| format!("{} is not under the scratch project", path.display()))
    }

    /// **Set a number in the map's own file**, by its path (`placements[2].at[0]`), through `file_edit`.
    fn edit_map(&mut self, edits: &[(String, f32)]) -> Result<(), String> {
        let path = self.relative(&self.open.map_path)?;
        let edits: Vec<Value> = edits.iter().map(|(at, value)| json!({"at": at, "value": value})).collect();
        self.dispatch("file_edit", &json!({"path": path, "edits": edits}))
    }

    fn library(&self) -> &Library {
        &self.project.library
    }

    fn map(&self) -> &Map {
        &self.open.map
    }

    /// **The operations this scene's session ran**, as the scratch's own log wrote them.
    fn operations(&self) -> Vec<OperationRecord> {
        ops::read_entries(&ops::oplog_path(&self.project))
            .into_iter()
            .filter(|entry| entry.session == self.session && entry.outcome == "applied")
            .map(|entry| {
                let line = serde_json::to_value(&entry).unwrap_or(Value::Null);
                OperationRecord {
                    operation_id: entry.operation_id.clone(),
                    op: entry.op.clone(),
                    seq: entry.seq,
                    source: line["source"].as_str().unwrap_or_default().to_owned(),
                    line,
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// What the engine measures. None of this calls the operation that made the change.
// ---------------------------------------------------------------------------------------------------------------------------------------------

/// **A piece's plan rectangle**: centre, two unit axes and two half extents, the shape `stack` tests with.
type Plan = ((f32, f32), [(f32, f32); 2], (f32, f32));

fn plan_of(d: &Descriptor, p: &Placed) -> Option<Plan> {
    let (w, _h, depth) = tipped_extents(d, p.tip)?;
    let (s, c) = p.yaw.to_radians().sin_cos();
    Some((p.at, [(c, -s), (s, c)], (w * 0.5, depth * 0.5)))
}

fn corners_of((centre, axes, half): Plan) -> Vec<(f32, f32)> {
    [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)]
        .into_iter()
        .map(|(sx, sz)| {
            (
                centre.0 + axes[0].0 * half.0 * sx + axes[1].0 * half.1 * sz,
                centre.1 + axes[0].1 * half.0 * sx + axes[1].1 * half.1 * sz,
            )
        })
        .collect()
}

/// **The area two convex quads share** (Sutherland–Hodgman, `subject` clipped by each edge of `clip`, which both wind the same way).
fn shared_area(subject: &[(f32, f32)], clip: &[(f32, f32)]) -> f64 {
    let side = |a: (f32, f32), b: (f32, f32), p: (f32, f32)| (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
    let mut out: Vec<(f32, f32)> = subject.to_vec();
    for i in 0..clip.len() {
        let (a, b) = (clip[i], clip[(i + 1) % clip.len()]);
        let input = std::mem::take(&mut out);
        for j in 0..input.len() {
            let (p, q) = (input[j], input[(j + 1) % input.len()]);
            let (sp, sq) = (side(a, b, p), side(a, b, q));
            if sp >= 0.0 {
                out.push(p);
            }
            if (sp >= 0.0) != (sq >= 0.0) {
                let t = sp / (sp - sq);
                out.push((p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t));
            }
        }
        if out.is_empty() {
            return 0.0;
        }
    }
    let twice: f64 = (0..out.len())
        .map(|i| {
            let (p, q) = (out[i], out[(i + 1) % out.len()]);
            f64::from(p.0) * f64::from(q.1) - f64::from(q.0) * f64::from(p.1)
        })
        .sum();
    (twice * 0.5).abs()
}

/// **How far the piece at `index` floats**: its resolved height with its authored lift, less its resolved height without it. `stack::resolve_y`
/// decides the datum (floor, a host's surface) and the lift nudges it afterwards, so the difference is exactly the gap the lift opens.
fn float_gap(map: &Map, library: &Library, index: usize) -> Result<f64, String> {
    let with = stack::resolve_y(map, library)?;
    let mut resting = map.clone();
    resting.placements[index].lift = 0.0;
    let without = stack::resolve_y(&resting, library)?;
    Ok(f64::from(with[index] - without[index]))
}

/// **The volume two pieces share**, in cubic metres: the plan rectangles' common area times the height the two share.
fn shared_volume(map: &Map, library: &Library, a: usize, b: usize) -> Result<Value, String> {
    let heights = stack::resolve_y(map, library)?;
    let (pa, pb) = (&map.placements[a], &map.placements[b]);
    let (da, db) = (stack::descriptor_of(library, pa).ok_or("a piece is not in the library")?, stack::descriptor_of(library, pb).ok_or("a piece is not in the library")?);
    let (ra, rb) = (plan_of(&da, pa).ok_or("a piece is unmeasured")?, plan_of(&db, pb).ok_or("a piece is unmeasured")?);
    let area = shared_area(&corners_of(ra), &corners_of(rb));
    let (ha, hb) = (tipped_extents(&da, pa.tip).map_or(0.0, |e| e.1), tipped_extents(&db, pb.tip).map_or(0.0, |e| e.1));
    let top = (heights[a] + ha).min(heights[b] + hb);
    let bottom = heights[a].max(heights[b]);
    let vertical = f64::from((top - bottom).max(0.0));
    let engine = stack::overlaps(map, library).iter().any(|&(i, j)| (i, j) == (a.min(b), a.max(b)));
    Ok(json!({
        "pair": [pa.id, pb.id],
        "engine_overlap": engine,
        "plan_overlap_m2": area,
        "vertical_overlap_m": vertical,
        "base_y_m": [heights[a], heights[b]],
        "height_m": [ha, hb],
        "overlap_volume_m3": area * vertical,
    }))
}

/// **How far a point is from a rectangle**: nothing inside it.
fn distance_outside(rect: [f32; 4], (x, z): (f32, f32)) -> f64 {
    let dx = (rect[0] - x).max(x - rect[2]).max(0.0);
    let dz = (rect[1] - z).max(z - rect[3]).max(0.0);
    f64::from(dx.hypot(dz))
}

/// **The smallest angle between two facings**, in degrees, `0..=180`.
fn angle_between(a: f32, b: f32) -> f64 {
    let d = (f64::from(a) - f64::from(b)).rem_euclid(360.0);
    d.min(360.0 - d)
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// Choosing what to break.
// ---------------------------------------------------------------------------------------------------------------------------------------------

/// **Whether a piece stands somewhere a lift reads as floating**: on the floor or on a surface. A wall piece, a ceiling piece, a doorway fill,
/// a floor tile and a decal have no "standing", so lifting one is not the defect.
fn can_float(d: &Descriptor) -> bool {
    matches!(d.mount, None | Some(Mount::OnFloor | Mount::OnSurface { .. } | Mount::OnSurfaceOrFloor { .. }))
}

/// **Whether a piece can be pushed into another**: it stands on the floor, and is not itself the host of anything resting on it.
fn can_be_pushed(map: &Map, library: &Library, index: usize) -> bool {
    let p = &map.placements[index];
    stack::descriptor_of(library, p).is_some_and(|d| has_volume(&d, p) && matches!(d.mount, None | Some(Mount::OnFloor)))
        && p.on.is_none()
        && !map.placements.iter().any(|other| other.on.as_deref() == Some(p.id.as_str()))
}

/// **Whether a piece fills any volume**: a plan, and a height over nothing. A library entry that records a height of zero (the shipped chair does)
/// shares no volume with anything, so a clip of it has nothing to measure and is not one this module makes.
fn has_volume(d: &Descriptor, p: &Placed) -> bool {
    plan_of(d, p).is_some() && tipped_extents(d, p.tip).is_some_and(|(_, height, _)| height > 0.0)
}

/// **The placements a turn touched**, in the order its facts name them, then every other, so the defect is made on what the person asked for
/// first.
fn order_of_interest(turn: &Turn, map: &Map) -> Vec<usize> {
    let mut named: Vec<usize> = Vec::new();
    for operation in turn.facts["operations"].as_array().into_iter().flatten() {
        for piece in operation["pieces"].as_array().into_iter().flatten() {
            if let Some(index) = piece["id"].as_str().and_then(|id| map.placements.iter().position(|p| p.id == id))
                && !named.contains(&index)
            {
                named.push(index);
            }
        }
    }
    let rest = (0..map.placements.len()).filter(|i| !named.contains(i));
    named.iter().copied().chain(rest.collect::<Vec<_>>()).collect()
}

/// **The words of the request**, lower-cased, with underscores and dashes read as spaces: what a piece's name is searched for in.
fn request_words(turn: &Turn) -> String {
    let request = &turn.request;
    let mut text = format!("{} {}", request.title, request.summary);
    for line in request.specified.iter().chain(&request.inferred).chain(&request.check).chain(request.note.iter()) {
        text.push(' ');
        text.push_str(line);
    }
    text.to_lowercase().replace(['_', '-', '@'], " ")
}

/// **What the person would call a piece**: its descriptor's id, with underscores as spaces (`coffee_table` is "coffee table").
fn name_of(p: &Placed) -> String {
    p.descriptor.to_lowercase().replace(['_', '-'], " ")
}

/// **Whether `needle` is in `text` as whole words.**
fn names(text: &str, needle: &str) -> bool {
    text.match_indices(needle).any(|(at, _)| {
        let before = text[..at].chars().next_back().is_none_or(|c| !c.is_alphanumeric());
        let after = text[at + needle.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        before && after
    })
}

/// **Whether the scene is as the turn left it**: the placements the turn's facts counted, and every piece it touched where the facts put it.
fn as_the_turn_left_it(turn: &Turn, project: &Project, map: &Map) -> Result<(), String> {
    let Some(counted) = turn.facts["map"]["placements"].as_u64() else {
        return Err("the turn's facts hold no map, so there is no scene to compare with".to_owned());
    };
    // **The census counts, not this module**: the one counter the panels and the log agree on.
    let held = bevy_grimoire_core::census::of_map(map, &project.compositions.compositions, &project.library)
        .map_err(|why| format!("the map's stamps do not expand, so its rows cannot be counted: {why}"))?
        .placements;
    if counted != held as u64 {
        return Err(format!("the map holds {held} placements and the turn left {counted}, so it has changed since"));
    }
    for operation in turn.facts["operations"].as_array().into_iter().flatten() {
        for piece in operation["pieces"].as_array().into_iter().flatten() {
            let Some(id) = piece["id"].as_str() else { continue };
            let there = map.placements.iter().find(|p| p.id == id);
            if piece["present"].as_bool() == Some(false) {
                if there.is_some() {
                    return Err(format!("`{id}` was gone when the turn ended and is on the map now, so it has changed since"));
                }
                continue;
            }
            let Some(placed) = there else {
                return Err(format!("`{id}` was on the map when the turn ended and is not now, so it has changed since"));
            };
            let at = (piece["at"][0].as_f64(), piece["at"][1].as_f64());
            let yaw = piece["yaw"].as_f64();
            let moved = match (at, yaw) {
                ((Some(x), Some(z)), Some(yaw)) => {
                    (f64::from(placed.at.0) - x).abs() > 1e-3 || (f64::from(placed.at.1) - z).abs() > 1e-3 || angle_between(placed.yaw, yaw as f32) > 1e-3
                }
                _ => false,
            };
            if moved {
                return Err(format!("`{id}` is not where the turn left it, so the map has changed since"));
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// The defects.
// ---------------------------------------------------------------------------------------------------------------------------------------------

/// **What a defect did**: what it was made on, and what the engine measured on each side of it.
struct Made {
    target: Vec<String>,
    before: Value,
    after: Value,
}

/// **Make one defect in the scratch scene**, or say why it cannot be made.
fn make(scene: &mut Scene, turn: &Turn, defect: Defect, size: Size) -> Result<Made, String> {
    let amount = defect.magnitude(size);
    match defect {
        Defect::Float => float(scene, turn, amount as f32),
        Defect::Clip => clip(scene, turn, amount as f32),
        Defect::OutsideArea => outside_area(scene, turn, amount as f32),
        Defect::DeleteNamed => delete_named(scene, turn),
        Defect::TurnWrong => turn_wrong(scene, turn, amount as f32),
        Defect::ThinForest => thin_forest(scene, amount as f32),
        Defect::FlattenPeak => flatten_peak(scene, amount as f32),
    }
}

fn float(scene: &mut Scene, turn: &Turn, lift: f32) -> Result<Made, String> {
    let candidates: Vec<usize> = order_of_interest(turn, scene.map())
        .into_iter()
        .filter(|&i| {
            let p = &scene.map().placements[i];
            p.lift == 0.0 && stack::descriptor_of(scene.library(), p).is_some_and(|d| can_float(&d) && plan_of(&d, p).is_some())
        })
        .collect();
    if candidates.is_empty() {
        return Err("no piece on the map stands on the floor or a surface with no lift of its own, so there is nothing to float".to_owned());
    }
    let mut last = String::new();
    for index in candidates {
        let id = scene.map().placements[index].id.clone();
        let before = json!({"piece": id, "lift_m": 0.0, "gap_m": float_gap(scene.map(), scene.library(), index)?});
        match scene.dispatch("placement_set_lift", &json!({"id": id, "lift": lift})) {
            Ok(()) => {
                let at = scene.map().placements.iter().position(|p| p.id == id).ok_or("the piece is gone after it was lifted")?;
                let after = json!({"piece": id, "lift_m": scene.map().placements[at].lift, "gap_m": float_gap(scene.map(), scene.library(), at)?});
                return Ok(Made { target: vec![id], before, after });
            }
            Err(why) => last = why,
        }
    }
    Err(last)
}

fn clip(scene: &mut Scene, turn: &Turn, depth: f32) -> Result<Made, String> {
    let order = order_of_interest(turn, scene.map());
    let heights = stack::resolve_y(scene.map(), scene.library())?;
    let overlapping: Vec<(usize, usize)> = stack::overlaps(scene.map(), scene.library());
    // The piece pushed, and the one it is pushed into: the pushed piece first in the turn's interest, the other the nearest on its own layer.
    for &a in &order {
        if !can_be_pushed(scene.map(), scene.library(), a) {
            continue;
        }
        let pa = &scene.map().placements[a];
        let da = stack::descriptor_of(scene.library(), pa).ok_or("a piece is not in the library")?;
        let mut others: Vec<(f32, usize)> = (0..scene.map().placements.len())
            .filter(|&b| b != a && !overlapping.iter().any(|&(i, j)| (i, j) == (a.min(b), a.max(b))))
            .filter(|&b| {
                let pb = &scene.map().placements[b];
                stack::descriptor_of(scene.library(), pb).is_some_and(|db| has_volume(&db, pb) && matches!(db.mount, None | Some(Mount::OnFloor)))
                    && pb.lift == 0.0
                    && (heights[a] - heights[b]).abs() < 1e-3
            })
            .map(|b| {
                let pb = &scene.map().placements[b];
                ((pb.at.0 - pa.at.0).hypot(pb.at.1 - pa.at.1), b)
            })
            .collect();
        others.sort_by(|x, y| x.0.total_cmp(&y.0));
        let Some(&(distance, b)) = others.first() else { continue };
        if distance <= f32::EPSILON {
            continue;
        }
        let pb = scene.map().placements[b].clone();
        let db = stack::descriptor_of(scene.library(), &pb).ok_or("a piece is not in the library")?;
        let rb = corners_of(plan_of(&db, &pb).ok_or("a piece is unmeasured")?);
        drop(db);
        let (ux, uz) = ((pb.at.0 - pa.at.0) / distance, (pb.at.1 - pa.at.1) / distance);
        // Slide `a` along the line to `b` and find, by halving, the distance at which the two first share any floor.
        let shares = |t: f32| {
            let mut moved = pa.clone();
            moved.at = (pa.at.0 + ux * t, pa.at.1 + uz * t);
            plan_of(&da, &moved).is_some_and(|plan| shared_area(&corners_of(plan), &rb) > 1e-6)
        };
        // **Already sharing floor** is the engine's own tolerance's business, not a push's: start from a piece that touches nothing of it.
        if shares(0.0) || !shares(distance) {
            continue;
        }
        let (mut apart, mut touching) = (0.0_f32, distance);
        for _ in 0..CONTACT_STEPS {
            let mid = (apart + touching) * 0.5;
            if shares(mid) {
                touching = mid;
            } else {
                apart = mid;
            }
        }
        drop(da);
        let t = (touching + depth).min(distance);
        let at = (pa.at.0 + ux * t, pa.at.1 + uz * t);
        let (id_a, id_b) = (pa.id.clone(), pb.id.clone());
        let before = shared_volume(scene.map(), scene.library(), a, b)?;
        scene.edit_map(&[(format!("placements[{a}].at[0]"), at.0), (format!("placements[{a}].at[1]"), at.1)])?;
        let (ia, ib) = (
            scene.map().placements.iter().position(|p| p.id == id_a).ok_or("the pushed piece is gone")?,
            scene.map().placements.iter().position(|p| p.id == id_b).ok_or("the piece pushed into is gone")?,
        );
        let after = shared_volume(scene.map(), scene.library(), ia, ib)?;
        return Ok(Made { target: vec![id_a, id_b], before, after });
    }
    Err("no piece that stands on the floor has another on its layer to be pushed into that it is not already inside".to_owned())
}

fn outside_area(scene: &mut Scene, turn: &Turn, outside: f32) -> Result<Made, String> {
    let level = board::level_place(&scene.root, &scene.map_name);
    let areas = design::areas_of_level(&scene.root, &level);
    if areas.is_empty() {
        return Err(format!("the map has no design area (`{level}` holds none), so no piece is in one to be taken out of it"));
    }
    let (hx, hz) = (scene.map().bounds.0 * 0.5, scene.map().bounds.2 * 0.5);
    let floor = [-hx, -hz, hx, hz];
    for index in order_of_interest(turn, scene.map()) {
        let p = scene.map().placements[index].clone();
        if !stack::descriptor_of(scene.library(), &p).is_some_and(|d| matches!(d.mount, None | Some(Mount::OnFloor))) || p.on.is_some() {
            continue;
        }
        let Some(area) = design::area_at(&areas, p.at).cloned() else { continue };
        let [x0, z0, x1, z1] = area.rect;
        // The four places just outside the nearest edges, nearest first; the first that stays on the map floor is where the piece goes.
        let mut ways = [
            (p.at.0 - x0, (x0 - outside, p.at.1)),
            (x1 - p.at.0, (x1 + outside, p.at.1)),
            (p.at.1 - z0, (p.at.0, z0 - outside)),
            (z1 - p.at.1, (p.at.0, z1 + outside)),
        ];
        ways.sort_by(|a, b| a.0.total_cmp(&b.0));
        let Some(&(_, to)) = ways.iter().find(|(_, to)| design::inside(floor, *to) && design::area_at(&areas, *to).is_none_or(|other| other.name != area.name)) else {
            continue;
        };
        let before = json!({"piece": p.id, "area": area.name, "rect": area.rect, "inside": design::inside(area.rect, p.at), "distance_outside_m": distance_outside(area.rect, p.at)});
        scene.edit_map(&[(format!("placements[{index}].at[0]"), to.0), (format!("placements[{index}].at[1]"), to.1)])?;
        let moved = scene.map().placements.iter().find(|q| q.id == p.id).ok_or("the moved piece is gone")?.at;
        let after = json!({"piece": p.id, "area": area.name, "rect": area.rect, "inside": design::inside(area.rect, moved), "distance_outside_m": distance_outside(area.rect, moved)});
        return Ok(Made { target: vec![p.id], before, after });
    }
    Err("no piece that stands on the floor is inside a design area with a place just outside it on the map".to_owned())
}

fn delete_named(scene: &mut Scene, turn: &Turn) -> Result<Made, String> {
    let words = request_words(turn);
    for index in order_of_interest(turn, scene.map()) {
        let p = scene.map().placements[index].clone();
        let needle = name_of(&p);
        if needle.is_empty() || !names(&words, &needle) {
            continue;
        }
        let count_of = |map: &Map| map.placements.iter().filter(|q| q.descriptor == p.descriptor).count();
        // `placement_remove` takes what stands on the piece with it, so the map's whole count is a fact too.
        let before = json!({"descriptor": p.descriptor, "named_by": needle, "piece": p.id, "present": true, "count": count_of(scene.map()), "placements": scene.map().placements.len()});
        scene.dispatch("placement_remove", &json!({"id": p.id}))?;
        let after = json!({"descriptor": p.descriptor, "named_by": needle, "piece": p.id, "present": scene.map().placements.iter().any(|q| q.id == p.id), "count": count_of(scene.map()), "placements": scene.map().placements.len()});
        return Ok(Made { target: vec![p.id], before, after });
    }
    Err("the request names nothing that is on the map (its title, summary, specified, inferred and check lines hold no piece's name), so there is nothing named to delete".to_owned())
}

fn turn_wrong(scene: &mut Scene, turn: &Turn, degrees: f32) -> Result<Made, String> {
    let candidates: Vec<usize> = order_of_interest(turn, scene.map())
        .into_iter()
        .filter(|&i| stack::descriptor_of(scene.library(), &scene.map().placements[i]).is_some_and(|d| plan_of(&d, &scene.map().placements[i]).is_some()))
        .collect();
    if candidates.is_empty() {
        return Err("no piece on the map is measured, so none has a facing to turn".to_owned());
    }
    let mut last = String::new();
    for index in candidates {
        let p = scene.map().placements[index].clone();
        let before = json!({"piece": p.id, "yaw_deg": p.yaw});
        match scene.dispatch("placement_set_yaw", &json!({"id": p.id, "yaw": p.yaw + degrees})) {
            Ok(()) => {
                let now = scene.map().placements.iter().find(|q| q.id == p.id).ok_or("the turned piece is gone")?.yaw;
                let after = json!({"piece": p.id, "yaw_deg": now, "turned_deg": angle_between(p.yaw, now)});
                return Ok(Made { target: vec![p.id], before, after });
            }
            Err(why) => last = why,
        }
    }
    Err(last)
}

fn forest_file(scene: &Scene) -> PathBuf {
    scene.project.grimoire_dir.join("forest").join(format!("{}.yaml", scene.map_name))
}

fn thin_forest(scene: &mut Scene, fraction: f32) -> Result<Made, String> {
    let path = forest_file(scene);
    let text = std::fs::read_to_string(&path).map_err(|_| format!("the map has no forest (`{}` is not there), so there are no trees to thin", scene.relative(&path).unwrap_or_default()))?;
    let mut forest: crate::ops_forest::Forest = bevy_grimoire_core::yaml::from_str(&text, "forest").map_err(|p| p.line())?;
    let before_count = forest.trees.len();
    if before_count == 0 {
        return Err("the forest holds no trees, so there are none to thin".to_owned());
    }
    let remove = ((before_count as f32 * fraction).round() as usize).clamp(1, before_count);
    // **Evenly spread**: tree `i` is taken when the running count of `remove` per `before_count` steps over a whole number at it, so exactly
    // `remove` trees go, the same on every run, and no one grove is cleared.
    let mut at = 0_usize;
    forest.trees.retain(|_| {
        let take = (at + 1) * remove / before_count > at * remove / before_count;
        at += 1;
        !take
    });
    let new = bevy_grimoire_core::yaml::to_string(&forest).map_err(|p| p.line())?;
    let lines = text.split_inclusive('\n').count();
    let rel = scene.relative(&path)?;
    scene.dispatch("file_edit", &json!({"path": rel, "edits": [{"at": {"lines": [1, lines]}, "value": new}]}))?;
    let now: crate::ops_forest::Forest =
        bevy_grimoire_core::yaml::from_str(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?, "forest").map_err(|p| p.line())?;
    Ok(Made {
        target: vec![format!("forest/{}", scene.map_name)],
        before: json!({"trees": before_count}),
        after: json!({"trees": now.trees.len(), "removed_fraction": 1.0 - now.trees.len() as f64 / before_count as f64}),
    })
}

fn terrain_file(scene: &Scene) -> PathBuf {
    scene.project.grimoire_dir.join("terrain").join(format!("{}.yaml", scene.map_name))
}

fn peak_of(field: &bevy_grimoire_core::heightfield::Heightfield) -> ((f32, f32), f32, f32) {
    let (mut at, mut top) = ((field.origin.0, field.origin.1), f32::NEG_INFINITY);
    for (i, &height) in field.heights.iter().enumerate() {
        if height > top {
            top = height;
            at = (field.origin.0 + (i % field.width) as f32 * field.cell_m, field.origin.1 + (i / field.width) as f32 * field.cell_m);
        }
    }
    let (low, _) = field.range();
    (at, top, low)
}

fn flatten_peak(scene: &mut Scene, fraction: f32) -> Result<Made, String> {
    let path = terrain_file(scene);
    let text = std::fs::read_to_string(&path).map_err(|_| format!("the map has no terrain (`{}` is not there), so there is no peak to flatten", scene.relative(&path).unwrap_or_default()))?;
    let field = bevy_grimoire_core::heightfield::Heightfield::parse(&text)?;
    let (at, top, low) = peak_of(&field);
    if top - low < PEAK_MIN_M {
        return Err(format!("the terrain's highest point stands {:.2} m above its lowest, under the {PEAK_MIN_M} m a peak needs, so there is no peak to flatten", top - low));
    }
    let target = top - fraction * (top - low);
    let before = json!({"x": at.0, "z": at.1, "height_m": top, "base_m": low});
    scene.dispatch(
        "terrain_stroke",
        &json!({
            "map": scene.map_name,
            "stroke": {"brush": {"Flatten": target}, "radius_m": FLATTEN_BRUSH_CELLS * field.cell_m, "strength": 1.0, "points": [[at.0, at.1]]},
        }),
    )?;
    let now = bevy_grimoire_core::heightfield::Heightfield::parse(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)?;
    let height = now.height_at(at.0, at.1);
    let after = json!({"x": at.0, "z": at.1, "height_m": height, "base_m": low, "flattened_fraction": f64::from(top - height) / f64::from(top - low)});
    Ok(Made { target: vec![format!("terrain/{}", scene.map_name)], before, after })
}

/// **Whether the engine's own measurement on the saved scene shows the defect at the size asked for.** A defect that applied and measures as
/// nothing is not a defect.
fn confirmed(defect: Defect, size: Size, made: &Made) -> Result<(), String> {
    let want = defect.magnitude(size);
    let num = |v: &Value, key: &str| v[key].as_f64().ok_or_else(|| format!("the engine's measurement holds no `{key}`"));
    let close = |got: f64, what: &str| {
        if (got - want).abs() <= MEASURE_TOLERANCE.max(want * 0.02) {
            Ok(())
        } else {
            Err(format!("the engine measures {what} {got:.4} on the saved scene, not the {want:.4} asked for, so the operation did not make this defect"))
        }
    };
    match defect {
        Defect::Float => close(num(&made.after, "gap_m")? - num(&made.before, "gap_m")?, "a gap of"),
        Defect::Clip => {
            if made.after["engine_overlap"] != json!(true) || num(&made.after, "overlap_volume_m3")? <= 0.0 {
                return Err(format!("the engine finds no overlap between the two pieces on the saved scene ({}), so nothing was clipped", made.after));
            }
            if made.before["engine_overlap"] == json!(true) {
                return Err("the two pieces already overlapped before the push, so the push is not what made the defect".to_owned());
            }
            Ok(())
        }
        Defect::OutsideArea => {
            if made.before["inside"] != json!(true) || made.after["inside"] != json!(false) {
                return Err("the area test does not show the piece going from inside its area to outside it".to_owned());
            }
            close(num(&made.after, "distance_outside_m")?, "a distance outside of")
        }
        Defect::DeleteNamed => {
            if made.after["present"] != json!(false) {
                return Err("the piece the request named is still on the saved map".to_owned());
            }
            Ok(())
        }
        Defect::TurnWrong => close(num(&made.after, "turned_deg")?, "a turn of"),
        Defect::ThinForest => close(num(&made.after, "removed_fraction")?, "a removed fraction of").or_else(|_| {
            // Whole trees only: the fraction is the nearest a count allows, at most one tree off.
            let trees = num(&made.before, "trees")?;
            if (num(&made.after, "removed_fraction")? - want).abs() <= 1.0 / trees + 1e-9 && num(&made.after, "removed_fraction")? > 0.0 {
                Ok(())
            } else {
                Err(format!("the saved forest holds {} of {trees} trees, which is not {want:.2} of them taken out", made.after["trees"]))
            }
        }),
        Defect::FlattenPeak => close(num(&made.after, "flattened_fraction")?, "a flattened fraction of"),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// Making a record.
// ---------------------------------------------------------------------------------------------------------------------------------------------

/// **What the caller draws the defective scene with**: given the scratch project's root and the map's name, the picture of it, or the reason there
/// is none.
pub type Render<'a> = &'a mut dyn FnMut(&Path, &str) -> Shot;

/// **One defect on one kept turn's scene**, as the record the ledger holds. Nothing is written: [`run`] does that.
///
/// `scratch_parent` is a folder this call makes its scratch project in and removes it from; it is never inside `project`.
pub fn perturb(turn: &Turn, project: &Path, scratch_parent: &Path, defect: Defect, size: Size, render: Render<'_>, serial: u32) -> Result<Perturbed, Refused> {
    let refuse = |why: String| Refused { turn: turn.id.clone(), defect, size, why };
    if !defect.sizes().contains(&size) {
        return Err(refuse(format!("a {} has no {} size", defect.name(), size.name())));
    }
    if scratch_parent.starts_with(project) {
        return Err(refuse(format!("the scratch folder {} is inside the project, which a perturbation never writes into", scratch_parent.display())));
    }
    if sha256_hex(project.to_string_lossy().as_bytes()) != turn.provenance.project_hash {
        return Err(refuse("this project is not the one the turn was made in (its path hashes differently)".to_owned()));
    }
    let Some(map_name) = turn.facts["map"]["name"].as_str() else {
        return Err(refuse("the turn's facts name no map, so there is no scene to perturb".to_owned()));
    };
    let name = format!("{}-{}-{}-{}", turn.id, defect.name(), size.name(), std::process::id());
    let scratch = Scratch::make(project, scratch_parent, &name).map_err(&refuse)?;
    let session = format!("labels-perturb-{serial}");
    let mut scene = Scene::open(&scratch.root, map_name, session).map_err(&refuse)?;
    as_the_turn_left_it(turn, &scene.project, scene.map()).map_err(&refuse)?;
    let made = make(&mut scene, turn, defect, size).map_err(&refuse)?;
    confirmed(defect, size, &made).map_err(&refuse)?;
    let operations = scene.operations();
    if operations.is_empty() {
        return Err(refuse("no operation was logged in the scratch project, so nothing was made".to_owned()));
    }
    let after = render(&scene.root, map_name);
    Ok(Perturbed {
        id: format!("perturbed-{}-{serial}", turn.handed_back_unix_ms),
        scene: scene_of(turn),
        source_turn: turn.id.clone(),
        at_unix_ms: now_ms(),
        defect: defect.name().to_owned(),
        size: size.name().to_owned(),
        magnitude: defect.magnitude(size),
        unit: defect.unit().to_owned(),
        target: made.target,
        tier: TIER_OBJECTIVE.to_owned(),
        label: LABEL_DEFECTIVE.to_owned(),
        before: turn.after.clone(),
        after,
        operations,
        facts: json!({"before": made.before, "after": made.after}),
        bench: turn.bench,
        provenance: Provenance {
            cauldron_version: env!("CARGO_PKG_VERSION").to_owned(),
            build_commit: crate::turn_ledger::BUILD_COMMIT.to_owned(),
            render: json!({"made_by": "perturb", "scratch": true}),
            agent: None,
            project_hash: turn.provenance.project_hash.clone(),
        },
    })
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// **What a run did.**
#[derive(Debug, Default)]
pub struct Report {
    /// The ids written.
    pub written: Vec<String>,
    /// Each defect that was not made, and why: never a record.
    pub refused: Vec<Refused>,
    /// **Kept turns that were passed over, and why** (a perturbed record, a benchmark turn, a turn the person undid).
    pub skipped: Vec<(String, String)>,
}

/// **The turns a person kept**: a gold `kept` label and no `undone` one, that are not themselves perturbed and not a benchmark's.
pub fn kept_turns(read: &Read) -> Vec<&Turn> {
    read.turns()
        .filter(|turn| !turn.bench)
        .filter(|turn| {
            let labels = || read.labels().filter(|label| label.turn == turn.id && label.tier == "gold");
            labels().any(|label| label.label == "kept") && !labels().any(|label| label.label == "undone")
        })
        .collect()
}

/// **Perturb every kept turn in the ledger, once per defect and size, and write what was made.**
///
/// A turn that already has a perturbed record of a defect and size is not made again, so a second run adds nothing. A defect that cannot be made
/// is in [`Report::refused`] with its sentence and writes nothing.
pub fn run(read: &Read, store: &mut Store, project: &Path, scratch_parent: &Path, render: Render<'_>) -> Result<Report, String> {
    let mut report = Report::default();
    let mut serial = read.perturbed().count() as u32;
    for turn in kept_turns(read) {
        for defect in Defect::ALL {
            for &size in defect.sizes() {
                if read.perturbed().any(|p| p.source_turn == turn.id && p.defect == defect.name() && p.size == size.name()) {
                    report.skipped.push((turn.id.clone(), format!("{} ({}) is already recorded", defect.name(), size.name())));
                    continue;
                }
                serial += 1;
                match perturb(turn, project, scratch_parent, defect, size, &mut *render, serial) {
                    Ok(record) => {
                        let id = record.id.clone();
                        store.append(&crate::turn_store::Line::perturbed(record))?;
                        report.written.push(id);
                    }
                    Err(refused) => report.refused.push(refused),
                }
            }
        }
    }
    Ok(report)
}

// ---------------------------------------------------------------------------------------------------------------------------------------------
// Real pictures: what `cauldron-negatives` needs to draw a scene, and a scene to draw without a turn.
// ---------------------------------------------------------------------------------------------------------------------------------------------

/// **A scene as if a turn had kept it, for a contact sheet** (DL-05, #464): the map as it is, and the words of a request to find a piece's name in.
///
/// **Never written to a ledger**: it is a turn nobody made. It has no operations and no pictures, so every piece of the map is a candidate for
/// a defect and the scene is, by construction, as the turn left it. The project is read, not written.
pub fn kept_turn_from_map(project: &Path, map_name: &str, request: &str) -> Result<Turn, String> {
    let opened = Project::open(project, None)?;
    let open = OpenMap::open(&opened, map_name)?;
    let why = "a contact sheet's scene was not made in a turn, so it has no picture of its own";
    Ok(Turn {
        id: format!("turn-sheet-{}", open.map.name),
        bench: false,
        started_unix_ms: 0,
        handed_back_unix_ms: 0,
        request: Request {
            hand_back: HandBackRef { kind: "presentation".to_owned(), id: "sheet".to_owned() },
            title: request.to_owned(),
            summary: request.to_owned(),
            specified: vec![request.to_owned()],
            inferred: Vec::new(),
            check: Vec::new(),
            note: None,
        },
        before: Shot::missing(why),
        after: Shot::missing(why),
        operations: Vec::new(),
        facts: json!({"map": {"name": open.map.name, "placements": open.map.placements.len()}, "operations": []}),
        decisions: Vec::new(),
        questions: Default::default(),
        provenance: Provenance {
            cauldron_version: env!("CARGO_PKG_VERSION").to_owned(),
            build_commit: crate::turn_ledger::BUILD_COMMIT.to_owned(),
            render: json!({"made_by": "perturb", "sheet": true}),
            agent: None,
            project_hash: sha256_hex(project.to_string_lossy().as_bytes()),
        },
    })
}

/// **The picture of the scene as it is**, drawn from a scratch copy so the person's project is never opened by an editor.
pub fn render_kept(project: &Path, scratch_parent: &Path, map_name: &str, render: Render<'_>) -> Result<Shot, String> {
    if scratch_parent.starts_with(project) {
        return Err(format!("the scratch folder {} is inside the project, which a perturbation never writes into", scratch_parent.display()));
    }
    let scratch = Scratch::make(project, scratch_parent, &format!("kept-{map_name}-{}", std::process::id()))?;
    Ok(render(&scratch.root, map_name))
}

/// **A PNG a renderer made, kept in the store as a picture**: its size, whether it is one flat colour, and the reason when it cannot be read.
pub fn shot_of_png(store: &mut Store, png: &[u8]) -> Shot {
    let decoded = match image::load_from_memory(png) {
        Ok(decoded) => decoded.to_rgba8(),
        Err(e) => return Shot::missing(format!("the renderer's picture did not decode: {e}")),
    };
    let mut pixels = decoded.as_raw().as_chunks::<4>().0.iter();
    let flat = pixels.next().is_none_or(|first| pixels.all(|pixel| pixel == first));
    match store.put_image(png, decoded.width(), decoded.height()) {
        Ok(stored) => Shot::taken(ImageRef { flat, ..stored }),
        Err(why) => Shot::missing(why),
    }
}
