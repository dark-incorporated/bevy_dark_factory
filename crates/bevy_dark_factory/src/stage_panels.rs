//! **Every panel can be staged for a capture** (UI-92, #258).
//!
//! A changelog clip of a panel has to start on that panel. The only way to get one there was a person's keys and clicks —
//! a storyboard guessing pixels at a tab's place — so the panels most worth recording (the launcher, the loader, the
//! settings screen, each dock panel) were the ones no clip could be made of.
//!
//! This module is the catalog and the one function that honours a request: a name in, the shell arranged to show that
//! panel, or a refusal saying why not. `tools/demo/record.py` sends the request through `blueprint/stage_panel` before a
//! sequence starts, and so can any BRP caller, so a capture and an agent's own look go through one door.
//!
//! # What a panel is, here
//!
//! Three kinds of name, each staged the way its own control stages it:
//!
//! - **A screen** — `launcher`, `loader` and `settings` — a `Screen` state or the dialog over the workbench.
//! - **A dock panel** — `inspector`, `viewport`, `place` — by the id [`bevy_gooey::PanelRegistry`] registers it under. It is
//!   staged by making its group's tab the selected one, which is what a click on that tab does, so a clip shows the panel a
//!   person would be looking at rather than a layout built for the camera.
//! - **A workbench state** (#696) — `style_inspector`, `section_cut`, `design_styles`, `terminal_shell` — by writing the
//!   state its own control writes: the inspector up and asked about a panel edge, the Map camera's section cut on, the Design
//!   workspace with its styles tree populated, the terminal raised in front of a shell. These four are the states three review
//!   passes reported they could not reach, because the only way in was a key script written afresh each pass.

use bevy::prelude::*;
use bevy_gooey::{Dock, DockLayout, PanelRegistry};

use crate::screen::Screen;

/// One thing a capture can start on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stage {
    /// The name a storyboard or an agent asks for.
    pub id: &'static str,
}

/// **Every state a capture can be staged on**, in the order a person meets them: the three screens before and around the
/// work, the dock panels in the shell's own left-to-right order, and the workbench states the review needs (#696).
///
/// **The status band is not here, on purpose** ([`NOT_STAGED`]): the frame draws it under every workbench panel rather
/// than by a tab, so a stage for it would either lie or build a layout for the camera.
pub const STAGES: &[Stage] = &[
    Stage { id: "launcher" },
    Stage { id: "loader" },
    Stage { id: "settings" },
    Stage { id: "inspector" },
    Stage { id: "viewport" },
    Stage { id: "place" },
    // **The four states three review passes could not reach** (#696), each one call and settled when it returns.
    Stage { id: "style_inspector" },
    Stage { id: "section_cut" },
    Stage { id: "design_styles" },
    Stage { id: "terminal_shell" },
];

/// **How long the loader is held on each stage when it is staged**, seconds: two frames at the slowest rate a capture
/// sequence may ask for (`sequencer::MIN_FPS`), so a staged loader always has a frame to give.
pub const STAGE_HOLD_S: f32 = 1.0;

/// The stage with this name.
pub fn stage(id: &str) -> Option<&'static Stage> {
    STAGES.iter().find(|stage| stage.id == id)
}

/// Every stage's name, in the catalog's order, for a refusal that says what would have worked.
pub fn names() -> String {
    STAGES.iter().map(|stage| stage.id).collect::<Vec<_>>().join(", ")
}

/// **Whether a name is a screen rather than a dock panel**: a screen is a `Screen` state or a dialog over the workbench,
/// never a `PanelRegistry` entry.
pub fn is_screen(id: &str) -> bool {
    matches!(id, "launcher" | "loader" | "settings")
}

/// **Whether a name is a workbench *state* rather than a panel** (#696): an overlay, the camera's cut, a workspace with
/// its tree filled, the terminal raised. Each is reached by writing the state its own control writes, and each is what a
/// picture of that control in use shows — so a capture can start on it without a key script.
pub fn is_state(id: &str) -> bool {
    matches!(id, "style_inspector" | "section_cut" | "design_styles" | "terminal_shell")
}

/// **Panels the shell registers and a capture cannot stage**, with the reason, so the catalog can be checked against the
/// registry in both directions. The status band is drawn by the frame under every workbench panel rather than by a tab, so
/// it has no group to select and is already in every workbench clip.
pub const NOT_STAGED: &[(&str, &str)] = &[
    (
        "status",
        "the status band is drawn by the frame under every workbench panel, not by a tab, so it is in every workbench clip",
    ),
    // **The console and the profile are overlays a chord opens** (#509, #504). They are not `PanelRegistry` entries — the
    // journal is not either — and a capture starts on one by pressing its chord, not by staging a tab.
    (
        "console",
        "the console is an overlay its own chord (Cmd/Ctrl+Shift+E) opens, so a clip starts on it by pressing that rather than by staging a tab",
    ),
    (
        "profile",
        "the frame profile is an overlay its own chord (Cmd/Ctrl+Shift+D) opens, so a clip starts on it by pressing that rather than by staging a tab",
    ),
];

/// **The order a run stages them in**: the workbench's own panels first, then the settings dialog over them, then the
/// loader, then the launcher last — because leaving the workbench is what the last two do, and only a stage that moves
/// the screen can come after it. A run in the catalog's reading order would refuse `inspector` after `launcher`.
pub fn capture_order() -> Vec<&'static str> {
    // **The dock panels, then the states over them, then the screens** (#696): a state is entered from the workbench and
    // leaves it open, so a run that visited one after `launcher` would refuse it.
    let mut order: Vec<&'static str> = dock_panels();
    order.extend(state_panels());
    order.extend(["settings", "loader", "launcher"]);
    order
}

/// The workbench states, in the catalog's order.
pub fn state_panels() -> Vec<&'static str> {
    STAGES.iter().map(|stage| stage.id).filter(|id| is_state(id)).collect()
}

/// The dock panels, in the catalog's order.
pub fn dock_panels() -> Vec<&'static str> {
    STAGES.iter().map(|stage| stage.id).filter(|id| !is_screen(id) && !is_state(id)).collect()
}

/// **What staging did**, so a caller has to say what it got rather than assume it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Staged {
    /// The panel that was staged.
    pub id: &'static str,
    /// The screen the editor is on, or is moving to this frame.
    pub screen: Screen,
}

/// **Arrange the shell so `id` is what a person is looking at**, and say what it arranged — or refuse, in words that name
/// the panel, what the shell has, and what would have worked.
///
/// The one function: `blueprint/stage_panel` calls it, and nothing else does, so a capture and an agent cannot disagree
/// about what staging a panel means.
pub fn arrange(world: &mut World, id: &str) -> Result<Staged, String> {
    let Some(stage) = stage(id) else {
        return Err(format!("`{id}` is not a panel a capture can start on. The panels are: {}.", names()));
    };
    // **No resource is assumed to exist.** `resource::<..>()` panics in Bevy when a resource is absent, and a BRP
    // command is reachable from a screen whose plugins have not put it there — so a missing one is a refusal that
    // names it, not a panic that takes the editor down.
    let Some(now) = world.get_resource::<State<Screen>>().map(|state| *state.get()) else {
        return Err("the editor has no screen state, so there is nothing here to arrange. Open a project first.".to_owned());
    };
    // **What the last stage put up comes down first**, or a clip of the panel after the settings screen would film the
    // settings screen. The dialog is gooey's own, so the way down is the `Leave` a person's Escape writes.
    if stage.id != "settings" {
        let mut open: Vec<Entity> = world.query_filtered::<Entity, With<crate::settings_screen::SettingsDialog>>().iter(world).collect();
        for dialog in open.drain(..) {
            world.entity_mut(dialog).insert(bevy_gooey::Leave);
        }
    }
    match stage.id {
        // **The launcher is a state**, reached the way File ▸ Close project reaches it.
        "launcher" => {
            let Some(mut next) = world.get_resource_mut::<NextState<Screen>>() else {
                return Err("the editor has no screen state to move to, so the launcher cannot be staged".to_owned());
            };
            next.set(Screen::Launcher);
            Ok(Staged { id: stage.id, screen: Screen::Launcher })
        }
        // **The loader is a state with work in it**, so staging it asks for the open it shows: the project this editor
        // has, through the same argv the command line takes, so the clip is of a project actually opening.
        "loader" => {
            let Some(argv) = re_open_argv(world) else {
                return Err("`loader` shows a project opening, and no project is open to open again".to_owned());
            };
            let Some(mut next) = world.get_resource_mut::<NextState<Screen>>() else {
                return Err("the editor has no screen state to move to, so the loader cannot be staged".to_owned());
            };
            next.set(Screen::Loading);
            world.insert_resource(crate::loading::LoadRequest(argv));
            world.insert_resource(crate::loading::StageDelay(std::time::Duration::from_secs_f32(STAGE_HOLD_S)));
            Ok(Staged { id: stage.id, screen: Screen::Loading })
        }
        // **The settings screen is a dialog over the workbench**, so it goes up where the person is.
        "settings" => {
            world.write_message(crate::settings_screen::OpenSettings);
            Ok(Staged { id: stage.id, screen: now })
        }
        // **The four workbench states** (#696): each writes the state its own control writes, so the capture shows the
        // control in use and not a layout built for the camera.
        state if is_state(state) => stage_a_state(world, state, now),
        panel => stage_a_dock_panel(world, panel, now),
    }
}

/// **The argv that opens the project this editor has**, door, map and kit included, so the loader shows the same project
/// a person opened rather than a default one. The words are the ones [`crate::args::split`] reads: the root and the map as
/// positionals, then `--door` and `--kit`.
fn re_open_argv(world: &World) -> Option<Vec<String>> {
    let project = world.get_resource::<crate::project::Project>()?;
    let door = world.get_resource::<crate::workspace::Workspace>().copied().unwrap_or_default();
    let mut argv = vec![project.root.to_string_lossy().into_owned()];
    if let Some(map) = world.get_resource::<crate::project::OpenMap>().map(|open| open.map.name.clone()).or_else(|| project.open_map_name.clone()) {
        argv.push(map);
    }
    argv.push("--door".to_owned());
    argv.push(door.label().to_lowercase());
    // A kit is the directory the library was read from, when it is not the project's own default.
    let default = project.root.join("assets").join("blueprint");
    if let Ok(rest) = project.blueprint_dir.strip_prefix(&default)
        && let Some(name) = rest.file_name()
    {
        argv.push("--kit".to_owned());
        argv.push(name.to_string_lossy().into_owned());
    }
    Some(argv)
}

/// **How far from the camera the staged section cut stands**, metres: near the camera, so the cut removes
/// the near wall of whatever the view holds and a picture shows the interior behind it (#472's own unit).
pub const STAGE_CUT_M: f32 = 1.5;

/// **A workbench state is staged by writing the state its own control writes** (#696).
///
/// Each of these was a state three review passes could not reach, because the only way in was a key script. Here is the
/// state each control leaves behind, so a capture starts on the control's result rather than on a layout made for the camera.
fn stage_a_state(world: &mut World, id: &str, now: Screen) -> Result<Staged, String> {
    if now != Screen::Editor {
        return Err(format!("`{id}` is a workbench state and the editor is not open (the screen is {now:?})"));
    }
    match id {
        // **The style inspector, up and asked about a panel**: the operation the chord dispatches is the one a person's
        // key dispatches (E-002), and the point it is opened on is the left panel's near edge, which is the panel edge a
        // person points at to ask why a panel looks the way it does.
        "style_inspector" => {
            let edge = panel_edge(world)?;
            let Some(mut inspector) = world.get_resource_mut::<crate::style_inspect::StyleInspector>() else {
                return Err("the editor has no style inspector to put up".to_owned());
            };
            inspector.on = true;
            if let Some(mut pointer) = world.get_resource_mut::<crate::view::Pointer>() {
                pointer.0 = Some(edge);
            }
            // The overlay is drawn by the view's own system from the pointer and the state, so the capture's first frame
            // has it — the same one frame a person's chord takes.
            crate::style_inspect::draw_the_inspector(world);
            let shown = world
                .query_filtered::<Entity, With<crate::style_inspect::StyleInspectorRoot>>()
                .iter(world)
                .next()
                .is_some();
            if !shown {
                return Err(format!(
                    "the style inspector is up but nothing classed is under ({:.0}, {:.0}) to ask about, so the overlay would be blank",
                    edge.x, edge.y
                ));
            }
            Ok(Staged { id: "style_inspector", screen: now })
        }
        // **The section cut, on**, placed the way `blueprint/section_cut` places it: one number on the Map camera's own
        // near plane, so nothing is rebuilt and no file changes.
        "section_cut" => {
            let Some(mut cut) = world.get_resource_mut::<crate::view::SectionCut>() else {
                return Err("the editor has no Map camera to cut, so the section cut cannot be staged".to_owned());
            };
            cut.set(STAGE_CUT_M);
            if cut.metres() != Some(STAGE_CUT_M) {
                return Err(format!("the Map camera refused a section cut at {STAGE_CUT_M} m"));
            }
            Ok(Staged { id: "section_cut", screen: now })
        }
        // **The Design workspace with pages in its styles tree**: the workspace switches, and the page shown is one the
        // project actually has — the game's own `design/style.md` or a place's, whichever the tree names first.
        "design_styles" => stage_the_design_tree(world, now),
        // **The terminal, raised in front of a shell that has printed its prompt**: the pane's collapse flag and the keys,
        // and the shell the frame's own pump would open, drained until its prompt is on screen.
        "terminal_shell" => stage_the_terminal(world, now),
        other => Err(format!("`{other}` is not a workbench state this catalog names")),
    }
}

/// **A point on a panel's edge**: the left panel's near (right) edge, halfway down, which is where a person points to ask
/// about a panel. Asked of the frame's own laid-out node, so it is the edge on this window and not a guessed pixel.
fn panel_edge(world: &mut World) -> Result<bevy::math::Vec2, String> {
    use bevy::ui::{ComputedNode, UiGlobalTransform};
    let left = world
        .get_resource::<crate::chrome::Frame>()
        .map(|frame| frame.left)
        .ok_or_else(|| "the frame has not been built, so there is no panel to point the style inspector at".to_owned())?;
    let (Some(node), Some(transform)) = (
        world.get::<ComputedNode>(left),
        world.get::<UiGlobalTransform>(left),
    ) else {
        return Err("the left panel is not laid out yet, so there is no edge to point at".to_owned());
    };
    let scale = node.inverse_scale_factor();
    let size = node.size() * scale;
    let at = transform.translation * scale;
    // **The panel's right edge, inset by a row's height**, so the point is over a classed node inside the panel rather than
    // on its border.
    let inset = crate::chrome::space(crate::chrome::STEP_TIGHT).max(4.0);
    Ok(bevy::math::Vec2::new(at.x + size.x * 0.5 - inset, at.y))
}

/// **The Design workspace, showing a page its own tree names** — so the capture's tree has pages in it, which is the state
/// the review could not reach. A project with no style pages is refused, naming where the first one goes, rather than
/// staged onto a tree with nothing in it.
fn stage_the_design_tree(world: &mut World, now: Screen) -> Result<Staged, String> {
    use crate::workspace::Workspace;
    // **Asked of the project's own tree reader**, not of the view's last snapshot: a stage is called from a fresh editor,
    // before the first sweep has run, and a refusal that depended on a sweep would refuse a project that has pages.
    let root = world
        .get_resource::<crate::project::Project>()
        .map(|project| project.root.clone())
        .ok_or_else(|| "no project is open, so there is no Design tree to show".to_owned())?;
    // The first page the tree lists: the game's own `style` when it has one (the game is first, `style` first in a place).
    let page = blueprint_model::design::Tree::read(&root)
        .places
        .iter()
        .find_map(|place| place.topics.first().map(|topic| topic.file.clone()))
        .ok_or_else(|| {
            format!(
                "`design_styles` needs a style page and {} has none: the game's goes at design/style.md",
                root.display()
            )
        })?;
    // **Crossed the way the door does** (`screen::cross`), so the Design panels are the ones a person's switch spawns. A map
    // with unsaved edits is not crossed from: the door would ask to save them first, and a stage does not answer for them.
    if world.get_resource::<Workspace>().copied() != Some(Workspace::Design) {
        if world.get_resource::<crate::project::OpenMap>().is_some_and(|open| open.dirty()) {
            return Err("the open map has unsaved edits, and leaving it for the Design workspace asks to save them first".to_owned());
        }
        crate::screen::cross(world, Workspace::Design)?;
    }
    crate::design_view::show_file(world, &page, None);
    Ok(Staged { id: "design_styles", screen: now })
}

/// **How long the `terminal_shell` stage waits for its shell's first prompt**, seconds. A login shell with a heavy rc file
/// (fish loading its plugins on a loaded host) takes a second or two; past this the stage refuses rather than hand back
/// a capture of a blank pane, which is the state the review already had.
pub const PROMPT_WITHIN_SECS: f32 = 5.0;

/// **The terminal raised, with its shell's prompt on screen** — or a refusal naming what is missing, because a pane over
/// an empty screen is the state the review already has.
///
/// The shell is the one the frame's own pump would open (`terminal_view::open_the_first_shell`: the person's choice, else
/// the environment's), and the call drains it until something is on its screen, so the stage is settled when it returns.
fn stage_the_terminal(world: &mut World, now: Screen) -> Result<Staged, String> {
    if !world.contains_resource::<crate::terminal_pane::TerminalPane>() {
        return Err("the editor has no terminal pane, so there is nothing to raise".to_owned());
    }
    if !world.contains_resource::<crate::terminal::Terminal>() {
        return Err("the editor has no terminal, so there is no pane to raise".to_owned());
    }
    world.resource_scope(|world, pane: Mut<crate::terminal_pane::TerminalPane>| {
        let mut terminal = world.resource_mut::<crate::terminal::Terminal>();
        // **The same gates the pump keeps**: before the settings are read a person who chose an agent would get a plain
        // shell, and with an agent chosen the pane invites it and starts nothing by itself.
        if !terminal.settings_taken() {
            return Err("the terminal has not read its settings yet, so it does not know which shell to start".to_owned());
        }
        if terminal.is_empty() && terminal.invites_the_agent() {
            return Err("this project chooses an agent for its terminal, so the pane invites it and starts no shell by itself".to_owned());
        }
        terminal.set_collapsed(false);
        crate::terminal_view::open_the_first_shell(&mut terminal, &pane);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs_f32(PROMPT_WITHIN_SECS);
        loop {
            terminal.pump();
            let Some(tab) = terminal.tab_mut() else {
                return Err("the shell left before it printed a prompt, and took its tab with it".to_owned());
            };
            if let Some(why) = tab.problem() {
                return Err(format!("the terminal's shell did not start: {why}"));
            }
            if tab.screen_mut().lines().iter().any(|line| line.cells.iter().any(|cell| !cell.ch.is_whitespace())) {
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!("the terminal's shell printed nothing within {PROMPT_WITHIN_SECS} s, so there is no prompt to capture"));
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    })?;
    // **Raised and holding the keys**, as the raise chord leaves a pane a person is about to type into.
    if let Some(mut focus) = world.get_resource_mut::<crate::terminal_pane::TerminalFocus>() {
        focus.take();
    }
    Ok(Staged { id: "terminal_shell", screen: now })
}

/// **A dock panel is staged by selecting its tab**, in every dock that holds it: the shell's arrangement is data, so this
/// is a write to that data and the dock shows it the same frame. A layout already showing the panel is left alone, so a
/// request for what is on screen does not reach `ui::save_the_layout` as a change.
fn stage_a_dock_panel(world: &mut World, panel: &str, now: Screen) -> Result<Staged, String> {
    let registered: Vec<&'static str> = match world.get_resource::<PanelRegistry>() {
        Some(registry) => dock_panels().into_iter().filter(|id| registry.contains(id)).collect(),
        None => Vec::new(),
    };
    if !registered.contains(&panel) {
        return Err(format!(
            "`{panel}` is not a panel this shell registers, so there is nothing to show. Registered: {}.",
            registered.join(", ")
        ));
    }
    if now != Screen::Editor {
        return Err(format!("`{panel}` is a workbench panel and the editor is not open (the screen is {now:?})"));
    }
    let mut docked = false;
    // **The arrangement's dock, and only it** (review of #247): a tab is selected in the dock that
    // arranges the shell, never in the bottom dock, whose one panel (`reload`) no capture stages.
    let mut docks = world.query_filtered::<&mut Dock, With<crate::chrome::ArrangementDock>>();
    for mut dock in docks.iter_mut(world) {
        if let Some(next) = select(&dock.0, panel) {
            dock.0 = next;
            docked = true;
        } else if dock.0.contains(panel) {
            docked = true;
        }
    }
    if !docked {
        return Err(format!(
            "`{panel}` is registered and no dock shows it, so there is no tab to select: the arrangement in force does not name it"
        ));
    }
    // The catalog's own `&'static str`, so a `Staged` never carries a caller's string.
    let id = stage(panel).map_or("unknown", |stage| stage.id);
    Ok(Staged { id, screen: now })
}

/// **The layout with `panel`'s tab selected**, or `None` when it is already the front one.
pub fn select(layout: &DockLayout, panel: &str) -> Option<DockLayout> {
    fn walk(node: &DockLayout, panel: &str) -> Option<DockLayout> {
        match node {
            DockLayout::Split { axis, fraction, first, second } => {
                let new_first = walk(first, panel);
                let new_second = walk(second, panel);
                (new_first.is_some() || new_second.is_some()).then(|| {
                    DockLayout::split(
                        *axis,
                        *fraction,
                        new_first.unwrap_or_else(|| (**first).clone()),
                        new_second.unwrap_or_else(|| (**second).clone()),
                    )
                })
            }
            DockLayout::Group { panels, selected } => {
                let at = panels.iter().position(|id| id == panel)?;
                (*selected != at).then(|| DockLayout::Group { panels: panels.clone(), selected: at })
            }
        }
    }
    walk(layout, panel)
}
