//! **A game's UI skin, previewed live** (#147).
//!
//! The data is `bevy_gooey`'s own [`GooeySkin`], written as RON in
//! `<project>/family/bevy_gooey/GooeySkin.ron` and applied to the running resource by reflection
//! ([`crate::family_host::apply_pending_settings`]), like every other settings type. The crate's type is
//! `#[serde(default)]`, so a partial file such as `(accent: Srgba((red: 0.2, green: 0.6, blue: 0.9,
//! alpha: 1.0)))` is the default skin with only that field changed, and the crate's own `apply_skin`
//! restyles every widget carrying a `Role` that frame. A file that is not a skin is refused and the last
//! good skin stays; a skin that breaks the crate's own rules ([`GooeySkin::problems`]: WCAG 2.2 contrast,
//! target size, unusable sizes) is applied and each broken rule is a problem. Removing the file puts the
//! crate's default skin back.

use bevy::prelude::*;
use bevy_gooey::GooeySkin;

use crate::editor::EditorState;

/// The crate the skin file belongs to, as `family.ron` names it.
pub const CRATE: &str = "bevy_gooey";

/// Says each rule the running skin breaks, after a skin file was applied.
pub fn report_skin_problems(world: &mut World) {
    let Some(problems) = world.get_resource::<GooeySkin>().map(GooeySkin::problems) else { return };
    let Some(mut state) = world.get_resource_mut::<EditorState>() else { return };
    for problem in problems {
        state.status.problem(format!("bevy_gooey skin: {problem}"));
    }
}
