//! **A workspace switch keeps the menu bar and builds what the workspace changes** (#678, #541).
//!
//! After the scan and portrait cuts, what a switch still paid on every arrival was `Update` and `PostUpdate`: the new
//! frame's nodes styled, laid out and their texts measured. Counted headless on `integrate/1127` (`a6c832b0`), a second
//! Map → Kit switch spawned **1,416 UI nodes and 600 texts, and kept none**; **913 of those nodes and 419 of the texts were
//! the menu bar**, `menu_bar::EditorMenuBar` — every command of the registry in eleven menus, closed, and the same bar on
//! every workspace. The rest is the frame and the panels the workspace shows. Map: 1,618 new, 919 of them the bar; World:
//! 1,538, 977 the bar.
//!
//! So the bar is kept: `screen::close_the_door` lifts it off the frame it sweeps and `menu_bar::keep_the_menu_bar` puts
//! it back on top of the new frame. These tests count the work, not the time: the bar and its rows stand through any
//! number of switches, once and on top of the frame; what comes and goes under it is the tooltip of a row whose greying
//! the new workspace changed; and a switch spawns fewer nodes than the bar holds. `--no-capture` prints each switch's
//! counts and its first frames' split.
//!
//! **The activity bar and the toolbar are kept the same way** (#678): the registered workspaces' tabs, and the map's
//! tools on the workspaces that have them, were the next-largest things a switch drew again that it did not change.
//! `close_the_door` lifts them with the menu bar; `activity_bar::keep_the_activity_bar` puts the activity bar back at the
//! far left of the new frame's body and `toolbar::the_bar_follows_the_workspace` puts the toolbar back after the chrome
//! bar, or sweeps it on a workspace without the map's tools. The last two tests count that no node under either is
//! spawned or swept by a switch that keeps it.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;

use bevy_dark_factory::activity_bar::{ActivityBar, ActivityStrip};
use bevy_dark_factory::chrome::Frame;
use bevy_dark_factory::harness;
use bevy_dark_factory::menu_bar::EditorMenuBar;
use bevy_dark_factory::profile::Profiler;
use bevy_dark_factory::toolbar::Toolbar;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// Frames the editor is given to settle after a switch: the transition runs in the first, the bar is put back in the
/// same frame, and gooey's rows settle in the next two.
const SETTLE_FRAMES: usize = 10;

fn editor(tag: &str) -> App {
    let mut fixture = fixtures::Fixture::new(&format!("switch_work_{tag}_{}", std::process::id()));
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

/// Switch through `screen::cross`, the call `door_open` and every workspace chord end in, and let the editor settle.
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

/// The one menu bar that stands.
fn the_bar(app: &mut App) -> Entity {
    let bars = with::<EditorMenuBar>(app);
    assert_eq!(bars.len(), 1, "one menu bar stands, not {}", bars.len());
    bars.into_iter().next().unwrap_or_else(|| panic!("a menu bar"))
}

fn parents(app: &mut App) -> HashMap<Entity, Entity> {
    let world = app.world_mut();
    let mut links = world.query::<(Entity, &ChildOf)>();
    links.iter(world).map(|(child, parent)| (child, parent.parent())).collect()
}

/// `of` and every entity under it.
fn subtree(parent: &HashMap<Entity, Entity>, of: Entity) -> HashSet<Entity> {
    let mut children: HashMap<Entity, Vec<Entity>> = HashMap::new();
    for (child, up) in parent {
        children.entry(*up).or_default().push(*child);
    }
    let mut out = HashSet::from([of]);
    let mut open = vec![of];
    while let Some(at) = open.pop() {
        for kid in children.get(&at).into_iter().flatten() {
            if out.insert(*kid) {
                open.push(*kid);
            }
        }
    }
    out
}

/// The greying each entity of `of` carries: `bevy_gooey::Disabled`'s reason, which is what puts a tooltip under a row.
fn greying(app: &App, of: &HashSet<Entity>) -> HashMap<Entity, Option<String>> {
    of.iter().map(|e| (*e, app.world().get::<bevy_gooey::Disabled>(*e).map(|d| d.0.clone()))).collect()
}

/// The nearest entity above `e` (by `parent`) that is in `kept`.
fn kept_above(parent: &HashMap<Entity, Entity>, kept: &HashSet<Entity>, mut e: Entity) -> Option<Entity> {
    while let Some(up) = parent.get(&e) {
        if kept.contains(up) {
            return Some(*up);
        }
        e = *up;
    }
    None
}

/// **A switch keeps the menu bar and its rows, on top of the new frame, and spawns fewer nodes than the bar holds.**
///
/// Fails on the tree before #678's bar cut: the bar was a child of the frame's root, so every switch despawned it with the
/// frame and `keep_the_menu_bar` drew it again — a new entity, and 913–977 new nodes of a switch's 1,416–1,618.
#[test]
fn a_switch_keeps_the_menu_bar_and_spawns_less_than_the_bar_holds() {
    let mut app = editor("bar");
    // **The first round trip is not counted**, so what a session builds once is not in the numbers.
    switch(&mut app, Workspace::Kit);
    switch(&mut app, Workspace::Map);
    let bar = the_bar(&mut app);
    let bar_nodes = {
        let parent = parents(&mut app);
        let rows = subtree(&parent, bar);
        let texts = rows.iter().filter(|e| app.world().get::<Text>(**e).is_some()).count();
        let nodes = rows.iter().filter(|e| app.world().get::<Node>(**e).is_some()).count();
        println!("the menu bar: {nodes} nodes, {texts} texts");
        nodes
    };
    assert!(bar_nodes > 100, "the bar holds every command of the registry: {bar_nodes} nodes");

    for to in [Workspace::Kit, Workspace::Map, Workspace::World, Workspace::Animation, Workspace::Map] {
        let nodes = with::<Node>(&mut app);
        let texts = with::<Text>(&mut app);
        let parent_before = parents(&mut app);
        let rows_before = subtree(&parent_before, bar);
        let greyed = greying(&app, &rows_before);
        let start = app.world().resource::<Profiler>().last().map_or(0, |frame| frame.number);

        switch(&mut app, to);

        let now = with::<Node>(&mut app);
        let new = now.difference(&nodes).count();
        let new_texts = with::<Text>(&mut app).difference(&texts).count();
        let split: Vec<String> = app
            .world()
            .resource::<Profiler>()
            .frames()
            .filter(|frame| frame.number > start)
            .take(2)
            .map(|frame| {
                let shares: Vec<String> = frame.schedules.iter().filter(|s| s.ms >= 0.5).map(|s| format!("{} {:.1}", s.name(), s.ms)).collect();
                format!("busy {:.1} ms ({})", frame.busy_ms, shares.join(", "))
            })
            .collect();

        assert_eq!(the_bar(&mut app), bar, "the switch to {} kept the menu bar", to.title());
        let parent_after = parents(&mut app);
        let rows_after = subtree(&parent_after, bar);
        let kept: HashSet<Entity> = rows_after.intersection(&rows_before).copied().collect();
        let regreyed = |row: Entity| greyed.get(&row).cloned().flatten() != app.world().get::<bevy_gooey::Disabled>(row).map(|d| d.0.clone());
        // **What comes and goes under the bar is a re-greyed row's tooltip**, and nothing else: every entity gained or
        // lost hangs under a kept row whose `Disabled` the new workspace changed.
        for (gone_or_new, parent) in [(rows_before.difference(&rows_after), &parent_before), (rows_after.difference(&rows_before), &parent_after)] {
            for e in gone_or_new {
                let row = kept_above(parent, &kept, *e);
                assert!(
                    row.is_some_and(&regreyed),
                    "the switch to {} drew {e:?} under the bar again, and no row above it changed its greying: the bar was rebuilt",
                    to.title()
                );
            }
        }
        let frame = app.world().resource::<Frame>().root;
        let top = app.world().get::<Children>(frame).and_then(|kids| kids.first().copied());
        assert_eq!(top, Some(bar), "the kept bar stands on top of the frame the switch to {} built", to.title());
        println!(
            "to {}: {new} of {} nodes new, {} kept; {new_texts} texts new; under the bar {} kept, {} new (re-greyed rows' tooltips); first frames: {}",
            to.title(),
            now.len(),
            now.intersection(&nodes).count(),
            kept.len(),
            rows_after.len() - kept.len(),
            split.join(" / ")
        );
        assert!(
            new < bar_nodes,
            "the switch to {} spawned {new} nodes, more than the {bar_nodes} the bar holds: the bar was built again",
            to.title()
        );
    }
}

/// **A switch keeps the activity bar and every node under it but the lit tab's rail, at the far left of the new frame's
/// body, and lights the workspace it lands on.**
///
/// What comes and goes under the bar is the rail the editor draws under the `Selected` tab (`ui::SelectedRail`): a switch
/// moves it from the tab it leaves to the tab it lands on, one swept and one spawned. Nothing else is.
///
/// Fails on the tree before the activity bar's cut: the bar was a child of the frame's body, so every switch despawned it
/// with the frame and `keep_the_activity_bar` drew it again — a new entity, and every node under it new.
#[test]
fn a_switch_keeps_the_activity_bar_and_spawns_none_of_its_nodes() {
    let mut app = editor("activity");
    switch(&mut app, Workspace::Kit);
    switch(&mut app, Workspace::Map);
    let the_activity_bar = |app: &mut App| {
        let bars = with::<ActivityBar>(app);
        assert_eq!(bars.len(), 1, "one activity bar stands, not {}", bars.len());
        bars.into_iter().next().unwrap_or_else(|| panic!("an activity bar"))
    };
    let bar = the_activity_bar(&mut app);
    let mut tabs = subtree(&parents(&mut app), bar);
    let tab_nodes = tabs.iter().filter(|e| app.world().get::<Node>(**e).is_some()).count();
    println!("the activity bar: {tab_nodes} nodes");
    assert!(tab_nodes > 10, "the bar holds a tab for every registered workspace: {tab_nodes} nodes");

    for to in [Workspace::Kit, Workspace::Map, Workspace::World, Workspace::Animation, Workspace::Map] {
        let nodes = with::<Node>(&mut app);
        switch(&mut app, to);

        assert_eq!(the_activity_bar(&mut app), bar, "the switch to {} kept the activity bar", to.title());
        let parent = parents(&mut app);
        let after = subtree(&parent, bar);
        let spawned: Vec<Entity> = after.difference(&tabs).copied().collect();
        let swept = tabs.difference(&after).count();
        let on_the_lit_tab = |e: &Entity| parent.get(e).is_some_and(|tab| app.world().get::<bevy::ui::Selected>(*tab).is_some());
        let named: Vec<String> = spawned
            .iter()
            .map(|e| {
                let names: Vec<String> = app.world().inspect_entity(*e).map(|c| c.map(|(_, c)| format!("{:?}", c.name())).collect()).unwrap_or_default();
                names.join(" ")
            })
            .collect();
        assert!(
            spawned.len() <= 1 && swept <= 1 && spawned.iter().all(on_the_lit_tab),
            "the switch to {} spawned {} and swept {swept} entities under the activity bar, more than the lit tab's rail: it was \
             drawn again; spawned: {named:?}",
            to.title(),
            spawned.len()
        );
        let frame = app.world().resource::<Frame>();
        let (root, body) = (frame.root, frame.body);
        let first = app.world().get::<Children>(body).and_then(|kids| kids.first().copied());
        assert_eq!(first, Some(bar), "the kept bar stands at the far left of the body the switch to {} built", to.title());
        assert_eq!(app.world().get::<ActivityBar>(bar).map(|b| b.frame), Some(root), "the kept bar names the new frame");
        let lit = {
            let world = app.world_mut();
            let mut strips = world.query::<(&ActivityStrip, &bevy_gooey::TabsValue)>();
            let (strip, value) = strips.single(world).unwrap_or_else(|e| panic!("one activity strip: {e}"));
            strip.0.get(value.0).copied()
        };
        assert_eq!(lit, Some(to), "the kept bar lights {}", to.title());
        println!(
            "to {}: {} nodes new, under the activity bar {} new (the lit tab's rail) of {tab_nodes}",
            to.title(),
            with::<Node>(&mut app).difference(&nodes).count(),
            spawned.len()
        );
        tabs = after;
    }
}

/// **A switch between the workspaces with the map's tools keeps the toolbar and every node under it, straight after the
/// new frame's chrome bar; a switch to one without them sweeps it, and the way back draws one.**
///
/// Fails on the tree before the toolbar's cut: the bar was a child of the frame's root, so every switch despawned it with
/// the frame and `the_bar_follows_the_workspace` drew it again — a new entity, and every node under it new.
#[test]
fn a_switch_keeps_the_toolbar_where_the_workspace_has_it_and_spawns_none_of_its_nodes() {
    let mut app = editor("toolbar");
    switch(&mut app, Workspace::Kit);
    switch(&mut app, Workspace::Map);
    let bar = {
        let bars = with::<Toolbar>(&mut app);
        assert_eq!(bars.len(), 1, "one toolbar stands on Map, not {}", bars.len());
        bars.into_iter().next().unwrap_or_else(|| panic!("a toolbar"))
    };
    let buttons = subtree(&parents(&mut app), bar);
    let bar_nodes = buttons.iter().filter(|e| app.world().get::<Node>(**e).is_some()).count();
    println!("the toolbar: {bar_nodes} nodes");
    assert!(bar_nodes > 10, "the bar holds the map's tools: {bar_nodes} nodes");

    for to in [Workspace::Kit, Workspace::Map, Workspace::World, Workspace::Map] {
        let nodes = with::<Node>(&mut app);
        switch(&mut app, to);

        assert_eq!(with::<Toolbar>(&mut app), HashSet::from([bar]), "the switch to {} kept the one toolbar", to.title());
        let after = subtree(&parents(&mut app), bar);
        assert_eq!(
            after.difference(&buttons).count() + buttons.difference(&after).count(),
            0,
            "the switch to {} spawned or swept nodes under the toolbar: it was drawn again",
            to.title()
        );
        let frame = app.world().resource::<Frame>();
        let (root, chrome_bar) = (frame.root, frame.chrome_bar);
        let kids: Vec<Entity> = app.world().get::<Children>(root).map(|kids| kids.to_vec()).unwrap_or_default();
        let chrome_at = kids.iter().position(|kid| *kid == chrome_bar);
        assert_eq!(
            chrome_at.and_then(|at| kids.get(at + 1)).copied(),
            Some(bar),
            "the kept toolbar stands straight after the chrome bar of the frame the switch to {} built",
            to.title()
        );
        println!("to {}: {} nodes new, none under the toolbar ({bar_nodes} kept)", to.title(), with::<Node>(&mut app).difference(&nodes).count());
    }

    switch(&mut app, Workspace::Animation);
    assert!(with::<Toolbar>(&mut app).is_empty(), "Animation has no map tools, so the switch to it swept the toolbar");
    switch(&mut app, Workspace::Map);
    let again = with::<Toolbar>(&mut app);
    assert_eq!(again.len(), 1, "the way back to Map draws one toolbar, not {}", again.len());
}
