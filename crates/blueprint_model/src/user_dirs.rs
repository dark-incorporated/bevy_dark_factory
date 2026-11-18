//! **The per-user folders the editor keeps its own files in, defined once** (#306).
//!
//! The editor (`recent.yaml`, `trusted.yaml`, `settings.yaml`, `dark_factory.log`) and its MCP server (which reads
//! the log and writes captures when no project is named) must agree on these paths, so they are named here
//! and nowhere else. None of them is inside a project: a project is a folder a person owns and shares.
//!
//! `<data_dir>` is `dirs::data_dir()`: `~/.local/share` (or `$XDG_DATA_HOME`) on Linux,
//! `~/Library/Application Support` on macOS, `%APPDATA%` on Windows. `<config_dir>` is `dirs::config_dir()`.

use std::path::PathBuf;

/// The directory under the platform's data and config directories that holds the editor's own files.
pub const APP_DIR: &str = "Blueprint";

/// The editor's log, inside [`data_dir`].
pub const LOG_FILE: &str = "dark_factory.log";

/// **The person's settings file**, inside [`config_dir`]. The editor reads and writes it (UI-70, #248) and so does
/// anything engine-free that must obey one of its keys (the decision model, DM-03, #454): the name is here so both name
/// the same file.
pub const SETTINGS_FILE: &str = "settings.yaml";

/// The folder under [`data_dir`] a capture goes to when no project is named.
const CAPTURES_DIR: &str = "captures";

/// The folder under [`data_dir`] the reports about the editor itself go to (`crate::feedback`).
const FEEDBACK_DIR: &str = "feedback";

/// The folder under [`data_dir`] the turn ledger is kept in (`crate::turn_ledger`, DL-01, #460).
const LABELS_DIR: &str = "labels";

/// `<data_dir>/Blueprint`, or `None` on a platform with no data directory.
pub fn data_dir() -> Option<PathBuf> {
    dirs::data_dir().map(|d| d.join(APP_DIR))
}

/// `<config_dir>/Blueprint`, or `None` on a platform with no config directory.
pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join(APP_DIR))
}

/// `<config_dir>/Blueprint/settings.yaml`: the person's settings, for every project.
pub fn settings_file() -> Option<PathBuf> {
    config_dir().map(|d| d.join(SETTINGS_FILE))
}

/// `<data_dir>/Blueprint/dark_factory.log`: the file the editor logs to and the MCP server reads.
pub fn log_file() -> Option<PathBuf> {
    data_dir().map(|d| d.join(LOG_FILE))
}

/// `<data_dir>/Blueprint/captures`: where a capture goes when the MCP server follows the editor with no project.
pub fn captures_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join(CAPTURES_DIR))
}

/// `<data_dir>/Blueprint/feedback`: the reports the owner files about the editor itself.
///
/// **Not inside a project**, because a report about the editor is not about the project that
/// happened to be open when it was noticed — and because the home screen, where a project is a card
/// rather than an open thing, must be able to file one.
pub fn feedback_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join(FEEDBACK_DIR))
}

/// `<data_dir>/Blueprint/labels`: the turn ledger, `turns.jsonl`, and the pictures it names.
///
/// **Never inside a project or its `assets/`** (owner decision on #459): what the ledger holds is about the person's
/// use of the editor, not about a game, and a project is a folder that is shared.
pub fn labels_dir() -> Option<PathBuf> {
    data_dir().map(|d| d.join(LABELS_DIR))
}
