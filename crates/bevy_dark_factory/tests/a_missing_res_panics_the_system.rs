//! **A system that takes `Res<T>` for a resource nobody inserted panics, and the panic names `T`** (#582).
//!
//! Nine comments in this tree state that rule, and until this test none of them could be checked in one step.
//! The mechanism, from the vendored 0.20 source: a missing `Res` yields
//! `SystemParamValidationError { invalid: true, skipped: false }`, and `system.rs` converts that to `Failed`
//! only when `skipped`; with the workspace's default `Severity::Panic`, a failed system panics naming the
//! parameter. If Bevy ever changes it to skip, this test fails and the comments become wrong — which is the point.

use bevy::prelude::*;

/// A resource no app in this file ever inserts.
#[derive(Resource)]
struct NeverInserted;

#[test]
#[should_panic(expected = "NeverInserted")]
fn a_system_that_takes_a_missing_res_panics_and_names_it() {
    // **The ordinary path a game takes**: a plugin's system is added and the schedule runs. No `run_system_once`,
    // no `Result` to swallow the failure — if the system were skipped this would pass and the rule would be false.
    let mut app = App::new();
    app.add_systems(Update, |_: Res<NeverInserted>| {});
    app.update();
}
