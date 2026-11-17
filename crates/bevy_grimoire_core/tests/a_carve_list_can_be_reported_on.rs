//! **A carve list can be reported on**: which `Add`s a later `Carve` cut through, which voids are
//! separate, and whether a point is air (#473).
//!
//! **The lair the issue was written from**, in miniature. It is the same order that made three
//! defects invisible in one session: a crypt carved out of a hill, a dais and a column added **inside**
//! the carve, and a gallery carved **after** them — a carve removes whatever solid stands where it
//! passes, including an `Add` made earlier, and nothing said so.
//!
//! **The issue's own three `at:` points are here verbatim**, at the coordinates its acceptance block
//! names, on a lair laid out to those coordinates: the dais's top step is solid, the crown vault is
//! air, and the rerouted gallery is air.

use bevy_grimoire_core::volume::{Edit, Mode, Shape, Volume};
use bevy_grimoire_core::volume_report::{EmptySpace, Ground, VolumeReport};

/// **The crypt's own floor**, a flat rock at `y = 6`: `p.y - 6`, negative in rock, so the hill is
/// solid below it and open above. The base the lair is carved into.
struct RockAtSix;
impl Ground for RockAtSix {
    fn distance(&self, p: [f32; 3]) -> f32 {
        p[1] - 6.0
    }
}

fn edit(shape: Shape, mode: Mode) -> Edit {
    Edit { shape, mode, smooth_m: 0.0 }
}

fn box_at(center: [f32; 3], half_m: [f32; 3], mode: Mode) -> Edit {
    edit(Shape::Box { center, half_m }, mode)
}

fn sphere(center: [f32; 3], radius_m: f32, mode: Mode) -> Edit {
    edit(Shape::Sphere { center, radius_m }, mode)
}

fn capsule(a: [f32; 3], b: [f32; 3], radius_m: f32, mode: Mode) -> Edit {
    edit(Shape::Capsule { a, b, radius_m }, mode)
}

/// **The lair, at the issue's coordinates.** A crypt room, a dais with a top step, a column in the
/// crypt, a crown vault above it, and a gallery whose `x` the author moved twice — ending at 47.5,
/// which clips the dais's top step by 0.2 m without saying so.
fn lair() -> Volume {
    let mut v = Volume::default();
    // The crypt: hollowed between x 38..44, y 6..12, z 28..36, carved out of the rock at y 6.
    v.edits.push(box_at([41.0, 9.0, 32.0], [3.0, 3.0, 4.0], Mode::Carve));
    // **The dais**, an `Add` inside that carve: 0.8 m tall, its top step at y 6.8, wide enough that
    // the gallery at x 47.5 clips its east end.
    v.edits.push(box_at([40.0, 6.4, 31.9], [8.0, 0.4, 2.0], Mode::Add));
    // **The column**, an `Add` inside the crypt's east side: its top at y 16.
    v.edits.push(box_at([43.0, 11.0, 32.0], [0.4, 5.0, 0.4], Mode::Add));
    // **The crown vault**, air above the crypt: 19 m up and 8 m along z from it, sealed from the floor,
    // at the issue's own `at:` point [[40, 19, 40]].
    v.edits.push(sphere([40.0, 19.0, 40.0], 2.0, Mode::Carve));
    // **The gallery**, carved last, at x 47.5: the issue's third position, which clipped the dais's
    // top step by 0.2 m and was found only by a hand-written voxel sampler.
    v.edits.push(capsule([47.5, 9.0, 26.0], [47.5, 9.0, 34.0], 1.0, Mode::Carve));
    v
}

#[test]
fn the_three_at_checks_the_issue_names_answer_solid_or_air() {
    let v = lair();
    let report = v.report(&RockAtSix, &[[40.0, 6.5, 31.9], [40.0, 19.0, 40.0], [47.0, 9.0, 26.0]]);
    let at = &report.at;
    assert_eq!(at.len(), 3);
    // **The three checks verbatim.** The dais's top step is solid; the crown vault and the rerouted
    // gallery are air.
    assert!(at[0].solid, "at [[40, 6.5, 31.9]] the dais's top step is solid: {:?}", at[0]);
    assert!(at[0].containing.contains(&1), "and the entry that made it is named: {:?}", at[0].containing);
    assert!(!at[1].solid, "at [[40, 19, 40]] the crown vault is air: {:?}", at[1]);
    assert!(at[1].containing.contains(&3), "and the carve that opened it is named: {:?}", at[1].containing);
    assert!(!at[2].solid, "at [[47, 9, 26]] the rerouted gallery is air: {:?}", at[2]);
    assert!(at[2].containing.contains(&4), "and the carve that opened it is named: {:?}", at[2].containing);
}

/// **The report's own check, against a volume with the ground the lair is carved into.** The dais's top
/// step is a flat slab whose top face is a surface, and the check is measured to the top face rather
/// than assumed.
#[test]
fn the_at_answer_reads_the_same_surface_the_mesher_draws() {
    let v = lair();
    // One sample inside the dais's top step (its top at y 6.8) is solid, and one just above is air.
    let inside = v.report(&RockAtSix, &[[40.0, 6.5, 31.9]]);
    assert!(inside.at[0].solid, "6.5 m is inside the dais, whose top is 6.8 m: {:?}", inside.at[0]);
    let above = v.report(&RockAtSix, &[[40.0, 7.0, 31.9]]);
    assert!(!above.at[0].solid, "7.0 m is above the dais, and the crypt's void is open there: {:?}", above.at[0]);
}

#[test]
fn an_add_a_later_carve_cuts_through_is_named_with_how_much_went() {
    let mut v = Volume::default();
    // A column, added, then a gallery carved *after* it: the second defect in the issue's table.
    v.edits.push(box_at([0.0, 8.0, 0.0], [0.5, 4.0, 0.5], Mode::Add));
    v.edits.push(capsule([-8.0, 8.0, 0.0], [8.0, 8.0, 0.0], 1.0, Mode::Carve));
    let report = v.report(&EmptySpace, &[]);
    let cuts = &report.carves_that_cut_a_later_solid;
    assert_eq!(cuts.len(), 1, "the gallery cut the column: {cuts:?}");
    assert_eq!(cuts[0].solid, 0, "the add is named: {:?}", cuts[0]);
    assert_eq!(cuts[0].cut_by, 1, "and the carve after it: {:?}", cuts[0]);
    // **The value the report actually makes**, not merely one above zero: the gallery is a 2 m
    // cylinder along `x` through the column's whole 1 m `x` and `z` footprint, so what it took is a
    // 1 x 2 x 1 m block and the thinnest of those extents is 1 m.
    assert!(
        (cuts[0].overlapping_m - 1.0).abs() < 1e-4,
        "the gallery takes a 1 m block out of the 1 m column, so 1.0 m went: {:?}",
        cuts[0]
    );
    assert!(report.has_defects(), "an `Add` a later `Carve` eats is the defect the report exists for");
}

#[test]
fn an_add_a_later_carve_passes_beside_is_not_a_defect() {
    let mut v = Volume::default();
    v.edits.push(box_at([0.0, 8.0, 0.0], [0.5, 4.0, 0.5], Mode::Add));
    // 10 m to the side: the bounds meet nowhere, so this is not "cut through".
    v.edits.push(capsule([8.0, 8.0, 0.0], [16.0, 8.0, 0.0], 1.0, Mode::Carve));
    let report = v.report(&EmptySpace, &[]);
    assert!(report.carves_that_cut_a_later_solid.is_empty(), "nothing was cut: {:?}", report.carves_that_cut_a_later_solid);
    assert!(!report.has_defects());
}

#[test]
fn an_add_the_list_puts_back_after_the_carve_is_not_a_defect() {
    let mut v = Volume::default();
    v.edits.push(box_at([0.0, 8.0, 0.0], [0.5, 4.0, 0.5], Mode::Add));
    v.edits.push(capsule([-8.0, 8.0, 0.0], [8.0, 8.0, 0.0], 1.0, Mode::Carve));
    // The column is put back after the gallery: the order worked, and the column stands.
    v.edits.push(box_at([0.0, 8.0, 0.0], [0.5, 4.0, 0.5], Mode::Add));
    let ground = EmptySpace;
    let report = v.report(&ground, &[]);
    assert!(
        report.carves_that_cut_a_later_solid.is_empty(),
        "an `Add` the list re-adds after the carve is still standing, so nothing was lost: {:?}",
        report.carves_that_cut_a_later_solid
    );
    assert!(v.at([0.0, 8.0, 0.0], &ground).solid, "and the re-added column is solid at its middle");
}

#[test]
fn a_crown_vault_is_an_isolated_component_of_one_and_is_not_an_error() {
    let v = lair();
    let report = v.report(&RockAtSix, &[]);
    // The vault is 7 m above the crypt's roof and reaches nothing: its own region, of its own carve.
    let vault_region = report
        .components
        .iter()
        .find(|region| region.contains(&3))
        .unwrap_or_else(|| panic!("the vault carve is in a region of its own: {:?}", report.components));
    assert_eq!(vault_region, &vec![3], "a component of one: {vault_region:?}");
    assert!(
        !report.isolated.contains(&3),
        "**a crown vault is not isolated and not an error** — it is the crypt's ceiling void, \
         separate on purpose: {:?}",
        report.isolated
    );
    // And the crypt itself, with its dais and column, is a region of its own.
    assert!(
        report.components.iter().any(|region| region.contains(&0)),
        "the crypt is a region: {:?}",
        report.components
    );
}

#[test]
fn a_carve_a_later_add_fills_completely_is_isolated() {
    let mut v = Volume::default();
    // A small carve, then a box that covers it whole: the void is gone, so the carve reaches no room.
    v.edits.push(sphere([0.0, 0.0, 0.0], 1.0, Mode::Carve));
    v.edits.push(box_at([0.0, 0.0, 0.0], [4.0, 4.0, 4.0], Mode::Add));
    let report = v.report(&EmptySpace, &[]);
    assert_eq!(report.isolated, vec![0], "the carve reaches no open cell: {:?}", report.components);
    assert!(report.components.is_empty(), "and it left no region: {:?}", report.components);
}

#[test]
fn the_entries_are_named_with_their_kind_mode_and_reach() {
    let v = lair();
    let report = v.report(&RockAtSix, &[]);
    assert_eq!(report.entries.len(), 5);
    assert_eq!(report.entries[0].shape, "Box");
    assert_eq!(report.entries[0].mode, "Carve");
    assert_eq!(report.entries[1].shape, "Box");
    assert_eq!(report.entries[1].mode, "Add");
    assert_eq!(report.entries[4].shape, "Capsule");
    // The dais's box reaches its own half-extents about its centre.
    let b = report.entries[1].bounds;
    assert!((b.min_m[1] - 6.0).abs() < 1e-4, "the dais's bottom is 6.0 m: {b:?}");
    assert!((b.max_m[1] - 6.8).abs() < 1e-4, "and its top step is 6.8 m: {b:?}");
}

#[test]
fn a_report_changes_nothing_and_an_empty_volume_reports_nothing() {
    let v = Volume::default();
    let report: VolumeReport = v.report(&EmptySpace, &[[0.0; 3]]);
    assert!(report.entries.is_empty());
    assert!(report.components.is_empty());
    assert!(!report.has_defects());
    // The empty list is empty space, so the point asked is not solid.
    assert!(!report.at[0].solid);
    assert!(report.at[0].containing.is_empty());
}

/// **A cut thinner than the report's own cell is below its resolution, and the doc says so.**
///
/// The samples are the overlap box's own faces at [`SAMPLE_CELL_M`], so a 0.2 m clip is not a number
/// this report can make: the two faces it does sample sit exactly on the carve's and the solid's
/// boundaries, where a carve removes nothing (`distance == 0` is the surface, not the inside), so the
/// pair is not listed at all. **The resolution is the answer's limit, stated on
/// [`CutSolid::overlapping_m`]**, and this test pins that a caller gets no cut rather than a floored
/// `0.25` that reads as a real depth.
#[test]
fn a_cut_thinner_than_one_sample_is_not_reported_as_a_depth() {
    let mut v = Volume::default();
    // A dais 1 m across whose top step is at y 6.8, and a carve that shaves its top 0.2 m off.
    v.edits.push(box_at([0.0, 6.4, 0.0], [0.5, 0.4, 0.5], Mode::Add));
    v.edits.push(box_at([0.0, 6.9, 0.0], [2.0, 0.3, 2.0], Mode::Carve));
    let report = v.report(&EmptySpace, &[]);
    assert!(
        report.carves_that_cut_a_later_solid.is_empty(),
        "**a cut thinner than {} m is below the report's resolution**, so it names no depth rather \
         than a floored one: {:?}",
        bevy_grimoire_core::volume_report::SAMPLE_CELL_M,
        report.carves_that_cut_a_later_solid
    );
}
