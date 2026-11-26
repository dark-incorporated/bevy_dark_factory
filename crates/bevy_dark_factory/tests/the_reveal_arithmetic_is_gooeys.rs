//! **`chrome::scroll_to_reveal` is gooey's arithmetic**, with this editor's margin and dead band on top
//! (#649).
//!
//! # What was wrong
//!
//! `chrome::scroll_to_reveal` re-implemented `bevy_gooey::scroll_to_reveal` (exported at
//! `src/lib.rs:360`, implemented at `src/model.rs:1287`) under the same name: the same clamp, the same
//! re-centre of a jump wider than one viewport, the same "a span taller than the viewport is read from
//! its top". The two agreed on all of that except the **context margin**, which gooey keeps in a private
//! constant (`CONTEXT_ROWS = 1.5`, `src/list.rs:24`) and the editor reads from its own sheet
//! (`layout.context_rows: 2.0`). Eleven call sites, twelve `chrome::Follow` state machines and ~400 lines
//! of test depended on the copy.
//!
//! # What this holds
//!
//! - the body calls gooey's function, and the local clamp/margin/centre arithmetic is gone (a source
//!   scan: a comment cannot be the proof);
//! - **the margin is the editor's input, not a constant inside the maths** — the mutation proof the
//!   issue asks for. A user layer sets `layout.context_rows: 0.5` and the same row, in the same place,
//!   asks for a different scroll than the shipped sheet's two rows do;
//! - the wrapper's own two concerns still hold: the half-pixel dead band (a correction under half a
//!   pixel is `None`, so a change-detected write does not re-fire layout every frame) and the
//!   physical→logical conversion (a 2× display halves the answer).

use bevy::prelude::*;

use bevy_dark_factory::chrome;
use bevy_dark_factory::harness;
use bevy_dark_factory::style;

use crate::fixtures;
use crate::settle_support;

/// `chrome::scroll_to_reveal`'s own body, from its signature to the function after it.
fn the_function_source() -> String {
    let source = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/chrome.rs"))
        .unwrap_or_else(|e| panic!("chrome.rs: {e}"));
    let from = source
        .find("pub fn scroll_to_reveal(")
        .unwrap_or_else(|| panic!("`scroll_to_reveal` is the function this file is about"));
    let body = &source[from..];
    let end = body
        .find("\npub fn scroll_bounds(")
        .unwrap_or_else(|| panic!("`scroll_bounds` follows `scroll_to_reveal`"));
    body[..end].to_owned()
}

/// **The body is gooey's**, and the editor's own copy of the clamp/margin/centre arithmetic is gone.
#[test]
fn the_reveal_arithmetic_is_gooeys_and_the_copy_is_deleted() {
    let body = the_function_source();
    assert!(
        body.contains("bevy_gooey::scroll_to_reveal("),
        "`chrome::scroll_to_reveal` must call `bevy_gooey::scroll_to_reveal`, the function the crate \
         exports for exactly this (#649)"
    );
    for (what, needle) in [
        ("the margin's own clamp to what the viewport can hold", "list_h - row_h) * 0.5"),
        ("the four-line above/below-the-fold branch", ") else if row_bottom + margin >"),
        ("the re-centre of a jump wider than a viewport", "delta.abs() > list_h"),
        ("the delta-then-scale arithmetic", "scroll_y + delta * inverse_scale"),
    ] {
        assert!(
            !body.contains(needle),
            "`scroll_to_reveal` still carries {what} ({needle:?}). gooey's `scroll_to_reveal` has done \
             this since the pinned rev (`src/model.rs:1287-1327`), and two copies of one rule is how they \
             drift (#649)"
        );
    }
}

/// The editor, with a user style layer that sets `layout.context_rows` to `rows`.
fn editor_with_context_rows(tag: &str, rows: f32) -> App {
    let root = fixtures::Fixture::new(&format!("scroll_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let config = root.join("user_config");
    std::fs::create_dir_all(&config).expect("a config folder");
    let user_style = config.join("style.yaml");
    std::fs::write(&user_style, format!("version: 1\nmetrics: {{layout: {{context_rows: {rows}}}}}\n")).expect("a layer");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(style::UserStyleFile(user_style));
    settle_support::settle(&mut app);
    app
}

/// **The margin is the editor's input.** One row of `context_rows` is worth less context than two, so the
/// same row flush against the fold asks for a smaller scroll — which cannot happen if the margin is a
/// constant inside the crate's maths.
///
/// Measured on the shipped `scroll_to_reveal` itself, not on a copy of its arithmetic: a 20 px row in a
/// 200 px fold, flush against the bottom edge, wants `2 × 20` of daylight at the sheet's 2.0 rows (40) and
/// `0.5 × 20` at 0.5 rows (10).
#[test]
fn the_context_margin_is_the_sheets_number_and_not_a_constant() {
    // A row spanning [280, 300] against a fold of [100, 300]: flush at the bottom, so a scroll is owed.
    let row = (290.0, 10.0);
    let list = (200.0, 100.0);
    let mut app = editor_with_context_rows("two_rows", 2.0);
    settle_support::settle(&mut app);
    assert_eq!(style::metrics().layout.context_rows, 2.0, "the layer's two rows are the running measure");
    let two_rows = chrome::scroll_to_reveal(row, list, 0.0, f32::INFINITY, 1.0);
    assert_eq!(
        two_rows,
        Some(40.0),
        "two rows of a 20 px row is 40 px of daylight: a rule this test's own numbers state, so the \
         assertion pins the margin rather than restating it"
    );

    let mut app = editor_with_context_rows("half_row", 0.5);
    settle_support::settle(&mut app);
    assert_eq!(style::metrics().layout.context_rows, 0.5, "the layer's half row is the running measure");
    let half_row = chrome::scroll_to_reveal(row, list, 0.0, f32::INFINITY, 1.0);
    assert_eq!(
        half_row,
        Some(10.0),
        "half a row is 10 px, so the margin followed the sheet and is not a constant inside gooey's maths \
         (gooey keeps 1.5 rows for its own lists, in a private constant, #649)"
    );

    // **The two differ, which is the mutation**: were the margin a constant, both calls would answer the same.
    assert_ne!(two_rows, half_row, "the sheet's number is what moved, so the margin is the editor's input");
}

/// **The dead band is the editor's own concern**, and it is still here: a correction under half a pixel is
/// noise, and answering `None` is what stops a float-jittering layout from re-marking `ScrollPosition`
/// changed every frame.
#[test]
fn a_sub_pixel_correction_is_still_swallowed_by_the_wrapper() {
    // A row bottom three tenths of a pixel past the comfort band of two rows.
    assert_eq!(
        chrome::scroll_to_reveal((250.3, 10.0), (200.0, 100.0), 0.0, f32::INFINITY, 1.0),
        None,
        "0.3 px is jitter, not a scroll"
    );
    // And a correction a person would see is answered.
    assert!(
        chrome::scroll_to_reveal((290.0, 10.0), (200.0, 100.0), 0.0, f32::INFINITY, 1.0).is_some(),
        "40 px is a scroll"
    );
}

/// **The physical→logical conversion is the wrapper's too**: a 2× display halves the correction, because
/// the row and the fold are measured in physical pixels and the scroll position is logical.
#[test]
fn the_wrapper_converts_physical_to_logical() {
    // 20 px of overshoot plus a 40 px margin is 60 physical, which is 30 logical on top of 40.
    assert_eq!(
        chrome::scroll_to_reveal((310.0, 10.0), (200.0, 100.0), 40.0, f32::INFINITY, 0.5),
        Some(70.0),
        "at `inverse_scale` 0.5 the same row asks for half the logical distance"
    );
}
