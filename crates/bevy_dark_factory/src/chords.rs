//! **The chords that are not rows of the key census, and a person may rebind them all the same.**
//!
//! Some chords are read by their own systems, outside [`crate::keys`]' census: the application's quit, the
//! palette's three, the console and the frame profile, the two reports, the interface's size, the session's tab
//! keys, the shell's left panel and Zen mode, and the terminal's own, which the pane reads before the shell hears
//! anything. They work on every screen and in no tab, which is why they are not census rows. Each was a constant
//! or a function in its own module; [`CHORDS`] is the one table of them now, so the keymap screen can list them as
//! rows a person rebinds and a settings file can name them in `keymap:` by id, as it names a command.
//!
//! [`chord`] is what a reader asks: the person's chord for one, or its default. It reads a process-wide table
//! published by `settings::sync_settings` whenever the settings change, the pattern of [`crate::chrome::ink`] and
//! [`crate::view::tunables`], because several of these are read where no system parameter reaches (a menu label, a
//! tooltip, a pure function building the palette's rows).

use std::collections::BTreeMap;
use std::sync::{LazyLock, PoisonError, RwLock};

use bevy::prelude::KeyCode;
use bevy_gooey::{Modifiers, Shortcut};

/// **Which fixed chord.** A type rather than a string, so a reader cannot ask for one that is not in [`CHORDS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fixed {
    Quit,
    SettingsOpen,
    GotoOpen,
    PaletteOpen,
    PaletteOpenAlt,
    ConsoleToggle,
    ProfileToggle,
    ProblemsShow,
    IssueCompose,
    ReportDevelopers,
    ReportProject,
    ScaleUp,
    ScaleDown,
    ScaleReset,
    SessionCloseTab,
    SessionReopenTab,
    SessionNextTab,
    SessionPreviousTab,
    ShellLeftPanel,
    ShellZen,
    TerminalFind,
    TerminalClear,
    TerminalCloseTab,
    TerminalCollapse,
    TerminalMaximise,
    /// **Raise or lower the pane** with `primary + Backquote` (#640). It lived only as a `Shortcut::new` in
    /// `terminal_pane.rs`, so it could not be rebound, the collision test could not see it, and the guard in
    /// `terminal_view.rs` had to restate the chord to know what to swallow.
    TerminalRaise,
    EditorDevAgent,
}

/// **One fixed chord**: its id in a settings file's `keymap:`, what it does in words, and its chord when no file
/// rebinds it. The default is a function because `Modifiers` has no `const` `|` at this pin.
pub struct NamedChord {
    pub fixed: Fixed,
    pub id: &'static str,
    pub title: &'static str,
    pub default: fn() -> Shortcut,
}

fn primary(key: KeyCode) -> Shortcut {
    Shortcut::new(Modifiers::PRIMARY, key)
}

fn primary_shift(key: KeyCode) -> Shortcut {
    Shortcut::new(Modifiers::PRIMARY | Modifiers::SHIFT, key)
}

/// **Every fixed chord**, in the order the keymap screen lists them.
pub const CHORDS: &[NamedChord] = &[
    NamedChord { fixed: Fixed::Quit, id: "app.quit", title: "Quit Dark Factory", default: || primary(KeyCode::KeyQ) },
    NamedChord { fixed: Fixed::SettingsOpen, id: "settings.open", title: "Open the settings", default: || primary(KeyCode::Comma) },
    // **`Cmd+P` to go somewhere, `Cmd+Shift+P` (and `Cmd+K`) to run something** (NV-05, #382), what the chords mean in every
    // editor a person already uses.
    NamedChord { fixed: Fixed::GotoOpen, id: "goto.open", title: "Go to anything", default: || primary(KeyCode::KeyP) },
    NamedChord { fixed: Fixed::PaletteOpen, id: "palette.open", title: "Open the command palette", default: || primary_shift(KeyCode::KeyP) },
    NamedChord {
        fixed: Fixed::PaletteOpenAlt,
        id: "palette.open_alt",
        title: "Open the command palette (second chord)",
        default: || primary(KeyCode::KeyK),
    },
    // **The console beside the journal's `Cmd+E`** (#509).
    NamedChord { fixed: Fixed::ConsoleToggle, id: "console.open", title: "Show or hide the log", default: || primary_shift(KeyCode::KeyE) },
    // **The frame profile** (#504, moved by #560): it shared `Cmd+Shift+D` with `report.project`, and both
    // handlers run in `Phase::Act` on one press, so a press opened both panels. `Cmd+Shift+D` is the project
    // report — what `agents.md` tells a person, and what `Cmd+D`'s family means — so the profile takes `F`,
    // free because it is only the unshifted find chord.
    NamedChord {
        fixed: Fixed::ProfileToggle,
        id: "profile.open",
        title: "Show or hide the frame profile",
        default: || primary_shift(KeyCode::KeyF),
    },
    NamedChord { fixed: Fixed::ProblemsShow, id: "problems.show", title: "Show the map's problems", default: || primary_shift(KeyCode::KeyX) },
    NamedChord { fixed: Fixed::IssueCompose, id: "issue.compose", title: "Write an issue", default: || primary_shift(KeyCode::KeyI) },
    // **`Cmd+D`: report to the people who make Dark Factory**, from every screen, no project needed.
    NamedChord {
        fixed: Fixed::ReportDevelopers,
        id: "report.developers",
        title: "Report to the developers",
        default: || primary(KeyCode::KeyD),
    },
    NamedChord {
        fixed: Fixed::ReportProject,
        id: "report.project",
        title: "Report about this project",
        default: || primary_shift(KeyCode::KeyD),
    },
    // **The interface's size** (`feedback-1794783912982`): `Cmd+=` (the unshifted key a browser binds), `Cmd+-`, `Cmd+Shift+0`.
    NamedChord {
        fixed: Fixed::ScaleUp,
        id: "interface_scale.up",
        title: "Make the whole interface larger",
        default: || primary(KeyCode::Equal),
    },
    NamedChord {
        fixed: Fixed::ScaleDown,
        id: "interface_scale.down",
        title: "Make the whole interface smaller",
        default: || primary(KeyCode::Minus),
    },
    NamedChord {
        fixed: Fixed::ScaleReset,
        id: "interface_scale.reset",
        title: "Put the interface size back to 100%",
        default: || primary_shift(KeyCode::Digit0),
    },
    NamedChord { fixed: Fixed::SessionCloseTab, id: "session.close_tab", title: "Close the tab", default: || primary(KeyCode::KeyW) },
    NamedChord {
        fixed: Fixed::SessionReopenTab,
        id: "session.reopen_tab",
        title: "Reopen the last closed tab",
        default: || primary_shift(KeyCode::KeyT),
    },
    NamedChord {
        fixed: Fixed::SessionNextTab,
        id: "session.next_tab",
        title: "Next tab",
        default: || Shortcut::new(Modifiers::CTRL, KeyCode::Tab),
    },
    NamedChord {
        fixed: Fixed::SessionPreviousTab,
        id: "session.previous_tab",
        title: "Previous tab",
        default: || Shortcut::new(Modifiers::CTRL | Modifiers::SHIFT, KeyCode::Tab),
    },
    // **VS Code's Toggle Primary Side Bar.** `Run` used to hold it (`keys.rs`); it is `Shift+Cmd+B` now.
    NamedChord { fixed: Fixed::ShellLeftPanel, id: "shell.left_panel", title: "Show or hide the left panel", default: || primary(KeyCode::KeyB) },
    NamedChord { fixed: Fixed::ShellZen, id: "shell.zen", title: "Begin or end Zen mode", default: || primary_shift(KeyCode::KeyM) },
    // **The terminal's own, read before the shell hears anything** (#326, #426): a shell never sees them from this pane.
    NamedChord { fixed: Fixed::TerminalFind, id: "terminal.find", title: "Find in the terminal", default: || primary(KeyCode::KeyF) },
    // **Ctrl+K on every platform** (#637). This was `Modifiers::META`, which is *the primary modifier on
    // macOS*, so on a Mac the chord collided with `palette.open_alt` (`primary(KeyK)`) and
    // `no_two_fixed_chords_are_the_same_key` failed there while passing on Linux — a real collision the
    // Linux gate could not see. `CONTROL` is what "Ctrl+K" means everywhere, and off a mac it is still
    // the shell's kill-line, which is the intent the old comment stated.
    NamedChord {
        fixed: Fixed::TerminalClear,
        id: "terminal.clear",
        title: "Clear the terminal",
        default: || Shortcut::new(Modifiers::CTRL, KeyCode::KeyK),
    },
    NamedChord {
        fixed: Fixed::TerminalCloseTab,
        id: "terminal.close_tab",
        title: "Close the terminal's tab",
        default: || primary_shift(KeyCode::Backquote),
    },
    // **The owner's chord (2026-11-06) and VS Code's Toggle Panel** (#315).
    NamedChord {
        fixed: Fixed::TerminalCollapse,
        id: "terminal.collapse",
        title: "Collapse or raise the terminal",
        default: || primary(KeyCode::KeyJ),
    },
    NamedChord {
        fixed: Fixed::TerminalMaximise,
        id: "terminal.maximise",
        title: "Maximise the terminal",
        default: || primary_shift(KeyCode::KeyJ),
    },
    // **The chord a person uses most and could not find** (#640): `primary + Backquote` raises or lowers the pane.
    // It was built inline at `terminal_pane.rs:1159` and restated at `terminal_view.rs:813`, so it was invisible to
    // the keymap screen, to `keys.md` and to `no_two_fixed_chords_are_the_same_key`.
    NamedChord {
        fixed: Fixed::TerminalRaise,
        id: "terminal.raise",
        title: "Raise or lower the terminal",
        default: || primary(KeyCode::Backquote),
    },
    // **The editor development agent** (`.omp/commands/editor-dev.md`): `D` for developers, beside `Cmd+D`'s report to them, with Alt
    // so it is neither the report nor the project report. A fixed chord rather than a census verb: what it opens is a terminal tab,
    // which no operation of the editor's `Ctx` can, and a new census verb owes one.
    NamedChord {
        fixed: Fixed::EditorDevAgent,
        id: "agent.editor_dev",
        title: "Open the editor development agent",
        default: || Shortcut::new(Modifiers::PRIMARY | Modifiers::ALT, KeyCode::KeyD),
    },
];

/// The fixed chord with settings id `id`.
pub fn by_id(id: &str) -> Option<&'static NamedChord> {
    CHORDS.iter().find(|named| named.id == id)
}

/// How many fixed chords there are: one per [`Fixed`] variant.
const FIXED_COUNT: usize = Fixed::EditorDevAgent as usize + 1;

/// **[`CHORDS`] holds one row per [`Fixed`], in the enum's order**, checked when the crate compiles, so a row is found
/// by indexing and a lookup cannot miss.
const _: () = {
    assert!(CHORDS.len() == FIXED_COUNT, "CHORDS has one row per Fixed variant");
    let mut at = 0;
    while at < CHORDS.len() {
        assert!(CHORDS[at].fixed as usize == at, "CHORDS is in Fixed's order");
        at += 1;
    }
};

fn named(fixed: Fixed) -> &'static NamedChord {
    &CHORDS[fixed as usize]
}

/// The chord `fixed` answers to when no file rebinds it.
pub fn default_of(fixed: Fixed) -> Shortcut {
    (named(fixed).default)()
}

/// The settings id of `fixed`.
pub fn id_of(fixed: Fixed) -> &'static str {
    named(fixed).id
}

/// **The chord `fixed` answers to now**: the person's or the project's rebinding, else its default.
pub fn chord(fixed: Fixed) -> Shortcut {
    let rebound = LIVE_CHORDS.read().unwrap_or_else(PoisonError::into_inner).get(id_of(fixed)).copied();
    rebound.unwrap_or_else(|| default_of(fixed))
}

static LIVE_CHORDS: LazyLock<RwLock<BTreeMap<String, Shortcut>>> = LazyLock::new(|| RwLock::new(BTreeMap::new()));

/// **Keep the fixed chords the resolved keymap's**: only its entries that name one. Only `settings::sync_settings` calls it.
pub(crate) fn publish(keymap: &BTreeMap<String, Shortcut>) {
    let next: BTreeMap<String, Shortcut> = keymap.iter().filter(|(id, _)| by_id(id).is_some()).map(|(id, chord)| (id.clone(), *chord)).collect();
    let mut held = LIVE_CHORDS.write().unwrap_or_else(PoisonError::into_inner);
    if *held != next {
        *held = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **One row per variant, and one variant per row**, so [`chord`] can never be asked for a chord the table lacks.
    #[test]
    fn every_fixed_chord_has_one_row() {
        let mut ids: Vec<&str> = CHORDS.iter().map(|named| named.id).collect();
        let fixed: Vec<Fixed> = CHORDS.iter().map(|named| named.fixed).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CHORDS.len(), "an id is listed twice");
        let mut unique = fixed.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), CHORDS.len(), "a variant is listed twice");
        for id in &ids {
            assert!(crate::commands::by_id(id).is_none(), "`{id}` is a command's id as well");
        }
    }
}
