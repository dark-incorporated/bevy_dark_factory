//! **The AGENT block's PLAN row reads the plan file through the project index, and keeps no clock of its own** (#501, checklist item 14;
//! `night_shift::PLAN_READ_SECS` is retired). The two halves every such test has are in `index_waits`. A plan that does not read is said by
//! its own reason, never `no plan`, and a plan taken away is `no plan`.

use std::path::PathBuf;
use std::time::Duration;

use bevy::prelude::*;
use bevy_dark_factory::night_shift::{NightShiftItem, NightShiftPlanLine};
use bevy_gooey::ValueChange;

use crate::fixtures;
use crate::index_waits::*;


fn plan_project(tag: &str) -> (App, PathBuf, PathBuf) {
    let root = fixtures::Fixture::new(&format!("w501c_plan_{tag}_{}", std::process::id())).descriptor("table", "alpha").place_as("table@0", "table", (0.0, 0.0)).build("m");
    let plan = root.join("assets/blueprint/maps/m.plan.yaml");
    write_in_place(&plan, PLAN_OF_ONE);
    let mut app = stepped(&root);
    let row = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<NightShiftItem>>().single(world).expect("the View menu carries one Show agent row")
    };
    app.world_mut().commands().trigger(ValueChange { source: row, value: true, is_final: true });
    pump_for(&mut app, Duration::from_millis(300));
    (app, root, plan)
}

const PLAN_OF_ONE: &str = "version: 1\nitems:\n- text: lay the walls\n  status: done\n";
const PLAN_OF_TWO: &str = "version: 1\nitems:\n- text: lay the walls\n  status: done\n- text: roof\n  status: pending\n";

fn plan_row(app: &mut App) -> String {
    let world = app.world_mut();
    world.query_filtered::<&Text, With<NightShiftPlanLine>>().single(world).expect("the block carries one PLAN row").0.clone()
}

/// **The PLAN row reads the plan file when the index says it moved, and on no clock.**
#[test]
fn the_plan_row_follows_the_plan_file_through_the_index() {
    let (mut app, root, plan) = plan_project("follow");
    let row = plan_row(&mut app);
    assert!(row.contains("1 [done] lay the walls") && row.contains("plan: 1 of 1 done"), "the plan on disk, as the block says it: {row:?}");
    sweep(&app, SWEEP_NEVER);

    write_in_place(&plan, PLAN_OF_TWO);
    let steps_on_disk = std::fs::read_to_string(&plan).unwrap_or_default().matches("- text:").count();
    assert_eq!(steps_on_disk, 2);
    pump_for(&mut app, LONGER_THAN_THE_OLD_CLOCKS);
    assert!(plan_row(&mut app).contains("plan: 1 of 1 done"), "the PLAN row changed with no word from the index: a clock is reading the file");

    announce(&root, &plan);
    let shown = pump_until(&mut app, SHOWN_WITHIN, |app| plan_row(app).contains(&format!("plan: 1 of {steps_on_disk} done")));
    assert!(shown.is_some(), "the announced plan was not shown within {SHOWN_WITHIN:?}: {:?}", plan_row(&mut app));
    assert!(plan_row(&mut app).contains("2 [pending] roof"));

    // A plan that does not read is said by its own reason, never `no plan`.
    write_in_place(&plan, "version: 1\nitems: [\n");
    announce(&root, &plan);
    let said = pump_until(&mut app, SHOWN_WITHIN, |app| plan_row(app).contains("m.plan.yaml"));
    assert!(said.is_some(), "the unreadable plan was not named within {SHOWN_WITHIN:?}: {:?}", plan_row(&mut app));
    assert_ne!(plan_row(&mut app), "no plan", "a plan that does not read is not no plan");

    // And a plan taken away is `no plan`.
    std::fs::remove_file(&plan).unwrap_or_else(|e| panic!("{e}"));
    announce(&root, &plan);
    assert!(pump_until(&mut app, SHOWN_WITHIN, |app| plan_row(app) == "no plan").is_some(), "a removed plan was not shown as no plan: {:?}", plan_row(&mut app));
}
