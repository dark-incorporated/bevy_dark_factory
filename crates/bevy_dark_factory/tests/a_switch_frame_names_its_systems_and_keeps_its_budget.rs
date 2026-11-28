//! **A switch's frame names the systems it ran, and a switch's work stays inside the budget measured for it** (#678, #541).
//!
//! #678 split a switch's frame by schedule (`StateTransition`, `Update`, `PostUpdate`), and a schedule is not a system:
//! `StateTransition` alone is every `OnExit` and `OnEnter` of the editor. The frame now keeps the door's own systems,
//! timed by `profile::door_span` — the sweep (`screen::close_the_door`) and the spawners `OnEnter(Editor)` runs — and the
//! ui layout's two spans, and `blueprint/state.frame.systems` reports them.
//!
//! #541's ceiling is 0.1 s for a switch. A wall clock is the host's as much as the editor's (the Xvfb drive on big read
//! the same build at 70 and at 102 ms p50 as its load moved, `docs/loop/iterations/2026-11-27-frame-split/`), so what is
//! asserted here is the **work** a switch does: the UI nodes and texts it spawns. The budget is what was measured on
//! `fix/678-measure-main` (`d85e3f0`), whose drive read p50 63–85 ms: a second switch spawned at most 667–668 nodes and
//! 259–260 texts. A cut that keeps more lowers it; a change that makes a switch build more fails here before any clock reads it.

use std::collections::HashSet;

use bevy::prelude::*;

use bevy_dark_factory::harness;
use bevy_dark_factory::profile::{self, Profiler};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// Frames a switch is given to settle: the transition runs in the first, gooey's rows settle in the next few.
const SETTLE_FRAMES: usize = 10;

/// **The most UI nodes a second switch may spawn**: the largest measured (Animation → Map, 667–668) with room for a row.
const NODE_BUDGET: usize = 700;

/// **The most texts a second switch may spawn**: the largest measured (Animation → Map, 259–260) with room for a row.
const TEXT_BUDGET: usize = 280;

fn editor(tag: &str) -> App {
    let mut fixture = fixtures::Fixture::new(&format!("switch_budget_{tag}_{}", std::process::id()));
    for i in 0..8 {
        let id = format!("alpha/p{i}");
        fixture = fixture.flat_descriptor(&id, 1.0, 1.0).place_as(&format!("p{i}@0"), &id, (2.0 * i as f32, 0.0));
    }
    let root = fixture.build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    app
}

fn switch(app: &mut App, to: Workspace) {
    bevy_dark_factory::screen::cross(app.world_mut(), to).unwrap_or_else(|e| panic!("the switch to {}: {e}", to.title()));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert_eq!(app.world().resource::<Workspace>(), &to, "the switch to {} landed", to.title());
}

fn with<C: Component>(app: &mut App) -> HashSet<Entity> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<C>>();
    q.iter(world).collect()
}

/// **A switch's frame names the door's systems and the layout's**: the sweep, the frame's spawner and `bevy_ui`'s layout
/// are in the systems of the frame whose `StateTransition` ran the switch. `--no-capture` prints each switch's heaviest.
#[test]
fn a_switch_frame_names_the_systems_in_the_doors_path() {
    let mut app = editor("names");
    switch(&mut app, Workspace::Kit);
    switch(&mut app, Workspace::Map);
    for to in [Workspace::Kit, Workspace::Map, Workspace::World, Workspace::Map] {
        let start = app.world().resource::<Profiler>().last().map_or(0, |frame| frame.number);
        switch(&mut app, to);
        let profiler = app.world().resource::<Profiler>();
        let door = profiler
            .frames()
            .filter(|frame| frame.number > start)
            .find(|frame| frame.systems.iter().any(|span| span.system == "screen::close_the_door"))
            .unwrap_or_else(|| panic!("no frame after the switch to {} names screen::close_the_door", to.title()));
        let named: Vec<&str> = door.systems.iter().map(|span| span.system).collect();
        for system in ["screen::close_the_door", "chrome::spawn_frame", "chrome::spawn_status_band", profile::LAYOUT_SPAN] {
            assert!(named.contains(&system), "the switch to {} frame names {system}: it named {named:?}", to.title());
        }
        let sorted = door.systems.windows(2).all(|pair| pair[0].ms >= pair[1].ms);
        assert!(sorted, "the frame's systems are heaviest first: {:?}", door.systems);
        let top: Vec<String> = door.systems.iter().take(6).map(|span| format!("{} {:.2}", span.system, span.ms)).collect();
        println!("to {}: frame {} busy {:.1} ms; systems: {}", to.title(), door.number, door.busy_ms, top.join(", "));
    }
}

/// **A switch's work stays inside the budget measured for it**: no second switch spawns more than [`NODE_BUDGET`] UI
/// nodes or [`TEXT_BUDGET`] texts. The first round trip is not counted, so what a session builds once is not in it.
#[test]
fn a_switch_spawns_no_more_than_its_measured_budget() {
    let mut app = editor("budget");
    switch(&mut app, Workspace::Kit);
    switch(&mut app, Workspace::Map);
    for to in [Workspace::Kit, Workspace::Map, Workspace::World, Workspace::Animation, Workspace::Map] {
        let (nodes, texts) = (with::<Node>(&mut app), with::<Text>(&mut app));
        switch(&mut app, to);
        let new_nodes = with::<Node>(&mut app).difference(&nodes).count();
        let new_texts = with::<Text>(&mut app).difference(&texts).count();
        println!("to {}: {new_nodes} nodes and {new_texts} texts spawned", to.title());
        assert!(new_nodes <= NODE_BUDGET, "the switch to {} spawned {new_nodes} UI nodes, over the {NODE_BUDGET} budget", to.title());
        assert!(new_texts <= TEXT_BUDGET, "the switch to {} spawned {new_texts} texts, over the {TEXT_BUDGET} budget", to.title());
    }
}
