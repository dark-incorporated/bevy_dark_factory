//! **The AGENT block lists the agent's plan, each step ticked as the agent marks it done** (UI-60b, #340).
//!
//! The agent declares its steps with `plan_set` (`maps/<map>.plan.yaml`), the one source the inspector's PLAN section and
//! `blueprint_state.plan` already read. The block's PLAN row reads the same file, so the person looking at what the agent did
//! also sees what it means to do, and which steps are done.
//!
//! Owner exception (2026-11-14): `night_shift::PLAN_READ_SECS` is retired (#501): the row reads the plan when the project index says it moved,
//! which is within its debounce and one sweep, so six frames 150 ms apart are no longer a valid wait. The row is waited for with a
//! wall-clock timeout (`wait_for_row`) and every assertion on its words is unchanged.

use bevy::prelude::*;
use bevy_dark_factory::night_shift::{NightShiftItem, NightShiftPlanLine};
use bevy_dark_factory::harness;
use bevy_dark_factory::project::OpenMap;
use bevy_gooey::ValueChange;
use blueprint_model::plan::{self, Plan, PlanItem, Status};

use crate::fixtures;

fn editor(name: &str) -> App {
    let root = fixtures::Fixture::new(name).descriptor("table", "alpha").place_as("table@0", "table", (0.0, 0.0)).build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    show_the_block(&mut app);
    app
}

fn show_the_block(app: &mut App) {
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<NightShiftItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    settle(app);
}

/// Frames at least `PLAN_READ_SECS` of wall time apart, so the block has looked at the file again.
fn settle(app: &mut App) {
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        app.update();
    }
}

/// **Frames until the PLAN row says what the test expects**, for at most `ROW_TIMEOUT_SECS` of wall time; the row's words are then returned for the
/// assertions that follow.
fn wait_for_row(app: &mut App, expected: &str) -> String {
    let started = std::time::Instant::now();
    loop {
        app.update();
        let text = plan_row(app);
        if text.contains(expected) || started.elapsed().as_secs_f64() > ROW_TIMEOUT_SECS {
            return text;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// How long the row may take to follow a plan the index has to see (a sweep and a debounce, with a margin for a loaded machine).
const ROW_TIMEOUT_SECS: f64 = 6.0;

fn item(text: &str, status: Status) -> PlanItem {
    PlanItem { text: text.to_owned(), status }
}

fn plan_row(app: &mut App) -> String {
    let world = app.world_mut();
    world.query_filtered::<&Text, With<NightShiftPlanLine>>().single(world).expect("the block carries one PLAN row").0.clone()
}

fn write_the_plan(app: &App, items: Vec<PlanItem>) {
    let path = plan::path_for_map(&app.world().resource::<OpenMap>().map_path);
    Plan::new(items).unwrap().save(&path).unwrap();
}

#[test]
fn the_block_lists_each_step_and_ticks_it_when_it_is_done() {
    let mut app = editor(&format!("agent_block_plan_{}", std::process::id()));
    write_the_plan(&app, vec![item("lay the walls", Status::Done), item("roof", Status::Pending)]);
    let text = wait_for_row(&mut app, "2 [pending] roof");
    assert!(text.contains("1 [done] lay the walls"), "the first step, done: {text:?}");
    assert!(text.contains("2 [pending] roof"), "the second step, waiting: {text:?}");
    assert!(text.contains("plan: 1 of 2 done"), "the count: {text:?}");

    write_the_plan(&app, vec![item("lay the walls", Status::Done), item("roof", Status::Done)]);
    let text = wait_for_row(&mut app, "2 [done] roof");
    assert!(text.contains("2 [done] roof"), "the second step is ticked once the agent marks it: {text:?}");
    assert!(text.contains("plan: 2 of 2 done"), "the count follows: {text:?}");
}

#[test]
fn with_no_plan_file_the_row_says_so() {
    let mut app = editor(&format!("agent_block_no_plan_{}", std::process::id()));
    assert_eq!(plan_row(&mut app), "no plan");
}
