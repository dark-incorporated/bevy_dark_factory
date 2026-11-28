//! **The empty map's prompt belongs to the map's viewport** (#700).
//!
//! On a project whose map has no placements and whose library has pieces, `empty_state` stands "Place a piece from the
//! library" and its **Open library** button over the viewport. Until #700 it stood there on every workspace, so on Files it
//! was drawn over the code editor and took the pointer. #540's capture test found it: a wheel turned over the centre of the
//! editor landed on the button and scrolled nothing. Now the prompt stands only on a workspace that shows the map.
//!
//! Two readings. Which workspaces hold the prompt: the four that show the map do, and none of the six that do not. Each is
//! reached by its own command, the way a person goes. And what the pointer finds at the centre of the Files editor's code
//! area: the code editor, through the same picking a person's mouse goes through (the window's cursor, `view::sense_pointer`,
//! `surface::retarget_pointer`, `bevy_ui`'s backend, the `HoverMap`).

use bevy::input::InputSystems;
use bevy::picking::hover::HoverMap;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::PrimaryWindow;

use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::empty_state::{EmptyMapPrompt, OpenLibraryButton};
use bevy_dark_factory::files;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::workspace::Workspace;
use bevy_gooey::{CodeEditor, Role};

use crate::fixtures;
use crate::settle_support;

/// The page the Files editor opens, long enough to fill its code area.
const PAGE: &str = "design/notes.md";

/// **A project whose map is empty and whose library has a piece to place**, the state the prompt stands in, with a page to open.
fn an_empty_map(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("prompt700_{tag}_{}", std::process::id())).descriptor("alpha/floor", "alpha").build("m");
    let page = root.join(PAGE);
    std::fs::create_dir_all(page.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    let text: String = (0..60).map(|n| format!("A line of the page that fills the editor, number {n}.\n")).collect();
    std::fs::write(&page, text).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    // **A window**, because the pointer's position is the window's cursor, and the interface is laid out at the size it gives.
    let window = Window::default();
    let (width, height) = (window.resolution.physical_width(), window.resolution.physical_height());
    app.world_mut().spawn((window, PrimaryWindow));
    harness::resize_surface(&mut app, width, height).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    app
}

fn run_command(app: &mut App, id: &str) {
    let command = commands::by_id(id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(app);
}

/// **How many prompts and how many of its buttons the world holds.** A prompt that is not there is neither drawn nor hit-tested.
fn prompts(app: &mut App) -> (usize, usize) {
    let world = app.world_mut();
    let prompts = world.query_filtered::<(), With<EmptyMapPrompt>>().iter(world).count();
    let buttons = world.query_filtered::<(), With<OpenLibraryButton>>().iter(world).count();
    (prompts, buttons)
}

/// **The prompt stands on the four workspaces that show the map and on none of the six that do not** (#700). The editor opens
/// on the Map with the prompt up. It then goes to every workspace by that workspace's own command, in `Workspace::ALL`'s order,
/// and back to the Map, which has the prompt again.
#[test]
fn the_empty_maps_prompt_stands_only_on_a_workspace_that_shows_the_map() {
    let mut app = an_empty_map("where");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Map, "the editor opens on the Map");
    assert_eq!(prompts(&mut app), (1, 1), "an empty map with pieces to place says what to do next on the Map");
    for workspace in Workspace::ALL.into_iter().chain([Workspace::Map]) {
        run_command(&mut app, &format!("workspace.{}", workspace.id()));
        assert_eq!(*app.world().resource::<Workspace>(), workspace, "{} is reachable by its command", workspace.title());
        let want = if workspace.shows_the_map() { (1, 1) } else { (0, 0) };
        assert_eq!(
            prompts(&mut app),
            want,
            "the prompt and its button on {}, which {} the map",
            workspace.title(),
            if workspace.shows_the_map() { "shows" } else { "does not show" }
        );
    }
}

/// **Where the window's cursor stands**, written in `PreUpdate` after the input systems, as a person's mouse arrives.
#[derive(Resource, Default)]
struct Aim(Option<Vec2>);

fn aim(at: Res<Aim>, mut windows: Query<&mut Window, With<PrimaryWindow>>) {
    if let Ok(mut window) = windows.single_mut() {
        window.set_cursor_position(at.0);
    }
}

/// Every entity at or under `root`.
fn under(world: &World, root: Entity) -> Vec<Entity> {
    let mut found = vec![root];
    let mut next = 0;
    while next < found.len() {
        if let Some(children) = world.get::<Children>(found[next]) {
            found.extend(children.iter());
        }
        next += 1;
    }
    found
}

/// A node's border box on the surface, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **The node the pointer stands over**: the nearest hit of the first pointer whose `HoverMap` entry holds one. It is the node a
/// click lands on and the one gooey's `code_on_wheel` walks up from to find the editor it scrolls.
fn nearest_hit(app: &App) -> Option<Entity> {
    let hover = app.world().get_resource::<HoverMap>()?;
    hover.values().find_map(|hits| hits.iter().min_by(|a, b| a.1.depth.total_cmp(&b.1.depth)).map(|(hit, _)| *hit))
}

/// **At the centre of the Files editor's code area the pointer finds the code editor** (#700). On an empty map this is where the
/// prompt's **Open library** button stood, so the pointer found the button and a wheel turned there scrolled nothing. The
/// surface is the window's size at a scale factor of 1, so a surface pixel is a window pixel.
#[test]
fn the_pointer_at_the_centre_of_the_files_code_area_finds_the_code_editor() {
    let mut app = an_empty_map("centre");
    run_command(&mut app, "workspace.files");
    assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "the Files workspace is reachable by its command");
    files::open_file(app.world_mut(), PAGE).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    assert_eq!(prompts(&mut app), (0, 0), "an empty map's prompt is not on the Files workspace");

    let editor = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<CodeEditor>>().single(world).unwrap_or_else(|e| panic!("one code editor is open: {e}"))
    };
    let centre = {
        let world = app.world();
        under(world, editor)
            .into_iter()
            .find(|e| world.get::<Role>(*e) == Some(&Role::CodeArea))
            .and_then(|area| border_box(world, area))
            .unwrap_or_else(|| panic!("the editor's code area is laid out"))
            .center()
    };
    app.init_resource::<Aim>();
    app.add_systems(PreUpdate, aim.after(InputSystems));
    app.world_mut().resource_mut::<Aim>().0 = Some(centre);
    settle_support::until(&mut app, "the pointer's hover reaches the interface", |app| nearest_hit(app).is_some());
    let hit = nearest_hit(&app).unwrap_or_else(|| panic!("the pointer stands over a node"));
    assert!(
        under(app.world(), editor).contains(&hit),
        "the pointer at the code area's centre {centre} finds {hit:?} ({:?}, {:?}), not the code editor",
        app.world().get::<Role>(hit),
        app.world().get::<Name>(hit)
    );
}
