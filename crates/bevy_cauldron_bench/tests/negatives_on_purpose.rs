//! **Negatives on purpose: engine-made defects on a kept scene, and the turns that fixed what an assessor flagged** (DL-05, #464; series labels
//! from use, #459).
//!
//! Use produces mostly successes, and a student trained on successes learns to say yes. Two sources of negatives with known answers are proved
//! here, on **fresh copies of the benchmark's fixture** (BM-01) and with no editor window and no model:
//!
//! 1. **Perturbations**: one known defect per kind, made with the editor's own operations on a scratch copy of the project, and **confirmed by
//!    the engine's own measurement of the saved scene** (the floating piece's gap, the overlap volume, the area test), at the size the named
//!    constant says, near the visible threshold and plainly there.
//! 2. **Assess-then-fix pairs**: a turn in which a check flagged a problem and a later operation fixed it yields one negative and one positive.
//!
//! **The person's project is never written**: every file of it is hashed before a run and after, and a run's scratch copies are gone.
//!
//! The scene is `bench_floating` (a desk, a chair, a coffee table and a lamp standing on it), with a design area, a forest and a terrain added to
//! the person's copy so every defect has something to be made on. The fixture in the repository is not touched.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bevy_cauldron::ops_forest::{Forest, Tree};
use bevy_cauldron::perturb::{self, Defect, Refused, Size};
use bevy_cauldron::turn_store::{
    self, AgentRef, HandBackRef, ImageRef, Label, Line, LEDGER_FILE, OperationRecord, Polarity, Provenance, Request, Shot, Store, Turn,
};
use bevy_cauldron::{assess_fix, turn_store::sha256_hex};
use bevy_cauldron_bench::fixture;
use bevy_grimoire_core::heightfield::Heightfield;
use serde_json::json;

const MAP: &str = "bench_floating";

/// **The repository root**, which the fixture is laid out from.
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|e| panic!("cannot resolve the repository root: {e}"))
}

/// **A scratch directory this test owns**, fresh each call.
fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-negatives-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **The person's project**: the fixture laid out, plus a design area round the desk and chair, a ten-tree forest and a terrain with one peak.
fn persons_project(test: &str) -> PathBuf {
    let project = scratch(test).join("project");
    fixture::lay_out(&repo(), &project).unwrap_or_else(|e| panic!("the fixture is laid out: {e}"));
    let area = project.join("design/levels").join(MAP).join("areas/office");
    std::fs::create_dir_all(&area).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(area.join("area.yaml"), bevy_grimoire_core::design::area_yaml([-2.0, -2.5, 0.5, 1.0]).unwrap_or_else(|e| panic!("{e}")))
        .unwrap_or_else(|e| panic!("{e}"));
    add_forest(&project, 10);
    add_terrain(&project);
    project
}

/// **Where the project keeps its data**, as the editor says: beside the authoring kit, which is not always `assets/grimoire` itself.
fn grimoire(project: &Path) -> PathBuf {
    bevy_cauldron::project::Project::open(project, None).unwrap_or_else(|e| panic!("the project opens: {e}")).grimoire_dir
}

fn maps_dir(project: &Path) -> PathBuf {
    bevy_cauldron::project::Project::open(project, None).unwrap_or_else(|e| panic!("the project opens: {e}")).maps_dir
}

fn add_forest(project: &Path, trees: u32) {
    let forest = Forest {
        version: bevy_cauldron::ops_forest::FOREST_VERSION,
        trees: (0..trees)
            .map(|id| Tree {
                id,
                species: "oak".to_owned(),
                at: (id as f32 * 0.5 - 2.0, 2.0),
                ground_m: 0.0,
                yaw: 0.0,
                scale: 1.0,
                variant: 0,
                age_years: None,
                seed: None,
            })
            .collect(),
    };
    let path = grimoire(project).join("forest").join(format!("{MAP}.yaml"));
    std::fs::create_dir_all(path.parent().unwrap_or_else(|| panic!("a parent"))).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(path, bevy_grimoire_core::yaml::to_string(&forest).unwrap_or_else(|p| panic!("{}", p.line()))).unwrap_or_else(|e| panic!("{e}"));
}

/// A flat 13 x 11 terrain of half-metre cells, with one peak two metres high at its middle.
fn add_terrain(project: &Path) {
    let mut field = Heightfield::flat((-3.0, -2.5), 0.5, 13, 11, 0.0);
    field.heights[5 * 13 + 6] = 2.0;
    let path = grimoire(project).join("terrain").join(format!("{MAP}.yaml"));
    std::fs::create_dir_all(path.parent().unwrap_or_else(|| panic!("a parent"))).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(path, bevy_grimoire_core::yaml::to_string(&field).unwrap_or_else(|p| panic!("{}", p.line()))).unwrap_or_else(|e| panic!("{e}"));
}

/// **Every file under a folder, by relative path, with its SHA-256**: the person's project as bytes.
fn digests(root: &Path) -> BTreeMap<String, String> {
    fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned();
                out.insert(rel, sha256_hex(&bytes));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn picture(tag: u8) -> Shot {
    Shot::taken(ImageRef { sha256: format!("{tag:02x}").repeat(32), width: 64, height: 48, bytes: 1, flat: false })
}

/// **A kept turn of the scene**: what the person asked for names a desk, a chair and a lamp; what the turn touched is the desk, then the chair.
fn kept_turn(project: &Path) -> Turn {
    Turn {
        id: "turn-1700000000000-1".to_owned(),
        bench: false,
        started_unix_ms: 1_700_000_000_000,
        handed_back_unix_ms: 1_700_000_060_000,
        request: Request {
            hand_back: HandBackRef { kind: "presentation".to_owned(), id: "present-1".to_owned() },
            title: "Furnish the study".to_owned(),
            summary: "A desk with a chair at it, and a lamp on the coffee table.".to_owned(),
            specified: vec!["a desk and a chair".to_owned(), "a lamp on the coffee table".to_owned()],
            inferred: vec![],
            check: vec![],
            note: None,
        },
        before: Shot::missing("a test's turn has no before picture"),
        after: picture(0xa1),
        operations: vec![],
        facts: json!({
            "map": {"name": MAP, "placements": 4},
            "operations": [{"operation_id": "op-1", "pieces": [
                {"id": "desk", "descriptor": "desk", "at": [-0.5, -1.5], "yaw": 0.0},
                {"id": "chair", "descriptor": "chair", "at": [-1.0, 0.5], "yaw": 180.0},
            ]}],
        }),
        decisions: vec![],
        questions: Default::default(),
        provenance: Provenance {
            cauldron_version: "test".to_owned(),
            build_commit: "unknown".to_owned(),
            render: json!({}),
            agent: Some(AgentRef { name: "test".to_owned(), version: "0".to_owned(), model: None }),
            project_hash: sha256_hex(project.to_string_lossy().as_bytes()),
        },
    }
}

/// **A renderer with no window**: the reason there is no picture.
fn no_window(_: &Path, _: &str) -> Shot {
    Shot::missing("a headless test has nothing to draw with")
}

fn perturbed(project: &Path, turn: &Turn, defect: Defect, size: Size, serial: u32) -> Result<turn_store::Perturbed, Refused> {
    let scratch = project.parent().unwrap_or_else(|| panic!("a parent")).join("scratch");
    perturb::perturb(turn, project, &scratch, defect, size, &mut no_window, serial)
}

fn number(v: &serde_json::Value, what: &str) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("{what} is not a number: {v}"))
}

/// **1. Each defect is made once, and the engine's own measurement of the saved scene confirms it.**
#[test]
fn each_defect_is_made_and_the_engines_facts_confirm_it() {
    let project = persons_project("each");
    let turn = kept_turn(&project);
    let mut seen = Vec::new();
    for (n, defect) in Defect::ALL.into_iter().enumerate() {
        let record = perturbed(&project, &turn, defect, Size::Clear, n as u32 + 1).unwrap_or_else(|why| panic!("{why}"));
        seen.push(record.defect.clone());
        // What every record carries: its source turn and scene, the defect and its size, an objective label, and the editor's own operations.
        assert_eq!(record.source_turn, turn.id);
        assert_eq!(record.scene, turn_store::scene_of(&turn));
        assert_eq!(record.defect, defect.name());
        assert_eq!(record.size, "clear");
        assert_eq!(record.magnitude, defect.magnitude(Size::Clear));
        assert_eq!(record.tier, "objective");
        assert_eq!(record.label, "defective");
        assert!(!record.operations.is_empty() && record.operations.iter().all(|o| o.line["outcome"] == "applied"), "{:?}", record.operations);
        assert_eq!(record.before, turn.after, "the kept scene's picture is the turn's own after picture");
        assert!(record.after.image.is_none() && record.after.missing.is_some(), "a headless run says it drew nothing: {:?}", record.after);
        let (before, after) = (&record.facts["before"], &record.facts["after"]);
        match defect {
            Defect::Float => {
                assert!(number(&before["gap_m"], "the gap before").abs() < 1e-3, "{before}");
                assert!((number(&after["gap_m"], "the gap") - f64::from(perturb::FLOAT_CLEAR_M)).abs() < 0.01, "the floating piece's gap: {after}");
            }
            Defect::Clip => {
                assert_eq!(before["engine_overlap"], false, "{before}");
                assert_eq!(after["engine_overlap"], true, "the engine's own overlap pass finds the pair: {after}");
                assert!(number(&after["overlap_volume_m3"], "the volume") > 0.0, "the overlap volume: {after}");
                assert!(number(&after["plan_overlap_m2"], "the area") > 0.0 && number(&after["vertical_overlap_m"], "the height") > 0.0, "{after}");
            }
            Defect::OutsideArea => {
                assert_eq!((&before["inside"], &after["inside"]), (&json!(true), &json!(false)), "the area test: {before} then {after}");
                assert!((number(&after["distance_outside_m"], "the distance") - f64::from(perturb::OUTSIDE_CLEAR_M)).abs() < 0.01, "{after}");
            }
            Defect::DeleteNamed => {
                assert_eq!((&before["present"], &after["present"]), (&json!(true), &json!(false)), "{before} then {after}");
                assert_eq!(number(&before["count"], "count") - number(&after["count"], "count"), 1.0);
                assert_eq!(after["named_by"], "desk", "the first piece the turn touched that the request names");
            }
            Defect::TurnWrong => {
                assert!((number(&after["turned_deg"], "the turn") - f64::from(perturb::TURN_CLEAR_DEG)).abs() < 0.5, "{after}");
            }
            Defect::ThinForest => {
                assert_eq!(before["trees"], 10);
                assert_eq!(after["trees"], 4, "three in five of ten trees are taken out: {after}");
            }
            Defect::FlattenPeak => {
                assert!((number(&before["height_m"], "the peak") - 2.0).abs() < 1e-3, "{before}");
                assert!((number(&after["flattened_fraction"], "the fraction") - f64::from(perturb::FLATTEN_CLEAR_FRACTION)).abs() < 0.02, "{after}");
            }
        }
    }
    assert_eq!(seen, ["float", "clip", "outside_area", "delete_named", "turn_wrong", "thin_forest", "flatten_peak"]);
}

/// **2. Sizes near the visible threshold are made too**: easy negatives teach little.
#[test]
fn a_defect_near_the_visible_threshold_is_made_at_its_named_size() {
    let project = persons_project("near");
    let turn = kept_turn(&project);
    for (n, defect) in Defect::ALL.into_iter().filter(|d| d.sizes().contains(&Size::Near)).enumerate() {
        let record = perturbed(&project, &turn, defect, Size::Near, n as u32 + 1).unwrap_or_else(|why| panic!("{why}"));
        assert_eq!(record.size, "near");
        assert_eq!(record.magnitude, defect.magnitude(Size::Near));
        assert!(defect.magnitude(Size::Near) < defect.magnitude(Size::Clear), "{defect:?}: the near size is the smaller");
        let after = &record.facts["after"];
        match defect {
            Defect::Float => assert!((number(&after["gap_m"], "gap") - f64::from(perturb::FLOAT_NEAR_M)).abs() < 0.01, "{after}"),
            Defect::Clip => {
                assert_eq!(after["engine_overlap"], true, "{after}");
                assert!(number(&after["overlap_volume_m3"], "volume") > 0.0, "{after}");
            }
            Defect::OutsideArea => assert!((number(&after["distance_outside_m"], "d") - f64::from(perturb::OUTSIDE_NEAR_M)).abs() < 0.01, "{after}"),
            Defect::TurnWrong => assert!((number(&after["turned_deg"], "turn") - f64::from(perturb::TURN_NEAR_DEG)).abs() < 0.5, "{after}"),
            Defect::ThinForest => assert_eq!(after["trees"], 8, "{after}"),
            Defect::FlattenPeak => assert!((number(&after["flattened_fraction"], "f") - f64::from(perturb::FLATTEN_NEAR_FRACTION)).abs() < 0.02, "{after}"),
            Defect::DeleteNamed => unreachable!("a deletion has no near size"),
        }
    }
    // The named constants are the visible-threshold ones: a clip no deeper than three of the engine's own tolerances.
    assert!(perturb::CLIP_NEAR_M <= 3.0 * bevy_grimoire_core::stack::OVERLAP_EPS + f32::EPSILON);
}

/// **3. The person's project is byte-for-byte what it was**, after a whole run over a ledger, and the run's scratch copies are gone.
#[test]
fn a_run_leaves_the_persons_project_byte_for_byte_as_it_was() {
    let project = persons_project("bytes");
    let labels = scratch("bytes-labels");
    let scratch_dir = scratch("bytes-scratch");
    let turn = kept_turn(&project);
    let mut store = Store::new(labels.clone());
    store.append(&Line::turn(turn.clone())).unwrap_or_else(|e| panic!("{e}"));
    store
        .append(&Line::label(Label {
            turn: turn.id.clone(),
            label: "kept".to_owned(),
            tier: "gold".to_owned(),
            at_unix_ms: 1_700_000_100_000,
            operation_id: None,
            by_turn: None,
            item: None,
        }))
        .unwrap_or_else(|e| panic!("{e}"));

    let before = digests(&project);
    assert!(before.len() > 10, "the project has files to compare: {}", before.len());
    let read = turn_store::read_file(&labels.join(LEDGER_FILE)).unwrap_or_else(|e| panic!("{e}"));
    let report = perturb::run(&read, &mut store, &project, &scratch_dir, &mut no_window).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(digests(&project), before, "the person's project changed");
    assert_eq!(std::fs::read_dir(&scratch_dir).map(|d| d.count()).unwrap_or(0), 0, "a scratch copy was left behind");

    // 7 defects, six of them in two sizes and a deletion in one: thirteen records, none refused on this scene.
    assert_eq!(report.refused, [], "{:?}", report.refused);
    assert_eq!(report.written.len(), 13, "{:?}", report.written);

    // They are in the same ledger, marked with the defect, its size and the turn they came from, and every one keeps the source scene's id.
    let after = turn_store::read_file(&labels.join(LEDGER_FILE)).unwrap_or_else(|e| panic!("{e}"));
    assert!(after.damaged.is_empty(), "{:?}", after.damaged);
    let records: Vec<_> = after.perturbed().collect();
    assert_eq!(records.len(), 13);
    assert!(records.iter().all(|r| r.source_turn == turn.id && r.scene == turn_store::scene_of(&turn) && r.tier == "objective"));
    assert_eq!(after.examples().len(), 26, "a positive and a negative from each");
    assert_eq!(after.examples().iter().filter(|e| e.polarity == Polarity::Negative).count(), 13);

    // A second run adds nothing: what is recorded is not made again.
    let again = perturb::run(&after, &mut store, &project, &scratch_dir, &mut no_window).unwrap_or_else(|e| panic!("{e}"));
    assert!(again.written.is_empty() && again.refused.is_empty(), "{again:?}");
    assert_eq!(digests(&project), before);
}

/// **4. A defect that cannot be made is refused, with the reason, and nothing is recorded for it.**
#[test]
fn a_defect_that_cannot_be_made_is_refused_and_says_why() {
    let project = persons_project("refused");
    let turn = kept_turn(&project);

    // No forest, no terrain, no area: three kinds of scene with nothing to break.
    std::fs::remove_file(grimoire(&project).join("forest").join(format!("{MAP}.yaml"))).unwrap_or_else(|e| panic!("{e}"));
    std::fs::remove_file(grimoire(&project).join("terrain").join(format!("{MAP}.yaml"))).unwrap_or_else(|e| panic!("{e}"));
    std::fs::remove_dir_all(project.join("design")).unwrap_or_else(|e| panic!("{e}"));
    for (defect, says) in [(Defect::ThinForest, "no forest"), (Defect::FlattenPeak, "no terrain"), (Defect::OutsideArea, "no design area")] {
        let refused = perturbed(&project, &turn, defect, Size::Clear, 1).expect_err("nothing to break");
        assert!(refused.why.contains(says), "{refused}");
    }

    // A flat terrain has no peak.
    let mut flat = Heightfield::flat((-3.0, -2.5), 0.5, 13, 11, 0.0);
    flat.heights[0] = 0.1;
    std::fs::create_dir_all(grimoire(&project).join("terrain")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(grimoire(&project).join("terrain").join(format!("{MAP}.yaml")), bevy_grimoire_core::yaml::to_string(&flat).unwrap_or_else(|p| panic!("{}", p.line())))
        .unwrap_or_else(|e| panic!("{e}"));
    let refused = perturbed(&project, &turn, Defect::FlattenPeak, Size::Clear, 1).expect_err("no peak");
    assert!(refused.why.contains("no peak to flatten"), "{refused}");

    // A request that names nothing on the map has nothing to delete.
    let mut vague = turn.clone();
    vague.request.title = "Tidy up".to_owned();
    vague.request.summary = "Made it nicer.".to_owned();
    vague.request.specified = vec!["something cosy".to_owned()];
    let refused = perturbed(&project, &vague, Defect::DeleteNamed, Size::Clear, 1).expect_err("nothing named");
    assert!(refused.why.contains("names nothing"), "{refused}");

    // A deletion has no near size.
    let refused = perturbed(&project, &turn, Defect::DeleteNamed, Size::Near, 1).expect_err("no near size");
    assert!(refused.why.contains("no near size"), "{refused}");

    // A scene the turn did not leave is refused, naming how it differs: the person has moved the desk since.
    let map_path = maps_dir(&project).join(format!("{MAP}.map.yaml"));
    let text = std::fs::read_to_string(&map_path).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&map_path, text.replacen("- -0.5\n  - -1.5", "- -0.9\n  - -1.5", 1)).unwrap_or_else(|e| panic!("{e}"));
    let moved = digests(&project);
    let refused = perturbed(&project, &turn, Defect::Float, Size::Clear, 1);
    if let Err(refused) = &refused {
        assert!(refused.why.contains("has changed since") || refused.why.contains("not where the turn left it"), "{refused}");
    } else {
        // The replacement text did not match this file's spelling; move the desk through the map itself instead.
        let mut parsed = bevy_grimoire_core::map::Map::parse(&text).unwrap_or_else(|e| panic!("{e}"));
        parsed.placements[0].at.0 = -0.9;
        std::fs::write(&map_path, bevy_grimoire_core::yaml::to_string(&parsed).unwrap_or_else(|p| panic!("{}", p.line()))).unwrap_or_else(|e| panic!("{e}"));
        let refused = perturbed(&project, &turn, Defect::Float, Size::Clear, 1).expect_err("the scene changed");
        assert!(refused.why.contains("not where the turn left it"), "{refused}");
    }
    let _ = moved;

    // Another person's project is refused by its hash.
    let elsewhere = persons_project("refused-elsewhere");
    let refused = perturbed(&elsewhere, &turn, Defect::Float, Size::Clear, 1).expect_err("another project");
    assert!(refused.why.contains("not the one the turn was made in"), "{refused}");
}

fn operation(id: &str) -> OperationRecord {
    OperationRecord { operation_id: id.to_owned(), op: "placement_set_lift".to_owned(), seq: 0, source: "agent".to_owned(), line: json!({"op": "placement_set_lift"}) }
}

fn answer(check: &str, found: bool, after_operations: usize, frame: Option<Shot>) -> serde_json::Value {
    let mut decision = json!({
        "params": {"check": check},
        "answer": {"check": check, "finding": {"check": check, "p_yes": if found { 0.91 } else { 0.04 }, "threshold": 0.7, "found": found}},
        "model": "clef-flash",
        "after_operations": after_operations,
    });
    if let Some(frame) = frame {
        decision["frame"] = serde_json::to_value(frame).unwrap_or_else(|e| panic!("{e}"));
    }
    decision
}

/// **5. An assess-then-fix turn yields one negative and one positive**, both silver; a turn that was flagged and never fixed, or fixed with no work
/// between the answers, yields none.
#[test]
fn an_assess_then_fix_turn_yields_one_negative_and_one_positive() {
    let project = persons_project("pairs");
    let mut turn = kept_turn(&project);
    turn.operations = vec![operation("op-1"), operation("op-2"), operation("op-3")];
    turn.decisions = vec![
        answer("floating", false, 1, None),
        answer("floating", true, 1, Some(picture(0xb1))),
        answer("floating", false, 3, Some(picture(0xb2))),
    ];

    let pairs = assess_fix::pairs(&turn);
    assert_eq!(pairs.len(), 1, "{pairs:?}");
    let pair = &pairs[0];
    assert_eq!((pair.check.as_str(), pair.tier.as_str()), ("floating", "silver"));
    assert_eq!((pair.flagged.decision, pair.cleared.decision), (1, 2));
    assert_eq!(pair.fixing_operations, ["op-2", "op-3"], "the work between the two answers is the fix");
    assert_eq!(pair.source_turn, turn.id);
    assert_eq!(pair.scene, turn_store::scene_of(&turn), "the pair stays in its scene");

    let examples = pair.examples();
    assert_eq!(examples.len(), 2);
    let negative = examples.iter().find(|e| e.polarity == Polarity::Negative).unwrap_or_else(|| panic!("no negative"));
    let positive = examples.iter().find(|e| e.polarity == Polarity::Positive).unwrap_or_else(|| panic!("no positive"));
    assert_eq!(negative.frame, picture(0xb1), "the flagged frame is the negative");
    assert_eq!(positive.frame, picture(0xb2), "the cleared frame is the positive");
    assert!(examples.iter().all(|e| e.tier == "silver" && e.scene == pair.scene));

    // Flagged and never cleared: no pair. Cleared with no work since the flag: no pair (a model changing its mind fixed nothing).
    let mut unfixed = turn.clone();
    unfixed.decisions = vec![answer("floating", true, 1, Some(picture(0xb1)))];
    assert!(assess_fix::pairs(&unfixed).is_empty());
    let mut idle = turn.clone();
    idle.decisions = vec![answer("floating", true, 2, Some(picture(0xb1))), answer("floating", false, 2, Some(picture(0xb2)))];
    assert!(assess_fix::pairs(&idle).is_empty());
    // Another check's clear is no fix of this one.
    let mut other = turn.clone();
    other.decisions = vec![answer("floating", true, 1, Some(picture(0xb1))), answer("overlap", false, 3, Some(picture(0xb2)))];
    assert!(assess_fix::pairs(&other).is_empty());
    // Ad-hoc questions are no named check's answer.
    let mut adhoc = turn.clone();
    adhoc.decisions = vec![json!({"params": {"questions": {"q": {}}}, "answer": {"q": {"p_yes": 0.9}}})];
    assert!(assess_fix::pairs(&adhoc).is_empty());

    // The editor takes a picture for a flag, and for the answer that clears one; not for an answer that flags nothing and clears nothing.
    let flag = answer("floating", true, 1, None);
    let clear = answer("floating", false, 3, None);
    assert!(assess_fix::wants_frame(&[], &flag));
    assert!(assess_fix::wants_frame(std::slice::from_ref(&flag), &clear), "an answer that clears a flag is framed");
    assert!(!assess_fix::wants_frame(&[], &clear), "a first answer of no is framed by no one");
    assert!(!assess_fix::wants_frame(&[flag, answer("floating", false, 2, None)], &clear), "a check that was already cleared has nothing more to clear");

    // A pair is a line of the ledger: it reads back whole, and a ledger written before pairs gets its pairs once.
    let labels = scratch("pairs-labels");
    let mut store = Store::new(labels.clone());
    store.append(&Line::turn(turn.clone())).unwrap_or_else(|e| panic!("{e}"));
    let read = turn_store::read_file(&labels.join(LEDGER_FILE)).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(assess_fix::write_missing(&read, &mut store).unwrap_or_else(|e| panic!("{e}")), [pair.id.clone()]);
    let read = turn_store::read_file(&labels.join(LEDGER_FILE)).unwrap_or_else(|e| panic!("{e}"));
    assert!(read.damaged.is_empty(), "{:?}", read.damaged);
    assert_eq!(read.assess_fixes().cloned().collect::<Vec<_>>(), pairs);
    assert_eq!(read.examples().len(), 2);
    assert!(assess_fix::write_missing(&read, &mut store).unwrap_or_else(|e| panic!("{e}")).is_empty(), "a second pass writes nothing");
}
