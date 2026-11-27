//! **The toolbar is the workspace's** (#690; the owner chose "per workspace", 2026-11-27).
//!
//! The 2026-11-27 review measured the map's toolbar — Place, Move, Erase, Clone, Run, Fill, undo, redo, save and
//! the view helpers — on all ten workspaces with Place lit, 46 px on the six where none of its tools can act. The
//! decision: the bar is drawn on Map, World and Kit and on no other workspace, so a workspace without the tools
//! has no bar and its panels have the room; and nothing is drawn lit where it cannot act.
//!
//! This file is the census the issue asks for. Each workspace is built on its own panel, so no switch is in the
//! picture, and names the controls its bar draws, the ones that can act there, and the one drawn lit. Then a
//! switch takes the bar away and brings it back, and the room it gave up is measured; and a bar that arrives
//! while a tool is armed lights that tool.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, InteractionDisabled, UiGlobalTransform};

use bevy_dark_factory::chrome::Frame;
use bevy_dark_factory::editor::{EditorState, Tool};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::{binding, Action};
use bevy_dark_factory::toolbar::{ToolModes, Toolbar, ToolbarCommand};
use bevy_dark_factory::workspace::Workspace;
use bevy_gooey::{GooeySkin, Role, Tooltip};

use crate::fixtures;

/// Frames the editor is given to settle: a switch's transition runs on the frame after the key, and what it
/// spawns is laid out on the frame after that.
const SETTLE_FRAMES: usize = 10;

/// **Every control the map's bar draws, in its order**: the mode group (Place, which has no command, then
/// the four tool commands), Fill beside it, then the pictures. Named by command id, and Place by its own word.
const THE_MAP_BAR: [&str; 13] = [
    "place",
    "map.move_mode",
    "map.remove_mode",
    "map.clone_mode",
    "map.run_mode",
    "map.fill",
    "edit.undo",
    "edit.redo",
    "file.save",
    "map.note",
    "map.generate",
    "view.frame",
    "tools.snip",
];

/// **What acts on every tab**, and so is live wherever the bar is drawn: the two commands bound `Global`.
const LIVE_ON_EVERY_TAB: [&str; 2] = ["file.save", "tools.snip"];

fn project(tag: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(&format!("toolbar_ws_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .descriptor("alpha/wall", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .place_as("wall@0", "alpha/wall", (2.0, 0.0))
        .build("m")
}

/// An editor opened straight onto `workspace`'s first panel: the door follows the panel.
fn editor_on(workspace: Workspace) -> App {
    let root = project(workspace.id());
    let mut app = harness::build_headless_at(&root, "m", None, workspace.opens_on()).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "the editor did not open on the {} workspace", workspace.title());
    app
}

/// **What one workspace's bar draws**: every control by name in its order, the ones that can act (not
/// `InteractionDisabled`, which gooey draws dimmed and which takes no click), and the ones drawn in the accent.
#[derive(Debug, PartialEq)]
struct Census {
    drawn: Vec<&'static str>,
    live: Vec<&'static str>,
    lit: Vec<&'static str>,
}

/// A control's name: a button's command id, and an option's the command whose title its tooltip is, else
/// `place` — the one option with no command of its own.
fn name_of(world: &World, control: Entity) -> Option<&'static str> {
    if let Some(command) = world.get::<ToolbarCommand>(control) {
        return Some(command.0);
    }
    if world.get::<Role>(control) != Some(&Role::GroupOption) {
        return None;
    }
    let tip = world.get::<Tooltip>(control).map(|t| t.text.clone()).unwrap_or_default();
    Some(
        bevy_dark_factory::toolbar::tools()
            .iter()
            .find(|(id, _)| bevy_dark_factory::commands::by_id(id).is_some_and(|c| c.title == tip))
            .map_or("place", |(id, _)| *id),
    )
}

fn census(app: &mut App) -> Census {
    let accent = app.world().resource::<GooeySkin>().accent;
    let world = app.world_mut();
    let bars: Vec<Entity> = world.query_filtered::<Entity, With<Toolbar>>().iter(world).collect();
    assert!(bars.len() <= 1, "{} toolbars are drawn at once", bars.len());
    let mut census = Census { drawn: Vec::new(), live: Vec::new(), lit: Vec::new() };
    let Some(&bar) = bars.first() else {
        return census;
    };
    // The bar's children in order, and the mode group's options in its place.
    let mut controls = Vec::new();
    for kid in world.get::<Children>(bar).map(|c| c.iter().collect::<Vec<_>>()).unwrap_or_default() {
        if world.get::<ToolModes>(kid).is_some() {
            controls.extend(world.get::<Children>(kid).map(|c| c.iter().collect::<Vec<_>>()).unwrap_or_default());
        } else {
            controls.push(kid);
        }
    }
    for control in controls {
        let Some(name) = name_of(world, control) else { continue };
        census.drawn.push(name);
        if world.get::<InteractionDisabled>(control).is_none() {
            census.live.push(name);
        }
        if world.get::<BackgroundColor>(control).is_some_and(|fill| fill.0 == accent) {
            census.lit.push(name);
        }
    }
    census
}

/// **The ten workspaces, and what each one's bar draws.** Map draws every tool and they all act, Place lit;
/// World and Kit draw the same bar with only Save and the snip live and nothing lit, because every other
/// control is bound on the Map tab; the other seven draw no bar at all.
///
/// Fails before the change: every workspace drew the Map's bar, all thirteen controls live and Place lit.
#[test]
fn each_workspace_names_the_tools_its_bar_shows() {
    let mut seen = Vec::new();
    for workspace in Workspace::ALL {
        let mut app = editor_on(workspace);
        let census = census(&mut app);
        let want = match workspace {
            Workspace::Map => Census { drawn: THE_MAP_BAR.to_vec(), live: THE_MAP_BAR.to_vec(), lit: vec!["place"] },
            Workspace::World | Workspace::Kit => {
                Census { drawn: THE_MAP_BAR.to_vec(), live: LIVE_ON_EVERY_TAB.to_vec(), lit: Vec::new() }
            }
            _ => Census { drawn: Vec::new(), live: Vec::new(), lit: Vec::new() },
        };
        seen.push((workspace.title(), census.drawn.len(), census.live.clone(), census.lit.clone()));
        assert_eq!(census, want, "the {} workspace's bar", workspace.title());
        assert_eq!(
            workspace.has_the_toolbar(),
            !census.drawn.is_empty(),
            "`Workspace::has_the_toolbar` and the bar on screen disagree on {}",
            workspace.title()
        );
    }
    println!("workspace, controls drawn, live, lit: {seen:?}");
}

/// **A click on a quiet tool cannot arm it.** On Kit the mode group is disabled, so choosing Move (the way
/// gooey's own click and Enter reach an option: `Activate` on it) sends nothing, and the tool stays Place.
#[test]
fn a_tool_that_cannot_act_here_cannot_be_armed_from_the_bar() {
    let mut app = editor_on(Workspace::Kit);
    let move_option = {
        let world = app.world_mut();
        let options: Vec<Entity> = world.query_filtered::<&Children, With<ToolModes>>().single(world).expect("one mode group").iter().collect();
        options.into_iter().find(|o| name_of(world, *o) == Some("map.move_mode")).expect("Move's option is on the bar")
    };
    app.world_mut().commands().trigger(bevy_gooey::Activate { entity: move_option });
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert_eq!(app.world().resource::<EditorState>().tool, Tool::Place, "a disabled Move option armed Move on the Kit");
    assert!(census(&mut app).lit.is_empty(), "choosing a quiet option lit it");
}

/// Hold a chord down for two frames and let it go, as a person's keyboard does.
fn press(app: &mut App, action: Action) {
    let row = binding(action);
    let mut keys = vec![row.key];
    if row.needs_mod {
        keys.push(if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft });
    }
    if row.needs_shift == Some(true) {
        keys.push(KeyCode::ShiftLeft);
    }
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in &keys {
            app.world_mut().write_message(KeyboardInput {
                key_code: *code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
        app.update();
    }
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

fn switch_to(app: &mut App, workspace: Workspace) {
    let action = Action::ALL
        .into_iter()
        .find(|a| a.workspace() == Some(workspace))
        .unwrap_or_else(|| panic!("no chord goes to the {} workspace", workspace.title()));
    press(app, action);
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "the key did not switch to {}", workspace.title());
}

/// A node's top and height, logical px.
fn top_and_height(app: &App, entity: Entity) -> (f32, f32) {
    let node = app.world().get::<ComputedNode>(entity).expect("laid out");
    let at = app.world().get::<UiGlobalTransform>(entity).expect("placed");
    let scale = node.inverse_scale_factor();
    let size = node.size() * scale;
    (at.translation.y * scale - size.y / 2.0, size.y)
}

/// The frame's body, chrome bar and strip, read again after anything that may have rebuilt the frame.
fn bands(app: &App) -> (Entity, Entity, Entity) {
    let frame = app.world().resource::<Frame>();
    (frame.body, frame.chrome_bar, frame.door_strip)
}

fn the_bar(app: &mut App) -> Option<Entity> {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<Toolbar>>().iter(world).next()
}

/// **A switch takes the bar away and brings it back, and the docks have its room meanwhile.** Whether a switch
/// keeps the frame or builds it again (#695), the bar is the workspace's: on Animation it is gone and the body
/// starts the bar's band higher; on Kit it is back and quiet; on the Map again every control acts and the
/// armed tool is the lit one.
#[test]
fn a_switch_takes_the_bar_away_gives_its_room_and_brings_it_back() {
    let mut app = editor_on(Workspace::Map);
    let (body, chrome_bar, strip) = bands(&app);
    let on_the_map = the_bar(&mut app).expect("the Map draws the bar");
    let (_, bar_h) = top_and_height(&app, on_the_map);
    assert!(bar_h > 0.0, "the bar is laid out on the Map");
    // **The band the bar takes**, margins and all: from the chrome bar's foot to the strip's head.
    let (chrome_top, chrome_h) = top_and_height(&app, chrome_bar);
    let band = top_and_height(&app, strip).0 - (chrome_top + chrome_h);
    assert!(band >= bar_h - 0.5, "the bar's band ({band} px) is shorter than the bar ({bar_h} px)");
    let (body_top_with_bar, _) = top_and_height(&app, body);

    switch_to(&mut app, Workspace::Animation);
    assert_eq!(the_bar(&mut app), None, "Animation has none of the bar's tools, and still draws the bar");
    let (body, _, _) = bands(&app);
    let (body_top_without_bar, _) = top_and_height(&app, body);
    assert!(
        (body_top_with_bar - body_top_without_bar - band).abs() < 0.5,
        "the body did not take the bar's {band} px: it starts at {body_top_without_bar} without the bar and at \
         {body_top_with_bar} with it"
    );

    switch_to(&mut app, Workspace::Kit);
    let census_on_kit = census(&mut app);
    assert_eq!(census_on_kit.drawn, THE_MAP_BAR.to_vec(), "Kit draws the bar again");
    assert_eq!(census_on_kit.live, LIVE_ON_EVERY_TAB.to_vec(), "on Kit only what acts on every tab is live");
    assert!(census_on_kit.lit.is_empty(), "Kit lit a tool it cannot use: {:?}", census_on_kit.lit);

    switch_to(&mut app, Workspace::Map);
    let census_on_map = census(&mut app);
    assert_eq!(census_on_map.live, THE_MAP_BAR.to_vec(), "back on the Map every control acts again");
    let tool = app.world().resource::<EditorState>().tool;
    assert_eq!(census_on_map.lit, vec![option_of(tool)], "the Map's bar lights the armed tool, {tool:?}");
}

/// The name [`census`] gives the option that stands for `tool`.
fn option_of(tool: Tool) -> &'static str {
    if tool == Tool::Place {
        return "place";
    }
    bevy_dark_factory::toolbar::tools()
        .iter()
        .find(|(_, mode)| *mode == Some(tool))
        .map(|(id, _)| *id)
        .unwrap_or_else(|| panic!("{tool:?} has no option on the bar"))
}

/// **A bar that arrives while a tool is armed lights that tool.** The bar is spawned afresh on each workspace that
/// has one, its mode group starts on its first option (Place), and the tool it should light was armed before it
/// existed — so nothing changes on the frame it arrives, and the group must still take the armed tool.
///
/// Fails before the change: the group followed the tool only on a frame the tool changed, so the new bar lit Place.
#[test]
fn a_bar_that_arrives_while_a_tool_is_armed_lights_that_tool() {
    let mut app = editor_on(Workspace::Map);
    app.world_mut().resource_mut::<EditorState>().tool = Tool::Clone;
    for _ in 0..2 {
        app.update();
    }
    assert_eq!(census(&mut app).lit, vec!["map.clone_mode"], "the bar follows a tool armed while it stands");
    // The bar goes, as it does on a workspace without one, and the frame is left to draw it again.
    let bar = the_bar(&mut app).expect("the Map draws the bar");
    app.world_mut().entity_mut(bar).despawn();
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    let again = the_bar(&mut app).expect("the Map draws its bar again");
    assert_ne!(again, bar, "a new bar stands in the frame");
    assert_eq!(app.world().resource::<EditorState>().tool, Tool::Clone, "nothing disarmed the tool");
    assert_eq!(census(&mut app).lit, vec!["map.clone_mode"], "the new bar lights the tool that was armed before it");
}

/// **On Files the bar's band is gone**: the chrome bar's foot is the door strip's head, with nothing between.
#[test]
fn on_files_nothing_stands_between_the_chrome_bar_and_the_strip() {
    let app = editor_on(Workspace::Files);
    let frame = app.world().resource::<Frame>();
    let (chrome_top, chrome_h) = top_and_height(&app, frame.chrome_bar);
    let (strip_top, _) = top_and_height(&app, frame.door_strip);
    assert!(
        (chrome_top + chrome_h - strip_top).abs() < 0.5,
        "something {} px tall stands between the chrome bar (foot at {}) and the strip (head at {strip_top})",
        strip_top - chrome_top - chrome_h,
        chrome_top + chrome_h
    );
}
