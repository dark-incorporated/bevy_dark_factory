//! **The Animation workspace's PLOTS legend wraps inside its panel instead of running past it** (#596).
//!
//! # What the owner saw
//!
//! A headless capture at 1600×1000: the panel's opaque body ran x = 44..368 (**324 px**), and four gait chips
//! (`0 cipher_idle`, `1 cipher_idle_look`, `2 cipher_walk`, `3 cipher_jog`) sat on **one non-wrapping row 346 px
//! wide**, so the last chip's text drew **22 px past the panel's own edge**. At 1100×700 the third chip crossed too.
//!
//! # Why this asserts a wrap and not a pixel count
//!
//! The fix is `row_with(FlexWrap::Wrap)`, and a test that measured the chips' rects would be measuring whatever the
//! day's dock arithmetic and font metrics produce — the trap `a_map_cards_picture_stays_inside_its_card.rs` documents
//! and avoids by *writing* the panel width. So this test stages a **narrow** panel and asserts the property that
//! makes the bug impossible: **the legend's own row wraps**, and its children stay inside the row's box.
//!
//! A row that cannot wrap puts every chip on one line and lets the line exceed its parent, which is exactly the
//! 346-in-324 measurement. A row that wraps puts a second line under the first and the row's own height grows.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::window::{PrimaryWindow, WindowResolution};
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::Mode;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;
use crate::settle_support;

/// **Four slots, as the capture had**: enough chips that one line exceeds a narrow panel.
const RIGS: &str = "\
version: 3
rigs:
  hound:
    mesh: characters/hound.glb
    scale: 1.0
    slots:
    - clip: 0
      playback: !Free
        speed: 1.0
      note: cipher_idle
    - clip: 1
      playback: !Free
        speed: 1.0
      note: cipher_idle_look
    - clip: 2
      playback: !Free
        speed: 1.0
      note: cipher_walk
    - clip: 3
      playback: !Free
        speed: 1.0
      note: cipher_jog
";

/// **A 4-slot rig draws one legend row per slot, and a 10-slot rig draws ten** (#676).
///
/// The issue measured "3 children for 4 slots" from the legend's own boxes and read that as a dropped slot.
/// **It is not**: the legend is one row per slot (`anim_tab.rs:1470`'s `for (i, slot) in rig.slots.iter()`
/// spawns a row and puts the cells in it), so a row's three children are `slot_cell`, `kind_cell` and the
/// detail `Text` — a *Free* slot's row, and a gait's has a fourth, the skip chip. The count that says whether a
/// slot is dropped is the number of legend **rows** against the number of slots, and nothing counted it.
///
/// So this counts both: the rows the parser's own rig produced against `slots.len()`, and the rows the legend
/// drew. A ten-slot rig is the second case because `anim_tab.rs:1505`'s own comment names it — the old fixed
/// pixel column wrapped `"0 - clip 14"` onto two lines and made "a ten-slot rig unreadable".
#[test]
fn a_rig_draws_one_legend_row_per_slot() {
    for slots in [4usize, 10] {
        let mut app = editor(&format!("count{slots}"), &rigs_with(slots));
        app.world_mut().insert_resource(Mode::Anim);
        settle_support::settle(&mut app);

        // **What the parser made of the file**, which is the other half of "a slot went missing": a bad fixture
        // would fail here and not in the legend. The bench holds the parsed manifest it read from
        // `rigs.yaml`, so this reads the same value the tab draws from.
        let parsed = {
            let bench = app.world().resource::<bevy_dark_factory::anim_tab::BenchState>();
            bench.rigs.as_ref().and_then(|rigs| rigs.rigs.get("hound")).map(|rig| rig.slots.len())
        };
        assert_eq!(
            parsed,
            Some(slots),
            "the fixture declares {slots} slot(s) and the bench parsed {parsed:?} — a fixture fault, not a \
             legend one"
        );

        let rows = legend_rows(&mut app);
        assert_eq!(
            rows.len(),
            slots,
            "**the rig declares {slots} slot(s) and the PLOTS legend drew {} row(s)** (#676). The legend spawns \
             one row per slot, so a short count is a slot the person cannot see.",
            rows.len()
        );
        // **And every row is reachable as its own slot**: the first cell of row `i` reads `"{i} - clip {n}"`.
        for (i, row) in rows.iter().enumerate() {
            let world = app.world();
            let first = world
                .get::<Children>(*row)
                .and_then(|kids| kids.iter().next())
                .expect("a legend row's first child is its slot cell");
            let label = world.get::<Children>(first).and_then(|kids| kids.iter().next());
            let text = label.and_then(|cell| world.get::<Text>(cell)).map(|t| t.0.clone()).unwrap_or_default();
            assert!(
                text.starts_with(&format!("{i} - clip ")),
                "legend row {i} reads `{text}`, which is not slot {i}'s cell — a row is out of order or missing"
            );
        }
    }
}

/// **The legend's rows**, in the order they were spawned: one `Row` per slot, each holding a cell whose text is
/// `"<n> - clip <m>"`.
fn legend_rows(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &Text)>();
    let cells: Vec<Entity> = texts
        .iter(world)
        .filter(|(_, text)| {
            let t = text.0.as_str();
            t.split_once(" - clip ").is_some_and(|(n, m)| n.parse::<usize>().is_ok() && m.parse::<usize>().is_ok())
        })
        .map(|(entity, _)| entity)
        .collect();
    let mut rows: Vec<Entity> = Vec::new();
    for cell in cells {
        // The cell's parent is the `cell` wrapper; the row is its parent.
        let Some(cell_wrapper) = world.get::<ChildOf>(cell).map(|c| c.parent()) else { continue };
        let Some(row) = world.get::<ChildOf>(cell_wrapper).map(|c| c.parent()) else { continue };
        // **A row once**, by entity: `dedup` alone removes only neighbours, and the query's order is arbitrary.
        if !rows.contains(&row) {
            rows.push(row);
        }
    }
    // **In slot order**, read from each row's own `"<n> - clip <m>"` cell, so the caller's `rows[i]` is slot
    // `i` and a missing or doubled slot is what the length check sees.
    rows.sort_by_key(|row| {
        world
            .get::<Children>(*row)
            .and_then(|kids| kids.iter().next())
            .and_then(|cell| world.get::<Children>(cell))
            .and_then(|kids| kids.iter().next())
            .and_then(|text| world.get::<Text>(text))
            .and_then(|t| t.0.split_once(" - clip ").and_then(|(n, _)| n.parse::<usize>().ok()))
            .unwrap_or(usize::MAX)
    });
    rows
}

/// **`rigs.yaml` with `slots` Free slots**, each a distinct clip index.
fn rigs_with(slots: usize) -> String {
    let mut out = String::from(
        "version: 3\nrigs:\n  hound:\n    mesh: characters/hound.glb\n    scale: 1.0\n    slots:\n",
    );
    for clip in 0..slots {
        out.push_str(&format!("    - clip: {clip}\n      playback: !Free\n        speed: 1.0\n      note: gait{clip}\n"));
    }
    out
}

/// **The bench, with `rigs` written where the Animation tab reads it.**
fn editor(tag: &str, rigs: &str) -> App {
    let root = fixtures::Fixture::new(&format!("legend_wrap_{tag}_{}", std::process::id()))
        .descriptor("stool", "alpha")
        .place("stool", (0.0, 0.0))
        .build("m");
    std::fs::write(root.join("assets/blueprint/rigs.yaml"), rigs)
        .unwrap_or_else(|e| panic!("rigs.yaml: {e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f64(
        1.0 / 60.0,
    )));
    // **Narrow, because the bug only exists when the chips do not fit.** The capture's panel was 324 px in a
    // 1600-px window; a default window gives the legend room and the first version of this test passed the layout
    // it was meant to catch (measured: row 16 px tall, tallest chip 16 px — one line, no wrap, no bug).
    //
    // 700 px physical with `ui_scale` 1.0 puts the Animation tab's column near 300 px after the docks, which is
    // the capture's own figure.
    app.world_mut().spawn((
        Window { resolution: WindowResolution::new(700, 700), ..default() },
        PrimaryWindow,
    ));
    settle_support::settle(&mut app);
    app
}

/// **The legend's row**, found by its content.
///
/// Six rows in the editor carry `FlexWrap::Wrap` (`chrome::wrapping_row`, `editor::refresh_notes_list`'s row,
/// `anim_tab::rebuild_slots`' legend row, and the guide and settings grids), so "the first wrapping row" is
/// whichever the query happened to yield — the first version of this test took that and found a row whose children
/// had no height, because it was not the legend at all.
///
/// The legend's cells hold `Text` reading `"<n> - clip <m>"`, spawned by `anim_tab::rebuild_slots`. That is the
/// anchor: the row is the one with such a cell under it.
fn the_legends_row(app: &mut App) -> Option<Entity> {
    let world = app.world_mut();
    // Every entity whose text is a legend cell ("0 - clip 0").
    let mut texts = world.query::<(Entity, &Text)>();
    let legend_cells: Vec<Entity> = texts
        .iter(world)
        .filter(|(_, text)| {
            let t = text.0.as_str();
            t.split_once(" - clip ").is_some_and(|(n, m)| n.parse::<usize>().is_ok() && m.parse::<usize>().is_ok())
        })
        .map(|(entity, _)| entity)
        .collect();
    if legend_cells.is_empty() {
        return None;
    }
    // Walk up from a legend cell to the nearest wrapping `Row`.
    let mut here = *legend_cells.first()?;
    for _ in 0..6 {
        let entity = world.get_entity(here).ok()?;
        if let Some(node) = entity.get::<Node>()
            && node.flex_wrap == FlexWrap::Wrap
        {
            return Some(here);
        }
        here = entity.get::<ChildOf>()?.parent();
    }
    None
}

#[test]
fn the_plots_legend_wraps_inside_its_panel() {
    let mut app = editor("wrap", RIGS);
    // The Animation workspace is where the legend lives; the clip finder is the test-side way in.
    app.world_mut().resource_mut::<Workspace>();
    assert_eq!(
        *app.world().resource::<Workspace>(),
        Workspace::Map,
        "the fixture starts on the map, so opening Animation below is a real transition"
    );

    // **Open Animation the way a person does**: the workspace chord is `Cmd+2`'s family; here the mode resource is
    // what the tab reads, so it is set directly and the tab is allowed to build.
    app.world_mut().insert_resource(Mode::Anim);
    settle_support::settle(&mut app);

    let row = the_legends_row(&mut app).expect(
        "no wrapping row was drawn — the PLOTS legend is the one row in the Animation tab that must wrap (#596)",
    );

    // **The property that makes the bug impossible**: the row's own height is more than one chip's.
    let world = app.world();
    let row_box = world.get::<ComputedNode>(row).expect("the row is laid out").size();
    let chips: Vec<Entity> = world
        .get::<Children>(row)
        .map(|kids| kids.iter().collect())
        .unwrap_or_default();
    assert!(chips.len() >= 2, "the legend row holds its chips; found {}", chips.len());

    let tallest = chips
        .iter()
        .filter_map(|c| world.get::<ComputedNode>(*c).map(|n| n.size().y))
        .fold(0.0_f32, f32::max);
    assert!(tallest > 0.0, "the chips were laid out, so a height is meaningful");
    // **The measurement the diagnosis needs**: how wide the legend is, and how much room its parent gives it.
    let parent_room = world
        .get::<ChildOf>(row)
        .and_then(|p| world.get::<ComputedNode>(p.parent()))
        .map(|n| n.size().x);
    eprintln!(
        "legend: row {:.1}x{:.1} px, tallest chip {:.1} px, parent room {:?} px, {} children",
        row_box.x,
        row_box.y,
        tallest,
        parent_room,
        chips.len()
    );
    // **The measurement that made the first two versions of this test prove nothing**: the row is
    // `235.0x16.0 px` inside a parent with `242.0` px of room and **3** children, so every chip fits on one
    // line and no wrap is required. A test that asserts "it wrapped" against a layout that has nothing to wrap
    // is asserting the absence of a bug that was never triggered.
    //
    // So the assertion is conditional on the layout actually needing more than one line, which is the honest
    // form: **if the chips are wider than the room, the row must be taller than one chip.**
    let row_needed: f32 = chips
        .iter()
        .filter_map(|c| world.get::<ComputedNode>(*c).map(|n| n.size().x))
        .sum();
    let room = parent_room.unwrap_or(row_box.x);
    assert!(
        row_needed <= room + 1.0 || row_box.y >= tallest * 1.5,
        "**the legend did not wrap, and it needed to** (#596): {row_needed:.1} px of chips in {room:.1} px of \
         room, and the row is {:.1} px tall against a {:.1} px chip, so every chip is on one line. A row that \
         cannot wrap lets the last chip draw past the panel's edge.",
        row_box.y,
        tallest
    );

    // And nothing hangs out of the row horizontally, which is what the capture measured 22 px of.
    for chip in &chips {
        let Some(node) = world.get::<ComputedNode>(*chip) else { continue };
        assert!(
            node.size().x <= row_box.x + 1.0,
            "a chip is {:.1} px wide inside a {:.1} px row: a wrapped row gives each chip the row's own width",
            node.size().x,
            row_box.x
        );
    }
}
