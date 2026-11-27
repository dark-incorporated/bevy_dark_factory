//! **No panel draws a control past its own content box** (#494, `feedback-1794921716691`: *"Button is spilling
//! outside."*).
//!
//! # What the owner saw
//!
//! A capture of the Map door at 2560 x 1604 physical px (`ui_scale` 2.4, project `lavender_hills`, map
//! `deadite_caverns`), with a pick on the right-hand Place panel at `[2151, 372, 2531, 979]` — 378 physical px
//! wide. Measured on the frame: the panel's border is at `x=2151`..`2529`, its content edge at `2528`, and a
//! chip/button inside it is drawn out to `x=2499` in the rows at `y≈770-850` — **29 px past the content edge**,
//! over the border and into the 30 px gutter before the window's edge. A second chip at `y=530-560` runs to
//! `x=2547`, **48 px past**.
//!
//! # One defect, four panels
//!
//! The same family as #468 (the Pieces panel), #481 (a switch in the Agent panel) and #482 (the left pane at a
//! narrow divider): a panel lays its children out wider than the room it has and draws them over its border.
//! The acceptance asks for one test that holds all four, so a fix for one cannot leave the other three free to
//! regress — hence **one `#[test]`, four panes**.
//!
//! # The measurement
//!
//! Each pane is staged the way the owner met it: the capture's window (2560 x 1604 at a 2.0 display factor,
//! so 1280 x 802 logical), with the person's **`interface_scale`** at 1.0 (the capture's own: `1.2 * 1.0 * 2.0 *
//! 1.0 = 2.4`) and at 1.6. The editor's own `fit_docks_to_the_window` then squeezes the docks the way it did on
//! the frame — the Place panel comes out 380 physical px against the capture's 378, which this file asserts, so
//! the layout measured is the one the capture was taken on and not a width this test picked.
//!
//! Then **every drawn node under the pane** is measured — a text from the union of its box and the boxes of the
//! glyphs it paints (`TextLayoutInfo`), because a node can be narrower than the words in it and that is half the
//! defect — and held to the pane's **content box**: its border box less its content inset. Layout rounds to
//! whole pixels, so an edge may stand 1 px past.
//!
//! # The fix that closed it (#494)
//!
//! **The chip, not the call site.** `chrome::chip` now carries two things a chip did not have, and
//! together they are what closes the PIECES panel at both scales (measured here: 46.0 px past at
//! scale 1.0 and 71.4 px at 1.6, both to 0):
//!
//! - its box carries `max_width: 100%`, so a chip is never laid out wider than the line it stands on,
//!   while `shrink` stays `Some(0.0)` — a chip in a toolbar is still a fixed target, and a sibling
//!   still cannot cut its word (the three `the_terminal_bar_keeps_every_chip_whole` sweeps hold that);
//! - its label breaks `WordOrCharacter` rather than `NoWrap`, so the chip's min-content width is its
//!   longest **word** (`IMPORTED`) instead of its whole counted phrase (`NOT IMPORTED (4)`).
//!   `LineBreak::WordOrCharacter` is the mechanism #481 used for the AGENT panel's rows.
//!
//! **The two are one fix and neither half works alone.** `NoWrap` was holding the chip's floor at
//! 379 px in a 319 px row, so the strip's `FlexWrap` had nothing to wrap — its one child was already
//! too wide for a line — and `max_width` alone would have squeezed that 379 px label into a narrower
//! box with its words cut. Measured on this change, before the break: the wrapper came down to the
//! line but the label still overhung it.
//!
//! **Two attempts are recorded on the issue as reverted, and this is not either of them.**
//! `min_width: Some(Px(0.0))` on `row_value` and on the chip label dropped the row's automatic
//! minimum and reproduced #481's regression (`no_badge_cluster_draws_through_another`,
//! `the_g87_nodes_are_gooey` and all three `the_terminal_bar_keeps_every_chip_whole` sweeps went red);
//! a `TextFlow { shrink: 1.0 }` at the shelf strip's call site left PIECES at 71.4 px at scale 1.6,
//! because the wrapper keeps the chip's content width.
//!
//! **Clipping is deliberately not applied.** A pane that hides what does not fit has still laid it out as
//! though the pane were wider, and the control beside it in that column inherits the mistake; the capture shows
//! the overflow *visible* in the gutter, so what matters is where a node is laid out, not what an ancestor
//! happens to cut.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::{PrimaryWindow, Window, WindowResolution};

use bevy_dark_factory::chrome::{self, PanelWidth, Side};
use bevy_dark_factory::harness;
use bevy_dark_factory::settings::UserSettingsFile;
use bevy_dark_factory::tiles::Mode;

use crate::fixtures;
use crate::settle_support;

/// **The capture's window**, physical px (`feedback-1794921716691` was 2560 x 1604).
const WINDOW_PHYSICAL: (u32, u32) = (2560, 1604);

/// The capture's own display factor: 2560 / 1280.
const DISPLAY_FACTOR: f32 = 2.0;

/// The interface scales the acceptance names: the capture's own (its `ui_scale` 2.4 over the 1.2 skin, the 1.0
/// display growth and the 2.0 display factor), and one and a half times it.
const INTERFACE_SCALES: [f32; 2] = [1.0, 1.6];

/// **The Place panel's width on the capture**, physical px: its border box ran `2151`..`2529`.
const CAPTURE_PLACE_PANEL_PX: f32 = 378.0;

/// How far the staged Place panel may stand from the capture's own width, physical px — a whole-pixel layout
/// round on either border.
const WIDTH_SLACK_PX: f32 = 2.0;

/// **The capture's text size**, as a multiple of the skin's — derived from the frame, not guessed.
///
/// The panel is the same width as the harness's (378 against 380 physical px), so a glyph's pitch is the
/// one number that differs: measured down the `NOT IMPORTED (0)` chip, fifteen glyphs span
/// `x=2200..2527` — **21.9 px** a character. The same string at the harness's own `text_scale` of 1.0
/// spans 231 px for fifteen characters, **15.4 px** each. `21.9 / 15.4 = 1.42`, and `CONTROL`'s 10 px at
/// a 2.4 `UiScale` with the face's 0.6 em advance is `10 * 2.4 * 0.6 = 14.4` px at 100% — so 21.6 px at
/// **150%**, which is the rung above. `INTERFACE_SCALES` moves the type and the panel together and so
/// cannot change this ratio; only the person's *Text size* can, and the capture's was not 100%.
const CAPTURE_TEXT_SCALE: f32 = 1.5;

/// How far a measured edge may stand past the content box it is held to, physical px.
const SLACK_PX: f32 = 1.0;


/// How many spills one pane reports before the message becomes a wall.
const REPORT_LIMIT: usize = 8;

/// **The most gooey table-cell texts the left pane may draw**: its history list is single-column, and its rows
/// are the only cells there. gooey spawns every cell's text `NoWrap` (`widgets::ONE_LINE`), which no `TextLayout`
/// written here can override — `bevy_gooey#109`, the exemption #482's test states. Counted rather than silently
/// skipped, so it cannot quietly grow to cover a second widget.
const MOST_GOOEY_CELLS: usize = 14;



/// An editor on a fixture with pieces in the library, opened on `mode` with the person's interface size at
/// `interface_scale`, at the capture's window.
///
/// **The window is spawned, and it is what makes the scale real.** `surface::fit_surface_to_window` writes
/// `UiScale` from the window's density and the person's interface size, and both systems that decide a panel's
/// width begin with a `PrimaryWindow` query — without one the editor keeps the harness's 1.2 and no dock is
/// ever squeezed, which is not the layout the capture was taken on.
///
/// **The interface size goes through a settings file**, not a write to `EditorSettings`: `sync_settings`
/// resolves that resource from the files every frame, so a direct write would be gone before the layout ran.
fn editor(tag: &str, mode: Mode, interface_scale: f32) -> App {
    // **An empty library, which is the capture's own**: its PLACE list says `NOT IMPORTED (0)` and
    // `MESHES (0)` and draws the standing *Add meshes…* button — the control the note picks. The packs
    // below give the Kit door's candidate list something to list, so the Pieces panel is not measured
      // on a project that has nothing to draw.
    let root = fixtures::Fixture::new(&format!("panel_box_{tag}_{}", std::process::id()))
        .pack("props", &["crate", "lamp_post", "surveillance_console", "a_rather_long_mesh_name_here"])
        .build("m");
    let user = root.join(format!("user_settings_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(user.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(
        &user,
        format!("version: 1\ntext_scale: {CAPTURE_TEXT_SCALE}\ninterface_scale: {interface_scale}\n"),
    )
    .expect("a settings file");
    let mut app = harness::build_headless_at(&root, "m", None, mode)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.insert_resource(UserSettingsFile(user));
    // The capture's screen: 1280 x 802 logical at a 2.0 factor, so 2560 x 1604 physical. `WindowResolution::new` takes PHYSICAL pixels (#535):
    // it was `new(1280, 802)`, a 640 x 401 logical window, which the interface size now limits (#537) to what it can hold.
    let window = app
        .world_mut()
        .spawn((
            Window { resolution: WindowResolution::new(WINDOW_PHYSICAL.0, WINDOW_PHYSICAL.1), ..default() },
            PrimaryWindow,
        ))
        .id();
    let mut window = app.world_mut().get_mut::<Window>(window).expect("the window just spawned");
    window.resolution.set_scale_factor_override(Some(DISPLAY_FACTOR));
    harness::resize_surface(&mut app, WINDOW_PHYSICAL.0, WINDOW_PHYSICAL.1).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    app
}

/// The `UiScale` the editor settled on, checked against the arithmetic the capture states.
fn check_scale(app: &App, interface_scale: f32, tag: &str) {
    let want = bevy_dark_factory::surface::ui_scale_for_a_person(1280.0, DISPLAY_FACTOR, interface_scale);
    let got = app.world().resource::<UiScale>().0;
    assert!(
        (got - want).abs() < 1e-3,
        "{tag}: the editor draws at interface scale {got}, not the {want} the window and the person's \
         {interface_scale} give — this layout is not the one the capture was taken on"
    );
}

/// A node's border-box rect, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **What a node is drawn over**: its box, widened to the union of the boxes of the glyphs it paints.
///
/// A pixel is drawn where a glyph is, so a run that fills its box is inside it however the laid-out
/// block measured, and a run that overhangs its box is out however the block measured. The block
/// ([`TextLayoutInfo::size`]) is deliberately **not** used: on a wrapped run it keeps the width the
/// text would have had on one line — measured here, `none yet` in a 97 px column reported a 119 px
/// block and wrapped to two lines of at most 97, which would be a spill that paints nothing.
fn drawn(world: &World, entity: Entity) -> Option<Rect> {
    let rect = border_box(world, entity)?;
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    let (Some(text), Some(info), Some(computed)) =
        (world.get::<Text>(entity), world.get::<TextLayoutInfo>(entity), world.get::<ComputedNode>(entity))
    else {
        return Some(rect);
    };
    if text.0.trim().is_empty() {
        return Some(rect);
    }
    let Some(first) = info.glyphs.first() else { return Some(rect) };
    let start = rect.min + computed.content_inset().min_inset;
    let mut painted = Rect::from_corners(start + first.position, start + first.position + first.atlas_info.rect.size());
    for glyph in &info.glyphs {
        let at = start + glyph.position;
        painted = painted.union(Rect::from_corners(at, at + glyph.atlas_info.rect.size()));
    }
    Some(rect.union(painted))
}

/// Every entity under `root` — the root out, a tooltip out (a tooltip is drawn outside its panel on purpose).
fn under(world: &World, root: Entity) -> Vec<Entity> {
    let kids_of = |at: Entity| -> Vec<Entity> {
        world
            .get::<Children>(at)
            .map(|kids| kids.iter().filter(|kid| !is_a_tooltip(world, *kid)).collect())
            .unwrap_or_default()
    };
    let mut found = kids_of(root);
    let mut next = 0;
    while next < found.len() {
        found.extend(kids_of(found[next]));
        next += 1;
    }
    found
}

/// Is this node a tooltip, or inside one?
fn is_a_tooltip(world: &World, entity: Entity) -> bool {
    std::iter::successors(Some(entity), |at| world.get::<ChildOf>(*at).map(ChildOf::parent))
        .any(|at| matches!(world.get::<bevy_gooey::Role>(at), Some(bevy_gooey::Role::Tooltip)))
}

/// What a node is, in words a person can act on.
fn name_of(world: &World, entity: Entity) -> String {
    if let Some(text) = world.get::<Text>(entity)
        && !text.0.trim().is_empty()
    {
        return format!("`{}`", text.0.trim());
    }
    if let Some(name) = world.get::<Name>(entity) {
        return format!("{name}");
    }
    match world.get::<bevy_gooey::Role>(entity) {
        Some(role) => format!("a {role:?}"),
        None => format!("{entity:?}"),
    }
}

/// **Every descendant of `pane` ends at or inside `pane`'s content box**, as sentences naming what did not.
///
/// The content box is the pane's border box less the inset it carries for its border and its padding — the
/// room a control is drawn in.
fn spills(app: &mut App, pane: Entity, tag: &str) -> Vec<String> {
    let world = app.world();
    let Some(pane_box) = border_box(world, pane) else { return vec![format!("{tag}: the pane is not laid out")] };
    let inset = world.get::<ComputedNode>(pane).map(ComputedNode::content_inset).unwrap_or_default();
    let content_right = pane_box.max.x - inset.max_inset.x;
    let content_left = pane_box.min.x + inset.min_inset.x;
    let mut refused = Vec::new();
    let mut cells = 0usize;
    let mut measured = 0usize;
    for entity in under(world, pane) {
        // **gooey's table cell text is drawn on one line and this crate cannot change that** (`bevy_gooey#109`),
        // the one exemption #482's test states. Counted, so it cannot grow unnoticed.
        if matches!(world.get::<bevy_gooey::Role>(entity), Some(bevy_gooey::Role::TableCellText)) {
            cells += 1;
            continue;
        }
        let Some(rect) = drawn(world, entity) else { continue };
        if world.get::<Text>(entity).is_some() {
            measured += 1;
        }
        if rect.max.x > content_right + SLACK_PX {
            refused.push(format!(
                "{tag}: `{}` ends at {:.1}, **{:.1} px past** the content edge {content_right:.1}",
                name_of(world, entity),
                rect.max.x,
                rect.max.x - content_right
            ));
        } else if rect.min.x < content_left - SLACK_PX {
            refused.push(format!(
                "{tag}: `{}` starts at {:.1}, **{:.1} px before** the content edge {content_left:.1}",
                name_of(world, entity),
                rect.min.x,
                content_left - rect.min.x
            ));
        }
        if refused.len() >= REPORT_LIMIT {
            refused.push(format!("{tag}: …and more, the first {REPORT_LIMIT} are above"));
            break;
        }
    }
    if cells > MOST_GOOEY_CELLS {
        refused.push(format!(
            "{tag}: {cells} gooey table cells are drawn under the pane, more than the {MOST_GOOEY_CELLS} its \
             history list has — the exemption for gooey's one-line cell text now covers more than it was \
             written for"
        ));
    }
    if measured == 0 {
        refused.push(format!("{tag}: no text is drawn under the pane, so this measurement says nothing"));
    }
    refused
}

/// **The panel that holds `marker`**, on `side`: the node carrying a [`PanelWidth`] that the marker sits under.
fn panel_over(app: &mut App, marker: Entity, side: Side, what: &str) -> Entity {
    let mut at = marker;
    loop {
        if app.world().get::<PanelWidth>(at).is_some_and(|panel| panel.side == side) {
            return at;
        }
        at = app
            .world()
            .get::<ChildOf>(at)
            .unwrap_or_else(|| panic!("{what} is inside a panel"))
            .parent();
    }
}

/// The one entity carrying `marker`, named for the report.
fn one<T: Component>(app: &mut App, what: &str) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<T>>().single(world).unwrap_or_else(|e| panic!("{what}: {e}"))
}

/// The panel's drawn width in physical px, or `None` when it is not laid out.
fn width_of(app: &App, panel: Entity) -> Option<f32> {
    border_box(app.world(), panel).map(|box_of| box_of.width())
}

/// **The pane the divider leaves on the left**: the frame's left region, inside gooey's `SplitPane` — the box
/// the #482 captures picked and the box a person sees as the pane.
fn left_pane(app: &mut App) -> Entity {
    let world = app.world();
    let left = world.resource::<chrome::Frame>().left;
    world
        .get::<ChildOf>(left)
        .map(ChildOf::parent)
        .expect("the left region stands in the split's pane")
}

/// **At the capture's window, at interface scale 1.0 and 1.6, no pane draws a control past its own content
/// box** — the Place panel (#494), the Pieces panel (#468), the Agent panel (#481) and the left pane (#482),
/// held in one test so the four cannot regress separately.
///
/// **PIECES was the last pane red** (`NOT IMPORTED (4)` 46.0 px past at scale 1.0 and 71.4 at 1.6, its
/// wrapper 61.0 and 95.0), and `chrome::chip`'s own `max_width` and longest-word label closed it — see
/// the module doc. The test was `#[ignore]`d for exactly as long as that fix was missing.
#[test]
fn no_panel_draws_a_control_past_its_own_content_box() {
    let mut refused = Vec::new();
    for interface_scale in INTERFACE_SCALES {
        let tag = format!("[interface scale {interface_scale}]");

        // **The Place panel** (#494), on the Map door: the panel the owner's pick names.
        let mut map = editor("place", Mode::Map, interface_scale);
        check_scale(&map, interface_scale, &tag);
        let palette = one::<bevy_dark_factory::editor::PaletteList>(&mut map, "the PLACE palette is drawn");
        let place = panel_over(&mut map, palette, Side::Right, "the PLACE palette");
        let wide = width_of(&map, place).unwrap_or_else(|| panic!("{tag}: the PLACE panel is not laid out"));
        assert!(wide > 0.0, "{tag}: the PLACE panel has no width, so this measures nothing");
        // **The capture's width, not a number this test chose.** The capture's panel ran 2151..2529: 378 px,
        // which the editor reaches by squeezing the docks at this window. A panel still at `LIST_W` would pass
        // this file on a layout nobody complained about.
        if interface_scale == 1.0 {
            assert!(
                (wide - CAPTURE_PLACE_PANEL_PX).abs() <= WIDTH_SLACK_PX,
                "{tag}: the PLACE panel is drawn {wide:.1} px wide, not the {CAPTURE_PLACE_PANEL_PX:.0} px the \
                 capture measured — this is not the layout #494 was filed about"
            );
        }
        refused.extend(spills(&mut map, place, &format!("{tag} PLACE panel, {wide:.0} px wide")));

        // **The left pane** (#482), at the same window: whatever the divider gives it.
        let pane = left_pane(&mut map);
        refused.extend(spills(&mut map, pane, &format!("{tag} the left pane")));

        // **The Pieces panel** (#468), on the Kit door: the candidate list's own panel.
        let mut kit = editor("pieces", Mode::Meshes, interface_scale);
        check_scale(&kit, interface_scale, &tag);
        let candidates = one::<bevy_dark_factory::tiles::CandidateList>(&mut kit, "the PIECES list is drawn");
        let pieces = panel_over(&mut kit, candidates, Side::Right, "the PIECES list");
        let wide = width_of(&kit, pieces).unwrap_or_else(|| panic!("{tag}: the PIECES panel is not laid out"));
        refused.extend(spills(&mut kit, pieces, &format!("{tag} PIECES panel, {wide:.0} px wide")));

        // **The Agent panel** (#481), on the Agent door: the pane's own panel in the left dock.
        let mut agent = editor("agent", Mode::Agent, interface_scale);
        check_scale(&agent, interface_scale, &tag);
        let pane = one::<bevy_dark_factory::agent_view::AgentPane>(&mut agent, "the AGENT pane is drawn");
        let panel = panel_over(&mut agent, pane, Side::Left, "the AGENT pane");
        let wide = width_of(&agent, panel).unwrap_or_else(|| panic!("{tag}: the AGENT panel is not laid out"));
        refused.extend(spills(&mut agent, panel, &format!("{tag} AGENT panel, {wide:.0} px wide")));
    }
    assert!(
        refused.is_empty(),
        "a panel draws a control outside the content box it is drawn in (#494, #468, #481, #482):\n{}",
        refused.join("\n")
    );
}

/// **Where each laid-out line of a text ends, counted in painted letters**: a space paints nothing, so the painted
/// glyphs are the letters, in order. `None` when the text is not laid out.
fn line_ends(world: &World, text: Entity) -> Option<Vec<usize>> {
    let info = world.get::<TextLayoutInfo>(text)?;
    let mut letters_per_line: Vec<usize> = Vec::new();
    for glyph in info.glyphs.iter().filter(|glyph| glyph.atlas_info.rect.size().min_element() > 0.0) {
        let line = glyph.line_index as usize;
        if letters_per_line.len() <= line {
            letters_per_line.resize(line + 1, 0);
        }
        letters_per_line[line] += 1;
    }
    Some(
        letters_per_line
            .iter()
            .filter(|letters| **letters > 0)
            .scan(0, |letters, on_line| {
                *letters += on_line;
                Some(*letters)
            })
            .collect(),
    )
}

/// Marks the column [`the_agent_panels_title_breaks_between_its_words`] stands a title in.
#[derive(Component)]
struct NarrowColumn;

/// **The Agent panel is titled `NIGHT SHIFT` and breaks between its two words, never inside one** (#692; owner,
/// 2026-11-27: the agent is the Night Shift, two words, as the README spells it).
///
/// The one-word `NIGHTSHIFT` was wider than the AGENT panel at the capture's window, and the title's letter break
/// (`LineBreak::WordOrCharacter`) wrapped it as `NIGHTSHIF` / `T`. Staged at both interface scales, the panel's own
/// title must end each laid-out line where a word of the name ends. Now that the Agent workspace gives its panel
/// the centre (#694) the title fits on one line there, so the same title, built by the same `chrome::title`, is
/// also stood in a column three quarters of its one-line width — narrower than the name, wider than either word —
/// where it must wrap, and must wrap as `NIGHT` / `SHIFT`.
#[test]
fn the_agent_panels_title_breaks_between_its_words() {
    use bevy_dark_factory::follow::AGENT_NAME;
    assert_eq!(AGENT_NAME, "Night Shift", "the agent is spelled the owner's way, two words");
    let title = AGENT_NAME.to_uppercase();
    // Where each word's letters end, counted in letters: `NIGHT SHIFT` is [5, 10].
    let word_ends: Vec<usize> = title
        .split_whitespace()
        .scan(0, |letters, word| {
            *letters += word.chars().count();
            Some(*letters)
        })
        .collect();
    for interface_scale in INTERFACE_SCALES {
        let tag = format!("[interface scale {interface_scale}]");
        let mut agent = editor("agent_title", Mode::Agent, interface_scale);
        check_scale(&agent, interface_scale, &tag);
        let pane = one::<bevy_dark_factory::agent_view::AgentPane>(&mut agent, "the AGENT pane is drawn");
        let panel = panel_over(&mut agent, pane, Side::Left, "the AGENT pane");
        let world = agent.world();
        let heading = under(world, panel)
            .into_iter()
            .find(|at| world.get::<Text>(*at).is_some_and(|text| text.0 == title))
            .unwrap_or_else(|| panic!("{tag}: the AGENT panel has no `{title}` title"));
        let ends = line_ends(world, heading).unwrap_or_else(|| panic!("{tag}: the title is not laid out"));
        assert_eq!(ends.last(), word_ends.last(), "{tag}: the title paints every letter of `{title}`: {ends:?}");
        assert!(
            ends.iter().all(|end| word_ends.contains(end)),
            "{tag}: the panel's `{title}` breaks inside a word: lines end at letters {ends:?}, words at {word_ends:?}"
        );

        // The same title in a column narrower than it: three quarters of its one-line width, in logical px.
        let one_line = world.get::<TextLayoutInfo>(heading).map(|info| info.size.x).unwrap_or_default();
        let to_logical = world.get::<ComputedNode>(heading).map(ComputedNode::inverse_scale_factor).unwrap_or(1.0);
        assert!(one_line > 0.0, "{tag}: the title has no width");
        let narrow = 0.75 * one_line * to_logical;
        agent.world_mut().commands().entity(panel).with_children(|p| {
            p.spawn((Node { width: Val::Px(narrow), ..default() }, NarrowColumn)).with_children(|c| chrome::title(c, &title));
        });
        agent.world_mut().flush();
        settle_support::settle(&mut agent);
        let column = one::<NarrowColumn>(&mut agent, "the narrow column is spawned");
        let world = agent.world();
        let narrowed = under(world, column)
            .into_iter()
            .find(|at| world.get::<Text>(*at).is_some())
            .unwrap_or_else(|| panic!("{tag}: the narrow column holds no title"));
        let ends = line_ends(world, narrowed).unwrap_or_else(|| panic!("{tag}: the narrowed title is not laid out"));
        assert_eq!(
            ends, word_ends,
            "{tag}: `{title}` in a {narrow:.0} px column (its one line is {:.0} px) must wrap between its words, one \
             word a line; its lines end at letters {ends:?}",
            one_line * to_logical
        );
    }
}
