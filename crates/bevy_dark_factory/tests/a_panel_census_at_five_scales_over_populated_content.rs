//! **A panel's controls stay inside its own content box, at five interface scales, over content that
//! fills the panel** (#494, `feedback-1794921716691`).
//!
//! # Why this file exists beside `a_panel_control_stays_inside_its_content_box`
//!
//! That test passes with 0.0 px of overhang on all seven panels, and a green that could never have been
//! red proves nothing. Measured against the capture, it differs from it in four ways, and this file
//! removes each:
//!
//! | The earlier census | The capture | This file |
//! |---|---|---|
//! | `interface_scale` 1.0 and 1.6 | `ui_scale` 2.4 at 2560 x 1604 physical | 0.5, 1.0, 1.6, 2.4 and 3.0, **at the capture's window**, 2560 x 1604 at a 2.0 display factor |
//! | text at 100% | the capture's glyph pitch is the 150% rung ([`CAPTURE_TEXT_SCALE`]) | both 100% and 150% |
//! | one descriptor, nothing in any list | a panel populated from a kit | 36 pieces with 50-character names, long kind/effect/look tokens, a second kit with a long name, 30 import candidates, 14 placements, 12 compositions and 12 tiles with long names, 6 rigs of 10 clips (the rig bench opened, as `Cmd`/`Ctrl`+`R` opens it) |
//! | a node's **box** against the content box | the capture is a *glyph* drawn over the border | the union of the box and **the glyphs it paints** (`TextLayoutInfo`), so a text wider than its node counts |
//!
//! The last row is the one that matters most: a node can be narrower than the words in it, and the old
//! census, which read only `ComputedNode`, could not see that half of the defect.
//!
//! # What is measured
//!
//! Every panel that is *shown* (a `PanelWidth` node whose `display` is not `None`) on each of the editor's
//! panels in turn, plus the left pane (the frame's left region's parent, #482). For each, every drawn
//! descendant of it, tooltips out, is held to the panel's **content box**: its border box less the
//! `ComputedNode::content_inset` it carries. The overhang is the farthest a descendant's drawn right edge,
//! or the farthest its drawn left edge, stands outside that box. The table is printed per panel per case,
//! and **every cell is asserted at most [`OVERHANG_PX`]**.
//!
//! gooey's table-cell text is one line by construction (`bevy_gooey#109`) and is exempted exactly as
//! `no_panel_draws_a_control_past_its_own_content_box` exempts it: counted, and capped.
//!
//! # That this census can see the defect
//!
//! the mutation is recorded where it was run (the commit body and the issue comment): the frame root's
//! `min_width: Some(Extent::Px(0.0))`, which `70a86f11` took *out* because it displaced a panel's child 29 px.

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
/// The assignment asked for 0.5, and it was measured before it was kept: at interface scales 0.5, 1.0, 1.6 and
/// 3.0, with nothing wider than its box anywhere, a node whose box is laid out on whole pixels stands exactly
/// **1.0 px** outside a panel's content box (`a ScrollViewport [59.0..301.0] in content [60.0..302.0]`; `a Sized
/// [1061.0..1380.0] in content [1060.0..1379.0]`): `content_inset` is the panel's own fractional border and
/// padding and the child is rounded to the pixel. Every such cell reads 1.0 and none reads between 0 and 1, so the
/// rounding step is the tolerance, as in `no_panel_draws_a_control_past_its_own_content_box`'s `SLACK_PX`. The
/// defects start at 30 px.
const OVERHANG_PX: f32 = 1.0;

/// Frames a layout needs to settle after the scale is read from the settings file, and after a panel is shown.
/// How many culprits a failing cell names.
const CULPRITS: usize = 4;

const SETTLE_FRAMES: usize = 18;
const SHOW_FRAMES: usize = 8;

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

/// How many pieces the Place panel's kit holds.
const PIECES: usize = 36;

/// **The most gooey table-cell texts one panel may draw** before the exemption for `bevy_gooey#109` is
/// treated as having grown past what it was written for.
const MOST_GOOEY_CELLS: usize = 40;

fn settle(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

/// A name no row can hold in one line at any scale: `n` words of length 9, so a break has somewhere to go.
fn long(stem: &str, i: usize) -> String {
    format!("{stem}_{i:02}_with_a_name_that_keeps_on_going_and_going")
}

/// A tag token that is long on purpose.
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
    let extra: Vec<String> = (0..12).map(|i| format!("{namespace}/{}", long("extra", i))).collect();
    let extra_refs: Vec<&str> = extra.iter().map(String::as_str).collect();
    fixture = fixture.kit("an_extra_kit_whose_directory_is_long", "beta", &extra_refs);
    // Import candidates: meshes on disk that are in no library, one with a name no column holds.
    let candidates: Vec<String> = (0..30).map(|i| long("candidate", i)).collect();
    let mut candidate_refs: Vec<&str> = candidates.iter().map(String::as_str).collect();
    candidate_refs.push("a_single_mesh_name_with_no_break_in_it_at_all_that_is_longer_than_any_column_holds");
    fixture = fixture.pack("a_pack_folder_with_a_long_name", &candidate_refs);
    // Placements, so the Inspector and the status block have something to say.
    for i in 0..14 {
        fixture = fixture.place(&long("piece", i), (i as f32 % 5.0, (i / 5) as f32));
    }
    // Compositions (the Compose panel's groups) and bounded ones (the Tiles panel's tiles).
    for i in 0..12 {
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
        for i in 0..8 {
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
                d.kind = (0..8).map(|i| token("kind", i)).collect();
                d.effects = (0..8).map(|i| token("effects", i)).collect();
                d.look = (0..8).map(|i| token("look", i)).collect();
                d.note = Some("a note long enough to take several lines in any column a panel can give it".to_owned());
            }
        });
    }

    // **Rigs with long names and many clips**, in the manifest's own spelling.
    let mut rigs = String::from("version: 3\nrigs:\n");
    for r in 0..6 {
        rigs.push_str(&format!("  {}:\n    mesh: characters/{r}.glb\n    scale: 1.0\n    slots:\n", long("rig", r)));
        for c in 0..10 {
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
        .spawn((Window { resolution: WindowResolution::new(1280, 802), ..default() }, PrimaryWindow))
        .id();
    let mut window = app.world_mut().get_mut::<Window>(window).expect("the window just spawned");
    window.resolution.set_scale_factor_override(Some(DISPLAY_FACTOR));
    harness::resize_surface(&mut app, WINDOW_PHYSICAL.0, WINDOW_PHYSICAL.1).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app, SETTLE_FRAMES);
    let want = bevy_dark_factory::surface::ui_scale_for_a_person(1280.0, DISPLAY_FACTOR, interface_scale);
    let got = app.world().resource::<UiScale>().0;
    assert!(
        (got - want).abs() < 1e-3,
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
    // **A `PositionedGlyph::position` is the glyph's centre**, not its corner (`bevy_text::pipeline`: `size / 2. +
    // glyph_pos + atlas_info.offset`): the box a glyph paints is `from_center_size(position, atlas rect size)`.
    let ink = |glyph: &bevy::text::PositionedGlyph| Rect::from_center_size(start + glyph.position, glyph.atlas_info.rect.size());
    let mut painted = ink(first);
    for glyph in &info.glyphs {
        painted = painted.union(ink(glyph));
    }
    Some(rect.union(painted))
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
    /// How far a node was **laid out** outside the box, whatever clipped it afterwards. Reported, not asserted:
    /// the sibling test's point that a pane which hides what does not fit has still laid it out too wide.
    laid_out: f32,
    /// How many texts were drawn under the region: how *full* it was.
    texts: usize,
    /// gooey's one-line table cells, exempt and counted.
    cells: usize,
}

impl Measured {
    /// Keep the worse of two readings of one named region, and the fuller of its counts.
    fn absorb(&mut self, other: Measured) {
        let (texts, cells, laid_out) = (self.texts.max(other.texts), self.cells.max(other.cells), self.laid_out.max(other.laid_out));
        self.overhang = self.overhang.max(other.overhang);
        self.culprits.extend(other.culprits);
        self.culprits.sort_by(|a, b| b.0.total_cmp(&a.0));
        self.culprits.dedup_by(|a, b| a.1 == b.1);
        self.culprits.truncate(CULPRITS);
        (self.texts, self.cells, self.laid_out) = (texts, cells, laid_out);
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
            format!("[{:.0}..{:.0}]", a.x.min(b.x), a.x.max(b.x))
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

fn measure(world: &World, region: Entity) -> Option<Measured> {
    let region_box = border_box(world, region)?;
    let inset = world.get::<ComputedNode>(region).map(ComputedNode::content_inset).unwrap_or_default();
    let right = region_box.max.x - inset.max_inset.x;
    let left = region_box.min.x + inset.min_inset.x;
    let mut out = Measured::default();
    for entity in under(world, region) {
        if matches!(world.get::<bevy_gooey::Role>(entity), Some(bevy_gooey::Role::TableCellText)) {
            out.cells += 1;
            continue;
        }
        let Some(rect) = drawn(world, entity) else { continue };
        if world.get::<Text>(entity).is_some() {
            out.texts += 1;
        }
        out.laid_out = out.laid_out.max((rect.max.x - right).max(left - rect.min.x));
        let Some(rect) = seen(world, entity, rect) else { continue };
        let past = (rect.max.x - right).max(left - rect.min.x);
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
            for (name, measured) in seen_here {
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
                table.get(&(case.clone(), p.clone())).map(|m| format!("{p}={:.1} (laid out {:.1}, {} texts)", m.overhang, m.laid_out, m.texts))
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
            && rigs.texts < 6
        {
            failures.push(format!("[{case}] the RIGS panel drew {} texts for six rigs", rigs.texts));
        }
        if let Some(place) = table.get(&(case.clone(), "PLACE".to_owned()))
            && place.texts < PIECES
        {
            failures.push(format!("[{case}] the PLACE panel drew {} texts for a kit of {PIECES} pieces", place.texts));
        }
    }
    assert!(
        failures.is_empty(),
        "a panel draws something outside the content box it is drawn in (#494):\n  {}",
        failures.join("\n  ")
    );
}
