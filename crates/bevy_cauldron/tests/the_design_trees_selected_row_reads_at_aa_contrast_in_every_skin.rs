//! **The Design tree's selected row is readable in every skin** (#436). Found recording the design workspace clip: the selected row drew its label dark grey
//! on a grey selection. The row's words and its ground are read **from the world after the frames have run**, as drawn (the label's `TextColor`, the
//! first opaque `BackgroundColor` at or above it), never from the skin's constants alone, and the pair must clear WCAG AA for text, 4.5:1
//! ([`bevy_gooey::TEXT_CONTRAST`]), in each theme the editor offers.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::Selected;
use bevy_cauldron::design_view::DesignView;
use bevy_cauldron::harness;
use bevy_cauldron::keys::{Action, binding};
use bevy_cauldron::settings::{Theme, UserSettingsFile};
use bevy_cauldron::workspace::Workspace;
use bevy_gooey::{Table, TableRow};

use crate::fixtures;

/// Frames run after each step: enough for gooey's colour glide to arrive at the colour it is drawn in.
const SETTLE: usize = 40;
/// One frame of the test clock, in seconds.
const FRAME_S: f32 = 1.0 / 60.0;
/// The window the editor is laid out in, in logical px.
const WINDOW_PX: (u32, u32) = (1280, 720);
/// WCAG AA for text, as a ratio of luminances (1.4.3).
const AA: f32 = bevy_gooey::TEXT_CONTRAST;

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn write(root: &std::path::Path, rel: &str, text: &str) {
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().expect("a folder")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, text).unwrap_or_else(|e| panic!("{e}"));
}

/// The editor in `theme`, on a project with a design tree (the game, a topic and a level), laid out at the window's size.
fn editor(theme: Theme) -> App {
    let tag = format!("design_selected_row_{theme:?}_{}", std::process::id());
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
    settle(&mut app);
    app
}

/// Press the Design workspace's chord, as a hand does.
fn open_design(app: &mut App) {
    let row = binding(Action::WorkspaceDesign);
    let mut keys = vec![row.key];
    if row.needs_mod {
        keys.push(if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft });
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
    settle(app);
}

/// The first opaque fill at or above `entity`, composited over the clear colour: the ground a word is read on.
fn ground_under(world: &World, entity: Entity) -> Option<Color> {
    let mut at = entity;
    loop {
        if let Some(fill) = world.get::<BackgroundColor>(at) {
            let fill = Srgba::from(fill.0);
            if fill.alpha >= 0.9 {
                let clear = Srgba::from(world.resource::<ClearColor>().0);
                return Some(Color::srgb(
                    fill.red * fill.alpha + clear.red * (1.0 - fill.alpha),
                    fill.green * fill.alpha + clear.green * (1.0 - fill.alpha),
                    fill.blue * fill.alpha + clear.blue * (1.0 - fill.alpha),
                ));
            }
        }
        at = world.get::<ChildOf>(at)?.parent();
    }
}

fn contrast(ink: Color, ground: Color) -> f32 {
    let c = |colour: Color| {
        let c = Srgba::from(colour);
        [c.red, c.green, c.blue]
    };
    bevy_gooey::contrast_ratio(c(ink), c(ground))
}

/// Every word of the selected rows of the Design tree: its text, its drawn ink and the ground under it.
fn selected_words(app: &mut App) -> Vec<(String, Color, Color)> {
    let world = app.world_mut();
    let tree: Vec<Entity> = world.query_filtered::<Entity, (With<Table>, With<bevy_cauldron::design_view::DesignTree>)>().iter(world).collect();
    let rows: Vec<Entity> = world
        .query_filtered::<(Entity, &TableRow), With<Selected>>()
        .iter(world)
        .filter(|(_, row)| tree.contains(&row.table))
        .map(|(entity, _)| entity)
        .collect();
    let mut words = Vec::new();
    let mut walk = rows;
    while let Some(entity) = walk.pop() {
        if let Some(children) = world.get::<Children>(entity) {
            walk.extend(children.iter());
        }
        let (Some(text), Some(ink)) = (world.get::<Text>(entity), world.get::<TextColor>(entity)) else { continue };
        if text.0.trim().is_empty() {
            continue;
        }
        let Some(ground) = ground_under(world, entity) else { continue };
        words.push((text.0.clone(), ink.0, ground));
    }
    words
}

/// **In every skin the selected row's label clears 4.5:1 against the row it is drawn on**, with the row the person is looking at selected (the game's, which
/// the workspace opens on).
#[test]
fn the_selected_row_of_the_design_tree_reads_at_aa_contrast_in_every_skin() {
    let mut refused = Vec::new();
    for theme in [Theme::ToolDark, Theme::Parchment, Theme::HighContrast] {
        let mut app = editor(theme);
        open_design(&mut app);
        assert_eq!(*app.world().resource::<Workspace>(), Workspace::Design, "{theme:?}: the Design workspace is open");
        assert!(app.world().resource::<DesignView>().selected().is_some(), "{theme:?}: a page is selected");
        {
            let world = app.world_mut();
            let rows: Vec<Entity> = world.query_filtered::<Entity, (With<TableRow>, With<Selected>)>().iter(world).collect();
            for row in rows {
                let mut at = Some(row);
                while let Some(e) = at {
                    eprintln!("  DBG {theme:?} {e}: bg={:?} role={:?} hovered={:?}", world.get::<BackgroundColor>(e).map(|b| Srgba::from(b.0)), world.get::<bevy_gooey::Role>(e), world.get::<bevy::picking::hover::Hovered>(e).map(|h| h.get()));
                    at = world.get::<ChildOf>(e).map(ChildOf::parent);
                    if at.is_some_and(|a| world.get::<TableRow>(a).is_none() && world.get::<Table>(a).is_some()) { break; }
                }
                let mut kids = vec![row];
                while let Some(k) = kids.pop() {
                    if let Some(c) = world.get::<Children>(k) { kids.extend(c.iter()); }
                    if let Some(t) = world.get::<Text>(k) { eprintln!("  DBG text {:?} color={:?} role={:?}", t.0, world.get::<TextColor>(k).map(|c| Srgba::from(c.0)), world.get::<bevy_gooey::Role>(k)); }
                }
            }
        }
        let words = selected_words(&mut app);
        assert!(!words.is_empty(), "{theme:?}: the tree's selected row draws a label");
        for (text, ink, ground) in words {
            let ratio = contrast(ink, ground);
            if ratio < AA {
                refused.push(format!("{theme:?}: `{text}` is {ratio:.2}:1 (ink {:?} on {:?}), under {AA}:1", Srgba::from(ink), Srgba::from(ground)));
            }
        }
    }
    assert!(refused.is_empty(), "the selected row of the Design tree is not readable:\n{}", refused.join("\n"));
}
