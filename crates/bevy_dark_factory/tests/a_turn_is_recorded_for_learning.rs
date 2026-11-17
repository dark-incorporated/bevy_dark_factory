//! **Every agent turn is one record, written by the editor, with nothing for the agent to do** (DL-01, #460; series labels from use, #459).
//!
//! A turn runs from the agent's first write after its last hand-back to its `blueprint_present`. The editor keeps what the person will want to
//! judge it by: the picture before and the picture after, what was asked, the operations and the facts, in the person's own data folder.
//!
//! # What is proved
//!
//! 1. **A turn over BRP is one record**: two placements and a `blueprint/present` leave one line holding both pictures (different SHA-256),
//!    the two operation ids, the specified lines, the pieces the operations touched and the build's commit.
//! 2. **The after picture waits for the follow camera**: not while it is moving, and not until nothing on screen has changed for
//!    [`SETTLE_FRAMES`] frames.
//! 3. **Off is off**: the same turn writes nothing and asks for no picture (counted).
//! 4. **The file is honest**: a line that does not read is reported by its number, a line of another version is not read, a disk budget stops
//!    collection and removes nothing, and a repeated picture is stored once.
//!
//! The editor here has no window and so draws nothing: the test plays the renderer, delivering a picture for each ticket the ledger hands out,
//! which is the same door the running editor's readback uses.

use std::path::{Path, PathBuf};
use std::time::Duration;

use bevy::prelude::*;
use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
use bevy::time::TimeUpdateStrategy;

use bevy_dark_factory::follow::Following;
use bevy_dark_factory::harness;
use bevy_dark_factory::turn_ledger::{BUILD_COMMIT, LabelsDir, SETTLE_FRAMES, ShotImage, TurnLedger};
use bevy_dark_factory::turn_store::{self, DEFAULT_BUDGET_BYTES, FORMAT, LEDGER_FILE, Line, Shot, Store};
use bevy_dark_factory::view::Rig;

use crate::fixtures;

/// One frame of the test clock, seconds: the follow's ease and dwell are measured in seconds, and a headless frame is about a millisecond.
const FRAME: f32 = 1.0 / 60.0;

/// **The most frames a turn is given to reach its record**: the follow's ease and dwell for two changes, then the settle.
pub(crate) const FRAMES_AT_MOST: usize = 900;

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("blueprint_turns_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn room(tag: &str) -> PathBuf {
    fixtures::Fixture::new(&format!("turns_{tag}_{}", std::process::id()))
        .side(64.0)
        .descriptor("bed", "alpha")
        .place("bed", (-28.0, -28.0))
        .build("m")
}

/// An editor with the bridge, the follow on and a folder for the ledger.
pub(crate) fn editor(tag: &str) -> (App, PathBuf) {
    let root = room(tag);
    let labels = scratch(tag);
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    // **Travel on**, which is the ordinary setting: the harness builds with motion off, and a follow that lands at once has no ease to wait for.
    app.insert_resource(bevy_gooey::MotionSettings::default());
    app.insert_resource(LabelsDir(labels.clone()));
    for _ in 0..8 {
        app.update();
    }
    (app, labels)
}

pub(crate) fn call(app: &mut App, method: &str, params: serde_json::Value) -> serde_json::Value {
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get(method).cloned() else {
        panic!("no method `{method}` is registered");
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
        .unwrap_or_else(|e| panic!("{method}: {e:?}"))
}

pub(crate) fn place(app: &mut App, at: [f32; 2]) -> String {
    let answer = call(
        app,
        "blueprint/operation_apply",
        serde_json::json!({"operation": "placement_add", "parameters": {"piece": "bed", "at": at}, "dry_run": false}),
    );
    assert_eq!(answer["outcome"], "applied", "{answer}");
    answer["operation_id"].as_str().expect("the apply names its id").to_owned()
}

/// **Play the renderer**: a picture for every ticket the ledger is waiting on, different for each, as a real frame would be.
pub(crate) fn render_what_is_asked(app: &mut App) {
    let tickets = app.world().resource::<TurnLedger>().pending_shots();
    for ticket in tickets {
        // **More than one colour**, as a drawn map is: a picture of one flat colour is the engine's `blank_frame`.
        let shade = (ticket * 53 % 251) as u8;
        let rgba: Vec<u8> = (0..4 * 4 * 4).map(|i| (i as u8).wrapping_mul(3).wrapping_add(shade)).collect();
        let image = ShotImage { width: 4, height: 4, rgba };
        assert!(app.world_mut().resource_mut::<TurnLedger>().deliver_shot(ticket, Ok(image)), "ticket {ticket} was not one the ledger asked for");
    }
}

pub(crate) fn written(app: &App) -> usize {
    app.world().resource::<TurnLedger>().written().len()
}

/// **Step frames, playing the renderer, until a record is written.** Returns the frames it took.
pub(crate) fn until_written(app: &mut App) -> usize {
    for frame in 0..FRAMES_AT_MOST {
        app.update();
        render_what_is_asked(app);
        if written(app) > 0 {
            return frame;
        }
    }
    panic!("no record was written in {FRAMES_AT_MOST} frames");
}

pub(crate) fn present(app: &mut App, ids: &[String], specified: &[&str]) -> String {
    let answer = call(
        app,
        "blueprint/present",
        serde_json::json!({
            "title": "two beds",
            "summary": "Two beds on the east side.",
            "specified": specified,
            "inferred": ["both facing north"],
            "operation_ids": ids,
        }),
    );
    answer["presentation"].as_str().expect("the present names itself").to_owned()
}

pub(crate) fn ledger_of(labels: &Path) -> turn_store::Read {
    turn_store::read_file(&labels.join(LEDGER_FILE)).unwrap_or_else(|e| panic!("{e}"))
}

/// **1. A turn over BRP is one record.**
#[test]
fn a_turn_over_brp_is_one_record_with_both_pictures() {
    let (mut app, labels) = editor("record");
    assert_eq!(app.world().resource::<TurnLedger>().shots_requested(), 0, "nothing is asked for before the agent writes");

    let first = place(&mut app, [20.0, 18.0]);
    // **The before picture is asked for as the first write arrives**, one ticket, before anything else has happened.
    assert_eq!(app.world().resource::<TurnLedger>().shots_requested(), 1, "the first write asks for the before picture");
    assert_eq!(app.world().resource::<TurnLedger>().pending_shots().len(), 1);
    assert!(app.world().resource::<TurnLedger>().turn_is_open());
    render_what_is_asked(&mut app);

    // **An agent's `blueprint_decide` call is reported to the editor over BRP**, and filed under the turn that is open.
    let said = call(
        &mut app,
        "blueprint/decide_report",
        serde_json::json!({
            "params": {"check": "floating"},
            "answer": {"check": "floating", "finding": {"check": "floating", "p_yes": 0.02, "threshold": 0.7, "found": false}},
            "model": "clef-flash",
            "endpoint": "http://127.0.0.1:8093/v1/systemone",
            "latency_ms": 412,
        }),
    );
    assert_eq!(said["recorded"], true, "{said}");

    let second = place(&mut app, [10.0, 12.0]);
    assert_eq!(
        app.world().resource::<TurnLedger>().shots_requested(),
        1,
        "a second write of the same turn asks for no second before picture"
    );

    let ids = [first.clone(), second.clone()];
    let presented = present(&mut app, &ids, &["a bed to the east", "another beside the door"]);
    assert_eq!(written(&app), 0, "the record waits for the view to settle: it is not written at the hand-back");
    until_written(&mut app);

    let read = ledger_of(&labels);
    assert!(read.damaged.is_empty(), "every line reads: {:?}", read.damaged);
    assert_eq!(read.lines.len(), 1, "one turn is one line");
    let turn = read.turns().next().expect("a turn");
    assert_eq!(read.lines[0].1.v, FORMAT);

    // Both pictures, and they are different pictures.
    let before = turn.before.image.as_ref().unwrap_or_else(|| panic!("no before picture: {:?}", turn.before));
    let after = turn.after.image.as_ref().unwrap_or_else(|| panic!("no after picture: {:?}", turn.after));
    assert_ne!(before.sha256, after.sha256, "the before and the after are one picture");
    for image in [before, after] {
        let path = labels.join("images").join(&image.sha256[..2]).join(format!("{}.png", image.sha256));
        let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(turn_store::sha256_hex(&bytes), image.sha256, "a picture is stored under the SHA-256 of its bytes");
        assert_eq!(image.bytes, bytes.len() as u64);
    }

    // The operations, by id, with the log's own lines.
    let operation_ids: Vec<&str> = turn.operations.iter().map(|o| o.operation_id.as_str()).collect();
    assert_eq!(operation_ids, [first.as_str(), second.as_str()]);
    assert!(turn.operations.iter().all(|o| o.op == "placement_add" && o.source == "agent"), "{:?}", turn.operations);
    assert_eq!(turn.operations[0].line["params"]["at"], serde_json::json!([20.0, 18.0]));

    // What was asked.
    assert_eq!(turn.request.specified, ["a bed to the east", "another beside the door"]);
    assert_eq!(turn.request.inferred, ["both facing north"]);
    assert_eq!(turn.request.hand_back.kind, "presentation");
    assert_eq!(turn.request.hand_back.id, presented);

    // The build.
    assert_eq!(turn.provenance.build_commit, BUILD_COMMIT);
    assert!(
        BUILD_COMMIT.len() == 40 && BUILD_COMMIT.chars().all(|c| c.is_ascii_hexdigit()),
        "the build was not stamped with a commit: `{BUILD_COMMIT}`"
    );
    assert_eq!(turn.provenance.dark_factory_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(turn.provenance.project_hash.len(), 64, "the project is a hash, never a path");
    assert!(!turn.bench);

    // The facts: the pieces each operation touched, with names and positions.
    let pieces = &turn.facts["operations"][0]["pieces"];
    assert_eq!(pieces[0]["descriptor"], "bed", "{}", turn.facts);
    assert_eq!(pieces[0]["at"], serde_json::json!([20.0, 18.0]));
    assert_eq!(turn.facts["map"]["placements"], 3, "the standing bed and the two the agent placed");
    assert!(turn.facts["camera"]["height"].is_number(), "{}", turn.facts);
    assert!(turn.facts["checks"].is_object(), "{}", turn.facts);

    // The decision the agent's server reported.
    assert_eq!(turn.decisions.len(), 1);
    assert_eq!(turn.decisions[0]["model"], "clef-flash");
    assert_eq!(turn.decisions[0]["latency_ms"], 412);

    // Two pictures were asked for in all, and nothing is left waiting.
    assert_eq!(app.world().resource::<TurnLedger>().shots_requested(), 2);
    assert!(!app.world().resource::<TurnLedger>().turn_is_open());
    assert_eq!(app.world().resource::<TurnLedger>().waiting_to_finish(), 0);
}

/// **A decision reported with no turn open belongs to no record**, and says so.
#[test]
fn a_decision_with_no_turn_open_is_not_recorded() {
    let (mut app, _labels) = editor("nodecision");
    let said = call(&mut app, "blueprint/decide_report", serde_json::json!({"params": {"check": "floating"}, "model": "clef-flash"}));
    assert_eq!(said["recorded"], false, "{said}");
}

/// **2. The after picture waits for the follow camera**, and for the view to stop changing.
#[test]
fn the_after_picture_waits_for_the_follow_camera() {
    let (mut app, _labels) = editor("settle");
    let first = place(&mut app, [20.0, 18.0]);
    render_what_is_asked(&mut app);
    let second = place(&mut app, [10.0, 12.0]);
    present(&mut app, &[first, second], &["two beds"]);

    // **While the camera is on its way, no picture is asked for.** Counted every frame, so a capture taken in passing would be caught.
    let mut moving = 0;
    let mut frames = 0;
    loop {
        assert!(frames < FRAMES_AT_MOST, "the follow never came to rest");
        let resting = app.world().resource::<Following>().at_rest();
        if resting {
            break;
        }
        moving += 1;
        assert_eq!(
            app.world().resource::<TurnLedger>().shots_requested(),
            1,
            "an after picture was asked for while the follow camera was still moving (frame {frames})"
        );
        app.update();
        frames += 1;
    }
    assert!(moving > 10, "the camera was never moving, so this proves nothing ({moving} frames)");

    // **And not while anything on screen is still changing**, even with the follow at rest: a camera the person is turning.
    for frame in 0..(SETTLE_FRAMES * 3) {
        app.world_mut().resource_mut::<Rig>().focus.x += 0.05;
        app.update();
        assert_eq!(
            app.world().resource::<TurnLedger>().shots_requested(),
            1,
            "an after picture was asked for while the camera was still turning (frame {frame})"
        );
    }

    // **Once it has stopped, the picture is asked for after the view has been still for the named number of frames, and not before.**
    let mut still = 0;
    loop {
        assert!(still < SETTLE_FRAMES + 8, "the view stood still and no picture was asked for");
        app.update();
        still += 1;
        if app.world().resource::<TurnLedger>().shots_requested() == 2 {
            break;
        }
    }
    assert!(still >= SETTLE_FRAMES, "the picture was asked for after {still} still frames; the view must be still for {SETTLE_FRAMES}");
}

/// **3. Off is off**: the same turn writes nothing, and asks for no picture.
#[test]
fn with_collection_off_nothing_is_written_and_no_picture_is_asked_for() {
    let (mut app, labels) = editor("off");
    let file = std::env::temp_dir().join(format!("blueprint_turns_off_settings_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(file.parent().expect("a folder")).expect("a scratch folder");
    std::fs::write(&file, "{version: 1, collect_labels: false}").expect("a settings file");
    app.insert_resource(bevy_dark_factory::settings::UserSettingsFile(file));
    for _ in 0..400 {
        app.update();
        if !app.world().resource::<bevy_dark_factory::settings::EditorSettings>().collect_labels {
            break;
        }
    }
    assert!(!app.world().resource::<bevy_dark_factory::settings::EditorSettings>().collect_labels, "the settings file was not read");
    for _ in 0..3 {
        app.update();
    }

    let first = place(&mut app, [20.0, 18.0]);
    let second = place(&mut app, [10.0, 12.0]);
    present(&mut app, &[first, second], &["two beds"]);
    for _ in 0..FRAMES_AT_MOST {
        app.update();
    }

    let ledger = app.world().resource::<TurnLedger>();
    assert_eq!(ledger.shots_requested(), 0, "a picture was asked for with collection off");
    assert!(ledger.pending_shots().is_empty());
    assert!(ledger.written().is_empty());
    assert!(!ledger.turn_is_open(), "a turn was opened with collection off");
    assert!(ledger.folder().is_none());
    assert!(!labels.exists(), "something was written to {} with collection off", labels.display());
}

/// **A hand-back with no write before it is recorded all the same**, with a before picture that says there was none.
#[test]
fn a_hand_back_with_no_write_before_it_says_there_was_no_before_picture() {
    let (mut app, labels) = editor("nowrite");
    present(&mut app, &[], &["nothing yet"]);
    until_written(&mut app);
    let read = ledger_of(&labels);
    let turn = read.turns().next().expect("a turn");
    assert!(turn.before.image.is_none());
    assert!(turn.before.missing.as_deref().is_some_and(|why| why.contains("no write")), "{:?}", turn.before);
    assert!(turn.after.image.is_some());
    assert_eq!(app.world().resource::<TurnLedger>().shots_requested(), 1, "only the after picture was asked for");
}

/// **A picture that does not land is a record that says so**, not a record held for ever.
#[test]
fn a_picture_that_never_lands_is_recorded_as_missing() {
    let (mut app, labels) = editor("nolanding");
    let first = place(&mut app, [20.0, 18.0]);
    present(&mut app, &[first], &["a bed"]);
    // Nobody draws: no picture is delivered.
    for _ in 0..FRAMES_AT_MOST + 400 {
        app.update();
        if written(&app) > 0 {
            break;
        }
    }
    let read = ledger_of(&labels);
    assert_eq!(read.lines.len(), 1, "the record was not written when the pictures never came");
    let turn = read.turns().next().expect("a turn");
    assert!(turn.before.missing.as_deref().is_some_and(|why| why.contains("did not land")), "{:?}", turn.before);
    assert!(turn.after.missing.as_deref().is_some_and(|why| why.contains("did not land")), "{:?}", turn.after);
}

/// **A disk budget stops collection and removes nothing.**
#[test]
fn a_full_ledger_stops_collecting_and_says_so() {
    let (mut app, labels) = editor("budget");
    app.world_mut().resource_mut::<TurnLedger>().set_budget(10);
    let first = place(&mut app, [20.0, 18.0]);
    render_what_is_asked(&mut app);
    present(&mut app, &[first], &["a bed"]);
    for _ in 0..FRAMES_AT_MOST {
        app.update();
        render_what_is_asked(&mut app);
        if app.world().resource::<TurnLedger>().stopped().is_some() && app.world().resource::<TurnLedger>().waiting_to_finish() == 0 {
            break;
        }
    }
    let ledger = app.world().resource::<TurnLedger>();
    let why = ledger.stopped().expect("collection did not stop at the budget");
    assert!(why.contains("budget") && why.contains("nothing was removed"), "{why}");
    assert!(!ledger.collecting());
    assert!(ledger.written().is_empty(), "a record was written past the budget");
    assert!(!labels.join(LEDGER_FILE).exists());
}

fn sample_turn(id: &str, shade: u8) -> turn_store::Turn {
    turn_store::Turn {
        id: id.to_owned(),
        bench: false,
        started_unix_ms: 1,
        handed_back_unix_ms: 2,
        request: turn_store::Request {
            hand_back: turn_store::HandBackRef { kind: "presentation".into(), id: "present-1".into() },
            title: "t".into(),
            summary: "s".into(),
            specified: vec!["a".into()],
            inferred: Vec::new(),
            check: Vec::new(),
            note: None,
        },
        before: Shot::missing("none"),
        after: Shot::missing(format!("none {shade}")),
        operations: Vec::new(),
        facts: serde_json::Value::Null,
        decisions: Vec::new(),
        provenance: turn_store::Provenance {
            dark_factory_version: "0".into(),
            build_commit: "unknown".into(),
            render: serde_json::Value::Null,
            agent: None,
            project_hash: String::new(),
        },
        questions: Default::default(),
    }
}

/// **4. A line that does not read is named by its number, and one of another version is not read.**
#[test]
fn a_damaged_ledger_line_is_reported_by_number() {
    let good = serde_json::to_string(&Line::turn(sample_turn("turn-1", 1))).expect("a line");
    let newer = good.replacen("\"v\":1", "\"v\":2", 1);
    let text = format!("{good}\n{{\"v\":1,\"kind\":\"turn\"\n\n{newer}\n{good}\n");
    let read = turn_store::read_text(&text);
    let numbers: Vec<usize> = read.lines.iter().map(|(n, _)| *n).collect();
    assert_eq!(numbers, [1, 5], "the two good lines keep their own numbers");
    let damaged: Vec<usize> = read.damaged.iter().map(|d| d.line).collect();
    assert_eq!(damaged, [2, 4], "the broken line and the newer one are named, and the blank one is nothing");
    assert!(read.damaged[1].why.contains("version 2") && read.damaged[1].why.contains(&FORMAT.to_string()), "{}", read.damaged[1].why);
}

/// **4. The ledger is append-only, a repeated picture is stored once, and the budget is a refusal that removes nothing.**
#[test]
fn the_store_appends_keeps_a_picture_once_and_refuses_past_its_budget() {
    let dir = scratch("store");
    let mut store = Store::new(dir.clone());
    let png = vec![7u8; 64];
    let first = store.put_image(&png, 4, 4).expect("a picture is kept");
    let again = store.put_image(&png, 4, 4).expect("the same picture again");
    assert_eq!(first, again, "one picture, one name");
    let bytes_once = store.used();
    store.append(&Line::turn(sample_turn("turn-1", 1))).expect("a line");
    store.append(&Line::turn(sample_turn("turn-2", 2))).expect("a second line");
    let read = turn_store::read_file(&store.ledger_path()).expect("reads");
    assert_eq!(read.turns().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["turn-1", "turn-2"], "appended, in order, the first never rewritten");
    assert_eq!(store.usage().turns, 2);
    assert!(store.usage().bytes > bytes_once);

    let mut small = Store::new(dir.clone()).with_budget(store.used());
    let why = small.append(&Line::turn(sample_turn("turn-3", 3))).expect_err("past the budget");
    assert!(why.contains("budget") && why.contains("nothing was removed"), "{why}");
    assert_eq!(turn_store::read_file(&small.ledger_path()).expect("reads").lines.len(), 2, "nothing was written or removed past the budget");
    let other = small.put_image(&[9u8; 64], 4, 4).expect_err("a new picture past the budget");
    assert!(other.contains("budget"));
    assert!(small.put_image(&png, 4, 4).is_ok(), "a picture already kept costs nothing");
}

/// **The Settings row names the budget it is shown with**, so the row and the constant cannot disagree.
#[test]
fn the_settings_row_says_the_budget_and_collection_is_on_by_default() {
    let row = bevy_dark_factory::setting_catalogue::find("collect_labels").expect("the row is listed");
    assert_eq!(row.category, "Decisions");
    let gigabytes = DEFAULT_BUDGET_BYTES / (1024 * 1024 * 1024);
    assert!(row.description.contains(&format!("{gigabytes} GB")), "the row says a budget the constant does not have: {}", row.description);
    assert!(bevy_dark_factory::settings::EditorSettings::default().collect_labels, "collected by default (owner, #459)");
}

/// **The row's readout is live**: it says how many turns the ledger holds and how much disk they take, and it names a ledger that is not there.
#[test]
fn the_settings_row_says_what_the_ledger_holds_now() {
    let (mut app, _labels) = editor("readout");
    let said = bevy_dark_factory::turn_ledger::collection_sentence(Some(app.world().resource::<TurnLedger>()));
    assert!(said.starts_with("0 turns kept"), "{said}");
    let first = place(&mut app, [20.0, 18.0]);
    render_what_is_asked(&mut app);
    present(&mut app, &[first], &["a bed"]);
    until_written(&mut app);
    let said = bevy_dark_factory::turn_ledger::collection_sentence(Some(app.world().resource::<TurnLedger>()));
    assert!(said.starts_with("1 turn kept") && said.contains(" MB of "), "{said}");
    assert!(bevy_dark_factory::turn_ledger::collection_sentence(None).contains("No turn ledger"));
}

/// **A readback's rows are padded to 256 bytes, and the padding is read as padding** (review of #460): for a width that is not a multiple of 64
/// every picture was recorded missing before.
#[test]
fn a_readback_with_padded_rows_is_read_as_the_picture() {
    use bevy::render::render_resource::TextureFormat;
    use bevy_dark_factory::turn_ledger::rgba_of;
    let (width, height) = (100u32, 3u32);
    let stride = (width as usize * 4).next_multiple_of(256);
    assert_ne!(stride, width as usize * 4, "the width must need padding for this to prove anything");
    let mut padded = vec![0xEEu8; stride * height as usize];
    for y in 0..height as usize {
        for x in 0..width as usize {
            padded[y * stride + x * 4..y * stride + x * 4 + 4].copy_from_slice(&[x as u8, y as u8, 7, 255]);
        }
    }
    let whole = rgba_of(&padded, width, height, TextureFormat::Rgba8UnormSrgb, [0, 0, width, height], &[]).expect("a padded readback reads");
    assert_eq!((whole.width, whole.height, whole.rgba.len()), (100, 3, 100 * 3 * 4));
    assert_eq!(&whole.rgba[(2 * 100 + 5) * 4..(2 * 100 + 5) * 4 + 4], &[5, 2, 7, 255], "a pixel is where the padding says, not where width x 4 says");
    assert!(!whole.rgba.contains(&0xEE), "padding bytes were read as pixels");

    // An unpadded buffer is read as it is.
    let tight: Vec<u8> = (0..16 * 2 * 4).map(|i| i as u8).collect();
    assert_eq!(rgba_of(&tight, 16, 2, TextureFormat::Rgba8Unorm, [0, 0, 16, 2], &[]).expect("reads").rgba, tight);
    // The wrong size is refused, not guessed at.
    assert!(rgba_of(&padded[..padded.len() - 1], width, height, TextureFormat::Rgba8Unorm, [0, 0, width, height], &[]).is_err());

    // **The picture is the map's viewport, cut out**: a crop inside the surface, and one outside it is refused.
    let cut = rgba_of(&padded, width, height, TextureFormat::Rgba8UnormSrgb, [10, 1, 20, 2], &[]).expect("a crop");
    assert_eq!((cut.width, cut.height), (20, 2));
    assert_eq!(&cut.rgba[0..4], &[10, 1, 7, 255]);
    assert!(rgba_of(&padded, width, height, TextureFormat::Rgba8Unorm, [90, 0, 20, 3], &[]).is_err(), "a viewport past the surface");

    // **A BGRA surface is turned into RGBA**, and a redacted rectangle (the terminal pane) is black.
    let bgra = rgba_of(&padded, width, height, TextureFormat::Bgra8UnormSrgb, [0, 0, width, height], &[[0, 0, 50, 2]]).expect("reads");
    assert_eq!(&bgra.rgba[(2 * 100 + 60) * 4..(2 * 100 + 60) * 4 + 4], &[7, 2, 60, 255], "blue and red swap");
    assert_eq!(&bgra.rgba[(100 + 10) * 4..(100 + 10) * 4 + 4], &[0, 0, 0, 255], "inside the redacted rectangle");
    assert_eq!(&bgra.rgba[(2 * 100 + 10) * 4..(2 * 100 + 10) * 4 + 4], &[7, 2, 10, 255], "below it is untouched");
}

/// **What the editor keeps of the decisions is bounded** (review of #460): a report that is too big is refused, and a turn holds a fixed number.
#[test]
fn a_turns_decisions_are_bounded() {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    use bevy_dark_factory::turn_ledger::{MAX_DECISIONS_PER_TURN, MAX_DECISION_BYTES};
    let (mut app, _labels) = editor("bounded");
    place(&mut app, [20.0, 18.0]);
    render_what_is_asked(&mut app);
    let Some(RemoteMethodSystemId::Instant(id)) = app.world().resource::<RemoteMethods>().get("blueprint/decide_report").cloned() else { panic!("the method") };
    let big = serde_json::json!({"params": {"facts": "x".repeat(MAX_DECISION_BYTES)}});
    let refused = app.world_mut().run_system_with(id, Some(big)).expect("runs");
    assert!(refused.is_err(), "an oversize report was taken");
    for n in 0..MAX_DECISIONS_PER_TURN {
        let said = call(&mut app, "blueprint/decide_report", serde_json::json!({"params": {"check": "floating"}, "n": n}));
        assert_eq!(said["recorded"], true, "decision {n}");
    }
    let over = call(&mut app, "blueprint/decide_report", serde_json::json!({"params": {"check": "floating"}}));
    assert_eq!(over["recorded"], false, "a turn kept more than {MAX_DECISIONS_PER_TURN} decisions: {over}");
}

/// **A ledger that stopped says how to resume** (review of #460).
#[test]
fn the_stop_sentence_says_how_to_resume() {
    let dir = scratch("resume");
    let mut store = Store::new(dir).with_budget(1);
    let why = store.append(&Line::turn(sample_turn("turn-1", 1))).expect_err("past the budget");
    assert!(why.contains("off and on again in Settings"), "{why}");
}

/// **Only the Map and the World are pictured** (review of #460): the viewport of the Files workspace holds a code editor, which can hold a secret.
///
/// The headless editor has no renderer, so the surface the readback reads is a small image of its own here: the refusal is made before anything is read back.
#[test]
fn a_hand_back_while_the_editor_is_on_files_records_no_picture() {
    use bevy_dark_factory::workspace::Workspace;
    let (mut app, labels) = editor("files");
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(Image::new_fill(
        bevy::render::render_resource::Extent3d { width: 64, height: 64, depth_or_array_layers: 1 },
        bevy::render::render_resource::TextureDimension::D2,
        &[0, 0, 0, 255],
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    ));
    app.insert_resource(bevy_dark_factory::surface::Surface { image });
    *app.world_mut().resource_mut::<Workspace>() = Workspace::Files;
    let first = place(&mut app, [20.0, 18.0]);
    present(&mut app, &[first], &["a bed"]);
    // The follow waits for the person on the Map, so the view settles only when the ledger gives up waiting for it: the before picture was refused long ago.
    for _ in 0..bevy_dark_factory::turn_ledger::SETTLE_GIVE_UP_FRAMES + 200 {
        *app.world_mut().resource_mut::<Workspace>() = Workspace::Files;
        app.update();
        if written(&app) > 0 {
            break;
        }
    }
    let read = ledger_of(&labels);
    let turn = read.turns().next().expect("the hand-back is recorded all the same");
    for shot in [&turn.before, &turn.after] {
        assert!(shot.image.is_none(), "a picture of the Files workspace was kept");
    }
    let why = turn.before.missing.as_deref().unwrap_or_default();
    assert!(why.contains("Map or the World"), "the before picture's reason: {why}");
}
