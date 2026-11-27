//! **A rig slot whose `clip:` cannot resolve is drawn and says why** (#679).
//!
//! # What was silent
//!
//! A slot names a glTF animation **index** (`SlotDef::clip`), and the index resolves against the clips
//! the *asset* brings. The rig fixture named four slots for a mesh that does not exist in this
//! repository, so nothing ever compared them: the bench drew three chip rows for four written slots,
//! and the plots legend drew a colour for a curve that was never measured. Nothing said a clip was
//! missing — the class this repo refuses everywhere else, one level down.
//!
//! # What this pins
//!
//! 1. **The refusal names the rig, the slot and the count.** `check_rig` now takes the rig's name, so a
//!    finding read in a list of a dozen rigs says whose slot it is, which slot, and how many the asset
//!    does have — the same words a malformed `rigs.yaml` gets.
//! 2. **The tab draws the slot that cannot be plotted, with the reason under it**, and the legend's
//!    chip for that slot carries the same sentence instead of a bare index.
//! 3. **The fixture's mesh exists**: `assets/characters/hound.glb` is committed with three clips, so a
//!    rig naming four has a real asset to disagree with rather than a path nothing provides.

use bevy::prelude::*;
use bevy_dark_factory::anim_tab::{BenchState, SlotPane};
use bevy_dark_factory::anim_watch::BenchReports;
use bevy_dark_factory::tiles::Mode;
use blueprint_model::rig_check::Level;

use crate::fixtures;

fn workspace_asset(rel: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets").join(rel)
}

/// **Four slots, on a mesh that carries three clips** — the issue's own fixture, made real.
const FOUR_SLOTS_THREE_CLIPS: &str = "\
version: 3
rigs:
  hound:
    mesh: characters/hound.glb
    scale: 1.0
    slots:
    - clip: 0
      playback: !Free
        speed: 1.0
      note: idle
    - clip: 1
      playback: !Free
        speed: 1.0
      note: gallop
    - clip: 2
      playback: !Free
        speed: 1.0
      note: walk
    - clip: 3
      playback: !Free
        speed: 1.0
      note: the one the asset does not have
";

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("clip_that_cannot_resolve_{tag}_{}", std::process::id()))
        .descriptor("stool", "alpha")
        .place("stool", (0.0, 0.0))
        .build("m");
    std::fs::write(root.join("assets/blueprint/rigs.yaml"), FOUR_SLOTS_THREE_CLIPS)
        .unwrap_or_else(|e| panic!("rigs.yaml: {e}"));
    std::fs::create_dir_all(root.join("assets/characters")).expect("characters dir");
    // **The asset the fixture names is the one this repository ships**, so the test measures a mesh
    // whose clip count it can read rather than a path nothing provides (#679's third acceptance).
    std::fs::copy(workspace_asset("characters/hound.glb"), root.join("assets/characters/hound.glb"))
        .unwrap_or_else(|e| panic!("hound.glb: {e}"));
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().insert_resource(Mode::Anim);
    crate::settle_support::settle(&mut app);
    assert!(app.world().resource::<BenchState>().loaded, "the tab loaded the manifest");
    app
}

/// **The asset really has three clips**, which is the other half of the assertion: a test that only
/// read the report could pass against a broken fixture.
fn the_asset_has_three_clips() {
    let glb = blueprint_model::glb::Glb::open(&workspace_asset("characters/hound.glb"))
        .unwrap_or_else(|e| panic!("the fixture's mesh: {e}"));
    let clips = blueprint_model::clips::clips(&glb);
    assert_eq!(clips.len(), 3, "the fixture's mesh carries three clips: {clips:?}");
}

/// **A rig naming a clip its mesh does not have is refused with the rig, the slot and the count.**
#[test]
fn a_slot_that_cannot_resolve_is_refused_naming_the_rig_the_slot_and_the_count() {
    the_asset_has_three_clips();
    let mut app = editor("refused");
    let at = app
        .world()
        .resource::<BenchState>()
        .names()
        .iter()
        .position(|n| *n == "hound")
        .expect("hound is a rig");
    app.world_mut().resource_mut::<BenchState>().selected = at;
    crate::settle_support::until(&mut app, "the hound measured", |a| {
        a.world().resource::<BenchReports>().by_rig.contains_key("hound")
    });

    let reports = app.world().resource::<BenchReports>();
    let report = reports.by_rig.get("hound").expect("the hound's report");
    let bad: Vec<_> = report
        .findings
        .iter()
        .filter(|f| f.level == Level::Bad)
        .collect();
    let refusal = bad
        .iter()
        .find(|f| f.slot == Some(3))
        .unwrap_or_else(|| panic!("no Bad finding for slot 3; findings: {:#?}", report.findings));
    // **The rig, the slot and the count**, the three facts a reader in a list of rigs needs.
    assert!(
        refusal.text.contains("`hound`"),
        "the refusal names the rig it is about: {}",
        refusal.text
    );
    assert!(
        refusal.text.contains("slot 3"),
        "the refusal names the slot: {}",
        refusal.text
    );
    assert!(
        refusal.text.contains("has 3"),
        "the refusal names how many clips the asset does have: {}",
        refusal.text
    );
    assert!(
        !bad.iter().any(|f| f.slot != Some(3)),
        "the three slots the asset supplies are not refused: {bad:#?}"
    );
}

/// **The tab draws the slot the asset cannot supply, and says why under it** — the silence the issue
/// measured was a slot that simply did not appear.
#[test]
fn the_pane_draws_the_slot_and_the_reason_it_cannot_be_plotted() {
    let mut app = editor("drawn");
    let at = app
        .world()
        .resource::<BenchState>()
        .names()
        .iter()
        .position(|n| *n == "hound")
        .expect("hound is a rig");
    app.world_mut().resource_mut::<BenchState>().selected = at;
    crate::settle_support::until(&mut app, "the hound measured and drawn", |a| {
        a.world().resource::<BenchReports>().by_rig.contains_key("hound")
    });
    crate::settle_support::settle(&mut app);

    // **Every declared slot has its own row**, slot 3's included: the row count is the slot count, so
    // a fourth slot the pane dropped is what this catches.
    let rows = slot_rows(&mut app);
    let slots: Vec<usize> = rows.iter().map(|(slot, _)| *slot).collect();
    assert_eq!(
        slots,
        vec![0, 1, 2, 3],
        "the pane draws one row per declared slot (`0 - clip 0` through `3 - clip 3`); it drew {slots:?}"
    );

    // **And the sentence under slot 3 says why there is no curve for it.**
    let texts = pane_texts(&mut app);
    let why = texts
        .iter()
        .find(|t| t.contains("slot 3") && t.contains("cannot") || t.contains("not in the asset"))
        .unwrap_or_else(|| panic!("the pane says why slot 3 has no curve; it draws: {texts:#?}"));
    assert!(
        why.contains("has 3") && why.contains("re-measure"),
        "the reason names the count and the remedy: {why}"
    );
}

/// **The plots legend's chip for an unplottable slot carries the same sentence**, so the legend is a
/// key and not a colour swatch with no curve under it.
#[test]
fn the_legends_chip_for_the_unplotted_slot_says_why() {
    let mut app = editor("legend");
    let at = app
        .world()
        .resource::<BenchState>()
        .names()
        .iter()
        .position(|n| *n == "hound")
        .expect("hound is a rig");
    app.world_mut().resource_mut::<BenchState>().selected = at;
    crate::settle_support::until(&mut app, "the hound measured and drawn", |a| {
        a.world().resource::<BenchReports>().by_rig.contains_key("hound")
    });
    crate::settle_support::settle(&mut app);

    let texts = pane_texts(&mut app);
    let chip = texts
        .iter()
        .find(|t| t.starts_with("3 ") && t.contains("not in the asset"))
        .unwrap_or_else(|| panic!("the legend's third slot chip says why it has no curve; it draws: {texts:#?}"));
    assert!(
        chip.contains("has 3"),
        "the chip names the count the asset does have: {chip}"
    );
}

/// **Every text in the bench's slot pane**, in the order it was spawned.
fn pane_texts(app: &mut App) -> Vec<String> {
    fn walk(world: &World, entity: Entity, out: &mut Vec<String>) {
        if let Some(text) = world.get::<Text>(entity) {
            out.push(text.0.clone());
        }
        if let Some(children) = world.get::<Children>(entity) {
            for child in children.iter() {
                walk(world, child, out);
            }
        }
    }
    let world = app.world_mut();
    let panes: Vec<Entity> = world.query_filtered::<Entity, With<SlotPane>>().iter(world).collect();
    let mut out = Vec::new();
    for pane in panes {
        walk(world, pane, &mut out);
    }
    out
}

/// **The slot rows, by their `"<n> - clip <m>"` cell** — one per declared slot.
///
/// A **row** is counted, not a cell: the pane spawns one row per slot and a cell per column inside it, so
/// a column's words say nothing about whether the slot drew. The cell's text is the slot's name, and
/// its parent's parent is the row (`cell` wrapper, then the row) — the same walk
/// `the_plots_legend_wraps_inside_its_panel` makes.
fn slot_rows(app: &mut App) -> Vec<(usize, Entity)> {
    let world = app.world_mut();
    let mut texts = world.query::<(Entity, &Text)>();
    let cells: Vec<(usize, Entity)> = texts
        .iter(world)
        .filter_map(|(entity, text)| {
            let (n, m) = text.0.split_once(" - clip ")?;
            let slot = n.parse::<usize>().ok()?;
            m.parse::<usize>().ok()?;
            Some((slot, entity))
        })
        .collect();
    let mut rows: Vec<(usize, Entity)> = Vec::new();
    for (slot, cell) in cells {
        let Some(wrapper) = world.get::<ChildOf>(cell).map(ChildOf::parent) else { continue };
        let Some(row) = world.get::<ChildOf>(wrapper).map(ChildOf::parent) else { continue };
        if !rows.iter().any(|(_, e)| *e == row) {
            rows.push((slot, row));
        }
    }
    rows.sort_by_key(|(slot, _)| *slot);
    rows
}
