//! **`Cmd+=` and `Cmd+-` step the whole interface's size, and `Cmd+Shift+0` resets it**
//! (`feedback-1794783912982`).
//!
//! > It seems like Cmd + "+" or "-" should change the layout size of the entire editor--like
//! > electron.
//!
//! Settings → Appearance has two scale levers — **Text size** (`text_scale`, type alone) and
//! **Interface size** (`interface_scale`, buttons, spacing and type together, because `UiScale`
//! multiplies every `Val::Px` and every font size). The note asks for the second; a browser's zoom
//! chords are the second; and this module is the second's keyboard front end.
//!
//! # One value, two front ends
//!
//! The chords write the same key in the same file the Settings row writes, through
//! [`crate::setting_catalogue::write_key`], so the running editor, the file and the row cannot
//! disagree: `sync_settings` re-resolves `EditorSettings` from the file, `surface` reads
//! `interface_scale` into `UiScale`, and the row reads the same file. There are no steps here to
//! keep in step — the ladder is [`crate::setting_catalogue::INTERFACE_STEPS`], which the row's own
//! arithmetic uses too.
//!
//! # The chords are outside the census, and that is deliberate
//!
//! `keys::BINDINGS` is a static table with a hand-kept action list beside it and a collision test
//! over the whole thing; `report::developers_chord` is the precedent for a chord that belongs to no
//! tab and no `Action` (it must work on the Projects home, where no door is open). A zoom chord is
//! the same shape: it acts on the editor's frame, not on a panel, and it must work on every screen.
//! So these are `bevy_gooey::Shortcut`s read directly, [`crate::keymap_screen`] lists them as
//! reserved so no command can be rebound onto one, and nothing in the census can shadow them.
//!
//! # `Cmd+0` is the Design workspace, so the reset is `Cmd+Shift+0`
//!
//! `Cmd+0` is the tenth workspace (`SG-05`, #404, and `docs/guide/keys.md` says so). Taking it for a
//! zoom reset would either shadow a documented workspace chord or need the workspace to move; the
//! issue's own note says the collision is to be **settled, not silently shadowed**. It is settled by
//! giving the reset its own chord: **`Cmd+Shift+0`**, next to the browser's `Cmd+0` and distinct from
//! the workspace's — and `keys.rs` says the Design row requires Shift to be **up**, so the two
//! chords never dispatch together. `tests/a_scale_chord_steps_the_whole_interface.rs` presses both.

use bevy::input::ButtonInput;
use bevy::prelude::*;
use bevy_gooey::{Modifiers, Shortcut};

use crate::setting_catalogue;

/// **`Cmd+=`** (`Cmd`+`+` is the same key held with Shift on most layouts, so the unshifted `Equal`
/// is what a browser binds and what this reads).
pub fn step_up_chord() -> Shortcut {
    Shortcut::new(Modifiers::PRIMARY, KeyCode::Equal)
}

/// **`Cmd+-`**.
pub fn step_down_chord() -> Shortcut {
    Shortcut::new(Modifiers::PRIMARY, KeyCode::Minus)
}

/// **`Cmd+Shift+0`**: the interface size back to 100%.
pub fn reset_chord() -> Shortcut {
    Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, KeyCode::Digit0)
}

/// **The three chords, as [`crate::keymap_screen`] lists them among the reserved ones**: a chord live
/// on every screen that no command may be rebound onto, because it answers first.
pub fn reserved_chords() -> [(&'static str, &'static str, Shortcut); 3] {
    [
        ("interface_scale.up", "Make the whole interface larger", step_up_chord()),
        ("interface_scale.down", "Make the whole interface smaller", step_down_chord()),
        ("interface_scale.reset", "Put the interface size back to 100%", reset_chord()),
    ]
}

pub struct ScaleChordsPlugin;

impl Plugin for ScaleChordsPlugin {
    fn build(&self, app: &mut App) {
        // **The keys go on every screen.** A zoom chord is about the frame, and a person on the
        // Projects home asking for a bigger interface is asking for the same thing as one in the
        // editor.
        //
        // **After `sync_settings`**, so two presses in two frames step from the value the first one
        // wrote rather than twice from the file as it was when the editor opened.
        app.add_systems(
            Update,
            the_scale_chords
                .in_set(crate::keys::Phase::Act)
                .after(crate::settings::sync_settings),
        );
    }
}

/// **Step or reset the interface size, writing it where the Settings row writes it.**
///
/// The ladder is [`setting_catalogue::INTERFACE_STEPS`]: a list, not a `× 1.1`, so four steps up is
/// 200% and not 146.41%, and a step up then down is where it started.
fn the_scale_chords(
    keys: Res<ButtonInput<KeyCode>>,
    editing: Query<(), With<bevy_gooey::TextEditing>>,
    settings: Option<Res<crate::settings::EditorSettings>>,
    user_file: Option<Res<crate::settings::UserSettingsFile>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    // **A field that owns the keyboard keeps it.** `Cmd+=` in a code editor is a keystroke someone
    // typed; the census refuses every chord while a box is typing, and these are no different.
    if !editing.is_empty() {
        return;
    }
    let now = settings.as_deref().map_or(1.0, |s| s.interface_scale);
    let (value, what) = if reset_chord().just_pressed(&keys) {
        (1.0, "the interface size is back at")
    } else if step_up_chord().just_pressed(&keys) {
        (setting_catalogue::interface_step_up(now), "the interface size is")
    } else if step_down_chord().just_pressed(&keys) {
        (setting_catalogue::interface_step_down(now), "the interface size is")
    } else {
        return;
    };
    // **No file, no change, and it is said.** The chord and the row write one key of one file; an
    // editor built without that file (a headless test, or a machine with no config directory) has
    // nowhere to keep the size, and writing it to the project's file instead would let a project
    // decide how large one person's screen is.
    let Some(file) = user_file else {
        if let Some(state) = state.as_deref_mut() {
            state.status.problem(format!(
                "no personal settings file is available on this machine, so the interface size was \
                 not saved. It stays at {:.0}%.",
                now * 100.0
            ));
        }
        return;
    };
    // **A refused write is said, not swallowed.** The size did not change, and a chord that quietly
    // did nothing is the defect (`report.rs`'s rule for every write the editor makes from a key).
    match setting_catalogue::write_key(
        &file.0,
        "interface_scale",
        Some(setting_catalogue::Value::Number(value)),
    ) {
        Ok(()) => {
            if let Some(state) = state.as_deref_mut() {
                state.status.note(format!("{what} {:.0}%", value * 100.0));
            }
        }
        Err(why) => {
            if let Some(state) = state.as_deref_mut() {
                state.status.problem(format!("could not save the interface size: {why}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The three chords are three different chords, and the reset's does not satisfy the workspace's.**
    #[test]
    fn the_reset_is_not_the_design_workspace_chord() {
        assert_ne!(reset_chord(), step_up_chord());
        assert_ne!(reset_chord(), step_down_chord());
        assert_ne!(step_up_chord(), step_down_chord());
        // `Cmd+0` is the Design workspace (`keys::Action::WorkspaceDesign`); the reset adds Shift.
        assert!(reset_chord().modifiers.shift, "the reset holds Shift, which is what keeps it off Cmd+0");
        assert!(!step_up_chord().modifiers.shift && !step_down_chord().modifiers.shift);
    }

    /// **A step walks the ladder in the direction asked and clamps at both ends**, and a size a person
    /// dragged to between two rungs lands on the next one rather than past it.
    #[test]
    fn a_step_walks_the_ladder_and_clamps_at_both_ends() {
        use setting_catalogue::{interface_step_down, interface_step_up, INTERFACE_STEPS};
        assert_eq!(interface_step_up(1.0), 1.10);
        assert_eq!(interface_step_up(1.10), 1.25);
        assert_eq!(interface_step_up(2.0), 2.0, "the top is the top");
        assert_eq!(interface_step_down(1.25), 1.10);
        assert_eq!(interface_step_down(1.0), 1.0, "and the bottom is the bottom");
        assert_eq!(interface_step_up(1.35), 1.50);
        assert_eq!(interface_step_down(1.35), 1.25);
        // **A step up then a step down is where it started**, for every rung but the last — the top
        // clamps, so a step up there moves nowhere and the step back down is a real descent. That
        // asymmetry is the ladder's, not a drift a multiply-by-1.1 would avoid.
        for rung in INTERFACE_STEPS.into_iter().take(INTERFACE_STEPS.len() - 1) {
            assert_eq!(interface_step_down(interface_step_up(rung)), rung, "up then down from {rung}");
        }
        assert_eq!(interface_step_up(2.0), 2.0, "and the top clamps rather than growing");
    }
}
