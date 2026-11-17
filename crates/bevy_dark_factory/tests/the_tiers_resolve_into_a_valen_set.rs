//! **The label tiers resolved and exported as Valen's JSONL, split by scene** (DL-06, #465; series labels from use, #459).
//!
//! The ledger holds what happened and what is known about it. The export is the opinion about which of it a student may learn from:
//!
//! 1. **Gold wins**, objective answers its own checks, silver is used only where its three votes agree with each other or with a gold or objective
//!    answer, and everything else is **parked** and counted.
//! 2. **A gold label that contradicts silver keeps the gold and records the disagreement**, which DL-08 counts.
//! 3. **The split is by scene**, so no `group_id` is in both `train.jsonl` and `eval.jsonl`, and a benchmark record's scene is eval.
//! 4. **The human holdout (DL-07) is its own file** and never mixed into train.
//! 5. **A record of a build older than the named one is left out by default**, and counted.
//! 6. **Two exports of one ledger are byte-identical**, and the files obey the contract Valen's own reader checks.

use std::collections::{BTreeMap, BTreeSet};

use bevy_oracle::Question;
use serde_json::{Value, json};

use bevy_dark_factory::turn_export::{self, EVAL_FILE, Exported, HOLDOUT_FILE, Options, SUMMARY_FILE, TRAIN_FILE};
use bevy_dark_factory::turn_questions::{Questions, TurnQuestion};
use bevy_dark_factory::turn_store::{
    AUDIT, AssessFix, AssessedAnswer, AssessedQuestion, Assessment, AssessorRef, HandBackRef, Label, Line as LedgerLine, Perturbed, Provenance, Request as Asked, Shot,
    Store, Turn, Vote,
};

const NEW_BUILD: &str = turn_export::OLDEST_BUILD;

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("blueprint_export_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn noul(id: &str, said: &str) -> TurnQuestion {
    TurnQuestion {
        id: id.to_owned(),
        source: "specified".to_owned(),
        about: "after".to_owned(),
        question: serde_json::to_value(Question::noul(said)).expect("a question"),
        engine: None,
    }
}

/// **A turn of one scene**, with four questions: two the person or the assessor answered, one the engine answered, and the turn question.
fn a_turn(store: &mut Store, id: &str, map: &str, version: &str, bench: bool, engine_floating: Option<bool>) -> Turn {
    let before = store.put_image(format!("{id} before").as_bytes(), 4, 4).expect("kept");
    let after = store.put_image(format!("{id} after").as_bytes(), 4, 4).expect("kept");
    let mut items = vec![noul("specified-1", "The result satisfies: a door on the east side"), noul("specified-2", "The result satisfies: a floor")];
    items.push(TurnQuestion {
        id: "check-floating".to_owned(),
        source: "check".to_owned(),
        about: "after".to_owned(),
        question: serde_json::to_value(Question::noul("Is any piece floating?")).expect("a question"),
        engine: engine_floating.map(|yes| bevy_dark_factory::turn_questions::EngineAnswer::new(yes, "measured from the map")),
    });
    // **A question about the change between the two pictures** (the shipped `out_of_area` check asks exactly this): its state must show both.
    items.push(TurnQuestion {
        id: "check-out_of_area".to_owned(),
        source: "check".to_owned(),
        about: "change".to_owned(),
        question: serde_json::to_value(Question::noul("Compared with the before image, did anything change outside the area?")).expect("a question"),
        engine: None,
    });
    items.push(TurnQuestion {
        id: "check-clipping".to_owned(),
        source: "check".to_owned(),
        about: "after".to_owned(),
        question: serde_json::to_value(Question::noul("Does any piece clip another?")).expect("a question"),
        engine: None,
    });
    items.push(noul("turn-kept", "The person keeps this result"));
    items.last_mut().expect("a question").source = "turn".to_owned();
    Turn {
        id: id.to_owned(),
        bench,
        started_unix_ms: 10,
        handed_back_unix_ms: 20,
        request: Asked {
            hand_back: HandBackRef { kind: "presentation".into(), id: "present-1".into() },
            title: "a courtyard".into(),
            summary: "A courtyard with a door.".into(),
            specified: vec!["a door on the east side".into(), "a floor".into()],
            inferred: Vec::new(),
            check: Vec::new(),
            note: None,
        },
        before: Shot::taken(before),
        after: Shot::taken(after),
        operations: Vec::new(),
        facts: json!({"map": {"name": map, "placements": 3}}),
        decisions: Vec::new(),
        provenance: Provenance {
            dark_factory_version: version.to_owned(),
            build_commit: format!("commit-{version}"),
            render: Value::Null,
            agent: None,
            project_hash: "the-one-project".to_owned(),
        },
        questions: Questions { template_version: 1, items, skipped: Vec::new() },
    }
}

fn label(turn: &str, what: &str, tier: &str, at: u64) -> Label {
    Label {
        turn: turn.to_owned(),
        label: what.to_owned(),
        tier: tier.to_owned(),
        at_unix_ms: at,
        operation_id: None,
        by_turn: None,
        item: None,
        question: None,
        answer: None,
        asked_because: None,
        holdout: false,
    }
}

/// **The person answered one question of the audit** (DL-07): gold, and the holdout.
fn audit(turn: &str, question: &str, answer: &str) -> Label {
    Label {
        turn: turn.to_owned(),
        label: AUDIT.to_owned(),
        tier: "gold".to_owned(),
        at_unix_ms: 5_000,
        question: Some(question.to_owned()),
        answer: Some(answer.to_owned()),
        asked_because: Some("random".to_owned()),
        holdout: true,
        ..label(turn, AUDIT, "gold", 5_000)
    }
}

fn vote(wording: u32, p_yes: Option<f64>) -> Vote {
    Vote {
        wording,
        answer: p_yes.map(|p| json!({"type": "noul", "noul": p})),
        p_yes,
        refused: p_yes.is_none().then(|| "the provider declined".to_owned()),
        model: Some("stand-in".to_owned()),
    }
}

/// **An assessor's answer to one question**, as three votes.
fn assessed(id: &str, votes: [Option<f64>; 3]) -> AssessedQuestion {
    let stored: Vec<Vote> = votes.iter().enumerate().map(|(at, p)| vote(at as u32, *p)).collect();
    let valid: Vec<f64> = votes.iter().flatten().copied().collect();
    let yes_fraction = (!valid.is_empty()).then(|| valid.iter().filter(|p| **p >= 0.5).count() as f64 / valid.len() as f64);
    let refused = votes.iter().filter(|p| p.is_none()).count() as u32;
    AssessedQuestion { id: id.to_owned(), votes: stored, yes_fraction, tally: BTreeMap::new(), refused }
}

fn assessment(turn: &str, answers: Vec<AssessedQuestion>) -> Assessment {
    Assessment {
        turn: turn.to_owned(),
        assessor: AssessorRef { version: 1, endpoint: "http://127.0.0.1:8093/v1/systemone".into(), site: "local".to_owned() },
        at_unix_ms: 3_000,
        answers,
        skipped: None,
    }
}

/// **A map name whose scene lands on the side asked for**, so the fixture exercises both files whatever the hash says.
fn a_map_on(map_side: bool) -> String {
    for n in 0..200 {
        let map = format!("court{n}");
        let key = format!("the-one-project\0{map}");
        let scene = format!("scene-{}", &bevy_dark_factory::turn_store::sha256_hex(key.as_bytes())[..16]);
        if turn_export::in_eval(&scene, turn_export::EVAL_SHARE) == map_side {
            return map;
        }
    }
    panic!("no map name lands on the side asked for");
}

/// **The records a file holds**: a file an export did not write (because no record of its side exists) holds none.
fn read_lines(dir: &std::path::Path, file: &str) -> Vec<Value> {
    let path = dir.join(file);
    match std::fs::read_to_string(&path) {
        Ok(text) => text.lines().filter(|line| !line.trim().is_empty()).map(|line| serde_json::from_str(line).expect("a record")).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => panic!("{}: {e}", path.display()),
    }
}

/// **Every file under a folder, by relative path and hash**: the labels folder as it is, for an export to leave alone.
fn tree(root: &std::path::Path) -> BTreeMap<String, String> {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut BTreeMap<String, String>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())).flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned();
                out.insert(rel, bevy_dark_factory::turn_store::sha256_hex(&bytes));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// **Every record the export wrote**, whichever side each scene landed on.
fn all_records(dir: &std::path::Path) -> Vec<Value> {
    read_lines(dir, TRAIN_FILE).into_iter().chain(read_lines(dir, EVAL_FILE)).collect()
}

fn probabilities(record: &Value, question: &str) -> Option<Value> {
    record["targets"].get(question)?.get("probabilities").cloned().filter(|p| !p.is_null())
}

fn question(record: &Value) -> &str {
    record["group_id"].as_str().expect("a group id")
}

/// **The fixture ledger**: one scene on each side of the split, with every tier in it.
fn fixture(tag: &str) -> std::path::PathBuf {
    let labels = scratch(tag);
    let train_map = a_map_on(false);
    let eval_map = a_map_on(true);
    let mut store = Store::new(labels.clone());

    // **A scene that trains**: the person kept it, so the turn question is gold; the engine answered `check-floating`; the assessor agreed with
    // itself on `specified-1` (three votes for yes), split on `specified-2` (two of three said yes, so it is parked), and said yes on
    // `check-clipping`, where nothing else answers — one lone vote, parked too.
    let kept = a_turn(&mut store, "turn-kept", &train_map, NEW_BUILD, false, Some(false));
    store.append(&LedgerLine::turn(kept)).expect("a turn");
    store.append(&LedgerLine::label(label("turn-kept", "kept", "gold", 100))).expect("a label");
    store
        .append(&LedgerLine::assessment(assessment(
            "turn-kept",
            vec![
                assessed("specified-1", [Some(0.9), Some(0.8), Some(0.7)]),
                assessed("specified-2", [Some(0.9), Some(0.8), Some(0.1)]),
                assessed("check-floating", [Some(0.1), Some(0.2), Some(0.3)]),
                assessed("check-out_of_area", [Some(0.1), Some(0.15), Some(0.2)]),
                assessed("check-clipping", [Some(0.9), None, None]),
            ],
        )))
        .expect("an assessment");

    // **A scene that evals**: the assessor was unanimous that `specified-1` holds, and nobody else answered it, so its soft label stands. The
    // person undid the hand-back, so the turn question is a hard no. The person's audit answer to `specified-2` is the holdout.
    let undone = a_turn(&mut store, "turn-undone", &eval_map, NEW_BUILD, true, None);
    store.append(&LedgerLine::turn(undone)).expect("a turn");
    store.append(&LedgerLine::label(label("turn-undone", "undone", "gold", 120))).expect("a label");
    store.append(&LedgerLine::label(audit("turn-undone", "specified-2", "yes"))).expect("an audit answer");
    store.append(&LedgerLine::assessment(assessment("turn-undone", vec![assessed("specified-1", [Some(0.9), Some(0.8), Some(0.7)])]))).expect("an assessment");

    // **A build too old to teach from**, whatever its answers are.
    let old = a_turn(&mut store, "turn-old", "court-old", "0.6.0", false, Some(true));
    store.append(&LedgerLine::turn(old)).expect("a turn");
    store.append(&LedgerLine::label(label("turn-old", "kept", "gold", 130))).expect("a label");

    labels
}

fn export_of(labels: &std::path::Path, tag: &str, options: &Options) -> (std::path::PathBuf, Exported) {
    let out = scratch(tag);
    let exported = turn_export::export(labels, &out, options).expect("the export");
    (out, exported)
}

/// **1. Every tier resolves by its rule, and the parked ones are counted with the reason.**
#[test]
fn each_tier_resolves_by_its_rule_and_the_rest_is_parked() {
    let labels = fixture("tiers");
    let (out, exported) = export_of(&labels, "tiers-out", &Options::default());
    let records: Vec<Value> = all_records(&out);
    let kept = records
        .iter()
        .find(|record| record["meta"]["records"].as_array().is_some_and(|r| r.iter().any(|id| id == "turn-kept")))
        .unwrap_or_else(|| panic!("no record of the kept turn: {records:#?}"));

    // **Gold wins the turn question**, hard: the person kept it.
    assert_eq!(probabilities(kept, "turn-kept"), Some(json!({"true": 1.0, "false": 0.0})), "{kept:#?}");
    assert_eq!(kept["meta"]["questions"]["turn-kept"]["tier"], "gold", "the person's verdict is gold");
    assert_eq!(kept["meta"]["questions"]["turn-kept"]["source"], "person");

    // **The engine answers its own check**, hard, and no teacher is consulted for it.
    assert_eq!(probabilities(kept, "check-floating"), Some(json!({"true": 0.0, "false": 1.0})), "{kept:#?}");
    assert_eq!(kept["meta"]["questions"]["check-floating"]["tier"], "objective");
    assert_eq!(kept["meta"]["questions"]["check-floating"]["source"], "engine");

    // **Silver where its three votes agree**, soft: a unanimous yes is a 1.0 fraction, and it says which assessor version wrote it.
    assert_eq!(probabilities(kept, "specified-1"), Some(json!({"true": 1.0, "false": 0.0})), "{kept:#?}");
    assert_eq!(kept["meta"]["questions"]["specified-1"]["tier"], "silver");
    assert_eq!(kept["meta"]["questions"]["specified-1"]["assessor_version"], 1);

    // **Parked**: the votes that split, and the lone vote nothing corroborates. Both are in the record with no probabilities, never dropped.
    assert_eq!(probabilities(kept, "specified-2"), None, "a split vote is not a label: {kept:#?}");
    assert_eq!(probabilities(kept, "check-clipping"), None, "one lone vote is not a label: {kept:#?}");
    let parked: Vec<&turn_export::Parked> = exported.parked.iter().filter(|parked| parked.record.contains("turn-kept")).collect();
    let kinds: BTreeSet<&str> = parked.iter().map(|parked| parked.kind).collect();
    assert!(kinds.contains("votes-split") && kinds.contains("votes-refused"), "the parked reasons: {parked:#?}");
    assert_eq!(exported.parked_by_kind().get("votes-split"), Some(&1), "{:#?}", exported.parked);
    assert_eq!(exported.parked_by_kind().get("votes-refused"), Some(&1), "{:#?}", exported.parked);

    // **The record is Valen's shape**: the questions are the question shapes it reads, the images are relative `image_url` parts, and every
    // asset's hash is the picture's own.
    assert_eq!(kept["request"]["state"]["messages"][0]["content"][1]["type"], "image_url");
    assert_eq!(kept["assets"][0]["path"], kept["request"]["state"]["messages"][0]["content"][1]["image_url"]["url"]);
    let bytes = std::fs::read(out.join(kept["assets"][0]["path"].as_str().expect("a path"))).expect("the picture was copied");
    assert_eq!(kept["assets"][0]["sha256"], bevy_dark_factory::turn_store::sha256_hex(&bytes), "the hash is the picture's");
    assert_eq!(kept["meta"]["scene"], question(kept), "the group id is the scene");
    assert_eq!(kept["meta"]["build_commit"], format!("commit-{NEW_BUILD}"));

    // **The old build is left out by default, and counted**, and the answer says which build was too old.
    assert!(records.iter().all(|record| { !record["meta"]["records"].as_array().is_some_and(|r| r.iter().any(|id| id == "turn-old")) }));
    assert_eq!(exported.left_out_by_kind().get("old-build"), Some(&1), "{:#?}", exported.left_out);
    assert!(exported.left_out.iter().any(|left| left.why.contains("0.6.0") && left.why.contains(NEW_BUILD)), "{:#?}", exported.left_out);
}

/// **2. The holdout is its own file**: the person's audit answer is graded on, never trained on.
#[test]
fn the_human_holdout_is_its_own_file() {
    let labels = fixture("holdout");
    let (out, exported) = export_of(&labels, "holdout-out", &Options::default());
    assert!(exported.holdout >= 1, "the audit answer reached no holdout file: {exported:#?}");
    let holdout: Vec<Value> = read_lines(&out, HOLDOUT_FILE);
    let record = &holdout[0];
    assert_eq!(probabilities(record, "specified-2"), Some(json!({"true": 1.0, "false": 0.0})), "{record:#?}");
    assert_eq!(record["meta"]["questions"]["specified-2"]["tier"], "gold");
    // **No training record carries it**: the train and eval files hold the same state with that question unanswered.
    let trained: Vec<Value> = all_records(&out);
    let scene = question(record);
    for trained in trained.iter().filter(|trained| question(trained) == scene) {
        assert_eq!(probabilities(trained, "specified-2"), None, "an audit answer was trained on: {trained:#?}");
    }
    assert!(!holdout.is_empty());
}

/// **3. The split is by scene**: no `group_id` is in both files, the benchmark scene is eval, and every record of a scene is on one side.
#[test]
fn no_scene_is_on_both_sides_of_the_split() {
    let labels = fixture("split");
    let (out, exported) = export_of(&labels, "split-out", &Options::default());
    assert!(exported.train >= 1 && exported.eval >= 1, "both files have records: {exported:#?}");
    let train: BTreeSet<String> = read_lines(&out, TRAIN_FILE).iter().map(|record| question(record).to_owned()).collect();
    let eval: BTreeSet<String> = read_lines(&out, EVAL_FILE).iter().map(|record| question(record).to_owned()).collect();
    let shared: Vec<&String> = train.intersection(&eval).collect();
    assert!(shared.is_empty(), "a scene is on both sides: {shared:?}");

    // **The benchmark record is eval**, and its whole scene with it.
    let eval_records = read_lines(&out, EVAL_FILE);
    assert!(!eval_records.is_empty(), "the benchmark scene reached no eval file");
    let bench: Vec<&Value> = eval_records.iter().filter(|record| record["meta"]["records"].as_array().is_some_and(|r| r.iter().any(|id| id == "turn-undone"))).collect();
    assert!(!bench.is_empty(), "the benchmark turn is not in eval: {eval:#?}");
    assert!(bench.iter().all(|record| { record["group_id"].as_str().is_some_and(|scene| eval.contains(scene)) }));
    // **And the side is the scene's own**: the rule the export states, checked against what it wrote.
    for record in read_lines(&out, TRAIN_FILE).iter().chain(eval_records.iter()) {
        let scene = question(record);
        assert_eq!(eval.contains(scene), turn_export::in_eval(scene, turn_export::EVAL_SHARE), "the side of {scene} is not its own hash");
    }
}

/// **4. Two exports of one ledger are byte-identical**, and the names in `summary.json` are the counts.
#[test]
fn two_exports_of_one_ledger_are_byte_identical() {
    let labels = fixture("deterministic");
    // **The labels folder as bytes**, before the first export: the export is derived, so this is what it must still be.
    let held = tree(&labels);
    let (first, one) = export_of(&labels, "deterministic-one", &Options::default());
    let (second, two) = export_of(&labels, "deterministic-two", &Options::default());
    assert_eq!(one.train, two.train);
    assert_eq!(one.parked, two.parked);
    assert_eq!(one.disagreements, two.disagreements);
    for name in [TRAIN_FILE, EVAL_FILE, HOLDOUT_FILE, SUMMARY_FILE] {
        let a = std::fs::read(first.join(name));
        let b = std::fs::read(second.join(name));
        assert_eq!(a.is_ok(), b.is_ok(), "{name} was written by one export and not the other");
        if let (Ok(a), Ok(b)) = (a, b) {
            assert!(a == b, "{name} differs between two exports of one ledger");
        }
    }
    let summary: Value = serde_json::from_slice(&std::fs::read(first.join(SUMMARY_FILE)).expect("a summary")).expect("json");
    assert_eq!(summary["train_records"], one.train);
    assert_eq!(summary["parked"], serde_json::to_value(one.parked_by_kind()).expect("counts"));
    // **Nothing was written into the labels folder**: the export is derived, so every file of it is what it was.
    let ledger = std::fs::read(labels.join("turns.jsonl")).expect("the ledger");
    assert!(!String::from_utf8_lossy(&ledger).contains("group_id"), "the export wrote into the ledger");
    assert_eq!(tree(&labels), held, "the export changed the labels folder");
}

/// **5. A gold answer contradicted by silver keeps the gold, and the disagreement is recorded**: DL-08 counts the teacher's errors.
#[test]
fn a_gold_answer_that_contradicts_silver_keeps_the_gold_and_says_so() {
    let labels = scratch("disagreement");
    let mut store = Store::new(labels.clone());
    let map = a_map_on(false);
    let turn = a_turn(&mut store, "turn-x", &map, NEW_BUILD, false, Some(true));
    store.append(&LedgerLine::turn(turn)).expect("a turn");
    // The person undid the result; the assessor, unanimous, said the door was there and nothing floated.
    store.append(&LedgerLine::label(label("turn-x", "undone", "gold", 100))).expect("a label");
    store
        .append(&LedgerLine::assessment(assessment(
            "turn-x",
            // The assessor says nothing floated, and the engine says one did: the engine stands and the disagreement is kept.
            vec![assessed("specified-1", [Some(0.9), Some(0.8), Some(0.7)]), assessed("check-floating", [Some(0.1), Some(0.2), Some(0.05)])],
        )))
        .expect("an assessment");
    let (out, exported) = export_of(&labels, "disagreement-out", &Options::default());
    let records: Vec<Value> = all_records(&out);
    let record = records.first().unwrap_or_else(|| panic!("no record"));
    assert_eq!(probabilities(record, "turn-kept"), Some(json!({"true": 0.0, "false": 1.0})), "the person's no stands: {record:#?}");
    assert_eq!(record["meta"]["questions"]["turn-kept"]["tier"], "gold");
    // **The engine's yes on floating stands over the assessor's no, and the disagreement is written down** with both answers.
    assert_eq!(probabilities(record, "check-floating"), Some(json!({"true": 1.0, "false": 0.0})), "{record:#?}");
    let found: Vec<&turn_export::Disagreement> = exported.disagreements.iter().filter(|d| d.question == "check-floating").collect();
    assert_eq!(found.len(), 1, "{:#?}", exported.disagreements);
    assert_eq!(found[0].kept, turn_export::Tier::Objective);
    assert_eq!((found[0].kept_answer.as_str(), found[0].silver_answer.as_str()), ("yes", "0.00 yes"), "{found:?}");
}

/// **6. Every written record obeys the contract Valen's own reader checks** (`valen/data/schema.py`), which is what makes the file loadable.
#[test]
fn every_record_obeys_valens_readers_contract() {
    let labels = fixture("contract");
    let (out, _) = export_of(&labels, "contract-out", &Options::default());
    for file in [TRAIN_FILE, EVAL_FILE, HOLDOUT_FILE] {
        for record in read_lines(&out, file) {
            assert!(!question(&record).is_empty(), "group_id is required for leakage-free splits");
            let state = &record["request"]["state"];
            assert!(state.is_string() || state.get("messages").is_some(), "{file}: state must be text or a messages object");
            let questions = record["request"]["questions"].as_object().expect("a nonempty mapping of questions");
            assert!(!questions.is_empty(), "{file}: questions must be a nonempty mapping");
            let targets = record["targets"].as_object().expect("targets");
            for id in targets.keys() {
                assert!(questions.contains_key(id), "{file}: target `{id}` names no question");
            }
            for (id, shape) in questions {
                match shape["type"].as_str().expect("a type") {
                    "noul" => {
                        assert!(!shape["instructions"].as_str().unwrap_or_default().trim().is_empty(), "{file}: `{id}` has no instructions");
                        candidates_are(id, &json!({"true": "", "false": ""}), targets.get(id), file);
                    }
                    "choice" => {
                        let criteria = shape["criteria"].as_object().unwrap_or_else(|| panic!("{file}: `{id}` is a choice with no criteria"));
                        assert!((1..=255).contains(&criteria.len()), "{file}: `{id}` needs 1–255 options");
                        candidates_are(id, &Value::Object(criteria.clone()), targets.get(id), file);
                    }
                    "score" => {
                        let criteria = shape["criteria"].as_array().unwrap_or_else(|| panic!("{file}: `{id}` is a score with no levels"));
                        assert!((2..=255).contains(&criteria.len()), "{file}: `{id}` needs 2–255 levels");
                    }
                    other => {
                        panic!("{file}: `{id}` is a `{other}`, which Valen's reader does not take")
                    }
                }
            }
            // **Every asset's bytes hash to the name it is filed under**, and every picture the state shows is one of the assets.
            for asset in record["assets"].as_array().expect("assets") {
                let path = asset["path"].as_str().expect("a path");
                assert!(!path.contains("://"), "{file}: `{path}` must be relative and local");
                let bytes = std::fs::read(out.join(path)).unwrap_or_else(|e| panic!("{file}: {path}: {e}"));
                assert_eq!(asset["sha256"], bevy_dark_factory::turn_store::sha256_hex(&bytes), "{file}: {path} does not hash to its name");
            }
            let content = &record["request"]["state"]["messages"][0]["content"];
            let shown: BTreeSet<&str> = content.as_array().expect("content").iter().filter_map(|part| part["image_url"]["url"].as_str()).collect();
            let named: BTreeSet<&str> = record["assets"].as_array().expect("assets").iter().filter_map(|asset| asset["path"].as_str()).collect();
            assert_eq!(shown, named, "{file}: the pictures shown and the assets named are not the same");
        }
    }
}

/// **Valen's reader takes a distribution whose keys are exactly the candidates, none negative, summing to 1.**
fn candidates_are(id: &str, candidates: &Value, target: Option<&Value>, file: &str) {
    let Some(target) = target else { return };
    let Some(probabilities) = target.get("probabilities").filter(|p| !p.is_null()) else {
        return;
    };
    let probabilities = probabilities.as_object().unwrap_or_else(|| panic!("{file}: `{id}` probabilities are not an object"));
    let expected: BTreeSet<&str> = candidates.as_object().expect("candidates").keys().map(String::as_str).collect();
    let found: BTreeSet<&str> = probabilities.keys().map(String::as_str).collect();
    assert_eq!(found, expected, "{file}: `{id}` probability keys must exactly match the candidates");
    let mut sum = 0.0;
    for (option, p) in probabilities {
        let p = p.as_f64().unwrap_or_else(|| panic!("{file}: `{id}`.{option} is not a number"));
        assert!(p >= 0.0 && p.is_finite(), "{file}: `{id}`.{option} is negative or not finite");
        sum += p;
    }
    assert!((sum - 1.0).abs() < 1e-6, "{file}: `{id}` probabilities sum to {sum}, not 1");
}

/// **8. A change question is trained on both pictures**, and one whose state shows only the result is parked rather than answered from half.
#[test]
fn a_change_question_carries_the_before_and_the_after() {
    let labels = fixture("change");
    let (out, exported) = export_of(&labels, "change-out", &Options::default());
    let records = all_records(&out);
    // The kept turn's state shows the before and the after, so its `check-out_of_area` is answered, and the record names both pictures.
    let changed = records
        .iter()
        .find(|record| !record["targets"]["check-out_of_area"]["probabilities"].is_null())
        .unwrap_or_else(|| panic!("no change question was answered, though its state shows both pictures: {records:#?}"));
    let content = &changed["request"]["state"]["messages"][0]["content"];
    let shown: Vec<&str> = content.as_array().expect("content").iter().filter_map(|part| part["image_url"]["url"].as_str()).collect();
    assert_eq!(shown.len(), 2, "a change question is shown one picture: {changed:#?}");
    assert!(changed["request"]["state"]["messages"][0]["content"][0]["text"].as_str().is_some_and(|text| text.contains("before") && text.contains("after")));
    assert_eq!(changed["assets"].as_array().map(Vec::len), Some(2), "the record does not name both pictures: {changed:#?}");

    // **No state of two pictures answered a change question from one**: every answered one names both.
    for record in records.iter().filter(|record| !record["targets"]["check-out_of_area"]["probabilities"].is_null()) {
        assert_eq!(record["assets"].as_array().map(Vec::len), Some(2), "a change question was answered from one picture: {record:#?}");
    }
    let _ = exported;
}

/// **9. A ledger line that does not read refuses the export**, naming the line: a torn line is a turn or a label nobody can account for.
#[test]
fn a_damaged_ledger_line_refuses_the_export() {
    let labels = fixture("damaged");
    let ledger = labels.join("turns.jsonl");
    let mut text = std::fs::read_to_string(&ledger).expect("the ledger");
    text.push_str("{\"v\": 1, \"kind\": \"label\", \"turn\": \"turn-kept\", \"label\":");
    let line = text.lines().count();
    std::fs::write(&ledger, &text).expect("a torn line");
    let out = scratch("damaged-out");
    let why = turn_export::export(&labels, &out, &Options::default()).expect_err("a torn line is not exported past");
    assert!(why.contains(&format!("line {line}")) && why.contains("does not read"), "{why}");
    assert!(!out.join(TRAIN_FILE).is_file() && !out.join(EVAL_FILE).is_file(), "an export ran past a damaged line");
}

/// **10. An oldest build that is not a version is refused**, naming it, rather than admitting every build.
#[test]
fn an_oldest_build_that_is_not_a_version_is_refused() {
    let labels = fixture("oldest");
    let out = scratch("oldest-out");
    let why = turn_export::export(&labels, &out, &Options { oldest_build: "0.6.x".to_owned(), ..Options::default() }).expect_err("a bad bound");
    assert!(why.contains("0.6.x") && why.contains("is not a version"), "{why}");
    assert!(!out.join(TRAIN_FILE).is_file(), "an export ran with an oldest build that cannot be read");
    // A share of zero has no eval side, and is refused too.
    let why = turn_export::export(&labels, &out, &Options { eval_share: 0, ..Options::default() }).expect_err("a share of zero");
    assert!(why.contains("cannot be 0"), "{why}");
}

/// **11. A person's own change of mind is not the teacher's error**, and a re-export into a used folder leaves no stale side.
#[test]
fn a_person_disagreement_is_not_a_teacher_error_and_no_stale_side_is_left() {
    let labels = scratch("person");
    let mut store = Store::new(labels.clone());
    let map = a_map_on(false);
    let turn = a_turn(&mut store, "turn-p", &map, NEW_BUILD, false, None);
    store.append(&LedgerLine::turn(turn)).expect("a turn");
    // The person kept it, and then undid it: one question, two gold answers, the later stands.
    store.append(&LedgerLine::label(label("turn-p", "kept", "gold", 100))).expect("a label");
    store.append(&LedgerLine::label(label("turn-p", "undone", "gold", 200))).expect("a label");
    store.append(&LedgerLine::label(label("turn-p", "audit", "gold", 300))).expect("an audit");
    // The assessor says the result was kept, unanimously, against the person's gold `undone`: the one teacher error in this ledger.
    store
        .append(&LedgerLine::assessment(assessment(
            "turn-p",
            vec![assessed("turn-kept", [Some(0.9), Some(0.9), Some(0.8)]), assessed("check-out_of_area", [Some(0.1), Some(0.1), Some(0.2)])],
        )))
        .expect("an assessment");
    let (out, exported) = export_of(&labels, "person-out", &Options::default());
    assert_eq!(exported.disagreements_by_kind().get("person"), Some(&1), "{:#?}", exported.disagreements);
    // **The teacher's contradiction of the gold answer is the only teacher error**: the person's own change of mind is not one.
    assert_eq!(exported.disagreements_by_kind().get("teacher"), Some(&1), "{:#?}", exported.disagreements);
    assert_eq!(exported.teacher_errors(), 1, "the teacher's error count is not the teacher's alone: {:#?}", exported.disagreements);
    assert!(
        exported.disagreements.iter().filter(|d| d.kind == "teacher").all(|d| d.question == "turn-kept"),
        "a person's change of mind was counted as the teacher's: {:#?}",
        exported.disagreements
    );
    let summary: Value = serde_json::from_slice(&std::fs::read(out.join(SUMMARY_FILE)).expect("a summary")).expect("json");
    assert_eq!(summary["teacher_errors"], 1);
    assert_eq!(summary["disagreements"]["person"], 1);

    // **A re-export that writes no eval side removes the one an earlier export left**: the split is the ledger's, not a leftover's.
    let scene_eval = a_map_on(true);
    let mut store = Store::new(labels.clone());
    // The engine answered its own check, so the bench scene has a record to put in eval.
    let evald = a_turn(&mut store, "turn-e", &scene_eval, NEW_BUILD, true, Some(false));
    store.append(&LedgerLine::turn(evald)).expect("a turn in eval");
    let (same, first) = export_of(&labels, "person-again", &Options::default());
    assert!(first.eval >= 1 && same.join(EVAL_FILE).is_file(), "the eval side was not written to begin with");
    // Take the eval scene's only record out of the ledger, and export again into the same folder.
    let ledger = std::fs::read_to_string(labels.join("turns.jsonl")).expect("the ledger");
    let kept: Vec<&str> = ledger.lines().filter(|line| !line.contains("turn-e")).collect();
    std::fs::write(labels.join("turns.jsonl"), kept.join("\n") + "\n").expect("the ledger without the eval turn");
    let again = turn_export::export(&labels, &same, &Options::default()).expect("the second export");
    assert_eq!(again.eval, 0);
    assert!(!same.join(EVAL_FILE).is_file(), "a stale eval.jsonl stands beside a fresh train.jsonl");
}

/// **7. An include-old-builds export keeps the old record**, and every kind of record the ledger offers reaches a file.
#[test]
fn an_old_build_can_be_asked_for_and_the_negatives_reach_the_set() {
    let labels = scratch("negatives");
    let mut store = Store::new(labels.clone());
    let map = a_map_on(false);
    let turn = a_turn(&mut store, "turn-n", &map, NEW_BUILD, false, Some(false));
    let scene = bevy_dark_factory::turn_store::scene_of(&turn);
    store.append(&LedgerLine::turn(turn)).expect("a turn");
    // One engine-made defect: the scene kept as a positive, the defective one a negative, both objective.
    let (before, after) = (store.put_image(b"scene kept", 4, 4).expect("kept"), store.put_image(b"scene broken", 4, 4).expect("kept"));
    store
        .append(&LedgerLine::perturbed(Perturbed {
            id: "perturbed-1".to_owned(),
            scene: scene.clone(),
            source_turn: "turn-n".to_owned(),
            at_unix_ms: 4_000,
            defect: "float".to_owned(),
            size: "clear".to_owned(),
            magnitude: 0.3,
            unit: "m".to_owned(),
            target: vec!["desk".to_owned()],
            tier: "objective".to_owned(),
            label: "defective".to_owned(),
            before: Shot::taken(before),
            after: Shot::taken(after),
            operations: Vec::new(),
            facts: json!({"before": {"gap_m": 0.0}, "after": {"gap_m": 0.3}}),
            bench: false,
            provenance: Provenance {
                dark_factory_version: NEW_BUILD.to_owned(),
                build_commit: format!("commit-{NEW_BUILD}"),
                render: json!({"made_by": "perturb"}),
                agent: None,
                project_hash: "the-one-project".to_owned(),
            },
        }))
        .expect("a perturbation");
    // One assess-then-fix pair: the check flagged, then cleared.
    fn pair(found: bool, shot: Shot) -> AssessedAnswer {
        AssessedAnswer {
            decision: 1,
            after_operations: 1,
            p_yes: Some(if found { 0.9 } else { 0.1 }),
            threshold: Some(0.7),
            model: Some("stand-in".to_owned()),
            frame: shot,
        }
    }
    let (flagged, cleared) = (store.put_image(b"flagged", 4, 4).expect("kept"), store.put_image(b"cleared", 4, 4).expect("kept"));
    store
        .append(&LedgerLine::assess_fix(AssessFix {
            id: "assessfix-turn-n-1".to_owned(),
            scene: scene.clone(),
            source_turn: "turn-n".to_owned(),
            check: "floating".to_owned(),
            tier: "silver".to_owned(),
            flagged: pair(true, Shot::taken(flagged)),
            cleared: pair(false, Shot::taken(cleared)),
            fixing_operations: vec!["op-1".to_owned()],
            bench: false,
        }))
        .expect("a pair");

    // **A record of a build too old to teach from**, on the same scene: the default export leaves it out and counts it.
    let old = a_turn(&mut store, "turn-old", &map, "0.6.0", false, None);
    store.append(&LedgerLine::turn(old)).expect("an old turn");
    store.append(&LedgerLine::label(label("turn-old", "kept", "gold", 140))).expect("a label");

    let (out, exported) = export_of(&labels, "negatives-out", &Options::default());
    let records: Vec<Value> = all_records(&out);
    let positive = records
        .iter()
        .find(|record| {
            record["meta"]["questions"]["check-floating"]["source"] == "perturbation"
                && probabilities(record, "check-floating") == Some(json!({"true": 0.0, "false": 1.0}))
        })
        .unwrap_or_else(|| panic!("the kept scene is not a positive: {records:#?}"));
    assert_eq!(positive["meta"]["questions"]["check-floating"]["tier"], "objective", "{positive:#?}");
    let negative = records
        .iter()
        .find(|record| {
            record["meta"]["questions"]["check-floating"]["source"] == "perturbation"
                && probabilities(record, "check-floating") == Some(json!({"true": 1.0, "false": 0.0}))
        })
        .unwrap_or_else(|| panic!("the defective scene is not a negative: {records:#?}"));
    assert_eq!(probabilities(negative, "turn-kept"), Some(json!({"true": 0.0, "false": 1.0})), "the defective scene is not one the person keeps: {negative:#?}");
    // **The pair's two frames answer the check, silver, one no and one yes.**
    let paired: Vec<&Value> =
        records.iter().filter(|record| record["meta"]["questions"]["check-floating"]["tier"] == "silver" && record["meta"]["source"] == "assess_fix").collect();
    let answers: BTreeSet<bool> =
        paired.iter().filter_map(|record| probabilities(record, "check-floating").map(|p| p["true"].as_f64().unwrap_or_default() >= 0.5)).collect();
    assert_eq!(answers, [true, false].into(), "the pair yielded {paired:#?}");
    // **A state with one picture parks a change question**, and says why: the perturbed record's kept scene shows the result alone and carries
    // the turn's questions, `check-out_of_area` among them.
    let lone: Vec<&Value> = records
        .iter()
        .filter(|record| record["meta"]["source"] == "perturbed" && record["assets"].as_array().map(Vec::len) == Some(1))
        .collect();
    assert!(!lone.is_empty(), "no one-picture record carries the turn's questions: {records:#?}");
    for record in lone {
        assert!(record["targets"]["check-out_of_area"]["probabilities"].is_null(), "a change question was answered from one picture: {record:#?}");
    }
    assert!(
        exported.parked.iter().any(|parked| parked.kind == "one-picture" && parked.question == "check-out_of_area"),
        "a one-picture state did not park the change question: {:#?}",
        exported.parked
    );
    assert_eq!(exported.left_out_by_kind().get("old-build"), Some(&1), "the 0.6.0 turn was not left out: {:#?}", exported.left_out);
    assert!(records.iter().all(|record| !record["meta"]["records"].as_array().is_some_and(|ids| ids.iter().any(|id| id == "turn-old"))), "an old build reached the set");

    // **The old build can be asked for**: the same ledger with the gate off keeps it, and that is one record more on the same side.
    let (out_all, all) = export_of(&labels, "negatives-all", &Options { include_old_builds: true, ..Options::default() });
    assert_eq!(all.left_out_by_kind().get("old-build"), None, "{:#?}", all.left_out);
    let kept_old: Vec<Value> =
        all_records(&out_all).into_iter().filter(|record| record["meta"]["records"].as_array().is_some_and(|ids| ids.iter().any(|id| id == "turn-old"))).collect();
    assert_eq!(kept_old.len(), 1, "the old build's turn is not one record: {kept_old:#?}");
    assert_eq!(
        all.train + all.eval,
        exported.train + exported.eval + 1,
        "including the old build did not add exactly the old turn's record: {} then {}",
        exported.train + exported.eval,
        all.train + all.eval
    );
    assert_eq!(kept_old[0]["meta"]["dark_factory_version"], "0.6.0");
}
