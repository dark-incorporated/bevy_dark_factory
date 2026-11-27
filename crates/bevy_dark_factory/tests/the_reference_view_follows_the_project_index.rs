//! **The reference-image view reads the project index, and keeps no clock of its own** (#501, checklist item 11;
//! `reference_view::RECHECK_SECS` is retired). The two halves every such test has are in `index_waits`. **A picture replaced in place** (the
//! same name, other bytes of the same length) **is read again, and only that one**: the bytes on disk and the dimensions in them are read
//! by the test itself, and a change to a file no board lists reads no picture (`ReferenceView::pictures_read`).

use std::path::Path;
use std::time::Duration;

use bevy::prelude::*;
use bevy_dark_factory::live::EditorReloads;
use bevy_dark_factory::ops::{self, Ctx};
use bevy_dark_factory::project::{OpenMap, Project};
use bevy_dark_factory::project_index::MIN_SWEEP;
use bevy_dark_factory::reference_view::ReferenceView;
use bevy_dark_factory::world_lists;
use blueprint_model::live::Outcome;
use serde_json::json;

use crate::fixtures;
use crate::index_waits::*;
use crate::settle_support::Frames;

/// How many frames the baseline look at both pictures may take, through the index's own (fast) sweep: ten seconds at [`FRAME_PAUSE`].
const BASELINE_WITHIN: Frames = Frames::covering(Duration::from_secs(10), FRAME_PAUSE);

const COURT: &str = "references/court.png";
const YARD: &str = "references/yard.png";
/// Listed on the board and never made.
const LATER: &str = "references/later.png";

/// A PNG header of this size: **always 29 bytes**, so two pictures of different dimensions are the same length.
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
    b.extend(w.to_be_bytes());
    b.extend(h.to_be_bytes());
    b.extend([8, 6, 0, 0, 0]);
    b
}

/// **The dimensions in the file's own bytes**, read by the test from the IHDR chunk: not by the header reader the view uses.
fn size_in_bytes(bytes: &[u8]) -> (u32, u32) {
    let at = |from: usize| u32::from_be_bytes([bytes[from], bytes[from + 1], bytes[from + 2], bytes[from + 3]]);
    (at(16), at(20))
}

fn reference_project(tag: &str) -> (App, Ctx) {
    let root = fixtures::Fixture::new(&format!("w501c_reference_{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    for folder in ["notes", "references"] {
        std::fs::create_dir_all(root.join(folder)).unwrap_or_else(|e| panic!("{folder}: {e}"));
    }
    write_in_place(&root.join("notes/unrelated.yaml"), "note: first\n");
    let mut app = stepped(&root);
    sweep(&app, MIN_SWEEP);
    let project = app.world().resource::<Project>().clone();
    let open = app.world().resource::<OpenMap>().clone();
    let mut ctx = Ctx { project: Some(project), open: Some(open), session: tag.to_owned(), ..Ctx::default() };
    for (file, bytes) in [(COURT, png(8, 8)), (YARD, png(4, 4))] {
        write_in_place_bytes(&root.join(file), &bytes);
        ops::dispatch(&mut ctx, "board_set", &json!({"action": "add", "path": file, "caption": "a picture"})).unwrap_or_else(|e| panic!("{e:?}"));
    }
    // **A picture the board lists that is not there**: the listing does not hold it, so each look has a verdict to reach for it.
    ops::dispatch(&mut ctx, "board_set", &json!({"action": "add", "path": LATER, "caption": "not made yet"})).unwrap_or_else(|e| panic!("{e:?}"));
    // **The baseline**: both pictures looked at once, by the index's own (fast) sweep.
    let baseline = pump_until(&mut app, BASELINE_WITHIN, |app| {
        let view = app.world().resource::<ReferenceView>();
        view.header_of(COURT).is_some() && view.header_of(YARD).is_some()
    });
    assert!(baseline.is_some(), "the view never looked at the pictures the boards list");
    sweep(&app, SWEEP_NEVER);
    (app, ctx)
}

fn write_in_place_bytes(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap_or_else(|e| panic!("{path:?}: {e}"));
}

fn dims(app: &App, picture: &str) -> Option<(u32, u32)> {
    app.world().resource::<ReferenceView>().header_of(picture).map(|h| (h.width, h.height))
}

fn pictures_read(app: &App) -> u64 {
    app.world().resource::<ReferenceView>().pictures_read()
}

fn decided_about(app: &App, picture: &str) -> bool {
    app.world().resource::<EditorReloads>().kinds().iter().any(|v| v.kind == "reference_images" && v.file == picture && v.outcome == Outcome::Refused)
}

/// **A picture replaced in place (the same name, new bytes of the same length) is read again, and only that one**: with no clock, and
/// with the file's own bytes as the ground truth.
#[test]
fn a_picture_replaced_in_place_is_read_again_when_the_index_says_so_and_a_change_elsewhere_reads_no_picture() {
    let (mut app, ctx) = reference_project("replaced");
    let root = ctx.project.as_ref().expect("a project").root.clone();
    let court = root.join(COURT);
    assert_eq!(dims(&app, COURT), Some((8, 8)));
    assert_eq!(dims(&app, YARD), Some((4, 4)));
    let read_for_the_baseline = pictures_read(&app);
    let judged = world_lists::verdict_lookups_on_this_thread();

    // **A change to a file no board lists moves the index and reads no picture.**
    let unrelated = root.join("notes/unrelated.yaml");
    let before = published(&app);
    write_in_place(&unrelated, "note: second, and longer\n");
    announce(&root, &unrelated);
    assert!(pump_until(&mut app, SHOWN_WITHIN, |app| published(app) > before).is_some(), "the index did not publish the unrelated change");
    pump_for(&mut app, AFTER_AN_UNRELATED_CHANGE);
    assert_eq!(pictures_read(&app), read_for_the_baseline, "a change to a file no board lists made the view read a picture");
    assert_eq!(
        world_lists::verdict_lookups_on_this_thread(),
        judged,
        "a change to a file no board lists made the view judge the picture that is not there again (it reads .gitignore)"
    );

    // **The replacement**: other bytes, the same length, the same name, in place.
    let old_bytes = std::fs::read(&court).unwrap_or_else(|e| panic!("{e}"));
    write_in_place_bytes(&court, &png(16, 16));
    let new_bytes = std::fs::read(&court).unwrap_or_else(|e| panic!("{e}"));
    assert_ne!(new_bytes, old_bytes, "the bytes on disk really are other bytes");
    assert_eq!(new_bytes.len(), old_bytes.len(), "and the same length, so only the modification time says the file moved");
    assert_eq!((size_in_bytes(&old_bytes), size_in_bytes(&new_bytes)), ((8, 8), (16, 16)), "read from the bytes, not from the view");

    // (1) No word from the index: no clock may show it.
    pump_for(&mut app, LONGER_THAN_THE_OLD_CLOCKS);
    assert_eq!(dims(&app, COURT), Some((8, 8)), "the view saw the replacement with no word from the index: a clock is reading the picture");
    assert_eq!(pictures_read(&app), read_for_the_baseline, "a picture was read with no word from the index");

    // (2) The writer says it is done.
    announce(&root, &court);
    let shown = pump_until(&mut app, SHOWN_WITHIN, |app| dims(app, COURT) == Some(size_in_bytes(&new_bytes)));
    assert!(shown.is_some(), "the replaced picture was not read again within {SHOWN_WITHIN:?}: {:?}", dims(&app, COURT));
    assert_eq!(pictures_read(&app), read_for_the_baseline + 1, "one replaced picture is one read: the other picture was not read again");
    assert_eq!(dims(&app, YARD), Some((4, 4)));
    let applied = app.world().resource::<EditorReloads>().kinds().iter().any(|v| v.kind == "reference_images" && v.file == COURT && v.outcome == Outcome::Applied);
    assert!(applied, "the edit is one `applied` verdict for the replaced picture: {:?}", app.world().resource::<EditorReloads>().kinds());
}

/// **A picture that stops being a picture is refused through the index, with no clock**, and mending it is applied.
#[test]
fn a_picture_that_stops_being_a_picture_is_refused_when_the_index_says_so() {
    let (mut app, ctx) = reference_project("refused");
    let root = ctx.project.as_ref().expect("a project").root.clone();
    let court = root.join(COURT);
    write_in_place_bytes(&court, b"a text where the picture was");
    pump_for(&mut app, LONGER_THAN_THE_OLD_CLOCKS);
    assert!(!decided_about(&app, COURT), "a refusal was decided with no word from the index");
    announce(&root, &court);
    let refused = pump_until(&mut app, SHOWN_WITHIN, |app| decided_about(app, COURT));
    assert!(refused.is_some(), "the picture that is no picture was not refused within {SHOWN_WITHIN:?}");
    assert_eq!(dims(&app, COURT), None, "a refused picture has no header");
}
