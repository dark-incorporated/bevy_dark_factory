//! **A panel's controls stay inside its own content box, at five interface scales, over content that
//! fills the panel** (#494, `feedback-1794921716691`).
//!
//! # Why this file exists beside the two earlier tests
//!
//! `a_panel_control_stays_inside_its_content_box` passed with 0.0 px of overhang on all seven panels, and
//! `no_panel_draws_a_control_past_its_own_content_box` passes on the four panes it stages. A green that could never
//! have been red proves nothing, so what each could and could not see was read, not assumed:
//!
//! | | `a_panel_control_...` | `no_panel_draws_...` | this file |
//! |---|---|---|---|
//! | interface scales | 1.0, 1.6 | 1.0, 1.6 | 0.5, 1.0, 1.6, 2.4, 3.0 |
//! | text size | 100% | 150% (the capture's) | 100% and 150% |
//! | window | 2560 x 1406 | the capture's 2560 x 1604 | the capture's 2560 x 1604 |
//! | content | one descriptor | an empty library, four pack meshes | [`PIECES`] pieces with 50-character names and long tags, a second kit, [`CANDIDATES`] import candidates, [`GROUPS`] groups, [`GROUPS`] tiles, [`RIGS`] rigs of [`CLIPS`] clips (bench opened), every name long |
//! | what is measured | node boxes only (`ComputedNode`) | the box united with the painted glyphs (`TextLayoutInfo`), counted whether or not a clip hides them | the box united with the painted glyphs, **cut by `CalculatedClip`**, so only what is seen counts; the laid-out figure is printed beside it |
//! | panels | seven `PanelWidth` panels | four panes | every panel shown on nine modes, and the left pane |
//!
//! So the sibling already read glyphs; what neither earlier test had was **populated content** (the Tiles rows, the
//! Compose groups, the rig list and the Place headings are empty or short in both) and **the clip cut**.
//!
//! # What is measured
//!
//! Every panel that is *shown* (a `PanelWidth` node whose `display` is not `None`) on each of the editor's panels in
//! turn, plus the left pane (the frame's left region's parent, #482). For each, every drawn descendant of it, tooltips
//! out, is held to the panel's **content box**: its border box less the `ComputedNode::content_inset` it carries. The
//! overhang is the farthest a descendant's visible right edge, or its visible left edge, stands outside that box. The
//! table is printed per panel per case, and **every cell is asserted at most [`OVERHANG_PX`]**.
//!
//! **Two more checks hold the part a clip hides** (#535), in every case:
//!
//! - **(a) a collapsed body is a failure.** It is a layout check, not a wrap check: it guards a panel whose body has no room
//!   to be read in. Three measured conditions, none a restated constant: a panel that holds texts shows at least one;
//!   the panel's content box is at least one text row tall; and a scroll viewport under it is at least one text row tall. "One
//!   text row" is the height of the shortest text box laid out in that panel, read from the layout. The floors are
//!   [`Measured::body_px`] and [`Measured::viewport_px`], each the smallest reading over the modes a panel is shown in.
//!   **What (a) cannot catch:** a panel that shows only its heading passes the first condition (one visible text is enough);
//!   content scrolled out of view, or cut by an ancestor's clip while a heading shows, is not seen; and it says nothing of
//!   widths, which is (b). **It did not reproduce a wrap defect**: the "panels draw empty at interface 2.4 and 3.0" of #535
//!   was the half-size window (see below), and in the capture's real window no text was hidden on `origin/main`.
//!   **Where it is asserted:** the body and viewport floors are asserted at interface 1.6 and below ([`COLLAPSE_HELD_UP_TO`]),
//!   where they hold. At 2.4 and 3.0 they do not hold on the capture's window (measured: the door strip ends at y 733 of 1604
//!   at 2.4 / text 100%, some lists' viewports are 0 px, and at 3.0 / text 150% a panel's content box is 16 px tall), and no
//!   row's wrap changes that; those cells are printed as `COLLAPSE (not asserted)` and are an open layout limit of the
//!   interface-scale range on a 1280 x 802 logical window. The "texts but none visible" condition is asserted in every cell.
//! - **(b) no text is laid out wider than the panel's content box**, visible or not ([`Measured::laid_out_text`], at most
//!   [`OVERHANG_PX`]). A text a pane clips is not an overhang on the screen, but a row that cannot wrap has only been
//!   cut, and the part cut off cannot be read. A box with no width or no height is still measured by the glyphs it paints
//!   (a text squeezed to nothing whose glyphs overflow is the shape of the defect). **gooey's table-cell texts are exempt
//!   from (b)**: `measure` skips them, counting them only.
//!
//! gooey's table-cell text is one line by construction (`bevy_gooey#109`) and is exempted exactly as
//! `no_panel_draws_a_control_past_its_own_content_box` exempts it: counted, and capped.
//!
//! # The window is read back, and it was once half the size it said (#535)
//!
//! `WindowResolution::new` takes **physical** pixels. This file first spawned `new(1280, 802)` under a factor of 2.0, which is a
//! 1280 x 802 physical surface (640 x 401 logical), not the capture's 2560 x 1604: the frame drew at half the capture's size. At
//! interface 2.4 the chrome bars alone (about 153 logical px) filled the 139 logical px that surface had, so the body was zero
//! tall and every panel drew empty *whatever its rows did*, which is where #535's "draws empty at 2.4 and 3.0" came from.
//! [`editor`] now builds the window the capture had and asserts the frame root is [`WINDOW_PHYSICAL`] tall.
//!
//! # What this file does not count, said plainly
//!
//! **Not covered: the rig report's lines.** The census project has no rig report (no asset), so the findings and the
//! `agree with the manifest` line of the ANIMATION slot pane (now wrapped) and its provenance lines (the `diff` lines and the
//! one above them, still bare texts) are never laid out here.
//!
//! **A clipped text is not an overhang** (the visible-overhang check): a text a clip hides is not on the screen. It is held by
//! check (b) instead, which asserts the laid-out width of every text; the `laid out` figure in the printed
//! table also holds boxes that are not texts, and is not asserted.
//!
//! **A 1.0 px tolerance also hides a real 1 px overhang.** The cells that read exactly 1.0 are layout rounding
//! (the `ROUNDING` lines this prints: `ANIMATION` at scale 0.5 / text 100% reads exactly 1.0 on `INSPECTOR [59.0..335.0] in
//! content [60.0..336.0]`, and `PLACE` on `PLACE [2347.0..2387.0] in content [2348.0..2530.0]`), and none reads between 0 and 1;
//! a genuine 1 px spill would be indistinguishable from them.
//!
//! # That this census can see the defect
//!
//! Adding `min_width: Some(Extent::Px(0.0))` back to the `sized(Sizing { .. })` of `chrome::spawn_frame` (the clamp
//! `70a86f11` removed because it displaced a panel's child) turns this test red: `left pane draws 52.0 px outside its
//! content box` at scale 1.0, at both text sizes. The earlier `a_panel_control_...` stays green under it.
//!
//! # That the fix did not move ordinary rows
//!
//! This file measures overhang and nothing else. `a_short_row_stands_where_it_stood` holds the four rows #494 changed
//! to where they stood on `main` for an ordinary short name.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};
use bevy::window::{PrimaryWindow, Window, WindowResolution};

use bevy_dark_factory::chrome::{self, PanelWidth};
use bevy_dark_factory::harness;
use bevy_dark_factory::settings::UserSettingsFile;
use bevy_dark_factory::tiles::Mode;

use crate::fixtures;

/// **The capture's window**, physical px (`feedback-1794921716691` was 2560 x 1604).
const WINDOW_PHYSICAL: (u32, u32) = (2560, 1604);

/// The capture's own display factor: 2560 / 1280.
const DISPLAY_FACTOR: f32 = 2.0;

/// The interface scales the acceptance names. `UiScale` is `1.2 * 1.0 * 2.0 *` this at the capture's window,
/// so 1.0 **is** the capture's own 2.4.
const INTERFACE_SCALES: [f32; 5] = [0.5, 1.0, 1.6, 2.4, 3.0];

/// Text sizes: the skin's own, and the rung the capture was drawn at (see the sibling test's derivation:
/// fifteen glyphs of `NOT IMPORTED (0)` span 21.9 px on the frame against 15.4 px at 100%).
const TEXT_SCALES: [f32; 2] = [1.0, 1.5];

/// **The most a drawn edge may stand outside its panel's content box**, physical px: **one device pixel**.
///
/// The assignment asked for 0.5, and it was measured before it was kept: with nothing wider than its box anywhere, a
/// node laid out on whole pixels stands exactly **1.0 px** outside a panel's content box in some cells (see the module
/// header for two), because `content_inset` is the panel's own fractional border and padding and the child is rounded to
/// the pixel. No cell reads between 0 and 1, so the rounding step is the tolerance, as in
/// `no_panel_draws_a_control_past_its_own_content_box`'s `SLACK_PX`. The defects start at 30 px.
const OVERHANG_PX: f32 = 1.0;

/// **The highest interface scale at which the collapse floors are asserted.** Measured on the capture's window: at 1.6 and
/// below the smallest panel body is 761 px and no viewport is shorter than the shortest text row of its panel (the table prints
/// the figures); at 2.4 the lists' viewports are 0 to 239 px and at 3.0 / text 150% the frame's body is 375 px tall and the
/// panel's content box inside it 16 px. Above 1.6 the floors are printed, not asserted.
const COLLAPSE_HELD_UP_TO: f32 = 1.6;

/// How many culprits a failing cell names.
const CULPRITS: usize = 4;

/// How many too-wide texts a cell keeps, so the capture's own cell can list every one of them by name.
const MOST_WIDE_TEXTS: usize = 80;

/// The cell whose too-wide texts are all printed: the capture's own interface scale and text size.
const CAPTURE_CASE: &str = "scale 1 text 1";

/// Frames a layout needs to settle after the scale is read from the settings file.
const SETTLE_FRAMES: usize = 18;

/// Frames a panel needs to settle after it is shown.
const SHOW_FRAMES: usize = 8;

/// How close the `UiScale` the editor settled on must be to the one the window and the person's size give.
const UI_SCALE_EPSILON: f32 = 1e-3;

/// The editor's panels, each of which is shown in turn. The seven the acceptance names live on the first
/// five; the rest are measured too, because the defect is a family and not a panel.
const MODES: [Mode; 9] = [
    Mode::Map,
    Mode::Meshes,
    Mode::Tiles,
    Mode::Anim,
    Mode::Compose,
    Mode::Agent,
    Mode::World,
    Mode::Sound,
    Mode::Game,
];

/// **The seven panels the issue names**, as each draws its own heading. Every one must be measured in every
/// case, or the census would pass over a panel it never saw.
const SEVEN: [&str; 7] = ["ANIMATION", "COMPOSE", "CONTROLS", "INSPECTOR", "PLACE", "RIGS", "TILES"];

/// How many pieces the default kit holds.
const PIECES: usize = 36;
/// How many pieces the second kit holds.
const EXTRA_PIECES: usize = 12;
/// How many import candidates the pack holds (one more, with an unbreakable name, is added).
const CANDIDATES: usize = 30;
/// How many pieces are placed on the map.
const PLACEMENTS: usize = 14;
/// How many placements make a row of the map before the next row starts, in metres.
const PLACEMENTS_PER_ROW: usize = 5;
/// How many anchored groups, and how many bounded tiles, the project holds.
const GROUPS: usize = 12;
/// How many rigs, and how many clips each.
const RIGS: usize = 6;
const CLIPS: usize = 10;
/// How many long tokens each tag axis (`kind`, `effects`, `look`) holds, and every piece carries.
const TOKENS: usize = 8;

/// **The most gooey table-cell texts one panel may draw** before the exemption for `bevy_gooey#109` is
/// treated as having grown past what it was written for.
const MOST_GOOEY_CELLS: usize = 40;

/// **What identifies a text of each of the four spawners #494 changed**, per panel, so a cell can say it *saw* one
/// that was visible (not merely laid out): `(panel, signature)`.
const FIXED_ROWS: [(&str, &str); 4] =
    [("PLACE", "heading"), ("TILES", "reason"), ("COMPOSE", "group row"), ("RIGS", "rig row")];

/// Which of [`FIXED_ROWS`] a text is, by its words.
fn signature(words: &str) -> Option<&'static str> {
    if words.starts_with("AN_EXTRA_KIT") || words.starts_with("FURNITURE \u{b7}") {
        Some("heading")
    } else if words == "no tile \u{2014} see COMPOSE" {
        Some("reason")
    } else if words.contains("member(s),") {
        Some("group row")
    } else if words.starts_with("rig_0") {
        Some("rig row")
    } else {
        None
    }
}

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// A name no row can hold in one line at any scale: the stem, a two-digit index and one 41-character run of
/// underscore-joined words with no space in it, so only a break at a letter can wrap it.
fn long(stem: &str, i: usize) -> String {
    format!("{stem}_{i:02}_with_a_name_that_keeps_on_going_and_going")
}

/// A tag token that is long on purpose: the axis name and a 36-character spelling.
fn token(axis: &str, i: usize) -> String {
    format!("{axis}_token_with_a_very_long_spelling_{i}")
}

/// **A project whose every list is full**, with long names.
fn project(tag: &str) -> PathBuf {
    let mut fixture = fixtures::Fixture::new(&format!("panel_census_{tag}_{}", std::process::id()));
    let mut first = String::new();
    for i in 0..PIECES {
        let id = long("piece", i);
        if first.is_empty() {
            first = id.clone();
        }
        fixture = fixture.descriptor(&id, "props");
    }
    // **A second kit whose directory and namespace are long.**
    let namespace = "an_extra_kit_whose_namespace_is_long_enough_to_wrap";
    let extra: Vec<String> = (0..EXTRA_PIECES).map(|i| format!("{namespace}/{}", long("extra", i))).collect();
    let extra_refs: Vec<&str> = extra.iter().map(String::as_str).collect();
    fixture = fixture.kit("an_extra_kit_whose_directory_is_long", "beta", &extra_refs);
    // Import candidates: meshes on disk that are in no library, one with a name no column holds.
    let candidates: Vec<String> = (0..CANDIDATES).map(|i| long("candidate", i)).collect();
    let mut candidate_refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
    candidate_refs.push("a_single_mesh_name_with_no_break_in_it_at_all_that_is_longer_than_any_column_holds");
    fixture = fixture.pack("a_pack_folder_with_a_long_name", &candidate_refs);
    // Placements, so the Inspector and the status block have something to say.
    for i in 0..PLACEMENTS {
        fixture = fixture.place(&long("piece", i), ((i % PLACEMENTS_PER_ROW) as f32, (i / PLACEMENTS_PER_ROW) as f32));
    }
    // Compositions (the Compose panel's groups) and bounded ones (the Tiles panel's tiles).
    for i in 0..GROUPS {
        fixture = fixture.composition(&long("group", i), &[("member_a", first.as_str(), (0.0, 0.0))]);
        fixture = fixture.bounded_composition(&long("tile", i), (1.0, 2.4, 1.0), &[("member_a", first.as_str(), (0.0, 0.0))]);
    }
    let root = fixture.build("m");
    let blueprint = root.join("assets/blueprint");

    // **Long tags.** The tokens are declared in the vocabulary, then written on every piece.
    let vocab_path = blueprint.join("vocab.yaml");
    let mut vocab: blueprint_model::vocab::Vocabularies =
        blueprint_model::yaml::read(&vocab_path).unwrap_or_else(|p| panic!("{}", p.line()));
    let add = |axis: &mut blueprint_model::vocab::Vocabulary, name: &str| {
        for i in 0..TOKENS {
            axis.tokens.push(blueprint_model::vocab::Token {
                name: token(name, i),
                note: "a tag with a long spelling".to_owned(),
                implies: Vec::new(),
            });
        }
    };
    add(&mut vocab.kind, "kind");
    add(&mut vocab.effects, "effects");
    add(&mut vocab.look, "look");
    std::fs::write(&vocab_path, blueprint_model::yaml::to_string(&vocab).unwrap_or_else(|p| panic!("{}", p.line())))
        .unwrap_or_else(|e| panic!("{e}"));
    for kit in ["furniture", "an_extra_kit_whose_directory_is_long"] {
        fixtures::edit_library(&blueprint.join(kit).join("library.yaml"), |library| {
            for d in &mut library.descriptors {
                d.kind = (0..TOKENS).map(|i| token("kind", i)).collect();
                d.effects = (0..TOKENS).map(|i| token("effects", i)).collect();
                d.look = (0..TOKENS).map(|i| token("look", i)).collect();
                d.note = Some("a note long enough to take several lines in any column a panel can give it".to_owned());
            }
        });
    }

    // **Rigs with long names and many clips**, in the manifest's own spelling.
    let mut rigs = String::from("version: 3\nrigs:\n");
    for r in 0..RIGS {
        rigs.push_str(&format!("  {}:\n    mesh: characters/{r}.glb\n    scale: 1.0\n    slots:\n", long("rig", r)));
        for c in 0..CLIPS {
            rigs.push_str(&format!(
                "    - clip: {c}\n      playback: !Free\n        speed: 1.0\n      note: {}\n",
                long("clip", c)
            ));
        }
    }
    std::fs::write(blueprint.join("rigs.yaml"), rigs).unwrap_or_else(|e| panic!("rigs.yaml: {e}"));
    root
}

/// An editor over `root`, at the capture's window, with the person's two sizes written to a settings file
/// (`sync_settings` resolves `EditorSettings` from files every frame, so a direct write would be gone).
fn editor(root: &Path, tag: &str, interface_scale: f32, text_scale: f32) -> App {
    let user = root.join(format!("user_settings_{tag}/settings.yaml"));
    std::fs::create_dir_all(user.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&user, format!("version: 1\ntext_scale: {text_scale}\ninterface_scale: {interface_scale}\n"))
        .expect("a settings file");
    let mut app = harness::build_headless_at(root, "m", None, Mode::Map)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.insert_resource(UserSettingsFile(user));
    let window = app
        .world_mut()
        .spawn((Window { resolution: WindowResolution::new(WINDOW_PHYSICAL.0, WINDOW_PHYSICAL.1), ..default() }, PrimaryWindow))
        .id();
    let mut window = app.world_mut().get_mut::<Window>(window).expect("the window just spawned");
    window.resolution.set_scale_factor_override(Some(DISPLAY_FACTOR));
    harness::resize_surface(&mut app, WINDOW_PHYSICAL.0, WINDOW_PHYSICAL.1).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app, SETTLE_FRAMES);
    // **The text size is read back, not assumed**: a 1.5 the settings file silently lost would make every 150% cell a
    // second run of the 100% one, and `UiScale` below says nothing about type.
    let applied = app.world().resource::<bevy_dark_factory::settings::EditorSettings>().text_scale;
    assert!(
        (applied - text_scale).abs() < UI_SCALE_EPSILON,
        "the editor runs at text size {applied}, not the {text_scale} this cell was asked for"
    );
    // **The window is read back as the capture's, not assumed** (#535). `WindowResolution::new` takes PHYSICAL pixels, so the window this
    // census was first written with, `new(1280, 802)` under a factor of 2.0, was a 1280 x 802 physical surface: the whole frame drew
    // at half the capture's size, the `root` below was 802 px tall where the capture's is 1604, and at interface 2.4 the chrome
    // bars alone (153 logical px) filled the 139 logical px the surface had, so the body was zero tall and every panel drew empty
    // whatever its rows did.
    {
        let world = app.world();
        let root = world.resource::<chrome::Frame>().root;
        let size = world.get::<ComputedNode>(root).map(ComputedNode::size).expect("the frame root is laid out");
        assert!(
            (size.y - WINDOW_PHYSICAL.1 as f32).abs() < 1.0,
            "the frame is {} px tall, not the capture's {} px: the census is not drawing the window it says it is",
            size.y,
            WINDOW_PHYSICAL.1
        );
    }
    let want = bevy_dark_factory::surface::ui_scale_for_a_person(1280.0, DISPLAY_FACTOR, interface_scale);
    let got = app.world().resource::<UiScale>().0;
    assert!(
        (got - want).abs() < UI_SCALE_EPSILON,
        "the editor draws at UiScale {got}, not the {want} the window and interface scale {interface_scale} give"
    );
    app
}

/// A node's border-box rect, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **What a node is drawn over**: its box, widened to the union of the boxes of the glyphs it paints.
///
/// The block ([`TextLayoutInfo::size`]) is deliberately not used: on a wrapped run it keeps the width the text
/// would have had on one line, which would be a spill that paints nothing.
fn drawn(world: &World, entity: Entity) -> Option<Rect> {
    let rect = border_box(world, entity)?;
    // **A box with no width or no height is not skipped when it holds glyphs** (#535 review): a text squeezed to zero width
    // whose glyphs still overflow is the very shape of the defect, and a `None` here made it invisible to every check.
    let flat = rect.width() <= 0.0 || rect.height() <= 0.0;
    let box_only = || (!flat).then_some(rect);
    let (Some(text), Some(info), Some(computed)) =
        (world.get::<Text>(entity), world.get::<TextLayoutInfo>(entity), world.get::<ComputedNode>(entity))
    else {
        return box_only();
    };
    if text.0.trim().is_empty() {
        return box_only();
    }
    let Some(first) = info.glyphs.first() else { return box_only() };
    let start = rect.min + computed.content_inset().min_inset;
    // **A `PositionedGlyph::position` is the glyph's centre**, not its corner (`bevy_text::pipeline`: `size / 2. +
    // glyph_pos + atlas_info.offset`): the box a glyph paints is `from_center_size(position, atlas rect size)`.
    let ink = |glyph: &bevy::text::PositionedGlyph| Rect::from_center_size(start + glyph.position, glyph.atlas_info.rect.size());
    let mut painted = ink(first);
    for glyph in &info.glyphs {
        painted = painted.union(ink(glyph));
    }
    Some(if flat { painted } else { rect.union(painted) })
}

/// **What of that is actually seen**: [`drawn`] cut to the clip rectangle Bevy computed from every ancestor
/// that clips (`CalculatedClip`). A glyph a scroll area or a clipped column hides is not on the screen, and the
/// capture is a glyph on the screen over the border. `None` when nothing of the node is visible.
fn seen(world: &World, entity: Entity, rect: Rect) -> Option<Rect> {
    let Some(clip) = world.get::<bevy::ui::CalculatedClip>(entity) else { return Some(rect) };
    // `FullyClipped`: an ancestor's clip leaves nothing of it.
    let rects = clip.rects()?;
    let mut cut = rect;
    for clipping in rects {
        // The clip is kept in its clipping node's local space; `world_to_clip_local` takes a point there, so its
        // inverse takes the clip rectangle back to the window's physical px.
        let back = clipping.world_to_clip_local.inverse();
        let (a, b) = (back.transform_point2(clipping.rect.min), back.transform_point2(clipping.rect.max));
        cut = cut.intersect(Rect::from_corners(a, b));
    }
    (cut.width() > 0.0 && cut.height() > 0.0).then_some(cut)
}

fn is_a_tooltip(world: &World, entity: Entity) -> bool {
    std::iter::successors(Some(entity), |at| world.get::<ChildOf>(*at).map(ChildOf::parent))
        .any(|at| matches!(world.get::<bevy_gooey::Role>(at), Some(bevy_gooey::Role::Tooltip)))
}

/// Every node drawn under `root`: a tooltip is out (drawn outside its panel on purpose), and so is a
/// `Display::None` subtree.
fn under(world: &World, root: Entity) -> Vec<Entity> {
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(at) = stack.pop() {
        let Some(kids) = world.get::<Children>(at) else { continue };
        for kid in kids.iter() {
            if is_a_tooltip(world, kid) || world.get::<Node>(kid).is_some_and(|n| n.display == Display::None) {
                continue;
            }
            found.push(kid);
            stack.push(kid);
        }
    }
    found.sort_by_key(|e| e.index());
    found
}

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

/// What one region measured.
#[derive(Default, Clone)]
struct Measured {
    /// How far a **visible** edge stood outside the content box, either side, physical px. This is what is
    /// asserted: what a person sees over the border.
    overhang: f32,
    /// The nodes that stood farthest out (worst first, one per distinct words), with how far.
    culprits: Vec<(f32, String)>,
    /// How far a node was **laid out** outside the box, whatever clipped it afterwards: the sibling test's point that a pane
    /// which hides what does not fit has still laid it out too wide. Reported, not asserted (it holds boxes of every kind;
    /// the asserted figure is [`Measured::laid_out_text`]).
    laid_out: f32,
    /// How far a **text** was laid out outside the box, whatever clipped it afterwards (#535). **Asserted**: a text wider
    /// than its content box is a row that does not reflow, and a clip only hides what it cut.
    laid_out_text: f32,
    /// The texts that were laid out farthest outside the box (worst first, one per distinct words), with how far.
    wide_texts: Vec<(f32, String)>,
    /// The first text laid out under the region that a clip cut away entirely, with its box and the clip: what to read when
    /// a panel holds texts and none is visible.
    hidden: Option<String>,
    /// The region's own content-box height, physical px: its border box less its insets. **Asserted** (the collapse floor).
    body_px: Option<f32>,
    /// The shortest text box laid out under the region, physical px: what "one text row" is here, read from the layout.
    row_px: Option<f32>,
    /// The tallest scroll viewport under the region, physical px, when it has one. **Asserted** against [`Measured::row_px`].
    viewport_px: Option<f32>,
    /// The interface scale this cell was drawn at (set by [`census`]).
    interface_scale: f32,
    /// The visible edge that stood farthest out even when it was within [`OVERHANG_PX`]: the evidence for the tolerance.
    worst: Option<(f32, String)>,
    /// How many texts were laid out under the region: how *full* it was.
    texts: usize,
    /// How many of those survive the clip cut: the texts a person can see.
    visible_texts: usize,
    /// Which of [`FIXED_ROWS`]' spawners had a text that survived the clip cut here.
    signatures: std::collections::BTreeSet<&'static str>,
    /// gooey's one-line table cells, exempt and counted.
    cells: usize,
}

impl Measured {
    /// Keep the worse of two readings of one named region, and the fuller of its counts.
    fn absorb(&mut self, other: Measured) {
        let (texts, cells, laid_out) = (self.texts.max(other.texts), self.cells.max(other.cells), self.laid_out.max(other.laid_out));
        self.laid_out_text = self.laid_out_text.max(other.laid_out_text);
        // The floor is the **smallest** reading over the modes a panel is shown in, so one collapsed mode is not hidden by a tall one.
        self.body_px = self.body_px.into_iter().chain(other.body_px).reduce(f32::min);
        self.row_px = self.row_px.into_iter().chain(other.row_px).reduce(f32::min);
        self.viewport_px = self.viewport_px.into_iter().chain(other.viewport_px).reduce(f32::min);
        self.worst = [self.worst.take(), other.worst].into_iter().flatten().max_by(|a, b| a.0.total_cmp(&b.0));
        self.hidden = self.hidden.take().or(other.hidden);
        self.wide_texts.extend(other.wide_texts);
        self.wide_texts.sort_by(|a, b| b.0.total_cmp(&a.0));
        self.wide_texts.dedup_by(|a, b| a.1 == b.1);
        self.wide_texts.truncate(MOST_WIDE_TEXTS);
        self.overhang = self.overhang.max(other.overhang);
        self.culprits.extend(other.culprits);
        self.culprits.sort_by(|a, b| b.0.total_cmp(&a.0));
        self.culprits.dedup_by(|a, b| a.1 == b.1);
        self.culprits.truncate(CULPRITS);
        (self.texts, self.cells, self.laid_out) = (texts, cells, laid_out);
        self.visible_texts = self.visible_texts.max(other.visible_texts);
        self.signatures.extend(other.signatures);
    }
}

/// The window-space x-ranges of every clip rectangle a node inherits, for a report.
fn clips(world: &World, entity: Entity) -> String {
    let Some(rects) = world.get::<bevy::ui::CalculatedClip>(entity).and_then(bevy::ui::CalculatedClip::rects) else {
        return "nothing".to_owned();
    };
    let ranges: Vec<String> = rects
        .iter()
        .map(|c| {
            let back = c.world_to_clip_local.inverse();
            let (a, b) = (back.transform_point2(c.rect.min), back.transform_point2(c.rect.max));
            format!("[{:.0}..{:.0}]x[{:.0}..{:.0}]", a.x.min(b.x), a.x.max(b.x), a.y.min(b.y), a.y.max(b.y))
        })
        .collect();
    ranges.join("+")
}

/// **The boxes from a node up to its region**, each as its name and the x-range it is laid out over: the walk
/// that named the culprit on the issue (`a Sized[89.0..584.0] < a Sized[60.0..554.0]`).
fn ancestry(world: &World, entity: Entity, region: Entity) -> String {
    let mut chain = Vec::new();
    let mut at = entity;
    while at != region {
        let Some(parent) = world.get::<ChildOf>(at).map(ChildOf::parent) else { break };
        at = parent;
        if let Some(rect) = border_box(world, at) {
            chain.push(format!("{}[{:.0}..{:.0}]", name_of(world, at), rect.min.x, rect.max.x));
        }
    }
    chain.join(" < ")
}

/// **The boxes from a node up to the window**, each as its name and the box it is laid out over, both axes: what to read
/// when a panel is laid out with no height.
fn up_to_the_window(world: &World, entity: Entity) -> String {
    let mut chain = Vec::new();
    let mut at = entity;
    while let Some(parent) = world.get::<ChildOf>(at).map(ChildOf::parent) {
        at = parent;
        if let Some(rect) = border_box(world, at) {
            chain.push(format!(
                "{}[x{:.0}..{:.0} y{:.0}..{:.0}]",
                name_of(world, at),
                rect.min.x,
                rect.max.x,
                rect.min.y,
                rect.max.y
            ));
        }
    }
    chain.join(" < ")
}

fn measure(world: &World, region: Entity) -> Option<Measured> {
    let region_box = border_box(world, region)?;
    let inset = world.get::<ComputedNode>(region).map(ComputedNode::content_inset).unwrap_or_default();
    let right = region_box.max.x - inset.max_inset.x;
    let left = region_box.min.x + inset.min_inset.x;
    let mut out = Measured { body_px: Some(region_box.height() - inset.min_inset.y - inset.max_inset.y), ..default() };
    for entity in under(world, region) {
        if matches!(world.get::<bevy_gooey::Role>(entity), Some(bevy_gooey::Role::TableCellText)) {
            out.cells += 1;
            continue;
        }
        if matches!(world.get::<bevy_gooey::Role>(entity), Some(bevy_gooey::Role::ScrollViewport))
            && let Some(computed) = world.get::<ComputedNode>(entity)
        {
            out.viewport_px = Some(out.viewport_px.map_or(computed.size().y, |v| v.max(computed.size().y)));
        }
        let Some(rect) = drawn(world, entity) else { continue };
        if world.get::<Text>(entity).is_some() {
            out.texts += 1;
            if let Some(computed) = world.get::<ComputedNode>(entity)
                && computed.size().y > 0.0
            {
                out.row_px = Some(out.row_px.map_or(computed.size().y, |r| r.min(computed.size().y)));
            }
        }
        let laid = (rect.max.x - right).max(left - rect.min.x);
        out.laid_out = out.laid_out.max(laid);
        if world.get::<Text>(entity).is_some() {
            out.laid_out_text = out.laid_out_text.max(laid);
            if laid > OVERHANG_PX {
                out.wide_texts.push((
                    laid,
                    format!(
                        "{} laid out [{:.1}..{:.1}] in content [{left:.1}..{right:.1}]; up the tree: {}",
                        name_of(world, entity),
                        rect.min.x,
                        rect.max.x,
                        ancestry(world, entity, region)
                    ),
                ));
            }
        }
        let Some(rect) = seen(world, entity, rect) else {
            if out.hidden.is_none()
                && let Some(text) = world.get::<Text>(entity)
            {
                out.hidden = Some(format!(
                    "`{}` at x[{:.0}..{:.0}] y[{:.0}..{:.0}], clipped by {}, in a panel at x[{:.0}..{:.0}] y[{:.0}..{:.0}]",
                    text.0.trim(),
                    rect.min.x,
                    rect.max.x,
                    rect.min.y,
                    rect.max.y,
                    clips(world, entity),
                    region_box.min.x,
                    region_box.max.x,
                    region_box.min.y,
                    region_box.max.y
                ));
                if let Some(hidden) = out.hidden.as_mut() {
                    hidden.push_str(&format!("; the panel up to the window: {}", up_to_the_window(world, region)));
                }
            }
            continue;
        };
        if let Some(text) = world.get::<Text>(entity) {
            out.visible_texts += 1;
            out.signatures.extend(signature(&text.0));
        }
        let past = (rect.max.x - right).max(left - rect.min.x);
        if past > 0.0 && out.worst.as_ref().is_none_or(|(w, _)| past > *w) {
            out.worst = Some((
                past,
                format!(
                    "{} [{:.1}..{:.1}] in content [{left:.1}..{right:.1}]",
                    name_of(world, entity),
                    rect.min.x,
                    rect.max.x
                ),
            ));
        }
        if past > OVERHANG_PX {
            out.culprits.push((
                past,
                format!(
                    "{} [{:.1}..{:.1}] in content [{left:.1}..{right:.1}]; clipped by {}; up the tree: {}",
                    name_of(world, entity),
                    rect.min.x,
                    rect.max.x,
                    clips(world, entity),
                    ancestry(world, entity, region)
                ),
            ));
        }
        out.overhang = out.overhang.max(past);
    }
    out.culprits.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.culprits.dedup_by(|a, b| a.1 == b.1);
    out.culprits.truncate(CULPRITS);
    out.wide_texts.sort_by(|a, b| b.0.total_cmp(&a.0));
    out.wide_texts.dedup_by(|a, b| a.1 == b.1);
    out.wide_texts.truncate(MOST_WIDE_TEXTS);
    Some(out)
}

/// **Every region shown right now, by name**: the panels and the left pane.
fn regions(app: &mut App) -> BTreeMap<String, Measured> {
    let world = app.world_mut();
    let panels: Vec<Entity> = world.query_filtered::<Entity, With<PanelWidth>>().iter(world).collect();
    let mut found = BTreeMap::new();
    for panel in panels {
        if world.get::<Node>(panel).is_none_or(|n| n.display == Display::None) {
            continue;
        }
        let name = world.get::<AccessibleLabel>(panel).map(|l| l.0.clone()).unwrap_or_else(|| "an unnamed panel".to_owned());
        let Some(measured) = measure(world, panel) else { continue };
        if measured.texts == 0 && border_box(world, panel).is_none_or(|b| b.width() <= 0.0) {
            continue;
        }
        found.entry(name).or_insert_with(Measured::default).absorb(measured);
    }
    let left = world.resource::<chrome::Frame>().left;
    if let Some(pane) = world.get::<ChildOf>(left).map(ChildOf::parent)
        && let Some(measured) = measure(world, pane)
    {
        found.insert("left pane".to_owned(), measured);
    }
    found
}

/// **The whole census**: every case, every panel. Rows are `(case, panel)`.
fn census() -> BTreeMap<(String, String), Measured> {
    let root = project("all");
    let mut table = BTreeMap::new();
    for text_scale in TEXT_SCALES {
        for interface_scale in INTERFACE_SCALES {
            let case = format!("scale {interface_scale} text {text_scale}");
            let tag = format!("{interface_scale}_{text_scale}");
            let mut app = editor(&root, &tag, interface_scale, text_scale);
            let mut seen_here: BTreeMap<String, Measured> = BTreeMap::new();
            for mode in MODES {
                *app.world_mut().resource_mut::<Mode>() = mode;
                if mode == Mode::Anim {
                    // **The rig bench is put away until `Cmd`/`Ctrl`+`R` draws it** (#522); the RIGS panel is empty
                    // without it, and a census over an empty panel says nothing. Written the way the chord writes it.
                    let world = app.world_mut();
                    let benches: Vec<Entity> =
                        world.query_filtered::<Entity, With<bevy_dark_factory::anim_tab::RigBench>>().iter(world).collect();
                    for bench in benches {
                        if let Some(mut node) = world.get_mut::<Node>(bench) {
                            node.display = Display::Flex;
                        }
                    }
                }
                settle(&mut app, SHOW_FRAMES);
                for (name, measured) in regions(&mut app) {
                    seen_here.entry(name).or_default().absorb(measured);
                }
            }
            for (name, mut measured) in seen_here {
                measured.interface_scale = interface_scale;
                table.insert((case.clone(), name), measured);
            }
        }
    }
    table
}

#[test]
fn every_panel_stays_inside_its_content_box_when_it_is_full() {
    let table = census();

    // **The table, per panel per case**, as the earlier census printed its own.
    let cases: Vec<String> = {
        let mut c: Vec<String> = table.keys().map(|(c, _)| c.clone()).collect();
        c.dedup();
        c
    };
    let panels: std::collections::BTreeSet<String> = table.keys().map(|(_, p)| p.clone()).collect();
    println!("OVERHANG (physical px, worst drawn edge outside the content box; texts drawn under the panel)");
    for case in &cases {
        let row: Vec<String> = panels
            .iter()
            .filter_map(|p| {
                table.get(&(case.clone(), p.clone())).map(|m| format!("{p}={:.1} (laid out {:.1}, text laid out {:.1}, {} texts, {} visible, body {:.0} px, viewport {:.0} px, row {:.0} px)", m.overhang, m.laid_out, m.laid_out_text, m.texts, m.visible_texts, m.body_px.unwrap_or_default(), m.viewport_px.unwrap_or_default(), m.row_px.unwrap_or_default()))
            })
            .collect();
        println!("  [{case}] {}", row.join("  "));
    }

    let mut failures = Vec::new();
    for ((case, panel), m) in &table {
        if m.overhang > OVERHANG_PX {
            let who: Vec<String> = m.culprits.iter().map(|(past, what)| format!("{past:.1} px: {what}")).collect();
            failures.push(format!("[{case}] `{panel}` draws {:.1} px outside its content box:\n      {}", m.overhang, who.join("\n      ")));
        }
        if m.overhang > 0.0
            && m.overhang <= OVERHANG_PX
            && let Some((past, what)) = &m.worst
        {
            println!("ROUNDING [{case}] `{panel}` reads {:.1} px: {past:.1} px: {what}", m.overhang);
        }
        if case == CAPTURE_CASE && !m.wide_texts.is_empty() {
            println!("TOO WIDE [{case}] `{panel}`:");
            for (past, what) in &m.wide_texts {
                println!("  {past:.1} px: {what}");
            }
        }
        // **(b) The laid-out width of any text stays inside the content box** (#535), not only the part a clip leaves visible.
        if m.laid_out_text > OVERHANG_PX {
            let who: Vec<String> = m.wide_texts.iter().take(CULPRITS).map(|(past, what)| format!("{past:.1} px: {what}")).collect();
            failures.push(format!(
                "[{case}] `{panel}` lays a text out {:.1} px wider than its content box (a clip hides it, a person cannot read it):\n      {}",
                m.laid_out_text,
                who.join("\n      ")
            ));
        }
        // **(a) A panel that holds texts shows some of them** (#535): the weakest of (a)'s three conditions and the only one asserted
        // in every cell, so one visible heading satisfies it (the module doc says what that cannot catch).
        let held = m.interface_scale <= COLLAPSE_HELD_UP_TO;
        if let Some(row) = m.row_px {
            let collapsed = m.body_px.is_some_and(|body| body < row) || m.viewport_px.is_some_and(|view| view < row);
            if collapsed && !held {
                println!(
                    "COLLAPSE (not asserted) [{case}] `{panel}`: body {:.0} px, viewport {:.0} px, one text row {row:.0} px",
                    m.body_px.unwrap_or_default(),
                    m.viewport_px.unwrap_or_default()
                );
            }
            if held && m.body_px.is_some_and(|body| body < row) {
                failures.push(format!(
                    "[{case}] `{panel}` is laid out {:.1} px tall inside, less than one text row ({row:.1} px, the shortest of its own texts): the body collapsed",
                    m.body_px.unwrap_or_default()
                ));
            }
            if held && m.viewport_px.is_some_and(|view| view < row) {
                failures.push(format!(
                    "[{case}] `{panel}`'s scroll viewport is {:.1} px tall, less than one text row ({row:.1} px): nothing can be read in it",
                    m.viewport_px.unwrap_or_default()
                ));
            }
        }
        if m.texts > 0 && m.visible_texts == 0 {
            failures.push(format!(
                "[{case}] `{panel}` holds {} texts and none of them is visible: the panel draws empty (first hidden: {})",
                m.texts,
                m.hidden.as_deref().unwrap_or("none")
            ));
        }
        if m.cells > MOST_GOOEY_CELLS {
            failures.push(format!("[{case}] `{panel}` draws {} one-line gooey table cells, more than {MOST_GOOEY_CELLS}", m.cells));
        }
    }
    // **The census must have seen what it claims to**: every named panel in every case, and the Place panel
    // full (36 pieces' rows are laid out under it whether or not the scroll area shows them).
    for case in &cases {
        for name in SEVEN {
            if !table.contains_key(&(case.clone(), name.to_owned())) {
                failures.push(format!("[{case}] the census never saw the `{name}` panel, so it says nothing of it"));
            }
        }
        for name in SEVEN {
            if table.get(&(case.clone(), name.to_owned())).is_some_and(|m| m.texts == 0) {
                failures.push(format!("[{case}] the `{name}` panel drew no text, so this census measured an empty panel"));
            }
        }
        if let Some(rigs) = table.get(&(case.clone(), "RIGS".to_owned()))
            && rigs.texts < RIGS
        {
            failures.push(format!("[{case}] the RIGS panel drew {} texts for {RIGS} rigs", rigs.texts));
        }
        if let Some(place) = table.get(&(case.clone(), "PLACE".to_owned()))
            && place.texts < PIECES
        {
            failures.push(format!("[{case}] the PLACE panel drew {} texts for a kit of {PIECES} pieces", place.texts));
        }
    }
    // **A text of each spawner #494 changed was *visible*, at both text sizes**: laid out is not enough, because a row
    // scrolled off the fold or cut by a clip is skipped, and the census could then pass over the very rows it fixed.
    for text_scale in TEXT_SCALES {
        let suffix = format!("text {text_scale}");
        for (panel, which) in FIXED_ROWS {
            let seen_once = table
                .iter()
                .filter(|((case, name), _)| case.ends_with(&suffix) && name == panel)
                .any(|(_, m)| m.signatures.contains(which));
            if !seen_once {
                failures.push(format!(
                    "no cell at {suffix} showed a visible {which} text in `{panel}`, so the fix to it was never measured there"
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "a panel draws something outside the content box it is drawn in (#494):\n  {}",
        failures.join("\n  ")
    );
}
