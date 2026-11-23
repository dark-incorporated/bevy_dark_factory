//! **What the settings screen lists: every key of a settings file, with its words** (UI-71, #249).
//!
//! A file ([`SettingsLayer`]) says only the keys it sets. The screen needs more than a struct to draw a
//! key: what it is called, which category it sits under, what it does, what its default is, and what
//! kind of control sets it. That is [`Setting`], and [`CATALOGUE`] is the one list of them, so the
//! screen, its search and its Reset read the same table and a key added to [`SettingsLayer`] is listed
//! here in the same commit (`tests/the_settings_screen_lists_every_key.rs` checks the two agree).
//!
//! # A value, whatever the key's type
//!
//! [`Value`] is a number, a switch or a choice among named options, so [`SettingsLayer::get`],
//! [`SettingsLayer::set`] and [`SettingsLayer::clear`] reach any key by its name without the screen
//! knowing the field. [`write_key`] is the only place a settings file is written from the screen: it
//! reads the file as the editor reads it, changes one key, and writes the file back atomically, and it
//! refuses to touch a file that does not read, as every writer in this crate does.

use std::path::Path;

use crate::settings::{EditorSettings, ExternalEdits, GRID_STRENGTH_RANGE, KIND, SettingsLayer, SyntaxTheme, TerminalAgent, TerminalCursor, Theme};
use bevy_oracle::DecisionModel;

/// One key's value: what a control shows and what a write carries.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    On(bool),
    Number(f32),
    /// The index of one of the key's named options.
    Option(usize),
}

/// How a key is set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Switch,
    /// A number between two ends, set by a slider; shown as a percentage.
    Scale { min: f32, max: f32 },
    /// One of a few named values.
    Options(&'static [&'static str]),
}

/// One key of a settings file, in the words the screen shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setting {
    /// The key as the file spells it.
    pub key: &'static str,
    /// The words on the control: sentence case, unique among settings.
    pub label: &'static str,
    /// The tab it is listed under.
    pub category: &'static str,
    /// What it does, in a sentence.
    pub description: &'static str,
    pub kind: Kind,
}

/// **The ends of the two size sliders**: 75% to 200%. A file may set any size
/// [`crate::settings::SCALE_RANGE`] allows, and a slider shows the nearest end of this range for one
/// outside it; the range is what a person can reach by moving the control.
pub const SIZE_SLIDER: (f32, f32) = (0.75, 2.0);

/// **How far one step of a scale slider moves it**: a twentieth, 5%.
pub const SCALE_STEP: f32 = 0.05;

/// **The ladder `Cmd+=` and `Cmd+-` step the interface size along**, as multiples of the design.
///
/// A list, not a multiply: `1.1^4` is 146.41%, and a scale that drifts to 137% when a person asked
/// for "one step up" three times is a control nobody can predict. Every rung is a number a settings
/// file reads back exactly, and the last is the slider's own ceiling ([`SIZE_SLIDER`]).
pub const INTERFACE_STEPS: [f32; 6] = [1.0, 1.10, 1.25, 1.50, 1.75, 2.00];

/// **The next rung up from `now`**, or `now` at the top. A size between two rungs (a file may set any
/// value in [`crate::settings::SCALE_RANGE`]) steps to the first rung above it.
pub fn interface_step_up(now: f32) -> f32 {
    INTERFACE_STEPS
        .iter()
        .copied()
        .find(|step| *step > now + f32::EPSILON)
        .unwrap_or(INTERFACE_STEPS[INTERFACE_STEPS.len() - 1])
}

/// **The next rung down from `now`**, or `now` at the bottom.
pub fn interface_step_down(now: f32) -> f32 {
    INTERFACE_STEPS
        .iter()
        .rev()
        .copied()
        .find(|step| *step < now - f32::EPSILON)
        .unwrap_or(INTERFACE_STEPS[0])
}

/// The categories, in the order the screen's tabs show them.
pub const CATEGORIES: &[&str] = &["Appearance", "Accessibility", "Map", "Terminal", "Decisions"];

/// **The intervals the periodic save offers** (#508), in seconds, and their words. `0` is off, which is the shipped default:
/// the draft is the part worth having unconditionally, and a timer that writes a project is a behaviour a person asks for.
pub const AUTO_SAVE_CHOICES: [f64; 5] = [0.0, 30.0, 60.0, 300.0, 600.0];
/// The words the Auto save row offers, in the same order as [`AUTO_SAVE_CHOICES`].
pub const AUTO_SAVE_NAMES: &[&str] = &["Off", "Every 30 s", "Every minute", "Every 5 minutes", "Every 10 minutes"];

/// The names of [`Theme`]'s variants, in declaration order.
pub const THEMES: &[&str] = &["Tool dark", "Parchment", "High contrast"];

/// The names of [`SyntaxTheme`]'s variants, in declaration order.
pub const SYNTAX_THEMES: &[&str] = &["Skin", "Warm", "Cool", "Mono", "Separated"];

/// The names of [`TerminalCursor`]'s variants, in declaration order.
pub const TERMINAL_CURSORS: &[&str] = &["Block", "Underline", "Bar"];

/// The names of [`TerminalAgent`]'s variants, in declaration order.
pub const TERMINAL_AGENTS: &[&str] = &["Off", "Oh My Pi", "Custom command"];

/// The names of [`DecisionModel`]'s variants, in declaration order.
pub const DECISION_MODELS: &[&str] = &["Local", "Remote"];

/// **What the Remote choice says before it is chosen** (DM-03, #454): where the pictures go, whose choice it is, and what it needs.
/// Part of the row's description, so it is read, and found by a search for `openrouter`.
pub const REMOTE_DISCLOSURE: &str = "Remote sends screenshots of this project to OpenRouter and Perplexity.";

/// **The scrollback lengths the screen offers** (#324), and their words. A file may set any length
/// [`crate::settings::SCROLLBACK_RANGE`] allows; the screen shows the nearest of these for one between.
pub const SCROLLBACK_CHOICES: &[usize] = &[1_000, 5_000, 10_000, 50_000];
pub const SCROLLBACK_NAMES: &[&str] = &["1,000 lines", "5,000 lines", "10,000 lines", "50,000 lines"];

/// **Every key a settings file may set.** The order is the order within a category.
pub const CATALOGUE: &[Setting] = &[
    Setting {
        key: "theme",
        label: "Theme",
        category: "Appearance",
        description: "The colours and look of the editor.",
        kind: Kind::Options(THEMES),
    },
    // **The code editor's hues** (#525, `feedback-1795094391027`): five presets gooey derives from the skin's seeds. `Skin`
    // is the shipped default and draws exactly the colours the editor drew before this row existed, so a person who never
    // opens the screen sees nothing move.
    Setting {
        key: "syntax_theme",
        label: "Code colours",
        category: "Appearance",
        description: "The hues the code editor gives a key, a string, a number, a keyword, a reference, a function and a type, in a file open in the Files workspace. Skin is the editor's own; Warm is amber and sand; Cool is blues and violets; Mono is one hue told apart by lightness, for quiet reading and for readers who cannot tell hues apart; Separated spreads the hues as far as they go, for reading a pane at a glance. A comment, an operator and an error keep their colours in every set.",
        kind: Kind::Options(SYNTAX_THEMES),
    },
    Setting {
        key: "text_scale",
        label: "Text size",
        category: "Appearance",
        description: "How large the text is. 100% is the size the editor was designed at.",
        kind: Kind::Scale { min: SIZE_SLIDER.0, max: SIZE_SLIDER.1 },
    },
    Setting {
        key: "interface_scale",
        label: "Interface size",
        category: "Appearance",
        description: "How large buttons and spacing are. 100% is the size the editor was designed at.",
        kind: Kind::Scale { min: SIZE_SLIDER.0, max: SIZE_SLIDER.1 },
    },
    Setting {
        key: "show_statistics",
        label: "Show statistics",
        category: "Appearance",
        description: "Show how many triangles are drawn, at the top right of the window.",
        kind: Kind::Switch,
    },
    Setting {
        key: "show_activity_bar",
        label: "Show activity bar",
        category: "Appearance",
        description: "Show the strip of workspace icons at the left edge of the window. Off, move between workspaces with the keys or the Window menu.",
        kind: Kind::Switch,
    },
    // **Why this row is in Decisions, and not a Privacy tab of its own** (DL-01, #460): what is collected is the data the decision model is trained on, and
    // a sixth tab changed the card's layout enough that the protected `a_settings_change_updates_only_its_row` tests, which compare rows' heights to the pixel
    // after the card re-centres, failed on a one pixel rounding of a row that had not changed.
    Setting {
        key: "collect_labels",
        label: "Learn from my use",
        category: "Decisions",
        description: "Keep a record of each agent turn, for training the editor's decision model: the pictures before and after, what was asked, what was done and what you did about it. It is kept on this computer, in the Blueprint folder of your data directory, never in a project, and nothing is sent anywhere. Collection stops by itself at 2 GB. Off, no picture is taken and nothing is written.",
        kind: Kind::Switch,
    },
    // **A capture filed as an issue** (#495): one switch, off by default so a person opts in once, and
    // one row naming the checkout that receives it. An empty checkout means captures stay local, which
    // is what the row's description says.
    Setting {
        key: "capture_issue",
        label: "File captures as issues",
        category: "Decisions",
        description: "When you press Cmd+D, copy the capture into the checkout below, commit it on a branch of its own, push it, and open a GitHub issue in the editor's repository with the pictures embedded. Off, a capture is only saved to this computer.",
        kind: Kind::Switch,
    },
    // **A file edited under unsaved work** (#502): the one value the map's watcher and the Files workspace's save both read.
    // `Prompt` is the shipped default, and the row's description says what each answer costs, because `Take the file` is the
    // one answer that can lose work.
    Setting {
        key: "external_edits",
        label: "When a file changes under my edits",
        category: "Map",
        description: "A file you have unsaved edits in is changed on disk — by an agent, a script or another editor. Ask me leaves both versions alone until you answer; Take the file drops your edits and reads the file; Keep mine keeps your text, and a save then overwrites the file. The map you are editing and a file open in the Files workspace obey this one setting, so they cannot disagree.",
        kind: Kind::Options(ExternalEdits::NAMES),
    },
    // **A periodic save, off until asked for** (#508): it reuses the manual save path (`map_save` and the buffers' own save),
    // so a timer's write is a `Cmd+S` in the log. Off is the shipped default; the draft is the part worth having anyway.
    Setting {
        key: "auto_save_interval",
        label: "Auto save",
        category: "Map",
        description: "How often the map and the files open in the Files workspace are saved, through the same save Cmd+S makes. Off keeps the work only when you save it, though an unsaved buffer's draft is still kept. Nothing is saved while a question is up, while no project is open, or while the game plays.",
        kind: Kind::Options(AUTO_SAVE_NAMES),
    },
    Setting {
        key: "reduced_motion",
        label: "Reduced motion",
        category: "Accessibility",
        description: "Dialogs, menus and panels stop sliding and growing and fade in place instead; colours still ease.",
        kind: Kind::Switch,
    },
    Setting {
        key: "follow_agent",
        label: "Follow the agent",
        category: "Map",
        description: "The view goes to each change the NightShift makes, one at a time, waiting while you are working in the viewport. Off, nothing moves and the look chip still offers Look.",
        kind: Kind::Switch,
    },
    Setting {
        key: "show_grid",
        label: "Grid lines",
        category: "Map",
        description: "Draw the map's grid over the ground in the viewport.",
        kind: Kind::Switch,
    },
    Setting {
        key: "grid_strength",
        label: "Grid strength",
        category: "Map",
        description: "How strongly the grid is drawn; less is fainter.",
        kind: Kind::Scale { min: GRID_STRENGTH_RANGE.0, max: GRID_STRENGTH_RANGE.1 },
    },
    Setting {
        key: "decision_model",
        label: "Decision model",
        category: "Decisions",
        description: "The model that answers the editor's decision questions, an agent's blueprint_decide and the labeller. Local asks d3-flash on this network, and nothing leaves it. Remote asks Perplexity's model through OpenRouter. Remote sends screenshots of this project to OpenRouter and Perplexity. Remote is your own choice, made here for just you (a project's settings file cannot make it), and it needs OPENROUTER_API_KEY in the environment.",
        kind: Kind::Options(DECISION_MODELS),
    },
    Setting {
        key: "terminal_text_scale",
        label: "Terminal text size",
        category: "Terminal",
        description: "How large the terminal's text is. 100% is the size it ships at; the shell is told the new rows and columns.",
        kind: Kind::Scale { min: SIZE_SLIDER.0, max: SIZE_SLIDER.1 },
    },
    Setting {
        key: "terminal_cursor",
        label: "Terminal cursor",
        category: "Terminal",
        description: "The terminal's cursor when a program asks for none: a block over the character, an underline or a bar.",
        kind: Kind::Options(TERMINAL_CURSORS),
    },
    Setting {
        key: "terminal_cursor_blink",
        label: "Terminal cursor blinks",
        category: "Terminal",
        description: "The terminal's cursor blinks; off, it stays lit.",
        kind: Kind::Switch,
    },
    Setting {
        key: "terminal_scrollback",
        label: "Terminal scrollback",
        category: "Terminal",
        description: "How many lines of output each terminal keeps to scroll back through and to find in.",
        kind: Kind::Options(SCROLLBACK_NAMES),
    },
    Setting {
        key: "terminal_agent",
        label: "Terminal agent",
        category: "Terminal",
        description: "The agent the terminal's Start agent button starts, connected to the editor's tools and checked with a self test. The terminal starts no agent by itself: a session costs tokens to warm up. Custom command runs terminal_agent_command from settings.yaml, and is offered once a command is set. Which model omp runs is terminal_agent_model in settings.yaml: name one, like anthropic/claude-sonnet-5-5, and every editor starts the same agent; leave it out and omp picks, which is a different model on each machine.",
        kind: Kind::Options(TERMINAL_AGENTS),
    },
];

/// The setting with this key.
pub fn find(key: &str) -> Option<&'static Setting> {
    CATALOGUE.iter().find(|s| s.key == key)
}

/// **Does `query` find this setting?** Every word of the query is somewhere in the setting's label,
/// key, category or description, whatever the case.
pub fn matches(setting: &Setting, query: &str) -> bool {
    let hay = format!("{} {} {} {}", setting.label, setting.key, setting.category, setting.description).to_lowercase();
    query.split_whitespace().all(|word| hay.contains(&word.to_lowercase()))
}

/// The settings `query` finds, in catalogue order; all of them when the query is empty. With a
/// `category`, only that tab's.
pub fn search(query: &str, category: Option<&str>) -> Vec<&'static Setting> {
    CATALOGUE
        .iter()
        .filter(|s| category.is_none_or(|c| s.category == c))
        .filter(|s| matches(s, query))
        .collect()
}

/// Where `theme` sits in [`THEMES`], the order the Settings row offers.
///
/// `pub` because a message that has to name a theme must use the same word the row does — one table,
/// so a refusal and the setting cannot call Parchment two things.
pub fn theme_index(theme: Theme) -> usize {
    match theme {
        Theme::ToolDark => 0,
        Theme::Parchment => 1,
        Theme::HighContrast => 2,
    }
}

/// Where a [`SyntaxTheme`] sits in [`SYNTAX_THEMES`], the order the Settings row offers.
pub fn syntax_theme_index(theme: SyntaxTheme) -> usize {
    match theme {
        SyntaxTheme::Skin => 0,
        SyntaxTheme::Warm => 1,
        SyntaxTheme::Cool => 2,
        SyntaxTheme::Mono => 3,
        SyntaxTheme::Separated => 4,
    }
}

fn syntax_theme_at(index: usize) -> SyntaxTheme {
    match index {
        1 => SyntaxTheme::Warm,
        2 => SyntaxTheme::Cool,
        3 => SyntaxTheme::Mono,
        4 => SyntaxTheme::Separated,
        _ => SyntaxTheme::Skin,
    }
}

fn cursor_index(cursor: TerminalCursor) -> usize {
    match cursor {
        TerminalCursor::Block => 0,
        TerminalCursor::Underline => 1,
        TerminalCursor::Bar => 2,
    }
}

fn cursor_at(index: usize) -> TerminalCursor {
    match index {
        1 => TerminalCursor::Underline,
        2 => TerminalCursor::Bar,
        _ => TerminalCursor::Block,
    }
}

/// Where an agent sits in [`TERMINAL_AGENTS`], for a screen that shows or writes the choice. Public
/// because the Settings screen shows the terminal's effective agent (`settings_screen::refresh`) and
/// that is the one place the option order is defined.
pub fn agent_index(agent: TerminalAgent) -> usize {
    match agent {
        TerminalAgent::Off => 0,
        TerminalAgent::OhMyPi => 1,
        TerminalAgent::Custom => 2,
    }
}

fn agent_at(index: usize) -> TerminalAgent {
    match index {
        1 => TerminalAgent::OhMyPi,
        2 => TerminalAgent::Custom,
        _ => TerminalAgent::Off,
    }
}

/// **The offered interval nearest `seconds`**, so a file that sets one the row does not offer still shows a rung.
fn auto_save_index(seconds: f64) -> usize {
    AUTO_SAVE_CHOICES
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| (*a - seconds).abs().total_cmp(&(*b - seconds).abs()))
        .map_or(0, |(at, _)| at)
}

/// The interval at `index` in [`AUTO_SAVE_CHOICES`]; anything past the end is off.
fn auto_save_at(index: usize) -> f64 {
    AUTO_SAVE_CHOICES.get(index).copied().unwrap_or(0.0)
}

/// Where an [`ExternalEdits`] answer sits in [`ExternalEdits::NAMES`], for the row that shows or writes it.
pub fn external_edits_index(answer: ExternalEdits) -> usize {
    match answer {
        ExternalEdits::Prompt => 0,
        ExternalEdits::ReloadFromDisk => 1,
        ExternalEdits::KeepEditorChanges => 2,
    }
}

/// The answer at `index` in [`ExternalEdits::NAMES`]; anything past the end is the default, `Prompt`.
fn external_edits_at(index: usize) -> ExternalEdits {
    match index {
        1 => ExternalEdits::ReloadFromDisk,
        2 => ExternalEdits::KeepEditorChanges,
        _ => ExternalEdits::Prompt,
    }
}

/// Where a decision model sits in [`DECISION_MODELS`], for the row that shows or writes the choice.
pub fn decision_index(model: DecisionModel) -> usize {
    match model {
        DecisionModel::Local => 0,
        DecisionModel::Remote => 1,
    }
}

fn decision_at(index: usize) -> DecisionModel {
    match index {
        1 => DecisionModel::Remote,
        _ => DecisionModel::Local,
    }
}

/// The offered scrollback length nearest `lines`.
fn scrollback_index(lines: usize) -> usize {
    SCROLLBACK_CHOICES
        .iter()
        .enumerate()
        .min_by_key(|(_, choice)| choice.abs_diff(lines))
        .map_or(0, |(at, _)| at)
}

fn scrollback_at(index: usize) -> usize {
    SCROLLBACK_CHOICES.get(index).copied().unwrap_or(crate::terminal::SCROLLBACK)
}

fn theme_at(index: usize) -> Theme {
    match index {
        1 => Theme::Parchment,
        2 => Theme::HighContrast,
        _ => Theme::ToolDark,
    }
}

impl EditorSettings {
    /// The value this key has now.
    pub fn value(&self, key: &str) -> Option<Value> {
        Some(match key {
            "theme" => Value::Option(theme_index(self.theme)),
            "syntax_theme" => Value::Option(syntax_theme_index(self.syntax_theme)),
            "text_scale" => Value::Number(self.text_scale),
            "interface_scale" => Value::Number(self.interface_scale),
            "reduced_motion" => Value::On(self.reduced_motion),
            "follow_agent" => Value::On(self.follow_agent),
            "show_grid" => Value::On(self.show_grid),
            "grid_strength" => Value::Number(self.grid_strength),
            "show_statistics" => Value::On(self.show_statistics),
            "show_activity_bar" => Value::On(self.show_activity_bar),
            "collect_labels" => Value::On(self.collect_labels),
            "terminal_text_scale" => Value::Number(self.terminal_text_scale),
            "terminal_cursor" => Value::Option(cursor_index(self.terminal_cursor)),
            "terminal_cursor_blink" => Value::On(self.terminal_cursor_blink),
            "terminal_scrollback" => Value::Option(scrollback_index(self.terminal_scrollback)),
            "terminal_agent" => Value::Option(agent_index(self.terminal_agent)),
            "decision_model" => Value::Option(decision_index(self.decision_model)),
            "capture_issue" => Value::On(self.capture_issue),
            "external_edits" => Value::Option(external_edits_index(self.external_edits)),
            "auto_save_interval" => Value::Option(auto_save_index(self.auto_save_interval_s)),
            _ => return None,
        })
    }
}

/// The value a key has when no file sets it.
pub fn default_value(key: &str) -> Option<Value> {
    EditorSettings::default().value(key)
}

/// **A value as the screen says it.**
pub fn say(setting: &Setting, value: Value) -> String {
    match (setting.kind, value) {
        (Kind::Switch, Value::On(on)) => if on { "On" } else { "Off" }.to_owned(),
        (Kind::Scale { .. }, Value::Number(n)) => format!("{}", (n * 100.0).round() / 100.0),
        (Kind::Options(names), Value::Option(i)) => names.get(i).copied().unwrap_or("?").to_owned(),
        _ => "?".to_owned(),
    }
}

impl SettingsLayer {
    /// The value this layer sets for `key`, or `None` when it leaves the key out.
    pub fn get(&self, key: &str) -> Option<Value> {
        match key {
            "theme" => self.theme.map(|t| Value::Option(theme_index(t))),
            "syntax_theme" => self.syntax_theme.map(|t| Value::Option(syntax_theme_index(t))),
            "text_scale" => self.text_scale.map(Value::Number),
            "interface_scale" => self.interface_scale.map(Value::Number),
            "reduced_motion" => self.reduced_motion.map(Value::On),
            "follow_agent" => self.follow_agent.map(Value::On),
            "show_grid" => self.show_grid.map(Value::On),
            "grid_strength" => self.grid_strength.map(Value::Number),
            "show_statistics" => self.show_statistics.map(Value::On),
            "show_activity_bar" => self.show_activity_bar.map(Value::On),
            "collect_labels" => self.collect_labels.map(Value::On),
            "terminal_text_scale" => self.terminal_text_scale.map(Value::Number),
            "terminal_cursor" => self.terminal_cursor.map(|c| Value::Option(cursor_index(c))),
            "terminal_cursor_blink" => self.terminal_cursor_blink.map(Value::On),
            "terminal_scrollback" => self.terminal_scrollback.map(|lines| Value::Option(scrollback_index(lines))),
            "terminal_agent" => self.terminal_agent.map(|a| Value::Option(agent_index(a))),
            "decision_model" => self.decision_model.map(|m| Value::Option(decision_index(m))),
            "capture_issue" => self.capture_issue.map(Value::On),
            "external_edits" => self.external_edits.map(|e| Value::Option(external_edits_index(e))),
            "auto_save_interval" => self.auto_save_interval_s.map(|s| Value::Option(auto_save_index(s))),
            // **The checkout is not a catalogue row** (#495): it is drawn in the switch's own row as a
            // text field, the way the Custom agent's command is, so the screen owns its write.
            "capture_issue_repo" => None,
            _ => None,
        }
    }

    /// Sets `key` to `value`; false when there is no such key or the value is of another kind.
    pub fn set(&mut self, key: &str, value: Value) -> bool {
        match (key, value) {
            ("theme", Value::Option(i)) => self.theme = Some(theme_at(i)),
            ("syntax_theme", Value::Option(i)) => self.syntax_theme = Some(syntax_theme_at(i)),
            ("text_scale", Value::Number(n)) => self.text_scale = Some(n),
            ("interface_scale", Value::Number(n)) => self.interface_scale = Some(n),
            ("reduced_motion", Value::On(on)) => self.reduced_motion = Some(on),
            ("follow_agent", Value::On(on)) => self.follow_agent = Some(on),
            ("show_grid", Value::On(on)) => self.show_grid = Some(on),
            ("grid_strength", Value::Number(n)) => self.grid_strength = Some(n),
            ("show_statistics", Value::On(on)) => self.show_statistics = Some(on),
            ("show_activity_bar", Value::On(on)) => self.show_activity_bar = Some(on),
            ("collect_labels", Value::On(on)) => self.collect_labels = Some(on),
            ("terminal_text_scale", Value::Number(n)) => self.terminal_text_scale = Some(n),
            ("terminal_cursor", Value::Option(i)) => self.terminal_cursor = Some(cursor_at(i)),
            ("terminal_cursor_blink", Value::On(on)) => self.terminal_cursor_blink = Some(on),
            ("terminal_scrollback", Value::Option(i)) => self.terminal_scrollback = Some(scrollback_at(i)),
            ("terminal_agent", Value::Option(i)) => self.terminal_agent = Some(agent_at(i)),
            ("decision_model", Value::Option(i)) => self.decision_model = Some(decision_at(i)),
            ("capture_issue", Value::On(on)) => self.capture_issue = Some(on),
            ("external_edits", Value::Option(i)) => self.external_edits = Some(external_edits_at(i)),
            ("auto_save_interval", Value::Option(i)) => self.auto_save_interval_s = Some(auto_save_at(i)),
            _ => return false,
        }
        true
    }

    /// Leaves `key` out of the layer: the layer below decides it again.
    pub fn clear(&mut self, key: &str) {
        match key {
            "theme" => self.theme = None,
            "syntax_theme" => self.syntax_theme = None,
            "text_scale" => self.text_scale = None,
            "interface_scale" => self.interface_scale = None,
            "reduced_motion" => self.reduced_motion = None,
            "follow_agent" => self.follow_agent = None,
            "show_grid" => self.show_grid = None,
            "grid_strength" => self.grid_strength = None,
            "show_statistics" => self.show_statistics = None,
            "show_activity_bar" => self.show_activity_bar = None,
            "collect_labels" => self.collect_labels = None,
            "terminal_text_scale" => self.terminal_text_scale = None,
            "terminal_cursor" => self.terminal_cursor = None,
            "terminal_cursor_blink" => self.terminal_cursor_blink = None,
            "terminal_scrollback" => self.terminal_scrollback = None,
            "terminal_agent" => self.terminal_agent = None,
            "decision_model" => self.decision_model = None,
            "capture_issue" => self.capture_issue = None,
            "external_edits" => self.external_edits = None,
            "auto_save_interval" => self.auto_save_interval_s = None,
            "capture_issue_repo" => self.capture_issue_repo = None,
            _ => {}
        }
    }
}

/// **The agent choices a Terminal agent row offers** (#425): `Custom command` only once a command is set, or while it is the choice
/// the row already shows (so the row never misreports what is in use). Stepping through them can never land on a choice that would be
/// refused.
pub fn agent_choices(has_command: bool, current: TerminalAgent) -> &'static [&'static str] {
    if has_command || current == TerminalAgent::Custom { TERMINAL_AGENTS } else { &TERMINAL_AGENTS[..agent_index(TerminalAgent::Custom)] }
}

/// **Writes one key of the settings file at `path`**, or takes it out when `value` is `None`; every other
/// key in the file stays as the file has it.
///
/// The file is read first, as the editor reads it ([`SettingsLayer::read`]): one that does not read is
/// refused with the file and the field and **left exactly as it is**, because writing over it would
/// throw away what a person typed. A file that is not there is made, with its folder. The write is
/// atomic ([`blueprint_model::atomic_file::write`]), so the watcher never sees half a file.
pub fn write_key(path: &Path, key: &str, value: Option<Value>) -> Result<(), String> {
    edit_layer(path, |layer| match value {
        Some(v) if !layer.set(key, v) => Err(format!("`{key}` is not a {KIND} key that takes {v:?}")),
        // **A choice without a command is never saved** (#425): `Custom` runs `terminal_agent_command`, so a file that chooses it
        // without one is a terminal whose agent cannot start. Only this key is held to it, so a file that already has the problem
        // can still be changed everywhere else.
        Some(_) if key == "terminal_agent" && layer.terminal_agent == Some(TerminalAgent::Custom) && layer.terminal_agent_command.is_none() => {
            Err("Custom command needs a command to run: type it in the Terminal agent row first, nothing was saved".to_owned())
        }
        Some(_) => Ok(()),
        None => {
            layer.clear(key);
            Ok(())
        }
    })
}

/// **Changes the settings file at `path` through `change`**, with every guarantee of [`write_key`]: the
/// file is read as the editor reads it and left alone when it does not read, what `change` leaves must read
/// back, and the write is atomic.
pub fn edit_layer(path: &Path, change: impl FnOnce(&mut SettingsLayer) -> Result<(), String>) -> Result<(), String> {
    let file = path.to_string_lossy().replace('\\', "/");
    let mut layer = match std::fs::read_to_string(path) {
        Ok(text) => SettingsLayer::read(&file, &text).map_err(|r| format!("{file} does not read ({}): {}; not changed", r.field.as_deref().unwrap_or("file"), r.reason))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => SettingsLayer::default(),
        Err(e) => return Err(format!("{file}: {e}")),
    };
    change(&mut layer)?;
    // Refuse a value the next read would refuse: the file must read back.
    let text = blueprint_model::yaml::to_string(&layer).map_err(|p| p.line())?;
    SettingsLayer::read(&file, &text).map_err(|r| r.reason)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    blueprint_model::atomic_file::write(path, text).map_err(|e| format!("{file}: {e}"))?;
    // **The write is done, so the editor's own indexes are told and have published** before this returns: the next frame sees the
    // change, as it did when the file was stat-ed every frame (#501 item 18).
    crate::project_index::note_path_written(path);
    Ok(())
}
