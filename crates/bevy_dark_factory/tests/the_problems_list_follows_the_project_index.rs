//! **The PROBLEMS list reads the relations file through the project index, and keeps no clock of its own** (#501, checklist item 9;
//! `problems_view::READ_SECS` is retired). The two halves every such test has are in `index_waits`. What the list says is checked against
//! relations the test wrote and a plain `std::fs` read of the file, and a moved piece shows on the frame it moves with no file read.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_dark_factory::problems_view::ProblemsList;
use bevy_dark_factory::project::OpenMap;
use bevy_dark_factory::project_index::MIN_SWEEP;
use bevy_dark_factory::world_lists;

use crate::fixtures;
use crate::index_waits::*;


/// Every placement faces +z. The chair at (2, 2) has its table behind it at (2, 0), so `Faces` is unmet; at (2, -2) the table is in front.
const BROKEN: &str = "{ of: chair, kind: !Faces table }";
const HOLDS: &str = "{ of: chair, kind: !Near { to: table, within: 3.0 } }";

fn relations_text(relations: &str) -> String {
    format!("version: 1\nrelations: [{relations}]\n")
}

fn problems_project(tag: &str, relations: &str) -> (PathBuf, PathBuf) {
    let root = fixtures::Fixture::new(&format!("w501c_problems_{tag}_{}", std::process::id()))
        .descriptor("site/chair", "alpha")
        .descriptor("site/table", "alpha")
        .place_as("chair", "site/chair", (2.0, 2.0))
        .place_as("table", "site/table", (2.0, 0.0))
        .build("m");
    let file = root.join("assets/blueprint/maps/m.relations.yaml");
    write_in_place(&file, &relations_text(relations));
    (root, file)
}

fn problem_rows(app: &mut App) -> Vec<String> {
    texts_under::<ProblemsList>(app).into_iter().filter(|t| t != "Go to").collect()
}

/// **The relations file is read when the index says it moved, and on no clock**, and **a moved piece shows with no file read at all**.
#[test]
fn the_problems_list_follows_the_relations_file_through_the_index() {
    let (root, file) = problems_project("follow", HOLDS);
    let mut app = stepped(&root);
    assert_eq!(problem_rows(&mut app), ["no problems"], "the map starts with every relation holding");
    sweep(&app, SWEEP_NEVER);

    // (1) The file is rewritten in place and nobody says so: no clock may show it.
    write_in_place(&file, &relations_text(BROKEN));
    assert!(std::fs::read_to_string(&file).unwrap_or_default().contains("Faces"), "the file on disk now holds the broken relation");
    pump_for(&mut app, LONGER_THAN_THE_OLD_CLOCKS);
    assert_eq!(problem_rows(&mut app), ["no problems"], "the list changed with no word from the index: a clock of its own is reading the file");

    // (2) The writer says it is done: the list shows it.
    announce(&root, &file);
    let shown = pump_until(&mut app, SHOWN_WITHIN, |app| problem_rows(app) == ["chair faces table"]);
    assert!(shown.is_some(), "the announced relations file was not shown within {SHOWN_WITHIN:?}: {:?}", problem_rows(&mut app));

    // (3) **A moved piece shows on the frame it moves, with no file read**: the table is in front of the chair now.
    app.world_mut().resource_mut::<OpenMap>().map.placements.iter_mut().find(|p| p.id == "chair").expect("the fixture places a chair").at = (2.0, -2.0);
    app.update();
    assert_eq!(problem_rows(&mut app), ["no problems"], "a piece moved and the list still lists the relation it mended");
    app.world_mut().resource_mut::<OpenMap>().map.placements.iter_mut().find(|p| p.id == "chair").expect("the fixture places a chair").at = (2.0, 2.0);
    app.update();
    assert_eq!(problem_rows(&mut app), ["chair faces table"], "the piece moved back and the relation is broken again");

    // (4) End to end through the index's own sweep, with no announcement: seen within the sweep and the debounce.
    sweep(&app, MIN_SWEEP);
    write_in_place(&file, &relations_text(HOLDS));
    let seen = pump_until(&mut app, SEEN_BY_A_FAST_SWEEP_WITHIN, |app| problem_rows(app) == ["no problems"]);
    assert!(seen.is_some(), "a saved relations file was not seen within {SEEN_BY_A_FAST_SWEEP_WITHIN:?} by the sweep: {:?}", problem_rows(&mut app));
}

/// **A relations file that does not read is said by name, never taken for no problems**, and mending it clears the line.
#[test]
fn a_relations_file_that_does_not_read_is_said_by_name() {
    let (root, file) = problems_project("broken", HOLDS);
    let mut app = stepped(&root);
    sweep(&app, SWEEP_NEVER);
    write_in_place(&file, "version: 1\nrelations: [\n");
    announce(&root, &file);
    let said = pump_until(&mut app, SHOWN_WITHIN, |app| problem_rows(app) == ["m.relations.yaml does not read"]);
    assert!(said.is_some(), "the unreadable file was not named within {SHOWN_WITHIN:?}: {:?}", problem_rows(&mut app));
    write_in_place(&file, &relations_text(HOLDS));
    announce(&root, &file);
    assert!(pump_until(&mut app, SHOWN_WITHIN, |app| problem_rows(app) == ["no problems"]).is_some(), "the mended file was not shown");
}

/// **A relations file the index does not list is said, and not read behind the index's back**: `.gitignore` names it, so the list cannot
/// follow it, and says so rather than `no problems` over a file that holds a broken relation.
#[test]
fn a_relations_file_the_index_does_not_list_is_refused_not_read() {
    let root = fixtures::Fixture::new(&format!("w501c_problems_ignored_{}", std::process::id()))
        .descriptor("site/chair", "alpha")
        .descriptor("site/table", "alpha")
        .place_as("chair", "site/chair", (2.0, 2.0))
        .place_as("table", "site/table", (2.0, 0.0))
        .build("m");
    let file = root.join("assets/blueprint/maps/m.relations.yaml");
    write_in_place(&file, &relations_text(BROKEN));
    write_in_place(&root.join(".gitignore"), "*.relations.yaml\n");
    let mut app = stepped(&root);
    let rows = problem_rows(&mut app);
    assert!(std::fs::read_to_string(&file).unwrap_or_default().contains("Faces"), "the ignored file does hold a broken relation");
    assert_eq!(rows.len(), 1, "one line is said: {rows:?}");
    assert!(rows[0].contains(".gitignore") && rows[0].contains("not followed"), "the list says the index cannot follow the file: {rows:?}");
}

/// **While the relations file is not there, a change to any other file reads nothing and judges nothing**: the file is not in the listing,
/// and "not there" stands until the listing's `.gitignore` or the file itself moves. Then the file arrives and is read once.
#[test]
fn an_unrelated_change_reads_nothing_and_judges_nothing_while_the_relations_file_is_not_there() {
    let root = fixtures::Fixture::new(&format!("w501c_problems_absent_{}", std::process::id()))
        .descriptor("site/chair", "alpha")
        .descriptor("site/table", "alpha")
        .place_as("chair", "site/chair", (2.0, 2.0))
        .place_as("table", "site/table", (2.0, 0.0))
        .build("m");
    let unrelated = root.join("notes/unrelated.yaml");
    std::fs::create_dir_all(root.join("notes")).unwrap_or_else(|e| panic!("{e}"));
    write_in_place(&unrelated, "note: first\n");
    let mut app = stepped(&root);
    assert_eq!(problem_rows(&mut app), ["no problems"]);
    sweep(&app, SWEEP_NEVER);
    let (reads, judged) = (world_lists::reads_on_this_thread(), world_lists::verdict_lookups_on_this_thread());

    let before = published(&app);
    write_in_place(&unrelated, "note: second, and longer\n");
    announce(&root, &unrelated);
    assert!(pump_until(&mut app, SHOWN_WITHIN, |app| published(app) > before).is_some(), "the index did not publish the unrelated change, so a count of nothing would prove nothing");
    pump_for(&mut app, std::time::Duration::from_millis(300));
    assert_eq!(world_lists::reads_on_this_thread(), reads, "a change to an unrelated file made the PROBLEMS list read a file");
    assert_eq!(world_lists::verdict_lookups_on_this_thread(), judged, "a change to an unrelated file made the PROBLEMS list judge a file that is not there again (it reads .gitignore)");

    let file = root.join("assets/blueprint/maps/m.relations.yaml");
    write_in_place(&file, &relations_text(BROKEN));
    announce(&root, &file);
    assert!(pump_until(&mut app, SHOWN_WITHIN, |app| problem_rows(app) == ["chair faces table"]).is_some(), "the relations file that arrived was not shown");
    assert_eq!(world_lists::reads_on_this_thread(), reads + 1, "the arrival of the file is one read");
}
