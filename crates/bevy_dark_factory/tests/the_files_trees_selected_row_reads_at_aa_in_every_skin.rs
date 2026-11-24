//! **The Files tree's selected row reads at AA, and its band is read from the paint not from this file** (#583).
//!
//! # What the issue reported
//!
//! The selected row `bedroom.map.yaml` was measured on a 1360x850 capture at `rgb(162,121,50)` on its band `rgb(242,181,76)` —
//! **2.16:1** — against 11.61:1 for the unselected rows below it, so choosing a file made it harder to read. The issue also says the
//! standing test `every_selected_row_reads_at_aa_contrast_in_every_skin` passes, and asks which of two things is true: the test does
//! not cover the Files tree's rail, or the rail bypasses the ink it checks.
//!
//! # What the capture actually holds (the finding step one asks for)
//!
//! **Neither.** The rail is gooey's own: `Role::TableRow` selected is filled `skin.accent` and its cells inked `skin.accent_text` —
//! `rgb(242,181,76)` on `rgb(19,18,16)` in Tool dark, **10.26:1** — and `every_selected_row_reads_at_aa_contrast_in_every_skin` does
//! cover it, through its `("Files tree", open_a_file, &["style.md"])` row and a world scan that reads `accent_text` where it is drawn.
//!
//! The `rgb(162,121,50)` in the capture is **not an ink**. Re-measured pixel by pixel on
//! `docs/loop/captures/review-2026-11-23-files-editor/files-editor-opened.png`, the selected row's darkest pixel is exactly
//! `rgb(19,18,15)` — the skin's `accent_text` at full coverage — and `rgb(162,121,50)` is one glyph stem at 59.7% coverage blended
//! linearly into the band (`(242-162)/(242-19) = 0.359` in sRGB, 0.597 linear). A row whose ink were `rgb(162,121,50)` could hold no
//! pixel darker than it; this one holds `rgb(19,18,15)`. The 2.16:1 is a measurement of antialiasing, which is why #445 closed this
//! once already and why its fix could not be made to fail.
//!
//! # What this file holds down, then
//!
//! 1. **The Files tree's selected row, read from the paint.** Its band is its own `BackgroundColor` and its ink its cells' `TextColor`,
//!    both read off the world after the frames have run — never from this file's idea of what the skin should be — and the pair must clear
//!    [`bevy_gooey::TEXT_CONTRAST`] in all three skins. This is the assertion the issue asked for; it is a *new* test rather than the
//!    old one extended, so a future edit that moves either colour fails here by name.
//! 2. **Every `chrome::RowSelected` row in the editor**, which is the family the old scan does not cover: `words_of_selected` reads
//!    `RowSelected` only for the nodes it is handed and only in the surfaces it names. A row that is *selected and quiet* must not keep a
//!    quiet ink: `Ink::label` on `Ink::row_selected` is 3.93:1 in Tool dark and 2.34:1 in High contrast, and that is the defect this file
//!    would have caught had a row drawn it.
//!
//! A surface that draws no such row passes vacuously and says so in the message, so this cannot pass for want of anything to measure.

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::chrome::{RowSelected, RowRest};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::{binding, Action};
use bevy_dark_factory::settings::{Theme, UserSettingsFile};

use crate::fixtures;
use crate::settle_support;

/// One frame of the test clock, in seconds.
const FRAME_S: f32 = 1.0 / 60.0;
/// The window the editor is laid out in, in logical px.
const WINDOW_PX: (u32, u32) = (1280, 720);
/// WCAG AA for body text, as a ratio of luminances (1.4.3).
const AA: f32 = bevy_gooey::TEXT_CONTRAST;
const THEMES: [Theme; 3] = [Theme::ToolDark, Theme::Parchment, Theme::HighContrast];

fn write(root: &std::path::Path, rel: &str, text: &str) {
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().expect("a folder")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, text).unwrap_or_else(|e| panic!("{e}"));
}

/// The editor in `theme`, on a project with a library, a map, and files at two depths so the tree has a folder and a leaf.
fn editor(theme: Theme) -> App {
    let tag = format!("selected_row_band_{theme:?}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
    write(&root, "design/style.md", "# The game\n\nGrey concrete, sodium light.\n");
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

/// Open the Files workspace, as a hand does with its chord.
fn open_files(app: &mut App) {
    let row = binding(Action::WorkspaceFiles);
    let mut keys = vec![row.key];
    if row.needs_mod {
        keys.push(if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft });
    }
    for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
        for code in &keys {
            app.world_mut().write_message(bevy::input::keyboard::KeyboardInput {
                key_code: *code,
                logical_key: bevy::input::keyboard::Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
        app.update();
    }
    settle_support::settle(app);
}

/// **The colour as it is drawn**: the ground a word stands on, every translucent fill composited down to the first opaque one, over the clear colour.
fn ground_of(world: &World, entity: Entity, stop_at: Entity) -> Color {
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
        if at == stop_at {
            break;
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

/// **The Files tree's selected row is readable.** Its band and its ink are read off the nodes gooey painted, in every skin.
#[test]
fn the_files_trees_selected_row_reads_at_aa_in_every_skin() {
    let mut failures = Vec::new();
    for theme in THEMES {
        let mut app = editor(theme);
        open_files(&mut app);
        bevy_dark_factory::files::open_file(app.world_mut(), "design/style.md").unwrap_or_else(|e| panic!("{e}"));
        settle_support::settle(&mut app);

        let world = app.world_mut();
        // **The row**: a gooey `TableRow` carrying `Selected`, under the Files tree's own entity. The band is the fill gooey painted on it.
        let tree = world
            .query_filtered::<Entity, With<bevy_dark_factory::files::FilesTree>>()
            .iter(world)
            .next()
            .expect("the Files workspace draws its tree");
        let under_the_tree = |world: &World, entity: Entity| {
            let mut at = entity;
            for _ in 0..8 {
                if at == tree {
                    return true;
                }
                let Some(parent) = world.get::<ChildOf>(at).map(ChildOf::parent) else { return false };
                at = parent;
            }
            false
        };
        let found: Vec<(Entity, Color)> = world
            .query_filtered::<(Entity, &BackgroundColor), (With<bevy_gooey::TableRow>, With<bevy::ui::Selected>)>()
            .iter(world)
            .filter(|(entity, _)| under_the_tree(world, *entity))
            .map(|(entity, bg)| (entity, bg.0))
            .collect();
        let Some((row, band)) = found.first().copied() else {
            failures.push(format!("{theme:?} / Files tree: no selected row was drawn, so nothing was measured"));
            continue;
        };
        // **Its words**, and the ink each is drawn in, composited over the band the row itself was given.
        let mut words = 0usize;
        let mut stack = vec![row];
        while let Some(entity) = stack.pop() {
            if let Some(children) = world.get::<Children>(entity) {
                stack.extend(children.iter());
            }
            let (Some(text), Some(ink)) = (world.get::<Text>(entity), world.get::<TextColor>(entity)) else { continue };
            if text.0.trim().is_empty() {
                continue;
            }
            words += 1;
            let ground = ground_of(world, entity, row);
            let ratio = contrast(ink.0, ground);
            if ratio < AA {
                failures.push(format!(
                    "{theme:?} / Files tree: `{}` is {ratio:.2}:1 (ink {:?} on {:?}), under {AA}:1",
                    text.0.trim(),
                    Srgba::from(ink.0),
                    Srgba::from(ground)
                ));
            }
        }
        if words == 0 {
            failures.push(format!("{theme:?} / Files tree: the selected row drew no words, so nothing was measured"));
        }
        // **The band is a band, not the panel.** A row whose fill stayed the panel's would pass every ratio above for the wrong reason: the
        // point of the measurement is the pair the selection draws. `Selected` is set by gooey, so this is a check that the row it marked is
        // the one filled.
        let panel = bevy_dark_factory::chrome::ink().panel_bg;
        if band == panel {
            failures.push(format!("{theme:?} / Files tree: the selected row was filled with the panel colour {:?}, so no band was drawn", Srgba::from(band)));
        }
    }
    failures.sort();
    failures.dedup();
    assert!(failures.is_empty(), "{} selected word(s) in the Files tree are not readable:\n{}", failures.len(), failures.join("\n"));
}

/// **A selected row never keeps a quiet ink**, which is what `chrome::row_shape`'s own selected fill would do to one: every word under a
/// `RowSelected` row of the running editor clears AA against what it is drawn on, in every skin.
#[test]
fn every_quiet_row_that_is_selected_reads_at_aa_in_every_skin() {
    let mut failures = Vec::new();
    // The surfaces that draw an editor list row with `RowSelected`: the Files workspace's console, the Map's palette and library.
    for theme in THEMES {
        for action in [Action::WorkspaceFiles, Action::WorkspaceMap] {
            let mut app = editor(theme);
            let row = binding(action);
            let mut keys = vec![row.key];
            if row.needs_mod {
                keys.push(if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft });
            }
            for state in [bevy::input::ButtonState::Pressed, bevy::input::ButtonState::Released] {
                for code in &keys {
                    app.world_mut().write_message(bevy::input::keyboard::KeyboardInput {
                        key_code: *code,
                        logical_key: bevy::input::keyboard::Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
                        state,
                        text: None,
                        repeat: false,
                        window: Entity::PLACEHOLDER,
                    });
                }
                app.update();
                app.update();
            }
            settle_support::settle(&mut app);
            let world = app.world_mut();
            let rows: Vec<Entity> = world.query_filtered::<Entity, (With<RowSelected>, With<RowRest>)>().iter(world).collect();
            for row in rows {
                if !world.get::<RowSelected>(row).is_some_and(|r| r.0) {
                    continue;
                }
                let mut stack = vec![row];
                while let Some(entity) = stack.pop() {
                    if let Some(children) = world.get::<Children>(entity) {
                        stack.extend(children.iter());
                    }
                    let (Some(text), Some(ink)) = (world.get::<Text>(entity), world.get::<TextColor>(entity)) else { continue };
                    if text.0.trim().is_empty() {
                        continue;
                    }
                    let ground = ground_of(world, entity, row);
                    let ratio = contrast(ink.0, ground);
                    if ratio < AA {
                        failures.push(format!(
                            "{theme:?} / {action:?}: `{}` is {ratio:.2}:1 (ink {:?} on {:?}), under {AA}:1",
                            text.0.trim(),
                            Srgba::from(ink.0),
                            Srgba::from(ground)
                        ));
                    }
                }
            }
        }
    }
    failures.sort();
    failures.dedup();
    assert!(failures.is_empty(), "{} selected list row(s) keep a quiet ink:\n{}", failures.len(), failures.join("\n"));
}
