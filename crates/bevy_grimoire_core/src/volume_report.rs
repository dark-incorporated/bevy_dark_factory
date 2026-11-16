//! **What a map's carve list actually made** (#473): the order defects, the voids and their
//! connections, and whether a point is air.
//!
//! Engine-free, and it changes nothing: the report is a fact about the edit list, so a test needs no
//! `App` and the editor's `grimoire_volume_report` verb only parses parameters and serialises.
//!
//! # Why the list is the subject and not the mesh
//!
//! `volume_edit` takes one entry per call and answers `edited`, so the question a carve raises had no
//! answer anywhere: **does this `Add` still exist after the carves that follow it?** A carve removes
//! whatever solid stands where it passes, including an `Add` made earlier, and the entry list says so
//! only to a reader who simulates it. Three defects in one session of work were found exactly that
//! way — by hand, with a Python voxel sampler over the YAML.
//!
//! # The three answers
//!
//! - [`VolumeReport::carves_that_cut_a_later_solid`] — **an `Add` a `Carve` after it cuts through**,
//!   with how much of the added solid went;
//! - [`VolumeReport::components`] — the void split into connected regions, so a sealed room is one
//!   list of one, and [`VolumeReport::isolated`] names the carves reaching none. **An isolated
//!   component is not a fault**: a crypt's crown vault is the ceiling void, and the issue says it
//!   should not read as an error;
//! - [`VolumeReport::at`] — for each point asked, the ordered entries that contain it and whether the
//!   result is solid, the ground truth for "is this floor air?".

use serde::{Deserialize, Serialize};

use crate::volume::{Edit, Mode, Shape, Volume};

/// **How finely the volume is sampled**, in metres: the report's cell edge.
///
/// **One number, named, and it is the answer's resolution as well as its cost.** A report coarser
/// than the mesher's own cell (one metre) would miss the 0.2 m clip the issue names as its third
/// defect, and finer would put a 26-entry list through millions of distance evaluations for a voxel
/// nobody can see.
pub const SAMPLE_CELL_M: f32 = 0.25;

/// **An axis-aligned box in metres**, the shape a report names an entry's reach with.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aabb {
    pub min_m: [f32; 3],
    pub max_m: [f32; 3],
}

impl Aabb {
    /// **The box around a centre and a half-extent**, the two numbers every primitive is built from.
    pub fn around(center: [f32; 3], half_m: [f32; 3]) -> Aabb {
        Aabb {
            min_m: [center[0] - half_m[0], center[1] - half_m[1], center[2] - half_m[2]],
            max_m: [center[0] + half_m[0], center[1] + half_m[1], center[2] + half_m[2]],
        }
    }

    /// **The box the two share**, or `None` when they do not meet: the answer "could these two
    /// entries have touched at all", before a single sample is taken.
    pub fn intersection(&self, other: &Aabb) -> Option<Aabb> {
        let min_m = [self.min_m[0].max(other.min_m[0]), self.min_m[1].max(other.min_m[1]), self.min_m[2].max(other.min_m[2])];
        let max_m = [self.max_m[0].min(other.max_m[0]), self.max_m[1].min(other.max_m[1]), self.max_m[2].min(other.max_m[2])];
        (0..3).all(|i| min_m[i] <= max_m[i]).then_some(Aabb { min_m, max_m })
    }

    /// **The box holding every point given**, or `None` for an empty list.
    fn enclosing(points: &[[f32; 3]]) -> Option<Aabb> {
        let first = *points.first()?;
        Some(points.iter().skip(1).fold(Aabb { min_m: first, max_m: first }, |b, p| Aabb {
            min_m: [b.min_m[0].min(p[0]), b.min_m[1].min(p[1]), b.min_m[2].min(p[2])],
            max_m: [b.max_m[0].max(p[0]), b.max_m[1].max(p[1]), b.max_m[2].max(p[2])],
        }))
    }
}

impl Edit {
    /// **The box the primitive cannot reach beyond**: an entry's own extent, so two entries that
    /// cannot meet are settled by their boxes rather than by sampling the whole list.
    ///
    /// **The blend widens it**, because a smooth union moves the surface a little past the hard edge
    /// (the reason `a_smooth_carve_blends_beyond_the_hard_edge` holds), so a defect at the very rim of
    /// a blended carve is still found.
    pub fn bounds(&self) -> Aabb {
        let reach = match self.shape {
            Shape::Sphere { center, radius_m } => Aabb::around(center, [radius_m; 3]),
            Shape::Box { center, half_m } => Aabb::around(center, half_m),
            Shape::Capsule { a, b, radius_m } => {
                let center = [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5, (a[2] + b[2]) * 0.5];
                let half = [
                    ((a[0] - b[0]) * 0.5).abs() + radius_m,
                    ((a[1] - b[1]) * 0.5).abs() + radius_m,
                    ((a[2] - b[2]) * 0.5).abs() + radius_m,
                ];
                Aabb::around(center, half)
            }
        };
        let blend = self.smooth_m.max(0.0);
        Aabb {
            min_m: [reach.min_m[0] - blend, reach.min_m[1] - blend, reach.min_m[2] - blend],
            max_m: [reach.max_m[0] + blend, reach.max_m[1] + blend, reach.max_m[2] + blend],
        }
    }
}

/// **One entry, as the report names it**: its index, kind, mode and reach.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntryReport {
    pub index: usize,
    pub shape: String,
    pub mode: String,
    pub bounds: Aabb,
}

/// **An `Add` a `Carve` after it cuts through** (#473): the order defect, named.
///
/// **The direction is the acceptance block's, not the field name's literal reading.** The issue's own
/// table is three cases and every one of them is *an `Add` that a later `Carve` cuts through*: the
/// shortened north gallery still crossing the dais, the gallery at `x 48` cutting the `(47, 35)`
/// column, `x 47.5` clipping the dais's top step by 0.2 m. So `solid` is the `Add` and `cut_by` the
/// `Carve` that came after it.
///
/// **`overlapping_m` is the thinnest extent of what the carve took from that `Add` and left gone**, in
/// metres: a slab clipped off a dais's top step is 0.2 m, not the dais's width, because the number says
/// how much of the earlier solid went rather than how big it was.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CutSolid {
    /// The index of the `Add` a later `Carve` eats into.
    pub solid: usize,
    /// The index of the `Carve` that does it.
    pub cut_by: usize,
    /// **How deep the carve reaches into the added solid**, in metres.
    pub overlapping_m: f32,
}

/// **A point an agent asked about** (#473): the ordered entries that contain it, and the result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AtReport {
    /// The point, as it was asked for.
    pub at: [f32; 3],
    /// **The entries whose own primitive contains the point**, in list order — every one, not only the
    /// ones that last touched it, because the order is what the question is about.
    pub containing: Vec<usize>,
    /// **Whether the result is solid there**, after every entry in order: the ground truth answer to
    /// "is this floor air?".
    pub solid: bool,
}

/// **What a carve list made** (#473): the entries, the order defects, the voids, and the points asked.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VolumeReport {
    /// Every entry, in list order.
    pub entries: Vec<EntryReport>,
    /// **An `Add` a later `Carve` cuts through**, one per pair that does it.
    pub carves_that_cut_a_later_solid: Vec<CutSolid>,
    /// **The void split into connected regions**, each the carve indices that opened it, in list
    /// order. A sealed room is one list of one.
    pub components: Vec<Vec<usize>>,
    /// **The carves belonging to no region at all** — one a later `Add` filled completely. A crown
    /// vault is *a* region (of one) and so is not here: being separate is not being lost.
    pub isolated: Vec<usize>,
    /// **For each point asked, its answer**; empty when none was.
    pub at: Vec<AtReport>,
}

impl VolumeReport {
    /// **Whether anything in the report is a defect an author should fix.** An isolated component is
    /// not one — a crown vault is the intended shape of a crypt's ceiling.
    pub fn has_defects(&self) -> bool {
        !self.carves_that_cut_a_later_solid.is_empty()
    }
}

/// **The ground a sample starts from**: `p.y - height` at `(x, z)`, negative in rock, the same base
/// the mesher hands [`Volume::sdf`].
pub trait Ground {
    fn distance(&self, p: [f32; 3]) -> f32;
}

/// **No ground: empty space**, so only the entries themselves make solid. What a test that asks about
/// a plain list of boxes wants, and the base for a volume built of `Add`s alone.
pub struct EmptySpace;

impl Ground for EmptySpace {
    fn distance(&self, _p: [f32; 3]) -> f32 {
        f32::INFINITY
    }
}

/// **A heightfield ground**, `p.y - height_at(x, z)`: the rock a tunnel is carved into.
pub struct OverGround<'a>(pub &'a crate::heightfield::Heightfield);

impl Ground for OverGround<'_> {
    fn distance(&self, p: [f32; 3]) -> f32 {
        p[1] - self.0.height_at(p[0], p[2])
    }
}

impl Volume {
    /// **The report the issue asks for** (#473), over `ground`.
    ///
    /// **Every part is measured, not guessed.** The order defects are found by sampling the volume
    /// before and after the carve that follows an `Add`; the components by a flood fill of the void at
    /// [`SAMPLE_CELL_M`]; `at` by [`Volume::is_solid`] at the point asked, so the report and the
    /// picture come from one function.
    pub fn report(&self, ground: &dyn Ground, at: &[[f32; 3]]) -> VolumeReport {
        let entries: Vec<EntryReport> = self
            .edits
            .iter()
            .enumerate()
            .map(|(index, e)| EntryReport { index, shape: e.shape_name().to_owned(), mode: e.mode_name().to_owned(), bounds: e.bounds() })
            .collect();
        let carves_that_cut_a_later_solid = self.cuts(ground);
        let (components, isolated) = self.voids(ground);
        let at = at.iter().map(|p| self.at(*p, ground)).collect();
        VolumeReport { entries, carves_that_cut_a_later_solid, components, isolated, at }
    }

    /// **The point's own answer**: which entries contain it, and whether the result is solid.
    pub fn at(&self, p: [f32; 3], ground: &dyn Ground) -> AtReport {
        let containing = self.edits.iter().enumerate().filter(|(_, e)| e.distance(p) <= 0.0).map(|(i, _)| i).collect();
        AtReport { at: p, containing, solid: self.is_solid(p, ground.distance(p)) }
    }

    /// **Every `Add` a later `Carve` cuts through and leaves cut.**
    fn cuts(&self, ground: &dyn Ground) -> Vec<CutSolid> {
        let mut out = Vec::new();
        for (i, add) in self.edits.iter().enumerate() {
            if add.mode != Mode::Add {
                continue;
            }
            for (j, carve) in self.edits.iter().enumerate().skip(i + 1) {
                if carve.mode != Mode::Carve {
                    continue;
                }
                let Some(overlap) = add.bounds().intersection(&carve.bounds()) else { continue };
                if let Some(overlapping_m) = self.taken_from(j, &overlap, ground) {
                    out.push(CutSolid { solid: i, cut_by: j, overlapping_m });
                }
            }
        }
        out
    }

    /// **What the carve at `carve` took out of the solid standing in `overlap`, and left gone**, as the
    /// thinnest extent of the removed cells in metres; `None` when it took nothing that still stands
    /// removed.
    ///
    /// **"Still gone" is the issue's question, not "did this carve touch it".** A carve that takes an
    /// `Add` the list re-adds after it has lost the author nothing — the solid is there, from a later
    /// entry — so a cell must be open after the **whole** list for the cut to be a defect.
    fn taken_from(&self, carve: usize, overlap: &Aabb, ground: &dyn Ground) -> Option<f32> {
        let step = SAMPLE_CELL_M;
        let total = self.edits.len();
        let cells = |lo: f32, hi: f32| ((hi - lo) / step).ceil() as usize + 1;
        let (nx, ny, nz) = (
            cells(overlap.min_m[0], overlap.max_m[0]),
            cells(overlap.min_m[1], overlap.max_m[1]),
            cells(overlap.min_m[2], overlap.max_m[2]),
        );
        let mut removed = Vec::new();
        for x in 0..nx {
            for y in 0..ny {
                for z in 0..nz {
                    let p = [
                        (overlap.min_m[0] + x as f32 * step).min(overlap.max_m[0]),
                        (overlap.min_m[1] + y as f32 * step).min(overlap.max_m[1]),
                        (overlap.min_m[2] + z as f32 * step).min(overlap.max_m[2]),
                    ];
                    let base = ground.distance(p);
                    // Solid before this carve, open after it, **and still open after every entry**.
                    if self.sdf_before(carve, p, base) <= 0.0
                        && self.sdf_before(carve + 1, p, base) > 0.0
                        && self.sdf_before(total, p, base) > 0.0
                    {
                        removed.push(p);
                    }
                }
            }
        }
        let box_removed = Aabb::enclosing(&removed)?;
        // **The thinnest extent**, floored at one sample: a slab clipped off a top step is its own
        // thickness, not the width of the step it came from.
        let thin = (0..3).map(|i| box_removed.max_m[i] - box_removed.min_m[i]).fold(f32::INFINITY, f32::min);
        Some(thin.max(step).max(0.0))
    }

    /// **The void split into connected regions**, and the carves in none of them.
    ///
    /// **A flood fill over the open samples at [`SAMPLE_CELL_M`]**, 6-connected so two rooms meeting
    /// only at a corner are two rooms. Each region is the carve indices whose own primitive reaches a
    /// cell of it, in list order; a carve that reaches no open cell any region holds — one a later
    /// `Add` filled — is [`VolumeReport::isolated`].
    ///
    /// **A sealed void is its own region and stays listed**: the issue says the crown vault "should not
    /// be an error", not that it should be hidden.
    fn voids(&self, ground: &dyn Ground) -> (Vec<Vec<usize>>, Vec<usize>) {
        let carves: Vec<usize> = self.edits.iter().enumerate().filter(|(_, e)| e.mode == Mode::Carve).map(|(i, _)| i).collect();
        if carves.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let reach = carves
            .iter()
            .map(|&i| self.edits[i].bounds())
            .reduce(|a, b| Aabb {
                min_m: [a.min_m[0].min(b.min_m[0]), a.min_m[1].min(b.min_m[1]), a.min_m[2].min(b.min_m[2])],
                max_m: [a.max_m[0].max(b.max_m[0]), a.max_m[1].max(b.max_m[1]), a.max_m[2].max(b.max_m[2])],
            })
            .unwrap_or(Aabb { min_m: [0.0; 3], max_m: [0.0; 3] });
        let step = SAMPLE_CELL_M;
        let cells = |lo: f32, hi: f32| ((hi - lo) / step).ceil() as usize + 1;
        let (nx, ny, nz) = (cells(reach.min_m[0], reach.max_m[0]), cells(reach.min_m[1], reach.max_m[1]), cells(reach.min_m[2], reach.max_m[2]));
        let index = |x: usize, y: usize, z: usize| (z * ny + y) * nx + x;
        let point = |x: usize, y: usize, z: usize| {
            [
                (reach.min_m[0] + x as f32 * step).min(reach.max_m[0]),
                (reach.min_m[1] + y as f32 * step).min(reach.max_m[1]),
                (reach.min_m[2] + z as f32 * step).min(reach.max_m[2]),
            ]
        };
        let total = nx * ny * nz;
        // **A cell is the void** when the volume is air there and some carve's own primitive reaches
        // it: the ground is left out of it, because the report is about the carves and their voids.
        let mut open = vec![false; total];
        for z in 0..nz {
            for y in 0..ny {
                for x in 0..nx {
                    let p = point(x, y, z);
                    if self.sdf(p, ground.distance(p)) > 0.0 && carves.iter().any(|&i| self.edits[i].distance(p) <= 0.0) {
                        open[index(x, y, z)] = true;
                    }
                }
            }
        }
        let mut seen = vec![false; total];
        let mut components: Vec<Vec<usize>> = Vec::new();
        let mut touched = vec![false; self.edits.len()];
        for start in 0..total {
            if !open[start] || seen[start] {
                continue;
            }
            seen[start] = true;
            let mut stack = vec![start];
            let mut region_cells = Vec::new();
            while let Some(c) = stack.pop() {
                region_cells.push(c);
                let (x, y, z) = (c % nx, (c / nx) % ny, c / (nx * ny));
                for (nx_, ny_, nz_) in [
                    (x.wrapping_sub(1), y, z),
                    (x + 1, y, z),
                    (x, y.wrapping_sub(1), z),
                    (x, y + 1, z),
                    (x, y, z.wrapping_sub(1)),
                    (x, y, z + 1),
                ] {
                    if nx_ >= nx || ny_ >= ny || nz_ >= nz {
                        continue;
                    }
                    let n = index(nx_, ny_, nz_);
                    if open[n] && !seen[n] {
                        seen[n] = true;
                        stack.push(n);
                    }
                }
            }
            let mut region_carves = Vec::new();
            for &i in &carves {
                let edit = &self.edits[i];
                if region_cells.iter().any(|&cell| {
                    let (x, y, z) = (cell % nx, (cell / nx) % ny, cell / (nx * ny));
                    edit.distance(point(x, y, z)) <= 0.0
                }) {
                    region_carves.push(i);
                    touched[i] = true;
                }
            }
            if !region_carves.is_empty() {
                components.push(region_carves);
            }
        }
        let isolated = carves.into_iter().filter(|&i| !touched[i]).collect();
        (components, isolated)
    }
}
