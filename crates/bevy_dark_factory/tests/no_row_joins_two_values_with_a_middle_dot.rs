//! **No row of the screens this pass touched joins two values with a middle dot** (#519,
//! `feedback-1795094122195`).
//!
//! > I do NOT care for separate text values in one row. It looks jank. We coding agent rule not to
//! > use middle dot as a separator
//!
//! # The distinction this file holds
//!
//! A `·` between **two field values** (`MAP · bedroom`, `Frame time 16.7 ms · memory 812 MB`) reads
//! as a janky table, and is what the owner objects to. A `·` between **clauses of a sentence** is
//! punctuation — a hint, a legend, a key list — and is left alone: `report.rs`'s hints,
//! `chrome.rs`'s shortcut line, `confirm.rs`'s key legend, `anim_stage.rs`'s two lines,
//! `editor.rs`'s trigger rows and `keys.rs`'s chords are all that kind and none is touched here.
//!
//! # The rows that are pinned, and left as they were
//!
//! These carried a `·` between two values and a pre-existing assertion pins the string verbatim.
//! Restating them is the owner's call, so this file measures the rest and leaves these named
//! rather than quietly skipped:
//!
//! - **The labels row** — `tiles.rs` (`confidence high · gemma-x · 2026-10-20`), pinned six times by
//!   `the_meshes_pane_says_how_sure_the_labels_are.rs`.
//! - **The agent card's rows and heading** — `omp connected · 22 tools`, `Agent self test · omp ·
//!   <model>`, `Agent connected: test-client 1.0 · 22 tools`, pinned by `terminal_agent.rs`'s own
//!   unit tests and by `an_agent_tab`/`an_agent_starts_with_the_model…`.
//! - **The version line** — `v0.3.4 · up to date`, pinned by three tests.
//! - **The Reload panel's lines** — `<file> · <answer>`, pinned by
//!   `a_file_says_what_the_game_made_of_its_edit.rs`, which also pins the trailing `·` before a
//!   refusal's time.
//! - **The palette's kit heading** — `furniture · prop`, pinned by `headless::palette_headings`.
//! - **The status band's counts** — `1 refused · 1 needs a restart`, pinned by
//!   `the_status_band_says_when_a_file_needs_attention.rs`.
//! - **The history's author** — `now: placed bed@1 · the Night Shift`, pinned by
//!   `a_change_to_the_map_reaches_the_lists_row_by_row.rs`.
//! - **The scripts panel's trigger rows** — `dawn · !Activity work · area`, pinned by
//!   `the_scripts_panel_lists_the_maps_triggers.rs`.

use std::path::PathBuf;

use bevy_dark_factory::agent_guide::GuideState;
use bevy_dark_factory::chrome;
use bevy_dark_factory::editor::{FrameTarget, palette_tooltip};
use bevy_dark_factory::follow::{FollowRequest, Following, LookChip, as_the_night_shift};
use bevy_dark_factory::game_view::{Snapshot, frame_and_memory};
use bevy_dark_factory::outliner::{Contents, Eyes, Locks, nodes_for};
use bevy_dark_factory::project::OpenMap;
use bevy_dark_factory::project_page::{GuideView, guide_status};
use bevy_dark_factory::workspace::Workspace;
use bevy_gooey::RowSource;
use blueprint_model::library::Library;
use blueprint_model::map::{Map, Placed};

/// The character the owner objects to between two values.
const DOT: char = '\u{b7}';

/// **Every line of `text` that joins exactly two values with a middle dot.**
///
/// Two values, not three or more: `W · A · S · D` and `history · problems · notes · log` are one
/// kind of thing repeated — a list, which the bar's own words call a list — and are not the shape
/// this issue names. A line with exactly two parts is one value beside another, which is.
fn two_values_in(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.contains(DOT))
        .filter(|line| line.split(DOT).filter(|part| !part.trim().is_empty()).count() == 2)
        .map(str::to_owned)
        .collect()
}

/// **The Agent guide's status line is one value a line** (#519): the state, then the path.
#[test]
fn the_agent_guides_status_line_puts_the_path_on_its_own_line() {
    let view = GuideView {
        state: GuideState::Allowed,
        trouble: None,
        text: "Do not run anything without asking.".to_owned(),
        problem: None,
    };
    let said = guide_status(&view, false);
    assert_eq!(
        said.lines().collect::<Vec<_>>(),
        vec!["allowed", blueprint_model::agent_guide::PROJECT_PATH],
        "the state and the path are two values and read as two lines: {said:?}"
    );
    assert!(two_values_in(&said).is_empty(), "no line joins two values: {said:?}");

    // A conflict is the state's own sentence, then where the file is.
    let conflict = guide_status(&view, true);
    assert!(conflict.starts_with("changed on disk\n"), "the state leads on its own line: {conflict:?}");
    assert!(two_values_in(&conflict).is_empty(), "{conflict:?}");

    // A problem that stops the decision says itself and nothing else.
    let troubled = GuideView { trouble: Some("the trust store will not read".to_owned()), ..view.clone() };
    let said = guide_status(&troubled, false);
    assert_eq!(said.lines().next(), Some("allowed"), "{said:?}");
    assert!(said.contains("the trust store will not read"), "{said:?}");
    assert!(two_values_in(&said).is_empty(), "{said:?}");
}

/// **A guide shown in part states each fact on its own line** (#519), and still says what part.
#[test]
fn a_partial_guides_status_line_joins_nothing_with_a_middle_dot() {
    let long = "a line of a long guide.\n".repeat(400);
    let said = guide_status(&GuideView { state: GuideState::Waiting, text: long, ..GuideView::default() }, false);
    assert!(said.starts_with("waiting for you\n"), "the state keeps the first line: {said:?}");
    assert!(said.contains("showing the first"), "it still says how much is shown: {said:?}");
    assert!(said.contains("edit the file at"), "and where to edit the rest: {said:?}");
    assert!(two_values_in(&said).is_empty(), "no line joins two values: {said:?}");
}

/// **The Game panel's frame time and memory are two readings, and read as two lines** (#519).
#[test]
fn the_games_frame_and_memory_are_two_lines() {
    let running = Snapshot { frame_ms: Some(16.7), rss_mb: Some(812.0), ..Snapshot::default() };
    assert_eq!(frame_and_memory(&running, true), "Frame time 16.7 ms\nmemory 812 MB");

    // A game that is not running has no readings; the not-yet case is said once, not twice.
    let idle = Snapshot::default();
    assert_eq!(frame_and_memory(&idle, false), "Frame time appears while the game runs");
    assert!(two_values_in(&frame_and_memory(&idle, true)).is_empty());

    // One read failing says its own line rather than hiding behind the other.
    let half = Snapshot { frame_ms: Some(8.0), memory_error: Some("ps is unavailable".to_owned()), ..Snapshot::default() };
    let said = frame_and_memory(&half, true);
    assert!(said.contains("memory unavailable: ps is unavailable"), "{said:?}");
    assert!(two_values_in(&said).is_empty(), "{said:?}");
}

/// **The chrome bar names the door and the subject as a label and its value** (#519).
#[test]
fn the_chrome_bar_does_not_join_the_door_and_the_subject_with_a_middle_dot() {
    assert_eq!(chrome::where_you_are(&Workspace::Map, None, None), "MAP");
    let open = OpenMap::new(Map { name: "bedroom".to_owned(), ..Map::default() }, PathBuf::from("maps/bedroom.map.yaml"));
    let said = chrome::where_you_are(&Workspace::Map, Some(&open), None);
    assert_eq!(said, "MAP: bedroom", "a label, a colon, and the value: {said:?}");
    assert!(!said.contains(DOT), "{said:?}");
}

/// **The look chip names the change, and the Look button beside it is not glued on with a dot**
/// (#519).
#[test]
fn the_look_chip_does_not_join_the_change_and_its_button_with_a_middle_dot() {
    let mut following = Following::default();
    following.request_named(FollowRequest { target: FrameTarget::Rect([0.0, 0.0, 1.0, 1.0]), ids: Vec::new() }, as_the_night_shift("placed `bed@0`"));
    let chip = LookChip::of(&following);
    let said = chip.sentence().expect("a change waits, so the chip says so").to_owned();
    assert!(said.starts_with(&as_the_night_shift("placed `bed@0`")), "{said:?}");
    assert!(!said.contains(DOT), "the chip's sentence carries no middle dot: {said:?}");
}

/// **A palette row's tooltip is the id, then the count on its own line** (#519).
#[test]
fn a_palette_tooltips_id_and_count_are_two_lines() {
    let said = palette_tooltip("wall", 1234);
    assert_eq!(said, "wall\n1,234 triangles");
    assert!(said.lines().next() == Some("wall"), "the id is what a person copies, alone on its line: {said:?}");
    assert!(two_values_in(&said).is_empty(), "{said:?}");
    // A piece whose mesh is not measured yet says only its id.
    assert_eq!(palette_tooltip("wall", 0), "wall");
}

/// **The outliner's piece, stamp and note rows hold no middle dot between two values** (#519).
#[test]
fn the_outliners_rows_hold_no_middle_dot_between_two_values() {
    let map = Map {
        name: "m".to_owned(),
        placements: vec![Placed { id: "chair@0".to_owned(), descriptor: "on_wall".to_owned(), ..Placed::default() }],
        ..Map::default()
    };
    let contents = Contents {
        compositions: vec![("arch".to_owned(), "the harbour gate".to_owned())],
        notes: vec![("note_one".to_owned(), "the light is wrong over here".to_owned())],
        ..Contents::default()
    };
    let library = Library {
        version: blueprint_model::library::LIBRARY_VERSION,
        note: None,
        descriptors: vec![blueprint_model::descriptor::Descriptor { id: "on_wall".to_owned(), ..Default::default() }],
    };
    let (nodes, _pieces, _children) = nodes_for(&map, &library, &contents, "", (&Eyes::default(), &Locks::default()), |_| Some(true));
    let tree = bevy_gooey::Tree::new(nodes);
    assert!(tree.len() != 0, "the tree has rows to measure");
    let labels: Vec<String> = (0..tree.len()).map(|row| tree.cell(row, 0).into_owned()).collect();
    for label in &labels {
        assert!(two_values_in(label).is_empty(), "a row joins two values with a middle dot: {label:?}");
    }
    // And the piece row still names both facts, one as the label and one as its value. The descriptor
    // is `chair`'s, looked up from the library; with no library the row names the id it fell back to.
    assert!(labels.iter().any(|label| label == "chair@0: on_wall"), "the piece row names its id and descriptor: {labels:?}");
}

/// **The project's servers row says the file's problem on its own line** (#519): the set and the
/// problem are two values.
#[test]
fn a_projects_servers_problem_is_not_joined_to_anything_with_a_middle_dot() {
    let root = std::env::temp_dir().join(format!("servers_row_519_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap_or_else(|e| panic!("{}: {e}", root.display()));
    std::fs::write(root.join(".mcp.json"), "{not json").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("mcp.json"), "{also not json").unwrap_or_else(|e| panic!("{e}"));
    let problem = bevy_dark_factory::agent_servers::project_file_problem(&root).expect("two broken files are two problems said");
    assert!(!problem.contains(DOT), "the problems are one a line, not joined: {problem:?}");
    assert_eq!(problem.lines().count(), 2, "each file's problem keeps a line: {problem:?}");
}