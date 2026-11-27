//! **The editor's settings, in three layers that change the running editor** (UI-70, #248).
//!
//! 1. The built-in defaults ([`EditorSettings::default`]).
//! 2. The user's file, `<config_dir>/Blueprint/settings.yaml` ([`user_file`]).
//! 3. The project's file, `<project>/assets/blueprint/editor.yaml` (kind `editor_settings` in
//!    `reload::KINDS`).
//!
//! **Precedence is per key, and the later layer wins.** A file is a [`SettingsLayer`]: every key is an
//! `Option`, so a file says only what it sets, and a key a layer leaves out falls through to the layer
//! below. `version: 1` and `text_scale: 1.5` in the user's file and `version: 1` and `reduced_motion:
//! true` in the project's give both; the same key in both gives the project's.
//!
//! **Live.** [`sync_settings`] follows both files while the editor runs and re-resolves [`EditorSettings`] when one moved. It
//! reads a file when it first sees its path, once more when the file's index has its first listing, and then only when that index
//! says the file changed ([`FileNews`]): the person's file through [`crate::project_index::OutsideFiles`] (it lives outside the
//! project), the project's through the project's [`crate::project_index::ProjectIndex`]. **Nothing is polled** (#501 item 18).
//! Deleting a key, or the file, falls back to the layer below at once. A file that
//! does not read (not YAML, an unknown key, a key of the wrong type, a value out of range, another
//! version) **keeps that layer's last good content**, is refused in `blueprint_state.reloads` with the
//! file and the field, and is a problem on the status line, once per distinct fault.
//!
//! The project layer is a kind of the reload contract, so the guide's table lists it. The user's file
//! belongs to no project, so it is not a kind; its verdicts name its full path.
//!
//! The settings screen (UI-71) writes these layers, and the keymap (UI-72) and the theme (UI-73) add
//! their keys to [`SettingsLayer`]; a new key is an `Option` there with its default in
//! [`EditorSettings`], and an old file still reads.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use bevy::ecs::system::SystemParam;

use bevy::prelude::*;
use blueprint_model::live::{KindVerdict, Outcome};
use blueprint_model::reload_check::{self, Refusal};
use serde::{Deserialize, Serialize};
use bevy_gooey::Shortcut;
use bevy_oracle::DecisionModel;

use crate::editor::EditorState;
use crate::live::EditorReloads;
use crate::project::Project;
use crate::project_index::{self, OutsideFileChanged, OutsideFiles, ProjectFileChanged, ProjectIndex};

pub const SETTINGS_VERSION: u32 = 1;

/// The id of the reload-contract kind both layers' verdicts carry.
pub const KIND: &str = "editor_settings";

/// The person's file, in the per-user config folder (named once, in the core, so every reader of one of its keys names the same file).
const USER_FILE: &str = blueprint_model::user_dirs::SETTINGS_FILE;
/// The project's file, under the project directory (`assets/blueprint`).
pub const PROJECT_FILE: &str = blueprint_model::reload_check::PROJECT_SETTINGS_FILE;

/// The smallest and largest a scale may be: below the first nothing is legible, above the second no
/// panel fits.
pub const SCALE_RANGE: (f32, f32) = (0.5, 3.0);

/// Where the user's settings live, or `None` on a platform with no config directory.
pub fn user_file() -> Option<PathBuf> {
    blueprint_model::user_dirs::config_dir().map(|d| d.join(USER_FILE))
}

/// The look of the editor's chrome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    #[default]
    ToolDark,
    Parchment,
    HighContrast,
}

/// **The colours the code editor gives each role of code** (#525, `feedback-1795094391027`: *"...as well as
/// syntax highlighting (we should have a setting to pick a syntax highlighting theme)"*; #610).
///
/// **A palette laid on the skin, not a skin**: the owner's call on #610 (2026-11-27) was *a separate "Code colours"
/// setting listing named syntax palettes (Skin default, plus e.g. Monokai, Solarized, GitHub)*. gooey derives a colour
/// for each [`SyntaxRole`](bevy_gooey::SyntaxRole) from the skin's seeds and a fixed hue table, and
/// [`GooeySkin::syntax_theme`](bevy_gooey::GooeySkin) lets a host choose those hues apart from the rest of the skin.
/// This is the person's choice of them: gooey's five [`SyntaxPreset`](bevy_gooey::SyntaxPreset)s, and three palettes
/// other editors made famous ([`crate::code_palette`]). Whichever is chosen, the skin keeps its ground and text and
/// gooey still holds every colour to the contrast bar.
///
/// **A setting the skin builder applies**, not a style-sheet value: `Theme` is the shape this repo already
/// has for "pick one of a few looks", and the style sheet's `skin:` section is a layer a person edits by hand
/// — a few choices with names the screen shows are a setting, and [`SettingsLayer`] is where the screen writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyntaxTheme {
    /// The skin's own hues: exactly the colours the editor drew before this setting existed.
    #[default]
    Skin,
    Warm,
    Cool,
    /// One hue, told apart by lightness: quiet reading, and for readers who cannot tell hues apart.
    Mono,
    /// The widest hue spacing the roles allow: read at a glance.
    Separated,
    /// Monokai's hues ([`crate::code_palette::MONOKAI`]).
    Monokai,
    /// Solarized's accents ([`crate::code_palette::SOLARIZED`]).
    Solarized,
    /// github.com's light code colours ([`crate::code_palette::GITHUB`]); the file says `github`.
    #[serde(rename = "github")]
    GitHub,
}

impl SyntaxTheme {
    /// Every choice, in the order the Settings row offers them.
    pub const ALL: [SyntaxTheme; 8] = [
        SyntaxTheme::Skin,
        SyntaxTheme::Warm,
        SyntaxTheme::Cool,
        SyntaxTheme::Mono,
        SyntaxTheme::Separated,
        SyntaxTheme::Monokai,
        SyntaxTheme::Solarized,
        SyntaxTheme::GitHub,
    ];

    /// What the row says this choice does, so similar-sounding names cannot be mistaken for each other.
    pub fn meaning(self) -> &'static str {
        match self {
            SyntaxTheme::Skin => "the skin's own colours, the default, as before this setting",
            SyntaxTheme::Warm => "amber, orange and sand, like a warm terminal",
            SyntaxTheme::Cool => "blues, teals and violets, like a night sky",
            SyntaxTheme::Mono => "one hue told apart by lightness, for quiet reading and colour-blind readers",
            SyntaxTheme::Separated => "the widest hue spacing there is, for reading a pane at a glance",
            SyntaxTheme::Monokai => "Monokai's pink keywords, yellow strings, purple numbers and green names",
            SyntaxTheme::Solarized => "Solarized's green keywords, cyan strings, magenta numbers and blue names",
            SyntaxTheme::GitHub => "GitHub's red keywords, navy strings, blue numbers and purple names",
        }
    }

    /// **The theme this choice puts on the skin** — the one place the setting meets gooey's presets and the
    /// editor's palettes, so a variant added here is a compile error until it says which it is.
    pub fn theme(self) -> bevy_gooey::SyntaxTheme {
        match self {
            SyntaxTheme::Skin => bevy_gooey::SyntaxPreset::Skin.theme(),
            SyntaxTheme::Warm => bevy_gooey::SyntaxPreset::Warm.theme(),
            SyntaxTheme::Cool => bevy_gooey::SyntaxPreset::Cool.theme(),
            SyntaxTheme::Mono => bevy_gooey::SyntaxPreset::Mono.theme(),
            SyntaxTheme::Separated => bevy_gooey::SyntaxPreset::Separated.theme(),
            SyntaxTheme::Monokai => crate::code_palette::MONOKAI.theme(),
            SyntaxTheme::Solarized => crate::code_palette::SOLARIZED.theme(),
            SyntaxTheme::GitHub => crate::code_palette::GITHUB.theme(),
        }
    }
}

/// **The terminal's caret when a program asks for none** (#324): VS Code's
/// `terminal.integrated.cursorStyle`. A program that asks for a shape (vim's DECSCUSR) still gets it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalCursor {
    #[default]
    Block,
    Underline,
    Bar,
}

impl TerminalCursor {
    /// The shape the terminal draws for this setting.
    pub fn shape(self) -> crate::terminal::CaretShape {
        match self {
            TerminalCursor::Block => crate::terminal::CaretShape::Block,
            TerminalCursor::Underline => crate::terminal::CaretShape::Underline,
            TerminalCursor::Bar => crate::terminal::CaretShape::Bar,
        }
    }
}

/// **The agent the terminal's `Invoke Agent` button starts**, connected to the editor's MCP server and proved with
/// a power-on self test (`terminal_agent`). `Custom` runs `terminal_agent_command` from the file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TerminalAgent {
    #[default]
    Off,
    OhMyPi,
    Custom,
}

impl TerminalAgent {
    /// The word chips and messages call this agent by.
    pub fn name(self) -> &'static str {
        match self {
            TerminalAgent::OhMyPi => "omp",
            TerminalAgent::Off | TerminalAgent::Custom => "agent",
        }
    }
}

/// **The program a machine must have for the terminal to start with the agent chosen** (#341).
///
/// It is what an `Invoke Agent` press types, so a machine that has it can start an agent and a machine
/// that does not would only fail its self test.
pub const AGENT_PROGRAM: &str = "omp";

/// **How long the login shell is given to answer, in seconds** (#341).
///
/// A login shell reads the person's rc files, so it is slower than a plain one and one of them can
/// hang. Past this the machine is read as having no agent: a window that never opens is worse than a
/// terminal that offers a shell.
pub const LOGIN_ASK_PATIENCE_S: u64 = 5;

/// **Is the agent installed on this machine?** — the executable [`AGENT_PROGRAM`] in any folder of the
/// `PATH` it is given.
///
/// Off the environment as a parameter rather than read inside, so the answer for any folder list can be
/// tested without touching the process's own. A file that is not executable is not a program: a machine
/// with one has no agent, and its terminal keeps its shell.
pub fn agent_in_path(path: Option<&str>) -> TerminalAgent {
    let Some(path) = path else { return TerminalAgent::Off };
    // An empty entry is "here" to a shell, so a test run from a folder holding an `omp` would answer
    // for a machine whose `PATH` does not name one; it is left out rather than read as a folder.
    let found = std::env::split_paths(path)
        .filter(|folder| !folder.as_os_str().is_empty())
        .any(|folder| is_runnable(&folder.join(AGENT_PROGRAM)));
    if found { TerminalAgent::OhMyPi } else { TerminalAgent::Off }
}

/// Is `program` a file this machine can run? A directory named `omp` is not, and neither is a file
/// whose mode has no execute bit on it.
fn is_runnable(program: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(program) else { return false };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// How often a shell that has answered is looked at for its exit, in milliseconds.
const LOGIN_ASK_EXIT_POLL_MS: u64 = 5;

/// **What this machine has, asked once** and carried for the whole session (#341).
///
/// A `PATH` that gains the agent while the editor runs does not change the editor under the person
/// using it: the answer is taken at start-up, which is also what lets Settings show the effective value
/// as a value rather than as a thing that moves. Inserted by `main`; absent in a test, whose editor
/// resolves from the built-in default alone.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MachineAgent(pub TerminalAgent);

impl MachineAgent {
    /// **Asks the machine the way the press runs the agent** (#341).
    ///
    /// It is `SHELL -lc 'command -v omp'`, not this process's `PATH`. A macOS app opened from Finder or
    /// the Dock inherits launchd's environment — `/usr/bin:/bin:/usr/sbin:/sbin` — so the owner's
    /// `~/.local/bin/omp` is not on it and the machine would answer `Off` while `Invoke Agent` works,
    /// because that press types the command into a *login* shell (`terminal_agent.rs` says why). The
    /// question is therefore put to the same login shell, so the pane's offer and the button's work
    /// come from one answer.
    ///
    /// **The shell is the pane's own** — `crate::terminal`'s `shell_program_of_environment`, the one a
    /// tab runs when no `terminal_shell` is chosen (`terminal_shell` is a setting read on the frames
    /// after this, so it is not the shell an answer could be put to here). One function decides which
    /// shell is this person's, so the question and the tab cannot disagree about `$SHELL` or about a
    /// machine that has neither `$SHELL` nor `/bin/sh`.
    ///
    /// Bounded: a login shell whose rc files hang answers nothing, and [`LOGIN_ASK_PATIENCE_S`] later
    /// the machine is read as having no agent rather than the window waiting.
    pub fn ask() -> MachineAgent {
        match crate::terminal::shell_program_of_environment() {
            Ok(shell) => MachineAgent(MachineAgent::ask_the_login_shell(&shell, LOGIN_ASK_PATIENCE_S)),
            // No shell at all: there is nothing to ask, and nothing the press could run either.
            Err(_) => MachineAgent(TerminalAgent::Off),
        }
    }

    /// [`MachineAgent::ask`] with the shell and its patience named, so a test can give both.
    ///
    /// `None` from `command -v` — the shell exits non-zero — is a machine without the agent; so is a
    /// shell that cannot be started, or one whose output does not arrive inside `patience_s`.
    ///
    /// **One deadline covers the whole question, the answer included.** The shell's exit alone is not
    /// the answer: the text is read on its own thread and sent down a channel, so a login shell that
    /// backgrounded a process which holds the pipe (`( sleep 30 & ) ; command -v omp ; exit 0`) cannot
    /// keep the editor waiting for an EOF that arrives 30 s later. Past `patience_s`, the child is
    /// killed and reaped and the machine reads `Off`.
    pub fn ask_the_login_shell(shell: &str, patience_s: u64) -> TerminalAgent {
        let script = format!("command -v {AGENT_PROGRAM}");
        let started = std::process::Command::new(shell)
            .arg("-lc")
            .arg(&script)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn();
        let Ok(mut child) = started else { return TerminalAgent::Off };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(patience_s);
        // **The text arrives on a channel, not from `join`.** A reader thread that is still blocked on
        // the pipe at the deadline is left holding it: it writes into a channel nobody reads and its
        // thread ends with the process, so the editor never waits on it.
        let (saying, heard) = std::sync::mpsc::channel();
        if let Some(mut out) = child.stdout.take() {
            std::thread::spawn(move || {
                use std::io::Read as _;
                let mut text = String::new();
                let _ = out.read_to_string(&mut text);
                let _ = saying.send(text);
            });
        }
        let text = {
            let left = deadline.saturating_duration_since(std::time::Instant::now());
            heard.recv_timeout(left).ok()
        };
        // **A shell that never closed its output says nothing worth reading**, and outlived its
        // patience: killed and reaped, so it does not stay a zombie for the hours this editor runs.
        let Some(text) = text else {
            let _ = child.kill();
            let _ = child.wait();
            return TerminalAgent::Off;
        };
        // **The exit, then the answer.** Output arriving is not the exit arriving: the shell may still
        // be in the microseconds before `wait` can see it, so the status is looked for inside the same
        // patience rather than assumed. A shell whose status never comes is one that did not finish.
        let failed_or_unfinished = loop {
            match child.try_wait() {
                Ok(Some(status)) => break !status.success(),
                Ok(None) if std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(LOGIN_ASK_EXIT_POLL_MS));
                }
                _ => break true,
            }
        };
        if failed_or_unfinished || text.trim().is_empty() { TerminalAgent::Off } else { TerminalAgent::OhMyPi }
    }

    /// **The agent a machine starts its terminal with**, from what the binary asked it.
    ///
    /// [`None`] is a process that never asked — every test's editor — and reads `Off`, the built-in
    /// default. One place, because the terminal's resolution and the Settings screen must not answer
    /// this question two ways.
    pub fn resolved(machine: Option<&MachineAgent>) -> TerminalAgent {
        machine.map_or(TerminalAgent::Off, |machine| machine.0)
    }
}

/// **What to do when a file the editor holds is edited underneath it while the person has unsaved edits** (#502).
///
/// **There is no correct default, which is why it is a setting.** Both sides are deliberate: someone meant to save that
/// file, and someone meant to type in it. What separates them is that the version on disk survives being ignored and the
/// version in memory does not, so the shipped default protects the one that cannot be recovered — the person's text.
///
/// **The map's own question and the Files workspace's are this one value**, so one setting cannot mean two things: a map
/// re-read (`watch::adopt_map`) and a buffer's save ([`crate::files::save_file`]) both read it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
// **snake_case on disk**, so a settings file reads `external_edits: keep_editor_changes`: a key of this file is typed by a
// person, and the row's own names are words rather than the enum's Rust spelling.
#[serde(rename_all = "snake_case")]
pub enum ExternalEdits {
    /// **Ask, every time**: nothing changes until the person says which version they want. The default.
    #[default]
    Prompt,
    /// **Take the file's**: the editor's own text is dropped and the file is read. The one answer that can lose work, so
    /// it is never a reflex — see [`crate::confirm::Asked::TakeTheFile`].
    ReloadFromDisk,
    /// **Keep the editor's**: the person's text stays, and a save then overwrites the file. The conflict is reported once.
    KeepEditorChanges,
}

impl ExternalEdits {
    /// The names the Settings row offers, in declaration order.
    pub const NAMES: &'static [&'static str] = &["Ask me", "Take the file", "Keep mine"];

    /// What the row says this answer does, in one clause, so the three cannot be mistaken for each other.
    pub fn meaning(self) -> &'static str {
        match self {
            ExternalEdits::Prompt => "ask you, and change nothing until you answer",
            ExternalEdits::ReloadFromDisk => "take the file's version and drop your edits",
            ExternalEdits::KeepEditorChanges => "keep your text, and a save overwrites the file",
        }
    }
}

/// **What the running editor uses**: the defaults with the user's file and then the project's laid over
/// them, key by key.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct EditorSettings {
    pub theme: Theme,
    /// **The code editor's syntax-highlighting hues** (#525): gooey derives every `Role::CodeSyntax` colour from
    /// the skin's seeds and this theme's hues, so a choice recolours every open code editor without a rebuild.
    pub syntax_theme: SyntaxTheme,
    /// Text size as a multiple of the skin's: 1.0 is as designed, 2.0 is 200%.
    pub text_scale: f32,
    /// Interface size (spacing and targets) as a multiple of the skin's.
    pub interface_scale: f32,
    /// Reduced motion (gooey's `MotionSettings::reduced`): what travels lands at once, fades and colours still ease.
    pub reduced_motion: bool,
    /// **Whether the view follows the agent's changes** (AF-04, #360): View ▸ Follow the agent.
    ///
    /// On by default, because the follow is the point of the AGENT series: an agent works faster than a
    /// person reads, and a view that stayed put would mean the person never saw the work at all. Off, the
    /// camera never moves for an agent's change and the look chip still offers *Look*, so a person who
    /// turned it off has not lost the ability to see — they have taken away only the moving.
    ///
    /// A settings key rather than a session switch for `reduced_motion`'s reason: it is how a person
    /// wants to watch an agent, not a fact about one project. **Its tests and its changelog line are
    /// AF-04's** (`tests/the_follow_waits_for_a_person_using_the_view.rs`, `af-04-pace`), which is
    /// where the behaviour it switches was built.
    pub follow_agent: bool,
    /// The map's grid is drawn in the viewport.
    pub show_grid: bool,
    /// How strongly the map's grid is drawn: 1.0 is the skin's grid colour, less is fainter.
    pub grid_strength: f32,
    /// The triangle count is shown at the top right (#302). Off until asked for: it is a number for
    /// someone tuning a scene, and the first thing a newcomer reads at the top of the window.
    pub show_statistics: bool,
    /// **View ▸ Show activity bar** (NV-03, #380): the vertical strip of workspace icons at the
    /// window's far left. On by default, because it is the one place the workspaces are all in view;
    /// off takes it away entirely and the choice is saved.
    pub show_activity_bar: bool,
    /// **Whether the editor keeps a record of each agent turn, to learn from** (DL-01, #460, `turn_ledger`): the pictures before and after,
    /// what was asked, the operations and the facts, in the person's own data folder and nowhere else. **On by default** (owner, #459): the
    /// labels come from using the editor, so there is no separate labelling session. Off is off: no picture is taken for it and nothing is
    /// written.
    pub collect_labels: bool,
    /// The rebound commands of every layer, by command id: what the keymap screen shows against each
    /// command's default chord.
    pub keymap: BTreeMap<String, Shortcut>,
    /// The terminal's text size as a multiple of the shipped size (#324): VS Code's
    /// `terminal.integrated.fontSize`, as a scale so it reads like the editor's own text size.
    pub terminal_text_scale: f32,
    /// The terminal's caret when a program asks for none.
    pub terminal_cursor: TerminalCursor,
    /// The terminal's caret blinks. On by default, as it has been since the terminal shipped.
    pub terminal_cursor_blink: bool,
    /// How many lines of output each terminal keeps above its screen (#324): VS Code's
    /// `terminal.integrated.scrollback`.
    pub terminal_scrollback: usize,
    /// The agent the terminal's `Invoke Agent` button starts, connected to the editor's tools.
    ///
    /// **The built-in default is `Off`**, a pure value with no machine behind it. What a person meets is
    /// the resolved settings, and that asks this machine once at start-up (`MachineAgent`, #341): a
    /// machine whose login shell finds `omp` resolves to `OhMyPi`, so its terminal opens on the empty pane that
    /// offers the agent; a machine without one resolves to `Off` and opens a shell. A `terminal_agent`
    /// key in either settings file wins over both.
    ///
    /// Either way nothing starts by itself — a session costs tokens to warm up — until the button is
    /// pressed.
    pub terminal_agent: TerminalAgent,
    /// The shell line `TerminalAgent::Custom` types. Deliberately not in the settings catalogue: the
    /// Settings screen has no text control (#324), so this key is set in the file only.
    pub terminal_agent_command: Option<String>,
    /// **The model the agent tab names on its launch line** (#477, `feedback-1794783326602`): an omp model id
    /// (`anthropic/claude-sonnet-5-5`, or a fuzzy word like `sonnet`), passed as `--model`.
    ///
    /// **`None` passes no `--model`**, so omp picks — which is a different model on every machine, and the thing
    /// the owner reported. A settings file names one once and every editor on the LAN starts the same agent. Only
    /// an omp tab is handed it: a custom command keeps its own flags.
    ///
    /// A free-text key, not in the settings catalogue: the Settings screen has no text control, so it is set in
    /// the file, as `terminal_agent_command` is. [`crate::terminal_agent::DEFAULT_MODEL`] is the id written beside
    /// the row's description, so a person knows what to type.
    pub terminal_agent_model: Option<String>,
    /// **Did the person leave the terminal collapsed or raised?** (#424) `None` until they raise or lower it: the pane then starts
    /// collapsed, on every screen. The editor writes `true` or `false` to the user's file when the pane is raised or lowered, so a
    /// relaunch opens it as it was left, the way the window's arrangement is kept. Not in the settings catalogue: it is a thing a
    /// person does with a chord, not a row they set.
    pub terminal_collapsed: Option<bool>,
    /// **The program a new terminal tab runs** (#324): VS Code's `terminal.integrated.defaultProfile`.
    /// `None` is `$SHELL`, then `/bin/sh`. Tabs already open keep the shell they started with. Like
    /// `terminal_agent_command`, not in the settings catalogue: the Settings screen has no text control.
    pub terminal_shell: Option<String>,
    /// The arguments `terminal_shell` is started with (VS Code's profile `args`); empty when none.
    pub terminal_shell_args: Vec<String>,
    /// **Each workspace's arrangement, as a person left it** (UI-33, #227; per workspace since
    /// NV-02, #379): the workspace's id and the `bevy_gooey` `DockLayout` it shows, written by the
    /// editor when a divider or a tab is moved and read back on the next start.
    ///
    /// **A map, not one layout.** A door was one arrangement because a door was one place; with nine
    /// workspaces a single layout would move Map's inspector when somebody dragged Kit's, which is
    /// the thing per-workspace layouts exist to stop. A workspace with no entry is
    /// [`Workspace::default_layout`](crate::workspace::Workspace::default_layout) — its own default,
    /// so a person who has never moved anything has no key in their file.
    pub workspace_layout: BTreeMap<String, bevy_gooey::DockLayout>,
    /// **Which decision model is asked** (DM-03, #454): Local (d3-flash on the LAN) or Remote (Perplexity's, through
    /// OpenRouter, which sends pictures off the LAN). The one setting the editor, its labeller and checks, and the MCP server
    /// all read.
    ///
    /// **Only the person's own file decides it.** A project file that sets it is a problem and is ignored
    /// ([`bevy_oracle::project_may_not_choose`]): a project cannot send its agents' pictures off the LAN.
    pub decision_model: DecisionModel,
    /// **Whether a capture is filed as a GitHub issue** (#495). Off by default; the Settings screen's
    /// row turns it on, and `capture_issue_repo` says where the capture's files go.
    pub capture_issue: bool,
    /// **What to do when a file the editor holds changes on disk under unsaved edits** (#502): the one value the map's
    /// watcher and the Files workspace's save both read, so they cannot disagree.
    pub external_edits: ExternalEdits,
    /// **How often the map and the open buffers are saved, in seconds** (#508). `0.0` is off, which is the shipped default:
    /// the draft ([`crate::draft`]) is the part worth having unconditionally, and a timer that writes a project is a
    /// behaviour a person should ask for. The screen offers [`crate::setting_catalogue::AUTO_SAVE_CHOICES`].
    pub auto_save_interval_s: f64,
    /// **The checkout that receives captures** (#495), or empty for "captures stay local".
    pub capture_issue_repo: String,
    /// **The editor's own source checkout** (`crate::style::editor_source`): where the base style sheet is read and watched
    /// and where the editor-development agent works. `None` is the checkout this binary was built from, when it is on disk.
    /// A free-text key, set in the file: an absolute path to a `bevy_dark_factory` checkout.
    pub editor_source: Option<PathBuf>,
    /// **The Map camera's tunables** (`view:`): how near and how far it goes, one wheel notch, the keys' pan and one
    /// turn of a piece. Read through [`crate::view::tunables`].
    pub view: ViewSettings,
}

/// **The Map camera's tunables**, every one a person may set in `view:` of a settings file.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewSettings {
    /// The nearest the rig comes, metres of height: low enough that a 12 cm mug can be looked at.
    pub zoom_min_m: f32,
    /// The furthest out it goes on an ordinary map, metres; a map too big to see whole from there raises it.
    pub zoom_max_m: f32,
    /// One wheel notch's factor on the height: above 1, so a notch in is the inverse of a notch out.
    pub zoom_step: f32,
    /// The keys' pan, in heights of the camera per second, so the ground crosses the screen at one pace near or far.
    pub pan_speed: f32,
    /// One turn of a piece, degrees.
    pub yaw_step_deg: f32,
}

impl Default for ViewSettings {
    fn default() -> Self {
        ViewSettings { zoom_min_m: 0.25, zoom_max_m: 80.0, zoom_step: 1.2, pan_speed: 16.0 / 18.0, yaw_step_deg: 15.0 }
    }
}

/// **The largest turn of a piece a person may set**, degrees: a quarter turn. Past it a turn skips the right angle a
/// piece is squared to.
pub const YAW_STEP_MAX_DEG: f32 = 90.0;

/// **One layer's `view:` keys**: only the ones it sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ViewLayer {
    #[serde(rename = "zoom_min", skip_serializing_if = "Option::is_none")]
    pub zoom_min_m: Option<f32>,
    #[serde(rename = "zoom_max", skip_serializing_if = "Option::is_none")]
    pub zoom_max_m: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom_step: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pan_speed: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yaw_step_deg: Option<f32>,
}

impl ViewLayer {
    /// Sets nothing, so a file written without `view:` is written back without it.
    pub fn is_empty(&self) -> bool {
        *self == ViewLayer::default()
    }

    /// `under` with this layer's keys laid over it.
    pub fn over(&self, under: ViewSettings) -> ViewSettings {
        ViewSettings {
            zoom_min_m: self.zoom_min_m.unwrap_or(under.zoom_min_m),
            zoom_max_m: self.zoom_max_m.unwrap_or(under.zoom_max_m),
            zoom_step: self.zoom_step.unwrap_or(under.zoom_step),
            pan_speed: self.pan_speed.unwrap_or(under.pan_speed),
            yaw_step_deg: self.yaw_step_deg.unwrap_or(under.yaw_step_deg),
        }
    }

    /// **What a camera with these keys could not do**, as the field and the reason: judged laid over the defaults, so a
    /// file that sets `zoom_max` alone is held to the default `zoom_min`.
    fn problem(&self) -> Option<(&'static str, String)> {
        let view = self.over(ViewSettings::default());
        if view.zoom_min_m.is_nan() || view.zoom_min_m <= 0.0 {
            return Some(("view.zoom_min", format!("{} m: the nearest the camera comes must be above 0 m", view.zoom_min_m)));
        }
        if view.zoom_max_m.is_nan() || view.zoom_max_m <= view.zoom_min_m {
            return Some(("view.zoom_max", format!("{} m is not further than zoom_min's {} m", view.zoom_max_m, view.zoom_min_m)));
        }
        if view.zoom_step.is_nan() || view.zoom_step <= 1.0 {
            return Some(("view.zoom_step", format!("{}: a notch must change the height, so the factor is above 1", view.zoom_step)));
        }
        if view.pan_speed.is_nan() || view.pan_speed <= 0.0 {
            return Some(("view.pan_speed", format!("{}: the keys must move the camera, so the speed is above 0", view.pan_speed)));
        }
        if view.yaw_step_deg.is_nan() || view.yaw_step_deg <= 0.0 || view.yaw_step_deg > YAW_STEP_MAX_DEG {
            return Some(("view.yaw_step_deg", format!("{} degrees is outside above 0 to {YAW_STEP_MAX_DEG}", view.yaw_step_deg)));
        }
        None
    }
}

impl Default for EditorSettings {
    fn default() -> Self {
        EditorSettings {
            theme: Theme::default(),
            syntax_theme: SyntaxTheme::default(),
            text_scale: 1.0,
            interface_scale: 1.0,
            reduced_motion: false,
            follow_agent: true,
            show_grid: true,
            grid_strength: 1.0,
            show_statistics: false,
            show_activity_bar: true,
            collect_labels: true,
            keymap: BTreeMap::new(),
            terminal_text_scale: 1.0,
            terminal_cursor: TerminalCursor::default(),
            terminal_cursor_blink: true,
            terminal_scrollback: crate::terminal::SCROLLBACK,
            terminal_agent: TerminalAgent::default(),
            terminal_agent_command: None,
            terminal_agent_model: None,
            terminal_collapsed: None,
            terminal_shell: None,
            terminal_shell_args: Vec::new(),
            workspace_layout: BTreeMap::new(),
            decision_model: DecisionModel::default(),
            capture_issue: false,
            capture_issue_repo: String::new(),
            external_edits: ExternalEdits::default(),
            auto_save_interval_s: 0.0,
            editor_source: None,
            view: ViewSettings::default(),
        }
    }
}

/// **The fewest and the most lines of history a terminal may keep** (#324): under the first, a build's
/// error is gone before it can be scrolled to; over the second, a long day's output is a slow leak in
/// the editor's memory (each line is a row of cells).
pub const SCROLLBACK_RANGE: (usize, usize) = (100, 100_000);

/// The faintest and fullest the grid may be drawn: below the first it cannot be seen.
pub const GRID_STRENGTH_RANGE: (f32, f32) = (0.1, 1.0);

/// **One file's worth of settings**: only the keys it sets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SettingsLayer {
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    /// **The code editor's syntax-highlighting hues** (#525). Absent leaves the layer below to decide, so a file
    /// written before this key existed reads as `Skin` — the colours the editor drew before the setting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub syntax_theme: Option<SyntaxTheme>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interface_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduced_motion: Option<bool>,
    /// **View ▸ Follow the agent** (AF-04, #360). `None` means the layer leaves it alone, so an
    /// `editor.yaml` written before this key existed reads as on — the default, not a stored `false`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub follow_agent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_grid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_strength: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_statistics: Option<bool>,
    /// **View ▸ Show activity bar** (NV-03, #380). `None` leaves the layer below to decide, so a file
    /// written before this key existed reads as on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_activity_bar: Option<bool>,
    /// **Turn labels from use** (DL-01, #460). `None` leaves the layer below to decide, so a file written before this key existed reads as on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collect_labels: Option<bool>,
    /// **Whether a capture is filed as a GitHub issue** (#495). Off by default, so a person opts in
    /// once; `capture_issue_repo` names the checkout that receives the capture's files.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_issue: Option<bool>,
    /// **The checkout that receives captures** (#495), when a person has opted in. Empty means
    /// captures stay local, as they do today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture_issue_repo: Option<String>,
    /// **Rebound commands**: a command's id (`file.save`, `commands::Command::id`) and the chord that
    /// now runs it. A command the file leaves out keeps its default chord (UI-72, #250).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub keymap: BTreeMap<String, Shortcut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_text_scale: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_cursor: Option<TerminalCursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_cursor_blink: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_scrollback: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_agent: Option<TerminalAgent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_agent_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_agent_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_collapsed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_shell: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_shell_args: Option<Vec<String>>,
    /// Each workspace's arrangement (UI-33, #227; per workspace since NV-02, #379). Not in the
    /// settings catalogue: it is a thing a person drags, not one they type, and Window ▸ Reset
    /// layout is the way back to the current workspace's default.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub workspace_layout: BTreeMap<String, bevy_gooey::DockLayout>,
    /// **The one arrangement files had before the workspaces** (UI-33, #227), read so no existing
    /// settings file is refused for it: [`SettingsLayer::read`] moves it into
    /// `workspace_layout["map"]` (the arrangement was the Map's) unless that entry exists. Never
    /// written back.
    #[serde(default, skip_serializing)]
    pub shell_layout: Option<bevy_gooey::DockLayout>,
    /// **Which decision model is asked** (DM-03, #454). Only the person's file counts: see [`EditorSettings::decision_model`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_model: Option<DecisionModel>,
    /// **What to do when a file changes under unsaved edits** (#502). Absent leaves the layer below to decide, so a file
    /// written before this key existed reads as [`ExternalEdits::Prompt`] — the shipped default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_edits: Option<ExternalEdits>,
    /// **How often the editor saves the map and the open buffers, in seconds** (#508). Absent is off.
    ///
    /// **`auto_save_interval` on disk**, the catalogue's own key: a person writes this file by hand and greps for the word
    /// the Settings screen shows, so the field's `_s` suffix is a Rust reading of the unit and not a second name for the key.
    #[serde(default, rename = "auto_save_interval", skip_serializing_if = "Option::is_none")]
    pub auto_save_interval_s: Option<f64>,
    /// **The editor's own source checkout** (`crate::style::editor_source`). Absent leaves the detected checkout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_source: Option<PathBuf>,
    /// **The Map camera's tunables** (`view:`), only the keys this file sets.
    #[serde(default, skip_serializing_if = "ViewLayer::is_empty")]
    pub view: ViewLayer,
}

impl Default for SettingsLayer {
    fn default() -> Self {
        SettingsLayer {
            version: SETTINGS_VERSION,
            theme: None,
            syntax_theme: None,
            text_scale: None,
            interface_scale: None,
            reduced_motion: None,
            follow_agent: None,
            show_grid: None,
            grid_strength: None,
            show_statistics: None,
            show_activity_bar: None,
            collect_labels: None,
            keymap: BTreeMap::new(),
            terminal_text_scale: None,
            terminal_cursor: None,
            terminal_cursor_blink: None,
            terminal_scrollback: None,
            terminal_agent: None,
            terminal_agent_command: None,
            terminal_agent_model: None,
            terminal_collapsed: None,
            terminal_shell: None,
            terminal_shell_args: None,
            workspace_layout: BTreeMap::new(),
            shell_layout: None,
            decision_model: None,
            capture_issue: None,
            capture_issue_repo: None,
            external_edits: None,
            auto_save_interval_s: None,
            editor_source: None,
            view: ViewLayer::default(),
        }
    }
}

impl SettingsLayer {
    /// **Reads a file's text as a layer**, refusing with the file and the field when it does not read,
    /// is another version, or holds a value no editor can use.
    pub fn read(file: &str, text: &str) -> Result<SettingsLayer, Refusal> {
        let mut layer: SettingsLayer = reload_check::read(KIND, file, text, |t| blueprint_model::yaml::from_str(t, "settings").map_err(|p| p.line()))?;
        if let Some(old) = layer.shell_layout.take() {
            layer.workspace_layout.entry(crate::workspace::Workspace::Map.id().to_owned()).or_insert(old);
        }
        // **A keymap that rebound a command the workspaces replaced still reads** (#379): the entry
        // binds nothing now, so it is dropped here and the next save writes the file without it.
        layer.keymap.retain(|id, _| !crate::commands::is_retired(id));
        let refuse = |field: &str, reason: String| Refusal { kind: KIND, file: file.to_owned(), field: Some(field.to_owned()), reason };
        if layer.version != SETTINGS_VERSION {
            return Err(refuse(
                "version",
                format!("version {} is not the {SETTINGS_VERSION} this build reads", layer.version),
            ));
        }
        for (field, value) in [
            ("text_scale", layer.text_scale),
            ("interface_scale", layer.interface_scale),
            ("terminal_text_scale", layer.terminal_text_scale),
        ] {
            if let Some(v) = value
                && !(SCALE_RANGE.0..=SCALE_RANGE.1).contains(&v)
            {
                return Err(refuse(field, format!("{v} is outside {} to {}", SCALE_RANGE.0, SCALE_RANGE.1)));
            }
        }
        if let Some(v) = layer.terminal_scrollback
            && !(SCROLLBACK_RANGE.0..=SCROLLBACK_RANGE.1).contains(&v)
        {
            return Err(refuse("terminal_scrollback", format!("{v} lines is outside {} to {}", SCROLLBACK_RANGE.0, SCROLLBACK_RANGE.1)));
        }
        if let Some(v) = layer.grid_strength
            && !(GRID_STRENGTH_RANGE.0..=GRID_STRENGTH_RANGE.1).contains(&v)
        {
            return Err(refuse("grid_strength", format!("{v} is outside {} to {}", GRID_STRENGTH_RANGE.0, GRID_STRENGTH_RANGE.1)));
        }
        if let Some(command) = &layer.terminal_agent_command
            && command.trim().is_empty()
        {
            return Err(refuse("terminal_agent_command", "an empty command runs nothing: leave the key out or name a program".to_owned()));
        }
        if let Some(source) = &layer.editor_source {
            if !source.is_absolute() {
                return Err(refuse("editor_source", "must be an absolute path to a bevy_dark_factory checkout".to_owned()));
            }
            if !source.join("crates/bevy_dark_factory/Cargo.toml").is_file() {
                return Err(refuse(
                    "editor_source",
                    format!("must be an absolute path to a bevy_dark_factory checkout: {} has no crates/bevy_dark_factory/Cargo.toml", source.display()),
                ));
            }
        }
        if let Some(shell) = &layer.terminal_shell
            && shell.trim().is_empty()
        {
            return Err(refuse("terminal_shell", "an empty shell runs nothing: leave the key out or name a program".to_owned()));
        }
        if layer.terminal_shell_args.is_some() && layer.terminal_shell.is_none() {
            return Err(refuse("terminal_shell_args", "arguments for a shell that is not named: set `terminal_shell` too, or leave the key out".to_owned()));
        }
        for (id, chord) in &layer.keymap {
            let field = format!("keymap.{id}");
            if crate::commands::by_id(id).is_none() && crate::chords::by_id(id).is_none() {
                return Err(refuse(&field, format!("`{id}` is not a command or a fixed chord the editor has")));
            }
            if !chord.is_valid() {
                return Err(refuse(&field, "this chord has no key (a modifier alone is not a shortcut)".to_owned()));
            }
        }
        if let Some((field, reason)) = layer.view.problem() {
            return Err(refuse(field, reason));
        }
        Ok(layer)
    }
}

impl EditorSettings {
    /// **The defaults with `user` and then `project` laid over them, key by key.**
    pub fn resolve(user: Option<&SettingsLayer>, project: Option<&SettingsLayer>) -> EditorSettings {
        EditorSettings::resolve_with(user, project, EditorSettings::default())
    }

    /// **The same, on this machine** (#341): the built-in defaults with the one key a machine answers
    /// filled in first — [`TerminalAgent`], which is `Off` unless `machine` says the agent is installed.
    ///
    /// **A file still wins, both ways.** `machine` fills the default, it does not overrule a layer, so a
    /// person on a machine with `omp` who wants a plain shell names `Off` in their file and gets it, and
    /// a person whose agent is somewhere their login shell does not reach names it too. Only the key nobody
    /// stated comes from the machine.
    pub fn resolve_on_this_machine(
        user: Option<&SettingsLayer>,
        project: Option<&SettingsLayer>,
        machine: TerminalAgent,
    ) -> EditorSettings {
        EditorSettings::resolve_with(user, project, EditorSettings { terminal_agent: machine, ..EditorSettings::default() })
    }

    /// **The one resolution**, from `base`. Every caller goes through here, so a key added to the
    /// defaults cannot be resolved one way by the editor and another by whatever asked a machine about
    /// it.
    fn resolve_with(user: Option<&SettingsLayer>, project: Option<&SettingsLayer>, mut out: EditorSettings) -> EditorSettings {
        for layer in [user, project].into_iter().flatten() {
            out.theme = layer.theme.unwrap_or(out.theme);
            out.syntax_theme = layer.syntax_theme.unwrap_or(out.syntax_theme);
            out.text_scale = layer.text_scale.unwrap_or(out.text_scale);
            out.interface_scale = layer.interface_scale.unwrap_or(out.interface_scale);
            out.reduced_motion = layer.reduced_motion.unwrap_or(out.reduced_motion);
            out.follow_agent = layer.follow_agent.unwrap_or(out.follow_agent);
            out.show_grid = layer.show_grid.unwrap_or(out.show_grid);
            out.grid_strength = layer.grid_strength.unwrap_or(out.grid_strength);
            out.show_statistics = layer.show_statistics.unwrap_or(out.show_statistics);
            out.show_activity_bar = layer.show_activity_bar.unwrap_or(out.show_activity_bar);
            out.collect_labels = layer.collect_labels.unwrap_or(out.collect_labels);
            out.external_edits = layer.external_edits.unwrap_or(out.external_edits);
            out.auto_save_interval_s = layer.auto_save_interval_s.unwrap_or(out.auto_save_interval_s);
            // Per command: a later layer's binding replaces the earlier one's for that command alone.
            out.keymap.extend(layer.keymap.iter().map(|(id, chord)| (id.clone(), *chord)));
            out.terminal_text_scale = layer.terminal_text_scale.unwrap_or(out.terminal_text_scale);
            out.terminal_cursor = layer.terminal_cursor.unwrap_or(out.terminal_cursor);
            out.terminal_cursor_blink = layer.terminal_cursor_blink.unwrap_or(out.terminal_cursor_blink);
            out.terminal_scrollback = layer.terminal_scrollback.unwrap_or(out.terminal_scrollback);
            out.terminal_agent = layer.terminal_agent.unwrap_or(out.terminal_agent);
            if let Some(command) = &layer.terminal_agent_command {
                out.terminal_agent_command = Some(command.clone());
            }
            if let Some(model) = &layer.terminal_agent_model {
                out.terminal_agent_model = Some(model.clone());
            }
            if let Some(collapsed) = layer.terminal_collapsed {
                out.terminal_collapsed = Some(collapsed);
            }
            // **A shell and its arguments are one value**: a layer that names a shell brings its own
            // arguments (none when it gives none), never the earlier layer's, which were for another program.
            if let Some(shell) = &layer.terminal_shell {
                out.terminal_shell = Some(shell.clone());
                out.terminal_shell_args = layer.terminal_shell_args.clone().unwrap_or_default();
            }
            // **The whole arrangement, not a key of it**: a layout is one value, and a later layer
            // that states one replaces the earlier's (the same rule as `theme`).
            for (workspace, layout) in &layer.workspace_layout {
                out.workspace_layout.insert(workspace.clone(), layout.clone());
            }
            out.view = layer.view.over(out.view);
        }
        // **Only the person's own layer chooses the decision model** (DM-03, #454): a project that could say Remote would
        // send its agents' pictures off the LAN for everyone who opens it. The project's key is a settings problem
        // (`agent_verdict` refuses the file, naming the field) and is ignored here.
        if let Some(user) = user {
            out.decision_model = user.decision_model.unwrap_or(out.decision_model);
        }
        // **Only the person's own file files an issue** (#495): a project that could turn it on would
        // push its captures to GitHub for everyone who opens it. The checkout is the person's too.
        if let Some(user) = user {
            out.capture_issue = user.capture_issue.unwrap_or(out.capture_issue);
            if let Some(repo) = user.capture_issue_repo.as_deref() {
                out.capture_issue_repo = repo.to_owned();
            }
        }
        // **Only the person's own file names the editor's checkout**: the editor-development agent builds and runs code
        // there, so a project that could point it elsewhere would choose what an agent builds for everyone who opens it.
        if let Some(source) = user.and_then(|user| user.editor_source.as_ref()) {
            out.editor_source = Some(source.clone());
        }
        out
    }
}

/// Names the user's file. Inserted by `main`; absent in a test, so no test reads the real config.
#[derive(Resource, Clone, Debug)]
pub struct UserSettingsFile(pub PathBuf);

/// What one look at a file found.
#[derive(Clone, Debug, PartialEq)]
enum Look {
    Applied,
    Refused { field: Option<String>, reason: String },
    /// No file: not an error, the layer is simply empty.
    Gone,
}

/// One layer's file, and what the editor last made of it.
#[derive(Default)]
struct Watch {
    path: Option<PathBuf>,
    /// A hash of the text last parsed, so the same text is not parsed again.
    hash: Option<u64>,
    /// **The last content that read.** Kept across a refusal.
    good: Option<SettingsLayer>,
    last: Option<Look>,
    /// That the project's file is not listed by the project's index has been said, so it is said once.
    unfollowed_said: bool,
}

fn hash_of(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

thread_local! {
    static FILE_READS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// **How many times a settings file has been read on this thread** by the editor's readers (`sync_settings` and the two screens). A test
/// reads it across frames to say that a file no one changed was not read: it counts what goes through `read_settings_file` and
/// nothing else.
pub fn file_reads_on_this_thread() -> u64 {
    FILE_READS.with(std::cell::Cell::get)
}

/// **The one read of a settings file** the editor's readers make, counted per thread.
pub(crate) fn read_settings_file(path: &Path) -> std::io::Result<String> {
    FILE_READS.with(|c| c.set(c.get() + 1));
    std::fs::read_to_string(path)
}

/// Whether a read failed because the file is not there: `NotFound` (the file, or its folder), or `NotADirectory` (a component of the path
/// is a file). The index says both are absent, so a reader treats both as no file.
pub(crate) fn is_absent(e: &std::io::Error) -> bool {
    matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory)
}

impl Watch {
    /// **Reads `path` when it is a new path, or when `moved` says its index heard of it**; the look it decided, or `None` when there is
    /// nothing new to say. No `stat` and no timer: whether to look is the index's to say.
    fn look(&mut self, path: &Path, file: &str, moved: bool) -> Option<Look> {
        let same_file = self.path.as_deref() == Some(path);
        if same_file && !moved && self.last.is_some() {
            return None;
        }
        if !same_file {
            *self = Watch { path: Some(path.to_owned()), ..Watch::default() };
        }
        let look = match read_settings_file(path) {
            Ok(text) => {
                let hash = hash_of(&text);
                match self.last.clone().filter(|_| self.hash == Some(hash)) {
                    Some(last) => last,
                    None => {
                        self.hash = Some(hash);
                        match SettingsLayer::read(file, &text) {
                            Ok(layer) => {
                                self.good = Some(layer);
                                Look::Applied
                            }
                            Err(r) => Look::Refused { field: r.field, reason: r.reason },
                        }
                    }
                }
            }
            Err(e) if is_absent(&e) => {
                self.hash = None;
                self.good = None;
                Look::Gone
            }
            Err(e) => {
                self.hash = None;
                Look::Refused { field: None, reason: e.to_string() }
            }
        };
        if self.last.as_ref() == Some(&look) {
            return None;
        }
        self.last = Some(look.clone());
        Some(look)
    }
}

/// **What the editor last made of the two files.**
#[derive(Resource, Default)]
pub struct SettingsView {
    user: Watch,
    project: Watch,
    heard: Heard,
}

impl SettingsView {
    /// Has the open project's own file been looked at? Until it has, [`EditorSettings`] is the user's layer alone,
    /// and a setting the project makes (the terminal's agent, say) is not yet in it.
    pub fn project_looked(&self) -> bool {
        self.project.path.is_some()
    }
}

/// **What one reader of the settings files has heard of their indexes**, kept by that reader (a `Local`, or a resource of its own).
#[derive(Default)]
pub struct Heard {
    user: Feed,
    project: Feed,
}

/// One file as a reader follows it. The style sheet's layers are followed the same way (`crate::style`).
#[derive(Default)]
pub(crate) struct Feed {
    path: Option<PathBuf>,
    /// The file's index has had its first listing since this path was first seen: the one read that catches a change made between the
    /// reader's first read and that listing (the first listing publishes no event).
    armed: bool,
    failure: Option<String>,
}

impl Feed {
    /// Whether the file must be read now: the index said it changed, or this is the first frame its index has a listing, or the index
    /// started or stopped failing (the read then says why, as it always did). `None` for no file.
    pub(crate) fn moved(&mut self, path: Option<&Path>, listing_in: bool, failure: Option<String>, changed: bool) -> bool {
        let Some(path) = path else {
            *self = Feed::default();
            return false;
        };
        if self.path.as_deref() != Some(path) {
            *self = Feed { path: Some(path.to_owned()), ..Feed::default() };
        }
        let mut moved = changed;
        if listing_in && !self.armed {
            self.armed = true;
            moved = true;
        }
        if failure != self.failure {
            self.failure = failure;
            moved = true;
        }
        moved
    }
}

/// Which of the two settings files moved this frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Moved {
    pub user: bool,
    pub project: bool,
}

impl Moved {
    /// Whether either file moved.
    pub fn any(self) -> bool {
        self.user || self.project
    }
}

/// **The two settings files' news, for any system that reads them** (#501 item 18): the project's file moved when the project's index
/// published a [`ProjectFileChanged`] for its path, the person's when [`OutsideFiles`] published an [`OutsideFileChanged`] for its
/// folder and name. Each system that takes it has its own cursor over the messages and its own [`Heard`].
#[derive(SystemParam)]
pub struct FileNews<'w, 's> {
    project_changed: MessageReader<'w, 's, ProjectFileChanged>,
    outside_changed: MessageReader<'w, 's, OutsideFileChanged>,
    project_index: Res<'w, ProjectIndex>,
    outside: Res<'w, OutsideFiles>,
}

impl FileNews<'_, '_> {
    /// **Which files must be read now**, given where they are (`None` for no file). Cheap enough to run every frame: two message
    /// readers and a few atomic loads, with no filesystem in it.
    pub fn moved(&mut self, heard: &mut Heard, user: Option<&Path>, project: Option<&Path>) -> Moved {
        let user_at = user.and_then(project_index::split_file);
        let user_hit = {
            let mut hit = false;
            for e in self.outside_changed.read() {
                hit |= user_at.as_ref().is_some_and(|(dir, name)| &e.root == dir && &e.rel == name);
            }
            hit
        };
        let (user_listing, user_failure) = match &user_at {
            Some((dir, _)) => (self.outside.generation(dir) != 0, self.outside.failure(dir)),
            None => (false, None),
        };
        // A project file is followed by the index only when the index follows the root it is under.
        let root = self.project_index.root();
        let project = project.filter(|p| root.is_some_and(|r| p.starts_with(r)));
        let project_hit = if self.project_changed.is_empty() {
            false
        } else {
            let rel = project.zip(root).and_then(|(p, r)| p.strip_prefix(r).ok()).and_then(|r| r.to_str()).map(|r| r.replace('\\', "/"));
            let mut hit = false;
            for e in self.project_changed.read() {
                hit |= rel.as_deref() == Some(e.rel.as_str());
            }
            hit
        };
        Moved {
            user: heard.user.moved(user, user_listing, user_failure, user_hit),
            project: heard.project.moved(project, self.project_index.generation() != 0, self.project_index.failure(), project_hit),
        }
    }

    /// **Whether the project's index lists `path`**: `None` until it has a listing (or when `path` is not under its root). A file the
    /// index does not list (a `.gitignore`d one, or one in a skipped folder) is read when it is first seen and its edits are not
    /// followed.
    fn project_file_listed(&self, path: &Path) -> Option<bool> {
        let snapshot = self.project_index.snapshot()?;
        let rel = path.strip_prefix(self.project_index.root()?).ok()?.to_str()?.replace('\\', "/");
        Some(snapshot.files.binary_search_by(|(listed, _)| listed.as_str().cmp(&rel)).is_ok())
    }
}

/// **Follow the folder the person's settings file is in**, for that file's name and no other file in it. A person who has never saved a
/// setting has no such folder, and that is not an error: the file is then a `Created` event when it comes.
pub fn follow_the_user_file(user: Option<Res<UserSettingsFile>>, mut outside: ResMut<OutsideFiles>, mut followed: Local<Option<(PathBuf, String)>>) {
    let Some(user) = user else {
        if let Some((dir, _)) = followed.take() {
            outside.unfollow(&dir);
        }
        return;
    };
    if !user.is_changed() {
        return;
    }
    let want = project_index::split_file(&user.0);
    if *followed == want {
        return;
    }
    if let Some((dir, _)) = followed.take() {
        outside.unfollow(&dir);
    }
    if let Some((dir, name)) = &want {
        outside.follow(dir, &[name.as_str()]);
    }
    *followed = want;
}

/// **Everything [`sync_settings`] resolves from**, grouped so its argument list stays readable and
/// under clippy's limit as a source is added.
#[derive(bevy::ecs::system::SystemParam)]
pub struct SettingsSources<'w, 's> {
    project: Option<Res<'w, Project>>,
    user_file: Option<Res<'w, UserSettingsFile>>,
    /// **What this machine has** (#341), asked once at start-up by the binary. Absent in a test, whose
    /// editor resolves from the built-in default alone and so opens the shell it always did.
    machine: Option<Res<'w, MachineAgent>>,
    /// Which of the two files moved ([`FileNews`]).
    news: FileNews<'w, 's>,
}

impl SettingsSources<'_, '_> {
    /// The agent this machine's terminal starts with when no file states one.
    fn machine_agent(&self) -> TerminalAgent {
        MachineAgent::resolved(self.machine.as_deref())
    }
}

/// **Which settings file a finding is about** (#425): the person's own, or the open project's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AgentFile {
    User,
    Project,
}

/// **What a settings file that chooses the Custom agent without a command says, and how to fix it** (#425). The Settings screen
/// offers the fix in the Terminal agent row; this is the same sentence in the file's own verdict, which the status band counts.
pub const CUSTOM_WITHOUT_COMMAND: &str = "terminal_agent is Custom but terminal_agent_command is not set, so no agent can start. Fix: choose Oh My Pi (or Off) for Terminal agent in Settings, or add terminal_agent_command to this file";

/// **The file that chose `Custom` where no layer gives it a command** (#425), or `None` when the agent is anything else or the command
/// is set somewhere. The project's layer is the later one, so it is the culprit when it chose Custom itself; otherwise the user's.
pub fn custom_agent_without_command(user: Option<&SettingsLayer>, project: Option<&SettingsLayer>) -> Option<AgentFile> {
    let chosen = [user, project].into_iter().flatten().filter_map(|layer| layer.terminal_agent).next_back();
    if chosen != Some(TerminalAgent::Custom) {
        return None;
    }
    if [user, project].into_iter().flatten().any(|layer| layer.terminal_agent_command.is_some()) {
        return None;
    }
    if project.is_some_and(|layer| layer.terminal_agent == Some(TerminalAgent::Custom)) {
        Some(AgentFile::Project)
    } else {
        Some(AgentFile::User)
    }
}

/// One file that read: applied, or refused for the Custom agent it chose without a command, or (the project's file only) for
/// choosing the decision model, which is the person's own choice and not a project's (DM-03, #454). The refusal names the file and the
/// field, and the choice the person made stays.
fn agent_verdict(file: String, source: AgentFile, culprit: Option<AgentFile>, project_chose: Option<DecisionModel>) -> KindVerdict {
    if culprit == Some(source) {
        KindVerdict::refused(KIND, file, Some("terminal_agent".to_owned()), CUSTOM_WITHOUT_COMMAND)
    } else if let (AgentFile::Project, Some(chose)) = (source, project_chose) {
        KindVerdict::refused(KIND, file, Some(bevy_oracle::SETTING_KEY.to_owned()), bevy_oracle::project_may_not_choose(chose))
    } else {
        KindVerdict::applied(KIND, file)
    }
}

/// **Look at both settings files, decide what changed, and keep [`EditorSettings`] the layers' sum.**
///
/// A file that does not read is refused and its layer stays as it last read; a file that is gone has no
/// layer. A verdict is said once per distinct finding, so a frame in between decides nothing.
pub fn sync_settings(
    mut sources: SettingsSources,
    mut view: ResMut<SettingsView>,
    mut settings: ResMut<EditorSettings>,
    mut reloads: Option<ResMut<EditorReloads>>,
    mut state: Option<ResMut<EditorState>>,
    mut last_culprit: Local<Option<AgentFile>>,
) {
    let user_path = sources.user_file.as_deref().map(|f| f.0.clone());
    let project_path = sources.project.as_deref().map(|p| p.project_dir.join(PROJECT_FILE));
    let moved = sources.news.moved(&mut view.heard, user_path.as_deref(), project_path.as_deref());
    let mut looks: Vec<(String, Look, AgentFile)> = Vec::new();
    // Every file this frame can name, looked at or not, so a verdict that depends on the *other* file can be said again.
    let mut named: Vec<(String, AgentFile)> = Vec::new();
    if let Some(file) = &user_path {
        let name = file.to_string_lossy().replace('\\', "/");
        named.push((name.clone(), AgentFile::User));
        if let Some(look) = view.user.look(file, &name, moved.user) {
            looks.push((name, look, AgentFile::User));
        }
    }
    if let (Some(project), Some(path)) = (&sources.project, &project_path) {
        let name = crate::map_files::file_name(project, path);
        named.push((name.clone(), AgentFile::Project));
        if let Some(look) = view.project.look(path, &name, moved.project) {
            looks.push((name, look, AgentFile::Project));
        }
        // **A project file the index does not list cannot be followed**: it is read now, and an edit to it is not seen. Said once.
        if moved.project
            && !view.project.unfollowed_said
            && matches!(view.project.last, Some(Look::Applied | Look::Refused { .. }))
            && sources.news.project_file_listed(path) == Some(false)
        {
            warn!("{} is not listed by the project index (a .gitignore rule or a skipped folder hides it), so an edit to it is not followed until the project is opened again", path.display());
            view.project.unfollowed_said = true;
        }
    } else if view.project.path.is_some() {
        view.project = Watch::default();
    }
    // **The file that chose a Custom agent and gave it no command** (#425), judged on both layers together: a project that
    // chooses Custom is fine when the user's file holds the command.
    let culprit = custom_agent_without_command(view.user.good.as_ref(), view.project.good.as_ref());
    let project_chose = view.project.good.as_ref().and_then(|layer| layer.decision_model);
    let looked: Vec<AgentFile> = looks.iter().map(|(_, _, source)| *source).collect();
    for (file, look, source) in looks {
        let verdict = match look {
            Look::Applied => agent_verdict(file, source, culprit, project_chose),
            Look::Gone => {
                if let Some(reloads) = reloads.as_deref_mut() {
                    reloads.forget_file(KIND, &file);
                }
                continue;
            }
            Look::Refused { field, reason } => {
                KindVerdict::refused(KIND, file, field, format!("{reason}; the last good settings stay"))
            }
        };
        if verdict.outcome != Outcome::Applied
            && let Some(state) = state.as_deref_mut()
        {
            state.status.problem(format!("{}: refused", verdict.sentence()));
        }
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.decide(vec![verdict]);
        }
    }
    // **The other file's verdict follows**: a project file that now sets the command clears the user file's problem, and one that
    // chooses Custom without it raises it, though that other file did not change. A file whose own text changed but read as `Applied`
    // again (the fix a person pressed) says nothing new as a look, so the verdict is also decided again whenever the culprit moved.
    let moved = culprit != *last_culprit;
    *last_culprit = culprit;
    if !looked.is_empty() || moved {
        for (file, source) in named {
            let good = match source {
                AgentFile::User => view.user.good.is_some(),
                AgentFile::Project => view.project.good.is_some(),
            };
            if !good || looked.contains(&source) {
                continue;
            }
            if let Some(reloads) = reloads.as_deref_mut() {
                reloads.decide(vec![agent_verdict(file, source, culprit, project_chose)]);
            }
        }
    }
    let resolved = EditorSettings::resolve_on_this_machine(
        view.user.good.as_ref(),
        view.project.good.as_ref(),
        sources.machine_agent(),
    );
    if *settings != resolved {
        // **The conflict policy is published where a caller with no world can read it** (#502): a save runs from a key
        // handler, an interval and the Files tree's menu, and none of them can ask this resource.
        crate::files::publish_conflict_policy(resolved.external_edits);
        // **And the camera's tunables, for the same reason**: the rig's maths runs in pure functions with no world.
        crate::view::publish_tunables(resolved.view);
        // **And the fixed chords** (`crate::chords`): a menu label and a tooltip read them where no world is.
        crate::chords::publish(&resolved.keymap);
        *settings = resolved;
    }
}
