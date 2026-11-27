//! **The editor's style sheet, layered and live**: every value of taste the editor draws with — the skin's
//! colours and scales, the editor's own measures and the class rules that style its chrome — read from YAML
//! and applied while the editor runs, the way a web page's CSS is.
//!
//! # The layers
//!
//! Four files, a later one winning key by key:
//!
//! 1. **base** — `src/style/editor.yaml`, the editor's own sheet;
//! 2. **theme** — `src/style/theme_<name>.yaml` when the settings' `theme` is not Tool dark: colours only;
//! 3. **user** — `<config_dir>/Blueprint/style.yaml` ([`UserStyleFile`]);
//! 4. **project** — the open project's `assets/blueprint/editor_style.yaml` ([`PROJECT_FILE`]).
//!
//! **Base and theme are read from the checkout when the editor runs from one** ([`editor_source`]): edit the
//! shipped file and the running editor changes as soon as the file index sees the save (about a second at its
//! default sweep). Without a checkout they are the compiled copies. Both feed the same read, the same check and
//! the same merge, so there is one path.
//!
//! **Nothing is polled** (#501): the checkout's style folder and the person's config folder are followed by
//! [`crate::project_index::OutsideFiles`] for the layer files' names, the project's layer by the project's
//! [`crate::project_index::ProjectIndex`], and a layer is read when its index says it moved.
//!
//! # The merge
//!
//! Each layer is checked whole ([`read_layer`]: `version`, a `GooeySkin`, [`EditorMetrics`] and gooey's
//! `StyleRules`, every unknown key refused with its path). Its `skin` and `metrics` are then laid over the
//! layers before it **by the keys it states**: a skin field replaces, `skin.tones` merges tone by tone, and
//! each `metrics.<group>` merges field by field; anything deeper replaces whole, so a tagged colour is never
//! half one variant and half another. Rules concatenate in layer order, so a later layer's rule wins a tie of
//! specificity, as a later rule in one file does.
//!
//! # What reads it
//!
//! [`StylePlugin`] writes the merged skin into gooey's `GooeySkin` (keeping the person's `text_scale`), the
//! metrics into [`metrics`] (read by code that measures, as [`crate::chrome::ink`] is), and the rules into
//! gooey's `StyleSheet`, whose cascade styles every node carrying a `StyleClass`. A file that does not read
//! keeps the last good one and is refused in `blueprint_state.reloads` (kind `editor_style`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, PoisonError, RwLock};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use blueprint_model::live::KindVerdict;
use blueprint_model::reload_check::{self, Refusal};
use bevy_gooey::{GooeySkin, StyleRules, StyleSheet, StyleSheetProblems};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::project_index::{self, OutsideFileChanged, OutsideFiles, ProjectFileChanged, ProjectIndex};
use crate::settings::{Feed, Theme};

/// The one version of a style layer this build reads.
pub const STYLE_VERSION: u32 = 1;

/// The reload registry's kind for every style layer (`blueprint_model::reload::KINDS`).
pub const KIND: &str = "editor_style";

/// **A project's style layer**, under the project's blueprint directory, beside its `editor.yaml` settings.
pub const PROJECT_FILE: &str = "editor_style.yaml";

/// Where the base and theme sheets live inside a checkout.
const STYLE_DIR: &str = "crates/bevy_dark_factory/src/style";

/// The base sheet's file name in [`STYLE_DIR`].
const BASE_FILE: &str = "editor.yaml";

/// The file a checkout must hold to be the editor's source ([`editor_source`]).
const SOURCE_MARK: &str = "crates/bevy_dark_factory/src/style/editor.yaml";

/// Every layer file a checkout's [`STYLE_DIR`] is followed for: the base and each theme's.
const CHECKOUT_FILES: [&str; 3] = [BASE_FILE, PARCHMENT_FILE, HIGH_CONTRAST_FILE];

/// The Parchment layer's file name in [`STYLE_DIR`].
const PARCHMENT_FILE: &str = "theme_parchment.yaml";
/// The High contrast layer's file name in [`STYLE_DIR`].
const HIGH_CONTRAST_FILE: &str = "theme_high_contrast.yaml";

/// The compiled base sheet.
const BASE_TEXT: &str = include_str!("style/editor.yaml");
/// The compiled Parchment layer.
const PARCHMENT_TEXT: &str = include_str!("style/theme_parchment.yaml");
/// The compiled High contrast layer.
const HIGH_CONTRAST_TEXT: &str = include_str!("style/theme_high_contrast.yaml");

/// The name a theme's layer has in a checkout's [`STYLE_DIR`], and its compiled text; `None` for Tool dark,
/// which is the base sheet alone.
fn theme_layer(theme: Theme) -> Option<(&'static str, &'static str)> {
    match theme {
        Theme::ToolDark => None,
        Theme::Parchment => Some((PARCHMENT_FILE, PARCHMENT_TEXT)),
        Theme::HighContrast => Some((HIGH_CONTRAST_FILE, HIGH_CONTRAST_TEXT)),
    }
}

// ── metrics ──────────────────────────────────────────────────────────────────────────────────────

/// **The type scale**: each role's size in logical px at 100% text, and two metrics of the face.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TypeMetrics {
    /// The panel's own name — one per panel.
    pub title: f32,
    /// A tab's word in the strip, and a chooser's column header.
    pub tab: f32,
    /// The readable default: a value, a field's contents, a census row.
    pub body: f32,
    /// A block or list heading; it outranks the label it heads (#518).
    pub heading: f32,
    /// The dim half of a label/value pair, and a log line.
    pub label: f32,
    /// A control's own word: a chip, a button, a field.
    pub control: f32,
    /// The smallest thing on screen: a badge's description, a footnote.
    pub hint: f32,
    /// A line of text as a multiple of its size: Bevy 0.20's default `LineHeight::RelativeToFont(1.2)`.
    pub line_height_factor: f32,
    /// One glyph's advance at `body`, logical px: the shipped face is monospace, so `chars * body_char_w` is
    /// exact.
    pub body_char_w: f32,
}

impl Default for TypeMetrics {
    fn default() -> Self {
        TypeMetrics {
            title: 15.0,
            tab: 13.0,
            body: 11.0,
            heading: 11.0,
            label: 10.0,
            control: 10.0,
            hint: 9.0,
            line_height_factor: 1.2,
            body_char_w: 6.6,
        }
    }
}

/// **The editor's layout measures**, logical px unless named otherwise.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LayoutMetrics {
    /// **The interface scale the design is drawn at**, and the one the application starts at. A base, not an
    /// answer (2026-09-03): the display's scale factor reports *density*, not *size*, so a bigger window grows
    /// from this in `crate::surface::ui_scale_for`; a larger constant made a 1280 x 800 window all docks.
    pub ui_scale: f32,
    /// Where the tab strip ends.
    pub tab_strip_bottom: f32,
    /// **A controls panel's preferred width — one number for every left dock.** It was 300 on one door and 380
    /// on three, so the viewport jumped 80 px between doors; the wider wins because the Tiles inspector's tag
    /// chips wrap to twice the height at 300.
    pub controls_w: f32,
    /// A list panel's preferred width: a row is a thumbnail, an id and a number rather than two columns.
    pub list_w: f32,
    /// **The narrowest a dock may be squeezed, as a share of `list_w`** (B-014): below it a panel stops being a
    /// panel (its chips stack into single words), so panels shrink down to it and the viewport gets the rest.
    pub dock_min_share: f32,
    /// **The width the viewport is owed before either dock gets more than its floor** (B-014): the map was the
    /// leftover of two fixed panels, 69 px at a 900 px window. A reserve, not a guarantee: a window too narrow
    /// for it and two floors gives the docks their floors and the viewport the remainder (379 px measured).
    pub viewport_min_w: f32,
    /// **The share of the window the viewport is owed once that is more than `viewport_min_w`** (#282).
    pub viewport_share: f32,
    /// **The room the shell's split divider takes** (#476, `feedback-1794783253218`: *"maybe only half as thick
    /// as this?"*): the owner's 12, which gooey draws at its `SplitThickness`.
    pub divider_w: f32,
    /// The bottom dock's height.
    pub bottom_dock_h: f32,
    /// The least height of a workbench's floor.
    pub workbench_floor_h: f32,
    /// A row's preview box: the map palette and the kit door's MESHES shelf draw a portrait in it.
    pub thumb_slot: f32,
    /// The chrome bar's height.
    pub bar_h: f32,
    /// A scroll bar's width.
    pub scroll_bar_w: f32,
    /// How far a scroll bar is inset from the panel's inner edge.
    pub scroll_bar_inset: f32,
    /// How many rows of list stay visible past the selection: one proves the list continues, a third scrolls
    /// the viewport sooner than the eye asks.
    pub context_rows: f32,
    /// **The rail that says "this one"** — the accent down a selected row's leading edge: a shape is
    /// unambiguous at any fill, where a loud fill failed contrast for a label on it.
    pub select_rail_w: f32,
    /// The snip's drag-rectangle border: heavier than the rail, because at 2 px a drag was 1% of a 600 px
    /// capture and hard to find.
    pub snip_edge_w: f32,
    /// One hairline. Not two: a 2 px border reads as a frame, and a work surface is not a picture.
    pub edge_w: f32,
    /// **How far a ground fill steps back (its alpha) while the shortcut key is held**, so a badge is plainly
    /// the nearest thing and the panel plainly behind it. Only the fill: words keep their opacity.
    pub ground_held: f32,
    /// How far the world's own wireframe steps back (its alpha) while the shortcut key is held: deeper than a
    /// ground, because a line crossing a badge reads as a leader.
    pub world_held: f32,
    /// The smallest window the layout is built for, width.
    pub window_min_w: f32,
    /// The smallest window the layout is built for, height.
    pub window_min_h: f32,
    /// The largest window the monitor fit gives, width.
    pub window_max_fit_w: f32,
    /// The largest window the monitor fit gives, height.
    pub window_max_fit_h: f32,
    /// The share of the monitor's logical size the fitted window takes.
    pub window_fit_fraction: f32,
}

impl Default for LayoutMetrics {
    fn default() -> Self {
        LayoutMetrics {
            ui_scale: 1.2,
            tab_strip_bottom: 46.0,
            controls_w: 380.0,
            list_w: 264.0,
            dock_min_share: 0.6,
            viewport_min_w: 420.0,
            viewport_share: 0.52,
            divider_w: 12.0,
            bottom_dock_h: 148.0,
            workbench_floor_h: 420.0,
            thumb_slot: 30.0,
            bar_h: 26.0,
            scroll_bar_w: 5.0,
            scroll_bar_inset: 1.0,
            context_rows: 2.0,
            select_rail_w: 2.0,
            snip_edge_w: 3.0,
            edge_w: 1.0,
            ground_held: 0.55,
            world_held: 0.25,
            window_min_w: 1024.0,
            window_min_h: 640.0,
            window_max_fit_w: 1600.0,
            window_max_fit_h: 1000.0,
            window_fit_fraction: 0.85,
        }
    }
}

impl LayoutMetrics {
    /// The narrowest a dock may be squeezed before the viewport gets the rest.
    pub fn dock_min_w(&self) -> f32 {
        self.list_w * self.dock_min_share
    }

    /// What a scroll bar reserves: the bar and its inset.
    pub fn scroll_gutter(&self) -> f32 {
        self.scroll_bar_w + self.scroll_bar_inset
    }
}

/// **The stacking order of the editor's layers** (Bevy's `GlobalZIndex`), lowest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ZMetrics {
    pub balloon: i32,
    pub compass: i32,
    pub terminal_maximised: i32,
    pub toast: i32,
    pub notice: i32,
    pub menu_overlay: i32,
    pub badge: i32,
    pub modal: i32,
    pub report: i32,
}

impl Default for ZMetrics {
    fn default() -> Self {
        ZMetrics {
            balloon: 50,
            compass: 60,
            terminal_maximised: 80,
            toast: 150,
            notice: 300,
            menu_overlay: 400,
            badge: 500,
            modal: 900,
            report: 1000,
        }
    }
}

/// **How the chrome's derived inks are stepped from the skin** ([`crate::chrome::Ink::of`]).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct InkMetrics {
    /// How far the chrome bar sits back from a slot's ground (a multiple of its channels).
    pub bar_step_back: f32,
    /// How far the unchecked ink is lifted off the suggest tone.
    pub unchecked_lift: f32,
    /// The surface-ladder step a selected row is drawn on.
    pub row_selected_step: u8,
    /// The step an inspector slot is drawn on.
    pub slot_step: u8,
    /// The step a group heading band is drawn on.
    pub header_step: u8,
    /// The step of the hairline that ends a surface.
    pub edge_step: u8,
}

impl Default for InkMetrics {
    fn default() -> Self {
        InkMetrics { bar_step_back: 0.78, unchecked_lift: 1.45, row_selected_step: 6, slot_step: 4, header_step: 2, edge_step: 7 }
    }
}

/// **The editor's own measures**, as the style sheet's `metrics` section: a layer may set any field and
/// leave the rest to the layers below.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EditorMetrics {
    pub r#type: TypeMetrics,
    pub layout: LayoutMetrics,
    pub z: ZMetrics,
    pub ink: InkMetrics,
}

/// **The metrics the editor is measuring with right now**, by the same rule as [`crate::chrome::ink`]: a
/// process-wide value because the code that measures (a dock's arithmetic, a text's width) holds no system
/// parameter, and a process has one editor. Only [`StylePlugin`] writes it.
static LIVE_METRICS: LazyLock<RwLock<EditorMetrics>> = LazyLock::new(|| RwLock::new(compiled_default().metrics));

/// **The editor's metrics now**, by value.
pub fn metrics() -> EditorMetrics {
    // A poisoned lock still holds a whole value: it is only ever replaced, never edited in place.
    *LIVE_METRICS.read().unwrap_or_else(PoisonError::into_inner)
}

/// Replaces the running metrics and returns the ones it replaced. Only [`StylePlugin`] calls it.
pub(crate) fn swap_metrics(next: EditorMetrics) -> EditorMetrics {
    std::mem::replace(&mut *LIVE_METRICS.write().unwrap_or_else(PoisonError::into_inner), next)
}

/// **The sheet's spacing scale now** (`skin.spacing`, logical px, smallest first), for code that measures
/// with a step rather than spawning one: a node spawns a step through a class rule, and arithmetic about
/// the same room (a badge's reach, a chip's width) reads it here so the two cannot disagree.
static LIVE_SPACING: LazyLock<RwLock<[f32; 6]>> = LazyLock::new(|| RwLock::new(compiled_default().skin.spacing));

/// **A step of the sheet's spacing scale in logical px**, as gooey's `Space::px` resolves it: a step past
/// the end is the last, a literal is itself, `Zero` is none.
pub fn space(space: bevy_gooey::Space) -> f32 {
    let scale = *LIVE_SPACING.read().unwrap_or_else(PoisonError::into_inner);
    match space {
        bevy_gooey::Space::Zero => 0.0,
        bevy_gooey::Space::Step(step) => scale[step.min(scale.len() - 1)],
        bevy_gooey::Space::Px(px) => px.max(0.0),
    }
}

fn publish_spacing(next: [f32; 6]) {
    let mut held = LIVE_SPACING.write().unwrap_or_else(PoisonError::into_inner);
    if *held != next {
        *held = next;
    }
}

// ── a layer ──────────────────────────────────────────────────────────────────────────────────────

/// What a layer must read as, whole, before any of it is merged: the check that names a bad key's path.
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct StyleLayerCheck {
    version: u32,
    #[allow(dead_code)]
    skin: GooeySkin,
    #[allow(dead_code)]
    metrics: EditorMetrics,
    rules: StyleRules,
}

impl Default for StyleLayerCheck {
    fn default() -> Self {
        StyleLayerCheck { version: STYLE_VERSION, skin: GooeySkin::default(), metrics: EditorMetrics::default(), rules: StyleRules::default() }
    }
}

/// **One layer that read**: the keys it states, its rules and the line each rule stands on.
#[derive(Clone, Debug, PartialEq)]
pub struct StyleLayer {
    /// The name a refusal and the inspector say: a checkout's `crates/.../style/editor.yaml`, the user's
    /// path, or the project's file.
    pub name: String,
    skin: Map<String, Value>,
    metrics: Map<String, Value>,
    rules: StyleRules,
    /// Each rule's selector and the line (from 1) its key stands on.
    lines: BTreeMap<String, usize>,
}

/// **Read `text` as a style layer named `name`**, refusing with the field when it does not read or is
/// another version.
pub fn read_layer(name: &str, text: &str) -> Result<StyleLayer, Refusal> {
    let check: StyleLayerCheck =
        reload_check::read(KIND, name, text, |t| blueprint_model::yaml::from_str(t, "style").map_err(|p| p.line()))?;
    let refuse = |field: Option<&str>, reason: String| Refusal { kind: KIND, file: name.to_owned(), field: field.map(str::to_owned), reason };
    if check.version != STYLE_VERSION {
        return Err(refuse(Some("version"), format!("version {} is not the {STYLE_VERSION} this build reads", check.version)));
    }
    let json = blueprint_model::yaml::to_json(text, name).map_err(|p| refuse(None, p.line()))?;
    let section = |key: &str| match json.get(key) {
        Some(Value::Object(map)) => map.clone(),
        _ => Map::new(),
    };
    let mut rules = check.rules;
    for rule in &mut rules.0 {
        rule.source = name.to_owned();
    }
    let at = blueprint_model::yaml::lines(text);
    let lines = rules
        .0
        .iter()
        .filter_map(|rule| at.get(&format!("rules.{}", rule.selector.text())).map(|line| (rule.selector.text().to_owned(), *line)))
        .collect();
    Ok(StyleLayer { name: name.to_owned(), skin: section("skin"), metrics: section("metrics"), rules, lines })
}

/// Lays `over`'s keys onto `under`; a key named in `deep` merges one level further (its own keys replace).
fn lay(under: &mut Map<String, Value>, over: &Map<String, Value>, deep: impl Fn(&str) -> bool) {
    for (key, value) in over {
        match (under.get_mut(key), value) {
            (Some(Value::Object(below)), Value::Object(above)) if deep(key) => {
                for (k, v) in above {
                    below.insert(k.clone(), v.clone());
                }
            }
            _ => {
                under.insert(key.clone(), value.clone());
            }
        }
    }
}

/// **The editor's style as the layers add up**: the skin, the metrics, the rules in cascade order, and
/// where each rule was written.
#[derive(Clone, Debug, PartialEq)]
pub struct EditorStyle {
    pub skin: GooeySkin,
    pub metrics: EditorMetrics,
    pub rules: StyleRules,
    /// `(layer name, selector)` → the line the rule's key stands on, from 1.
    pub lines: BTreeMap<(String, String), usize>,
}

/// **The layers, merged in order** (later wins), or why the sum does not read as a skin or metrics.
///
/// **The type scale is declared once, in `metrics.type`, and derived into the skin here** (#648). The sheet
/// used to state `skin.text_size`, `skin.heading_size` and `skin.type_scale` as well, three copies of numbers
/// that matched `metrics.type` by hand: the editor's own labels read `metrics.type`, every gooey label reads
/// `skin.text_size`, and so editing `metrics.type.body` left one window with two body sizes — 14 px beside
/// 11 px, the split #518 measured at 550 of 602 texts. Deriving here means a layer may still write a skin scale
/// (a theme does not, and a person should not) but it cannot disagree with the metrics.
pub fn merge(layers: &[&StyleLayer]) -> Result<EditorStyle, String> {
    let (mut skin, mut metrics) = (Map::new(), Map::new());
    let mut rules = StyleRules::default();
    let mut lines = BTreeMap::new();
    for layer in layers {
        lay(&mut skin, &layer.skin, |key| key == "tones");
        lay(&mut metrics, &layer.metrics, |_| true);
        rules.0.extend(layer.rules.0.iter().cloned());
        lines.extend(layer.lines.iter().map(|(selector, line)| ((layer.name.clone(), selector.clone()), *line)));
    }
    let mut skin: GooeySkin = serde_json::from_value(Value::Object(skin)).map_err(|e| format!("the merged skin does not read: {e}"))?;
    let metrics: EditorMetrics = serde_json::from_value(Value::Object(metrics)).map_err(|e| format!("the merged metrics do not read: {e}"))?;
    skin.text_size = metrics.r#type.body;
    skin.heading_size = metrics.r#type.title;
    skin.type_scale = type_scale_of(&metrics.r#type);
    Ok(EditorStyle { skin, metrics, rules, lines })
}

/// **The skin's `type_scale`, as the five steps `metrics.type` names**, smallest first.
///
/// `skin.type_scale` is a ladder gooey indexes by step (`GooeySkin::type_size`: step 0 is the smallest), not a
/// copy of any one role, and it has five rungs (`GooeySkin`'s array is `[f32; 5]`). The editor's seven roles
/// name five *rungs*: `hint`, `label`, `body`, `tab`, `title`. `control` and `heading` are not rungs of their
/// own — at the shipped sheet `control` is the label's 10 px (`chrome::text::CONTROL`'s doc: "the same number as
/// [`LABEL`], and a seventh role anyway") and `heading` is the body's 11 (`HEADING`'s doc: "a heading has to
/// outrank the label it heads") — so a layer that gives either a size of its own moves that role through
/// `chrome::text` and leaves this monotone ladder the five rungs it can hold.
///
/// Sorted, so `type_size` never reads a smaller rung after a larger one however a layer edits a step.
fn type_scale_of(sizes: &TypeMetrics) -> [f32; 5] {
    let mut ladder = [sizes.hint, sizes.label, sizes.body, sizes.tab, sizes.title];
    ladder.sort_by(f32::total_cmp);
    ladder
}

/// The compiled base and theme layers, read once: their text is in the binary, so a failure is a build that
/// shipped a broken sheet, which `tests/the_style_sheet_reloads_live.rs` and the theme tests catch.
static COMPILED: LazyLock<[EditorStyle; 3]> = LazyLock::new(|| {
    let read = |name: &str, text: &str| read_layer(name, text).unwrap_or_else(|r| panic!("the compiled style layer {}", r.sentence()));
    let base = read("style/editor.yaml", BASE_TEXT);
    [Theme::ToolDark, Theme::Parchment, Theme::HighContrast].map(|theme| {
        let theme_layer = theme_layer(theme).map(|(file, text)| read(&format!("style/{file}"), text));
        let layers: Vec<&StyleLayer> = std::iter::once(&base).chain(theme_layer.as_ref()).collect();
        merge(&layers).unwrap_or_else(|e| panic!("the compiled style for {theme:?}: {e}"))
    })
});

/// **The compiled sheet under `theme`**: the base and the theme's layer as the binary carries them, with no
/// user or project layer. For a test, and for the value the editor starts with before its first look.
pub fn compiled_for(theme: Theme) -> EditorStyle {
    let index = match theme {
        Theme::ToolDark => 0,
        Theme::Parchment => 1,
        Theme::HighContrast => 2,
    };
    COMPILED[index].clone()
}

/// **The compiled sheet under the default theme** (Tool dark).
pub fn compiled_default() -> EditorStyle {
    compiled_for(Theme::default())
}

/// **What the compiled sheet declares for exactly `selector`** (as written, `".chrome-col-wide"`), or `None` when
/// no rule names it. For code and tests that must agree with a rule rather than restate its number.
pub fn shipped_rule(selector: &str) -> Option<bevy_gooey::StyleDecl> {
    compiled_default().rules.0.into_iter().find(|rule| rule.selector.text() == selector).map(|rule| rule.decl)
}

// ── the source checkout ──────────────────────────────────────────────────────────────────────────

/// **The checkout this binary was built from**, when it is still on disk: `CARGO_MANIFEST_DIR/../..`
/// holding the style sheet. The rule `launcher::example_project` already uses for the example.
pub fn detected_checkout() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    is_checkout(&root).then(|| root.canonicalize().unwrap_or(root))
}

/// Whether `root` holds the editor's style sheet where a checkout keeps it.
pub fn is_checkout(root: &Path) -> bool {
    root.join(SOURCE_MARK).is_file()
}

/// The checkout the running editor reads its sheet from, as [`StylePlugin`] last resolved it.
static SOURCE: LazyLock<RwLock<Option<PathBuf>>> = LazyLock::new(|| RwLock::new(detected_checkout()));

/// **The editor's own source checkout**, or `None` when it runs from a binary with none: the settings'
/// `editor_source` when it is set, else the checkout this binary was built from ([`detected_checkout`]).
/// The base and theme sheets are read and watched there, and the editor-development agent works in it.
pub fn editor_source() -> Option<PathBuf> {
    SOURCE.read().unwrap_or_else(PoisonError::into_inner).clone()
}

fn publish_source(source: Option<PathBuf>) {
    let mut held = SOURCE.write().unwrap_or_else(PoisonError::into_inner);
    if *held != source {
        *held = source;
    }
}

// ── watching ─────────────────────────────────────────────────────────────────────────────────────

/// Names the person's style layer. Inserted by `main`; absent in a test, so no test reads the real one.
#[derive(Resource, Clone, Debug)]
pub struct UserStyleFile(pub PathBuf);

/// `<config_dir>/Blueprint/style.yaml`, or `None` on a platform with no config directory.
pub fn user_file() -> Option<PathBuf> {
    blueprint_model::user_dirs::style_file()
}

/// **What moves with each application of the sheet**, so a system that keeps something derived from the
/// metrics (a label's size, the chrome's ink) knows when to look again.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StyleRevision {
    /// Moves when [`metrics`] changes.
    pub metrics_revision: u64,
    /// Moves when gooey's `StyleSheet` is replaced.
    pub sheet_revision: u64,
}

/// **Where each rule of the running sheet was written**, for the style inspector: `(layer, selector)` →
/// line, and each layer's file on disk when it has one.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct StyleLines {
    pub lines: BTreeMap<(String, String), usize>,
    pub paths: BTreeMap<String, PathBuf>,
}

/// One layer's file, as last looked at.
#[derive(Default)]
struct Look {
    /// The file looked at, or `None` for a compiled layer.
    path: Option<PathBuf>,
    /// The name its verdicts and rules carry.
    name: String,
    /// What this layer has heard of its file's index: when it must be read again.
    feed: Feed,
    /// The text last decided, so the same bytes are decided once.
    text: Option<String>,
    /// The layer as it last read.
    good: Option<StyleLayer>,
}

/// What one look found.
enum Found {
    /// The layer read; it is the good one now.
    Applied,
    /// The file did not read: the last good layer stays.
    Refused(Refusal),
    /// The file is gone: its layer is dropped.
    Gone,
}

impl Look {
    /// Aim at a file, or at a compiled text when `path` is `None`; reset whenever either changes, and decided at once then: a
    /// file is read when it is first aimed at, and again only when its index says it moved.
    fn aim(&mut self, path: Option<PathBuf>, name: String, compiled: Option<&str>) -> Option<Found> {
        if self.path == path && self.name == name {
            return None;
        }
        *self = Look { path, name, ..Look::default() };
        match &self.path {
            None => compiled.map(|text| self.decide(text.to_owned())),
            Some(_) => self.look(),
        }
    }

    /// Reads the file; `None` when its text is the one last decided, or it is still absent.
    fn look(&mut self) -> Option<Found> {
        let path = self.path.clone()?;
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let had = self.text.is_some() || self.good.is_some();
                self.text = None;
                self.good = None;
                return had.then_some(Found::Gone);
            }
            Err(e) => return Some(Found::Refused(Refusal { kind: KIND, file: self.name.clone(), field: None, reason: e.to_string() })),
        };
        if self.text.as_ref() == Some(&text) {
            return None;
        }
        Some(self.decide(text))
    }

    fn decide(&mut self, text: String) -> Found {
        let found = match read_layer(&self.name, &text) {
            Ok(layer) => {
                self.good = Some(layer);
                Found::Applied
            }
            Err(refusal) => Found::Refused(refusal),
        };
        self.text = Some(text);
        found
    }
}

/// **What the file indexes said this frame about the layers' files** (#501): the project's layer moves with the project's index, the
/// others with the folders [`follow_the_style_files`] follows. Two message readers and a few atomic loads; no filesystem.
#[derive(SystemParam)]
pub struct StyleNews<'w, 's> {
    project_changed: MessageReader<'w, 's, ProjectFileChanged>,
    outside_changed: MessageReader<'w, 's, OutsideFileChanged>,
    project_index: Res<'w, ProjectIndex>,
    outside: Res<'w, OutsideFiles>,
}

/// This frame's news, read once and asked per layer.
struct Heard {
    /// Project-relative paths the project's index said changed.
    project: Vec<String>,
    /// `(folder, name)` pairs an outside folder's index said changed.
    outside: Vec<(PathBuf, String)>,
}

impl StyleNews<'_, '_> {
    fn heard(&mut self) -> Heard {
        Heard {
            project: self.project_changed.read().map(|e| e.rel.clone()).collect(),
            outside: self.outside_changed.read().map(|e| (e.root.clone(), e.rel.clone())).collect(),
        }
    }

    /// **Whether `look`'s file must be read now**: its index said it changed, had its first listing, or started or stopped failing.
    fn moved(&self, heard: &Heard, look: &mut Look) -> bool {
        let Some(path) = look.path.clone() else { return false };
        let under_project = self.project_index.root().and_then(|root| path.strip_prefix(root).ok()).map(|rel| rel.to_string_lossy().replace('\\', "/"));
        let (listing_in, failure, changed) = match under_project {
            Some(rel) => (self.project_index.generation() != 0, self.project_index.failure(), heard.project.contains(&rel)),
            None => match project_index::split_file(&path) {
                Some((dir, name)) => (
                    self.outside.generation(&dir) != 0,
                    self.outside.failure(&dir),
                    heard.outside.iter().any(|(root, rel)| *root == dir && *rel == name),
                ),
                None => (false, None, false),
            },
        };
        look.feed.moved(Some(&path), listing_in, failure, changed)
    }
}

/// **Follow the folders outside the project that hold style layers**, for those files' names and no others: the checkout's style folder
/// for the base and theme sheets, and the person's config folder for `style.yaml`. A checkout no longer named is unfollowed; the
/// person's folder never is, because it is also the folder `settings::follow_the_user_file` follows for `settings.yaml`, and an
/// unfollow ends the folder's thread for every name in it.
pub fn follow_the_style_files(
    settings: Option<Res<crate::settings::EditorSettings>>,
    user_file: Option<Res<UserStyleFile>>,
    mut outside: ResMut<OutsideFiles>,
    mut followed: Local<Option<PathBuf>>,
) {
    let checkout = settings.as_ref().and_then(|s| s.editor_source.clone()).or_else(detected_checkout).map(|root| root.join(STYLE_DIR));
    if *followed != checkout {
        if let Some(old) = followed.take() {
            outside.unfollow(&old);
        }
        if let Some(dir) = &checkout {
            outside.follow(dir, &CHECKOUT_FILES);
        }
        *followed = checkout;
    }
    if let Some(user) = user_file
        && user.is_changed()
        && let Some((dir, name)) = project_index::split_file(&user.0)
    {
        outside.follow(&dir, &[name.as_str()]);
    }
}

/// **The four layers' looks** and what the sheet last applied.
#[derive(Resource, Default)]
pub struct StyleWatch {
    base: Look,
    theme: Look,
    user: Look,
    project: Look,
    /// The theme the layers were last aimed for.
    theme_now: Option<Theme>,
    /// The merged style last applied, so an unchanged sum writes nothing.
    applied: Option<EditorStyle>,
    /// The problems of the rules last reported.
    problems: Vec<String>,
}

/// **Everything [`apply_style`] writes**, grouped so its argument list stays readable.
#[derive(SystemParam)]
pub struct StyleTargets<'w> {
    skin: ResMut<'w, GooeySkin>,
    sheet: Option<ResMut<'w, StyleSheet>>,
    revision: ResMut<'w, StyleRevision>,
    lines: ResMut<'w, StyleLines>,
    rebuild: MessageWriter<'w, RebuildPanels>,
}

/// **A `layout` or `z` metric moved**, and those are read where a panel is spawned (a dock's width, an
/// overlay's stacking order): the door is rebuilt in place, the way a workspace switch rebuilds it
/// (`crate::screen::rebuild_the_door`). A type size or an ink step needs no rebuild: labels resize and inks
/// repaint where they stand.
#[derive(Message, Clone, Copy, Debug)]
pub struct RebuildPanels;

/// **Read the layers their indexes say moved, decide what changed, and keep the skin, the metrics and the rules
/// the layers' sum.** A file that does not read is refused and its layer stays as it last read; a file that is
/// gone has no layer. The person's `text_scale` is the skin's whatever the sheet says.
#[allow(clippy::too_many_arguments)]
pub fn apply_style(
    mut news: StyleNews,
    settings: Option<Res<crate::settings::EditorSettings>>,
    project: Option<Res<crate::project::Project>>,
    user_file: Option<Res<UserStyleFile>>,
    mut watch: ResMut<StyleWatch>,
    mut targets: StyleTargets,
    mut reloads: Option<ResMut<crate::live::EditorReloads>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
    mut commands: Commands,
) {
    let theme = settings.as_ref().map_or(Theme::default(), |s| s.theme);
    let source = settings.as_ref().and_then(|s| s.editor_source.clone()).or_else(detected_checkout);
    publish_source(source.clone());
    let watch = &mut *watch;
    let mut found: Vec<(String, Found)> = Vec::new();
    // Aim each layer at its file (or its compiled text); a layer aimed somewhere new is decided at once.
    let base_path = source.as_ref().map(|root| root.join(STYLE_DIR).join(BASE_FILE));
    let base_name = base_path.as_ref().map_or_else(|| "style/editor.yaml".to_owned(), |p| shown(p, source.as_deref()));
    let theme_file = theme_layer(theme);
    let theme_path = theme_file.and_then(|(file, _)| source.as_ref().map(|root| root.join(STYLE_DIR).join(file)));
    let theme_name = match (&theme_path, theme_file) {
        (Some(p), _) => shown(p, source.as_deref()),
        (None, Some((file, _))) => format!("style/{file}"),
        (None, None) => String::new(),
    };
    let user_path = user_file.as_ref().map(|f| f.0.clone());
    let user_name = user_path.as_ref().map_or_else(String::new, |p| p.to_string_lossy().replace('\\', "/"));
    let project_path = project.as_ref().map(|p| p.project_dir.join(PROJECT_FILE));
    let project_name = match (&project, &project_path) {
        (Some(project), Some(path)) => crate::map_files::file_name(project, path),
        _ => String::new(),
    };
    let aims: [(&mut Look, Option<PathBuf>, String, Option<&str>); 4] = [
        (&mut watch.base, base_path, base_name, Some(BASE_TEXT)),
        (&mut watch.theme, theme_path, theme_name, theme_file.map(|(_, text)| text)),
        (&mut watch.user, user_path, user_name, None),
        (&mut watch.project, project_path, project_name, None),
    ];
    for (look, path, name, compiled) in aims {
        let was_named = !look.name.is_empty();
        let old_name = look.name.clone();
        if let Some(f) = look.aim(path, name, compiled) {
            found.push((look.name.clone(), f));
        }
        // A layer that names another file now: the old one's verdict is not this file's.
        if look.name != old_name
            && was_named
            && let Some(reloads) = reloads.as_deref_mut()
        {
            reloads.forget_file(KIND, &old_name);
        }
    }
    let heard = news.heard();
    for look in [&mut watch.base, &mut watch.theme, &mut watch.user, &mut watch.project] {
        if news.moved(&heard, look)
            && let Some(f) = look.look()
        {
            found.push((look.name.clone(), f));
        }
    }
    let theme_moved = watch.theme_now != Some(theme);
    watch.theme_now = Some(theme);
    for (name, f) in &found {
        let verdict = match f {
            Found::Applied => KindVerdict::applied(KIND, name.clone()),
            Found::Gone => {
                if let Some(reloads) = reloads.as_deref_mut() {
                    reloads.forget_file(KIND, name);
                }
                continue;
            }
            Found::Refused(refusal) => {
                KindVerdict::refused(KIND, name.clone(), refusal.field.clone(), format!("{}; the last good style stays", refusal.reason))
            }
        };
        if verdict.outcome != blueprint_model::live::Outcome::Applied
            && let Some(state) = state.as_deref_mut()
        {
            state.status.problem(format!("{}: refused", verdict.sentence()));
        }
        if let Some(reloads) = reloads.as_deref_mut() {
            reloads.decide(vec![verdict]);
        }
    }
    // **The person's text size is the skin's on every path**, settled or not: the sheet's own `text_scale` is
    // only where a layer would put it, never what is drawn.
    let text_scale = settings.as_ref().map(|s| s.text_scale);
    if let Some(scale) = text_scale
        && (targets.skin.text_scale - scale).abs() >= f32::EPSILON
    {
        targets.skin.text_scale = scale;
    }
    // **And the code editor's hues are the person's choice on every path** (#525), for the same reason and by the
    // same shape as `text_scale`: the sheet's `skin.syntax_theme` is where a layer would put it, and a setting chosen
    // on the Settings screen is not overridden by a project's file. Every `Role::CodeSyntax` run recolours in this
    // frame — gooey restyles every widget whose skin changed — so an open editor moves without a rebuild.
    let syntax_theme = settings.as_ref().map(|s| s.syntax_theme);
    if let Some(chosen) = syntax_theme
        && targets.skin.syntax_theme != chosen.theme()
    {
        targets.skin.syntax_theme = chosen.theme();
    }
    if found.is_empty() && !theme_moved && watch.applied.is_some() {
        return;
    }
    // **No base, no sum**: a base sheet that has never read (a checkout's file broken before the editor
    // started) leaves the running style as it is — the compiled one `StylePlugin` put in — and its refusal
    // says so. Merging the other layers onto nothing would draw gooey's defaults, which no layer asked for.
    if watch.base.good.is_none() {
        return;
    }
    let layers: Vec<&StyleLayer> =
        [&watch.base, &watch.theme, &watch.user, &watch.project].into_iter().filter_map(|look| look.good.as_ref()).collect();
    let style = match merge(&layers) {
        Ok(style) => style,
        Err(reason) => {
            let verdict = KindVerdict::refused(KIND, "merged", None, format!("{reason}; the last good style stays"));
            if let Some(state) = state.as_deref_mut() {
                state.status.problem(format!("{}: refused", verdict.sentence()));
            }
            if let Some(reloads) = reloads.as_deref_mut() {
                reloads.decide(vec![verdict]);
            }
            return;
        }
    };
    // **Only what moved since the sheet last applied is written**, and before the first application that is the
    // compiled sheet `StylePlugin` put in. So a sheet that says what the binary says writes nothing, and a skin
    // something else installed (a test's, a preview's) stands until a layer changes — the rule the theme
    // follower kept before the sheet existed.
    let previous = watch.applied.clone().unwrap_or_else(compiled_default);
    if previous.skin != style.skin {
        publish_spacing(style.skin.spacing);
        let mut skin = style.skin.clone();
        if let Some(scale) = text_scale {
            skin.text_scale = scale;
        }
        // **A sheet's `skin.syntax_theme` is not the person's choice** (#525): the setting is the one answer, as `text_scale` is.
        if let Some(chosen) = syntax_theme {
            skin.syntax_theme = chosen.theme();
        }
        if *targets.skin != skin {
            *targets.skin = skin;
        }
    }
    if previous.metrics != style.metrics && metrics() != style.metrics {
        let was = swap_metrics(style.metrics);
        targets.revision.metrics_revision += 1;
        if was.layout != style.metrics.layout || was.z != style.metrics.z {
            targets.rebuild.write(RebuildPanels);
        }
    }
    if previous.rules != style.rules {
        match targets.sheet.as_deref_mut() {
            Some(sheet) => {
                sheet.rules = style.rules.clone();
                sheet.revision += 1;
                targets.revision.sheet_revision = sheet.revision;
            }
            None => {
                commands.insert_resource(StyleSheet { rules: style.rules.clone(), revision: 1 });
                targets.revision.sheet_revision = 1;
            }
        }
    }
    let paths: BTreeMap<String, PathBuf> = [&watch.base, &watch.theme, &watch.user, &watch.project]
        .into_iter()
        .filter_map(|look| look.path.clone().map(|path| (look.name.clone(), path)))
        .collect();
    let lines = StyleLines { lines: style.lines.clone(), paths };
    if *targets.lines != lines {
        *targets.lines = lines;
    }
    watch.applied = Some(style);
}

/// A checkout's file as a refusal names it: relative to the checkout, so `crates/.../style/editor.yaml`.
fn shown(path: &Path, root: Option<&Path>) -> String {
    root.and_then(|root| path.strip_prefix(root).ok()).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

/// **The rules' own problems** (a tone, a host paint or a surface step that is not there), as one
/// `editor_style` verdict on the file `rules` and a line on the status band each, said when they change.
pub fn report_rule_problems(
    problems: Option<Res<StyleSheetProblems>>,
    mut watch: ResMut<StyleWatch>,
    mut reloads: Option<ResMut<crate::live::EditorReloads>>,
    mut state: Option<ResMut<crate::editor::EditorState>>,
) {
    let Some(problems) = problems else { return };
    if !problems.is_changed() || watch.problems == problems.0 {
        return;
    }
    watch.problems = problems.0.clone();
    let verdict = if problems.0.is_empty() {
        KindVerdict::applied(KIND, "rules")
    } else {
        if let Some(state) = state.as_deref_mut() {
            for problem in &problems.0 {
                state.status.problem(format!("style rule {problem}"));
            }
        }
        KindVerdict::refused(KIND, "rules", None, format!("{}; those properties keep the node's own value", problems.0.join("; ")))
    };
    if let Some(reloads) = reloads.as_deref_mut() {
        reloads.decide(vec![verdict]);
    }
}

/// **The editor's inks under their names, for a rule to paint with** (`background: !Host row_hover`): gooey's
/// [`bevy_gooey::HostPaints`], derived from the running skin and the sheet's ink steps by the same
/// [`crate::chrome::Ink::of`] the chrome paints with, so a rule's `row_hover` is the chrome's. Written only when
/// a name's colour moved, because gooey's cascade restyles every classed node when the paints change.
pub fn publish_host_paints(skin: Option<Res<GooeySkin>>, revision: Res<StyleRevision>, mut paints: ResMut<bevy_gooey::HostPaints>) {
    let Some(skin) = skin else { return };
    if !skin.is_changed() && !revision.is_changed() && !paints.0.is_empty() {
        return;
    }
    let ink = crate::chrome::Ink::of(&skin, &metrics().ink);
    let next: BTreeMap<String, Color> = crate::chrome::INK_NAMES.iter().map(|name| (*name).to_owned()).zip(ink.fields()).collect();
    if paints.0 != next {
        paints.0 = next;
    }
}

/// **The live style sheet**: follows the layers' files through the file indexes and applies their sum before
/// gooey's cascade.
pub struct StylePlugin;

impl Plugin for StylePlugin {
    fn build(&self, app: &mut App) {
        project_index::ensure(app);
        project_index::ensure_outside(app);
        let style = compiled_default();
        app.add_message::<RebuildPanels>()
            .init_resource::<StyleWatch>()
            .init_resource::<StyleRevision>()
            .init_resource::<StyleLines>()
            .init_resource::<bevy_gooey::HostPaints>()
            .insert_resource(StyleSheet { rules: style.rules, revision: 0 })
            .add_systems(
                Update,
                (follow_the_style_files, apply_style, publish_host_paints, report_rule_problems)
                    .chain()
                    .after(crate::settings::sync_settings)
                    .after(crate::ui_skin::skin_scope)
                    .before(bevy_gooey::GooeySystems::Cascade),
            );
    }
}
