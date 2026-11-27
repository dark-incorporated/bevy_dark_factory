//! **The prose face** (#469, decided as option 2 on 2026-11-15: *recommend, record, proceed*).
//!
//! The editor draws in one monospace face, FiraMono, and that is right for keys, ids, paths, the Inspector's columns and the terminal: it is a
//! tool for people who read those. It is wrong for a sentence. A line of 60 to 90 characters set one column per character reads as terminal
//! output, which is what a person said of the What's new page (`feedback-1794024825941`: *"it reads like a terminal"*).
//!
//! So a text that is **a sentence meant to be read** draws in **Fira Sans** (the same family and the same SIL OFL 1.1 licence as FiraMono; the
//! licence text sits beside the face in `assets/fonts/`), and everything else keeps FiraMono. The surfaces that are prose today are in
//! `docs/ui/vision.md`, "Prose and code".
//!
//! **How a text asks for it: spawn it with [`Prose`] and [`face`].** gooey writes a widget's face from `GooeyFonts` on every restyle, and a
//! [`TextFace`](bevy_gooey::TextFace) is the one way to pin one a host owns: the ink and the size stay the role's (a button's state still moves
//! them), and the face is the host's (`bevy_gooey#118`). **One writer of a `TextFont` per text**, so nothing here runs after gooey's style pass,
//! and the old `set_the_prose_face` — which re-applied the face every frame — is gone.
//!
//! **The face is an asset of its own**, not the one installed over Bevy's default id (`harness::install_font_in`), so a project's own
//! `FiraMono-Regular.ttf` replaces the editor's monospace face and leaves the prose face alone.

use bevy::prelude::*;
use bevy::text::{Font, FontSource};

/// Fira Sans Regular, SIL Open Font License 1.1 (`assets/fonts/FiraSans-OFL.txt`). 457 KB: the one extra face in the binary.
pub const PROSE_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/FiraSans-Regular.ttf");

/// **The prose face's id**, fixed, so the face can be named before its bytes are in the store — the way the terminal's face
/// (`terminal_view::TERMINAL_FONT`) is kept.
pub const PROSE_FONT: Handle<Font> = bevy::asset::uuid_handle!("37b4ecc9-4d24-48d6-bbe8-811820467d61");

/// The prose face, [`PROSE_FONT`], in the asset store for as long as the app runs.
#[derive(Resource, Clone, Debug)]
pub struct ProseFont(pub Handle<Font>);

/// **This text is prose**: a sentence a person reads, drawn in [`ProseFont`] whatever its role says. Headings, keycaps, ids, paths, tables, the
/// Inspector and the terminal never carry it.
///
/// The marker is a marker: the face goes on beside it with [`face`], at the spawn site.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct Prose;

/// **The prose face as gooey's `TextFace`**, for a text spawned with [`Prose`]:
/// `(label(..), Prose, prose::face(&prose))`.
pub fn face(prose: &ProseFont) -> bevy_gooey::TextFace {
    bevy_gooey::TextFace(FontSource::Handle(prose.0.clone()))
}

/// **Names the prose face when the plugin is built, and puts its bytes in the store at `Startup`.**
///
/// The handle is a fixed id, so [`ProseFont`] exists before any system runs, and a text the launcher spawns in its `OnEnter` carries the face
/// whether or not the bytes are in yet: Bevy lays a text out once its face has loaded. Nothing here needs `TextPlugin` to be built first, so the
/// plugin may be added in any order and to any app — `headless::the_editor_plugin_registers_the_tool_resources_its_systems_take` adds the
/// editor's plugins to `MinimalPlugins` alone, where loading the face at build time panicked.
pub struct ProsePlugin;

impl Plugin for ProsePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ProseFont(PROSE_FONT)).add_systems(Startup, install_the_prose_face);
    }
}

/// Put the prose face's bytes in the font assets under [`PROSE_FONT`]. An app with no `Assets<Font>` has no `TextPlugin` and draws no
/// text, so there is no face to install.
fn install_the_prose_face(fonts: Option<ResMut<Assets<Font>>>) {
    let Some(mut fonts) = fonts else { return };
    if let Err(why) = fonts.insert(PROSE_FONT.id(), Font::from_bytes(PROSE_FONT_BYTES.to_vec())) {
        error!("the prose face could not be installed, so prose draws in no face: {why}");
    }
}
