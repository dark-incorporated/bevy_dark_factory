//! A map's carved and added volume, saved beside the map as its edit list (#101, F5).
//!
//! Engine-free. The volume is not voxels: it is the list of SDF primitives the author added and
//! carved, in order, so it stays small and diffable and meshes at any resolution. [`Volume::sdf`]
//! evaluates the list over a base distance (the ground, for a tunnel through a hill), negative
//! inside solid; a mesher such as `bevy_isomesh` samples that function per chunk.

use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

/// A primitive. Lengths are metres in the map's frame, y up.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub enum Shape {
    Sphere { center: [f32; 3], radius_m: f32 },
    Box { center: [f32; 3], half_m: [f32; 3] },
    Capsule { a: [f32; 3], b: [f32; 3], radius_m: f32 },
}

/// Whether an edit makes solid or removes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Mode {
    Add,
    Carve,
}

/// One edit: a primitive, applied by union (`Add`) or subtraction (`Carve`), blended over
/// `smooth_m` (0 is a hard edge).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Edit {
    pub shape: Shape,
    pub mode: Mode,
    #[serde(default)]
    pub smooth_m: f32,
}

impl Edit {
    /// **The shape's kind, as the schema spells it** (`Sphere`, `Box`, `Capsule`): the same word a
    /// caller sends in `edit.shape`, so an answer naming what stood in an entry reads in the
    /// vocabulary the caller already writes and not in a second one.
    pub fn shape_name(&self) -> &'static str {
        match self.shape {
            Shape::Sphere { .. } => "Sphere",
            Shape::Box { .. } => "Box",
            Shape::Capsule { .. } => "Capsule",
        }
    }

    /// **Whether it makes solid or removes it**, in the words the schema's `mode` uses.
    pub fn mode_name(&self) -> &'static str {
        match self.mode {
            Mode::Add => "Add",
            Mode::Carve => "Carve",
        }
    }

    /// Refuses a non-finite number, a non-positive size or a negative blend.
    pub fn check(&self) -> Result<(), String> {
        let (nums, sizes): (Vec<f32>, Vec<f32>) = match self.shape {
            Shape::Sphere { center, radius_m } => (center.to_vec(), vec![radius_m]),
            Shape::Box { center, half_m } => (center.iter().chain(&half_m).copied().collect(), half_m.to_vec()),
            Shape::Capsule { a, b, radius_m } => (a.iter().chain(&b).copied().collect(), vec![radius_m]),
        };
        if nums.iter().chain(&[self.smooth_m]).any(|n| !n.is_finite()) {
            return Err("every number must be finite".to_owned());
        }
        if sizes.iter().any(|s| *s <= 0.0) {
            return Err("every radius and half-size must be above zero".to_owned());
        }
        if self.smooth_m < 0.0 {
            return Err("the blend must not be negative".to_owned());
        }
        Ok(())
    }

    /// Signed distance to the primitive (negative inside).
    pub fn distance(&self, p: [f32; 3]) -> f32 {
        match self.shape {
            Shape::Sphere { center, radius_m } => len(sub(p, center)) - radius_m,
            Shape::Box { center, half_m } => {
                let q = sub(p, center);
                let d = [q[0].abs() - half_m[0], q[1].abs() - half_m[1], q[2].abs() - half_m[2]];
                len([d[0].max(0.0), d[1].max(0.0), d[2].max(0.0)]) + d[0].max(d[1]).max(d[2]).min(0.0)
            }
            Shape::Capsule { a, b, radius_m } => {
                let ab = sub(b, a);
                let ap = sub(p, a);
                let t = (dot(ap, ab) / dot(ab, ab).max(f32::MIN_POSITIVE)).clamp(0.0, 1.0);
                len(sub(ap, [ab[0] * t, ab[1] * t, ab[2] * t])) - radius_m
            }
        }
    }
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn len(a: [f32; 3]) -> f32 {
    dot(a, a).sqrt()
}

/// Polynomial smooth minimum; `k == 0` is a plain `min`.
fn smin(a: f32, b: f32, k: f32) -> f32 {
    if k <= 0.0 {
        return a.min(b);
    }
    let h = (0.5 + 0.5 * (b - a) / k).clamp(0.0, 1.0);
    b + (a - b) * h - k * h * (1.0 - h)
}

/// The format version of a volume file (#202). A file with no `version` is the first.
pub const VOLUME_VERSION: u32 = 1;

/// Every edit of a map, in the order they were made.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Volume {
    /// The format version: [`VOLUME_VERSION`]. A file from before it existed has none and reads as the
    /// first; a file of another version is refused, naming the one this build reads.
    #[serde(default = "crate::format_version::first", deserialize_with = "crate::format_version::read::<_, VOLUME_VERSION>")]
    pub version: u32,
    #[serde(default)]
    pub edits: Vec<Edit>,
}

impl Default for Volume {
    fn default() -> Self {
        Volume { version: VOLUME_VERSION, edits: Vec::new() }
    }
}

impl Volume {
    pub fn is_empty(&self) -> bool {
        self.edits.is_empty()
    }

    /// **Every entry checked by the rule the reader holds a file to**, naming the first that fails by
    /// its index: `edit 2: every radius and half-size must be above zero`.
    ///
    /// **The one place that rule is applied to a list**, so a list written from `volume_set` cannot
    /// carry an entry the next read refuses, and a hand-written file cannot either: [`Volume::parse`]
    /// calls this, and so does the operation that sets a whole list (#474).
    pub fn check(&self) -> Result<(), String> {
        for (i, e) in self.edits.iter().enumerate() {
            e.check().map_err(|why| format!("edit {i}: {why}"))?;
        }
        Ok(())
    }

    /// **Read a volume from its file's text**, refusing an edit [`Edit::check`] would refuse (a number
    /// that is not finite, a size that is not above zero, a negative blend) and naming it (`edit 2`).
    /// A hand edit that reads as a list of edits but would feed the mesher a NaN is refused here.
    pub fn parse(text: &str) -> Result<Volume, String> {
        let volume: Volume = crate::yaml::from_str(text, "volume").map_err(|p| format!("volume: {}", p.line()))?;
        volume.check().map_err(|why| format!("volume: {why}"))?;
        Ok(volume)
    }

    /// Append one edit; a bad one is refused and nothing changes.
    pub fn push(&mut self, edit: Edit) -> Result<(), String> {
        edit.check()?;
        self.edits.push(edit);
        Ok(())
    }

    /// **Replace the edit at `at`, or refuse naming the index.** A bad edit is refused by the same
    /// rule [`Volume::push`] refuses one, so a replace cannot put in the file an entry the reader
    /// would refuse (#474). Returns the edit that stood there.
    pub fn replace(&mut self, at: usize, edit: Edit) -> Result<Edit, String> {
        if at >= self.edits.len() {
            return Err(format!("there is no edit at index {at}: the list holds {}.", self.edits.len()));
        }
        edit.check()?;
        Ok(std::mem::replace(&mut self.edits[at], edit))
    }

    /// **Take the edit at `at` out, or refuse naming the index.** Returns the edit removed, so a
    /// refusal to undo it can say what went.
    pub fn remove(&mut self, at: usize) -> Result<Edit, String> {
        if at >= self.edits.len() {
            return Err(format!("there is no edit at index {at}: the list holds {}.", self.edits.len()));
        }
        Ok(self.edits.remove(at))
    }

    /// **The signed distance at `p`** (negative inside solid) after every edit, in order, starting
    /// from `base`: the ground's distance (`p.y - height`) to carve into a hill, or `f32::INFINITY`
    /// for empty space.
    pub fn sdf(&self, p: [f32; 3], base: f32) -> f32 {
        self.sdf_before(self.edits.len(), p, base)
    }

    /// **The signed distance after the first `count` entries**, over `base`: the list as it stood
    /// partway through, which is how "what did this carve cut that was standing when it arrived" is
    /// asked of the same fold rather than a second implementation of it (#473).
    pub fn sdf_before(&self, count: usize, p: [f32; 3], base: f32) -> f32 {
        self.edits.iter().take(count).fold(base, |d, e| {
            let s = e.distance(p);
            match e.mode {
                Mode::Add => smin(d, s, e.smooth_m),
                Mode::Carve => -smin(-d, s, e.smooth_m),
            }
        })
    }

    /// **Whether the point is solid after every edit**, the ground's own distance before them
    /// (`base`): the `true`/`false` an agent asks for with `at:`. The same function the mesher samples,
    /// asked one question instead of a field of them, so a report and a picture cannot disagree.
    pub fn is_solid(&self, p: [f32; 3], base: f32) -> bool {
        self.sdf(p, base) <= 0.0
    }

    /// **The floor a piece rests on inside a cave** (#101): the height of the first solid met going
    /// down from `from_y` at world `(x, z)`, within `reach_m`. `base` is the distance before any
    /// edit (the ground's `p.y - height`, or `INFINITY` for empty space). `None` when `from_y` is
    /// itself inside solid (the start is not in the open) or nothing solid lies within reach. A march
    /// in 5 cm steps, then bisection, so the answer is exact to well under a millimetre.
    pub fn floor_below(&self, x: f32, z: f32, from_y: f32, reach_m: f32, base: &dyn Fn([f32; 3]) -> f32) -> Option<f32> {
        const STEP_M: f32 = 0.05;
        let d = |y: f32| self.sdf([x, y, z], base([x, y, z]));
        if !(from_y.is_finite() && reach_m.is_finite()) || d(from_y) <= 0.0 {
            return None;
        }
        let (mut above, mut y) = (from_y, from_y);
        while from_y - y < reach_m {
            y = (y - STEP_M).max(from_y - reach_m);
            if d(y) <= 0.0 {
                let mut below = y;
                for _ in 0..30 {
                    let mid = 0.5 * (above + below);
                    if d(mid) <= 0.0 {
                        below = mid;
                    } else {
                        above = mid;
                    }
                }
                return Some(above);
            }
            above = y;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sphere(mode: Mode, smooth_m: f32) -> Edit {
        Edit { shape: Shape::Sphere { center: [0.0, 0.0, 0.0], radius_m: 2.0 }, mode, smooth_m }
    }

    #[test]
    fn a_carved_sphere_opens_a_hole_in_the_ground_and_only_there() {
        let mut v = Volume::default();
        v.push(sphere(Mode::Carve, 0.0)).unwrap();
        let solid = |p: [f32; 3]| p[1]; // ground distance: rock for y < 0
        assert!(v.sdf([0.0, -1.0, 0.0], solid([0.0, -1.0, 0.0])) > 0.0, "inside the sphere the rock is gone");
        assert!(v.sdf([0.0, -4.0, 0.0], solid([0.0, -4.0, 0.0])) < 0.0, "beyond it the rock stands");
        assert_eq!(v.sdf([9.0, -1.0, 0.0], solid([9.0, -1.0, 0.0])), solid([9.0, -1.0, 0.0]));
    }

    #[test]
    fn add_then_carve_leaves_a_shell_and_primitives_measure_their_surface() {
        let mut v = Volume::default();
        v.push(Edit { shape: Shape::Box { center: [0.0; 3], half_m: [3.0; 3] }, mode: Mode::Add, smooth_m: 0.0 }).unwrap();
        v.push(sphere(Mode::Carve, 0.0)).unwrap();
        assert!(v.sdf([0.0; 3], f32::INFINITY) > 0.0, "hollow centre");
        assert!(v.sdf([2.5, 0.0, 0.0], f32::INFINITY) < 0.0, "wall between the hollow and the box face");
        let cap = Edit { shape: Shape::Capsule { a: [0.0; 3], b: [4.0, 0.0, 0.0], radius_m: 1.0 }, mode: Mode::Add, smooth_m: 0.0 };
        assert!((cap.distance([2.0, 3.0, 0.0]) - 2.0).abs() < 1e-5);
        assert!((cap.distance([6.0, 0.0, 0.0]) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn bad_edits_are_refused_and_change_nothing() {
        let mut v = Volume::default();
        let mut bad = sphere(Mode::Add, 0.0);
        bad.shape = Shape::Sphere { center: [0.0, f32::NAN, 0.0], radius_m: 1.0 };
        assert!(v.push(bad).is_err());
        assert!(v.push(Edit { shape: Shape::Sphere { center: [0.0; 3], radius_m: 0.0 }, mode: Mode::Add, smooth_m: 0.0 }).is_err());
        assert!(v.push(sphere(Mode::Add, -1.0)).is_err());
        assert!(v.is_empty());
    }

    #[test]
    fn a_smooth_carve_blends_beyond_the_hard_edge() {
        let mut hard = Volume::default();
        let mut soft = Volume::default();
        hard.push(sphere(Mode::Carve, 0.0)).unwrap();
        soft.push(sphere(Mode::Carve, 1.0)).unwrap();
        let p = [0.0, -2.2, 0.0];
        assert!(soft.sdf(p, -0.5) > hard.sdf(p, -0.5), "blending removes a little more rock around the cut");
    }
}
