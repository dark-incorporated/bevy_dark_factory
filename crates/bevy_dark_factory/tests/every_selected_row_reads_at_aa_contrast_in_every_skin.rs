//! **Every selected row, tab and chip in the editor reads at AA contrast in every skin** (#445, from the clip batch: a selected label was grey-brown on amber and
//! hard to read). #436 fixed the Design tree's rows; this holds the rest to the same rule, measured the same way: each word is read **from the world after the
//! frames have run**, as drawn (its `TextColor`, composited over the translucent fills and the first opaque `BackgroundColor` at or above it), never from the
//! skin's constants, and the pair must clear [`bevy_gooey::TEXT_CONTRAST`] (4.5:1) in each theme the editor offers (Tool dark, Parchment, High contrast).
//!
//! **What counts as selected**, found in the world and not listed by hand: a gooey table or tree row carrying `bevy::ui::Selected` (`Role::TableRow`), a button
//! carrying it (a dock tab, a navigation tab, a card), and an editor list row whose `chrome::RowSelected` is set (the palette's, the lists of the Kit, Animation
//! and Sound doors). Every word under such a node is measured.
//!
//! **The surfaces below are the trees and lists this covers** (each opens one with a row selected, and must find the words it names, so a surface that stops
//! drawing a selection fails here instead of passing for want of anything to measure):
//!
//! | surface | opened by | words it must find |
//! |---|---|---|
//! | dock tabs, every workspace | the ten workspace chords | `INSPECTOR`, `VIEWPORT` |
//! | Files tree | the Files workspace, a file opened | `style.md` |
//! | Design tree | the Design workspace | `The game` |
//! | history list | the Map workspace | `now: the original map` |
//! | library palette | the Map workspace, a piece armed | `Wall` |
//! | outliner | View ▸ Show outliner, a piece selected | `Wall` |
//! | home | the launcher screen | `Projects` |
//! | settings tabs | Settings, opened by its message (the chord and the home's button write the same one) | `All` |
//! | command palette | the palette chord | the first command's words |
//!
//! **A new tree or list is added here** by writing one function that opens it with a row selected and adding a row to [`SURFACES`] naming the words it must find;
//! the measuring is [`words_of_selected`] and needs no change. A tree or list that is *not* in the table is still measured whenever it is on screen while one of
//! these is open (the scan reads the whole world), but nothing proves it was ever shown.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::Selected;
use bevy_dark_factory::chrome::RowSelected;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::{Action, binding};
use bevy_dark_factory::settings::{Theme, UserSettingsFile};

use crate::fixtures;
use crate::settle_support;

/// One frame of the test clock, in seconds.
const FRAME_S: f32 = 1.0 / 60.0;
/// The window the editor is laid out in, in logical px.
const WINDOW_PX: (u32, u32) = (1280, 720);
/// WCAG AA for text, as a ratio of luminances (1.4.3).
const AA: f32 = bevy_gooey::TEXT_CONTRAST;
const THEMES: [Theme; 3] = [Theme::ToolDark, Theme::Parchment, Theme::HighContrast];



fn write(root: &std::path::Path, rel: &str, text: &str) {
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().expect("a folder")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, text).unwrap_or_else(|e| panic!("{e}"));
}

/// The editor in `theme`, on a project with a library, a map, design pages and files, laid out at the window's size.
fn editor(theme: Theme) -> App {
    let tag = format!("every_selected_{theme:?}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    write(&root, "design/style.md", "# The game\n\nGrey concrete, sodium light.\n");
    write(&root, "design/dialogue.md", "# Dialogue\n\nShort lines.\n");
    write(&root, "design/levels/m/style.md", "# Yard\n\nSodium light on wet concrete.\n");
    // **Through the file, not the resource**: `sync_settings` re-resolves `EditorSettings` from the files every frame.
    let settings = root.join(format!("user_config_{tag}/settings.yaml"));
    std::fs::create_dir_all(settings.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&settings, format!("{{version: 1, theme: {theme:?}}}")).expect("a settings file");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(settings));
    harness::resize_surface(&mut app, WINDOW_PX.0, WINDOW_PX.1).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME_S)));
    settle_support::settle(&mut app);
    app
}

/// Press a workspace's chord, as a hand does.
fn open(app: &mut App, action: Action) {
    let row = binding(action);
    let mut keys = vec![row.key];
    if row.needs_mod {
        keys.push(if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft });
    }
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in &keys {
            app.world_mut().write_message(KeyboardInput { key_code: *code, logical_key: Key::Unidentified(NativeKey::Unidentified), state, text: None, repeat: false, window: Entity::PLACEHOLDER });
        }
        app.update();
        app.update();
    }
    settle_support::settle(app);
}

/// The ground a word is read on: every translucent fill from the word up to the first opaque one, composited over it (or over the clear colour).
fn ground_under(world: &World, entity: Entity) -> Color {
    let mut fills = Vec::new();
    let mut at = entity;
    loop {
        if let Some(fill) = world.get::<BackgroundColor>(at) {
            let fill = Srgba::from(fill.0);
            if fill.alpha > 0.0 {
                fills.push(fill);
                if fill.alpha >= 0.99 {
                    break;
                }
            }
        }
        match world.get::<ChildOf>(at) {
            Some(parent) => at = parent.parent(),
            None => break,
        }
    }
    let mut under = Srgba::from(world.resource::<ClearColor>().0);
    for fill in fills.iter().rev() {
        under = Srgba::new(
            fill.red * fill.alpha + under.red * (1.0 - fill.alpha),
            fill.green * fill.alpha + under.green * (1.0 - fill.alpha),
            fill.blue * fill.alpha + under.blue * (1.0 - fill.alpha),
            1.0,
        );
    }
    Color::Srgba(under)
}

fn contrast(ink: Color, ground: Color) -> f32 {
    let c = |colour: Color| {
        let c = Srgba::from(colour);
        [c.red, c.green, c.blue]
    };
    bevy_gooey::contrast_ratio(c(ink), c(ground))
}

/// One drawn word under a selected node.
struct Read {
    text: String,
    ink: Color,
    ground: Color,
    /// What the selected node is, for the message: its role, and which of the three marks it carries.
    node: String,
}

impl Read {
    fn ratio(&self) -> f32 {
        contrast(self.ink, self.ground)
    }
}

/// **Every word under every selected node of the world**: a row carrying `Selected` (a gooey table or tree row, a tab, a card) and a list row whose
/// `RowSelected` is set.
fn words_of_selected(app: &mut App) -> Vec<Read> {
    let world = app.world_mut();
    let mut selected: Vec<Entity> = world.query_filtered::<Entity, With<Selected>>().iter(world).collect();
    selected.extend(world.query::<(Entity, &RowSelected)>().iter(world).filter(|(_, mark)| mark.0).map(|(entity, _)| entity));
    selected.sort();
    selected.dedup();
    let mut words = Vec::new();
    for node in selected {
        let described = format!("{:?} (Selected: {}, RowSelected: {:?})", world.get::<bevy_gooey::Role>(node), world.get::<Selected>(node).is_some(), world.get::<RowSelected>(node).map(|r| r.0));
        let mut walk = vec![node];
        while let Some(entity) = walk.pop() {
            if let Some(children) = world.get::<Children>(entity) {
                walk.extend(children.iter());
            }
            let (Some(text), Some(ink)) = (world.get::<Text>(entity), world.get::<TextColor>(entity)) else { continue };
            if text.0.trim().is_empty() {
                continue;
            }
            words.push(Read { text: text.0.trim().to_owned(), ink: ink.0, ground: ground_under(world, entity), node: described.clone() });
        }
    }
    words
}

/// What opens a surface with a row selected.
type Open = fn(&mut App);

/// **The trees and lists the editor has**, each with what opens it and the words it must be found drawing.
const SURFACES: &[(&str, Open, &[&str])] = &[
    ("dock tabs", open_every_workspace, &["INSPECTOR", "VIEWPORT"]),
    ("Files tree", open_a_file, &["style.md"]),
    ("Design tree", open_design, &["The game"]),
    ("history list", open_map, &["now: the original map"]),
    ("library palette", arm_a_piece, &["Wall"]),
    ("outliner", select_in_the_outliner, &["Wall"]),
    ("home", stand_on_the_home, &["Projects"]),
    ("settings tabs", open_settings, &["All"]),
    ("command palette", open_the_palette, &["Compose: Arm this composition for the map"]),
];

fn stand_on_the_home(app: &mut App) {
    let recent = std::env::temp_dir().join(format!("every_selected_recent_{}", std::process::id()));
    std::fs::create_dir_all(&recent).expect("a scratch folder");
    app.insert_resource(bevy_dark_factory::recent::RecentProjects(recent.join("recent.yaml")));
    app.add_plugins(bevy_dark_factory::launcher::LauncherPlugin);
    app.world_mut().resource_mut::<NextState<bevy_dark_factory::screen::Screen>>().set(bevy_dark_factory::screen::Screen::Launcher);
    settle_support::settle(app);
}

fn open_settings(app: &mut App) {
    app.world_mut().write_message(bevy_dark_factory::settings_screen::OpenSettings);
    settle_support::settle(app);
}

fn open_the_palette(app: &mut App) {
    let primary = if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft };
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in [primary, KeyCode::KeyK] {
            app.world_mut().write_message(KeyboardInput { key_code: code, logical_key: Key::Unidentified(NativeKey::Unidentified), state, text: None, repeat: false, window: Entity::PLACEHOLDER });
        }
        app.update();
    }
    settle_support::settle(app);
}

fn open_every_workspace(app: &mut App) {
    for action in [
        Action::WorkspaceProject,
        Action::WorkspaceMap,
        Action::WorkspaceWorld,
        Action::WorkspaceKit,
        Action::WorkspaceAnimation,
        Action::WorkspaceSound,
        Action::WorkspaceFiles,
        Action::WorkspaceGame,
        Action::WorkspaceAgent,
    ] {
        open(app, action);
        // Each workspace's own selected tabs are read at the moment it is open.
        let words = words_of_selected(app);
        assert!(words.iter().any(|w| w.text == "INSPECTOR" || w.text == "VIEWPORT"), "{action:?}: the workspace's dock draws a selected tab");
        VISITED.with(|v| v.borrow_mut().extend(words));
    }
}

thread_local! {
    /// The words read while a surface was opening, for the surfaces that visit more than one screen.
    static VISITED: std::cell::RefCell<Vec<Read>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn open_a_file(app: &mut App) {
    open(app, Action::WorkspaceFiles);
    bevy_dark_factory::files::open_file(app.world_mut(), "design/style.md").unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(app);
}

fn open_design(app: &mut App) {
    open(app, Action::WorkspaceDesign);
}

fn open_map(app: &mut App) {
    open(app, Action::WorkspaceMap);
}

fn arm_a_piece(app: &mut App) {
    open(app, Action::WorkspaceMap);
    app.world_mut().resource_mut::<bevy_dark_factory::editor::EditorState>().brush = Some(0);
    settle_support::settle(app);
}

fn select_in_the_outliner(app: &mut App) {
    open(app, Action::WorkspaceMap);
    let row = app.world_mut().query_filtered::<Entity, With<bevy_dark_factory::outliner::OutlinerItem>>().single(app.world()).expect("the View menu carries one Show outliner row");
    app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange { source: row, value: true, is_final: true });
    settle_support::settle(app);
    let id = app.world().resource::<bevy_dark_factory::project::OpenMap>().map.placements[0].id.clone();
    assert!(app.world_mut().resource_mut::<bevy_dark_factory::selection::Selection>().only(&id), "the piece is selected");
    settle_support::settle(app);
}

/// **In every skin, every word under a selected row, tab or chip of every surface clears 4.5:1 against what it is drawn on.**
#[test]
fn every_selected_row_reads_at_aa_contrast_in_every_skin() {
    let mut refused = Vec::new();
    for theme in THEMES {
        for (surface, opener, must_find) in SURFACES {
            let mut app = editor(theme);
            VISITED.with(|v| v.borrow_mut().clear());
            opener(&mut app);
            let mut words = words_of_selected(&mut app);
            words.extend(VISITED.with(|v| std::mem::take(&mut *v.borrow_mut())));
            for needle in *must_find {
                assert!(words.iter().any(|w| w.text == *needle), "{theme:?} / {surface}: no selected word `{needle}` was drawn, so nothing was measured there; found {:?}", words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>());
            }
            for word in &words {
                let ratio = word.ratio();
                if ratio < AA {
                    refused.push(format!("{theme:?} / {surface}: `{}` is {ratio:.2}:1 (ink {:?} on {:?}), under {AA}:1, on {}", word.text, Srgba::from(word.ink), Srgba::from(word.ground), word.node));
                }
            }
        }
    }
    refused.sort();
    refused.dedup();
    assert!(refused.is_empty(), "{} selected word(s) are not readable:\n{}", refused.len(), refused.join("\n"));
}
