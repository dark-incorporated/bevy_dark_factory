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
//! **How a text asks for it: spawn it with [`Prose`].** gooey writes a widget's face from its `GooeyFonts` (one `body` face for every label and
//! button text, so setting that would turn the keycaps and the Inspector proportional too), and a host-set face on a widget with a `Role` is written
//! over the next time it is restyled. So [`set_the_prose_face`] runs after gooey's style pass and puts the prose face on every [`Prose`] text, and
//! the text keeps everything else its role gives it (ink that follows a button's state, the person's text size).
//!
//! **The face is an asset of its own**, not the one installed over Bevy's default id (`harness::install_font_in`), so a project's own
//! `FiraMono-Regular.ttf` replaces the editor's monospace face and leaves the prose face alone.

use bevy::prelude::*;
use bevy::text::{Font, FontSource};

/// Fira Sans Regular, SIL Open Font License 1.1 (`assets/fonts/FiraSans-OFL.txt`). 457 KB: the one extra face in the binary.
pub const PROSE_FONT_BYTES: &[u8] = include_bytes!("../assets/fonts/FiraSans-Regular.ttf");

/// The prose face, once loaded.
#[derive(Resource, Clone, Debug)]
pub struct ProseFont(pub Handle<Font>);

/// **This text is prose**: a sentence a person reads, drawn in [`ProseFont`] whatever its role says. Headings, keycaps, ids, paths, tables, the
/// Inspector and the terminal never carry it.
#[derive(Component, Default, Clone, Copy, Debug)]
pub struct Prose;

/// Loads the prose face and keeps it on every [`Prose`] text.
pub struct ProsePlugin;

impl Plugin for ProsePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreStartup, load_the_prose_face);
        // After gooey's style pass, which writes the role's face (`GooeyFonts::body`) over every restyled widget.
        app.add_systems(Update, set_the_prose_face.after(bevy_gooey::GooeySystems::Style));
    }
}

/// The face goes into the asset store once.
fn load_the_prose_face(mut commands: Commands, mut fonts: ResMut<Assets<Font>>) {
    commands.insert_resource(ProseFont(fonts.add(Font::from_bytes(PROSE_FONT_BYTES.to_vec()))));
}

/// Every [`Prose`] text draws in the prose face. Compares before it writes, so a settled editor changes nothing.
pub fn set_the_prose_face(prose: Option<Res<ProseFont>>, mut texts: Query<&mut TextFont, With<Prose>>) {
    let Some(prose) = prose else { return };
    let want = FontSource::Handle(prose.0.clone());
    for mut font in &mut texts {
        if font.font != want {
            font.font = want.clone();
        }
    }
}
