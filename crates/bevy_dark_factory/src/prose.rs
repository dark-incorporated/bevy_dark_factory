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

/// The prose face, in the asset store for as long as the app runs.
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

/// Loads the prose face **when the plugin is built**, not in a startup system.
///
/// A startup system would be too late to be the only path: the launcher builds its pages in `OnEnter`, and a face pinned by a hook or read from a
/// resource the schedule has not inserted yet is a face the text never gets. Built here, [`ProseFont`] exists before any system can spawn a text.
pub struct ProsePlugin;

impl Plugin for ProsePlugin {
    fn build(&self, app: &mut App) {
        let handle = app.world_mut().resource_mut::<Assets<Font>>().add(Font::from_bytes(PROSE_FONT_BYTES.to_vec()));
        app.insert_resource(ProseFont(handle));
    }
}
