//! **The rows #494 changed stand where they stood** (#494, the reviewer's regression check).
//!
//! `a_panel_census_at_five_scales_over_populated_content` measures overhang and nothing else, so it cannot see a row
//! that stopped overhanging by *moving*. The spawners the fix touched are probed here with an **ordinary short name**,
//! at the capture's window (physical px, 2560 x 1604 at a 2.0 display factor, interface scale 1.0, text 100%, so
//! `UiScale` 2.4). Every value was read by this same probe on `origin/main` at `4e4a399f`, before any fix, and is pinned
//! in [`ON_MAIN`]:
//!
//! | spawner | what is measured | on `main` | this branch |
//! |---|---|---|---|
//! | PLACE category heading (`PROP`) | its left edge from its row's content edge | 50.0 | held within 1 px |
//! | COMPOSE group row | the name's left edge from its row's content edge | -1.0 | held within 1 px |
//! | COMPOSE armed mark | the `*`'s left edge minus the last name glyph's right edge | **128.1** (the name's box was wider than the row) | at most [`MARK_MAX_PX`] |
//! | COMPOSE member row | its left edge from its row's content edge | -1.0 | held within 1 px |
//! | RIGS row | the name's left edge from its row's content edge | -1.0 | held within 1 px |
//! | TILES openable row's `1 member(s)` | its right edge from the row's content edge | 0.0 | held within 1 px |
//! | TILES reason `no tile — see COMPOSE` | its right edge from the row's content edge | **-81.0** (the bug: 81 px past) | its ink ends within [`INK_PX`] of where an openable row's count's ink does |
//!
//! The TILES reason and the armed mark have no baseline to hold, because on `main` they were wrong for this row: the
//! 21-character sentence beside even a short id does not fit the panel's 319 px, so it stood 81 px past the edge, and the
//! armed mark followed a name box wider than its row. The branch is held to the intent. (The Tiles row's own id was probed
//! too, and its text is not drawn as a plain `short_group` in that list, so it is not pinned.)
//! Each value is printed as a `PROBE name=value` line.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy::window::{PrimaryWindow, Window, WindowResolution};

use bevy_dark_factory::harness;
use bevy_dark_factory::settings::UserSettingsFile;
use bevy_dark_factory::tiles::Mode;

use crate::fixtures;

/// The capture's window, physical px, and its display factor.
const WINDOW_PHYSICAL: (u32, u32) = (2560, 1604);
const DISPLAY_FACTOR: f32 = 2.0;

/// Frames a layout needs to settle after the window is fitted, and after a panel is shown: the docks glide into place
/// over many frames, and a probe taken mid-glide read a different panel width on each run.
const SETTLE_FRAMES: usize = 90;
const SHOW_FRAMES: usize = 60;

/// Frames to let the armed row draw before it is read. Measured: after 60 the mark is no longer drawn on `main` (the
/// armed group is cleared again), and after 8 it is, so the row is read early.
const ARM_FRAMES: usize = 8;

/// How far a measured offset may stand from the one recorded on `main`, physical px: one device pixel of layout rounding.
const SAME_PX: f32 = 1.0;

/// How far two glyph edges may stand from one another and still be the same place: a glyph's own side bearing differs by character.
const INK_PX: f32 = 5.0;

/// **What this probe read on `origin/main` at `4e4a399f`**, as `(name, physical px)`.
const ON_MAIN: [(&str, f32); 5] = [
    ("place_heading_left_from_row", 50.0),
    ("tiles_count_right_gap_to_row", 0.0),
    ("compose_name_left_from_row", -1.0),
    ("compose_member_left_from_row", -1.0),
    ("rigs_name_left_from_row", -1.0),
];

/// **What `main` read for the three probes that have no number to hold** (printed by the same probe on `4e4a399f`):
/// the TILES reason ended **81.0 px past** its row's right edge (its ink 79.0 past; an openable row's count: box 0.0, ink
/// 4.0 inside), and the armed `*` stood **128.1 px** after the last glyph of the name, because the name was a bare `Text`
/// laid out wider than its row and the mark followed its box. They are bugs, so the branch is held to the intent instead:
/// the reason ends where the count does, and the mark is [`MARK_MAX_PX`] from the name's last glyph at most.
///
/// How far the armed `*` may stand from the last glyph of the name before it: two spaces and the star's own bearing
/// (measured on this branch: the span puts it 17.8 px after, at the capture's window and text size).
const MARK_MAX_PX: f32 = 20.0;




/// One editor over a project whose every name is short, opened on `mode`.
fn editor(tag: &str, mode: Mode) -> App {
    let root = fixtures::Fixture::new(&format!("short_rows_{tag}_{}", std::process::id()))
        .descriptor("stool", "props")
        .composition("short_group", &[("member_a", "stool", (0.0, 0.0))])
        .bounded_composition("short_tile", (1.0, 2.4, 1.0), &[("member_a", "stool", (0.0, 0.0))])
        .build("m");
    std::fs::write(
        root.join("assets/blueprint/rigs.yaml"),
        "version: 3\nrigs:\n  hound:\n    mesh: characters/hound.glb\n    scale: 1.0\n    slots:\n    - clip: 0\n      playback: !Free\n        speed: 1.0\n      note: idle\n",
    )
    .unwrap_or_else(|e| panic!("rigs.yaml: {e}"));
    let user = root.join(format!("user_settings_{tag}/settings.yaml"));
    std::fs::create_dir_all(user.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(&user, "version: 1\ntext_scale: 1.0\ninterface_scale: 1.0\n").expect("a settings file");
    let mut app = harness::build_headless_at(&root, "m", None, mode)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    app.insert_resource(UserSettingsFile(user));
    let window = app
        .world_mut()
        .spawn((Window { resolution: WindowResolution::new(1280, 802), ..default() }, PrimaryWindow))
        .id();
    let mut window = app.world_mut().get_mut::<Window>(window).expect("the window just spawned");
    window.resolution.set_scale_factor_override(Some(DISPLAY_FACTOR));
    harness::resize_surface(&mut app, WINDOW_PHYSICAL.0, WINDOW_PHYSICAL.1).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE_FRAMES { app.update(); }
    app
}

fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// The one text whose words satisfy `want`, with its box and the content box of the node it stands in.
fn find_once(app: &mut App, what: &str, want: &impl Fn(&str) -> bool) -> Option<(Rect, Rect)> {
    let world = app.world_mut();
    let mut found = Vec::new();
    for (entity, text) in world.query::<(Entity, &Text)>().iter(world) {
        // A hidden panel's rows are laid out to nothing; only what is drawn is probed.
        let drawn = world.get::<ComputedNode>(entity).is_some_and(|c| c.size().x > 0.0 && c.size().y > 0.0);
        if drawn && want(&text.0) {
            found.push(entity);
        }
    }
    found.sort_by_key(|e| e.index());
    let Some(entity) = found.first().copied() else {
        let _ = what;
        return None;
    };
    let parent = world.get::<ChildOf>(entity).map(ChildOf::parent)?;
    let text_box = border_box(world, entity)?;
    let parent_box = border_box(world, parent)?;
    let inset = world.get::<ComputedNode>(parent).map(ComputedNode::content_inset).unwrap_or_default();
    let content = Rect::new(
        parent_box.min.x + inset.min_inset.x,
        parent_box.min.y,
        parent_box.max.x - inset.max_inset.x,
        parent_box.max.y,
    );
    Some((text_box, content))
}

/// The window-space box of every glyph a text paints, in order (`PositionedGlyph::position` is the glyph's centre).
fn glyph_inks(app: &mut App, want: impl Fn(&str) -> bool) -> Option<Vec<Rect>> {
    find(app, "a text with glyphs", &want)?;
    let world = app.world_mut();
    let mut found: Vec<Entity> = world
        .query::<(Entity, &Text)>()
        .iter(world)
        .filter(|(e, t)| want(&t.0) && world.get::<ComputedNode>(*e).is_some_and(|c| c.size().x > 0.0))
        .map(|(e, _)| e)
        .collect();
    found.sort_by_key(|e| e.index());
    let entity = *found.first()?;
    let rect = border_box(world, entity)?;
    let start = rect.min + world.get::<ComputedNode>(entity)?.content_inset().min_inset;
    let info = world.get::<TextLayoutInfo>(entity)?;
    Some(info.glyphs.iter().map(|g| Rect::from_center_size(start + g.position, g.atlas_info.rect.size())).collect())
}

/// **How far the armed `*` stands from the last glyph of the name before it**, whichever way the row builds the mark:
/// as a text of its own after the name (`main`), or as the last glyph of the name's own text (a `TextSpan`).
fn mark_gap(app: &mut App) -> Option<f32> {
    let name = glyph_inks(app, |t| t.starts_with("short_group  —"))?;
    match glyph_inks(app, |t| t == "  *") {
        Some(star) => Some(star.last()?.min.x - name.last()?.max.x),
        None => {
            let [.., before, star] = name.as_slice() else { return None };
            Some(star.min.x - before.max.x)
        }
    }
}

/// How far the last glyph of a text stands from the row's content edge on the right.
fn ink_right_gap(app: &mut App, want: impl Fn(&str) -> bool + Copy) -> Option<f32> {
    let (_, row) = find(app, "the row", want)?;
    Some(row.max.x - glyph_inks(app, want)?.iter().map(|g| g.max.x).fold(f32::MIN, f32::max))
}

/// **The text, waiting for it**: a panel's rows arrive on whatever frame the project's asynchronous loads finish, and a
/// probe that gave up after a fixed few frames read "no such text" on one run and a number on the next.
fn find(app: &mut App, what: &str, want: impl Fn(&str) -> bool) -> Option<(Rect, Rect)> {
    for _ in 0..PATIENCE {
        if let Some(found) = find_once(app, what, &want) {
            return Some(found);
        }
        for _ in 0..PATIENCE_STEP { app.update(); }
    }
    println!("PROBE-MISSING {what}: no such text is drawn after {} frames", PATIENCE * PATIENCE_STEP);
    None
}

/// How many times a probe looks for its text, and how many frames it lets pass between looks.
const PATIENCE: usize = 30;
const PATIENCE_STEP: usize = 4;

fn show(app: &mut App, mode: Mode) {
    *app.world_mut().resource_mut::<Mode>() = mode;
    for _ in 0..SHOW_FRAMES { app.update(); }
}

/// One probe: where a row's text stands, by `read` over the text's box and the content box of the node it stands in.
fn probe(
    out: &mut Vec<(&'static str, f32)>,
    app: &mut App,
    name: &'static str,
    want: impl Fn(&str) -> bool,
    read: impl Fn(Rect, Rect) -> f32,
) -> Option<Rect> {
    let (text, row) = find(app, name, want)?;
    out.push((name, read(text, row)));
    Some(text)
}

/// **Every probe**, as `(name, physical px)`.
fn probes() -> Vec<(&'static str, f32)> {
    let mut out = Vec::new();
    let from_left = |text: Rect, row: Rect| text.min.x - row.min.x;
    let to_right = |text: Rect, row: Rect| row.max.x - text.max.x;

    let mut app = editor("map", Mode::Map);
    probe(&mut out, &mut app, "place_heading_left_from_row", |t| t == "PROP", from_left);

    show(&mut app, Mode::Tiles);
    let reason = |t: &str| t == "no tile — see COMPOSE";
    let count = |t: &str| t.ends_with("member(s)") && !t.contains("short_");
    probe(&mut out, &mut app, "tiles_reason_right_gap_to_row", reason, to_right);
    probe(&mut out, &mut app, "tiles_count_right_gap_to_row", count, to_right);
    if let Some(gap) = ink_right_gap(&mut app, reason) {
        out.push(("tiles_reason_ink_right_gap_to_row", gap));
    }
    if let Some(gap) = ink_right_gap(&mut app, count) {
        out.push(("tiles_count_ink_right_gap_to_row", gap));
    }

    // Not `show`: the armed group is cleared again some tens of frames after it is set (measured, on `main` too), so the
    // Compose panel is given as few frames as the armed row needs.
    *app.world_mut().resource_mut::<Mode>() = Mode::Compose;
    for _ in 0..ARM_FRAMES { app.update(); }
    app.world_mut().insert_resource(bevy_dark_factory::compose::Armed(Some("short_group".to_owned())));
    for _ in 0..ARM_FRAMES { app.update(); }
    let name = probe(&mut out, &mut app, "compose_name_left_from_row", |t| t.starts_with("short_group  —"), from_left);
    let _ = name;
    if let Some(gap) = mark_gap(&mut app) {
        out.push(("compose_mark_gap_after_name_glyph", gap));
    }
    probe(&mut out, &mut app, "compose_member_left_from_row", |t| t.starts_with("member_a"), from_left);

    let mut app = editor("anim", Mode::Anim);
    let world = app.world_mut();
    let benches: Vec<Entity> =
        world.query_filtered::<Entity, With<bevy_dark_factory::anim_tab::RigBench>>().iter(world).collect();
    for bench in benches {
        if let Some(mut node) = world.get_mut::<Node>(bench) {
            node.display = Display::Flex;
        }
    }
    for _ in 0..SHOW_FRAMES { app.update(); }
    probe(&mut out, &mut app, "rigs_name_left_from_row", |t| t == "hound", from_left);
    out
}

/// How many probes there are, so a text that was not found is a failure and not a quiet absence.
const PROBES: usize = 9;

#[test]
fn a_short_row_stands_where_it_stood() {
    let measured = probes();
    for (name, value) in &measured {
        println!("PROBE {name}={value:.1}");
    }
    let mut failures = Vec::new();
    if measured.len() != PROBES {
        failures.push(format!("{} of {PROBES} probes found their text", measured.len()));
    }
    for (name, was) in ON_MAIN {
        match measured.iter().find(|(n, _)| *n == name) {
            Some((_, now)) if (now - was).abs() <= SAME_PX => {}
            Some((_, now)) => failures.push(format!("`{name}` is {now:.1} px; on `main` it was {was:.1}")),
            None => failures.push(format!("`{name}` is recorded on `main` and not probed")),
        }
    }
    // **The two right-hand cells of the Tiles page line up**: a reason that refuses and a count that opens end at one edge.
    let get = |wanted: &str| measured.iter().find(|(n, _)| *n == wanted).map(|(_, v)| *v).unwrap_or(f32::NAN);
    if get("compose_mark_gap_after_name_glyph").is_nan() || get("compose_mark_gap_after_name_glyph") > MARK_MAX_PX {
        failures.push(format!(
            "the armed mark stands {:.1} px after the name's last glyph, more than the {MARK_MAX_PX:.0} px of its two spaces",
            get("compose_mark_gap_after_name_glyph")
        ));
    }
    if (get("tiles_reason_ink_right_gap_to_row") - get("tiles_count_ink_right_gap_to_row")).abs() > INK_PX {
        failures.push(format!(
            "the TILES reason's ink ends {:.1} px from its row's edge, an openable row's count's {:.1}",
            get("tiles_reason_ink_right_gap_to_row"),
            get("tiles_count_ink_right_gap_to_row")
        ));
    }
    assert!(failures.is_empty(), "a row #494 changed moved for an ordinary short name:\n  {}", failures.join("\n  "));
}
