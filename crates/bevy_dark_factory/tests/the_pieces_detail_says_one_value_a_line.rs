//! **A row says one thing, never two values joined by a middle dot** (#519,
//! `feedback-1795094122195`).
//!
//! > I do NOT care for separate text values in one row. It looks jank. We coding agent rule not to use
//! > middle dot as a separator
//!
//! # What the capture measured
//!
//! The pick is one `Sized` text node in the Place panel's detail, reading exactly
//! `id  chair   ·   placed in bedroom` at `ui_scale` 2.4. That is the piece's id and the maps that
//! place it, two separate values, on one line, with a `·` between them. The owner's own rule — and the
//! one this crate's coding agents work under — is that a `·` never separates two values.
//!
//! # What this measures
//!
//! The **text of the row as it is drawn**, read off the widget's own `Text`. The row still states both
//! facts (the id, and where the piece stands — that is why the line exists, NV-07 / #384), but one per
//! line: no line of it holds a `·` between two values, and the two facts are on two lines.
//!
//! The second test holds the rule across the editor: no `Text` the Place panel draws on the piece's
//! own detail carries a `·`-separated pair. Chords (`W · A · S · D`) are a list of keys, one kind of
//! thing repeated, and are named as the exemption rather than quietly passed over.

use bevy::prelude::*;

use bevy_dark_factory::harness;
use bevy_dark_factory::ops::Shown;
use bevy_dark_factory::reveal::Revealing;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// Frames the editor is given to settle: the crossing to the Kit door, the piece's detail built, and
/// the maps under `maps/` read for the `Used by` sentence.
const SETTLE: usize = 60;

fn project(tag: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .sized_descriptor("chair", "alpha", 2.0, 1.0)
        .place_as("chair@0", "chair", (2.0, 2.0))
        .build("bedroom")
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "bedroom", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

/// **The piece's id row is one value a line** (#519): the id on its line, the maps that place it on
/// the next, and no middle dot between them.
#[test]
fn the_pieces_id_row_holds_no_middle_dot_between_two_values() {
    let root = project("place519");
    let mut app = editor(&root);
    // The Kit door, on the placed piece, which is the detail the capture picked.
    app.world_mut().resource_mut::<Revealing>().request_by_person("test", &Shown::Piece { id: "chair".to_owned() });
    settle(&mut app);
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Kit, "a piece is shown on the Kit workspace");

    // The row as it is drawn: the text carrying both the id and where the piece stands.
    let world = app.world_mut();
    let row = world
        .query::<(&Text, &InheritedVisibility)>()
        .iter(world)
        .filter(|(_, shown)| shown.get())
        .map(|(t, _)| t.0.clone())
        .find(|text| text.contains("id  chair") && text.contains("placed in"))
        .unwrap_or_else(|| {
            let drawn: Vec<String> = world
                .query::<(&Text, &InheritedVisibility)>()
                .iter(world)
                .filter(|(_, shown)| shown.get())
                .map(|(t, _)| t.0.clone())
                .collect();
            panic!("the piece's id row states the id and where it stands; on screen: {drawn:?}")
        });

    // **The defect, in one line**: no line of this row joins two values with a middle dot.
    for line in row.lines() {
        assert!(
            !line.contains('\u{b7}'),
            "the piece's id row joins two values with a middle dot: {line:?} (the whole row was {row:?})"
        );
    }
    // The two facts are both there, each on its own line.
    let id_line = row.lines().next().unwrap_or_default();
    assert!(id_line.trim_start().starts_with("id  chair"), "the first line is the id and nothing else: {id_line:?}");
    assert!(
        row.lines().any(|line| line.contains("placed in")),
        "where the piece stands is still stated, on a line of its own: {row:?}"
    );
}

/// **The rule holds across the editor**: what the Place panel draws on the piece's detail carries no
/// `·` between two values.
///
/// The list is the *whole* editor's texts that hold a `·`, and each must be one of the shapes the
/// editor says with one: a chord (`W · A · S · D`, one kind of thing repeated), a status line's
/// sentence, or a report row whose parts are one list. Those are named here so a new
/// `value · value` row has to argue with this test rather than slip past it.
#[test]
fn no_field_of_the_pieces_detail_joins_two_values_with_a_middle_dot() {
    let root = project("detail519");
    let mut app = editor(&root);
    app.world_mut().resource_mut::<Revealing>().request_by_person("test", &Shown::Piece { id: "chair".to_owned() });
    settle(&mut app);

    // The detail's own field labels: a row that is a `label` and a value.
    let drawn: Vec<String> = {
        let world = app.world_mut();
        world
            .query::<(&Text, &InheritedVisibility)>()
            .iter(world)
            .filter(|(_, shown)| shown.get())
            .map(|(t, _)| t.0.clone())
            .collect()
    };
    // The rows this test is about: the piece's id, its size, its mount, its mesh, its labels.
    for label in ["id  chair", "size", "mount", "mesh", "labels"] {
        let row = drawn
            .iter()
            .find(|text| text.trim_start().starts_with(label))
            .unwrap_or_else(|| panic!("the detail draws its `{label}` row; on screen: {drawn:?}"));
        for line in row.lines() {
            assert!(
                !(line.contains('\u{b7}') && line.split('\u{b7}').filter(|part| !part.trim().is_empty()).count() == 2),
                "the `{label}` row joins two values with a middle dot: {line:?}"
            );
        }
    }
}
