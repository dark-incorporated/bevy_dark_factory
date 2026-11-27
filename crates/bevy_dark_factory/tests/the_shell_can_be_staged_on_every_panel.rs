//! **Every panel can be staged for a capture** (UI-92, #258).
//!
//! A clip of a panel has to start on that panel. [`stage_panels::STAGES`] is the catalog and [`stage_panels::arrange`] the
//! one function that puts the shell on one: a screen is a `Screen` transition, the settings screen is the dialog over the
//! workbench, and a dock panel is its group's tab selected — the arrangement a person's click makes, so a clip shows what
//! a person would be looking at rather than a layout built for the camera.
//!
//! These tests read the registry and the dock the editor actually has, so a panel added to the shell without a stage, or a
//! stage with nothing behind it, fails here rather than in a blank clip.

use bevy::prelude::*;
use bevy_dark_factory::screen::Screen;
use bevy_dark_factory::stage_panels::{self, STAGES};
use bevy_gooey::{Dock, DockLayout, PanelRegistry};

use crate::fixtures;

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    updates(&mut app, 8);
    app
}

/// The tab each group of `layout` shows, one entry per group, in tree order.
fn shown_tabs(layout: &DockLayout) -> Vec<(Vec<String>, String)> {
    fn walk(node: &DockLayout, out: &mut Vec<(Vec<String>, String)>) {
        match node {
            DockLayout::Split { first, second, .. } => {
                walk(first, out);
                walk(second, out);
            }
            DockLayout::Group { panels, selected } => {
                out.push((panels.clone(), panels.get(*selected).cloned().unwrap_or_default()));
            }
        }
    }
    let mut out = Vec::new();
    walk(layout, &mut out);
    out
}

/// The dock's arrangement as the editor holds it, or the default one when no dock has been built yet.
fn dock_here(app: &mut App) -> DockLayout {
    app.world_mut()
        .query_filtered::<&Dock, With<bevy_dark_factory::chrome::ArrangementDock>>()
        .iter(app.world())
        .next()
        .map_or_else(|| bevy_dark_factory::ui::shell_layout(bevy_dark_factory::workspace::Workspace::Map), |dock| dock.0.clone())
}

/// **The catalog names every panel the shell has, and each name is a screen or a registered panel.** A dock panel in the
/// catalog and not in the registry is a name `record.py` would send and nothing would show.
#[test]
fn the_catalog_is_the_shells_own_panels() {
    let mut app = editor(&format!("stage_catalog_{}", std::process::id()));
    let registry = app.world_mut().resource::<PanelRegistry>().clone();
    let missing: Vec<&str> = stage_panels::dock_panels().into_iter().filter(|id| !registry.contains(id)).collect();
    assert!(missing.is_empty(), "a staged panel the shell does not register would be a blank clip: {missing:?}");
    let staged = stage_panels::dock_panels().len();
    assert!(staged >= 3, "the shell has more than three dock panels: {staged}");
    // **And nothing registered is left without an answer**: either it stages, or the catalog says why it does not.
    for id in ["inspector", "viewport", "place", "status"] {
        assert!(registry.contains(id), "`{id}` is a panel the shell registers");
        let covered = stage_panels::dock_panels().contains(&id) || stage_panels::NOT_STAGED.iter().any(|(name, _)| *name == id);
        assert!(covered, "`{id}` is registered and no capture can stage it, with no reason on the record");
    }
    for screen in ["launcher", "loader", "settings"] {
        assert!(stage_panels::is_screen(screen), "{screen} is a screen, not a dock panel");
        assert!(stage_panels::stage(screen).is_some(), "the catalog names {screen}");
    }
    assert!(stage_panels::stage("no.such.panel").is_none());
    assert!(stage_panels::dock_panels().iter().all(|id| !stage_panels::is_screen(id)));
}

/// **Every panel in the catalog stages**, and the answer names the screen it left the editor on. The run walks
/// [`stage_panels::capture_order`], which is the order `record.py` stages them in, so the test fails if that order is one
/// the editor cannot follow.
#[test]
fn every_panel_in_the_catalog_stages() {
    let mut app = editor(&format!("stage_every_{}", std::process::id()));
    // **What the workbench states need to be shown** (#696): a style page for `design_styles` to open, and a POSIX shell
    // for `terminal_shell` to wait on, so the walk does not depend on the host's login shell.
    let root = app.world().resource::<bevy_dark_factory::project::Project>().root.clone();
    std::fs::create_dir_all(root.join("design")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("design/style.md"), "# The game\n").unwrap_or_else(|e| panic!("{e}"));
    app.world_mut()
        .resource_mut::<bevy_dark_factory::terminal::Terminal>()
        .set_shell(Some(bevy_dark_factory::terminal::ShellChoice { program: "/bin/sh".to_owned(), args: Vec::new() }));
    let order = stage_panels::capture_order();
    assert_eq!(order.len(), STAGES.len(), "the capture order names every panel once: {order:?}");
    let mut staged = Vec::new();
    for id in &order {
        let stage = stage_panels::stage(id).unwrap_or_else(|| panic!("{id} is not in the catalog"));
        let answer = stage_panels::arrange(app.world_mut(), stage.id)
            .unwrap_or_else(|e| panic!("`{}` is in the catalog and will not stage: {e}", stage.id));
        assert_eq!(answer.id, stage.id);
        let want = match stage.id {
            "launcher" => Screen::Launcher,
            "loader" => Screen::Loading,
            _ => Screen::Editor,
        };
        assert_eq!(answer.screen, want, "`{}` says which screen it staged", stage.id);
        staged.push(stage.id);
    }
    assert_eq!(staged.len(), STAGES.len(), "one answer per panel, and the test visited every one");
    // **The last stage's move is queued**, which is what a capture started in the same frame reads: a state change is
    // applied by the next state-transition pass, so the queue is the editor's answer this frame.
    assert!(
        matches!(app.world().resource::<NextState<Screen>>(), NextState::Pending(Screen::Launcher)),
        "the last stage in the order is the launcher: {:?}",
        app.world().resource::<NextState<Screen>>()
    );
}

/// **A dock panel is staged by selecting its tab**, in every group that holds it, and the layout's other groups are left as
/// they were.
#[test]
fn a_dock_panel_is_staged_by_selecting_its_tab() {
    let mut app = editor(&format!("stage_dock_{}", std::process::id()));
    let groups = shown_tabs(&dock_here(&mut app));
    assert!(groups.len() >= 2, "the shell has a left dock and a right one: {groups:?}");
    for (panels, was_showing) in groups {
        // The tab that is not in front, where the group has more than one.
        let panel = panels.iter().find(|p| **p != was_showing).cloned().unwrap_or(was_showing.clone());
        stage_panels::arrange(app.world_mut(), &panel).unwrap_or_else(|e| panic!("{panel}: {e}"));
        let now = shown_tabs(&dock_here(&mut app));
        assert!(
            now.iter().any(|(group, shown)| group.contains(&panel) && *shown == panel),
            "`{panel}` is the tab its group shows after being staged: {now:?}"
        );
    }
}

/// **The four workbench states #696 names are stage ids, and each is reached in one call** — the acceptance the issue
/// states. A state that stages and leaves nothing on screen is the blank clip the issue is about, so each test asks the
/// world what the stage arranged rather than trusting the answer.
#[test]
fn the_states_three_reviews_could_not_reach_are_stage_ids() {
    for id in ["style_inspector", "section_cut", "design_styles", "terminal_shell"] {
        assert!(stage_panels::stage(id).is_some(), "`{id}` is a stage id");
        assert!(stage_panels::is_state(id), "`{id}` is a workbench state, not a screen and not a dock panel");
        assert!(!stage_panels::is_screen(id), "`{id}` is not a screen");
        assert!(!stage_panels::dock_panels().contains(&id), "`{id}` is not a dock panel, so the catalog cannot ask for its tab");
    }
}

/// **The style inspector, up and asked about a panel edge**: the overlay exists in the world after the one call, which is
/// what a picture of it needs.
#[test]
fn the_style_inspector_stages_up_on_a_panel_edge() {
    let mut app = editor(&format!("stage_style_{}", std::process::id()));
    let staged = stage_panels::arrange(app.world_mut(), "style_inspector").unwrap_or_else(|e| panic!("#696's style inspector stage: {e}"));
    assert_eq!(staged.id, "style_inspector");
    assert_eq!(staged.screen, Screen::Editor, "the inspector is up over the workbench");
    assert!(app.world().resource::<bevy_dark_factory::style_inspect::StyleInspector>().on, "the inspector is on");
    let overlays = app.world_mut().query_filtered::<Entity, With<bevy_dark_factory::style_inspect::StyleInspectorRoot>>().iter(app.world()).count();
    assert_eq!(overlays, 1, "the overlay is up, asked about the panel edge, after one call");
}

/// **The section cut, on**: the one number `blueprint/section_cut` writes, written by the stage, and nothing else.
#[test]
fn the_section_cut_stages_on() {
    let mut app = editor(&format!("stage_cut_{}", std::process::id()));
    assert_eq!(app.world().resource::<bevy_dark_factory::view::SectionCut>().metres(), None, "off before it is staged");
    let staged = stage_panels::arrange(app.world_mut(), "section_cut").unwrap_or_else(|e| panic!("#696's section cut stage: {e}"));
    assert_eq!(staged.id, "section_cut");
    assert_eq!(
        app.world().resource::<bevy_dark_factory::view::SectionCut>().metres(),
        Some(stage_panels::STAGE_CUT_M),
        "the cut stands where the stage put it"
    );
}

/// **The Design workspace, showing a page its own tree names**: the workspace is crossed to and the first page the tree
/// lists — the game's own `style` — is the one open, so the capture shows the tree and a page together.
#[test]
fn the_design_styles_stage_shows_a_populated_tree() {
    let mut app = editor(&format!("stage_design_{}", std::process::id()));
    let root = app.world().resource::<bevy_dark_factory::project::Project>().root.clone();
    std::fs::create_dir_all(root.join("design/levels/m")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("design/levels/m/style.md"), "# Yard\n").unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(root.join("design/style.md"), "# The game\n").unwrap_or_else(|e| panic!("{e}"));
    let staged = stage_panels::arrange(app.world_mut(), "design_styles").unwrap_or_else(|e| panic!("#696's design styles stage: {e}"));
    assert_eq!(staged.id, "design_styles");
    assert_eq!(*app.world().resource::<bevy_dark_factory::workspace::Workspace>(), bevy_dark_factory::workspace::Workspace::Design);
    // The tree the capture shows is the project's own, read by its one reader: the game, the level written above, and the
    // page the stage opened.
    let tree = blueprint_model::design::Tree::read(&root);
    let paths: Vec<&str> = tree.places.iter().map(|place| place.path.as_str()).collect();
    assert!(paths.contains(&"design/levels/m"), "the level the test wrote is in the tree: {paths:?}");
    let selected = app.world().resource::<bevy_dark_factory::design_view::DesignView>().selected().map(str::to_owned);
    assert_eq!(selected.as_deref(), Some("design/style.md"), "the game's own style page, the tree's first, is the one open");
}

/// **A project with no style pages is refused, naming where the first one goes**, and the workspace is left where it was:
/// a stage onto an empty tree is the capture the review already had.
#[test]
fn the_design_styles_stage_refuses_a_project_with_no_pages() {
    let mut app = editor(&format!("stage_design_none_{}", std::process::id()));
    let before = *app.world().resource::<bevy_dark_factory::workspace::Workspace>();
    let refused = stage_panels::arrange(app.world_mut(), "design_styles").expect_err("a project with no style page has nothing to show");
    assert!(refused.contains("design/style.md"), "the refusal says where the game's page goes: {refused}");
    assert_eq!(*app.world().resource::<bevy_dark_factory::workspace::Workspace>(), before, "a refused stage crosses nowhere");
}

/// **The terminal, raised in front of a shell that has printed its prompt**, when the one call returns: a pane over a
/// blank screen is the capture the review already had. The shell is a POSIX `sh`, the person's choice the pane honours, so
/// the test does not depend on the login shell of the host it runs on.
#[test]
fn the_terminal_shell_stage_raises_the_pane_over_a_prompt() {
    use bevy_dark_factory::terminal::{ShellChoice, Terminal};
    let mut app = editor(&format!("stage_terminal_{}", std::process::id()));
    {
        let mut terminal = app.world_mut().resource_mut::<Terminal>();
        terminal.set_collapsed(true);
        terminal.set_shell(Some(ShellChoice { program: "/bin/sh".to_owned(), args: Vec::new() }));
    }
    assert!(app.world().resource::<Terminal>().is_empty(), "no shell before the stage: the pane was never raised");
    let staged = stage_panels::arrange(app.world_mut(), "terminal_shell").unwrap_or_else(|e| panic!("#696's terminal stage: {e}"));
    assert_eq!(staged.id, "terminal_shell");
    let mut terminal = app.world_mut().resource_mut::<Terminal>();
    assert!(!terminal.is_collapsed(), "the pane is raised");
    let tab = terminal.tab_mut().expect("the stage opened a shell");
    let screen: String = tab.screen_mut().lines().iter().flat_map(|line| line.cells.iter().map(|cell| cell.ch)).collect();
    assert!(screen.contains('$') || screen.contains('#'), "the shell's prompt is on screen when the call returns: {screen:?}");
}

/// **A workbench state is refused off the workbench**, as a dock panel is, rather than writing state into a closed editor.
#[test]
fn a_workbench_state_is_refused_off_the_workbench() {
    let mut app = editor(&format!("stage_state_closed_{}", std::process::id()));
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    updates(&mut app, 6);
    let refused = stage_panels::arrange(app.world_mut(), "terminal_shell").expect_err("the workbench is closed");
    assert!(refused.contains("terminal_shell") && refused.contains("Launcher"), "{refused}");
}

/// **A name the shell has not got is refused in words that say what it has.**
#[test]
fn an_unknown_panel_is_refused_by_name() {
    let mut app = editor(&format!("stage_refused_{}", std::process::id()));
    let refused = stage_panels::arrange(app.world_mut(), "not a panel").expect_err("an unknown name stages nothing");
    assert!(refused.contains("not a panel"), "{refused}");
    assert!(refused.contains("inspector"), "the refusal names what would have worked: {refused}");
}

/// **A workbench panel cannot be staged from another screen**, and the refusal says so rather than leaving a stale layout
/// behind for the next capture.
#[test]
fn a_workbench_panel_is_refused_off_the_workbench() {
    let mut app = editor(&format!("stage_closed_{}", std::process::id()));
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    updates(&mut app, 6);
    let refused = stage_panels::arrange(app.world_mut(), "place").expect_err("the workbench is closed");
    assert!(refused.contains("place") && refused.contains("Launcher"), "{refused}");
}

/// **Staging a panel of the workbench leaves the project open**, so a capture of the next panel starts where the last one
/// ended rather than on an empty editor.
#[test]
fn staging_the_workbench_leaves_the_project_open() {
    let mut app = editor(&format!("stage_keeps_{}", std::process::id()));
    let root = app.world().resource::<bevy_dark_factory::project::Project>().root.clone();
    stage_panels::arrange(app.world_mut(), "viewport").expect("the viewport stages");
    updates(&mut app, 3);
    assert_eq!(app.world().resource::<bevy_dark_factory::project::Project>().root, root);
    assert_eq!(*app.world().resource::<State<Screen>>().get(), Screen::Editor);
}

/// **The answer a caller gets is the arrangement, not a promise.** `blueprint/stage_panel` answers with what the shell is now
/// on; a queued arrangement could only report a refusal in a log line, and the live editor measured on 2026-11-09 answered
/// `staged: true` for an `inspector` the loading screen had no workbench to show — the warning sat in the editor's own log
/// while the caller read success. So the arrangement runs in the request, and a refusal is what the caller gets.
#[test]
fn the_staging_command_answers_with_the_arrangement_or_the_refusal() {
    let mut app = editor(&format!("stage_op_{}", std::process::id()));
    // The workbench's own panel: arranged, and the answer names the screen it is on.
    let staged = stage_panels::arrange(app.world_mut(), "viewport").expect("the viewport stages");
    assert_eq!(staged.id, "viewport");
    assert_eq!(staged.screen, Screen::Editor);
    // A name nobody has is a refusal in words, which is what the command returns as its error.
    let refused = stage_panels::arrange(app.world_mut(), "nowhere").expect_err("a name the shell has not got");
    assert!(refused.contains("nowhere") && refused.contains("inspector"), "{refused}");
    // And a workbench panel with the workbench closed refuses rather than arranging a tab nobody can see.
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    updates(&mut app, 6);
    let refused = stage_panels::arrange(app.world_mut(), "place").expect_err("the workbench is closed");
    assert!(refused.contains("place") && refused.contains("Launcher"), "{refused}");
}

/// **Staging a panel selects its tab in the arrangement's dock, never in the bottom dock** (review of
/// #247, MUST-FIX 1).
///
/// `stage_panels::stage_a_dock_panel` writes the selected tab of every dock it can reach. With a second
/// gooey `Dock` in the frame an unfiltered loop also visited the bottom dock, whose one panel is
/// `reload` — a capture asking for `place` left the strip alone, but a `select()` that matched there
/// would have rewritten the bottom dock's layout and despawned its page. This asks the bottom dock's
/// layout directly after staging each workbench panel.
#[test]
fn staging_a_panel_leaves_the_bottom_dock_alone() {
    use bevy_dark_factory::chrome::BottomDock;

    let mut app = editor(&format!("stage_bottom_{}", std::process::id()));
    let bottom = {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<BottomDock>>()
            .single(world)
            .expect("the frame has a bottom dock")
            .to_owned()
    };
    let before = app.world().get::<Dock>(bottom).expect("a gooey Dock").0.clone();
    for id in stage_panels::dock_panels() {
        stage_panels::arrange(app.world_mut(), id).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert_eq!(
            app.world().get::<Dock>(bottom).map(|d| d.0.clone()),
            Some(before.clone()),
            "staging `{id}` rewrote the bottom dock's layout"
        );
    }
}
