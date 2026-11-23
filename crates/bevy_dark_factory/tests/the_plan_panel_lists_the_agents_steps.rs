//! **The Plan panel draws the agent's plan beside the map, one numbered row per step with its
//! status** (E-069, #134), follows the plan file as it changes, and says so when there is none.

use bevy::prelude::*;
use blueprint_model::plan::{self, Plan, PlanItem, Status};
use bevy_dark_factory::project::OpenMap;

use crate::fixtures;

/// **The project index at its shortest sweep, with its first listing in hand** — what a wait on a condition needs, so it is the
/// condition that is paid for and not the index's one-second default sweep.
fn index_at_its_shortest_sweep(app: &mut App) {
    let index = app.world().resource::<bevy_dark_factory::project_index::ProjectIndex>();
    index.set_sweep(bevy_dark_factory::project_index::MIN_SWEEP);
    index.wait_ready(std::time::Duration::from_secs(30));
}
use crate::index_view_support as support;



/// The plan panel's lines: the step rows (`<n> [<status>] <text>`) and its empty line.
fn shown(app: &mut App) -> Vec<String> {
    let mut lines: Vec<String> = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .map(|t| t.0.clone())
        .filter(|t| t == "no plan" || t.starts_with("plan:") || t.split_once(" [").is_some_and(|(n, _)| n.parse::<usize>().is_ok()))
        .collect();
    lines.sort();
    lines
}

fn item(text: &str, status: Status) -> PlanItem {
    PlanItem { text: text.to_owned(), status }
}

#[test]
fn the_panel_lists_the_plan_in_order_and_follows_it() {
    let root = fixtures::Fixture::new(&format!("plan_panel_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));

    // The index's shortest sweep, and the first listing waited for: what follows is a wait on the condition, not on a clock.
    index_at_its_shortest_sweep(&mut app);    support::settle(&mut app);
    assert_eq!(shown(&mut app), ["no plan"], "with no plan file the panel says so");

    let path = plan::path_for_map(&app.world().resource::<OpenMap>().map_path);
    Plan::new(vec![item("lay the walls", Status::Done), item("add a gate", Status::Doing), item("plant trees", Status::Pending)])
        .unwrap()
        .save(&path)
        .unwrap();
    support::settle(&mut app);
    assert_eq!(
        shown(&mut app),
        ["1 [done] lay the walls", "2 [doing] add a gate", "3 [pending] plant trees", "plan: 1 of 3 done"],
        "every step is drawn, numbered in the agent's order, with its status and a count"
    );

    Plan::new(vec![item("lay the walls", Status::Done), item("add a gate", Status::Done), item("plant trees", Status::Skipped)])
        .unwrap()
        .save(&path)
        .unwrap();
    support::settle(&mut app);
    assert_eq!(
        shown(&mut app),
        ["1 [done] lay the walls", "2 [done] add a gate", "3 [skipped] plant trees", "plan: 2 of 3 done"],
        "a changed plan is redrawn"
    );

    Plan::new(Vec::new()).unwrap().save(&path).unwrap();
    support::settle(&mut app);
    assert_eq!(shown(&mut app), ["no plan"], "a cleared plan leaves the panel empty again");
}
