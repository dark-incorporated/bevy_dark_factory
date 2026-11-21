//! **The tour points at the control a step is about** (#510).
//!
//! `guides/first_map.json`'s first step says *"click any piece in the palette on the left"* and, before this, the person had to
//! find the palette himself: the tour knew what to say and nothing about what it was saying it *about*. `badges.rs` already
//! answers *"where is the control with this id"* (`sole_control` over `ComputedNode` + `UiGlobalTransform`); the tour had no id
//! to ask about. These tests are the join.
//!
//! The failure `renzora_tutorial/src/steps.rs:174` records is the one to design against: a step naming a control that matches
//! nothing *"silently matched nothing, so this step simply never highlighted"*. So a guide naming a control is checked at the
//! corpus level, and a running step that cannot find its control says so on the status line rather than drawing nothing.
#![cfg(feature = "debugger")]

use bevy::prelude::*;

use bevy_dark_factory::harness;
use bevy_dark_factory::tour::{self, TourRunning};
use bevy_debugger_bevy::Guide;

use crate::fixtures;

fn root(tag: &str) -> std::path::PathBuf {
    fixtures::Fixture::new(&format!("tour_point_{tag}_{}", std::process::id()))
        .descriptor("barrel", "alpha")
        .place_as("barrel@0", "barrel", (0.0, 0.0))
        .build("m")
}

fn editor(tag: &str, width: u32, height: u32) -> App {
    let mut app = harness::build_headless(&root(tag), "m", None).unwrap_or_else(|e| panic!("{e}"));
    // **A window, and no `resize_surface`** — the order `an_injected_pointer_over_a_dock_is_pickable`
    // uses, and the one that keeps the pointer on the surface image the UI camera renders to.
    app.world_mut().spawn((
        Window { resolution: bevy::window::WindowResolution::new(width, height), ..default() },
        bevy::window::PrimaryWindow,
    ));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// Every shipped guide, as `(file name, parsed script)`.
fn guides() -> Vec<(String, serde_json::Value)> {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(bevy_dark_factory::guided::GUIDES_DIR);
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()));
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        let script: serde_json::Value =
            serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name}: bad JSON: {e}"));
        out.push((name, script));
    }
    assert!(!out.is_empty(), "no guides found under {}", dir.display());
    out
}

/// The control a step is about, keyed by the step's own label — the shape a guide file carries.
fn controls_of(script: &serde_json::Value) -> Vec<(String, String)> {
    script["controls"]
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(label, id)| {
                    (
                        label.clone(),
                        id.as_str()
                            .unwrap_or_else(|| panic!("control of step `{label}` is not a string"))
                            .to_owned(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

fn step_labels(script: &serde_json::Value) -> Vec<String> {
    script["steps"]
        .as_array()
        .map(|s| s.iter().filter_map(|s| s["label"].as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

/// **Every control a shipped guide names is a control, and resolves to exactly one laid-out node.**
///
/// The guide-side twin of `every_checkpoint_a_shipped_guide_names_is_registered_and_runs`: a name with no `ControlId` behind it,
/// or one that is drawn twice, would be a pointer aimed at nothing or at two things, and neither says so at runtime.
#[test]
fn every_control_a_shipped_guide_names_resolves_to_exactly_one_rect() {
    let shipped = guides();
    let mut named: Vec<(String, String, String)> = Vec::new();
    for (file, script) in &shipped {
        let labels = step_labels(script);
        for (label, control) in controls_of(script) {
            assert!(
                labels.contains(&label),
                "{file}: `controls` names step `{label}`, which no step has. Steps: {}",
                labels.join(", ")
            );
            assert!(
                tour::control_of(&control).is_some(),
                "{file}: step `{label}` names control `{control}`, which is not a `keys::ControlId`. \
                 A step pointing at nothing is the failure `renzora_tutorial/src/steps.rs:174` \
                 records: it silently highlights nothing"
            );
            named.push((file.clone(), label, control));
        }
    }
    assert!(
        !named.is_empty(),
        "no shipped guide names a control at all, so nothing below is measuring the join"
    );

    // **Every named control is laid out exactly once on at least one tab, and never twice on any.**
    //
    // Zero somewhere is right (a control belongs to one tab and the others have it hidden), so the assertion is "at
    // least one tab draws it, exactly once". Two on one tab is `sole_control`'s `None` — the pointer would aim at
    // nothing and the runtime problem would be the only report, so the corpus refuses it here as well.
    //
    // **One app per tab, built once**, and every id counted against each — twelve tabs times nine ids is 108
    // builds otherwise, and a headless editor is seconds apiece.
    let ids: Vec<(String, String, bevy_dark_factory::keys::ControlId)> = named
        .iter()
        .map(|(file, label, control)| {
            (
                file.clone(),
                label.clone(),
                tour::control_of(control).unwrap_or_else(|| {
                    panic!("{file}: step `{label}` names `{control}`, not a control")
                }),
            )
        })
        .collect();
    // The Map is the tour's own tab, so its four are read from an editor on the Map.
    let mut app = editor("resolve", 1600, 1200);
    let on_map: Vec<&(String, String, bevy_dark_factory::keys::ControlId)> = ids
        .iter()
        .filter(|(_, _, id)| bevy_dark_factory::badges::control_rect(app.world_mut(), *id).is_some())
        .collect();
    assert!(
        !on_map.is_empty(),
        "not one shipped guide names a control that is on the Map, so the tour's own steps are unchecked: {named:?}"
    );
    // Every tab, once each: which ids it draws, and how many times.
    // `ControlId` is not `Hash` (a symbolic id needs no map), so the counts are a flat list keyed by id.
    let mut counts: Vec<(bevy_dark_factory::keys::ControlId, String, usize)> = Vec::new();
    for mode in bevy_dark_factory::tiles::Mode::ALL {
        let mut other = harness::build_headless_at(&root("tabs"), "m", None, mode)
            .unwrap_or_else(|e| panic!("{e}"));
        for _ in 0..6 {
            other.update();
        }
        for (_, _, id) in &ids {
            let n = bevy_dark_factory::badges::controls_shown(other.world_mut(), *id);
            if n > 0 {
                counts.push((*id, mode.label().to_owned(), n));
            }
        }
    }
    for (file, label, id) in &ids {
        let draws: Vec<(String, usize)> = counts
            .iter()
            .filter(|(c, ..)| c == id)
            .map(|(_, mode, n)| (mode.clone(), *n))
            .collect();
        let twice: Vec<&(String, usize)> = draws.iter().filter(|(_, n)| *n > 1).collect();
        assert!(
            twice.is_empty(),
            "{file}: step `{label}` names {id:?}, laid out twice on the same tab: {twice:?}. \
             `badges::sole_control` answers `None` to a duplicate, so the pointer would aim at nothing"
        );
        assert!(
            !draws.is_empty(),
            "{file}: step `{label}` names {id:?}, which no tab lays out. A guide that points at a control the \
             editor does not draw is the `renzora_tutorial` bug this exists to prevent"
        );
    }
}

/// **A step naming a control that is not on screen says so on the status line**, rather than pointing at nothing.
///
/// `badges.rs`'s `sole_control` answers `None` for a control drawn twice as well as for one absent; both are a guide that cannot be
/// walked, and both must reach the person rather than the log.
#[test]
fn a_step_naming_a_control_that_is_not_there_raises_a_problem() {
    let mut app = editor("absent", 1600, 1200);
    app.world_mut().resource_mut::<Guide>().post(vec![bevy_debugger_bevy::Step {
        label: "a step about a tile".to_owned(),
        goal: String::new(),
        hints: vec![],
        checkpoint: None,
        with: None,
        recovery: "there is none".to_owned(),
    }]);
    app.world_mut().insert_resource(tour::ScriptControls::of(&serde_json::json!({
        "controls": { "a step about a tile": "Grid" }
    })));
    app.world_mut().insert_resource(TourRunning);
    app.update();
    let said = app
        .world()
        .resource::<bevy_dark_factory::editor::EditorState>()
        .status
        .problem_text()
        .to_owned();
    assert!(
        said.contains("Grid") && said.contains("a step about a tile"),
        "a step pointed at `Grid` with no tile open, and the status line said {said:?}. A pointer that \
         draws nothing is the `renzora_tutorial` bug this is designed against"
    );
}

/// **A click at the highlighted control's centre reaches the control while the tour runs.**
///
/// The outline and its arrow sit over the palette, so every node of the tour's layer has to be `Pickable::IGNORE` — otherwise the
/// pointer that the step tells the person to use lands on the thing pointing at it.
// **Ignored while #533 is open**: this is the only test here that injects a pointer, and the
// pointer never hovers the palette — `HoverMap` stays empty, so the click has nothing to land on.
// Its subject is unproven rather than disproved, and leaving it red would block a suite of 5,049
// tests over one assertion nobody has made pass yet. The fixture's missing `PrimaryWindow` used to
// make this hang for 270 s instead of failing; that is fixed, and this is what it was hiding.
#[ignore = "the injected pointer never hovers the palette (#533)"]
#[test]
fn a_click_at_the_outlined_control_reaches_it() {
    use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
    use bevy::picking::pointer::{Location, PointerAction, PointerId, PointerInput};

    let mut app = editor("clickthrough", 1600, 1200);
    app.world_mut().resource_mut::<Guide>().post(vec![bevy_debugger_bevy::Step {
        label: "Pick a piece".to_owned(),
        goal: String::new(),
        hints: vec![],
        checkpoint: None,
        with: None,
        recovery: "click a piece".to_owned(),
    }]);
    app.world_mut().insert_resource(tour::ScriptControls::of(&serde_json::json!({
        "controls": { "Pick a piece": "Palette" }
    })));
    app.world_mut().insert_resource(TourRunning);
    for _ in 0..4 {
        app.update();
    }
    let at = {
        let id = tour::control_of("Palette").expect("Palette is a control");
        let rect = bevy_dark_factory::badges::control_rect(app.world_mut(), id)
            .expect("the palette is on the Map");
        rect.center()
    };

    let handle = app
        .world()
        .get_resource::<bevy_dark_factory::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    let location = Location {
        target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }),
        position: at,
    };
    app.world_mut()
        .write_message(PointerInput::new(PointerId::Mouse, location, PointerAction::Move { delta: Vec2::ZERO }));
    app.update();

    let hits: Vec<Entity> = app
        .world()
        .get_resource::<bevy::picking::hover::HoverMap>()
        .and_then(|map| map.0.get(&PointerId::Mouse))
        .map(|hits| hits.iter().map(|(entity, _)| *entity).collect())
        .unwrap_or_default();
    assert!(!hits.is_empty(), "a move onto the palette hovered nothing at {at:?}");

    let layer = app
        .world_mut()
        .query_filtered::<Entity, With<tour::TourPointerLayer>>()
        .single(app.world())
        .expect("a running step with a control draws a pointer layer");
    let world = app.world_mut();
    let mut parents = world.query::<&ChildOf>();
    let parents = parents.iter(world).cloned().collect::<Vec<_>>();
    let in_layer = |entity: Entity| {
        let mut at = Some(entity);
        while let Some(e) = at {
            if e == layer {
                return true;
            }
            at = parents.iter().find(|p| p.parent() == e).map(|p| p.parent());
        }
        false
    };
    assert!(
        !hits.iter().any(|e| in_layer(*e)),
        "the pointer at the palette's centre {at:?} hit the tour's own layer. Every node of it must be \
         `Pickable::IGNORE`, or the outline eats the click the step asks for"
    );
    // And the button the step names is really under that point: a press there would land on it.
    let mut buttons = world.query_filtered::<Entity, With<bevy::ui_widgets::Button>>();
    let pressed = buttons.iter(world).any(|e| hits.contains(&e));
    assert!(
        pressed || hits.iter().any(|e| world.get::<bevy_dark_factory::chrome::Control>(*e).is_some()),
        "the point {at:?} is the palette's own centre and no button or panel of it is under it: {hits:?}"
    );
}

/// **The arrow flips above a target that has no room below it**, and points down at it there — the case
/// `renzora_tutorial/src/highlight.rs:1-20` names (a status-bar menu close enough to the bottom edge that an arrow under it would be
/// clipped). Tested at both ends, because "always below" would pass a test that only ever asked about the middle of the window.
#[test]
fn the_arrow_flips_above_a_target_near_the_bottom_edge() {
    let window = Rect::from_corners(Vec2::ZERO, Vec2::new(1280.0, 720.0));
    let arrow = 24.0;
    // Room below: an arrow under the target.
    let high = Rect::from_corners(Vec2::new(100.0, 100.0), Vec2::new(300.0, 140.0));
    assert_eq!(
        tour::arrow_side(high, window, arrow),
        tour::ArrowSide::Below,
        "a target with the whole window under it gets the arrow below it"
    );
    // No room below (the status band is here): flip, and point down.
    let low = Rect::from_corners(Vec2::new(100.0, 660.0), Vec2::new(300.0, 700.0));
    assert_eq!(
        tour::arrow_side(low, window, arrow),
        tour::ArrowSide::Above,
        "a target 20 px off the window's foot has no room for an arrow below it, so the arrow must flip \
         above and point down or it is clipped"
    );
    assert_eq!(
        tour::arrow_side(low, window, arrow).glyph(),
        '\u{25bc}',
        "an arrow above its target points down at it"
    );
    assert_eq!(
        tour::arrow_side(high, window, arrow).glyph(),
        '\u{25b2}',
        "an arrow below its target points up at it"
    );
}
