//! **The editor's shared furniture** — one palette, one panel, one row shape.
//!
//! Every tab is the same two shapes: a controls panel down one side, and a list down the other. They
//! were written twice and were already drifting. The census key-row block appeared in `editor.rs` and
//! `tiles.rs` byte-for-byte, *including its five-line comment about `min_width`*, and rendered in two
//! different pairs of colours — the same list, a shade darker in one tab, which nobody chose. The
//! colour table itself was declared twice, with two of its entries carrying the same value under two
//! names (`TEXT_DIM`/`DIM`, `ROW_ARMED`/`ROW_SELECTED`), so a change to one tab's greys silently was
//! not a change to the other's.
//!
//! That is the failure the key census records for bindings, in a second place: a fact stated
//! more than once drifts. Ousterhout's *A Philosophy of Software Design* ch. 17 is the general form —
//! *"once you have learned how something is done in one place, you can use that knowledge to
//! immediately understand other places that use the same approach."* Here that is literal: a third tab
//! costs ~30 lines of panel code instead of ~110, and it cannot come out looking like a different
//! program.
//!
//! Nothing here knows what a map or a tile is. It knows what a panel looks like.

use bevy::picking::hover::Hovered;
use bevy::prelude::*;
use bevy::ui_widgets::ScrollArea;

use crate::keys::{self};

// ── the elevation ladder ─────────────────────────────────────────────────────────────────────────
//
// **A ground is a step on a ladder, and the step is measured.** The 2026-09-03 audit's finding was
// that this palette separated *ink from ground* superbly and *ground from ground* not at all:
// `PANEL_BG` stood 1.03:1 against `VOID`, `ROW_BG` 1.08:1 against `PANEL_BG`, and hover 1.19:1
// against a row — with no border and no radius to carry the boundary instead. Reported at the
// keyboard as *"the layout overall is muddy as hell"*, which is exactly what a screen looks like
// when every surface edge is below the just-noticeable difference.
//
// **WCAG's contrast ratio is the wrong instrument down here** and using it is how the old ladder
// passed review: the `+0.05` flare term dominates at near-black, so two clearly different darks and
// two identical darks both score about 1.0. The right instrument for a large flat field is CIE
// **L\***, where a just-noticeable difference is about 2 and a comfortable one about 4. Measured on
// the shipped ladder, `VOID`→`PANEL_BG` was **ΔL\* 1.60** — below the JND. Every adjacent pair below
// is **ΔL\* ≥ 2.95**, and `the_ladder_is_a_ladder` fails the build if one closes up again.
//
// Contrast ratio is still the right instrument for *text*, and every ink keeps ≥ 4.5:1 against the
// grounds it actually renders on — `the_ink_clears_its_grounds` names the pairing per ink, because
// "every ink on every ground" is a rule this palette cannot satisfy and should not pretend to.
//
//    VOID        L*  5.4   the window behind everything
//    PANEL_BG    L* 10.0   a panel's own ground              ΔL* 4.58
//    HEADER_BG   L* 13.0   a group heading band              ΔL* 2.95
//    ROW_BG      L* 15.9   a row, a chip, a field at rest    ΔL* 5.84 over PANEL_BG
//    SLOT_BG     L* 18.1   an inspector slot, a thumbnail
//    OVERLAY_BG  L* 19.2   a modal card, above the scrim
//    ROW_HOVER   L* 21.1   under the pointer                 ΔL* 5.25 over ROW_BG
//    ROW_SELECTED L* 25.0  the one being acted on            ΔL* 3.89 over ROW_HOVER
//    PANEL_EDGE  L* 33.1   the hairline that ends a surface
//
// Two grounds run the other way on purpose: `ROW_PRESSED` and `FOCUS_BG` are *recessed*, below the
// surface they sit in, because pushed-in and typed-into are the two states where the metaphor is a
// well rather than a card.

/// **The editor's own ink, and the only place a colour is named** (UI-02, #210; follows the theme, #428).
///
/// This module used to declare 28 `pub const … : Color` — a table the 2026-08-17 audit had already
/// caught drifting against itself twice. They are gone: every colour the chrome paints is a field
/// here, and every field is read from **the running skin** ([`Ink::of`]) — the same data `bevy_gooey`
/// restyles the widgets from and the same data `GooeySkin::problems()` measures. One palette, one
/// skin, one check. The skin is the style sheet's (`crate::style`): `src/style/editor.yaml`, with a
/// theme's `src/style/theme_*.yaml` laid over it.
///
/// # Why a struct, and why [`ink`] is a function of the skin and not a parameter
///
/// The chrome is a body of spawn systems and helpers that hold no `Res<GooeySkin>`, and threading a
/// skin through all of them would be a second parameter on every call. So the ink is one live value
/// ([`ink`]), rebuilt by `crate::theme_ink::follow_the_skin` whenever the running skin changes colour,
/// and the same system moves everything already on screen onto the new one: **a theme changes every
/// colour the editor draws, live** (#428). A project's skin never reaches it (UI-03, #211 — a game's
/// palette belongs inside the Game UI preview panel and nowhere else): only the running skin does.
///
/// # The tone names are the file's keys
///
/// Twelve of these have no `GooeySkin` field — `key`, `label`, `muted`, `suggest`, `labeled`,
/// `grid_line`, `bounds_line`, `veil`, `focus_bg`, `row_pressed`, `snip_wash`, `overlay_bg` — and
/// live in the skin's `tones` map (dark-incorporated/bevy_gooey#33). A tone the skin does not carry is
/// answered in magenta, this module's convention for a colour that cannot be derived
/// ([`scaled`], [`ink`]); for the editor's own skin it cannot happen, and
/// `tests/the_skin_carries_the_editors_other_colours.rs` holds all twelve in place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ink {
    /// A panel's ground — the skin's `surface`.
    pub panel_bg: Color,
    /// A row, a chip, a field at rest — the skin's `surface_raised`.
    pub row_bg: Color,
    /// A row under the pointer — the skin's `hover`.
    pub row_hover: Color,
    /// A row that is the one being acted on — the ladder's seventh step.
    pub row_selected: Color,
    /// A control with the pointer down on it — the skin's `pressed`, recessed below [`Ink::row_bg`].
    pub row_pressed: Color,
    /// Body text: a value, an id, the thing that changes.
    pub text: Color,
    /// Quieter body text — the skin's `text_muted`.
    pub dim: Color,
    /// **The one live-edit colour**: a value being changed right now, and nothing else.
    pub accent: Color,
    /// The key column, brighter than the description beside it.
    pub key: Color,
    /// A label column, quieter than its value.
    pub label: Color,
    /// Destructive, and only destructive.
    pub danger: Color,
    /// The scrim behind a modal — black at 55%, so the question is the only lit thing.
    pub scrim: Color,
    /// The veil the shortcut badges are read through — lighter than the scrim on purpose.
    pub veil: Color,
    /// A row that exists on purpose and is deliberately not participating.
    pub muted: Color,
    /// Machine-proposed, human-unconfirmed — a cool slate, neither accent nor danger.
    pub suggest: Color,
    /// [`Ink::suggest`] one state later, lifted toward white so it reads on a chosen row.
    pub unchecked: Color,
    /// A mesh that has been judged — the one state that means *ready*.
    pub labeled: Color,
    /// An inspector slot, an empty thumbnail — the ladder's fifth step.
    pub slot_bg: Color,
    /// A modal card's ground, above the scrim — the skin's `card`.
    pub overlay_bg: Color,
    /// A text box that owns the keyboard: recessed below [`Ink::panel_bg`].
    pub focus_bg: Color,
    /// A group heading band — the ladder's third step.
    pub header_bg: Color,
    /// The working grid: the map's cell lattice and the seating lattice.
    pub grid_line: Color,
    /// A stated extent, brighter than the grid inside it.
    pub bounds_line: Color,
    /// The void the panels float over — the clear colour both entry points install.
    pub void: Color,
    /// The window's own chrome: the bar above the door strip and the band at its foot.
    pub bar_bg: Color,
    /// The hairline that ends any surface.
    pub panel_edge: Color,
    /// A louder edge, for the surface that owns the keyboard.
    pub edge_strong: Color,
    /// The snip's translucent wash over its rectangle.
    pub snip_wash: Color,
}

impl Ink {
    /// **The ink as a skin says it.** Every field is one of the skin's own roles or tones, except
    /// the two that are *derived* and say so: [`Ink::bar_bg`] from a slot's ground and
    /// [`Ink::unchecked`] from the suggest tone. **How** they are derived, and which step of the skin's
    /// ladder a row, a slot, a heading band and an edge stand on, is the style sheet's `metrics.ink`
    /// (`steps`): the ladder is `background, surface, header, raised, slot, hover, selected, edge`,
    /// lowest first.
    pub fn of(skin: &bevy_gooey::GooeySkin, steps: &crate::style::InkMetrics) -> Ink {
        let tone = |name: &str| skin.tone(name).unwrap_or_else(|| {
            // **Magenta, loudly.** This module answers a colour it cannot derive in fuchsia rather
            // than with a silent grey ([`scaled`], [`ink`]); a skin missing one of the editor's tones
            // is exactly that case. The editor's own skin carries all twelve, held by
            // `tests/the_skin_carries_the_editors_other_colours.rs`.
            Color::srgb(1.0, 0.0, 1.0)
        });
        let slot_bg = skin.surface_at(steps.slot_step);
        let suggest = tone("suggest");
        Ink {
            panel_bg: skin.surface,
            row_bg: skin.surface_raised,
            row_hover: skin.hovered(),
            row_selected: skin.surface_at(steps.row_selected_step),
            row_pressed: skin.pressed(),
            text: skin.text,
            dim: skin.text_muted,
            accent: skin.accent,
            key: tone("key"),
            label: tone("label"),
            danger: skin.danger,
            scrim: skin.scrim,
            veil: tone("veil"),
            muted: tone("muted"),
            suggest,
            unchecked: scaled(suggest, steps.unchecked_lift),
            labeled: tone("labeled"),
            slot_bg,
            overlay_bg: skin.card(),
            focus_bg: tone("focus_bg"),
            header_bg: skin.surface_at(steps.header_step),
            grid_line: tone(GRID_TONE),
            bounds_line: tone("bounds_line"),
            void: skin.background,
            bar_bg: scaled(slot_bg, steps.bar_step_back),
            panel_edge: skin.surface_at(steps.edge_step),
            edge_strong: skin.outline,
            snip_wash: tone("snip_wash"),
        }
    }

    /// **The ink the editor starts with**, from the compiled style sheet (`src/style/editor.yaml`) — the
    /// Tool dark theme, and the value [`ink`] holds until a skin says otherwise.
    pub fn editor() -> Ink {
        let style = crate::style::compiled_default();
        Ink::of(&style.skin, &style.metrics.ink)
    }

    /// **Every field, in declaration order**, so a theme change can say "the colour that was `panel_bg`
    /// is now this theme's `panel_bg`" ([`Ink::remap`]) without naming 28 fields twice. The pattern has
    /// no `..`, so a field added to [`Ink`] fails to compile here until it is listed.
    pub fn fields(&self) -> [Color; INK_FIELDS] {
        let Ink {
            panel_bg, row_bg, row_hover, row_selected, row_pressed, text, dim, accent, key, label, danger, scrim, veil, muted, suggest,
            unchecked, labeled, slot_bg, overlay_bg, focus_bg, header_bg, grid_line, bounds_line, void, bar_bg, panel_edge, edge_strong,
            snip_wash,
        } = *self;
        [
            panel_bg, row_bg, row_hover, row_selected, row_pressed, text, dim, accent, key, label, danger, scrim, veil, muted, suggest,
            unchecked, labeled, slot_bg, overlay_bg, focus_bg, header_bg, grid_line, bounds_line, void, bar_bg, panel_edge, edge_strong,
            snip_wash,
        ]
    }

    /// **What a colour drawn from `from` becomes when the ink is `to`** (#428), or `None` when it was
    /// not drawn from `from` at all.
    ///
    /// A colour that *is* a field of `from` takes the same field of `to`, alpha and all. One that has the
    /// field's colour at another alpha (a fade, the shortcut key's step back) keeps its own alpha and
    /// takes the new field's colour. Anything else — a measured colour, a game's, a terminal program's, a
    /// fully transparent one — is not the editor's ink and is left alone, which is the point of asking.
    ///
    /// **It matches by value, so the caller chooses what to ask about.** A component whose default colour is
    /// white (an `ImageNode`'s tint) cannot be told from High contrast's white `text`, which is why
    /// `crate::theme_ink` does not ask about images: their tint belongs to the picture, not to the theme.
    pub fn remap(from: &Ink, to: &Ink, colour: Color) -> Option<Color> {
        let (from, to) = (from.fields(), to.fields());
        let same = |a: Color, b: Color| {
            let (a, b) = (Srgba::from(a), Srgba::from(b));
            [a.red - b.red, a.green - b.green, a.blue - b.blue].iter().all(|d| d.abs() <= INK_MATCH_TOLERANCE)
        };
        let alpha = Srgba::from(colour).alpha;
        // **Nothing is drawn at alpha 0**, so it is no ink's: `Color::NONE` has the RGB of every black field
        // (`scrim`, High contrast's `void`) and must stay `NONE` through a theme change.
        if alpha <= 0.0 {
            return None;
        }
        let exact = from.iter().position(|f| same(*f, colour) && (Srgba::from(*f).alpha - alpha).abs() <= INK_MATCH_TOLERANCE);
        if let Some(at) = exact {
            return Some(to[at]);
        }
        let at = from.iter().position(|f| same(*f, colour))?;
        Some(to[at].with_alpha(alpha))
    }
}

/// How many colours an [`Ink`] holds.
pub const INK_FIELDS: usize = 28;

/// **Each [`Ink`] field's name, in [`Ink::fields`] order**: what a style sheet rule paints with
/// (`background: !Host row_hover`), published by `crate::style` as gooey's `HostPaints`.
pub const INK_NAMES: [&str; INK_FIELDS] = [
    "panel_bg", "row_bg", "row_hover", "row_selected", "row_pressed", "text", "dim", "accent", "key", "label", "danger", "scrim", "veil", "muted",
    "suggest", "unchecked", "labeled", "slot_bg", "overlay_bg", "focus_bg", "header_bg", "grid_line", "bounds_line", "void", "bar_bg",
    "panel_edge", "edge_strong", "snip_wash",
];

/// **How near two sRGB channels must be to be the same ink**: a thousandth, well under one 8-bit step
/// (1/255), so a colour that went through a `Color` conversion still matches and two inks that differ
/// by a step do not.
const INK_MATCH_TOLERANCE: f32 = 1.0e-3;

/// **The ink the editor is drawing with right now** — the running skin's, by [`Ink::of`] (#428).
///
/// There is one, and `crate::theme_ink::follow_the_skin` is the only thing that writes it: whenever the
/// running [`bevy_gooey::GooeySkin`] changes colour (a theme chosen in Settings, a settings file edited
/// by hand) the ink is rebuilt from it and everything already drawn is moved onto it. Until a skin says
/// otherwise it is the editor's own style sheet's, `src/style/editor.yaml`.
///
/// It is a process-wide value rather than a resource because painters call [`ink`] where no system
/// parameter reaches — the same reason [`text_scale`] is one atomic. A process has one editor.
static LIVE_INK: std::sync::LazyLock<std::sync::RwLock<Ink>> = std::sync::LazyLock::new(|| std::sync::RwLock::new(Ink::editor()));

/// **The editor's ink now**, by value (28 colours, `Copy`): what a painter reads, whatever theme is
/// running. A value and not a reference, because it changes under a running editor.
pub fn ink() -> Ink {
    // A poisoned lock still holds a whole `Ink`: it is only ever replaced, never edited in place.
    *LIVE_INK.read().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Replaces the editor's ink and returns the one it replaced. Only `crate::theme_ink` calls it.
pub(crate) fn swap_ink(next: Ink) -> Ink {
    std::mem::replace(&mut *LIVE_INK.write().unwrap_or_else(std::sync::PoisonError::into_inner), next)
}

/// A world gizmo's colour, faded while the shortcut key is held, to the sheet's
/// `metrics.layout.world_held` alpha: deeper than a ground's `ground_held`, because a panel's fill is a
/// flat wash a badge sits on and the world's envelope is a *line* crossing the same ground a badge stands
/// on (*"the tiles tab is super muddy"*). A fade and not a hide: the envelope is what the author is
/// building.
///
/// A free function rather than a resource because the drawers are ordinary `Update` systems with no
/// stated order against anything in `badges`, and a resource written in one phase and read in
/// another would be a frame stale on some runs and not others — the trap `badges::ink`'s own
/// header records paying for.
pub fn stepped_back(color: Color, held: bool) -> Color {
    if held {
        color.with_alpha(color.alpha() * crate::style::metrics().layout.world_held)
    } else {
        color
    }
}

// ── world ink ────────────────────────────────────────────────────────────────────────────────────
//
// Gizmo colours the tabs draw INTO the scene — the exception to "nothing here knows what a map is",
// carried because each is a fact two tabs state: a colour defined per-tab drifted into two names
// for one value twice before this block existed (the audit's `BOUNDS_LINE == CELLS` finding).

// (`BAR_EDGE` was here until 2026-09-03. It was `ROW_SELECTED × 0.62` — an edge derived from a row
// fill, which stopped making sense the moment every surface got an edge: a chrome bar's hairline and
// a panel's hairline are one fact, and this module's whole argument is that a fact stated twice
// drifts. Its three call sites take `PANEL_EDGE`.)

// ── edges and corners ────────────────────────────────────────────────────────────────────────────
//
// **The other half of the elevation model.** A fill step of ΔL* 4 is a comfortable separation and it
// is still only a change of shade; what makes a surface read as an *object* is that it has an edge
// and the edge turns a corner. Before 2026-09-03 this crate had four `BorderRadius` in 58,000 lines
// (two of them the compass) and no panel border at all, so a panel was a rectangle of very slightly
// different paint. Gestalt closure does the work a fill cannot: an outline groups its contents
// whatever their contrast, which is why a bordered panel survives a bright thumbnail landing in it
// and a fill-only panel does not.

/// **Which panel the keyboard is in**, as an edge rather than as a second selected row.
///
/// Asked for 2026-09-03: in the chooser nothing said which of the three columns the arrows were
/// walking, so `↑` was a guess until it moved something. A focused *row* already exists and cannot
/// answer this — every column has one, all the time. The panel's own edge can, and it costs no
/// layout: the border is already there at `Ink::panel_edge`, and focus lights it.
pub fn focus_edge(focused: bool) -> Color {
    if focused { ink().accent } else { ink().panel_edge }
}

/// **The lattice an author reads a map by, as the running skin colours it** (UI-80, #254).
///
/// The map's grid and the kit door's seating lattice are the editor's two grids, and both used to
/// read `chrome::ink().grid_line` whatever skin was running. This is the one expression they share:
/// the skin's own `grid_line` tone where it carries one, the shipped tone where it does not, so the
/// editor's look is unchanged by construction and a theme or a project that names `grid_line` moves
/// both lattices together. A test reads it, which is why it is a function and not a line inside a
/// `Gizmos` call — a headless app renders no gizmos, so nothing else can ask what colour they are.
pub fn grid_ink(skin: Option<&bevy_gooey::GooeySkin>) -> Color {
    skin.and_then(|skin| skin.tone(GRID_TONE)).unwrap_or_else(|| ink().grid_line)
}

/// **The skin tone a lattice's colour is kept under.** One name for the two places it is read — the
/// lookup in [`grid_ink`] and the `Ink::grid_line` field's own read of it in `Ink::of` — so a rename in
/// the style sheet's `skin.tones` cannot leave one of them asking for a tone that no longer exists.
pub const GRID_TONE: &str = "grid_line";

/// **A wash at `share` of a named colour's own strength**: `Ink::snip_wash` at `0.5` is the quieter tint
/// a report draws over a pick it has already taken, so a person adding a second can tell which is the
/// decision already made and which is the one being made now.
///
/// Derived rather than declared, because a second colour constant beside the first is the duplicate
/// this table already had once (`the_colour_table_only_shrinks` counts them, and it only shrinks).
pub fn wash_at(colour: Color, share: f32) -> Color {
    match colour {
        Color::Srgba(srgba) => Color::srgba(srgba.red, srgba.green, srgba.blue, srgba.alpha * share),
        other => other,
    }
}

/// **A surface-pixel coordinate as the overlay's `Val::Px`.**
///
/// The layout multiplies `Val::Px` by [`bevy::ui::UiScale`], so an overlay drawing a
/// rectangle it was handed in surface (physical) pixels divides by it, as `compose.rs` places its own
/// rectangles. Two things draw such a rectangle — the snip's drag box and a feedback report's
/// outline — and they had a copy each before this was one function.
pub fn overlay_px(surface: f32, ui_scale: f32) -> f32 {
    let s = if ui_scale > 0.0 { ui_scale } else { 1.0 };
    surface / s
}

/// A chrome colour scaled toward black, for a quieter sibling of a named colour — so the derived
/// value cannot drift from its parent the way `compose`'s hand-halved ACCENT could have.
pub const fn scaled(colour: Color, k: f32) -> Color {
    match colour {
        Color::Srgba(c) => Color::srgb(c.red * k, c.green * k, c.blue * k),
        // Every chrome colour is authored as sRGB; anything else reaching here is a caller error,
        // answered loudly in magenta rather than with a silent guess.
        _ => Color::srgb(1.0, 0.0, 1.0),
    }
}

/// A chrome colour as raster bytes, for the CPU-drawn plots — one palette, two encodings, no
/// hand-transcribed mirror (the audit found three byte copies of chrome colours in `anim_plots`).
///
/// Named `ink_bytes` and not `ink`: [`ink`] is the palette itself, and a second `ink` that took a
/// colour as an argument was the one collision this rename cost.
pub const fn ink_bytes(colour: Color) -> [u8; 4] {
    match colour {
        Color::Srgba(c) => [
            (c.red * 255.0 + 0.5) as u8,
            (c.green * 255.0 + 0.5) as u8,
            (c.blue * 255.0 + 0.5) as u8,
            255,
        ],
        _ => [255, 0, 255, 255],
    }
}

// ── layout ───────────────────────────────────────────────────────────────────────────────────────
//
// **The editor's measures are the style sheet's** (`crate::style::LayoutMetrics`, `metrics.layout` in
// `src/style/editor.yaml`): the interface scale, the docks' preferred widths and their floors (B-014), the
// viewport's reserve and share (#282), the divider (#476), the bottom dock and every hairline. Their
// reasons are written beside each field there, where a person changing one reads them.

// ── the widget layer ─────────────────────────────────────────────────────────────────────────────

/// **`bevy_feathers`' machinery, with this editor's palette in it.**
///
/// Separate from [`ChromePlugin`] because `add_plugins` tuples cap at **15**
/// (`bevy_app-0.20.0/src/plugin.rs:186`) and `harness::add_editor_plugins` is near it — a plugin
/// that nests its own group keeps the shared list one list.
///
/// **`UiTheme` is inserted, never defaulted.** `UiTheme::default()` is empty and every token miss
/// renders fuchsia with a warning; a theme nobody seeded is a magenta editor, and no test would see
/// it because nothing here reads a colour back. [`theme`] is the seed and
/// `the_theme_is_seeded_from_the_palette` is what notices if it stops being.
pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::feathers::FeathersPlugins)
            .insert_resource(bevy::feathers::theme::UiTheme(theme()));
    }
}

// ── theme ────────────────────────────────────────────────────────────────────────────────────────

/// **This editor's palette, expressed as a `bevy_feathers` theme.**
///
/// # Machinery, not greys — and that reconciliation is written down here because nowhere else has it
///
/// Two rules stand about `bevy_feathers`: *"its visuals are Bevy's editor skin — do not adopt
/// them"*, and *"Do not add a UI framework crate"*. What is adopted is the **mechanism**
/// — a token table, a themed scrollbar, and focus outlines that would otherwise be hand-written
/// badly — and every colour this editor states comes from the constants above.
///
/// **What is NOT true, and was written here as if it were:** Feathers' greys are not overwritten
/// wholesale. `create_dark_theme()` populates 137 tokens and the table below replaces 14, so 123 of
/// Bevy's editor greys are live in the resource. Nothing in this crate spawns a Feathers-themed
/// widget today, so none of them reaches the screen — but that is a fact about the call sites, not
/// about this function, and the first themed widget anybody adds will render in somebody else's
/// skin. Seeding from `ThemeProps::default()` instead is what would make the paragraph above true;
/// it is a decision about what a missing token should look like, not a typo, so it is named here
/// rather than taken.
///
/// The editor already deviates from that section once, deliberately and on the record (the full font
/// face, where the game keeps Bevy's 95-codepoint default). This is the second, and the same shape:
/// take the machinery, keep the house's own decisions.
///
/// # Why this is not a fifth dialect
///
/// The 2026-08-17 audit's verdict is that four tabs were drifting into four dialects, and its remedy
/// is *one name per fact*. A token table that **restated** a colour — `srgb(0.098, 0.092, 0.082)`
/// beside `Ink::row_bg` — would be exactly the leak it measured, one indirection later. So every entry
/// below is a reference to a const, never a literal, and `tests/chrome_census.rs` fails on a literal
/// in this file's theme.
///
/// `UiTheme::color` renders **fuchsia** and warns once for a token it has no entry for — the house
/// convention, which [`scaled`] and [`ink`] also keep (*"loudly in magenta rather than with a silent
/// guess"*). **That alarm cannot currently fire here**, and the difference matters: seeding from
/// `create_dark_theme()` gives every token an entry, so `color`'s `None` arm is unreachable and a
/// token this table misses answers with Bevy's grey rather than shouting. The safety property this
/// paragraph used to claim is the one thing the seeding gives up.
pub fn theme() -> bevy::feathers::theme::ThemeProps {
    use bevy::feathers::theme::{SemanticToken, ThemeToken};
    use bevy::feathers::tokens;
    let mut props = bevy::feathers::dark_theme::create_dark_theme();
    // **Bevy 0.20 split a token from its colour.** A design token now maps to a `SemanticToken`
    // (`token_assignments`), and the semantic token maps to a colour, per surface level
    // (`semantic_base`, `semantic_overrides`). This editor has no surface levels, so each token it
    // restates gets a semantic token of its own — one colour, whatever the context — which is the
    // 0.20 theme migration's quick path.
    let mut assign = |token: ThemeToken, color: Color| {
        let semantic = SemanticToken::new(format!("bevy_dark_factory.{token}").into());
        props.token_assignments.insert(token, semantic.clone());
        props.semantic_base.insert(semantic, color);
    };
    assign(tokens::WINDOW_BG, ink().void);
    assign(tokens::PANE_BODY_BG, ink().panel_bg);
    assign(tokens::PANE_HEADER_BG, ink().header_bg);
    assign(tokens::SUBPANE_BODY_BG, ink().slot_bg);
    assign(tokens::SUBPANE_HEADER_BG, ink().header_bg);
    assign(tokens::TEXT_MAIN, ink().text);
    assign(tokens::TEXT_DIM, ink().dim);
    // The focus ring is the one thing Feathers gives that this editor has never had. `ACCENT` at
    // half alpha, because a ring is a *state* and the audit's rule is that persistent signals sit
    // at medium contrast with the loud end held in reserve.
    assign(tokens::FOCUS_RING, ink().accent.with_alpha(0.5));
    assign(tokens::SCROLLBAR_BG, ink().header_bg);
    assign(tokens::SCROLLBAR_THUMB, scaled(ink().row_selected, 0.8));
    assign(tokens::SCROLLBAR_THUMB_HOVER, ink().row_selected);
    assign(tokens::LISTROW_BG, ink().row_bg);
    assign(tokens::LISTROW_BG_HOVER, ink().row_hover);
    assign(tokens::LISTROW_BG_SELECTED, ink().row_selected);
    props
}

// ── type ─────────────────────────────────────────────────────────────────────────────────────────

/// **The type scale, as roles rather than numbers.**
///
/// The palette got named constants and the spacing got a scale; **size never got a name**, and the
/// 2026-08-17 audit (the owner's, at the keyboard on mac_air) measured what that cost: six sizes
/// across a hundred-odd call sites with no rule between them. Section headings were 9 (`chrome::section`), 10 (the editor's and anim's
/// hand-rolled ones) or 11 (Compose's three) — *three dialects for one role in one editor*. Compose
/// showed two "COMPOSITIONS" headings, the real `section` plus an 11 px twin. Label/value pairs were
/// 10/11, 10/10 or flat 11 depending on the tab. And the anim bench rendered its central pairing
/// **inverted** — declared at 10 over measured at 9 — so the number the author is checking was the
/// smaller of the two.
///
/// A role is not the same thing as a distinct size, and two roles sharing a value is not a defect:
/// [`text::HEADING`] and [`text::LABEL`] are both 10 because both are quiet supporting text, and what separates
/// a heading from a label is that it is uppercase, in a different ink, and preceded by
/// [`STEP_GROUP`]. `section`'s own doc says so — *"quiet, and separated from what came before it…
/// the separation is the work"* — which is why the fix for three heading dialects is the smallest
/// of them and not the largest. A scale where headings out-shouted values would be a different
/// editor, and nobody asked for that one.
///
/// **What actually moved**, so a capture diff can be read rather than guessed at: `section` 9 → 10,
/// Compose's three hand-made headings 11 → 10, the anim pair un-inverted, and the name box's 18 →
/// [`text::TITLE`]. Everything else keeps the size it had and gains a name.
pub mod text {
    /// **A type role, and the reason it is not an `f32`.**
    ///
    /// The 2026-09-03 audit found the size ratchet had a laundering hole: `chrome_census.rs` scans
    /// for `from_font_size(<digit>`, and `chip`/`text_field` took `px: f32` and called
    /// `from_font_size` *inside* `chrome.rs`, which the scan skips. Twenty call sites were passing
    /// bare `9.0` / `10.0` / `11.0` through a function argument, in a crate whose own test claims a
    /// size is *"a role, never a number"*.
    ///
    /// A regex cannot close that; a type can. A builder takes a [`Role`], a `Role` is only ever one
    /// of the constants below, and a bare number at a call site is a compile error rather than a
    /// finding in the next audit.
    ///
    /// **The size is the style sheet's** (`metrics.type` in `src/style/editor.yaml`): a role names
    /// which size, and [`Role::px`] reads it from the running metrics, so an edit to the sheet
    /// resizes every label live ([`super::resize_the_labels`]).
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    pub struct Role(Size);

    /// Which of the sheet's type sizes a role is.
    #[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
    enum Size {
        Title,
        Tab,
        Body,
        Heading,
        Label,
        Control,
        Hint,
    }

    impl Role {
        /// The role's size in logical pixels at 100% text, from the running style sheet. For
        /// [`super::font`] and for measurement (`chars * body_char_w`); **not** a way back to
        /// writing numbers at a call site.
        pub fn px(self) -> f32 {
            let sizes = crate::style::metrics().r#type;
            match self.0 {
                Size::Title => sizes.title,
                Size::Tab => sizes.tab,
                Size::Body => sizes.body,
                Size::Heading => sizes.heading,
                Size::Label => sizes.label,
                Size::Control => sizes.control,
                Size::Hint => sizes.hint,
            }
        }
    }

    /// The panel's own name — `BLUEPRINT MAPPER`, `MESHES AND TILES`. One per panel.
    pub const TITLE: Role = Role(Size::Title);
    /// A tab's word in the strip, and the chooser's column headers.
    pub const TAB: Role = Role(Size::Tab);
    /// **The readable default.** A row's value, a field's contents, the problem banner, a census
    /// row — anything the author is reading rather than orienting by. The chord on a badge is
    /// this size on purpose: it is the one thing a badge exists to have read.
    pub const BODY: Role = Role(Size::Body);
    /// A block or list heading. Quiet: uppercase, dim ink, and a group's gap above it.
    // **Eleven, not ten** (#518, owner 2026-11-21): a heading has to outrank the label it heads.
    // It was 10 beside `LABEL`'s 10 because both are "quiet supporting text", and that left the
    // hierarchy entirely in the case and the ink — which is what the owner's note was about. One
    // pixel is inside `the_type_scale_stays_short`'s seven-role cap and needs no new role.
    pub const HEADING: Role = Role(Size::Heading);
    /// The dim half of a label/value pair, and a log line.
    pub const LABEL: Role = Role(Size::Label);
    /// **A control's own word** — the text on a chip, a button, a field. Ten pixels, the same
    /// number as [`LABEL`], and a seventh role anyway.
    ///
    /// `the_type_scale_stays_short` says a seventh role *"is a decision somebody should have to make
    /// on purpose"*, so here is the purpose. The 2026-09-03 decision was **shape carries kind,
    /// colour carries severity**: a toggle and a command stopped being the same box. The word on a
    /// control is therefore a role in its own right — it is the only text in this interface that
    /// names an *action* rather than describing a value — and the twenty call sites that used to
    /// pass a bare `10.0` into `chip(px: f32)` had nothing else honest to say.
    pub const CONTROL: Role = Role(Size::Control);
    /// Badge descriptions, footnotes, and the always-on hint — the smallest thing on screen, and
    /// never the thing being read. (Badge *chords* moved up to [`BODY`]; the description beside a
    /// chord stays down here, quieter on both axes.)
    pub const HINT: Role = Role(Size::Hint);

    /// Every role, largest first.
    pub const ALL: [Role; 7] = [TITLE, TAB, BODY, HEADING, LABEL, CONTROL, HINT];
}

/// **What the chrome bar's readouts are drawn at** (#480, feedback-1794783606350: *"The debug text is overlayed on the
/// editor top menu like file, edit, view--we should make it distinct."*).
///
/// The bar is a band of **state** — where you are, which door, which decision model — one rule below a row of
/// **commands** (File, Edit, View). Drawn at [`text::BODY`], the way the menus are, the two were one texture and the
/// owner could not tell them apart. So the bar's words come down a step, and the band is quieter than the menu by
/// size as well as by ink.
///
/// **It names an existing role rather than adding one.** `chrome_census::the_type_scale_stays_short` caps the scale
/// at seven roles and five sizes, and a readout is not a kind of text this interface had no word for — it is a label
/// (a quiet line under a value it belongs to), which is exactly what [`text::LABEL`] is. The name is the decision, so
/// a word added to the bar later takes it by construction instead of by whoever remembers.
pub const BAR_READOUT: text::Role = text::LABEL;

/// **The only way this crate makes a `TextFont`.**
///
/// `TextFont::from_font_size` takes an `f32` and will therefore always accept a number; this takes a
/// [`text::Role`] and cannot. `the_type_scale_is_a_type` fails the build on a `from_font_size`
/// outside this module, which is the rule the old digit-matching regex was reaching for.
pub fn font(role: text::Role) -> TextFont {
    TextFont::from_font_size(role.px() * text_scale())
}

/// **The role a drawn label was made at, so a change of text size can redraw it** (UI-73, #251).
///
/// A `TextFont` is written once, when a widget is spawned. [`font`] reads the running
/// [`text_scale`] at that moment, so a label spawned after a person chose 175% is drawn at 175% —
/// and one that was **already on screen** keeps the size it was built at, which is exactly what the
/// review of #251 measured and what this component exists to fix.
///
/// The marker is the role, not the size: a role is a fact of the type scale and does not go stale,
/// and [`resize_the_labels`] can therefore recompute every label from `role.px() * scale` when the
/// person's multiple changes, without knowing what any of them says.
///
/// **Spawn through [`typed`]**, which puts this beside the `TextFont` in one bundle. A label that
/// calls [`font`] alone is drawn at the size it was made and never follows the setting.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct TypeRole(text::Role);

impl TypeRole {
    /// The role this label is drawn at.
    pub fn role(self) -> text::Role {
        self.0
    }
}

/// **A hand-drawn label's type: the `TextFont` it is drawn with *and* the role to redraw it at.**
///
/// The pair is one bundle because they are one fact — a label's size — stated twice on purpose: the
/// `TextFont` is what `bevy_text` lays out, and the [`TypeRole`] is what [`resize_the_labels`] reads
/// when the person's text size changes. Every `chrome::font` call site in this crate spawns through
/// this, so no label can carry the one without the other.
pub fn typed(role: text::Role) -> impl Bundle {
    (font(role), TypeRole(role))
}

/// **A label already on screen is redrawn when the text size changes** (UI-73, #251).
///
/// `chrome::font` reads [`text_scale`] when it builds a `TextFont`, so a label spawned after the
/// setting changed is already right; a label spawned before it is not, and a person who chose 175%
/// would have half an editor at 175% and the chrome they read most at 100%. This rewrites every
/// [`TypeRole`]'s `TextFont` from its role the frame the resolved setting differs from the one last
/// applied, and does nothing at all on a settled editor (one float comparison).
///
/// **It reads the resolved `EditorSettings`, not the atomic**: the atomic exists so a spawn closure
/// with no system parameters can read the scale, while this needs the value the settings file
/// resolved to and a place to remember what it last drew.
///
/// **A `TextSpan` spawned through [`typed`] is rewritten like any label**: [`typed`] gives it a `TypeRole` and a `TextFont`, which is what this
/// system queries. A span spawned with its own `TextFont` and no `TypeRole` is not. COMPOSE's armed mark is the first span here, and it
/// goes through [`typed`].
pub fn resize_the_labels(
    settings: Option<Res<crate::settings::EditorSettings>>,
    revision: Option<Res<crate::style::StyleRevision>>,
    mut applied: Local<Option<(f32, u64)>>,
    mut labels: Query<(&TypeRole, &mut TextFont)>,
) {
    let Some(settings) = settings else { return };
    // **The text size and the sheet's type scale are one key**: a size edited in the style sheet resizes every
    // label already drawn, the same way a person's text size does.
    let metrics = revision.map_or(0, |r| r.metrics_revision);
    if applied.is_some_and(|(last, seen)| (last - settings.text_scale).abs() < f32::EPSILON && seen == metrics) {
        return;
    }
    *applied = Some((settings.text_scale, metrics));
    let scale = settings.text_scale;
    for (role, mut font) in &mut labels {
        let want = FontSize::Px(role.0.px() * scale);
        if font.font_size != want {
            font.font_size = want;
        }
    }
}

/// **The person's text size, as every label the editor draws sees it** (UI-73, #251).
///
/// Every hand-drawn label goes through [`typed`], the pair `(font(role), TypeRole(role))`, which is
/// a function a painter calls with no system parameter to spare: a role's size is a fact of the type
/// scale, the scale a person chose is a fact about the editor, and the count is not the point.
/// The running scale is kept here in one atomic, and [`set_text_scale`] is its only writer
/// ([`crate::ui::follow_the_text_size`]).
///
/// **`u32` thousandths rather than an `f32`**, because an `AtomicU32` is lock-free on every target
/// this editor ships and an `f32` atomic does not exist; 1.0 is [`TEXT_SCALE_ONE`], a text size is
/// never negative, and a thousandth of a step is finer than any type the scale can draw.
///
/// **A frame that has not set it reads 1.0**, so a label built before any settings were read — a
/// harness, a build step — is drawn at the size the design was made at, not at zero.
static TEXT_SCALE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(TEXT_SCALE_ONE);

/// 1.0 as [`TEXT_SCALE`] holds it: the shipped size, in thousandths.
const TEXT_SCALE_ONE: u32 = 1_000;

/// **How large the type is against the size the editor was designed at.** 1.0 is as designed; see
/// [`crate::settings::SCALE_RANGE`] for the ends a file may ask for.
pub fn text_scale() -> f32 {
    TEXT_SCALE.load(std::sync::atomic::Ordering::Relaxed) as f32 / TEXT_SCALE_ONE as f32
}

/// **Set the type's size** (UI-73, #251). The only writer, and the value is clamped to the range a
/// settings file may hold so a hand-edited file cannot draw a label of no height.
pub fn set_text_scale(scale: f32) {
    let clamped = scale.clamp(crate::settings::SCALE_RANGE.0, crate::settings::SCALE_RANGE.1);
    let thousandths = (clamped * TEXT_SCALE_ONE as f32).round() as u32;
    TEXT_SCALE.store(thousandths, std::sync::atomic::Ordering::Relaxed);
}

/// **A role's size times a person's scale** (#324): the terminal's text size setting multiplies the
/// role, so the size still comes from the type scale and a setting cannot name a bare number.
pub fn font_scaled(role: text::Role, scale: f32) -> TextFont {
    TextFont::from_font_size(role.px() * scale)
}

/// **One glyph's advance at [`text::BODY`], logical pixels** (`metrics.type.body_char_w`). The shipped face
/// is `FiraMono-Regular.ttf` — monospace — so `chars * body_char_w()` is exact, not an estimate, and a
/// string can be measured without a text-layout round trip. Stated once, in the style sheet beside the
/// scale it belongs to: it was measured independently in `badges` (chord columns) and `compose` (centred
/// slot labels), and two hand-copied 6.6s drift the day the face or the size changes. If the face ever goes
/// proportional, layouts drift; they do not break.
pub fn body_char_w() -> f32 {
    crate::style::metrics().r#type.body_char_w
}

/// **One nesting step of the editor's own trees, in logical pixels: two advances of the body face.**
/// This is #489's rule — *"the editor's own indents should be the 2-space step the file editor already
/// uses"* — stated as the width it is drawn at rather than as a count of spaces.
///
/// **It is the file editor's own step** ([`crate::files::YAML_INDENT_SPACES`] x [`body_char_w`]), so a
/// tree row one level in stands exactly where a YAML line one level in stands in the editor beside it.
/// Before #523 there was no such step: gooey's `tree_indent` sized a level at `1.5 x the text size`
/// (24 px here, measured — 3.64 advances), so a folder's children stepped four-odd spaces while the
/// file the folder held indented by two. Two steps in one tree, which is what the owner saw.
///
/// Written onto every `TreeTwisty` by [`the_tree_steps_two_advances`], after gooey has laid the skin
/// out, and read by the tests that measure a row's inset.
pub fn tree_step() -> f32 {
    crate::files::YAML_INDENT_SPACES as f32 * body_char_w()
}

/// **One line of [`text::BODY`] at 100% text, logical pixels**: the body size times the sheet's
/// `line_height_factor`, Bevy 0.20's default `LineHeight` (`RelativeToFont(1.2)`, `bevy_text` `text.rs`).
/// For reserving room for text that comes and goes, so its arrival does not move what sits below it.
pub fn body_line() -> f32 {
    text::BODY.px() * crate::style::metrics().r#type.line_height_factor
}

/// **One line of [`text::BODY`] at the person's text size** — [`body_line`] times [`text_scale`].
///
/// A box that reserves room for text has to reserve it at the size the text is drawn, or a person
/// at 200% gets a row one line tall holding a two-line-tall label: the text is clipped, and
/// `every_label_fits_its_panel_at_two_hundred_percent_text` measures exactly that. Spawn sites use
/// this; a box that has to follow a *live* change asks gooey for `Extent::LinesCeil`, which tracks the
/// running text size itself.
pub fn body_line_h() -> f32 {
    body_line() * text_scale()
}

// ── spacing ──────────────────────────────────────────────────────────────────────────────────────
//
// **A scale, used as a set.** van den Berg, Cornelissen & Roerdink 2009 (`10.1167/9.4.24`) find that
// clutter is *crowding*, not element count: what makes a group read as a group is that its members
// sit closer to each other than to anything else. So what matters here is the RATIO between these,
// not their absolute values — a panel where every gap is 4 px has no groups, however few things are
// in it, and that is what the tiles detail block was.
//
// Density's other half cuts against reflexive minimalism: the test is
// Vicente & Rasmussen's *"does this force interpretation?"*, not "how many things are on screen",
// and Yang et al. 2017 measured more information **improving** performance. So the answer to a
// crowded panel is spacing and grouping, not deletion.

// **The steps are the skin's** (`skin.spacing` in the style sheet: `[3, 5, 12, 16, 20, 24]` px for the
// editor), named below as [`STEP_TIGHT`] (between items that belong to each other), [`STEP_ROW`] (between
// the rows of one block), [`STEP_PAD`] (inside a panel, and a panel from the window's edge) and
// [`STEP_GROUP`] (between blocks: several times the tight step, which is the whole point). A node takes its
// room from a class rule in the sheet (`.chrome-chip`, `.chrome-field`, `.chrome-button`, the label columns
// `.chrome-col-*`); arithmetic about the same room reads [`space`], so the two cannot disagree.

/// **A step of the sheet's spacing scale, logical px** — for code that measures with a step rather than
/// spawning one (a badge's reach, a chip's width). See `crate::style::space`.
pub fn space(step: bevy_gooey::Space) -> f32 {
    crate::style::space(step)
}

/// A block heading: quiet, and separated from what came before it.
///
/// The separation is the work — a heading with the same gap above it as below is a label that could
/// belong to either side.
///
/// **A word longer than the panel breaks at a letter** (`LineBreak::WordOrCharacter`, #535): a heading can carry a name, as COMPOSE's
/// `MEMBERS OF <group id>` does, and with Bevy's word-boundary break a 50-character id stayed one word and was laid out 500 px past the panel's
/// content box at the capture's scale, where a clip hid it and nothing could be read of it. The break alone, and no zero
/// minimum width, is [`body`]'s and [`row_value`]'s: the box keeps reserving its own line.
pub fn section<'a>(parent: &'a mut ChildSpawnerCommands, label: &str) -> EntityCommands<'a> {
    parent.spawn((
        Text::new(label.to_owned()),
        TextColor(ink().label),
        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
        typed(text::HEADING),
        spaced(bevy_gooey::Sides { top: Some(STEP_GROUP), bottom: Some(STEP_TIGHT), ..NO_MARGIN }),
    ))
}

/// Which edge a panel is pinned to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
}

/// **A panel.** A child of one of the frame's docks, opaque, and as tall as the frame lets it be.
///
/// It used to be `PositionType::Absolute` at the old `top: TAB_STRIP_BOTTOM`, pinned to a window edge and
/// floating over the world — which is why nothing on screen filled the window and why a panel's
/// height was a number rather than a consequence. [`Frame`] owns position now; this owns width,
/// which side, and whether the panel wants the dock's whole height.
///
/// `full_height` becomes `flex_grow` rather than a pinned `bottom`, and the reason to keep the
/// distinction is unchanged: a list inside needs a real height to scroll within, and a `max_height`
/// inside a content-sized panel is never reached and does nothing — the map palette shipped that way
/// and rendered two rows. **`min_height: 0` comes with it**, because a flex item's automatic minimum
/// size is its content, so without it the panel grows to fit every row and `overflow` has nothing
/// left to clip.
///
/// **Every panel carries `Hovered`.** `view::drive` and `drive_place` both ask "is the pointer
/// over UI" by looking for any true `Hovered`, and when only the *rows* carried one the gaps between
/// them counted as open map — a wheel turn over a list zoomed the world, and a click that missed a
/// row by a pixel dropped a piece behind the panel. `Hovered` is true for an entity **or any
/// descendant** (`bevy_picking-0.20.0/src/hover.rs:401`), so one on the root answers for the whole
/// surface. The frame above it deliberately carries none — see [`Frame`].
/// **What width a panel would like, so the frame can answer how much it may have.**
///
/// The pair is the whole of B-014's fix: `preferred` is what the panel's own constant asks for and
/// stays the width on any window that can afford it, while `fit_docks_to_the_window` lowers the
/// *applied* width toward [`crate::style::metrics`]'s `layout.dock_min_w()` on a window that cannot. Stored as data rather than
/// recomputed from `Node` so the system never has to guess which of `Node`'s widths was the wish.
#[derive(Component, Debug, Clone, Copy)]
pub struct PanelWidth {
    pub preferred: f32,
    /// Which dock it lives in. Carried rather than inferred from the node's margin: the margin is a
    /// layout detail that could be anything, and the question this answers is which side of the row.
    pub side: Side,
}

pub fn panel_root<'a>(
    commands: &'a mut Commands,
    frame: &Frame,
    side: Side,
    width: f32,
    full_height: bool,
    hidden: bool,
    // What a screen reader calls this panel. Every spawner has one — it is the heading the panel
    // draws — so this is never a second copy of a name (see `headless::the_tab_strip_describes_itself`).
    label: &str,
) -> EntityCommands<'a> {
    let dock = match side {
        Side::Left => frame.left,
        Side::Right => frame.right,
    };
    // **The panel's fill is the skin's, repainted by `dim_the_ground`** (UI-02, #210 slice 4b).
    // `panel_root` is called from six spawn systems that hold no `Res<GooeySkin>`, so reading the skin
    // here would mean threading it through all six and would still leave every panel built *before* a
    // theme edit on the old colour. The marker says "this ground is a panel's"; the system that already
    // walks every ground is what makes it the skin's, once per frame, comparing before it writes.
    let panel = commands
        .spawn((
            // **A panel, in gooey's own sense of the word** (#650): `Role::Panel` is what makes this a box
            // in the accessibility tree a reader can enter, and what gives it the skin's fill
            // (`skin.surface`), its corner and its row gap. It is `bevy_gooey::panel()` — a column, and the
            // role — with the `Sizing` the editor writes, because `fit_docks_to_the_window` and the left
            // pane own the width. **One `Role` per bundle**: `bevy_ecs` panics on a duplicate component, so
            // this replaces the `sized` box the panel used to be rather than stacking beside it.
            //
            // Its **margin** stays the sheet's `.chrome-panel`, because a panel's place is its dock's and no
            // role property states one; its **inset** is the sheet's stated override of `skin.pad()` (see the
            // rule), which is how a person's density still reaches it. A hairline edge, and it grows into
            // what the dock leaves when `full_height`. Its fill and edge are the editor's (`InkFollow`:
            // `Ground`, `PanelEdge` lit by focus and the theme move them). `display` is the tabs' to write
            // (`ShownFollow`), and a hidden panel starts closed.
            (
                bevy_gooey::panel(),
                bevy_gooey::Sizing {
                    width: Some(bevy_gooey::Extent::Px(width)),
                    // **No `max_width` here** (#494): a `Percent(100)` ceiling was tried and reverted. It
                    // narrowed every panel to its pane, which made a panel's own content wrap to an extra
                    // line and pushed the MAP SIZE row down 16 px — `a_hint_does_not_push_map_size_down`
                    // and `a_receipt_does_not_move_the_panel` both refused it, correctly: a hint that comes
                    // and goes with the cursor must not move the panel under the pointer. The overflow this
                    // was meant to fix is a *child* inset 29 px inside its parent, not the panel's width,
                    // and that is measured on the issue.
                    min_height: full_height.then_some(bevy_gooey::Extent::Px(0.0)),
                    ..default()
                },
            ),
            (
                bevy_gooey::RowLayout { direction: Some(FlexDirection::Column), ..default() },
                bevy_gooey::TextFlow { grow: full_height.then_some(1.0), ..default() },
                // Its margin and its inset are the sheet's `.chrome-panel`: the first because a panel's
                // place is its dock's, the second because this editor wants twice the air a dialog's
                // panel gets. The role supplies the corner and the row gap, so the rule states neither.
                bevy_gooey::StyleClass::of("chrome-panel"),
                bevy_gooey::BorderPx(bevy_gooey::Sides::all(crate::style::metrics().layout.edge_w)),
                bevy_gooey::InkFollow,
            ),
            PanelWidth { preferred: width, side },
            // **The panel's name, for something that cannot see it** (UI-30, #224). A `Role::Panel` is a
            // box in the accessibility tree with no name on it, so a reader meets the editor as a set of
            // unlabelled regions. `label` is what the panel calls itself on screen, passed in by its
            // spawner; `AccessibleLabel` is what a reader reads out.
            bevy::prelude::AccessibleLabel::new(label),
            BackgroundColor(ink().panel_bg),
            Ground(ink().panel_bg),
            PanelGround,
            // The edge and the corner are what make this read as a surface rather than as a
            // rectangle of slightly different paint — see the elevation-ladder header.
            BorderColor::all(ink().panel_edge),
            // Which panel the keyboard is in is drawn on this border; nothing lights it until a
            // `Focused` lands, so a panel nobody has focused keeps `PANEL_EDGE`.
            PanelEdge,
            Hovered::default(),
            // **A panel, and a switch's to take down** (#695, #678): it is a child of a dock, so the roots
            // sweep cannot reach it, and the frame it stands in is spared with the scene.
            crate::screen::WorkspacePanel,
        ))
        .id();
    if hidden {
        // `Display::None`, never `Visibility` alone: a visibility-hidden UI node still occupies layout and still answers hover, which would leave a hidden tab's rows eating clicks aimed at the
        // world, and in a dock it would hold the dock open at its width. `ShownFollow` leaves `display` to the tabs that open and close it, and starts it closed.
        commands.entity(panel).insert(bevy_gooey::ShownFollow::closed());
    } else {
        commands.entity(panel).insert(bevy_gooey::ShownFollow);
    }
    commands.entity(dock).add_child(panel);
    commands.entity(panel)
}

/// **The window's own layout, and the one thing on screen that belongs to no panel.**
///
/// # Why there is a frame at all
///
/// There were two layouts and neither filled the window. The editor drew `PositionType::Absolute`
/// panels of fixed pixel width, anchored to the left and right edges and floating over a 3-D camera
/// that owned the whole window; the menu was a fixed-pixel flex grid whose own code said it *"is
/// fixed-size and simply sits in whatever window there is"*. On a 2560x1406 window that left about
/// two fifths of the screen as ground nothing could use, panels sized for twenty rows holding two,
/// and rows wrapping into their own value column — reported as *"it's bad"*, and visible in the
/// capture that started this.
///
/// So position stops being something a panel decides. A panel says how wide it wants to be and which
/// side it lives on; **where it goes is the frame's answer**, and the frame is the window.
///
/// ```text
/// +--------------------------------------------------+
/// |  < kits & maps                 KIT . furniture   |  chrome bar
/// +--------------------------------------------------+
/// |  KIT | MAP | RIGS      1 MESHES 2 TILES 3 COMPOSE |  door strip
/// +----------+---------------------------+-----------+
/// | left     |                           | right     |
/// | dock     |        viewport           | dock      |  body: flex_grow 1
/// |          |                           |           |
/// +----------+---------------------------+-----------+
/// |  stamped 4 pieces        Hold K . Cmd+O back      |  status band
/// +--------------------------------------------------+
/// ```
///
/// # Flex, deliberately, and not `Val::Percent`
///
/// [`crate::style::metrics`]'s `layout.ui_scale` multiplies every `Val::Px` and every font size and leaves `Val::Percent`
/// alone. A layout mixing percentage widths with pixel minimums would therefore
/// scale two ways at once on a 2x display, which is a bug that only appears on somebody else's
/// monitor. `flex_grow` on the centre and pixel widths on the docks is responsive *and*
/// density-correct, because the centre is defined as whatever is left.
///
/// # What the frame must not carry
///
/// **No `Hovered`, and `Pickable::IGNORE` throughout: on the frame and on every node inside it.**
/// `view::over_ui` and `view::drive` both ask "is the pointer on the interface" by looking for a
/// true `Hovered`, and `Hovered` is true for an entity
/// *or any descendant*. So one pickable child inside a non-hoverable row (a label, a value, a
/// glyph) turns that row's `Hovered` on, and the frame's with it, and the answer is yes for the
/// entire window: the map would stop taking clicks and the wheel would stop zooming, everywhere,
/// with nothing to point at. `Hovered` belongs on the panels, which is where [`panel_root`] puts
/// it, and on the one node a row exposes as a button (`Pickable::default()` with `Hovered`).
#[derive(Resource)]
pub struct Frame {
    pub root: Entity,
    /// Above everything, and the same on every door.
    pub chrome_bar: Entity,
    /// The door's own strip of panels.
    pub door_strip: Entity,
    /// Everything between the two bars. The editor fills its three slots; the **menu** puts its own
    /// columns straight in here, because a menu has no docks and no viewport — the frame is the
    /// window's shape, not the editor's.
    pub body: Entity,
    pub left: Entity,
    /// The hole the world is drawn through. Carries [`ViewportSlot`].
    pub viewport: Entity,
    pub right: Entity,
    /// **The bottom dock** (UI-61, #247): a gooey `Dock` across the foot of the workbench, under the
    /// body and above the status band. It holds the panels that say what became of the work —
    /// `docs/ui/vision.md`'s "bottom dock: history · problems · notes · log", of which the reload
    /// panel is the first to move here.
    pub bottom: Entity,
    /// One band at the foot of the window: what happened, what went wrong, what the keys are.
    pub status: Entity,
}

/// **The node the world shows through, and the only one with no background.**
///
/// `crate::surface` reads its rect and gives it to the map camera as a viewport, which is what makes
/// the viewport a region rather than the whole window with panels floating over it. Reflected so a
/// BRP caller (`tools/ux/first_run_walk.py`) can find the map on screen and click it.
#[derive(Component, Reflect)]
#[reflect(Component)]
pub struct ViewportSlot;

#[derive(Component)]
pub struct ShellSplit;

/// **The shell's split is still sized by its left panel, not by a person** — see
/// [`size_the_shell_split`]. Present from spawn until the first time the fraction is written by anyone else.
#[derive(Component, Default)]
struct SplitUnsized {
    /// The fraction this system last wrote, so a different one is known to be somebody else's.
    wrote: Option<f32>,
}

/// Everything that spawns into a dock runs after this.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameSystems;

/// **The census's name for this control** — what makes a key badge land on the thing it acts through.
///
/// Attached by the panel that spawns the node, and joined by id in `crate::badges`. Symbolic on
/// purpose: `keys.rs` declares a [`keys::ControlId`] for a verb's home and never learns what a
/// palette *is*, and a badge system querying fourteen domain markers (`PaletteRow`, `TagChip`,
/// `CellButton`, …) would be the second census `keys.rs` exists to delete.
///
/// **One per node, and one visible node per id.** `crate::badges::anchor` resolves an id to the
/// unique node carrying it with a non-zero [`bevy::ui::ComputedNode`] — which is also the visibility
/// test, since [`panel_root`]'s hidden form is `Display::None` and lays out to nothing. Two visible
/// at once is a bug rather than a tie to break, and
/// `every_control_the_census_homes_a_verb_at_is_on_screen` is what says so.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Control(pub keys::ControlId);

/// **This node's fill is ground, not ink — and this is the colour it rests at.**
///
/// A panel's `Ink::panel_bg` and a band's `Ink::bar_bg` are what their rows and words *stand on*. They
/// carry no information of their own, so a badge parked on bare ground covers nothing a reader
/// needs — which is the whole difference between an overlay that has somewhere to go and one that
/// hugs a dock edge and lands on the map. `badges::ink` reads this to subtract a container from the
/// occupancy census while keeping every child in it.
///
/// **The colour is carried rather than recomputed**, the way `badges::BadgeRest` is: [`dim_the_ground`]
/// restores exactly what was spawned, so it is not a second place that decides what a panel is
/// coloured, and a panel that changes its fill cannot drift out of sync with the system that dims it.
#[derive(Component)]
pub struct Ground(pub Color);

/// **This ground is a panel's, so its colour is the running skin's `surface`** (UI-02, #210 slice 4b).
///
/// `panel_root` is called from six spawn systems that hold no `Res<GooeySkin>`, so the spawn cannot
/// read the skin without threading a parameter through all of them. [`dim_the_ground`] can: it already
/// runs every frame over every [`Ground`], already compares before writing, and is therefore the one
/// place where "what a panel is coloured" can be one expression rather than six.
///
/// The marker is what tells a *panel's* ground (the skin's `surface`) from a *band's* (`Ink::bar_bg`,
/// which gooey has no role for). Without it the repaint would repaint the status bar as a panel.
#[derive(Component)]
pub struct PanelGround;

/// **This ground is a band's** — the chrome bar and the status bar. Painted from
/// [`PanelFills::band`], which is the skin's `"bar_bg"` tone, through the same system that paints a
/// panel's, so a project's skin restyles both and the editor keeps one place that decides a ground.
#[derive(Component)]
pub struct BandGround;

/// **The fills a panel and a band take from the running skin** (UI-02, #210, slice 4b).
///
/// `panel_root` used to spawn `Ink::panel_bg` and `Ink::bar_bg` — constants — so every panel in the editor was
/// painted once, at spawn, and never asked the skin again. A project's `src/style/editor.yaml`, or a
/// `family/bevy_gooey.yaml` edit at run time, therefore reached the list rows and the migrated ink but
/// **not the largest surface on screen**.
///
/// One function, called by the spawn and by [`dim_the_ground`] alike, the way [`RowFills::of`] is
/// called by `style_list_rows` and its test. An assertion here is an assertion about what a panel
/// paints, and it needs no window and no pointer.
///
/// **The fallback is the shipped table**, never a second palette: a headless harness that never ran
/// `EditorUiPlugin` has no `GooeySkin`, and it must get exactly the colours the editor has always had.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelFills {
    /// A panel's ground — [`GooeySkin::surface`](bevy_gooey::GooeySkin::surface).
    pub panel: Color,
    /// A band's ground — the status bar and the chrome bar. A skin that carries a `"bar_bg"` tone paints
    /// the bands with it; one that does not (including the editor's own, whose exact tone set is pinned
    /// by `the_skin_carries_the_editors_other_colours.rs`) falls back to `Ink::bar_bg`, which is what the
    /// editor has always painted. So a *project's* skin can restyle the bands today without a protected
    /// assertion moving, and the editor's own look is unchanged by construction.
    pub band: Color,
}

impl PanelFills {
    /// From the running skin when there is one, the constants when there is not.
    pub fn of(skin: Option<&bevy_gooey::GooeySkin>) -> PanelFills {
        match skin {
            Some(s) => PanelFills {
                panel: s.surface,
                band: s.tone("bar_bg").unwrap_or(ink().bar_bg),
            },
            None => PanelFills { panel: ink().panel_bg, band: ink().bar_bg },
        }
    }
}

/// [`PanelFills`] for the world as it stands.
pub fn panel_fills(world: &World) -> PanelFills {
    PanelFills::of(world.get_resource::<bevy_gooey::GooeySkin>())
}

/// **A surface whose border says whether the keyboard is in it.** Carried by every [`panel_root`].
#[derive(Component)]
pub struct PanelEdge;

/// **Put this on a [`PanelEdge`] to light its border.** The owner of the focus decides — the chooser
/// moves it with its column, an editor door with its `Context` — because *where the keyboard is* is
/// a fact those modules already hold and a second definition of it here would be a second answer.
#[derive(Component)]
pub struct Focused;

/// Paint [`PanelEdge`] from [`Focused`]. Compares before writing — `BorderColor` is change-detected
/// and there are thirteen panels, so an unconditional write would dirty the whole interface sixty
/// times a second for a border that moves twice a session (`tests/no_system_writes_every_frame.rs`).
fn light_the_focused_panel(mut panels: Query<(&mut BorderColor, Has<Focused>), With<PanelEdge>>) {
    for (mut border, focused) in &mut panels {
        let want = focus_edge(focused);
        if border.top != want {
            *border = BorderColor::all(want);
        }
    }
}

/// **How wide each panel should be, given the window** — B-014.
///
/// The frame's contract is that *a panel says how wide it wants to be, and the frame answers how much
/// room there is*. The answer was missing: every panel was `width: Val::Px(preferred)` and nothing
/// could take it back, so a 900 px window gave the two docks 485 + 346 and the map **69 px**.
///
/// **The arithmetic is over every visible panel, not the docks.** A dock is as wide as its widest
/// visible child, so the row's demand is the sum of the widest panel on each side, and that is what
/// has to fit. One panel per door is visible at a time (`Display::None` is how the rest hide, which
/// is also why they lay out to nothing and are skipped here).
///
/// **Both sides are squeezed by the same fraction**, so the pair keeps the proportion its two
/// constants chose rather than one side winning a negotiation it started with a wider number. When
/// the fraction is 1 nothing is written at all, which is the common case and the reason this costs
/// nothing on an ordinary window.
///
/// The floor is the sheet's `dock_min_w` per side. A window narrower than the two floors can afford does not
/// get a smaller interface, it gets the viewport it has left — the docks cannot lie about their
/// content by becoming narrower than a row.
fn fit_docks_to_the_window(
    windows: Query<&Window, With<bevy::window::PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut panels: Query<(&PanelWidth, &Node, &mut bevy_gooey::Sizing, &ComputedNode), Without<ViewportSlot>>,
    owned: Query<(), (With<ShellSplit>, Without<SplitUnsized>)>,
    collapsed: Query<(), (With<ShellSplit>, With<bevy_gooey::SplitSaved>)>,
    bars: Query<&ComputedNode, With<crate::activity_bar::ActivityBar>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let scale = ui_scale.0.max(f32::EPSILON);
    // **The activity bar (NV-03, #380) is paid by the docks, not by the map**: it stands in the body's row
    // beside them, so its width comes out of what the two docks may take while the map's share stays what
    // it was. Measured, because the bar's width is gooey's icon tab and its edge; absent, or not laid out
    // yet, it is nothing.
    let bar: f32 = bars.iter().map(|n| n.size().x * n.inverse_scale_factor()).sum();
    // The row's room in `Val::Px` units — the units the constants above are written in. `Window`'s
    // width is already logical, and `UiScale` multiplies every `Val::Px`, so dividing takes the
    // logical width back to the units the panel widths are expressed in.
    let room = window.resolution.width() / scale;

    // **The row's demand is the widest *visible* panel on each side** — a dock is as wide as its
    // widest visible child, which is the dock's own definition, and the rest are `Display::None` and
    // lay out to nothing. Two per door are visible on no door, so the two slots below are exactly
    // the two docks.
    let mut widest = [0.0f32; 2];
    // Each dock is its panel plus the margin that panel carries, **measured rather than derived**: the
    // margin is the style sheet's (`.chrome-panel`), so it is read off the laid-out node, never restated.
    let mut overhead = 0.0f32;
    // **A collapsed left pane still has the panel's room counted** (UI-R06, #336): the pane slides to nothing and a peek floats it
    // back over the viewport, and a right dock that was re-fitted to the room the hidden panel gave up would jump with the
    // collapse and move the viewport again when a peek showed the panel. The panel is out of the layout (its dock is
    // `Display::None`) and is what the right dock's share was worked out against, so that is still the answer.
    let left_stands = !collapsed.is_empty();
    for (want, node, _, computed) in &panels {
        let counted = node.display != Display::None && (computed.size().x > 0.0 || (left_stands && want.side == Side::Left));
        if !counted {
            continue;
        }
        let side = usize::from(want.side == Side::Right);
        widest[side] = widest[side].max(want.preferred);
        let px = |val: Val| if let Val::Px(px) = val { px } else { 0.0 };
        overhead = overhead.max(px(node.margin.left) + px(node.margin.right));
    }
    let layout = crate::style::metrics().layout;
    let sides: Vec<f32> = widest.iter().copied().filter(|w| *w > 0.0).collect();
    if sides.is_empty() {
        return;
    }
    let sum_pref: f32 = sides.iter().sum();
    let sum_overhead = overhead * sides.len() as f32;

    // **The viewport is paid first.** The docks were not overflowing a 900 px window — measured, they
    // took 835 of it and left the map 65 — so the question is not "do they fit" but "what is the map
    // owed". `squeeze` is the fraction of its preferred width each panel keeps.
    //
    // **Owed: the floor, or its share of the window, and the divider comes out of the room first**
    // (#282). The shell's split costs a divider's width that the two fixed docks never paid, and a
    // pane that is as wide as its panel (see [`size_the_shell_split`]) leaves the map what is left:
    // measured 741 px at a 1600 px window, 28 short of the 769 the docks alone left, and 504 at 1280
    // where the frame's own test asks for half the window. So the map is owed the sheet's `viewport_share` of
    // the window once that is more than the floor, and the panels give the difference back.
    let room = room - layout.divider_w;
    let owed = layout.viewport_min_w.max(layout.viewport_share * room);
    // (The map is owed its share of the room *before* the bar comes out, below.)
    let room = room - bar;
    let squeeze = if room - (sum_pref + sum_overhead) >= owed {
        1.0
    } else {
        let affordable = room - owed - sum_overhead;
        (affordable / sum_pref).clamp(0.0, 1.0)
    };
    // **No panel narrower than the floor**, which binds hardest on the *narrowest* side: the two
    // sides scale by one fraction, so the fraction that keeps the smaller panel at `dock_min_w` is
    // the one that keeps both usable.
    let floor_fraction = layout.dock_min_w() / sides.iter().copied().fold(f32::INFINITY, f32::min);
    let squeeze = squeeze.max(floor_fraction.min(1.0));

    let person_owns_the_split = !owned.is_empty();
    for (want, _, mut sizing, _) in &mut panels {
        // **The left panel is the pane's once a person has dragged the divider**
        // ([`the_left_panel_follows_its_pane`]); writing its width here too would be two systems
        // disagreeing about one number every frame.
        if person_owns_the_split && want.side == Side::Left {
            continue;
        }
        let width = Some(bevy_gooey::Extent::Px(want.preferred * squeeze));
        if sizing.width != width {
            sizing.width = width;
        }
    }
}

/// **Build the frame and remember its slots.**
///
/// The `Frame` resource is how [`panel_root`] finds the dock to put a panel in; BSN-style name
/// lookup does not exist here and a marker query would need a flush between the frame and the first
/// panel. Ordering is [`FrameSystems`], which every panel spawner runs `after`.
pub fn spawn_frame(
    mut commands: Commands,
    settings: Option<Res<crate::settings::EditorSettings>>,
    workspace: Option<Res<crate::workspace::Workspace>>,
    // **A switch keeps the frame, so a second one would stand on top of it** (#695, #678): the resource
    // survives the workspace reset and its root is spared, so there is already a frame to draw into.
    already: Option<Res<Frame>>,
) {
    if already.is_some() {
        return;
    }
    // **The workspace's own arrangement, else its default** (UI-33, #227; per workspace since
    // NV-02, #379). `sync_settings` has read the files by the time a frame is built, so this is the
    // layout the editor opens on — and it is keyed by the workspace, so Map's inspector being wide
    // says nothing about Kit's.
    let workspace = workspace.as_deref().copied().unwrap_or_default();
    let layout = settings
        .as_deref()
        .and_then(|s| s.workspace_layout.get(workspace.id()).cloned())
        .unwrap_or_else(|| crate::ui::shell_layout(workspace));
    // **A bar** (G-87): a gooey row the sheet's `bar_h` tall and the window wide, its parts centred, never shrinking, its gap and its inset from each
    // end the sheet's `.chrome-bar`, with a hairline along the one side that faces the rest of the screen. Its colours are the editor's (`InkFollow`):
    // `Ground`/`BandGround` paint the fill and the theme moves them.
    let bar_h = crate::style::metrics().layout.bar_h;
    let bar = |border: bevy_gooey::Sides<f32>| {
        (
            bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Center), shrink: Some(0.0), ..default() }),
            bevy_gooey::StyleClass::of("chrome-bar"),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), height: Some(bevy_gooey::Extent::Px(bar_h)), ..default() },
            bevy_gooey::BorderPx(border),
            bevy_gooey::InkFollow,
        )
    };

    let chrome_bar = commands
        .spawn((
            bar(bevy_gooey::Sides { bottom: Some(1.0), ..default() }),
            BackgroundColor(ink().bar_bg),
            Ground(ink().bar_bg),
            BandGround,
            BorderColor::all(ink().panel_edge),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();

    // The strip sets its own padding — a tab is a box, not a word on a bar — so this one only says
    // how tall the band is and where it starts.
    let door_strip = commands
        .spawn((
            // The door strip: a row the window wide with its tabs 2 px apart (`GapPx`), `MARGIN` in from the left, stretched to the strip's height, never shrinking.
            bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Stretch), shrink: Some(0.0), ..default() }),
            bevy_gooey::GapPx(2.0),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), ..default() },
            bevy_gooey::Spacing { padding: bevy_gooey::Sides { left: Some(STEP_PAD), ..default() }, ..default() },
            bevy::picking::Pickable::IGNORE,
        ))
        .id();

    // **A dock is a column with no width of its own.** Its width is its widest visible panel, which
    // is what lets four tabs' panels share one dock and each keep the width it argued for
    // (`layout.controls_w` against the Tiles tab's own) without a dock-level table saying it twice.
    let dock = || {
        (
            // A column of its panels to the start that never shrinks. A `sized` box, not a gooey column: `Role::Column` gives a node a least width of 0, and a dock's width is its widest
            // panel (its automatic minimum), which is what keeps a panel from being squeezed by the pane it stands in.
            bevy_gooey::sized(bevy_gooey::Sizing::default()),
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Column), align: Some(AlignItems::FlexStart), shrink: Some(0.0), ..default() },
            bevy::picking::Pickable::IGNORE,
        )
    };
    // **Each side dock is a focus region too** (UI-35, #229). A `Host` viewport with no other region is
    // a keyboard trap: F6 would cycle to itself and Tab would be the only way out. The docks navigate
    // normally — `ArrowPolicy::Navigate` — so F6 walks viewport → left dock → right dock and back, and
    // the arrows in a dock move between its fields, which is what a person expects there.
    // **Each region says its name** (UI-30, #224). The frame was reachable only as an entity id in
    // the `Frame` resource: nothing a reader or a test could search for, so `record.py` guessed
    // pixels and a screen reader met three unnamed boxes. These four names are the shell's own
    // vocabulary — the same words `docs/ui/vision.md` uses for the regions.
    let left = commands
        .spawn((
            dock(),
            bevy_gooey::FocusRegion::navigate(),
            AccessibleLabel::new("Left dock"),
        ))
        .id();
    let right = commands
        .spawn((
            dock(),
            bevy_gooey::FocusRegion::navigate(),
            AccessibleLabel::new("Right dock"),
        ))
        .id();

    let viewport = commands
        .spawn((
            ViewportSlot,
            AccessibleLabel::new("Viewport"),
            // **A focus region that keeps the arrows for the map** (UI-35, #229). The viewport pans and
            // orbits on the arrow keys, so it asks gooey for `ArrowPolicy::Host` — with focus on or
            // inside it, the arrows move no focus and stay readable, while Tab and F6 still leave
            // (WCAG 2.1.2: a region never traps the keyboard). `TabIndex(0)` makes the viewport its own
            // first widget, so a click or F6 puts focus here.
            bevy_gooey::FocusRegion::host(),
            bevy::input_focus::tab_navigation::TabIndex(0),
            // **The centre is what is left**, which is the whole idea: a gooey fill column (it grows and never has a least width of its own, so a wide panel narrows it instead of pushing it
            // off the window), still a row of what it holds.
            bevy_gooey::fill_column(0.0),
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Row), align: Some(AlignItems::Stretch), ..default() },
            bevy_gooey::Sizing { min_height: Some(bevy_gooey::Extent::Px(0.0)), ..default() },
            // No `BackgroundColor` at all: this is the hole the world shows through.
            bevy::picking::Pickable::IGNORE,
        ))
        .id();

    let body = commands
        .spawn((
            // The body: what the bars leave, a row of the docks and the viewport.
            bevy_gooey::fill_region(),
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Row), align: Some(AlignItems::Stretch), ..default() },
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), min_height: Some(bevy_gooey::Extent::Px(0.0)), ..default() },
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    // **A gooey split between the left dock and the viewport** (UI-30, #224).
    let split = commands
        .spawn((
            bevy_gooey::split(bevy_gooey::SplitAxis::Horizontal, (), ()),
            bevy_gooey::SplitFraction(0.22),
            // **Both panes keep a floor, so a drag cannot squash the world** (UI-30, #224).
            // Without it `SplitMin` is `(0, 0)` and `SplitFraction` 0.95 left the viewport 47 px of
            // a 1280-px body — a dock covering the world in the only way that matters. gooey's own
            // dock uses the same 96 px for both panes. The left pane's floor is a panel's own
            // (the sheet's `dock_min_w` plus its margins, a panel's `STEP_PAD` each side), because a
            // dragged-narrow pane makes the panel narrow with it ([`the_left_panel_follows_its_pane`])
            // and a panel under that floor stops being a panel.
            bevy_gooey::SplitMin::new(crate::style::metrics().layout.dock_min_w() + 2.0 * space(STEP_PAD), bevy_gooey::DOCK_PANE_MIN_PX),
            // **`Pickable::default()`, not `IGNORE`** (UI-30, #224). `bevy_picking` skips an
            // entity *and everything below it* when the entity is `IGNORE`, so an ignoring split
            // root made its divider — the one grabbable node in the frame — unhittable: the
            // pointer reached the divider's cursor, the drag press landed on nothing, and the
            // fraction stayed at 0.22 through every drag. The frame's rule is about `Hovered`
            // propagating (`spawn_frame`'s header), and a pickable node without `Hovered` sets
            // none: the panes keep `IGNORE` via `adopt_the_shell_split`, and the divider carries
            // no `Hovered` in this gooey revision. Measured over BRP after the fix: a 105-px drag
            // takes `SplitFraction` from 0.22 to 0.32.
            bevy::picking::Pickable::default(),
            // **The divider is the sheet's seam, not gooey's target** (#689, #476). `Role::SplitDivider`
            // gives the channel `MIN_TARGET_PX` (24 logical px) unless the split carries a thickness, which
            // `update_splits` writes into the divider's own `Sizing` and keeps across every restyle — the
            // knob `bevy_gooey#108` was filed for. The sheet's `divider_w` is the number the dock arithmetic
            // already subtracts (`size_the_shell_split`), so the width a pane is measured with is the width
            // drawn.
            bevy_gooey::SplitThickness(crate::style::metrics().layout.divider_w),
            ShellSplit,
            SplitUnsized::default(),
        ))
        .id();
    // **What makes the split's panes fit is the regions being parented into them**
    // (`adopt_the_shell_split`): at HEAD, with no panes occupied, the viewport measured 504 px of a
    // 1280-px body; with the adoption it measures 724. The zero basis below is the same shape every
    // other centre in this frame uses — a flex item's basis is its content when it is `Auto`, so the
    // split would otherwise take a width of its own rather than the room the docks leave.
    // A filling split (G-88): as tall as the body, as wide as what the right dock leaves (`Extent::Auto` + `grow`), with no least size of its own.
    commands.entity(split).insert((
        bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Auto), height: Some(bevy_gooey::Extent::Percent(100.0)), min_height: Some(bevy_gooey::Extent::Px(0.0)), ..default() },
        bevy_gooey::TextFlow { grow: Some(1.0), min_width: Some(bevy_gooey::Extent::Px(0.0)), ..default() },
    ));
    commands.entity(body).add_children(&[split, right]);
    let _ = (left, viewport);

    let status = commands
        .spawn((
            bar(bevy_gooey::Sides { top: Some(1.0), ..default() }),
            AccessibleLabel::new("Status bar"),
            BackgroundColor(ink().bar_bg),
            Ground(ink().bar_bg),
            BandGround,
            BorderColor::all(ink().panel_edge),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();

    let root = commands
        .spawn((
            // **`Vh`, not `Percent`.** A UI root is a grid item of Bevy's implicit viewport node
                // whose single row is `auto`, so a percentage height is cyclic there and, under
                // Bevy 0.20's taffy, resolves to the content's height: the root grew to its
                // content and no `ScrollArea` below it ever had anything to scroll. A viewport
                // unit is a definite length and does not depend on the track.
            bevy_gooey::sized(bevy_gooey::Sizing {
                width: Some(bevy_gooey::Extent::Percent(100.0)),
                height: Some(bevy_gooey::Extent::Vh(100.0)),
                // **The window's width is a ceiling, not a suggestion** (#497). A `Percent(100.0)`
                // width says how wide the frame is; it does not stop a *descendant* from claiming
                // more, because a flex item's automatic least width is its content's and that least
                // width propagates up the chain. The terminal's body holds one unwrapped row per line
                // of output (`TextLayout::no_wrap`, so a shell's 200-column `cargo` line is one
                // `Text`), and at text scale 3.0 that row measured 1541 px in a 1280-px window: the
                // root then laid out 1569 px wide and the whole window scrolled sideways.
                //
                // Measured across ten frames at 1280 x 720: at 1.0 and 1.6 the root, the pane and the
                // body were all 1280, and at 3.0 all three were 1569. Hiding the terminal pane left
                // the root at 1569, so no pane was what stretched it. Stating the least width as
                // nothing is the whole fix: afterwards the root, the pane and the body measured 1280
                // at 3.0, with 1.0 and 1.6 unchanged. What clips the long row is the body's own
                // `clip`, and the pane re-wraps its scrollback to the columns the narrowed body gives
                // it, which is what `terminal_view::measure_the_cell` exists for.
                //
                // **Not stated here** (measured 2026-11-20): `min_width: Px(0.0)` on the frame root
                // fixes the text-scale-3.0 overflow on its own, but combined with c1's status band it
                // makes a panel's child sit 29 px right of where the pane allows — `no_panel_draws_a_
                // control_past_its_own_content_box`, `the_terminal_bar_keeps_every_chip_whole` and both
                // `rig_history` tests refuse it in the full suite while plain main passes all four. The
                // ceiling belongs where the overflowing row's own box can express it, not on the root.
                ..default()
            }),
            // See the type doc: no `Hovered`, and nothing here eats a click meant for the map.
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    // **And the arrangement itself, as a gooey dock** (UI-30, #224). The hand-built `left`/`viewport`/
    // `right` row above is still what the frame *names*, because twelve systems read those entities to
    // place a control or a badge; the dock is what now holds the viewport's pane, so its divider can be
    // dragged. Moving the two docks onto gooey's `Dock` as well is the rest of this issue.
    let dock = commands
        .spawn((
            ArrangementDock,
            bevy_gooey::Dock(layout),
            // **The map is where a library piece may be let go** (UI-52, #238): the dock covers the viewport, so it is
            // the drop target, and `editor::drop_a_piece_on_the_map` places what lands on it.
            bevy_gooey::DropTarget::<crate::editor::PieceDrag>::new(),
            // **It covers the viewport and takes no room in it** (#437, #438). In the flow it was a `width: 100%` sibling of whatever a workspace puts in
            // the viewport slot (the Design page, the Files editor), which shared the row with it: a page stood in the slot's right-hand part and the rest was
            // empty, or ran past the window's edge. `cover` is gooey's own absolute fill: it is still the whole of the hole the world is drawn through, for the
            // drop to land on, and it is under the pages (it is the slot's first child).
            bevy_gooey::cover(),
        ))
        // **The drop target needs the pointer**: `cover` is `Pickable::IGNORE`, which skips the entity and what is under it. Inserted after the spawn, never in
        // the same tuple as `cover` (a bundle with `Pickable` twice panics).
        .insert(bevy::picking::Pickable::default())
        .id();
    // **The dock's own body is the host's, and it is not laid out** (dark-incorporated/bevy_gooey#85).
    //
    // The dock here is the arrangement's *data*: what `ui::save_the_layout` saves, what
    // `ui::follow_the_saved_layout` restores, what `stage_panels::arrange` selects a tab in, and the node a
    // library piece is let go on. The panels themselves are still drawn by this file's own spawners into
    // `Frame`'s nodes — what `ui::register_the_shell_panels` registers is a placeholder that carries the
    // panel's id, not its contents.
    //
    // **That was silently true until gooey #85.** `update_docks` used to return early when a dock the host
    // had spawned had no `Role::DockBody` child, which is what this bare `Dock` was: it built nothing. Since
    // #85 a host-placed dock grows its body, its landing and its ghost, and builds every page its layout
    // names — so the shell grew three `panel()` cards (INSPECTOR at 22%, VIEWPORT, PLACE) filled with the
    // skin's surface, stacked over the map between the author and the level. Measured before this: the
    // viewport's own node was under a card, and `crate::view::OverUi` answered *yes* at its centre.
    //
    // gooey has no opt-out, and the path it does document is this one — *"a host that made one of them
    // itself keeps it"* (`bevy_gooey`'s own docs) — so the body is the frame's, a `Display::None` node the
    // layout is built into. A `Display::None` node has a zero rect, so nothing under it is laid out, drawn
    // or picked: the pages exist and are inert. **When the panels themselves move into this dock** (the rest
    // of UI-30, #224) this node becomes the real body and this paragraph goes with the change.
    // A gooey box with the dock body's role put over its own (`dock_body()` is not public), hidden through `Shown`, which writes both `Display::None` and `Visibility::Hidden`.
    let silent_body = commands
        .spawn((bevy_gooey::sized(bevy_gooey::Sizing::default()), bevy_gooey::Shown(false)))
        .insert(bevy_gooey::Role::DockBody)
        .id();
    commands.entity(dock).add_child(silent_body);
    commands.entity(viewport).add_child(dock);

    // **And the bottom dock** (UI-61, #247): a gooey `dock()` — the bundle, which brings its own real
    // body, so its panels are built and laid out — across the foot of the workbench. A row of the root's
    // own column between the body and the status band, so it takes its height from the window and every
    // panel above it keeps the rest. It starts on the reload panel: what became of each file's last edit
    // is the thing an author looks away from the map to read.
    //
    // **The `Node` is inserted after the spawn, not beside the bundle.** `bevy_gooey::dock()` brings a
    // `Node` of its own (`dock()` is `Node { 100% x 100% }, Role::Dock, Dock(layout), children![..]`),
    // and a bundle carrying the same component twice panics at spawn — every system that reads
    // `Frame` then failed with "Resource does not exist", because `spawn_frame` never finished.
    let bottom = commands
        .spawn((
            BottomDock,
            bevy_gooey::dock(bevy_gooey::DockLayout::group(["reload"])),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    // The window wide and the sheet's `bottom_dock_h` tall, never shrinking, with a hairline along its top, in the editor's colours (`InkFollow`). **Not shown until there is room**
    // ([`crate::shell_view::give_the_bottom_dock_its_room`] opens it, writing `display` itself: `ShownFollow`): a dock that is never laid out has no text for a label census to
    // find drawn outside it, and takes nothing from the workbench.
    let measures = crate::style::metrics().layout;
    commands.entity(bottom).insert((
        bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), height: Some(bevy_gooey::Extent::Px(measures.bottom_dock_h)), ..default() },
        bevy_gooey::TextFlow { shrink: Some(0.0), ..default() },
        bevy_gooey::BorderPx(bevy_gooey::Sides { top: Some(measures.edge_w), ..default() }),
        bevy_gooey::InkFollow,
        bevy_gooey::ShownFollow::closed(),
        BorderColor::all(ink().panel_edge),
    ));

    commands
        .entity(root)
        .add_children(&[chrome_bar, door_strip, body, bottom, status]);

    // **The frame is the window's, and every workspace that shows the map draws into it** (#695, #678): a
    // switch between Map, World, Kit and Animation changes which panels are in it, not the frame itself.
    commands.entity(root).insert(crate::screen::KeptAcrossASwitch);

    commands.insert_resource(Frame {
        root,
        chrome_bar,
        door_strip,
        body,
        left,
        viewport,
        right,
        bottom,
        status,
    });
}

/// **The dock that holds the workspace's arrangement** (UI-30, #224; a second dock since #247).
///
/// `ui::save_the_layout` and `ui::follow_the_saved_layout` write and read the *workspace's* layout, so
/// they must reach this dock and no other: with a bottom dock in the frame, an unfiltered query
/// would write the arrangement into the bottom dock and read the bottom dock's layout back as the
/// workspace's.
#[derive(Component)]
pub struct ArrangementDock;

/// **The bottom dock** (UI-61, #247): the panels that say what became of the work.
#[derive(Component)]
pub struct BottomDock;

/// **A panel's heading.**
///
/// `Ink::text`, not `Ink::accent`. The amber used to head every left dock, and that was one of the five jobs
/// the 2026-09-03 decision took off it: amber now means *a value you are changing right now*, and a
/// panel's name never changes. What separates a title from the rows under it is that it is the
/// largest thing on the panel ([`text::TITLE`]) and has a group gap below it — size and space, which
/// is what a heading is supposed to be made of.
pub fn title(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text.to_owned()),
        TextColor(ink().text),
        typed(text::TITLE),
        spaced(margin_bottom(STEP_ROW)),
    ));
}

// ── what a tab has to say ────────────────────────────────────────────────────────────────────────

/// **A tab's voice: the running commentary, and the thing that went wrong.**
///
/// # The renderer was guessing at severity, and each tab guessed differently
///
/// Every tab used to hold one `status: String` written from ~215 places, and decided at *render* time
/// whether it was bad news. The Map tab sniffed `status.starts_with("NOT SAVED")`; the Anim tab
/// coloured by whether `rigs.yaml` had loaded, which is a fact about a file rather than about the
/// sentence; the Compose tab painted everything `Ink::accent`; the Tiles tab had no colour rule at all,
/// so a refusal was byte-identical to a receipt. `stamp refused:`, `is not a number of metres` and
/// `NOT WRITTEN:` all rendered in the same grey as `stamped 4 piece(s)`.
///
/// **Severity is known at the write site and nowhere else.** So the write site states it, and no
/// renderer is allowed an opinion — the same move `crate::keys` makes for bindings and this module
/// makes for panels, for the third time and the same reason: a fact stated in two places drifts.
///
/// # Two slots, because one cannot both stick and stay current
///
/// A problem **survives every note**. That is the whole point — a refusal used to vanish the moment
/// the cursor moved, because half of these messages fire on ordinary hovering. But a single slot that
/// refused to be overwritten would swallow the receipts too: an author who fixes the problem and saves
/// would watch the old error sit there and never learn the save worked.
///
/// So a note and a problem are different things with different lifetimes, and the panel shows both.
/// `editor.rs`'s `Field::Edges` had already reached this conclusion for the one readout it owns —
/// *"a fault is a state the map is in — it does not happen once and stop being true, so putting it on
/// `Last` would let the next action erase it"*. This generalises that line to every tab.
///
/// # The banner and the log are one list seen twice
///
/// A problem does not replace the last one — it joins it. The banner shows the newest, because that
/// is what just happened; the log down the bottom shows the run, because *"what has gone wrong on
/// this tab"* is a different question from *"what went wrong just now"*, and the second one was
/// unanswerable: each new refusal erased the one before it, so a session that raised five could only
/// ever show the fifth.
///
/// One list means one clearing rule. [`Status::dismiss`] — `Esc` — takes down the banner **and** the
/// log, because they are two views of the same thing and two clearing rules for one datum is the
/// drift this module exists to prevent.
///
/// Not a `Resource`: each tab holds one inside the state it already owns, so there is nothing to
/// register and no question about which tab a bare `Res<Status>` would have meant.
#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Status {
    note: String,
    problems: Vec<Problem>,
    /// **Everything this status has ever been handed, repeats included** — and never reset, not
    /// even by [`Status::dismiss`].
    ///
    /// It is the [`Journal`]'s clock. The journal is session-wide and a `Status` is a tab's working
    /// list capped at [`MAX_PROBLEMS`] and cleared by `Esc`, so the journal cannot be derived by
    /// looking at what a status currently holds — by the time it looks, the evidence may be gone.
    /// A number that only goes up says *"there have been three more since you last looked"* without
    /// either side keeping a copy of the other's list.
    raised: u64,
    /// How many fell off the front of [`Self::problems`] at [`MAX_PROBLEMS`].
    ///
    /// Counted rather than forgotten. This crate's caps *"refuse and name rather than truncate"*,
    /// and a log cannot refuse — the newest problem is the one most worth having — so it names what
    /// it dropped instead. A log that silently forgot its first entries would be a log that reads
    /// complete and is not.
    dropped: usize,
}

/// One problem, and how many times it has been raised back to back.
///
/// **Consecutive repeats collapse.** A refusal fires per gesture, and gestures repeat: an author
/// clicking four times at a blocked cell would otherwise push four identical lines and bury
/// everything else on the tab. Only *consecutive* ones fold, so the order stays honest — the same
/// rule `keys::rows` uses to collapse adjacent bindings that share a description.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub text: String,
    pub count: usize,
    /// **What `text` was printed from** (UI-05): the problem's `what`, `where`, `why` and `fix`, kept
    /// unprefixed so the log, the journal and an agent's answer carry the same parts. `None` for a
    /// journal row written from a bare string ([`Journal::record`]).
    pub problem: Option<blueprint_model::problem::Problem>,
}

impl Problem {
    /// How it reads in the log: the text, and the tally only when there is one worth showing.
    pub fn line(&self) -> String {
        if self.count > 1 {
            format!("{}  (x{})", self.text, self.count)
        } else {
            self.text.clone()
        }
    }

    /// **What this refusal is about**, when it names one thing: the entity id in its location, which
    /// the toast's Show button selects.
    pub fn cause(&self) -> Option<&str> {
        self.problem.as_ref()?.where_.as_ref()?.entity.as_deref()
    }
}

/// How many problems a tab keeps. Past this the oldest is dropped and counted — see
/// [`Status::dropped`].
///
/// Sized to a panel rather than to memory: this is a list somebody reads down, and past a dozen or
/// so a list stops being read and starts being scrolled, which is the same argument
/// `keys::no_context_gains_a_key` makes one panel over.
pub const MAX_PROBLEMS: usize = 12;

impl Status {
    /// **What just happened, and went as asked.** Replaces the last note; never touches the problem.
    ///
    /// This is the right call for a refusal the author is not owed an alarm about — clicking bare
    /// ground with the remove tool armed, dragging a box round nothing. Those are answers, not
    /// failures, and a red block for each would teach an author to stop reading red blocks.
    ///
    /// # The line between an answer and a refusal
    ///
    /// **The test is whether the editor did what was asked.**
    ///
    /// | | the slot |
    /// |---|---|
    /// | the author asked for nothing in particular — clicked bare ground, dragged a box round nothing | **a note.** The editor answered, and the answer is "there was nothing here" |
    /// | the author asked for something and the editor **could not do it** | **a problem.** A piece that mounts on a surface cannot be flood filled; a map that cannot be written is not saved |
    /// | the author asked for something and the editor **did only part of it** | **a problem.** A fill that stopped at the cap laid some of what was asked |
    ///
    /// **Three refusals were in this slot and in the wrong one** — the fill's three (`editor.rs`,
    /// the box fill, the riders and the flood), whose own comments said *"a refusal is the answer,
    /// not a failure"*. **That sentence read the rule from the wrong half of this contract**: the
    /// half that says *an answer* is about a no-op the author can see was a no-op, and **a fill that
    /// could not be laid is not a no-op** — the author asked for a floor and did not get one.
    ///
    /// **And the difference is not cosmetic.** A `note` is one dim row, and it is erased by the next
    /// receipt and by a tab switch ([`Status::clear_note`]). A `problem` reaches the toast and the
    /// journal, and survives both. NN/g's Error-Message Guidelines: *"the very worst error messages
    /// are those that don't exist."* **Measured:** a fill that was refused said nothing anywhere the
    /// author looks, while the ghost's refusal for the same piece was plainly visible.
    pub fn note(&mut self, text: impl Into<String>) {
        self.note = text.into();
    }

    /// **The editor could not do what was asked, or did only part of it.**
    ///
    /// The line, in practice: a write that did not reach disk, a refusal handed back by `blueprint_model`,
    /// input that would not parse, a piece that changed underneath the author, or an edit that
    /// succeeded and then failed to redraw. Anything a person would want to still be on screen a
    /// minute later.
    pub fn problem(&mut self, text: impl Into<String>) {
        let text = text.into();
        let problem = blueprint_model::problem::Problem::new(text.clone());
        self.push_problem(text, problem);
    }

    /// **A [`blueprint_model::problem::Problem`], raised with its parts.** The line the toast and
    /// the log show is `prefix` and the problem's own [`line`](blueprint_model::problem::Problem::line),
    /// and the problem itself is kept on the row, unprefixed, so the journal and an agent's answer
    /// read the same `what`, `where` and `fix` from it that the line was printed from.
    pub fn raise(&mut self, prefix: &str, problem: blueprint_model::problem::Problem) {
        let text = format!("{prefix}{}", problem.line());
        self.push_problem(text, problem);
    }

    /// **A refusal about one named thing**: the line is exactly `text`, and the id (a library piece or
    /// a placement) is kept as where it happened, so the toast offers Show for it ([`Problem::cause`]).
    pub fn problem_about(&mut self, text: impl Into<String>, entity: &str) {
        use blueprint_model::problem::{Detail, Location, Problem as Core};
        let text = text.into();
        let problem = Core::with(text.clone(), Detail::default().at(Location::entity(entity)));
        self.push_problem(text, problem);
    }

    fn push_problem(&mut self, text: String, problem: blueprint_model::problem::Problem) {
        // Counted before the fold, because a repeat is a thing that happened — see [`Self::raised`].
        self.raised += 1;
        if let Some(last) = self.problems.last_mut()
            && last.text == text
        {
            last.count += 1;
            return;
        }
        self.problems.push(Problem { text, count: 1, problem: Some(problem) });
        if self.problems.len() > MAX_PROBLEMS {
            self.problems.remove(0);
            self.dropped += 1;
        }
    }

    /// **A call that either worked or refused**, routed by which it did.
    ///
    /// For the helpers that return `Result<String, String>` — `Ok` is the receipt, `Err` is the
    /// refusal. Written once here so no call site can talk itself into stringifying an error into a
    /// note, which is exactly how `NOT WRITTEN:` ended up rendering grey.
    pub fn say(&mut self, outcome: Result<String, String>) {
        match outcome {
            Ok(receipt) => self.note(receipt),
            Err(refusal) => self.problem(refusal),
        }
    }

    /// **Drop the running commentary**, leaving the problems alone.
    ///
    /// A note is a receipt for something that just happened *here*; a problem is a state the editor
    /// is in. So changing what "here" means ends a note's relevance and none of a problem's — which
    /// is why this is not [`Self::dismiss`].
    ///
    /// Reported at the keyboard, 2026-08-18, and the report was of a different bug entirely: a
    /// rotate receipt — `lamp_tall 270,270,180 deg` — sat on the panel across every tab switch,
    /// because `note` is one `String` that is only ever overwritten and Meshes and Tiles share one
    /// `ImportState`. The author read a message announcing a rotation, on a tab they had just
    /// arrived at, beside a piece that was lying on its side, and concluded the tab switch had
    /// turned it. Measured over BRP: the rotation quaternion is identical on both tabs and
    /// `library.yaml` is not written. **The stale receipt was the whole of the bug.**
    pub fn clear_note(&mut self) {
        self.note.clear();
    }

    /// **Take the notices down** — the banner and the log together, because they are one list.
    ///
    /// `Esc`'s last layer, and the only thing that clears a problem without another replacing it.
    pub fn dismiss(&mut self) {
        self.problems.clear();
        self.dropped = 0;
    }

    /// See [`Self::raised`] — the journal's clock, not a count of what is currently held.
    pub fn raised(&self) -> u64 {
        self.raised
    }

    pub fn note_text(&self) -> &str {
        &self.note
    }

    /// **The newest problem** — what the banner shows. Empty when there is none.
    pub fn problem_text(&self) -> &str {
        self.problems.last().map_or("", |p| p.text.as_str())
    }

    /// The whole run, oldest first — what the log shows and what a copy carries.
    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }

    /// How many fell off the front at [`MAX_PROBLEMS`].
    pub fn dropped(&self) -> usize {
        self.dropped
    }

    pub fn has_problem(&self) -> bool {
        !self.problems.is_empty()
    }

    /// **The one thing to say, when only one thing fits** — the problem if there is one, else the
    /// note.
    ///
    /// For a log line and for the sentinel driver, whose whole job is to make a refusal visible in a
    /// captured frame. Printing the note there would report `stamped 4 piece(s)` from an earlier verb
    /// while the verb being logged had just refused.
    pub fn line(&self) -> &str {
        if self.has_problem() {
            self.problem_text()
        } else {
            &self.note
        }
    }

    /// Nothing to say at all — neither a receipt nor a refusal.
    pub fn is_empty(&self) -> bool {
        self.note.is_empty() && self.problems.is_empty()
    }
}

/// **Every refusal since the application started**, across tabs and doors.
///
/// # Why this is not four `Status`es
///
/// A [`Status`] is a tab's working list: capped at [`MAX_PROBLEMS`], cleared by `Esc`, and thrown
/// away with the door that owned it. That is right for *"what has gone wrong here"* and useless for
/// *"what has gone wrong at all"* — asked for at the keyboard: *"a hidden log that pops up if you
/// press the key… and that log shows every error message that's happened since the beginning of the
/// application."*
///
/// So this outlives both: `Ownership::Session`, which is the only class in `screen::OWNERSHIP` that
/// survives a door change, and exactly what a session log means.
///
/// # Written by watching, not by being told
///
/// `Status::problem` is a method on a plain struct four tabs own — it has no `Commands` and no way
/// to reach a resource, and giving it one would mean routing every refusal in the editor through an
/// event just so a log could exist. `notice::record_problems` watches [`Status::raised`] instead: a
/// counter that only goes up, so the journal can tell *"three more since I looked"* without either
/// side holding a copy of the other's list — which is what makes `Esc` clearing a tab harmless here.
#[derive(Resource, Default)]
pub struct Journal {
    entries: Vec<Problem>,
    dropped: usize,
    /// **The [`bevy_gooey::ToastLog`] id of each entry's row**, parallel to `entries` (`None` until `notice::sync_the_log` has written it), so a
    /// repeat folds into the row it belongs to and the refusal's toast can be the same row ([`bevy_gooey::ToastBundle::logged_as`]).
    pub(crate) log_ids: Vec<Slot>,
    /// **The agent's steps for this session (E-068)**, oldest first, capped by
    /// [`crate::steps::CAP`]. Read from the operation log's own `result` lines rather than written
    /// here, so a row cannot claim more than the log says.
    ///
    /// **One list with the refusals.** The journal is opened with `Cmd+E` and read by scrolling; a person watching an agent wants the steps and
    /// the refusals in one column — a refusal is a step that did not happen.
    pub steps: Vec<crate::steps::Step>,
    /// How many steps fell off the front at the cap, so the list can say so rather than looking
    /// complete.
    pub steps_dropped: usize,
    /// How many log lines are not entries — a log torn by a crash, or a file edited by hand.
    pub steps_unreadable: usize,
    /// **The one step that can be taken back now** (UI-100, #442): the operation id of the newest change in effect, which `ops::undo_target`
    /// names. Its row carries the **Undo** button; every other row is history.
    pub undoable: Option<String>,
    /// The log rows of the steps drawn, by operation id.
    pub(crate) step_rows: Vec<(String, u64)>,
    /// The log rows that say what the lists left out (steps dropped at the cap, lines of the log that could not be read, refusals dropped), by
    /// what they say, with the sentence each was written as.
    pub(crate) said: Vec<(Said, String, u64)>,
    /// The log rows of refusals that fell off the front of `entries` at the cap, taken out of the log on the next sync.
    pub(crate) forgotten: Vec<u64>,
    /// The steps whose rows the log had no room for (the oldest), by operation id: not written again, and said in the note of what was left out.
    pub(crate) left_steps: Vec<String>,
}

/// Which fact a row of the centre states about the lists themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Said {
    StepsDropped,
    StepsUnreadable,
    RefusalsDropped,
    /// Rows of the journal the log had no room to keep (the oldest), which the centre says it left out.
    LeftOut,
}

/// **Where a refusal of the journal stands in gooey's log.**
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Slot {
    /// Not written yet.
    Unwritten,
    /// **Written and then left out** because the log holds [`bevy_gooey::ToastLog::CAPACITY`] rows and this one was the oldest; the number is the tally it had, so a repeat
    /// (a different tally) is written again as a row of its own, and nothing else is.
    Left(u32),
    /// The row it is, by the id the log gave it.
    Row(u64),
}

/// How many the journal keeps. Ten times a panel's [`MAX_PROBLEMS`], because this one is read by
/// scrolling rather than at a glance — and capped at all for the reason that cap exists: a batch
/// that raises one refusal per frame is a real thing this editor does.
pub const JOURNAL_CAP: usize = 200;

impl Journal {
    /// **Record `times` occurrences of one refusal.** Consecutive repeats fold, exactly as they do
    /// in a [`Status`] — the same rule, so the journal reads like the toast that raised it.
    pub fn record(&mut self, text: &str, times: usize) {
        self.record_problem(text, times, None);
    }

    /// **[`Self::record`], with the structured [`blueprint_model::problem::Problem`] the text was
    /// printed from**, so a row in the journal carries the same `what`, `where` and `fix` as the toast
    /// that raised it and the answer an agent got.
    pub fn record_problem(
        &mut self,
        text: &str,
        times: usize,
        problem: Option<&blueprint_model::problem::Problem>,
    ) {
        if times == 0 {
            return;
        }
        if let Some(last) = self.entries.last_mut()
            && last.text == text
        {
            last.count += times;
            return;
        }
        self.entries.push(Problem { text: text.to_owned(), count: times, problem: problem.cloned() });
        self.log_ids.push(Slot::Unwritten);
        if self.entries.len() > JOURNAL_CAP {
            self.entries.remove(0);
            if let Slot::Row(id) = self.log_ids.remove(0) {
                // The row of a refusal that fell off the front goes with it, on the next sync.
                self.forgotten.push(id);
            }
            self.dropped += 1;
        }
    }

    /// Oldest first, the order they happened in.
    pub fn entries(&self) -> &[Problem] {
        &self.entries
    }

    /// **The log row of the newest refusal that said `text`**, for the toast that raised it to be the same row.
    pub fn log_id_of(&self, text: &str) -> Option<u64> {
        self.entries.iter().zip(&self.log_ids).rev().find(|(e, _)| e.text == text).and_then(|(_, slot)| match slot {
            Slot::Row(id) => Some(*id),
            Slot::Unwritten | Slot::Left(_) => None,
        })
    }

    /// Named rather than silently forgotten — [`Status::dropped`]'s argument, one scope up.
    pub fn dropped(&self) -> usize {
        self.dropped
    }
}

/// **The text of the toast that is up: the one `Text` a refusal is read from.**
///
/// A marker, put by `notice.rs` on the text of the gooey toast it raised for the live tab's newest
/// refusal, so that there is exactly one such text on screen and `Cmd+C` harvests the right sentence.
/// It carries no field: which tab is live is `Mode`'s answer, asked once by
/// `crate::notice::paint_notices`.
#[derive(Component, Clone, Copy)]
pub struct ProblemBanner;

/// **The problem, as a toast over the viewport.**
///
/// # It was a block in the status band, and that is what went wrong
///
/// The band is twenty-six pixels of chrome. A padded, wrapping block of the longest text this editor
/// renders was a flow child of it, so it stood proud of the band it was supposed to be inside.
/// Reported from the keyboard: *"when there's an error, it appears over the status bar at the bottom
/// and isn't quite in alignment, but it's pretty close. So it looks really bad."* Close-but-not is
/// worse than either — a block that plainly floats reads as deliberate, and a block a few pixels out
/// reads as broken.
///
/// # Sticky, because a refusal is a state and the journal is its record
///
/// A problem in this editor is **sticky by design** — it is a state, not an event. The toast stands
/// until it is dismissed (its own Dismiss, `Esc`, or the next refusal replacing it), and the session
/// [`Journal`] behind `Cmd+E` keeps every refusal, across tabs, out of `Esc`'s reach:
/// `notice::record_problems` has already written the journal's copy before the toast is up.
///
/// # Where it goes
///
/// Centred at the top of the **viewport**, which is the one region that belongs to nothing else:
/// the docks own both sides, `compass` owns the bottom-left, and `badges` puts its legend in the
/// bottom-right. Asked for as *"a place that isn't going to occlude much"*.
///
/// # It is `bevy_gooey`'s toast now (UI-39, #233)
///
/// The card this spawned was hand-built: a `Text` on a coloured node, faded by a timer in `notice.rs`.
/// What is spawned here now is the **host** for gooey's own toast: the strip below, and in it the one
/// [`bevy_gooey::ToastStack`] every gooey toast stands in (gooey would create its own at the window's
/// root on the first toast; made here, it hangs under the viewport and shows and hides with the strip).
/// `notice.rs` spawns the toast itself, an alert that stands until it is dismissed, with a Show button
/// when the refusal names what it is about.
pub fn problem_toast(mut commands: Commands, frame: Res<Frame>) {
    let stack = commands
        .spawn((
            Name::new("ToastStack"),
            // The stack is gooey's: a window-sized box (`sized`) with the toast stack's role put over its own.
            //
            // **The height is [`bevy_gooey::Extent::Vh`], not `Percent`** (#479). A `Val::Percent`
            // resolves against the node's parent, and this stack's parent is the `placed` layer below:
            // a row asked for 100% of the width and no height at all. Measured on 2026-11-17, that
            // handed the stack **18 px** of room around a 167 px alert, and `Role::Toast` writes
            // `Overflow::clip()` — so a seven-line `play.yaml` refusal was drawn three lines and cut,
            // which is exactly the strip the owner photographed. `Vh` is a share of the *viewport*, so
            // the stack is the window's height whatever its host's own height resolves to.
            bevy_gooey::sized(bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), height: Some(bevy_gooey::Extent::Vh(100.0)), ..default() }),
            bevy_gooey::ToastStack,
            bevy::picking::Pickable::IGNORE,
            GlobalZIndex(crate::style::metrics().z.toast),
        ))
        .insert(bevy_gooey::Role::ToastStack)
        .id();
    commands
        .spawn((
            // A toast is not a panel. It is over the viewport rather than in the layout, which is the whole point: `Frame` owns where panels go and this is deliberately not
            // one of them. Placed a group's step down and the viewport wide with its stack centred, and hidden (through `Shown`) until a toast is raised.
            bevy_gooey::placed(UiRect { left: Val::Px(0.0), top: Val::Px(space(STEP_GROUP)), ..UiRect::all(Val::Auto) }),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), ..default() },
            bevy_gooey::RowLayout { justify: Some(JustifyContent::Center), ..default() },
            bevy_gooey::Shown(false),
            ToastLayer,
        ))
        .add_child(stack)
        .insert(ChildOf(frame.viewport));
}

/// **Gooey's toast stack keeps clear of the menu bar** (#305).
///
/// A toast is `TopCenter` at the window's top, and the bar is the first 28 px of it, so the tour's
/// offer covered the Compose/Rigs/Tools/Help titles — photographed by the first-run walk at
/// 1088x640 and 1024x640. The editor's **own** host ([`problem_toast`]) hangs inside the viewport,
/// which already starts under the bar, so a refusal was fine; gooey spawns its own stack at the
/// window's root whenever a toast is raised with no host (the tour's offer, raised as a project
/// opens), and nothing told that stack about the bar.
///
/// The height is read from the bar's own laid-out node rather than from a constant: `menu_bar`'s
/// height comes from the skin's type size and padding, so a bigger text scale or a skin change
/// moves it, and `ToastInset` is logical px scaled by `UiScale` — the same units the bar's
/// `ComputedNode` reports through `inverse_scale_factor`. Written only when it differs, so a
/// settled editor costs a comparison.
///
/// **Ungated by screen**: the tour's offer is raised on `OnEnter(Screen::Editor)` before this pass's
/// first `Update`, and a toast can be raised on any screen.
pub fn keep_toasts_under_the_menu_bar(
    mut settings: ResMut<bevy_gooey::ToastSettings>,
    bars: Query<&ComputedNode, With<crate::menu_bar::EditorMenuBar>>,
) {
    let Some(bar) = bars.iter().next().filter(|bar| bar.size().y > 0.0) else { return };
    // Physical in, logical out: `ComputedNode::size` is surface pixels and `UiScale` multiplies
    // every `Val::Px`, so this is the height the bar's own `Val::Px` adds up to.
    let want = bevy_gooey::ToastInset::top(bar.size().y * bar.inverse_scale_factor());
    if settings.inset != want {
        settings.inset = want;
    }
}

/// **The strip the toast stands in.** Its `Display` is what shows and hides the toasts; gooey's stack
/// inside carries them. See [`problem_toast`].
#[derive(Component)]
pub struct ToastLayer;

/// **The dialog the session's notification centre stands in, hidden until `Cmd+E`** (UI-100, #442).
///
/// The journal is **`bevy_gooey`'s [`bevy_gooey::notification_center`]**: every line of it (an agent's step, a refusal, a note) is a
/// [`bevy_gooey::NotificationRow`] of the one [`bevy_gooey::ToastLog`], drawn by gooey, newest first, with its **Undo** button when the step can
/// be taken back. `notice::sync_the_log` writes the log from [`Journal`]; nothing here draws a line. This is only where the centre stands, which
/// gooey's own guide asks of a game ("put it in a panel or a dialog of your own"): an overlay placed 8% down and 10% in, 80% wide and at most 76%
/// tall, with the key hint and the after-edit check beside the centre.
///
/// **An overlay, not a panel in the frame's flow**: it is asked for, read and dismissed, and `Frame` owns the docks. Its lines are `STEP_ROW` apart
/// and a panel's padding in (`.chrome-panel`), a 1 px edge, in the editor's own colours (`InkFollow`); closed until `Cmd+E`, and `display` is the notices' to write (`ShownFollow`).
///
/// No `CopyPane` marker: `notice::copy_out` harvests the TEXT of every visible node by walking the roots, so an open centre is carried by `Cmd+C`
/// already.
///
/// **`Hovered`, and deliberately not `Pickable::IGNORE`** — the rule [`panel_root`] states, for a panel that is not in a dock: an opaque box that
/// no "is the pointer on UI" gate could see made a click on the open journal stamp or delete a placement on the map behind it, and the wheel zoom
/// the world instead of scrolling the list. A `Display::None` node has a zero rect, which both the picking backend and `over_ui` skip.
pub fn notifications_overlay(mut commands: Commands, frame: Res<Frame>) {
    let overlay = commands
        .spawn((
            bevy_gooey::placed(UiRect { left: Val::Percent(10.0), top: Val::Percent(8.0), ..UiRect::all(Val::Auto) }),
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(80.0)), max_height: Some(bevy_gooey::Extent::Percent(76.0)), ..default() },
            bevy_gooey::RowLayout { direction: Some(FlexDirection::Column), ..default() },
            // Its gap and inset are the sheet's `.chrome-overlay`.
            bevy_gooey::StyleClass::of("chrome-overlay"),
            bevy_gooey::BorderPx(bevy_gooey::Sides::all(1.0)),
            bevy_gooey::InkFollow,
            bevy_gooey::ShownFollow::closed(),
            BackgroundColor(ink().panel_bg),
            BorderColor::all(ink().key),
            NotificationsOverlay,
            Hovered::default(),
        ))
        // `placed` is `Pickable::IGNORE`; the journal is the opposite (see above), so it is put back over it.
        .insert(Pickable::default())
        .id();
    commands.entity(overlay).with_children(|p| {
        title(p, "NOTIFICATIONS");
        p.spawn((
            Text::new(format!("{} closes  ·  {} copies this list", keys::chord(keys::Action::Cancel), keys::chord(keys::Action::CopyInfo))),
            TextColor(ink().dim),
            typed(text::LABEL),
        ));
        // Asks the project's after-edit checks about the picture on screen now. A person's click only: nothing runs while they are simply editing.
        button(p, crate::after_edit::CheckViewButton, "Check view", Severity::Plain);
        p.spawn(bevy_gooey::notification_center());
    });
    commands.entity(overlay).insert(ChildOf(frame.viewport));
}

/// **The dialog the session's notification centre stands in.** See [`notifications_overlay`]: its `display` is the open/closed state of the
/// journal, written by `notice::toggle_journal`, `Esc` and the `view_toggle_journal` operation.
#[derive(Component)]
pub struct NotificationsOverlay;


/// **A scrolling list that fills what is left of its panel.**
///
/// `flex_grow` with `min_height: 0`, not `max_height`: a flex item's automatic minimum size is its
/// content, which would grow the node to fit every row and leave `overflow` with nothing to clip. The
/// panel must be `full_height` for this to bound anything.
/// **Where a scroll must move so a row is on screen** — the one arithmetic every list-follow
/// system shares (`tiles::keep_selection_on_screen`, `editor::keep_palette_selection_on_screen`).
///
/// Extracted when the palette gained the same correction the candidates list had (F-9,
/// **Arms when the SELECTION changes — not when the resource holding it does.**
///
/// Every list that follows its highlight needs the same two-step: notice the selection moved, then
/// scroll *next* frame, because the rows are rebuilt on that same change and this frame's
/// `ComputedNode` still describes the previous layout.
///
/// # Why this is a type and not two lines in each system
///
/// The two lines were `if state.is_changed() { pending = true; return; }`, written twice, and both
/// were quietly broken: `is_changed` is true whenever **anything** touches the resource, and both
/// `EditorState` and `ImportState` are written most frames — a status line, a hover, a preview
/// watchdog. So the flag was re-armed every frame and the correction never ran.
///
/// Reported from the keyboard twice. First on 2026-08-14 — *"if I arrow down and the scroll view, it
/// just goes off the screen. The scroll doesn't actually happen."* — and again on 2026-08-16, *"I
/// still have the same bug."* Both times the headless test passed, because in a test nothing else
/// writes the resource, so `is_changed` goes false on the next frame and the correction fires. The
/// test was measuring a world that only exists in tests.
///
/// Keyed on the selection itself, this cannot happen: unrelated writes are invisible to it, and the
/// only thing that arms it is the thing it exists to follow.
pub struct Follow<K> {
    last: Option<K>,
    pending: bool,
}

// A manual impl, because `#[derive(Default)]` would demand `K: Default` and a selection has no
// meaningful zero — `None` is "nothing selected", which is what this starts at.
impl<K> Default for Follow<K> {
    fn default() -> Self {
        Follow { last: None, pending: false }
    }
}

impl<K: PartialEq> Follow<K> {
    /// Give it this frame's selection; it answers whether to scroll **now**.
    ///
    /// `false` on the frame the selection moves — the layout is still last frame's — and `true` on
    /// the one after, so the scroll position is written once per move rather than sixty times a
    /// second.
    ///
    /// # Except on a fast walk, where it fires one move stale rather than never
    ///
    /// The version above returned `pending` **only on a frame where the selection had not moved**,
    /// and that is a third way for this to be starved — the one the 2026-09-03 review found after
    /// the two in the header. A held arrow accelerates to `keys::REPEAT_FAST_SECS` (30 ms) and the
    /// steps land back-to-back on some frames, each pair silently dropping one correction; at any
    /// sustained frame time above 30 ms the selection changes every frame and this fires **never**.
    /// That is the reported symptom exactly: *"I don't see a list of meshes as I press up and down
    /// arrows, but I see the mesh change."*
    ///
    /// So a move that arrives while a correction is already armed **fires anyway**, using a layout
    /// that is one move behind. That is the right trade and it is only right because of the margin
    /// [`scroll_to_reveal`] now keeps: one row stale inside a two-row margin is still on screen,
    /// and the alternative is a list that never scrolls at all.
    pub fn should_scroll(&mut self, now: Option<K>) -> bool {
        if self.last != now {
            self.last = now;
            return std::mem::replace(&mut self.pending, true);
        }
        std::mem::take(&mut self.pending)
    }
}

/// 2026-08-14): two hand-copied versions of fold geometry is how the two lists drift a half-pixel
/// apart, and the arithmetic is the testable part — fold detection, the physical→logical
/// conversion, the clamp, and the dead-band all hold in a unit test, where the pixel scroll itself
/// needs a window.
///
/// Inputs are **physical** pixels — `ComputedNode` and `UiGlobalTransform`, centre and half-size —
/// and the answer is the new **logical** `ScrollPosition::y`, or `None` when the row is already
/// comfortably visible or the correction is under half a pixel (the dead-band that keeps a
/// change-detected write from re-firing layout every frame). A row taller than the list scrolls to
/// its **top**, the half you read first.
///
/// # It reveals with context, and that is the seed bug
///
/// Until 2026-09-03 the correction was exactly `row_bottom - bottom`: enough to put the row's edge
/// on the viewport's edge and not one pixel more. A test pinned that as correct — *"Touching the
/// edges exactly is still inside — flush is not off-screen"* — and it is the wrong rule for a list
/// you walk with the keyboard. Measured live on the Meshes list: after forty presses the selection
/// is glued to the bottom pixel with twenty-six rows of already-passed context above it and **zero
/// rows of what is coming below**. Reported as *"I don't see a list of meshes as I press up and down
/// arrows, but I see the mesh change"* — the list is there, and the part of it that would tell you
/// where you are going is not.
///
/// So the row is revealed with `layout.context_rows` of list either side of it, in units of the row's own
/// height — no new parameter, and it scales with whatever the caller's rows measure. The margin is
/// clamped so a viewport too short to hold row-plus-margins asks for the largest margin that still
/// fits rather than thrashing, which also preserves the over-tall row's scroll-to-top.
///
/// **A jump of more than one viewport re-centres instead of creeping.** A `Shift`×5 step, a click,
/// or a filter that rebuilds the list under the cursor are not walks, and edging such a move to the
/// margin puts the author at the boundary of a region they have not seen; centring says *here* .
pub fn scroll_to_reveal(
    row: (f32, f32),
    list: (f32, f32),
    scroll_y: f32,
    max_scroll_y: f32,
    inverse_scale: f32,
) -> Option<f32> {
    let (row_top, row_bottom) = (row.0 - row.1, row.0 + row.1);
    let (top, bottom) = (list.0 - list.1, list.0 + list.1);
    let (row_h, list_h) = (row.1 * 2.0, list.1 * 2.0);
    // The largest margin that still leaves the row itself room, so a short viewport degrades to
    // flush rather than to a fight between two margins it cannot satisfy.
    let margin = (crate::style::metrics().layout.context_rows * row_h).min(((list_h - row_h) * 0.5).max(0.0));
    // Above the fold, or below it. Never both — the above check winning is what sends an
    // over-tall row to its top.
    let delta = if row_top - margin < top {
        row_top - margin - top
    } else if row_bottom + margin > bottom {
        row_bottom + margin - bottom
    } else {
        return None;
    };
    // Further than a viewport is a jump, not a step: put the row in the middle.
    let delta = if delta.abs() > list_h { row.0 - list.0 } else { delta };
    // **Clamped at BOTH ends.** The floor was always here; the ceiling is what the two-row margin
    // made necessary — a reveal at the end of a list asks for `max + margin`, Bevy clamps only
    // `ComputedNode::scroll_position` (`bevy_ui-0.20.0/src/layout/mod.rs:493-496`, through
    // `bypass_change_detection`), and the component every caller reads back keeps the surplus. The
    // list then stands still while the highlight walks off it — the 2026-09-03 seed, reintroduced
    // by its own fix.
    let want = (scroll_y + delta * inverse_scale).clamp(0.0, max_scroll_y.max(0.0));
    ((scroll_y - want).abs() > 0.5).then_some(want)
}

/// **Where a scroll viewport actually is, and how far it can go** — both in LOGICAL pixels, both
/// read off the node's own `ComputedNode`.
///
/// The position is the **effective** one: Bevy writes the clamped, floored value into
/// `ComputedNode::scroll_position` and leaves the `ScrollPosition` component alone, so the
/// component can hold a position the list is not at. Row geometry is laid out at the effective
/// one, so that is the only honest basis for a delta measured off the screen.
///
/// The maximum is upstream's own: `content - layout + scrollbar`, floored at zero
/// (`bevy_ui-0.20.0/src/layout/mod.rs:493-496`). `scroll_list` overlays its bar rather than
/// reserving a gutter, so `scrollbar_size` is zero here today; it is in the expression because
/// the day it is not, a reveal that ignored it would be short by exactly the bar.
pub fn scroll_bounds(list: &ComputedNode) -> (f32, f32) {
    let inv = list.inverse_scale_factor;
    let max = (list.content_size.y - list.size().y + list.scrollbar_size.y).max(0.0);
    (list.scroll_position.y * inv, max * inv)
}

/// The scrollbar's track. Marked so `forward_a_wheel_over_a_scrollbar` can find it and its own width is
/// stated once.
#[derive(Component)]
pub struct ScrollTrack;

/// **What a scroll bar reserves**: the sheet's bar and its inset (`metrics.layout.scroll_bar_w` and
/// `scroll_bar_inset`), derived from the bar rather than picked, so the lane and the thing it is a lane for
/// cannot drift apart.
pub fn scroll_gutter() -> f32 {
    crate::style::metrics().layout.scroll_gutter()
}

/// **A list that scrolls, and now says so.**
///
/// # It scrolled and nothing on screen admitted it
///
/// This spawned one node with `overflow: scroll_y` and a `ScrollArea`, and **no bar of any kind** —
/// so a list longer than its panel clipped, answered the wheel, and gave the author no indication
/// that there was more. It is visible in the 2026-08-18 captures: the Kit door's candidate list
/// opens part-way down, scrolled to the selection, with nothing saying what is above it.
///
/// # Shape, and why there is a wrapper
///
/// `bevy_ui_widgets::Scrollbar` goes on a **track** entity pointing at the scrolled one, and
/// `ScrollbarThumb` is its child. The track has to be positioned against the list's own box, so the
/// list gains a wrapper and the two are siblings inside it. The **marker stays on the scrolling
/// node** and that is load-bearing: every caller's queries, `chrome::Follow`'s reveal arithmetic and
/// `tests/every_list_follows_its_selection.rs` all key on it, and this returns the viewport rather
/// than the wrapper so not one call site changes.
///
/// **The thumb deliberately has no `Node`.** Upstream lays it out itself in `PostUpdate` after
/// `ui_layout_system`; giving it one looks like it works and then fights `update_scrollbar_thumb`.
///
/// **The bar overlays rather than reserving a gutter.** `Node.scrollbar_width` would reserve space,
/// and it also turns on a shipped disagreement: `ScrollArea`'s wheel handler computes its maximum
/// from `size()` while the scrollbar's own code subtracts `scrollbar_size`
/// (`bevy_ui_widgets-0.20.0/src/scrollarea.rs:32` vs `bevy_ui_widgets-0.20.0/src/scrollbar.rs:176`),
/// so the two disagree by exactly the gutter width. Overlaying sidesteps it.
///
/// # It bounds itself with `flex_grow`, so it needs a bounded parent
///
/// Right for a pane that owns the rest of its dock, and useless **nested inside another scroll**:
/// there the parent is content-sized, `flex_grow` resolves to nothing, and the whole thing collapses
/// to zero — measured, when the tag block vanished outright. A `scroll_box` taking a stated height
/// was written for that case and then deleted with its one caller: the answer was that a scroll
/// inside a scroll is two bars a hand has to choose between, and the pane already scrolled.
pub fn scroll_list<'a>(
    parent: &'a mut ChildSpawnerCommands,
    marker: impl Bundle,
) -> EntityCommands<'a> {
    // **gooey's scroll viewport** (G-90): one bundle that IS the scrolling viewport (the wheel, a drag on the rail, glide and edge fade, keyboard focus followed into view, Page/Home/End),
    // filling the room its parent leaves and able to be nothing tall. Its children's gap is the sheet's `.chrome-scroll`, and the sheet's scroll gutter on its right stays clear
    // for the rail, so nothing is drawn under it (the audit's F7). The rail is gooey's, a sibling of the viewport in its parent; [`tag_the_scroll_rails`] gives it the editor's `ScrollTrack`.
    parent.spawn((
        bevy_gooey::scroll_viewport(0.0, Some(bevy_gooey::Space::Px(scroll_gutter()))),
        bevy_gooey::StyleClass::of("chrome-scroll"),
        marker,
    ))
}

/// **A rail gooey spawned is a scroll track** to the editor's tests and to `forward_a_wheel_over_a_scrollbar`: the same marker the hand-built track carried, put on the rail the frame it
/// appears. The rail shows only while the content does not fit (gooey's own rule, which replaced `hide_idle_scrollbars`).
///
/// **And no rail is drawn while the shell's split is sliding** (#517) — see [`hide_rails_while_the_shell_slides`].
pub fn tag_the_scroll_rails(rails: Query<Entity, Added<bevy_gooey::ScrollRailOf>>, mut commands: Commands) {
    for rail in &rails {
        commands.entity(rail).insert(ScrollTrack);
    }
}

/// **A pane that is sliding says nothing about what fits in it, so it draws no scrollbar** (#517).
///
/// The owner's note, on `feedback-1795094031429`: *"The scroll bar here stays visible during the
/// animation that kicks off when I hit 'Cmd+B'"*. The pick is the PROJECT page's scroll viewport —
/// `Sized > FillRegion > Split > SplitPane > Sized > Sized` — whose rail is drawn over the pane as it
/// slides shut.
///
/// # Measured, and why gooey's own rule cannot see it
///
/// `update_lists` shows a rail exactly while `content > size` (`bevy_gooey/src/list.rs:99`), read off
/// the **last frame's** layout. That is right for a list whose rows came and went, and blind to a pane
/// changing size on a spring: measured on this build at the capture's window (2560x1406, `ui_scale`
/// 2.4, Map door), the left pane's rail was `Display::Flex` with `content` 877 px against a `size` of
/// 553 on **all 29 frames** of a shut and on all 10 sampled frames of a restore, clearing only 2
/// frames after the glide landed. The numbers never move because a full-height pane changes *width*
/// as it slides while the rail's rule is a *height* comparison — so the engine's rule keeps answering
/// about the pane that was there before the chord. A pane at a third of its width is not a pane whose
/// contents fit; it is a pane mid-air, and a bar over it claims a reading about to be untrue.
///
/// # One direction, and gooey puts it back
///
/// This only ever **takes a bar away**, and only while the shell's split carries a
/// [`bevy_gooey::SplitGlide`]. Nothing here puts one back: `update_lists` already writes `Flex` the
/// frame the glide is gone if the content still does not fit, and leaves the `None` it wrote if the
/// content fits — so a settled pane's bar is **gooey's rule**, not a copy of it, and a pane that fits
/// after the slide shows nothing. That is what stops this being "never draw a scrollbar".
///
/// **Every rail, not every viewport.** gooey spawns rails three ways and only one of them
/// (`scroll_viewport`) carries [`bevy_gooey::ScrollRail`]; `list()` and `scroll_region()` spawn a rail
/// of their own that a host cannot switch off, and the editor's panels use all three — measured, the
/// Map door's inspector list is the first kind and its rail survived a marker-based fix. Every one is
/// a `Role::ListRail` pointing at its viewport through [`bevy::ui_widgets::Scrollbar::target`], which
/// is the one thing all three have in common.
///
/// **After gooey styles**, or `update_lists` writes over it in the same frame. **Only the shell's
/// split**, so a section's opening glide (`SectionGlide`) or a dialog's own motion is not this. A
/// settled shell writes nothing at all.
pub fn hide_rails_while_the_shell_slides(
    splits: Query<&Children, (With<ShellSplit>, With<bevy_gooey::SplitGlide>)>,
    roles: Query<&bevy_gooey::Role>,
    parents: Query<&ChildOf>,
    mut rails: Query<(&bevy_gooey::Role, &bevy::ui_widgets::Scrollbar, &mut Node)>,
) {
    let panes: Vec<Entity> = splits
        .iter()
        .flat_map(|kids| kids.iter())
        .filter(|kid| matches!(roles.get(*kid), Ok(bevy_gooey::Role::SplitPane)))
        .collect();
    // **A settled shell costs this query and no write.**
    if panes.is_empty() {
        return;
    }
    let under_a_pane =
        |view: Entity| panes.contains(&view) || parents.iter_ancestors(view).any(|up| panes.contains(&up));
    for (role, bar, mut node) in &mut rails {
        if *role != bevy_gooey::Role::ListRail || !under_a_pane(bar.target) {
            continue;
        }
        if node.display != Display::None {
            node.display = Display::None;
        }
    }
}

/// **A wheel turned over a scrollbar scrolls what the bar is for** (#283).
///
/// The track is a pickable node of its own, a sibling of the scroll area rather than inside it, so a
/// wheel over its five pixels targeted the track, bubbled up through the panel and reached nothing:
/// measured on big, 2026-11-03, a wheel at the inspector's right edge did nothing while the same
/// wheel 12 px to the left scrolled it. `bevy_ui_widgets`' own handler (`scrollarea.rs`) only listens
/// on the area, so this forwards the notch to the area the bar's `Scrollbar::target` names, with the
/// same arithmetic: the pixels the event is worth, held to `[0, content - visible]`.
fn forward_a_wheel_over_a_scrollbar(
    mut scroll: On<bevy::picking::events::PointerScroll>,
    tracks: Query<&bevy::ui_widgets::Scrollbar, With<ScrollTrack>>,
    mut areas: Query<(&Node, &ComputedNode, &mut ScrollPosition), With<ScrollArea>>,
    ratio: Res<bevy::input::mouse::MouseScrollPixelsPerLine>,
) {
    let Ok(bar) = tracks.get(scroll.entity) else {
        return;
    };
    scroll.propagate(false);
    let Ok((node, computed, mut position)) = areas.get_mut(bar.target) else {
        return;
    };
    let visible = computed.size() * computed.inverse_scale_factor();
    let content = computed.content_size() * computed.inverse_scale_factor();
    let delta = scroll.to_pixels(&ratio);
    let range = (content - visible).max(Vec2::ZERO);
    if node.overflow.y == bevy::ui::OverflowAxis::Scroll {
        position.y = (position.y - delta.y).clamp(0.0, range.y);
    }
}

// ── the row vocabulary ───────────────────────────────────────────────────────────────────────────
//
// The builders below were each written AFTER their shape had been hand-rolled at least three times
// (the 2026-08-17 audit counted 8 label/value rows, 7 list rows, 5 chip variants, 6 text fields, 3
// list headings and 2 severity-rail dialects). Each one returns or takes just enough for the call
// sites that exist — a parameter with no caller is a stub, which is why several return
// `EntityCommands` for the caller to finish rather than trying to hold every variation.

/// **A list panel's heading** — "PLACE", "RIGS": the word at the top of a whole panel, over its
/// filter and list. One step louder than [`section`] (10 vs 9) because it heads the panel, not a
/// block within one — the two roles the 2026-08-17 type-role decision named.
pub fn list_heading(parent: &mut ChildSpawnerCommands, text: &str) {
    list_heading_in(parent, text, ListHeadingInk::shipped());
}

/// **The ink a list heading draws in** (UI-02, #210): the shipped `Ink::label` where there is no skin, the
/// skin's own label tone where there is one.
///
/// A separate value rather than a `Res<GooeySkin>` parameter because every caller builds its heading
/// inside a `with_children` closure, where a system parameter cannot reach — the caller reads the skin
/// once, beside the panel it is spawning, and passes the answer in. That is the same shape
/// [`RowFills::of`] and [`crate::anim_tab::BenchInk`]'s constructor use.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ListHeadingInk(pub Color);

impl ListHeadingInk {
    /// The shipped palette, for a build with no skin.
    pub fn shipped() -> ListHeadingInk {
        ListHeadingInk(ink().label)
    }

    /// The skin's ink where it has one, the shipped constant where it does not. `label` is a **tone**
    /// rather than a field (gooey has none for a list's quiet heading), so a project's skin that names
    /// it gets its own; the editor's own skin does not carry one, which keeps it byte-identical.
    pub fn of(skin: Option<&bevy_gooey::GooeySkin>) -> ListHeadingInk {
        match skin {
            Some(skin) => ListHeadingInk(skin.tone("label").unwrap_or(ink().label)),
            None => ListHeadingInk::shipped(),
        }
    }
}

/// **A list heading in a given ink.** The builder every panel uses; see [`ListHeadingInk`].
pub fn list_heading_in(parent: &mut ChildSpawnerCommands, text: &str, ink: ListHeadingInk) {
    parent.spawn((
        Text::new(text.to_owned()),
        TextColor(ink.0),
        typed(text::HEADING),
    ));
}

/// **What arms a list row, and what an armed row does** — the PLACE panel's own sentence (#486).
///
/// One line a person can act on: the mark a row wears, the press that sets it, and the press that spends it. It says
/// "ready" in the words the panel can keep true — every row in the list is offered, and the armed one is the piece the
/// next click on the map places. The character is [`crate::marks::ARMED_MARK`] and is written here from that constant, so
/// the sentence and the mark the rows draw cannot drift.
pub fn place_rule() -> String {
    format!(
        "A click on a row arms that piece ({}), and the next click on the map places it.",
        crate::marks::ARMED_MARK
    )
}

/// **A panel's rule of readiness, drawn above the list it is about** (#486), carrying the words it drew so a test reads
/// the one sentence rather than the constants behind it.
///
/// The note that filed this: *"What makes these ready to use?"*, on a panel where an armed row was marked by a rail and a
/// fill and nothing anywhere said what either meant — and where, in a project whose library is empty, every row was in
/// the same state and the state had no name.
#[derive(Component, Clone)]
pub struct PaletteRule(pub String);

/// **A rule of readiness above a list** (#486): one short sentence, its own line, wrapping inside its own box rather than
/// running past the panel's border.
///
/// It takes `scroll_gutter()` off its right, the lane the scroll list keeps clear for its rail: without it the sentence
/// ends where the *bar* stands rather than where the rows end, and the two edges of one column would not line up.
pub fn palette_rule(parent: &mut ChildSpawnerCommands, words: &str) {
    parent.spawn((
        Text::new(words.to_owned()),
        TextColor(ink().label),
        typed(text::HINT),
        // The layout role a bare text needs for `Spacing` to apply to it; it draws nothing and leaves the ink and the
        // face alone (the rule [`spaced`] states).
        bevy_gooey::Role::Sized,
        bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), ..default() },
        // The rail's lane, so the sentence and the rows end at one edge.
        bevy_gooey::Spacing {
            padding: bevy_gooey::Sides { right: Some(bevy_gooey::Space::Px(scroll_gutter())), ..default() },
            ..default()
        },
        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
        PaletteRule(words.to_owned()),
    ));
}

/// **The label columns of `LABEL  value` rows**, as the style-sheet classes that size them (`chrome-col-tight`,
/// `chrome-col-label`, `chrome-col-wide`): it was six unnamed literals — 76, 62, 56, 48, 40 and 14 — one per author,
/// which is why no two panels lined their values up. Three widths, chosen by what the column has to hold: a single
/// glyph or an axis letter, an ordinary word (`size`, `mount`, `pieces`), a phrase (`new work lands here`). A value
/// that will not fit its column is a `Wide` row, not a seventh number. Their widths are the sheet's rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Col {
    /// A single glyph or an axis letter.
    Tight,
    /// An ordinary word.
    Label,
    /// A phrase.
    Wide,
}

impl Col {
    /// The class whose rule gives the column its width.
    pub fn name(self) -> &'static str {
        match self {
            Col::Tight => "chrome-col-tight",
            Col::Label => "chrome-col-label",
            Col::Wide => "chrome-col-wide",
        }
    }
}

/// **The lane a list row's mark stands in, logical px at 100% text**: the tight label column's width, so a mark
/// (`x`, `!`, a fold) and an axis letter take one lane. Spawned as a gooey `cell`, measured in body sizes, so it
/// grows with the person's reading size instead of clipping the mark at 200%.
pub const MARK_LANE_PX: f32 = 16.0;

/// **The label column of a `LABEL  value` row** — the shape the audit found hand-rolled eight times,
/// differing only in this width. 10 px `Ink::label`, never wrapping, never shrinking. `column` is the class
/// that sizes it: a [`Col`]'s, or a panel's own (`editor-readout-label`).
///
/// # The column is a floor, not a cap, and that is the F5 fix
///
/// It used to be `width: Val::Px(width)`. A `Text` does not shrink to its box and UI overflow is
/// visible by default, so a label longer than its column simply **drew over the value beside it** —
/// which is what the menu's KIT INFO row had been doing in every capture: `new work lands here` is
/// 19 characters, plus the selection mark the inspector prefixes, is 126 px of glyphs starting in a
/// 76 px column, so `yes` rendered underneath the word `lands`.
///
/// Measured by `ChooserSlice` on 2026-09-03, and the measurement is the argument: widening the
/// column to `Wide` takes the overlap from ~74 px to ~30 px. It does not remove it, because
/// no fixed number can — a caller can always be handed a longer string.
///
/// So the column's rule sets a `min_width`, which holds the alignment the column exists for, and a label
/// that does not fit pushes its own value right instead of painting on it. Short labels line up at
/// exactly the column, which is the whole point, and no call site changes.
pub fn row_label(row: &mut ChildSpawnerCommands, column: &str, label: &str) {
    row_label_with(row, column, label, ());
}

/// **A [`row_label`] that keeps its column and says a long phrase on two lines** (#302).
///
/// `row_label` lets a label wider than its column push the value right, which keeps the words apart and
/// moves that one row's value out of line with every other row's. Here the column is the label's width
/// (the sheet's `.chrome-col-wrapping` rule for the column) and a phrase of two words wraps inside it, so
/// `UNDER CURSOR` stands as `UNDER` over `CURSOR` and its value starts where the others do. A word still
/// wider than the column is the caller's to size the column for: it is never cut.
pub fn row_label_wrapping(row: &mut ChildSpawnerCommands, column: &str, label: &str) {
    let id = row_label_with(row, column, label, ());
    row.commands().entity(id).insert((
        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
        bevy_gooey::StyleClass::of(&format!("chrome-row-label chrome-col-wrapping {column}")),
    ));
}

/// [`row_label`] with a marker on its label, for a label that changes while the row is on screen.
pub fn row_label_with(
    row: &mut ChildSpawnerCommands,
    column: &str,
    label: &str,
    marker: impl Bundle,
) -> Entity {
    row.spawn((
        // **The value never starts at the label's last glyph**: the label keeps its words' width and is at least the column wide (`column`'s rule), and a margin
        // of its own (`.chrome-row-label`: the row step) stands the value off however wide a long label turned out, because the row's `column_gap` only applies
        // between boxes, and a label longer than its column pushes its own box wider (measured on the KIT INFO panel: 5 px against 133 px).
        (
            bevy_gooey::Role::Sized,
            bevy_gooey::TextFlow { grow: None, shrink: Some(0.0), min_width: None, align: None },
            bevy_gooey::StyleClass::of(&format!("chrome-row-label {column}")),
        ),
        Text::new(label.to_owned()),
        TextColor(ink().label),
        typed(text::LABEL),
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
        marker,
    ))
    .id()
}

/// **The value beside a [`row_label`]** — 11 px, one step louder than its label, which is the
/// role map's one value size (the audit found 10/11 and 10/10 coexisting; 10/11 won).
///
/// **It gives way to the column it is drawn in** (#494): the value is the last thing in a row whose
/// label column is a fixed width, so a value wider than the room left is drawn over the panel's
/// border — measured in the owner's capture `feedback-1794921716691`, `none yet` ended **7 px past**
/// the AGENT panel's content edge at a 380 px panel with the person's text at 150%.
///
/// **The break is the whole fix, and the floor is deliberately absent** — the lesson
/// `let_a_toggle_label_break` records (#481). `WordOrCharacter` makes the box's minimum its longest
/// *word* rather than its whole phrase, which is what lets flex shrink it inside its column. A zero
/// `min_width` (what [`text_narrows`] carries, and the obvious companion) drops the automatic minimum
/// to nothing: the AGENT block is a `column` in a scrolling pane, so the block stopped reserving its
/// own line and `Control(Hint)` came up under it, and every chip in the terminal bar lost its word —
/// measured on this change, `the_g87_nodes_are_gooeys`, `no_badge_cluster_draws_through_another` and
/// the three `the_terminal_bar_keeps_every_chip_whole` cases all failed with the floor in place.
pub fn row_value(row: &mut ChildSpawnerCommands, text: impl Into<String>, colour: Color, marker: impl Bundle) {
    row.spawn((
        Text::new(text.into()),
        TextColor(colour),
        typed(text::BODY),
        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
        marker,
    ));
}

/// **A line of body text drawn by this crate rather than by a gooey widget** — a list row, the note
/// under a heading — at [`text::BODY`], in the ink the caller names, wrapping inside its own box.
///
/// # Why this exists beside [`bevy_gooey::label`]
///
/// A `bevy_gooey::label` is styled by `apply_skin` with `skin.text_size * skin.text_scale`, **then
/// snapped to a whole device pixel** (`bevy_gooey::snap_text_size`). At this editor's `UiScale` of
/// 1.2 that makes the skin's declared 11 px draw at `ceil(11 * 1.2) / 1.2 = 11.666… px`. Measured on
/// the World workspace (#518), **550 of 602 texts on screen were drawn at 11.666 px** — a size the
/// type scale (`chrome::text`, `{9, 10, 11, 13, 15}`) does not name, and 6 % larger than the
/// [`text::BODY`] that `chrome::font` gives every label this crate draws itself. So one window held
/// two body sizes, and the more common one was the one outside the scale.
///
/// This draws the line through [`typed`], so it is `text::BODY` like every other label here, and it
/// follows the person's text size for free ([`resize_the_labels`]) — a `bevy_gooey::label` does not,
/// because gooey recomputes it from `skin.text_scale` on its own pass.
///
/// **[`LineBreak::WordOrCharacter`] rather than `NoWrap`, and no zero `min_width`**: the same pair
/// [`row_value`] records — the break lets a long line give way inside its column, and the absent
/// floor keeps the box reserving its own line so the block below it does not come up under it.
pub fn body(parent: &mut ChildSpawnerCommands, words: impl Into<String>, colour: Color) {
    parent.spawn((
        Text::new(words.into()),
        TextColor(colour),
        typed(text::BODY),
        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
    ));
}

/// **A switch's label breaks inside the row that bounds it** (#481, the AGENT panel's capture
/// `feedback-1794783669349`: *"The widgets are breaking out of the div here."*).
///
/// gooey's [`bevy_gooey::toggle`] lays its row out as `SpaceBetween` — the label on the left, the switch on the right —
/// and every control's label is `NoWrap` (`widgets::ONE_LINE`), because gooey sizes a control's row by its widest child.
/// That is right in a dialog and wrong in a dock whose pane is narrower than the two: the label's min-content width is
/// its whole unwrapped phrase, the switch keeps its own, and the pair overflows. Measured on the frame: the panel's
/// viewport content ends at `x≈673`, and `Pause agent writes`'s track is drawn from `x=631` to `x=728` — the switch
/// sits **55 px past** the viewport's edge, in the scrollbar's gutter and over the panel's border.
///
/// **The break is the whole fix, and it is the only write here on purpose.** `WordOrCharacter` makes the label's
/// min-content width its longest *word* rather than its whole phrase, which is what lets flex shrink the label into the
/// room the track leaves, where a `NoWrap` label could not give at all.
///
/// **The floor is the trap, and it is measured.** gooey's toggle label has no `TextFlow` of its own, so its
/// automatic minimum is its content; writing a zero `min_width` on the row and on the label — the obvious companion to
/// the break, and what `fill_text` carries — drops the row's automatic minimum to nothing. The AGENT block is a
/// `column` in a scrolling pane, so a block that no longer reserves its own line makes every row below it shorter, and
/// the panel's hint line came up under the block: measured, `Control(Hint)` covering **24x14 px and 24x7 px** of ink
/// in a 1280x720 window, caught by `headless::no_badge_cluster_draws_through_another`. With the break alone, that test
/// passes, the two `the_decision_model_is_a_setting` rows pass, and #481's own two tests pass.
///
/// # Why a system, and why every frame
///
/// The label is gooey's — `toggle` spawns it, so the editor can only reach it after the fact — and gooey
/// re-asserts its control-label layout on every style pass. Retagging it would trade the skin's `ControlText` ink for
/// a break, so this keeps gooey's role and ink and rewrites only the layout,
/// [after](bevy_gooey::GooeySystems::Style) gooey has written the skin's node — a write before it would be
/// overwritten the same frame. Every write is compared first, so a settled panel costs a few reads a frame.
pub fn let_a_toggle_label_break(
    // **The two queries are disjoint by `Text`, not by `Role`** (B0001): a toggle's own row is never a text, and
    // every word this reaches is one. A `Without<Role>` on the words query would be a *second* filter with a cost and a
    // wrong one: the skin puts `ControlText` on exactly the labels this exists to break.
    toggles: Query<(&bevy_gooey::Role, &Children), Without<Text>>,
    mut words: Query<&mut TextLayout, With<Text>>,
) {
    for (role, kids) in &toggles {
        if *role != bevy_gooey::Role::Toggle {
            continue;
        }
        for kid in kids.iter() {
            let Ok(mut layout) = words.get_mut(kid) else { continue };
            if layout.linebreak != LineBreak::WordOrCharacter {
                layout.linebreak = LineBreak::WordOrCharacter;
            }
        }
    }
}

/// **A tree row steps [`tree_step`], not whatever the skin's `tree_indent` says** (#523).
///
/// `bevy_gooey` sizes one level of a tree at `tree_indent(text_px)` = `1.5 x the text size` (24 px at
/// the editor's 11 px body, measured: 3.64 advances), and it writes that width onto every
/// `TreeTwisty` in `apply_skin`. The editor's own step is two advances, the same step its file editor
/// indents YAML and Markdown by (`files::YAML_INDENT_SPACES`), so a folder's children and the files
/// inside it stood 24 px and 2-space apart in the same tree — the two steps the owner photographed.
///
/// # Why a system, and why every frame
///
/// The twisty is gooey's: the widget spawns it, and gooey re-asserts its width on every style pass
/// and on every row it rebinds to a new node (`bind_lead`). So this runs
/// [after](bevy_gooey::GooeySystems::Style), the frame gooey has written its width, and rewrites only
/// the `width` — the box's height, its `flex_shrink` and its margin are the widget's. Every write is
/// compared first, so a settled tree costs a read a frame and no change.
///
/// **The depth a row was placed at, read once from gooey's own indent** (#523).
///
/// `bevy_gooey` stands a row `depth x tree_indent` in from the table's edge and rewrites that margin
/// whenever it rebinds a pooled row to a new node. So the depth is read **when the widget writes it**
/// and kept here: reading it back from the margin every frame would read the editor's own step after
/// the first frame (`13.2 / 24` rounds to 1) and collapse every row below the first onto one indent —
/// measured, one level deeper came out `0.00` further in before this component existed.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeDepth(pub u32);

/// **Both numbers the step is made of are the widget's, and both must move.** `spawn_lead` stands the
/// twisty `depth x indent` in from the row's edge and makes it one `indent` wide, so the chevron and
/// the label after it sit at `(depth + 1) x indent`. Writing only the width would leave every row at
/// the depth gooey placed it and just narrow the chevron's own box. So this keeps the depth gooey
/// placed the row at ([`TreeDepth`]) and writes both at [`tree_step`].
///
/// **The depth is re-read whenever the margin is exactly the widget's own width times a whole
/// number**, which is the state `spawn_lead`/`bind_lead` leave a rebound row in. A margin this
/// system wrote is `depth x TREE_STEP`, which is not that number unless the two happen to agree, so
/// the two reads never fight.
pub fn the_tree_steps_two_advances(
    mut twists: Query<(Entity, &bevy_gooey::Role, &mut Node, Option<&TreeDepth>)>,
    mut commands: Commands,
    skin: Option<Res<bevy_gooey::GooeySkin>>,
) {
    let Some(skin) = skin else { return };
    let theirs = bevy_gooey::tree_indent((skin.text_size * skin.text_scale).max(1.0));
    if theirs <= 0.0 {
        return;
    }
    let step_px = tree_step();
    let step = Val::Px(step_px);
    for (entity, role, mut node, known) in twists.iter_mut() {
        if *role != bevy_gooey::Role::TreeTwisty {
            continue;
        }
        // The widget's own placement: a whole number of its widths.
        let placed = match node.margin.left {
            Val::Px(left) if (left / theirs - (left / theirs).round()).abs() < 0.01 => Some((left / theirs).round().max(0.0) as u32),
            _ => None,
        };
        let depth = match (placed, known) {
            (Some(depth), _) => {
                if known != Some(&TreeDepth(depth)) {
                    commands.entity(entity).insert(TreeDepth(depth));
                }
                depth
            }
            // The widget has not written this row since this system last did: keep the depth read.
            (None, Some(known)) => known.0,
            (None, None) => 0,
        };
        let in_by = Val::Px(depth as f32 * step_px);
        if node.margin.left != in_by {
            node.margin.left = in_by;
        }
        if node.width != step {
            node.width = step;
        }
    }
}

/// **The editor skin's spacing steps, named** (#260): the style sheet's `skin.spacing` scale is `[3, 5, 12, 16, 20, 24]` px, so the editor's own gaps
/// are steps of it. [`bevy_gooey::Spacing`] takes a step.
pub const STEP_TIGHT: bevy_gooey::Space = bevy_gooey::Space::Step(0);
/// Between the rows of one block (5 px) as a step of the skin's spacing scale.
pub const STEP_ROW: bevy_gooey::Space = bevy_gooey::Space::Step(1);
/// Inside a panel (12 px) as a step of the skin's spacing scale.
pub const STEP_PAD: bevy_gooey::Space = bevy_gooey::Space::Step(2);
/// Between blocks (16 px) as a step of the skin's spacing scale.
pub const STEP_GROUP: bevy_gooey::Space = bevy_gooey::Space::Step(3);

/// **The gap between a container's children, as the style-sheet class that sets it**: `chrome-gap-tight`, `-row` and `-group`,
/// whose rules in `src/style/editor.yaml` give the [`STEP_TIGHT`], [`STEP_ROW`] and [`STEP_GROUP`] of the skin's scale. A row's or
/// a column's rhythm is taste, so it is the sheet's: put `Gap::Tight.class()` beside a gooey `row(0.0)` or `column(0.0)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gap {
    /// Between items that belong to each other — chips in one axis, cells in one row.
    Tight,
    /// Between the rows of one block.
    Row,
    /// Between blocks: several times the tight gap, which is the whole point.
    Group,
}

impl Gap {
    /// The class whose rule sets this gap.
    pub fn name(self) -> &'static str {
        match self {
            Gap::Tight => "chrome-gap-tight",
            Gap::Row => "chrome-gap-row",
            Gap::Group => "chrome-gap-group",
        }
    }

    /// The class, to spawn beside a gooey container.
    pub fn class(self) -> bevy_gooey::StyleClass {
        bevy_gooey::StyleClass::of(self.name())
    }
}

/// No margin on any side.
pub const NO_MARGIN: bevy_gooey::Sides<bevy_gooey::Space> = bevy_gooey::Sides { top: None, right: None, bottom: None, left: None };

/// A margin above only.
pub const fn margin_top(space: bevy_gooey::Space) -> bevy_gooey::Sides<bevy_gooey::Space> {
    bevy_gooey::Sides { top: Some(space), ..NO_MARGIN }
}
/// A margin below only.
pub const fn margin_bottom(space: bevy_gooey::Space) -> bevy_gooey::Sides<bevy_gooey::Space> {
    bevy_gooey::Sides { bottom: Some(space), ..NO_MARGIN }
}
/// A margin to the left only.
pub const fn margin_left(space: bevy_gooey::Space) -> bevy_gooey::Sides<bevy_gooey::Space> {
    bevy_gooey::Sides { left: Some(space), ..NO_MARGIN }
}
/// A margin to the right only.
pub const fn margin_right(space: bevy_gooey::Space) -> bevy_gooey::Sides<bevy_gooey::Space> {
    bevy_gooey::Sides { right: Some(space), ..NO_MARGIN }
}

/// **A quiet status item's words are a control's, not a button's** (#260): gooey's `status_item_quiet` puts `Role::ButtonText` on its text, and the editor's tests count `ButtonText`
/// over the world to say what a dialog's answers are (`the_bands_item_does_not_call_itself_a_button`), so the item's words become `Role::ControlText` (and take `marker`) when the item is
/// spawned. `item` is the quiet item's entity, spawned in the same command flush.
pub fn the_words_of_a_quiet_item_are_a_control(commands: &mut Commands, item: Entity, marker: impl Bundle) {
    commands.queue(move |world: &mut World| {
        let kids: Vec<Entity> = world.get::<Children>(item).map(|c| c.iter().collect()).unwrap_or_default();
        if let Some(words) = kids.into_iter().find(|k| world.get::<Text>(*k).is_some()) {
            world.entity_mut(words).insert((bevy_gooey::Role::ControlText, Pickable::IGNORE, marker));
        }
    });
}

/// **The shape of a small box that holds one edge colour** (#260): a sized box laid out as a row, its inset and corner the style sheet's
/// rule for `class` (`chrome-chip`, `chrome-button`), and a `px` border in `edge`, which is passed as `InkOverride` because the skin paints any
/// `BorderPx` edge in its outline unless the host's colour is pinned (so a box whose edge the host changes by state cannot be this: see the chip's
/// GAP4). A row writes only what it is asked to, so a caller's `entry::<Node>()` stands; `shrink` is `Some(0.0)` for a box that never gives way to
/// its neighbours.
pub fn small_box(class: &str, border: bevy_gooey::Sides<f32>, edge: Color, shrink: Option<f32>) -> impl Bundle {
    (
        bevy_gooey::sized(bevy_gooey::Sizing::default()),
        bevy_gooey::RowLayout { direction: Some(FlexDirection::Row), shrink, ..default() },
        bevy_gooey::StyleClass::of(class),
        bevy_gooey::BorderPx(border),
        bevy_gooey::InkOverride { edge: Some(edge), ..default() },
    )
}

/// **The shape of a small box whose colours the editor paints** (#260, G-87): a sized box laid out as a row, its inset and corner the style sheet's
/// rule for `class` (`chrome-chip`, `chrome-row`, `chrome-button`), a `px` border, marked `InkFollow`, so the skin writes neither its fill nor its
/// edge and the editor's `RowRest`/`style_list_rows`, tag painters and `theme_ink::follow_the_skin` stay the one thing that colours it (by state:
/// rest, hover, press, held, selected). A row writes only what it is asked to, so a caller's `entry::<Node>()` (a margin, a centred `align_items`)
/// stands; `shrink` is `Some(0.0)` for a box that never gives way to its neighbours.
pub fn held_box(class: &str, border: bevy_gooey::Sides<f32>, shrink: Option<f32>, sizing: bevy_gooey::Sizing) -> impl Bundle {
    (
        // A gooey `sized` box that is laid out as a row. Not a gooey row: `Role::Row` gives a node a least width of 0, and a chip, a button or a list row keeps its automatic least
        // width (its content), so a row in a narrow lane is as wide as what it holds rather than squeezed under it.
        bevy_gooey::sized(sizing),
        bevy_gooey::RowLayout { direction: Some(FlexDirection::Row), shrink, ..default() },
        bevy_gooey::StyleClass::of(class),
        bevy_gooey::BorderPx(border),
        bevy_gooey::InkFollow,
    )
}

/// **A margin for a text or a cell to stand off its neighbours by** (UI-59, #245; #260): gooey's [`bevy_gooey::Spacing`], in steps of the skin's scale.
///
/// A text carries a `Node` of its own, and a margin is the one thing about it a panel decides: the gap above a block, the half-step below a
/// heading. `Spacing` is written on a node with a `Role`, and a bare `Text` has none, so this carries gooey's `Role::Sized`, a layout role that
/// draws nothing and leaves the text's own ink and font as the host set them (`the_section_headings_keep_their_ink_and_their_margins`).
pub fn spaced(margin: bevy_gooey::Sides<bevy_gooey::Space>) -> impl Bundle {
    (bevy_gooey::Role::Sized, bevy_gooey::Spacing { margin, padding: default() })
}

/// **A text that takes the room its row leaves and may narrow to nothing** (#260): gooey's `TextFlow` on a bare text, with the layout role `TextFlow` is written on
/// (`Role::Sized`, which draws nothing and leaves the text's ink and font as the host set them). A long name wraps in its own column and never pushes its row wider.
pub fn text_narrows() -> impl Bundle {
    (
        bevy_gooey::Role::Sized,
        bevy_gooey::TextFlow { grow: Some(1.0), shrink: None, min_width: Some(bevy_gooey::Extent::Px(0.0)), align: None },
    )
}

/// **A name that never breaks inside a word** (#693): the width its row leaves, clipped, with `Word` breaking and a real `\u{2026}`
/// for what does not fit.
///
/// The crate breaks at a letter on purpose wherever it does (`chrome::fill_text`, `LineBreak::WordOrCharacter` — #535, #481): a
/// file name such as `surveillance_console.glb` is one word wider than its column, and cutting it at a letter is better than
/// drawing it past the panel. **A piece's name in the PLACE list is the other case**: `Fridge` broke as `Fridg` / `e` at 1100 px wide
/// because the column was narrower than the word, and a name in a list of things a person recognises is read whole or not at all.
/// So the name takes the width the row leaves, breaks only between words, and ends what does not fit in an ellipsis.
///
/// **`NoWrap` rather than `Word`**: this text is one line of a list row, and `Word` on a box narrower than its longest word still
/// breaks that word. `NoWrap` never does; `TextLayout::linebreak` is Bevy 0.20's ellipsis, so a name too wide for its column is
/// cut with `\u{2026}` instead of being drawn over the count beside it.
pub fn a_name_that_never_breaks_inside_a_word() -> impl Bundle {
    (
        bevy_gooey::Role::Sized,
        bevy_gooey::TextFlow {
            grow: Some(1.0),
            shrink: Some(1.0),
            min_width: Some(bevy_gooey::Extent::Px(0.0)),
            align: None,
        },
        bevy_gooey::Sizing { clip: Some(true), ..default() },
        TextLayout::new(Justify::Left, LineBreak::NoWrap),
    )
}

/// **A count that gives way before the name beside it does** (#693, the owner's *"count gives way"*): a fixed column that may
/// shrink to nothing, laid out right to left for the last digit when it is cut.
///
/// A `cell` is a floor, not a cap (F5), and the palette's row was built that way: the thumbnail, the mark lane and the count all
/// kept their width and the name gave way, which is how `Fridge` came to break at a letter. The owner's order is the other one:
/// the size the piece costs is recoverable from its tooltip and the name is not, so the count yields first.
pub fn a_count_that_gives_way() -> impl Bundle {
    (
        bevy_gooey::Role::Sized,
        bevy_gooey::TextFlow { grow: None, shrink: Some(1.0), min_width: Some(bevy_gooey::Extent::Px(0.0)), align: None },
        TextLayout::new(Justify::Right, LineBreak::NoWrap),
    )
}

/// **A text that stands at its row's far end** (#542): `TextFlow { align: AlignSelf::End }` on a bare text, with the layout role that makes gooey
/// write it (`Role::Sized`, which draws nothing and leaves the text's own ink and font as the host set them).
///
/// **The role is the point, not the field.** `apply_skin` writes a `TextFlow` only on a node it styles, so a bare `Text` given one and no `Role`
/// keeps a flow nothing ever applied — which is why `Ln 1, Col 1` drew 500 px from its row's right edge beside a path whose `grow` had never been
/// written either. `align_self` moves a flex item along the row's main axis; `grow` would share the free width with the path and leave the readout
/// in the middle.
pub fn text_at_the_rows_end() -> impl Bundle {
    (bevy_gooey::Role::Sized, bevy_gooey::TextFlow { align: Some(AlignSelf::End), ..default() })
}

/// **A text that keeps its words' width** (#260): `TextFlow { shrink: 0 }` on a bare text, so a tally or a mark keeps a column of its own when its row is squeezed.
pub fn text_keeps_its_width() -> impl Bundle {
    (bevy_gooey::Role::Sized, bevy_gooey::TextFlow { grow: None, shrink: Some(0.0), min_width: None, align: None })
}

/// **A margin for a gooey widget** (it has a `Role` already, which `Spacing` needs): the same margin as [`spaced`], without the layout role a bare text needs.
pub fn spacing(margin: bevy_gooey::Sides<bevy_gooey::Space>) -> bevy_gooey::Spacing {
    bevy_gooey::Spacing { margin, padding: default() }
}

/// **The classes a row helper's row carries**: `chrome-row-layout`, the helper's own modifier (`chrome-row-gapped`, `-wrapping`,
/// `-rigid`, `-end`) and, when it has one, the [`Gap`]'s class — so a sheet rule can style every helper row, one kind, or one rhythm.
fn row_classes(kind: &str, gap: Option<Gap>) -> bevy_gooey::StyleClass {
    let gap = gap.map_or("", Gap::name);
    bevy_gooey::StyleClass::of(&format!("chrome-row-layout chrome-row-{kind} {gap}"))
}

/// **A gooey row with room around it** (UI-59, #245): [`bevy_gooey::row_with`] with no gap of its own, the margin outside it and
/// its cells on `align` across. `.insert(..)` a marker on the returned commands, or `.with_children(..)` the row's cells.
pub fn spaced_row<'a>(parent: &'a mut ChildSpawnerCommands, margin: bevy_gooey::Sides<bevy_gooey::Space>, align: AlignItems) -> EntityCommands<'a> {
    gapped_row(parent, None, margin, align)
}

/// **A [`spaced_row`] whose cells stand a [`Gap`] apart** (the sheet's rule for the gap's class).
pub fn gapped_row<'a>(parent: &'a mut ChildSpawnerCommands, gap: Option<Gap>, margin: bevy_gooey::Sides<bevy_gooey::Space>, align: AlignItems) -> EntityCommands<'a> {
    parent.spawn((
        bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(align), ..default() }),
        bevy_gooey::Spacing { margin, padding: default() },
        row_classes("gapped", gap),
    ))
}

/// **A gooey row that wraps its cells onto a second line instead of running off the panel's edge** (UI-59, #245): the cells
/// stand a [`Gap`] apart, and so do the lines (a gap is both a row's and a column's), and the row may narrow below its widest run,
/// which a gooey row always may, so a last chip is never clipped by the panel (the 2026-09-03 audit's F7).
pub fn wrapping_row<'a>(parent: &'a mut ChildSpawnerCommands, gap: Option<Gap>, margin: bevy_gooey::Sides<bevy_gooey::Space>, align: AlignItems) -> EntityCommands<'a> {
    parent.spawn((
        bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(align), wrap: Some(FlexWrap::Wrap), ..default() }),
        bevy_gooey::Spacing { margin, padding: default() },
        row_classes("wrapping", gap),
    ))
}

/// **A [`gapped_row`] that is never squeezed** (#260): the cells stand a [`Gap`] apart on `align` across, and the row keeps its height when
/// the panel is shorter than its rows (a flex item that may shrink is made shorter than its text, which draws one line over the next).
pub fn rigid_row<'a>(parent: &'a mut ChildSpawnerCommands, gap: Option<Gap>, align: AlignItems) -> EntityCommands<'a> {
    parent.spawn((bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(align), shrink: Some(0.0), ..default() }), row_classes("rigid", gap)))
}

/// **A [`gapped_row`] whose cells gather at the right edge** (#260): a dialog's answers.
pub fn end_row<'a>(parent: &'a mut ChildSpawnerCommands, gap: Option<Gap>) -> EntityCommands<'a> {
    parent.spawn((bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { justify: Some(JustifyContent::FlexEnd), ..default() }), row_classes("end", gap)))
}

/// **A text that takes the rest of its row and wraps inside it** (UI-59, #245): a label column that grows with the row and
/// may narrow below its words, so a long name wraps in its own column instead of widening the row past the panel.
///
/// A flex item's automatic minimum size is its content, so without the zero floor the column refuses to narrow and the
/// row's last cell is clipped by the panel's edge (the 2026-09-03 audit's F7). Returns the commands so a caller can mark it.
///
/// **A word longer than the column breaks at a letter** (`LineBreak::WordOrCharacter`, as gooey's own `fill_text` does). Bevy's default
/// breaks only between words, so a file name such as `surveillance_console.glb` (one word) kept its whole width inside a box narrower
/// than itself and was drawn out past the panel's border (#468).
pub fn fill_text<'a>(row: &'a mut ChildSpawnerCommands, text: impl Into<String>, ink: Color, role: text::Role) -> EntityCommands<'a> {
    row.spawn((
        text_narrows(),
        Text::new(text.into()),
        TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
        TextColor(ink),
        typed(role),
    ))
}

/// **A fixed-width column holding one text** (UI-59, #245): gooey's `cell`, whose width is in body text sizes, so the px it stands for is
/// divided by the body type size and the column grows with the person's text size instead of falling short of it. A glyph column
/// (`chrome-col-tight`: a severity mark, a fold chevron) is the case. The cell cuts what does not fit rather than running onto its neighbour.
pub fn text_cell(row: &mut ChildSpawnerCommands, width_px: f32, text: impl Into<String>, ink: Color, role: text::Role) {
    row.spawn(bevy_gooey::cell(width_px / text::BODY.px())).with_children(|cell| {
        cell.spawn((Text::new(text.into()), TextColor(ink), typed(role)));
    });
}

/// **An empty fixed-width gap between two cells of a row** (UI-59, #245): a gooey `cell` holding nothing, sized like [`text_cell`].
pub fn gap_cell(row: &mut ChildSpawnerCommands, width_px: f32) {
    row.spawn(bevy_gooey::cell(width_px / text::BODY.px()));
}

/// **A chip** — one small clickable word. `chrome-chip`'s padding and a permanent 1 px border whose COLOUR
/// asks the questions (a ghost proposal lights it `Ink::suggest`; everything else runs [`Color::NONE`]),
/// so a chip never changes size when its state does.
///
/// **`marker` must not contain a `Node`, and the failure is a panic rather than a warning.** This
/// builder spawns one, and a bundle carrying two of the same component panics at spawn in Bevy 0.20
/// naming the component and not the call site — the trap the repo's `CLAUDE.md` already records for
/// `button_visual()`. To adjust the node, `.insert(Node { .. })` on the returned `EntityCommands`,
/// which overrides rather than duplicates. The same holds for [`list_row`], [`quiet_row`] and
/// [`text_field`], which each carry a `Node` of their own.
pub fn chip<'a>(
    parent: &'a mut ChildSpawnerCommands,
    marker: impl Bundle,
    label: &str,
    role: text::Role,
    ink: Color,
    fill: Color,
    border: Color,
) -> EntityCommands<'a> {
    let mut c = parent.spawn((
        bevy::ui_widgets::Button,
        Hovered::default(),
        // **A chip answers the pointer, like every row does.** It carried `Hovered` from the day it
        // was written — but only as a hit-test, for the "is the pointer over UI" question, and
        // nothing ever painted it. The 2026-08-17 audit found that as its seventh defect: *"hover
        // exists as a hit-test everywhere and as feedback almost nowhere"*, and the tag block is 55
        // clickable things that gave no sign of being clickable. `RowRest` is the whole fix —
        // `style_list_rows` already keys on it and has never needed to know what it is painting.
        RowRest(fill),
        marker,
        // **A chip narrows before the row wraps it to a second line** (#526, owner's rule: *"shrink the
        // buttons, then, when they can't shrink more due to the text, then make it a new row"*). Its
        // label breaks at a word (`LineBreak::WordOrCharacter` below), so a narrowed chip reflows inside
        // itself rather than cutting its word — which is what the `the_terminal_bar_keeps_every_chip_whole`
        // sweeps actually protect, and they pass 3/3 with this. The old `0.0` here was #494's fixed-target
        // rule; #526 supersedes it and `the_g87_nodes_are_gooeys` moves with the contract.
        held_box(
            "chrome-chip",
            bevy_gooey::Sides::all(crate::style::metrics().layout.edge_w),
            Some(1.0),
            bevy_gooey::Sizing { max_width: Some(bevy_gooey::Extent::Percent(100.0)), ..default() },
        ),
        BorderColor::all(border),
        BackgroundColor(fill),
    ));
    c.with_children(|chip| {
        chip.spawn((
            Text::new(label.to_owned()),
            TextColor(ink),
            typed(role),
            // **The label's least width is its longest word** (#494) — the mechanism #481 used for a
            // switch's label. A `NoWrap` label's min-content width is its whole counted phrase, so
            // `NOT IMPORTED (4)` held its chip 379 px wide in a 319 px row: the `FlexWrap` above had
            // nothing to wrap, because its one child was already wider than a line. Breaking at a
            // word drops that floor to `IMPORTED`, so the box narrows to the line it is given and the
            // count drops to a second line rather than painting over the panel's border.
            TextLayout::new(Justify::Left, LineBreak::WordOrCharacter),
        ));
    });
    c
}

/// The fill a list row returns to when the pointer leaves it. Carried on every [`list_row`] so ONE
/// system can give the whole editor's lists their hover state — the rebuilt lists are painted
/// inside change-gated `rebuild_*` systems that never see mouse motion, which is why hover was
/// deferred until the shared builder existed (the 2026-08-17 hover-scope decision).
#[derive(Component, Clone, Copy)]
pub struct RowRest(pub Color);

/// **A selectable list row** — full width, `chrome-chip`'s inset, `Ink::row_selected` when it is the one
/// being acted on, `Ink::row_hover` under the pointer. Returns the row for the caller to fill; what a
/// row holds is the tab's business, that it looks like every other row is not.
pub fn list_row<'a>(
    parent: &'a mut ChildSpawnerCommands,
    selected: bool,
    marker: impl Bundle,
) -> EntityCommands<'a> {
    // `Button` is what puts a row on the editor's `Activate` bus — see [`quiet_row`] for who is
    // deliberately not on it.
    parent.spawn((bevy::ui_widgets::Button, row_shape(selected, "chrome-row"), marker))
}

/// **The same row, not on the `Activate` bus.**
///
/// # Why this exists, which is worth reading before merging the two back together
///
/// This editor has **many global `Activate` observers**, and most take a Map-door resource:
/// `on_row_click` wants `Res<Project>`, `on_tag_chip` wants `ResMut<Project>`. A global observer
/// fires for *any* `Activate` anywhere in the application, and in Bevy 0.20 a missing `Res<T>`
/// **panics** rather than skipping — so the first click on a `Button` outside the editor takes the
/// whole application down.
///
/// That was invisible while `list_row` was only ever called by editor panels. The moment the menu
/// adopted the shared row vocabulary, its first click panicked: `Res<Project>` does not exist on
/// `Screen::Project`. The observers that take a Map-door resource directly are each the same landmine
/// for the next caller, so the count is not the point and is not kept here: `grep -rn 'On<Activate>'`
/// is the measurement, and it moves with every panel added.
///
/// So a row that brings its own click handling says so, and stays off the bus. It keeps everything
/// that makes a row *look* like a row — [`RowRest`], `Hovered`, the fill, the padding — and
/// `style_list_rows` repaints it exactly the same, because that system keys on `RowRest` and has
/// never needed `Button`.
/// **How tall a cut row is, as a multiple of its own text size** — the console's own 1.4.
const CUT_LINE_FACTOR: f32 = 1.4;

/// **One line of text, cut at the edge**: a fixed line height with whatever overflows it clipped.
///
/// gooey's `Sizing` carries both (`height` and `clip`), so this is not a raw `Node` — which is what
/// the raw-node ratchet requires, and what the first version of this helper got wrong.
#[must_use]
pub fn clipped_one_line() -> impl Bundle {
    bevy_gooey::Sizing {
        height: Some(bevy_gooey::Extent::Px(text::LABEL.px() * CUT_LINE_FACTOR)),
        clip: Some(true),
        ..default()
    }
}

pub fn quiet_row<'a>(
    parent: &'a mut ChildSpawnerCommands,
    selected: bool,
    marker: impl Bundle,
) -> EntityCommands<'a> {
    parent.spawn((quiet_row_shape(selected), marker))
}

/// **What [`quiet_row`] spawns**, for a row built where there are no `Commands` (an overlay spawned on the `World`).
pub fn quiet_row_shape(selected: bool) -> impl Bundle {
    row_shape(selected, "chrome-row chrome-row-quiet")
}

/// **Whether a row is the chosen one.** Carried rather than baked into the fill alone, because since
/// 2026-09-03 selection is a fill *and* a rail and the two must not be able to disagree.
#[derive(Component, Clone, Copy)]
pub struct RowSelected(pub bool);

/// What a list row looks like, shared by [`list_row`] and [`quiet_row`] so the two cannot drift into
/// two row shapes — which is the drift this whole module exists to stop.
///
/// **The rail is a left border, not a child node**, and the width is reserved whether or not the row
/// is selected — so selecting a row cannot move its text sideways by two pixels, which is what an
/// added child or a conditional border would do to a list the author is reading down.
fn row_shape(selected: bool, classes: &str) -> impl Bundle {
    let rest = if selected { ink().row_selected } else { ink().row_bg };
    (
        Hovered::default(),
        RowRest(rest),
        RowSelected(selected),
        held_box(
            classes,
            bevy_gooey::Sides { left: Some(crate::style::metrics().layout.select_rail_w), ..default() },
            None,
            bevy_gooey::Sizing { width: Some(bevy_gooey::Extent::Percent(100.0)), ..default() },
        ),
        BorderColor::all(if selected { ink().accent } else { Color::NONE }),
        BackgroundColor(rest),
    )
}

/// The row's non-background parts, as [`style_list_rows`] reads them.
type RowState = (
    &'static RowRest,
    &'static Hovered,
    Has<bevy::ui::Pressed>,
    Has<bevy::ui::InteractionDisabled>,
);

/// **The five states, once, for every row / chip / button in the editor.**
///
/// Disabled beats pressed beats selected beats hover beats rest. Before 2026-09-03 the middle two
/// did not exist: rest, hover and selected were the whole machine, so a control that ran a slow verb
/// looked identical while it ran and a control that could not be used looked exactly like one that
/// could.
///
/// **Why the mouse button is read here rather than trusting `Pressed`.** `bevy_ui::Pressed` is
/// maintained by `bevy_ui_widgets::ButtonPlugin`'s observers, which match `With<Button>`
/// (`bevy_ui_widgets-0.20.0/src/button.rs:41`) — and [`quiet_row`] deliberately has no `Button`,
/// because a global `Activate` observer taking a door's resource panics on the menu. So a quiet row
/// would have been the one shape in the editor that never acknowledged a click. Hover plus a held
/// primary button is the same fact one layer down, and it is what the author actually did.
/// **The colours a list row takes from the skin**, in one place, so a test can ask what the editor
/// would paint without a pointer it cannot place (UI-02, #210).
///
/// `Hovered` is bevy's `Immutable` picking component: only the picker may write it, and a headless app
/// has none, so a row's hover colour cannot be observed by hovering. What *can* be observed is the
/// expression `style_list_rows` paints from, which is this — one function, called by the system and
/// by the test alike, so an assertion here is an assertion about the row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowFills {
    pub rest_panel: Color,
    pub hover: Color,
    pub pressed: Color,
}

impl RowFills {
    /// From the running skin when there is one, the constants when there is not (a headless harness
    /// that never ran `EditorUiPlugin`).
    pub fn of(skin: Option<&bevy_gooey::GooeySkin>) -> RowFills {
        match skin {
            Some(s) => RowFills {
                rest_panel: s.surface,
                hover: s.hover.unwrap_or(ink().row_hover),
                pressed: s.pressed.unwrap_or(ink().row_pressed),
            },
            None => RowFills { rest_panel: ink().panel_bg, hover: ink().row_hover, pressed: ink().row_pressed },
        }
    }
}

/// [`RowFills`] for the world as it stands.
pub fn row_fills(world: &World) -> RowFills {
    RowFills::of(world.get_resource::<bevy_gooey::GooeySkin>())
}

fn style_list_rows(
    mouse: Res<ButtonInput<MouseButton>>,
    // **The rows' states come from the skin** (UI-02, #210). This is the one chokepoint every list
    // row's fill passes through — `style_list_rows` keys on `RowRest`, so a row spawned by any tab is
    // painted here — which makes it the first place the editor's own chrome can read `GooeySkin`
    // rather than the constants beside it. The constants stay for the callers that have not moved
    // yet (131 sites outside this module); what changes is that a *theme edit* now reaches every row
    // in the editor, which is the thing #210 exists for.
    skin: Option<Res<bevy_gooey::GooeySkin>>,
    mut rows: Query<(RowState, &mut BackgroundColor)>,
) {
    let fills = RowFills::of(skin.as_deref());
    let (panel, pressed_col, hover_col) = (fills.rest_panel, fills.pressed, fills.hover);
    // gooey has no `selected` role, so a selected row keeps the editor's own colour — the honest
    // half-measure until that constant group moves.
    let selected_col = ink().row_selected;
    let held = mouse.pressed(MouseButton::Left);
    for ((rest, hovered, pressed, disabled), mut bg) in &mut rows {
        let want = if disabled {
            // Not a fill of its own: a disabled control drops back to the panel it sits on, so it
            // reads as part of the surface rather than as a thing that is merely quiet today.
            panel
        } else if pressed || (hovered.0 && held) {
            pressed_col
        } else if rest.0 == ink().row_selected {
            selected_col
        } else if hovered.0 {
            hover_col
        } else {
            rest.0
        };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// **What a control does, which is the half its shape cannot say.**
///
/// Shape says *kind* — a [`chip`] toggles, a [`button`] happens — and this says *severity*, which is
/// the 2026-09-03 split. It has exactly three values on purpose: before it, `clear this cell` was
/// red, `rescan mesh` was amber and every other verb was grey, with no rule between them, so red
/// meant "destructive or expensive or the author felt strongly" and therefore meant nothing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The ordinary case. Most verbs.
    Plain,
    /// The one obvious thing to do here — a single button per block at most.
    Primary,
    /// **Throws work away.** The only thing in this editor that is red.
    Destructive,
}

impl Severity {
    /// The word's ink. The *fill* is `Ink::row_bg` in every case: a filled red button in a panel of
    /// grey ones is a stop sign, and most destructive verbs here are ordinary editing.
    /// Public because `confirm`'s card is built once and repainted per question: which button is
    /// destructive depends on which question is up, so the mapping has to be readable from outside
    /// without being restated outside.
    pub fn ink(self) -> Color {
        match self {
            Severity::Plain => ink().text,
            Severity::Primary => ink().accent,
            Severity::Destructive => ink().danger,
        }
    }
}

/// **A command button** — a control that makes something happen, as opposed to a [`chip`], which is
/// a control that is on or off.
///
/// The two were the same box until 2026-09-03, in the same row, in the same panel: on the Meshes tab
/// the tag chips (toggles), `solid`/`edge`/`clear` (cell commands) and `rescan mesh` (an expensive
/// action) all rendered as one grey rectangle. The only way to learn which was which was to press
/// one. So a button is now visibly heavier — `chrome-button`'s padding rather than `chrome-chip`'s, and a lit
/// `Ink::panel_edge` border where a chip's border is `Color::NONE` unless it has something to say.
///
/// **`marker` must not contain a `Node`** — see [`chip`] for why that is a panic rather than a
/// warning.
pub fn button<'a>(
    parent: &'a mut ChildSpawnerCommands,
    marker: impl Bundle,
    label: &str,
    severity: Severity,
) -> EntityCommands<'a> {
    let mut b = parent.spawn((
        bevy::ui_widgets::Button,
        Hovered::default(),
        RowRest(ink().row_bg),
        marker,
        held_box("chrome-button", bevy_gooey::Sides::all(crate::style::metrics().layout.edge_w), Some(0.0), bevy_gooey::Sizing::default()),
        BorderColor::all(ink().panel_edge),
        BackgroundColor(ink().row_bg),
    ));
    b.with_children(|b| {
        b.spawn((
            Text::new(label.to_owned()),
            TextColor(severity.ink()),
            typed(text::CONTROL),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
        ));
    });
    b
}

// ── the overlay stack, in one place ──────────────────────────────────────────────────────────────
//
// Eight overlays shipped with five z-values and **two with none at all** — the session journal and
// the problem toast, which therefore stacked by spawn order against siblings that had opinions.
// Written as numbers at five call sites in four files, there was nothing to read to find out what
// was in front of what; written here, the list *is* the answer and `every_overlay_declares_its_z`
// fails a build that adds a sixth number somewhere else.
//
// Read it top-down: a question you must answer is in front of the keys that teach you the
// application, which are in front of a readout you glance at.

// The numbers are the style sheet's `metrics.z` (`crate::style::ZMetrics`), in this order, lowest first:
// `balloon` (a speech balloon over a piece, #106: in front of the viewport, behind every panel), `compass`
// (a persistent readout, never the thing you look at; below gooey's dialog layer, 100), `terminal_maximised`
// (#435: above the map's overlays, below the dialogs and every notice), `toast` (above a dialog's scrim, as
// gooey stands its own stack), `notice` (the session journal and the problem toast, which carried no z until
// 2026-09-03), `menu_overlay`, `badge` (the key badges: below a modal, which is exactly when they must not
// cover its answer), `modal` (in front of everything, because nothing else is actionable) and `report`.
/// **How wide a [`text_field`] is**, as the style-sheet class that says so.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldWidth {
    /// As wide as its readout.
    Hug,
    /// All of its row (`.chrome-field-fill`).
    Fill,
    /// **The box a number is typed into** (`.chrome-field-number`): the widest thing typed into one is `12.34`
    /// plus the caret, measured at the body size (2026-09-03, when 56 in one file and 62 in another were one idea).
    Number,
}

impl FieldWidth {
    fn classes(self) -> &'static str {
        match self {
            FieldWidth::Hug => "chrome-field",
            FieldWidth::Fill => "chrome-field chrome-field-fill",
            FieldWidth::Number => "chrome-field chrome-field-number",
        }
    }
}

/// **A text field's box** — the sheet's `.chrome-field` floor and inset, `Ink::row_bg` fill, with its
/// readout text spawned inside. The audit found this shape six times with three paddings.
pub fn text_field<'a>(
    parent: &'a mut ChildSpawnerCommands,
    width: FieldWidth,
    field: impl Bundle,
    role: text::Role,
    initial: (String, Color),
    readout: impl Bundle,
) -> EntityCommands<'a> {
    let mut boxed = parent.spawn((
        bevy::ui_widgets::Button,
        Hovered::default(),
        field,
        // The field's box (G-87): a gooey row that never shrinks, its width, least height and inset the sheet's
        // `.chrome-field` (and `-fill` or `-number`). Its fill is the editor's (`InkFollow`), `Ink::row_bg`.
        bevy_gooey::sized(bevy_gooey::Sizing::default()),
        bevy_gooey::RowLayout { direction: Some(FlexDirection::Row), shrink: Some(0.0), ..default() },
        bevy_gooey::StyleClass::of(width.classes()),
        bevy_gooey::InkFollow,
        BackgroundColor(ink().row_bg),
    ));
    boxed.with_children(|f| {
        f.spawn((
            Text::new(initial.0),
            TextColor(initial.1),
            typed(role),
            readout,
        ));
    });
    boxed
}

/// **What a field's readout shows**: the live keystrokes with a caret while focused, the committed
/// value otherwise. The `{raw}_` + `Ink::accent` idiom was written five times before it had one home;
/// the caret is what makes "empty because you cleared it" distinguishable from "empty".
pub fn field_text(editing: Option<&str>, idle: (String, Color)) -> (String, Color) {
    match editing {
        Some(raw) => (format!("{raw}_"), ink().accent),
        None => idle,
    }
}

/// **One severity's tint and word, for every rail in the editor.** Two tabs printed the same words
/// under forked maps (tiles: Warn→ACCENT, Note→DIM; anim: everything-not-blocking→LABEL) — same
/// vocabulary, three colours, which is the drift this module exists to stop. The word and the hue
/// travel together so no site can pair "worth checking" with the wrong ink.
pub fn severity_style(severity: blueprint_model::import::Severity) -> (Color, &'static str) {
    match severity {
        blueprint_model::import::Severity::Blocking => (ink().danger, "blocking"),
        blueprint_model::import::Severity::Warn => (ink().accent, "worth checking"),
        blueprint_model::import::Severity::Note => (ink().dim, "note"),
    }
}

/// **A severity rail** — the tinted left border that makes a finding's weight visible before it is
/// read. 2 px (the tiles dialect; the decision of 2026-08-17), [`STEP_TIGHT`] breathing room, a
/// [`STEP_ROW`] gap to the next. Returns the block for the caller to fill — the severity word first,
/// by convention, in the same tint. A clickable rail (anim's jump rows) adds `Button`/`Hovered`
/// and a fill through `marker`.
pub fn severity_rail<'a>(
    parent: &'a mut ChildSpawnerCommands,
    tint: Color,
    marker: impl Bundle,
) -> EntityCommands<'a> {
    parent.spawn((
        // A column that does not give way to its neighbours, with a 2 px rail along its left, 7 px (not a step of the scale) in from it, a tight step above and below and a row step under.
        bevy_gooey::column_with(0.0, bevy_gooey::RowLayout { shrink: Some(0.0), ..default() }),
        bevy_gooey::BorderPx(bevy_gooey::Sides { left: Some(2.0), ..default() }),
        bevy_gooey::InkOverride { edge: Some(tint), ..default() },
        bevy_gooey::Spacing {
            padding: bevy_gooey::Sides { top: Some(STEP_TIGHT), bottom: Some(STEP_TIGHT), left: Some(bevy_gooey::Space::Px(7.0)), ..default() },
            margin: margin_bottom(STEP_ROW),
        },
        BorderColor::all(tint),
        marker,
    ))
}

/// **The name box** — the centred prompt for naming a new composition.
///
/// `M` on the Map keeps the set in hand as one composition, and this is where it is named. It used to
/// be a field in the Map's status readout, which put the question in the corner of the screen while
/// the whole screen waited for it.
///
/// Two tabs asked it when Compose could also make a composition. That is why this is a shared widget
/// painted by `paint_name_box` rather than a Map-local one: authoring collapsed onto the Map, and a
/// widget that survives losing one of its two callers is cheaper than one that has to be rebuilt if a
/// second ever returns.
#[derive(Component)]
pub struct NameBox;

#[derive(Component)]
struct NameBoxTitle;

#[derive(Component)]
struct NameBoxValue;

#[derive(Component)]
struct NameBoxHint;

fn spawn_name_box(mut commands: Commands) {
    commands
        .spawn((
            NameBox,
            // **A full-screen container without this eats every world click**: gooey's `cover` is `Pickable::IGNORE`, so a click while naming
            // falls through to the stage, which is right: the box is a prompt, not a modal that has to be dismissed before anything else works. Its card is centred in it,
            // and it is hidden through `Shown` until a tab asks a name (`paint_name_box`). The dim is the editor's own (`InkFollow`), not the skin's scrim.
            bevy_gooey::cover(),
            bevy_gooey::RowLayout { align: Some(AlignItems::Center), justify: Some(JustifyContent::Center), ..default() },
            bevy_gooey::Shown(false),
            bevy_gooey::InkFollow,
            // Above the panels and the tab strip (101), and below the key badges and the confirm
            // prompt — the tier for "the screen is asking you something", which outranks a panel and
            // is outranked by a question you have to answer (`metrics.z.menu_overlay`).
            GlobalZIndex(crate::style::metrics().z.menu_overlay),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
        ))
        .with_children(|p| {
            p.spawn((
                // The dialog card: a column whose gap, inset and least width are the sheet's `.chrome-dialog-card`, in the editor's panel colour.
                bevy_gooey::column(0.0),
                bevy_gooey::StyleClass::of("chrome-dialog-card"),
                bevy_gooey::InkFollow,
                BackgroundColor(ink().panel_bg),
                // **The dialog itself is UI; the dimmed backdrop behind it is not.**
                //
                // "Is the pointer over UI" is asked everywhere as "is any `Hovered` true"
                // (`view::drive`, `drive_place`, `compose::pick_slot`), and this panel carried no
                // `Hovered` — so scrolling over a visible, open dialog zoomed the world behind it.
                //
                // This narrows the root's deliberate `Pickable::IGNORE` rather than undoing it. The
                // root stays click-through, because a prompt is not a modal that has to be dismissed
                // before anything else works; the 360 px box stops being, because a click landing *on*
                // the dialog and placing a piece behind it is the same bug as the scroll.
                //
                // Not `Pickable::IGNORE` here — that would make it unhoverable and reopen the hole.
                // `Hovered` is true for an entity or any descendant, so one on this panel answers for
                // the title, the value and the hint.
                Hovered::default(),
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new(String::new()),
                    typed(text::BODY),
                    TextColor(ink().label),
                    NameBoxTitle,
                ));
                b.spawn((
                    Text::new(String::new()),
                    typed(text::TITLE),
                    TextColor(ink().accent),
                    NameBoxValue,
                ));
                b.spawn((
                    Text::new(String::new()),
                    typed(text::BODY),
                    TextColor(ink().dim),
                    NameBoxHint,
                ));
            });
        });
}

/// One of the name box's three text lines, narrowed by the filter that tells them apart.
type NameLine<'w, 's, F> = Query<'w, 's, &'static mut Text, F>;

/// **Show the live tab's name field, or nothing.**
///
/// Reads the Map's own state rather than a shared projection, and formats the value the way the Map
/// commits it — snake_case as you type. Relocating the prompt must not quietly change what is saved.
fn paint_name_box(
    editor: Res<crate::editor::EditorState>,
    build: Res<crate::build::Build>,
    compose: Res<crate::compose::ComposeState>,
    mut roots: Query<&mut bevy_gooey::Shown, With<NameBox>>,
    mut titles: NameLine<'_, '_, (With<NameBoxTitle>, Without<NameBoxValue>, Without<NameBoxHint>)>,
    mut values: NameLine<'_, '_, (With<NameBoxValue>, Without<NameBoxTitle>, Without<NameBoxHint>)>,
    mut hints: NameLine<'_, '_, (With<NameBoxHint>, Without<NameBoxTitle>, Without<NameBoxValue>)>,
) {
    // **`grouping.is_some()` and the box being visible are the same condition**, and that is the
    // invariant rather than a description of the code.
    //
    // This used to also match on `Mode::Map`, which looked like a harmless guard and was not:
    // `EditorState::grouping` has no mode scope, so clicking the tab strip mid-name hid the box while
    // `group_name_keys` kept consuming the keyboard — every keystroke vanished until `Esc`, with
    // nothing on screen to say where they were going. Two conditions for one question is what made
    // that state reachable.
    //
    // The tab switch now clears `grouping` (see `tiles::leaving_a_tab_puts_the_name_prompt_down`), so
    // there is one owner and this reads it. The prompt is the Map's, and if a second tab ever asks
    // again the answer is another field, not another condition on this one.
    // **Two fields, one box** — the Map names a composition, the Tiles tab names a tile, and each
    // owns its own `Option<String>`. That is what this function's own note asked for when the
    // second one arrived: *"another field, not another condition on this one."* They cannot both be
    // open, because the tabs are never live together, and the title says which is being answered.
    let asking: Option<(&str, String, String)> = editor
        .grouping
        .as_ref()
        .map(|raw| {
            (
                "NAME THIS COMPOSITION",
                // Forced to snake_case as it is typed, so the naming rule teaches itself.
                format!("{}_", blueprint_model::naming::to_snake_case(raw)),
                "Enter keeps it.   Esc leaves the set in hand.".to_owned(),
            )
        })
        .or_else(|| {
            build.naming.as_ref().map(|prompt| {
                (
                    "NAME THIS TILE",
                    format!("{}_", blueprint_model::naming::to_snake_case(&prompt.raw)),
                    "Enter opens it.   Esc leaves things as they are.".to_owned(),
                )
            })
        })
        .or_else(|| {
            // **The COMPOSE tab's own question.** A third field, not a third condition on one of
            // the two above — this function's own note asked for exactly that: *"if a second tab
            // ever asks again the answer is another field, not another condition on this one."* The
            // tabs are never live together, so the chain picks the one that is asking.
            compose.renaming.as_ref().map(|rename| {
                (
                    "RENAME THIS COMPOSITION",
                    // **The same spelling the commit will write** — `to_id`, not `to_snake_case`,
                    // because a composition's id may carry a `/` namespace. A field that shows one
                    // spelling and writes another is a lie the author finds out about after the
                    // rename.
                    format!("{}_", blueprint_model::naming::to_id(&rename.raw)),
                    "Enter keeps it, and every map that stamps it follows.   Esc leaves it alone."
                        .to_owned(),
                )
            })
        });
    let shown = asking.is_some();
    for mut root in &mut roots {
        if root.0 != shown {
            root.0 = shown;
        }
    }
    let Some((title, value, hint)) = asking else {
        return;
    };
    // Guarded against the no-op write, like `editor::refresh_status`: this runs every frame and
    // `Text` is change-detected, so writing an unchanged string would re-lay the box continuously.
    // Three separate loops rather than one over a tuple: the queries have different filters, so they
    // are different types and cannot share an array.
    //
    // **It takes `&mut Mut<Text>`, not `&mut Text`.** Coercing a `Mut<Text>` down to `&mut Text` at
    // the call site runs `Mut::deref_mut`, which calls `set_changed()` — so the string was compared
    // and the component was dirtied anyway, every frame, which is exactly what the comment above
    // says this guard exists to stop. Reading `text.0` through `&mut Mut<_>` uses `Deref`; only the
    // assignment reaches `DerefMut`.
    fn set(text: &mut Mut<Text>, want: &str) {
        if text.0 != want {
            text.0 = want.to_owned();
        }
    }
    for mut t in titles.iter_mut() {
        set(&mut t, title);
    }
    for mut t in values.iter_mut() {
        set(&mut t, &value);
    }
    for mut t in hints.iter_mut() {
        set(&mut t, &hint);
    }
}


/// **The window's own furniture** — the frame, the chrome bar, the status band, the name box, and
/// the hover every list row shares.
///
/// The key list used to live here too, as a centred two-column table behind a scrim. It is
/// `crate::badges` now, and the argument for moving it is worth keeping where the deletion happened:
/// a table asks you to map a phrase back onto something on screen, and two verbs that had been bound
/// all along (`R`, `Shift+Delete`) stayed invisible for two sessions because their phrase was
/// collapsed onto one row. Drawing the chord *on the thing it acts on* removes that step.
///
/// What survives the move is the line that says the key exists. That line is not optional: Cockburn,
/// Gutwin, Scarr & Malacria 2014 (`10.1145/2659796`, already cited by `crate::keys`) document the
/// intermodal-transition failure — a fast path offered *beside* a slow one does not get adopted on
/// its own — and ExposeHK names its own weakness as exactly this, that an overlay behind a modifier
/// *"has no visual representation to aid discovery until the user … presses its trigger."* See
/// [`shortcut_hint`].
/// **Adopt the left dock and the viewport into the shell split's panes** (UI-30, #224).
///
/// **Both panes must hold a region before the split has a size at all.** Measured: with the split spawned and
/// nothing parented into it, the panes are 0×0 and the viewport gets no room. `split` spawns
/// `[pane, divider, pane]`; the panes carry a `Node` and a `Role` and no `Pickable::IGNORE`, and the divider
/// carries `Hovered` — all three forbidden under the frame, so both are corrected here.
///
/// **While the split is collapsed, the divider has its `Hovered` back, and the first pane has its own** (UI-R06, #336).
/// gooey's peek of a collapsed pane (#69) reads the pointer on the divider (the collapsed edge) and on the pane, and puts
/// `Hovered` on the first pane for as long as the split is collapsed: that is what lets the pane be pointed at, so
/// `Pickable::IGNORE` stays on a pane without it (an open split's panes must never mark the world as interface) and
/// comes off the one that has it, so a floating pane's dock can be clicked. A collapsed split's divider is a sliver on
/// the viewport's edge that is the interface, so it answers the pointer there; an open split's does not.
///
/// **Idempotent**, because the frame is spawned on each door.
fn adopt_the_shell_split(
    mut commands: Commands,
    frame: Option<Res<Frame>>,
    splits: Query<(Entity, Has<bevy_gooey::SplitSaved>), With<ShellSplit>>,
    roles: Query<&bevy_gooey::Role>,
    has_hover: Query<(), With<Hovered>>,
    pickable: Query<&bevy::picking::Pickable>,
    tree: (Query<&Children>, Query<&ChildOf>),
) {
    let (children, parents) = tree;
    let Some(frame) = frame else { return };
    for (split, collapsed) in &splits {
        let Some(kids) = children.get(split).ok() else { continue };
        for child in kids.iter() {
            match roles.get(child) {
                Ok(bevy_gooey::Role::SplitDivider) => {
                    if has_hover.get(child).is_ok() != collapsed {
                        if collapsed {
                            commands.entity(child).insert(Hovered::default());
                        } else {
                            commands.entity(child).remove::<Hovered>();
                        }
                    }
                }
                Ok(bevy_gooey::Role::SplitPane) => {
                    let want = if has_hover.get(child).is_ok() {
                        bevy::picking::Pickable::default()
                    } else {
                        bevy::picking::Pickable::IGNORE
                    };
                    if pickable.get(child).ok() != Some(&want) {
                        commands.entity(child).insert(want);
                    }
                }
                _ => {}
            }
        }
        let panes: Vec<Entity> = kids
            .iter()
            .filter(|e| matches!(roles.get(*e), Ok(bevy_gooey::Role::SplitPane)))
            .collect();
        let [first, second] = panes[..] else { continue };
        for (pane, region) in [(first, frame.left), (second, frame.viewport)] {
            if parents.get(region).map(|p| p.parent()) == Ok(pane) {
                continue;
            }
            commands.entity(region).insert(ChildOf(pane));
        }
    }
}

/// **Give the left pane the width of the panel in it** (#282, #283).
///
/// The split was spawned with `SplitFraction(0.22)`, which at a 1280 px window is a 204 px pane
/// around a panel that asks for `layout.controls_w` 380 (456 on screen). The pane clips what overflows it
/// and the viewport is drawn over the rest, so the right half of every control in the inspector was
/// cut off or unreachable — `ENVIRONMENT`'s `18:30` drew half a button, `23:00` none, and a click at
/// `23:00`'s own reported centre landed on the viewport (measured on big, 2026-11-03: the pointer's
/// `HoverMap` entry was the viewport node).
///
/// **Follow the panel until the person moves the divider** — a window that resizes, or a second
/// dock that is laid out a frame after the first, changes the width the panel was squeezed to, and a
/// pane sized once would keep the first answer (measured: 386 px against the 349 the settled
/// widths ask for). This runs while [`SplitUnsized`] is on the split, which the frame spawns with
/// it, remembering what it wrote; a fraction that is no longer what it wrote is a drag, an arrow
/// key or a game setting, and the marker comes off for good. The width is the widest visible left
/// panel **as [`fit_docks_to_the_window`] sized it** plus the margin the panel carries
/// (`2 * MARGIN`, the same overhead that system measures), over the split's room in the same
/// `Val::Px` units, so a narrow window's squeezed panel gets a pane squeezed to match.
fn size_the_shell_split(
    mut commands: Commands,
    mut splits: Query<(Entity, &mut bevy_gooey::SplitFraction, &mut SplitUnsized, &ComputedNode, &Children)>,
    dividers: Query<&ComputedNode, With<bevy_gooey::SplitDivider>>,
    panels: Query<(&PanelWidth, &Node, &bevy_gooey::Sizing, &ComputedNode)>,
    windows: Query<(), With<bevy::window::PrimaryWindow>>,
) {
    // **No window, no room to answer for**: [`fit_docks_to_the_window`] returns here too, so a panel's
    // width is still its preferred one and a pane sized to it would be the wrong pane.
    if windows.is_empty() {
        return;
    }
    let mut want = 0.0f32;
    for (panel, node, sizing, computed) in &panels {
        if panel.side != Side::Left || node.display == Display::None || computed.size().x <= 0.0 {
            continue;
        }
        if let Some(bevy_gooey::Extent::Px(w)) = sizing.width {
            want = want.max(w);
        }
    }
    if want <= 0.0 {
        return;
    }
    for (split, mut fraction, mut sized, node, kids) in &mut splits {
        // **Someone else wrote the fraction since this did**: a drag, an arrow key, a game setting.
        // It is theirs from now on.
        if sized.wrote.is_some_and(|w| w != fraction.0) {
            commands.entity(split).remove::<SplitUnsized>();
            continue;
        }
        let bar = kids
            .iter()
            .find_map(|c| dividers.get(c).ok())
            .map_or(0.0, |d| d.size().x * d.inverse_scale_factor());
        let extent = node.size().x * node.inverse_scale_factor() - bar;
        if extent <= 0.0 {
            continue;
        }
        // The panel's margin each side: a panel's `STEP_PAD` (the sheet's `.chrome-panel`).
        let to = ((want + 2.0 * space(STEP_PAD)) / extent).clamp(0.0, 1.0);
        if fraction.0 != to {
            fraction.0 = to;
        }
        if sized.wrote != Some(to) {
            sized.wrote = Some(to);
        }
    }
}

/// The shell's split while its pane is collapsed, or sliding to or from it.
type CollapsedOrGliding = (With<ShellSplit>, Or<(With<bevy_gooey::SplitSaved>, With<bevy_gooey::SplitGlide>)>);

/// **Once a person drags the divider, the left panel is as wide as the pane they made** (#224).
///
/// Before that, [`size_the_shell_split`] sizes the pane to the panel; after it the pane is the
/// person's, and a panel that kept its own width would be cut off by a narrower pane or float in a
/// wider one — a dock that resizes without its content reflowing is not a dock. The panel's width
/// is the pane's less its margins, never under [`DOCK_MIN_W`] (the pane's own floor, `SplitMin`,
/// keeps the two from disagreeing). [`fit_docks_to_the_window`] leaves the left panel to this once
/// the split is the person's, so one system writes the number.
///
/// **Not while the split is collapsed or sliding** (UI-R06, #336): the pane is shrinking to nothing or growing back, and a panel that
/// followed it would reflow every frame of the slide and be a different width when the peek floats it over the viewport.
/// Collapsed, it keeps the width it had; a restore lands and this takes the number back from the pane, which is that width.
fn the_left_panel_follows_its_pane(
    frame: Option<Res<Frame>>,
    owned: Query<(), (With<ShellSplit>, Without<SplitUnsized>)>,
    parents: Query<&ChildOf>,
    panes: Query<&ComputedNode>,
    mut panels: Query<(&PanelWidth, &mut bevy_gooey::Sizing)>,
    windows: Query<(), With<bevy::window::PrimaryWindow>>,
    moving: Query<(), CollapsedOrGliding>,
) {
    if owned.is_empty() || windows.is_empty() || !moving.is_empty() {
        return;
    }
    let Some(frame) = frame else { return };
    let Ok(pane) = parents.get(frame.left).map(|p| p.parent()) else { return };
    let Ok(node) = panes.get(pane) else { return };
    let wide = node.size().x * node.inverse_scale_factor();
    if wide <= 0.0 {
        return;
    }
    let width = Some(bevy_gooey::Extent::Px((wide - 2.0 * space(STEP_PAD)).max(crate::style::metrics().layout.dock_min_w())));
    for (panel, mut sizing) in &mut panels {
        if panel.side == Side::Left && sizing.width != width {
            sizing.width = width;
        }
    }
}

pub struct ChromePlugin;

impl Plugin for ChromePlugin {
    fn build(&self, app: &mut App) {
        app
            // A wheel over a scrollbar scrolls its area (#283).
            .add_observer(forward_a_wheel_over_a_scrollbar)
            // **The frame first, and everything that puts a panel in it after.** `Res<Frame>` is
            // how `panel_root` finds its dock, and in Bevy 0.20 a missing `Res<T>` panics its
            // system rather than skipping it — so this is an ordering the build must state, not one
            // that happens to hold. Ordering across a set also gets a sync point inserted, which is
            // what makes the deferred `insert_resource` visible to the spawners.
            .add_systems(
                OnEnter(crate::screen::Screen::Editor),
                spawn_frame.in_set(FrameSystems),
            )
            .add_systems(Update, adopt_the_shell_split.after(FrameSystems))
            // **The band's count watches the set in hand** (UI-34, #228): every frame, gated on the
            // resource, because a set is picked up and put down long after the screen is entered.
            // Registering it in the `OnEnter` tuple above would run it once, on the frame the band is
            // spawned, and never again.
            // **Before gooey styles them**, so the frame the kind changes is the frame the mark draws:
            // `apply_skin` restyles off `Changed<StatusKind>`, and a write after it would be seen a
            // frame late (`jobs::refresh_band` is ordered the same way, for the same reason).
            .add_systems(
                Update,
                (show_selection_count, show_problems_count)
                    .before(bevy_gooey::GooeySystems::Style)
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_systems(
                OnEnter(crate::screen::Screen::Editor),
                (
                    spawn_name_box,
                    spawn_chrome_bar,
                    spawn_status_band,
                    problem_toast,
                )
                    .after(FrameSystems),
            )
            // Every list row's hover, in one place — the rows themselves are spawned by
            // change-gated rebuilds that never see mouse motion.
            //
            // **Ungated by screen**, unlike the systems below it: the menu draws rows too, and the
            // query simply matches nothing on a screen that has none. Gating it would be a second
            // place that has to know which screens have lists.
            .add_systems(
                Update,
                (
                    style_list_rows,
                    tag_the_scroll_rails,
                    // **No bar over a pane that is mid-air** (#517): gooey's `update_lists` compares the
                    // last frame's layout, and a pane on a spring is a width that has not caught up with
                    // its contents. After gooey styles so `update_lists` cannot write over it; the frame
                    // the glide is gone this stops, and gooey's rule has the bar back or not.
                    hide_rails_while_the_shell_slides.after(bevy_gooey::GooeySystems::Style),
                    repaint_where_you_are,
                    // Ungated for the same reason: both screens have panels, and which one holds
                    // the keyboard is a question the menu asks louder than the editor does.
                    light_the_focused_panel,
                    // **gooey's control labels break where a dock bounds them** (#481): a switch's label is
                    // `NoWrap` because a dialog sizes itself by its widest control, and a dock does not.
                    let_a_toggle_label_break.after(bevy_gooey::GooeySystems::Style),
                    // **A tree row steps two advances, the file editor's own step** (#523): gooey
                    // sizes a level at a text size and a half, and every `TreeTwisty` is the widget's.
                    the_tree_steps_two_advances.after(bevy_gooey::GooeySystems::Style),
                    // **A tree row steps two advances, the file editor's own step** (#523): gooey
                    // sizes a level at a text size and a half, and every `TreeTwisty` is the widget's.
                ),
            )
            // **After `Phase::Text`, not in it.** The field consumes the keystroke there; painting
            // before it would show the box one character behind what has been typed.
            .add_systems(Update,
                (paint_name_box.after(keys::Phase::Text).before(bevy_gooey::GooeySystems::Style))
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            // B-014: the docks answer to the window. Editor-only, because the menu's own columns
            // go straight into `body` and carry no `PanelWidth`.
            .add_systems(
                Update,
                fit_docks_to_the_window
                    .after(FrameSystems)
                    // The width is a `Sizing`, which the skin turns into the node's width in the same frame.
                    .before(bevy_gooey::GooeySystems::Style)
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_systems(
                Update,
                size_the_shell_split
                    .after(fit_docks_to_the_window)
                    .after(adopt_the_shell_split)
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_systems(
                Update,
                the_left_panel_follows_its_pane
                    .after(size_the_shell_split)
                    .before(bevy_gooey::GooeySystems::Style)
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            .add_systems(Update,
                (light_the_back_button, stand_down_the_back_chord, dim_the_ground)
                    .run_if(in_state(crate::screen::Screen::Editor)),
            )
            // **Gooey's own toast stack stands under the menu bar** (#305). Ungated by screen: a
            // toast can be raised before the editor's frame exists (the tour's offer) and on a
            // screen with no frame at all, and gooey puts its stack at the window's root every
            // time. See `keep_toasts_under_the_menu_bar`.
            .add_systems(Update, keep_toasts_under_the_menu_bar.after(FrameSystems));
    }
}



#[cfg(test)]
mod scroll_tests {
    use super::{scroll_to_reveal, Follow};

    /// The ceiling every pre-existing case runs under: none. The arithmetic those cases pin is
    /// unchanged by the clamp; the two tests at the end of this module are the clamp's own.
    const NO_LIMIT: f32 = f32::INFINITY;

    /// **The arming rule, which is the half that was broken for two days — and then for longer.**
    ///
    /// `scroll_to_reveal`'s arithmetic was always right and always tested; nothing ever called it,
    /// because the flag that should have said "now" was re-armed every frame by an unrelated write.
    /// So this tests the flag, not the sums.
    #[test]
    fn a_follower_arms_on_the_selection_and_fires_exactly_once() {
        let mut f: Follow<usize> = Follow::default();

        // The frame the selection appears: do NOT scroll — the rows were rebuilt this frame and the
        // geometry still describes the previous layout.
        assert!(!f.should_scroll(Some(3)), "the move frame reads stale layout");
        // The next frame, with the selection unchanged: scroll, once.
        assert!(f.should_scroll(Some(3)), "the frame after is when the rows are real");
        assert!(!f.should_scroll(Some(3)), "and it does not write every frame after that");
        assert!(!f.should_scroll(Some(3)));

        // Moving again re-arms the same two-step.
        assert!(!f.should_scroll(Some(4)));
        assert!(f.should_scroll(Some(4)));

        // **The regression itself**: unrelated churn must not touch this. A resource-keyed follower
        // saw `is_changed` every frame — a status line, a hover, a preview watchdog — re-armed
        // itself, and never fired. Nothing here can observe that, which is the point.
        assert!(!f.should_scroll(Some(4)), "still quiet however busy the resource is");

        // Losing the selection arms too, so a list that had a highlight and now has none does not
        // scroll to a row that is gone.
        assert!(!f.should_scroll(None));
        assert!(f.should_scroll(None));
    }

    /// **A held arrow must not starve it, which is the third way this was broken.**
    ///
    /// The version before 2026-09-03 returned `pending` only on a frame where the selection had not
    /// moved. A held arrow reaches `keys::REPEAT_FAST_SECS` — 30 ms — and lands two steps on one
    /// frame often enough that corrections were being dropped in pairs; at any sustained frame time
    /// above 30 ms the selection changes every frame and the follow fired **never**. That is the
    /// reported symptom: the mesh changes, the list does not move.
    ///
    /// A move arriving while a correction is already armed now fires on the spot, one move stale,
    /// which the two-row margin absorbs.
    #[test]
    fn a_fast_walk_still_scrolls() {
        let mut f: Follow<usize> = Follow::default();

        // First move arms; nothing to fire yet.
        assert!(!f.should_scroll(Some(0)));
        // Now move on every single frame, as a held arrow does. Every one of these must scroll —
        // under the old rule every one of them returned false, for ever.
        for step in 1..=10 {
            assert!(
                f.should_scroll(Some(step)),
                "step {step} of a held arrow dropped its correction"
            );
        }
        // And it still settles: one more fire when the walk stops, then silence.
        assert!(f.should_scroll(Some(10)), "the last move is corrected once the walk ends");
        assert!(!f.should_scroll(Some(10)), "and then it is quiet");
    }



    /// A row with [`super::CONTEXT_ROWS`] of list on both sides asks for nothing — the common case,
    /// and the one that keeps a change-detected `ScrollPosition` from being touched sixty times a
    /// second.
    #[test]
    fn a_row_with_context_around_it_asks_for_no_scroll() {
        // List centred at 200, half 100 → fold [100, 300]. Row half 10 → margin is 2 rows = 40.
        // Comfortable band is therefore [140, 260]; a row at 200 sits in the middle of it.
        assert_eq!(scroll_to_reveal((200.0, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0), None);
        // Exactly two rows clear of each edge is still comfortable.
        assert_eq!(scroll_to_reveal((150.0, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0), None);
        assert_eq!(scroll_to_reveal((250.0, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0), None);
    }

    /// **Flush is not comfortable, and that reverses a decision.**
    ///
    /// The test here used to assert the opposite — *"Touching the edges exactly is still inside —
    /// flush is not off-screen"* — and it was true and beside the point. A row on the boundary has
    /// nothing after it, so a list being walked downward reads as having ended. Measured live on the
    /// Meshes list before the change: forty presses, selection on the bottom pixel, zero rows of
    /// what was coming next.
    #[test]
    fn a_flush_row_scrolls_to_earn_its_margin() {
        // Row flush against the bottom edge of fold [100, 300]: it wants 2 rows (40 px) of daylight.
        assert_eq!(
            scroll_to_reveal((290.0, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0),
            Some(40.0)
        );
        // And flush against the top edge, the same the other way.
        assert_eq!(
            scroll_to_reveal((110.0, 10.0), (200.0, 100.0), 100.0, NO_LIMIT, 1.0),
            Some(60.0)
        );
    }

    /// **A viewport too short for row-plus-margins degrades to flush** rather than fighting two
    /// margins it cannot satisfy at once.
    #[test]
    fn a_short_viewport_gives_up_its_margin() {
        // Fold [180, 220] is 40 tall; a 20-tall row leaves 10 either side, not 40.
        // Row at 215 (spans [205, 225]) overhangs the bottom by 5, and wants 10 more.
        assert_eq!(
            scroll_to_reveal((215.0, 10.0), (200.0, 20.0), 0.0, NO_LIMIT, 1.0),
            Some(15.0)
        );
    }

    /// **A jump of more than a viewport re-centres instead of creeping to the margin.**
    ///
    /// A `Shift`×5 step, a click, or a filter rebuilding the list under the cursor are not walks;
    /// edging such a move to the margin leaves the author on the boundary of a region they have not
    /// seen.
    #[test]
    fn a_page_jump_centres_the_row() {
        // Fold [100, 300], 200 tall. A row centred at 900 is far past a viewport away, so the
        // correction is "put it in the middle": 900 - 200 = 700.
        assert_eq!(
            scroll_to_reveal((900.0, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0),
            Some(700.0)
        );
    }

    /// Walking just past the fold scrolls by the overshoot **plus the margin it is owed**.
    #[test]
    fn an_off_screen_row_scrolls_by_its_overshoot() {
        // Row bottom at 320 against a fold ending at 300: 20 px of overshoot, plus a 40 px margin.
        assert_eq!(
            scroll_to_reveal((310.0, 10.0), (200.0, 100.0), 40.0, NO_LIMIT, 1.0),
            Some(100.0)
        );
        // Row top at 80 against a fold starting at 100: 20 px back up, plus the same margin.
        assert_eq!(
            scroll_to_reveal((90.0, 10.0), (200.0, 100.0), 100.0, NO_LIMIT, 1.0),
            Some(40.0)
        );
    }

    /// The answer is logical pixels: a 2x display (inverse scale 0.5) halves the physical delta —
    /// margin included, since the margin is measured in the same physical pixels the row is.
    #[test]
    fn the_correction_converts_physical_to_logical() {
        // 20 px of overshoot plus a 40 px margin is 60 physical, which is 30 logical on top of 40.
        assert_eq!(
            scroll_to_reveal((310.0, 10.0), (200.0, 100.0), 40.0, NO_LIMIT, 0.5),
            Some(70.0)
        );
    }

    /// The scroll never goes negative — the top of the list is the top.
    #[test]
    fn the_scroll_clamps_at_zero() {
        assert_eq!(
            scroll_to_reveal((50.0, 10.0), (200.0, 100.0), 10.0, NO_LIMIT, 1.0),
            Some(0.0)
        );
    }

    /// A correction under half a pixel is noise, not a scroll — the dead-band that stops a
    /// float-jittering layout from re-marking the resource changed every frame.
    ///
    /// Measured against the **comfortable** band rather than the fold, since 2026-09-03: with a
    /// 20 px row in a 200 px fold the margin is 40, so the band ends at 260 and a row bottom at
    /// 260.3 is three tenths of a pixel of jitter, not a scroll.
    #[test]
    fn a_sub_pixel_correction_is_swallowed() {
        assert_eq!(
            scroll_to_reveal((250.3, 10.0), (200.0, 100.0), 0.0, NO_LIMIT, 1.0),
            None
        );
    }

    /// A row taller than the whole list aligns its TOP — the half you read first — rather than
    /// oscillating between its two unsatisfiable edges.
    #[test]
    fn an_over_tall_row_aligns_its_top() {
        // Row spans [40, 560] against fold [100, 300]: top wins, scroll up by 60.
        assert_eq!(
            scroll_to_reveal((300.0, 260.0), (200.0, 100.0), 100.0, NO_LIMIT, 1.0),
            Some(40.0)
        );
    }

    /// **A reveal never asks past the end of the content.** The flush-bottom case above wants 40;
    /// with 25 of scroll left, it gets 25. Bevy clamps only `ComputedNode::scroll_position`, so
    /// an unclamped answer here would leave the `ScrollPosition` component carrying a surplus the
    /// list is not at — and every later delta computed from a place the list never went.
    #[test]
    fn a_reveal_never_asks_past_the_end_of_the_content() {
        assert_eq!(
            scroll_to_reveal((290.0, 10.0), (200.0, 100.0), 0.0, 25.0, 1.0),
            Some(25.0)
        );
    }

    /// **A scroll already at the end asks for nothing.** No write, so no re-layout, which is what
    /// the dead-band exists for — and the frame-after-frame write at the end of a list is exactly
    /// how the seed presented.
    #[test]
    fn a_scroll_already_at_the_end_asks_for_nothing() {
        assert_eq!(
            scroll_to_reveal((290.0, 10.0), (200.0, 100.0), 25.0, 25.0, 1.0),
            None
        );
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    /// **The whole point.** A refusal survives every receipt that follows it — which is what the
    /// single `status: String` could not do, because half these messages fire on ordinary hovering
    /// and the next one along overwrote the last.
    #[test]
    fn a_problem_outlives_every_note() {
        let mut s = Status::default();
        s.problem("NOT SAVED: read-only file system");
        for receipt in ["placed crate@7", "removal mode off", "saved", "filled 12"] {
            s.note(receipt);
            assert_eq!(
                s.problem_text(),
                "NOT SAVED: read-only file system",
                "`{receipt}` erased the refusal"
            );
        }
        assert_eq!(
            s.note_text(),
            "filled 12",
            "the receipt line stopped being current"
        );
    }

    /// The other half, and the reason there are two slots rather than one that refuses to be
    /// overwritten: an author who fixes the problem and saves has to be able to see the save work.
    #[test]
    fn a_note_is_still_shown_while_a_problem_stands() {
        let mut s = Status::default();
        s.problem("cannot record `break_table`: no such member");
        s.note("`break_table` armed");
        assert_eq!(s.note_text(), "`break_table` armed");
        assert!(s.has_problem());
    }

    /// **A problem joins the last one; it does not replace it.** The banner shows the newest, the
    /// log shows the run — which is the question that was unanswerable when a new refusal erased
    /// the one before it.
    #[test]
    fn problems_accumulate_and_the_newest_is_the_banner() {
        let mut s = Status::default();
        s.problem("first");
        s.problem("second");
        assert_eq!(s.problem_text(), "second", "the banner shows the newest");
        assert_eq!(s.problems().len(), 2, "and the log keeps both");
        // One list, one clearing rule.
        s.dismiss();
        assert!(!s.has_problem());
        assert!(s.problems().is_empty());
        assert!(s.is_empty(), "dismissing left something behind");
    }

    /// **Consecutive repeats fold.** A refusal fires per gesture and gestures repeat; four identical
    /// lines would bury everything else on the tab.
    #[test]
    fn a_repeated_problem_folds_into_a_count() {
        let mut s = Status::default();
        for _ in 0..4 {
            s.problem("blocked: `floor@3` already covers that spot");
        }
        assert_eq!(s.problems().len(), 1);
        assert_eq!(s.problems()[0].count, 4);
        assert!(s.problems()[0].line().ends_with("(x4)"));

        // Only CONSECUTIVE ones, so the order stays honest.
        s.problem("NOT SAVED: read-only file system");
        s.problem("blocked: `floor@3` already covers that spot");
        assert_eq!(
            s.problems().len(),
            3,
            "a repeat after something else is a new entry"
        );
    }

    /// **The cap names what it dropped.** This crate's caps refuse and name rather than truncate; a
    /// log cannot refuse the newest entry, so it counts the ones it let go instead.
    #[test]
    fn the_log_caps_and_says_how_many_it_dropped() {
        let mut s = Status::default();
        for i in 0..MAX_PROBLEMS + 5 {
            s.problem(format!("problem {i}"));
        }
        assert_eq!(s.problems().len(), MAX_PROBLEMS);
        assert_eq!(s.dropped(), 5);
        assert_eq!(
            s.problem_text(),
            format!("problem {}", MAX_PROBLEMS + 4),
            "the newest must survive the cap — it is the one worth having"
        );
        s.dismiss();
        assert_eq!(s.dropped(), 0, "dismissing clears the tally with the list");
    }

    /// `say` is the one place a `Result` becomes a message, so no call site can stringify an error
    /// into the quiet slot — the bug that had `NOT WRITTEN:` rendering as a receipt at nine
    /// `tiles::persist` call sites.
    #[test]
    fn say_routes_by_the_result_and_not_by_the_wording() {
        let mut s = Status::default();
        s.say(Ok("recorded 3 member(s)".to_owned()));
        assert_eq!(s.note_text(), "recorded 3 member(s)");
        assert!(!s.has_problem());

        // Deliberately worded like a receipt: routing must not depend on how it reads.
        s.say(Err("everything went fine, honestly".to_owned()));
        assert_eq!(s.problem_text(), "everything went fine, honestly");
        assert_eq!(
            s.note_text(),
            "recorded 3 member(s)",
            "an Err overwrote the receipt"
        );
    }

    /// One line for a log or a captured frame, and it must be the bad news when there is any.
    #[test]
    fn the_one_line_form_prefers_the_problem() {
        let mut s = Status::default();
        s.note("stamped 4 piece(s)");
        assert_eq!(s.line(), "stamped 4 piece(s)");
        s.problem("stamp refused: nothing offers a surface");
        assert_eq!(s.line(), "stamp refused: nothing offers a surface");
    }
}

/// **One line where eighteen rows used to be.** See [`ChromePlugin`] for why it is not optional.
/// **What this door is open on** — the map's name, or the kit's if no map is open.
///
/// One function because two things say it: the window's title (`main::name_the_window`) and the
/// chrome bar. Which *name* is the right one is a decision, and a decision made twice is a decision
/// that drifts — the window would go on saying `furniture` while the bar said the map, and nobody
/// would notice for a week.
pub fn subject(
    open_map: Option<&crate::project::OpenMap>,
    project: Option<&crate::project::Project>,
) -> Option<String> {
    match (open_map, project) {
        (Some(m), _) => Some(m.map.name.clone()),
        (None, Some(p)) => Some(p.namespace.clone()),
        _ => None,
    }
}

/// **The chrome bar's words for the door and the subject**: `MAP: bedroom`, or the door's name alone
/// when nothing is open (#519).
///
/// **The door's name is the label and the subject its value.** This read `MAP · bedroom`, two
/// values on one line with a middle dot between them, which the owner read as *"separate text values
/// in one row. It looks jank."* A colon is the label-and-value idiom the editor's rows already draw
/// (`chrome::row_label`), and the bar is one line tall, so the two facts stay on the one line the
/// band has for them.
///
/// Public because [`spawn_chrome_bar`] and [`repaint_where_you_are`] both ask it, and because a test
/// reads the words from here rather than from a node that is only on screen in a window.
pub fn where_you_are(
    door: &crate::workspace::Workspace,
    open_map: Option<&crate::project::OpenMap>,
    project: Option<&crate::project::Project>,
) -> String {
    match subject(open_map, project) {
        Some(name) => format!("{}: {name}", door.label()),
        None => door.label().to_owned(),
    }
}

/// **The way back to the chooser, and the one piece of the window that belongs to no door.**
///
/// # It failed twice as a row in a panel
///
/// Asked for at the keyboard: *"when we go into the map editor, we actually need a button to go back
/// to the main UI."* There was only `Cmd+O`, and a key nothing on screen mentions is a key nobody
/// finds. So a button was added — **inside the left panel**, under that panel's own heading, on
/// `Ink::slot_bg`, the ground this editor uses for an inspector slot. It was reported missing again:
/// *"When I enter kit editing, there's no clear way to get back to the main menu."*
///
/// The diagnosis was that it was not the contrast: the encoding
/// said *"a field of the thing you are looking at"* rather than *"a way out of it"*, because it read
/// as a row in a list of rows, and **nothing at window level was navigation at all**. This repo
/// already settled what to do when a signal fails twice — *"the encoding was not weak, it was
/// wrong"*.
///
/// So it is chrome now, above the door's own strip, the same on every door. **Four call sites
/// became one**, which is the other half of the point: a way out that each panel places is a way out
/// each panel can forget, and the Rigs door drew it in a different place from the Map door.
///
/// It still names its chord beside itself, which is `ExposeHK`'s rehearsal argument — the pointer is
/// the way in, and the label beside it is what turns a pointing habit into a typing one. The click
/// and the key both go through `editor::leave_for_menu`, so the unsaved-work refusal cannot differ
/// between them.
pub fn spawn_chrome_bar(
    mut commands: Commands,
    frame: Res<Frame>,
    door: Option<Res<crate::workspace::Workspace>>,
    open_map: Option<Res<crate::project::OpenMap>>,
    project: Option<Res<crate::project::Project>>,
) {
    // **The bar's words come from [`where_you_are`]**, so the spawn and the repaint cannot disagree
    // about them any more than they can about the name (`subject`).
    let here = door.as_deref().map(|d| where_you_are(d, open_map.as_deref(), project.as_deref()));

    // **The way out names where it goes**: the project's own name, which is the page it returns to (`project_page.rs`).
    let back = back_name(project.as_deref());

    commands.entity(frame.chrome_bar).with_children(|bar| {
        bar.spawn((
            BackButton,
            Control(keys::ControlId::Back),
            // A chip-sized row: its words' gap and its inset the sheet's `.chrome-back`, never shrinking, in the editor's own fill (`InkFollow`).
            bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Center), shrink: Some(0.0), ..default() }),
            bevy_gooey::StyleClass::of("chrome-back"),
            bevy_gooey::InkFollow,
            BackgroundColor(ink().slot_bg),
            // The bar is `Pickable::IGNORE` so the frame answers no clicks; this narrows that for
            // the one node on it that is a button — the same move `spawn_name_box` makes.
            bevy::picking::Pickable::default(),
            Hovered::default(),
        ))
        .with_children(|b| {
            b.spawn((
                Text::new(format!("\u{2039} {back}")),
                TextColor(ink().key),
                typed(BAR_READOUT),
                bevy::picking::Pickable::IGNORE,
            ));
            b.spawn((
                BackChord,
                Text::new(keys::chord(keys::Action::MainMenu)),
                TextColor(ink().label),
                typed(BAR_READOUT),
                bevy::picking::Pickable::IGNORE,
            ));
        });

        // Pushes the subject to the far end. A spacer rather than `SpaceBetween` on the bar, so a
        // third thing added later lands where it is put rather than being redistributed.
        bar.spawn(bevy_gooey::spacer());

        if let Some(here) = here {
            bar.spawn((
                WhereYouAre,
                // Where you are is what `N` renames, so its badge goes on the words it changes.
                Control(keys::ControlId::Title),
                Text::new(here),
                TextColor(ink().dim),
                typed(BAR_READOUT),
                bevy::picking::Pickable::IGNORE,
            ));
        }
        // **Which decision model is asked does not stand here any more** (#514). It was one LED with the model's name at this bar's far
        // end (DM-03, #454), and the agent's light was in the terminal bar while the reason for a failure was a card over the terminal:
        // the owner's note asks for *"a unified place to put the agent and decision model status"*, so the pair stands together in the
        // status band's [`spawn_agent_band`] and this bar carries no model. The reasoning that put it here — "the status band is crowded
        // enough" — is answered by the band's own rule that nothing is added beside the hint, which is behind a spacer at the far end.
    });
}

/// The bar's right-hand end: which door, on what. Marked so a test can find it — and so
/// [`repaint_where_you_are`] can keep it true.
#[derive(Component)]
pub struct WhereYouAre;

/// **The bar says what the title says, for as long as both are on screen.**
///
/// [`spawn_chrome_bar`] runs once, at `OnEnter(Editor)`, and nothing repainted this — so renaming a
/// map with `N` moved `main::name_the_window`'s title and left the bar reading the old name for the
/// rest of the session. That is exactly the drift [`subject`] was extracted to make impossible,
/// arriving through the half that was never wired: one function deciding the name is worth nothing
/// if only one of its two readers ever asks it.
///
/// Compared before writing, per the standing rule — `Text` is change-detected and this runs every
/// frame.
pub fn repaint_where_you_are(
    door: Option<Res<crate::workspace::Workspace>>,
    open_map: Option<Res<crate::project::OpenMap>>,
    project: Option<Res<crate::project::Project>>,
    mut bar: Query<&mut Text, With<WhereYouAre>>,
) {
    let Some(door) = door else { return };
    // **The same words the bar was spawned with**, from the one function that composes them.
    let want = where_you_are(&door, open_map.as_deref(), project.as_deref());
    for mut text in &mut bar {
        if text.0 != want {
            text.0 = want.clone();
        }
    }
}

/// **One band at the foot of the window: what went wrong, and what the keys are.**
///
/// Both of these were spawned *inside* each panel, once per tab, which had two costs. The banner
/// carries the longest text this editor renders — refusals naming descriptors and compositions —
/// and a 300 px controls panel is the narrowest place on screen to put it. And the hint line, at
/// `layout.controls_w`, wrapped: *"Hold K for shortcuts · Cmd+O back to kits & maps"* broke across two
/// lines on the Map door, which is visible in the capture that started this work.
///
/// Full window width fixes both by construction, and it deletes four copies of the same two calls.
///
/// **The hint goes at the far end behind a spacer, not in a `SpaceBetween` row**, so it does not
/// move when the banner appears — a hint that shifts sideways the moment something goes wrong is a
/// hint you have to re-find exactly when you are least able to.
///
/// The problem **log** deliberately stays in the panels. The banner answers *"what just happened"*
/// and belongs to the window; the log answers *"what has gone wrong here"*, is a stack of lines
/// rather than one, and a band that grew to hold it would move the viewport every time a refusal
/// landed.
pub fn spawn_status_band(mut commands: Commands, frame: Res<Frame>, project: Option<Res<crate::project::Project>>) {
    let back = back_name(project.as_deref());
    commands.entity(frame.status).with_children(|band| {
        // **The agent and the decision model, at the band's left end** (#514). Where it stands is a layout decision with a
        // measurement behind it: the hint sits behind the band's spacer at the far end, and `badges` settles the hint's key badge
        // just to its left — which is exactly the ground this region occupied when it was placed between the spacer and the hint. Gate
        // 124 measured the result: `Control(Hint)` badge `Rect{614,837 .. 638,862}` covering 24x17 px of this region's own ink. The
        // left end is ground nothing else claims, so the pair reads first and covers nothing.
        spawn_agent_band(band);
        // **The problem is not here any more.** It used to be a flow child of this band — a padded,
        // wrapping block inside twenty-six pixels of chrome, so it stood proud of the band it was
        // supposed to be in. Reported from the keyboard: *"it appears over the status bar at the
        // bottom and isn't quite in alignment… so it looks really bad."* It is a toast over the
        // viewport now; see [`problem_toast`].
        // **Which model is answering, once for the session.** It used to be printed on every
        // proposal, which is the same fact restated per piece — see `labels::Labeler`, which owns
        // both the text and the rule about when the word `connected` may be used. Empty until the
        // config has been read, so an unconfigured project shows nothing rather than a placeholder.
        // **What is in hand** (UI-34, #228): `"3 selected"` while a set is held, empty otherwise, so
        // the band answers the one question a person asks before a stamp — how many am I carrying —
        // without them counting from the ghost. Empty when nothing is held rather than `"0 selected"`:
        // a count of nothing is a row that says the same thing on every frame and teaches nobody.
        // **What the reloads say** (UI-34, #228, #647): a count of files refused or needing a restart,
        // in gooey's own status item, so the kind is the kind a mark is drawn from and the ink is the
        // running skin's rather than a colour this band writes beside it. Empty when every file
        // applied, and `StatusKind::Info` then draws no mark at all: nothing wrong is the absence of a
        // mark, never a quiet bar. A person driving over BRP asks this in `blueprint_state.reloads`;
        // the band answers it where they are looking.
        //
        // **A quiet item**, because a readout is not on the `Activate` bus: this editor has many global
        // `Activate` observers and a `Button` fires all of them (`jobs.rs` documents the day that took
        // the application down). Its words become `Role::ControlText` for the same family of reason
        // (`the_words_of_a_quiet_item_are_a_control`): the interface's tests count `ButtonText` over
        // the world to say what a dialog's answers are.
        // **Closed while it has nothing to say** (`Shown`, gooey's rule for an item that is not there:
        // the job row and the wish chip are hidden the same way). A status item reserves its padding and
        // a target's height even with empty words, so an item left open on a clean band would stand in
        // the row as a blank box on every frame, where the bare label it replaces took no width.
        let problems = band
            .spawn((
                ProblemsCount,
                bevy_gooey::status_item_quiet(String::new()),
                bevy_gooey::Shown(false),
                bevy::picking::Pickable::IGNORE,
            ))
            .id();
        // **No `typed(..)` marker on these words**: gooey's `Role::ControlText` inks and sets them as a
        // control's, so a `TypeRole` here would be a second size fighting `resize_the_labels`.
        the_words_of_a_quiet_item_are_a_control(band.commands_mut(), problems, ());
        band.spawn((
            SelectionCount,
            Text::new(String::new()),
            TextColor(ink().dim),
            typed(text::LABEL),
            bevy::picking::Pickable::IGNORE,
        ));
        band.spawn((
            LabelerLine,
            Text::new(String::new()),
            TextColor(ink().dim),
            typed(text::LABEL),
            bevy::picking::Pickable::IGNORE,
        ));
        band.spawn(bevy_gooey::spacer());
        shortcut_hint(band, &back);
    });
}

/// **The one place that says what the agent is doing and which decision model answers** (#514).
///
/// The owner's note on the capture: *"It seems like we need a unified place to put the agent and decision model status."* Before this the
/// two facts sat in three places, and none of them held the pair: the agent's light and state word were in the **terminal bar** (`Agent
/// off`, `Agent busy`, …), the reason for a failure was a **card floating over the terminal** that came and went, and the decision model
/// was a light at the far end of the **chrome bar** (`○ d3-flash`). A person asking *"what is the agent doing, and what is answering it?"*
/// had to read a band at the bottom, a bar at the top and a card in between — and the two lights, one shape in two colours, meant two
/// different things.
///
/// So both stand in **one region of the status band**, the agent first and the decision model second, at the band's foot where a person's
/// eye already goes for the state of things. The region is [`AgentBand`]; each half keeps its own marker ([`AgentBandLed`] and
/// [`crate::decision_model::DecisionLed`]), so the test that pins the model's name and hover follows the light here from the chrome bar
/// without a second vocabulary.
///
/// **It says what it is doing, and the reason it is not**, in one sentence in its hover, written by
/// [`crate::terminal_agent::paint_the_band`] from the drawn tab's `AgentPost` ([`crate::terminal_agent::band_off_words`] is the
/// off-form of the same sentence): the whole card, in the place a person looks. That is the other half of "one place" — the reason a
/// failure gives used to be reachable only by finding a chip in a bar that scrolls off when the pane narrows.
pub fn spawn_agent_band(band: &mut ChildSpawnerCommands) {
    band.spawn((
        AgentBand,
        // A row: the agent's half, then the decision model's, a tight step apart (the sheet's `.chrome-agent-band`) so the pair reads as
        // one region and each half reads as one fact.
        bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Center), ..default() }),
        bevy_gooey::StyleClass::of("chrome-agent-band"),
        AccessibleLabel::new("Agent and decision model"),
        Name::new("agent band"),
        bevy::picking::Pickable::IGNORE,
    ))
    .with_children(|region| {
        // **The agent's half**: dark until an agent tab is drawn, and painted from the tab's `AgentPost` by
        // [`crate::terminal_agent::paint_the_band`], so this is the same light the terminal bar draws in the one place it does not go away.
        //
        // **`Pickable` and `Hovered` on the half, not the region**, so hovering the model's light does not open the agent's card and the
        // band's other rows keep `Pickable::IGNORE` for the reason [`panel_root`] states — a control the "is the pointer on the interface"
        // gate cannot see would pass a click through to the map behind it.
        region
            .spawn((
                AgentBandHalf,
                bevy_gooey::row_with(0.0, bevy_gooey::RowLayout { align: Some(AlignItems::Center), ..default() }),
                bevy_gooey::StyleClass::of("chrome-agent-band-half"),
                bevy::picking::Pickable::default(),
                Hovered::default(),
                bevy_gooey::Tooltip::new(String::new()),
            ))
            .with_children(|half| {
                half.spawn((
                    bevy_gooey::led(AGENT_LABEL, bevy_gooey::LedState::Off),
                    AgentBandLed,
                ));
            });
        // **The decision model's light**: the same bundle the chrome bar's chip was, so the model's name, its state and its hover are
        // written by one system and read by one test.
        region.spawn(crate::decision_model::chip());
    });
}

/// **The status band's one agent-and-decision-model region** (#514). See [`spawn_agent_band`].
#[derive(Component)]
pub struct AgentBand;

/// **The agent's half of the band**, which carries the hover that says in a sentence what the light is doing.
#[derive(Component)]
pub struct AgentBandHalf;

/// **The agent's light in the status band** (#514), beside the decision model's. Its dot and state word follow the drawn agent tab's
/// `AgentPost`, exactly as the terminal bar's light does; this is the one that never leaves the screen.
#[derive(Component)]
pub struct AgentBandLed;

/// What the agent's light is called before its state word (`Agent off`, `Agent live`). It is also the light's accessible name, so a
/// reader hears what it is and not only its state.
pub const AGENT_LABEL: &str = "Agent";

/// **The status band's labeler readout.** The node is here because the band is; what it says is
/// `labels::Labeler`'s, so this module does not learn what a model is.
#[derive(Component)]
pub struct LabelerLine;

/// **How many files the last reload refused or could not take without a restart**, in the status band
/// (UI-34, #228). Written by [`show_problems_count`].
#[derive(Component)]
pub struct ProblemsCount;

/// **What the band says about the reloads, and the kind it says it as** (UI-34, #228, #647).
///
/// The test that pins it does not restate these rules: it writes a verdict and reads what the band
/// drew, so an assertion about the band is an assertion about what is painted, not about a resource
/// that happens to hold the same value.
///
/// **Four rules, each with a reason:**
///
/// - An `applied` file is not counted. The status line already writes a receipt for every edit, and a
///   band that said "3 reloaded" on every keystroke would be a row nobody reads.
/// - **Both a refusal and a restart are the warning kind**, which is the issue's own wording ("a
///   refused reload turns the problems item to the warning kind"). A refusal keeps the previous state
///   and the editor keeps running; nothing is lost, but something a person wrote did not take, and
///   that is what the mark is for.
/// - **Both counts are named when both exist**, because a person who fixes the refusal still has a
///   restart owed, and one number in place of the other would hide half the work.
/// - **Nothing wrong says nothing**, rather than `"0 problems"` — the band is silent in the state it
///   is in almost all the time.
///
/// **The kind is the whole answer, and no ink comes out of here** (#647). gooey draws a
/// `StatusKind::Warning` as the skin's `accent` and a `StatusKind::Error` as its `danger`, on the mark
/// its `status_item` spawns, and inks the words as a `ControlText` in the skin's `text`. A colour
/// computed here from the same kind was a second copy of one fact, and only the copy nothing drew was
/// the one that said a mark existed at all.
pub fn reload_readout(kinds: &[blueprint_model::live::KindVerdict]) -> (String, bevy_gooey::StatusKind) {
    use blueprint_model::live::Outcome;
    let refused = kinds.iter().filter(|k| k.outcome == Outcome::Refused).count();
    let waiting = kinds.iter().filter(|k| k.outcome == Outcome::NeedsRestart).count();

    let mut parts: Vec<String> = Vec::new();
    if refused > 0 {
        parts.push(if refused == 1 { "1 refused".to_owned() } else { format!("{refused} refused") });
    }
    if waiting > 0 {
        parts.push(if waiting == 1 {
            "1 needs a restart".to_owned()
        } else {
            format!("{waiting} need a restart")
        });
    }

    let kind = if refused + waiting == 0 {
        bevy_gooey::StatusKind::Info
    } else {
        bevy_gooey::StatusKind::Warning
    };
    (parts.join(" \u{b7} "), kind)
}

/// **Say what became of the reloads, when the answer changes.**
///
/// Gated on `EditorReloads` (a file was decided) or on the item itself being new, so a band with
/// nothing wrong writes no text on any frame — and the band still reads the reloads it inherited on
/// the frame it is built, rather than waiting for the next edit to show a refusal that already
/// happened (`apply_style` decides the layers before the editor's first frame). Every write is
/// compared in place as well, which is `tests/no_system_writes_every_frame.rs`'s rule: the words, the
/// kind and whether the item is drawn are change-detected, and gooey's `apply_skin` restyles the
/// item's mark and words off the kind in the same frame.
///
/// **Two facts, two components, two owners** (#647): the text is this crate's and goes on the item's
/// words; the kind is stated on the item, where gooey reads it for the mark. Nothing here writes a
/// colour — the mark's fill and the words' ink are `apply_skin`'s, from the kind and the skin.
pub fn show_problems_count(
    reloads: Option<Res<crate::live::EditorReloads>>,
    mut counts: Query<(&mut bevy_gooey::StatusKind, &mut bevy_gooey::Shown, Option<&Children>), With<ProblemsCount>>,
    fresh: Query<(), Added<ProblemsCount>>,
    mut words: Query<&mut Text>,
) {
    let Some(reloads) = reloads else { return };
    if !reloads.is_changed() && fresh.is_empty() {
        return;
    }
    let (said, kind) = reload_readout(reloads.kinds());
    let on = !said.is_empty();
    for (mut told, mut shown, children) in &mut counts {
        for child in children.into_iter().flat_map(|c| c.iter()) {
            if let Ok(mut text) = words.get_mut(child)
                && text.0 != said
            {
                text.0 = said.clone();
            }
        }
        if *told != kind {
            *told = kind;
        }
        if shown.0 != on {
            shown.0 = on;
        }
    }
}

/// **How many pieces are in hand**, in the status band (UI-34, #228). Written by
/// [`show_selection_count`] from [`crate::editor::CloneDrag`], which owns the answer.
#[derive(Component)]
pub struct SelectionCount;

/// **Say how many pieces are in hand, when the answer changes.**
///
/// Gated on the resource's change detection, so a band with nothing in hand writes no text on any
/// frame: the count only moves when a set is picked up, added to or put down.
pub fn show_selection_count(
    drag: Res<crate::editor::CloneDrag>,
    mut counts: Query<&mut Text, With<SelectionCount>>,
) {
    if !drag.is_changed() {
        return;
    }
    let said = match drag.held_pieces() {
        Some(n) => format!("{n} selected"),
        None => String::new(),
    };
    for mut text in &mut counts {
        if text.0 != said {
            text.0 = said.clone();
        }
    }
}

/// The clickable way back. Marked so `editor` can hang one observer on it wherever a panel put it.
#[derive(Component)]
pub struct BackButton;

/// **The chord printed on the button**, so it can stand down while the badges are up.
#[derive(Component)]
pub struct BackChord;

/// **One `Cmd+O` on the button, never two.**
///
/// The chord is printed beside `‹ kits & maps` for the reason [`spawn_chrome_bar`] argues at
/// length — ExposeHK's rehearsal path, and an author who pressed `Esc` three times looking for the
/// exit. Holding the shortcuts key then draws that same chord *again*, as the button's badge, one
/// gap away from itself. Reported from the keyboard: the back button prints `Cmd+O` twice.
///
/// So while the overlay is up, the inline copy stands down and the badge is the one that speaks.
/// This is the move `compass::show_by_tab` already makes for the gizmo, and for the same reason: two
/// things that are each right on their own are not both wanted in the same instant.
///
/// Compares before writing — `Node` is change-detected by the layout system, and `Mut::deref_mut`
/// marks it changed whether or not the value moved.
fn stand_down_the_back_chord(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    mut chords: Query<&mut Node, With<BackChord>>,
) {
    let want = if keys::pressed(&keyboard, *live, keys::Action::Shortcuts) {
        Display::None
    } else {
        Display::Flex
    };
    for mut node in &mut chords {
        if node.display != want {
            node.display = want;
        }
    }
}

/// **While the shortcuts key is held, the grounds step back and the badges are the nearest thing.**
///
/// This is the other half of letting a badge stand on a panel's empty middle. `badges::ink` reads
/// [`Ground`] to say that a container's fill carries nothing a reader needs, so the ground under a
/// short list is placeable — but a badge is itself an `Ink::panel_bg` box, and an opaque box on an
/// opaque box of the same colour reads as one shape with a line through it. Dropping the ground to
/// `layout.ground_held` lets the `Ink::veil` and the window behind it show through, and the badge — which
/// keeps its own opacity — separates by depth rather than by its border alone.
///
/// **Fill only, and restored from the carried colour.** Every row, field, chip and word inside the
/// panel keeps its opacity, because those are exactly what the badges are pointing at.
///
/// Compares before writing: `BackgroundColor` is change-detected, and touching every panel every
/// frame would mark the whole interface dirty sixty times a second for one held key
/// (`tests/no_system_writes_every_frame.rs`).
pub fn dim_the_ground(
    keyboard: Res<ButtonInput<KeyCode>>,
    live: Res<keys::Live>,
    // **The panel's colour is the running skin's** (UI-02, #210 slice 4b). This system already walks
    // every ground, already compares before writing, and is the only place with the skin in reach —
    // `panel_root`'s six callers have no `Res<GooeySkin>`. A `None` skin (a headless harness that
    // never ran `EditorUiPlugin`) keeps the constant, which is the shipped table.
    skin: Option<Res<bevy_gooey::GooeySkin>>,
    mut grounds: Query<(&Ground, Option<&PanelGround>, Option<&BandGround>, &mut BackgroundColor)>,
) {
    let held = keys::pressed(&keyboard, *live, keys::Action::Shortcuts);
    let fills = PanelFills::of(skin.as_deref());
    for (ground, is_panel, is_band, mut bg) in &mut grounds {
        // A panel's and a band's rest colour follow the skin; anything else carries its own. Reading
        // the skin *here* rather than at spawn is what makes a theme edit reach a panel that was built
        // before it — the whole point of a skin being data.
        let rest = if is_panel.is_some() {
            fills.panel
        } else if is_band.is_some() {
            fills.band
        } else {
            ground.0
        };
        let want = if held { rest.with_alpha(rest.alpha() * crate::style::metrics().layout.ground_held) } else { rest };
        if bg.0 != want {
            bg.0 = want;
        }
    }
}

/// The back button's hover state, as the lift reads it.
type BackButtonRow = (&'static Hovered, &'static mut BackgroundColor);

/// Lift the button while the pointer is on it, so it reads as pressable before it is pressed.
pub fn light_the_back_button(
    mut buttons: Query<BackButtonRow, (With<BackButton>, Changed<Hovered>)>,
) {
    for (hovered, mut bg) in &mut buttons {
        bg.0 = if hovered.get() { ink().row_selected } else { ink().slot_bg };
    }
}

/// **Where the way out goes, in words**: the project's folder name, which is the page `Cmd+O` and the
/// back button return to (`project_page.rs`), and `Projects` when no project is open.
fn back_name(project: Option<&crate::project::Project>) -> String {
    project
        .and_then(|p| p.root.file_name())
        .map_or_else(|| "Projects".to_owned(), |n| n.to_string_lossy().into_owned())
}

pub fn shortcut_hint(parent: &mut ChildSpawnerCommands, back: &str) {
    parent.spawn((
        // **The way out is named here, not only on the button above it.**
        //
        // The button says `‹ kits & maps` at 11px and this line said `Hold K for shortcuts` at 10px,
        // and an author looking for the exit found neither — they pressed `Esc` three times instead,
        // which is the one key deliberately wired to mean "not that" rather than "out". Naming the
        // chord where the eye already goes for keys is ExposeHK's rehearsal argument: the novice
        // path and the expert path are the same path.
        //
        // **It says what the key does now, not what it opens.** `K` used to raise a table you read;
        // it labels the interface in place. "For shortcuts" described a document, and the whole
        // reason the document was replaced is that a reader had to carry a phrase from it back to
        // something on screen.
        Text::new(format!(
            "Hold {} — every key lands on what it does   ·   {} back to {back}",
            keys::chord(keys::Action::Shortcuts),
            keys::chord(keys::Action::MainMenu),
        )),
        TextColor(ink().label),
        typed(text::LABEL),
        // **And holding it puts that key's own badge on this line.** The one control whose verb is
        // "show me the verbs" — ExposeHK's admitted weakness is that a modifier-triggered overlay
        // *"has no visual representation to aid discovery until the user … presses its trigger"*, and
        // this is the loop closing: the line that tells you about `K` is where `K` labels itself.
        Control(keys::ControlId::Hint),
    ));
}
