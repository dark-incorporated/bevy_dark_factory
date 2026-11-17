//! **A report is bounded, and its refusal names the limit** (§5 of the #473 review).
//!
//! The void's flood fill runs synchronously in the editor's update, so its cost is a held frame: a
//! 60 m carve list is ~13.8 M cells at [`SAMPLE_CELL_M`], each costing one distance evaluation per
//! carve. A report that paid that silently would stall the editor for seconds in a verb an agent calls
//! in a loop, so a volume whose reach is over [`MAX_VOID_CELLS`] is **refused, naming the limit**,
//! rather than measured.
//!
//! **The limit is on the work, not on the carving**: a list within it reports exactly what
//! [`Volume::report`] reports, so the bound cannot change an answer it allows.

use blueprint_model::volume::{Edit, Mode, Shape, Volume};
use blueprint_model::volume_report::{EmptySpace, MAX_VOID_CELLS};

fn carve(shape: Shape) -> Edit {
    Edit { shape, mode: Mode::Carve, smooth_m: 0.0 }
}

#[test]
fn a_carve_list_reaching_further_than_the_limit_is_refused_naming_it() {
    let mut v = Volume::default();
    // 60 m on every axis: 241 samples an axis at the report's 0.25 m cell, ~14 M cells.
    v.edits.push(carve(Shape::Box { center: [0.0, 30.0, 0.0], half_m: [30.0; 3] }));
    let cells = v.void_cells();
    assert!(cells > MAX_VOID_CELLS, "a 60 m carve list is over the limit: {cells} cells");
    let refused = v.report_within_budget(&EmptySpace, &[]).expect_err("over the limit, the report is refused, not paid");
    assert!(
        refused.contains(&MAX_VOID_CELLS.to_string()),
        "**the refusal names the limit**, so a caller knows what to shrink: {refused}"
    );
    assert!(refused.contains(&cells.to_string()), "and the work the list would have cost: {refused}");
    assert!(refused.contains("Nothing was changed"), "and that the refusal wrote nothing: {refused}");
}

#[test]
fn a_list_within_the_limit_reports_exactly_what_the_unbounded_report_does() {
    let mut v = Volume::default();
    v.edits.push(carve(Shape::Sphere { center: [0.0, 0.0, 0.0], radius_m: 2.0 }));
    assert!(v.void_cells() <= MAX_VOID_CELLS, "a 2 m sphere is a handful of cells: {}", v.void_cells());
    let at = [[0.0; 3], [3.0, 0.0, 0.0]];
    assert_eq!(
        v.report_within_budget(&EmptySpace, &at).expect("within the limit"),
        v.report(&EmptySpace, &at),
        "the bound refuses work; it never changes an answer it allows"
    );
}

#[test]
fn a_list_that_carves_nothing_costs_nothing() {
    let mut v = Volume::default();
    v.edits.push(Edit { shape: Shape::Box { center: [0.0; 3], half_m: [1.0; 3] }, mode: Mode::Add, smooth_m: 0.0 });
    assert_eq!(v.void_cells(), 0, "there is no void to flood-fill, so the reach is empty");
    assert!(v.report_within_budget(&EmptySpace, &[]).is_ok(), "and the report is not refused");
}
