//! **The operation-log view reads `oplog.jsonl` when the project index says it moved, and no clock of its own** (#501, item 7:
//! `oplog_view`'s `RECHECK_SECS`, 1.0 s, is retired).
//!
//! The dispatcher appends a line to the log on every operation, so this is the busiest file any view follows. The tests say:
//!
//! - a hand edit that damages a line while the editor is open is named within the index's debounce, and the line the verdict names is
//!   the one an independent scan of the file finds;
//! - a file beside the log that changes makes the view read nothing;
//! - **what an append costs**: it is one `Modified` event for the whole file (the stamp is a length and a time), and the view reads the
//!   whole file again, once. The numbers for a log of 5,000 and of 50,000 lines are printed by this test (`--no-capture`);
//! - **an undo is never delayed by the index**: it walks the file itself, so one made at once after an operation (no frame, no debounce
//!   elapsed, the sweep set ten minutes apart) takes that operation back, while the view has not yet seen the append.

use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_dark_factory::oplog_view::OplogView;
use bevy_dark_factory::ops::{self, Applied, Ctx};
use bevy_dark_factory::project_index::{ChangeKind, ProjectFileChanged};
use serde_json::{Value, json};

use crate::index_view_support as support;

const LOG_REL: &str = "assets/blueprint/oplog.jsonl";

fn make(ctx: &mut Ctx, name: &str) -> String {
    match ops::dispatch(ctx, "map_create", &json!({"name": name, "bounds": [8.0, 4.0, 8.0]})) {
        Ok(Applied::Done { operation_id, .. }) => operation_id,
        other => panic!("the create did not apply: {other:?}"),
    }
}

fn log_path(ctx: &Ctx) -> std::path::PathBuf {
    ops::oplog_path(ctx.project.as_ref().unwrap_or_else(|| panic!("a project")))
}

fn lines(ctx: &Ctx) -> Vec<String> {
    std::fs::read_to_string(log_path(ctx)).unwrap_or_else(|e| panic!("{e}")).lines().map(str::to_owned).collect()
}

#[derive(Resource, Default)]
struct Heard(Vec<ProjectFileChanged>);

fn hear(mut read: MessageReader<ProjectFileChanged>, mut heard: ResMut<Heard>) {
    heard.0.extend(read.read().filter(|e| e.rel == LOG_REL).cloned());
}

fn reads(app: &App) -> u64 {
    app.world().resource::<OplogView>().reads()
}

#[test]
fn a_line_damaged_while_the_editor_is_open_is_named_within_the_index_debounce() {
    let (mut app, mut ctx) = support::editor("oplog_follow_edit");
    for name in ["east", "west", "north"] {
        make(&mut ctx, name);
    }
    support::until_seen(&mut app, "the log applied", |a| support::verdict(a, "oplog")["outcome"] == json!("applied"));
    support::settle_until_the_index_is_quiet(&mut app);

    let mut broken = lines(&ctx);
    let at = broken[1].find("\"at\"").unwrap_or_else(|| panic!("an entry has `at`")) + 5;
    broken[1].truncate(at);
    std::fs::write(log_path(&ctx), broken.join("\n") + "\n").unwrap_or_else(|e| panic!("{e}"));
    let took = support::until_seen(&mut app, "the damaged line named", |a| support::verdict(a, "oplog")["outcome"] == json!("refused"));
    assert!(took < support::SEEN_WITHIN, "the damage took {took:?} to be named");

    // **Ground truth is a plain scan of the file**: the first line that is not a whole JSON object.
    let on_disk = std::fs::read_to_string(log_path(&ctx)).unwrap_or_else(|e| panic!("{e}"));
    let first_bad = on_disk.lines().position(|l| serde_json::from_str::<Value>(l).is_err()).map(|i| i + 1).unwrap_or_else(|| panic!("the test cut a line"));
    let v = support::verdict(&mut app, "oplog");
    let field = v["field"].as_str().unwrap_or_default().to_owned();
    assert!(field.starts_with(&format!("line {first_bad}")), "the verdict names `{field}`, the file's first bad line is {first_bad}");
}

#[test]
fn a_file_beside_the_log_that_changes_makes_the_view_read_nothing() {
    let (mut app, mut ctx) = support::editor("oplog_follow_neighbour");
    make(&mut ctx, "east");
    support::until_seen(&mut app, "the log applied", |a| support::verdict(a, "oplog")["outcome"] == json!("applied"));
    support::a_neighbour_is_not_read(&mut app, &log_path(&ctx), reads);
}

/// **An append is one `Modified` event for the whole file, and one whole read.** The cost of that read is measured and printed.
#[test]
fn an_append_is_one_modified_event_and_one_read_of_the_whole_file() {
    let (mut app, mut ctx) = support::editor("oplog_follow_append");
    app.init_resource::<Heard>().add_systems(Update, hear);
    make(&mut ctx, "east");
    let entry: Value = serde_json::from_str(&lines(&ctx)[0]).unwrap_or_else(|e| panic!("{e}"));
    support::until_seen(&mut app, "the log applied", |a| support::verdict(a, "oplog")["outcome"] == json!("applied"));

    for count in [5_000usize, 50_000] {
        // A log of `count` entries, each its own `seq`, written whole.
        let text: String = (0..count)
            .map(|n| {
                let mut line = entry.clone();
                line["seq"] = json!(n);
                line.to_string() + "\n"
            })
            .collect();
        std::fs::write(log_path(&ctx), &text).unwrap_or_else(|e| panic!("{e}"));
        let wrote = reads(&app);
        support::until_seen(&mut app, &format!("the {count}-line log read"), |a| reads(a) > wrote);
        support::settle_until_the_index_is_quiet(&mut app);

        // **The append**: the dispatcher's own, one operation.
        let (reads_before, heard_before) = (reads(&app), app.world().resource::<Heard>().0.len());
        let len_before = std::fs::metadata(log_path(&ctx)).unwrap_or_else(|e| panic!("{e}")).len();
        make(&mut ctx, &format!("appended{count}"));
        let len_after = std::fs::metadata(log_path(&ctx)).unwrap_or_else(|e| panic!("{e}")).len();
        let appended = len_after - len_before;

        // Step frames, timing each, until the view has read; the frame that did is the cost on the UI thread.
        let start = Instant::now();
        let mut read_frame = Duration::ZERO;
        while reads(&app) == reads_before && start.elapsed() < Duration::from_secs(10) {
            let frame = Instant::now();
            app.update();
            if reads(&app) != reads_before {
                read_frame = frame.elapsed();
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        support::pump(&mut app, Duration::from_millis(400));

        let heard = app.world().resource::<Heard>().0[heard_before..].to_vec();
        assert!(!heard.is_empty() && heard.iter().all(|e| e.kind == ChangeKind::Modified), "an append is a Modified event: {heard:?}");
        assert_eq!(heard.last().and_then(|e| e.stamp).map(|s| s.len), Some(len_after), "the event carries the whole file's length, not the appended bytes: {heard:?}");
        let took = reads(&app) - reads_before;
        assert!((1..=2).contains(&took), "one append is one read of the log (two if the writer stalled past the debounce): {took}");
        eprintln!(
            "oplog append, log of {count} lines ({len_before} bytes): {appended} bytes appended, {} Modified event(s), {took} whole-file read(s), the frame that read took {read_frame:?}",
            heard.len()
        );
        assert_eq!(support::verdict(&mut app, "oplog")["outcome"], json!("applied"), "the log still reads");
    }
}

/// **An undo made at once after an operation is not delayed by the index**: it reads the log itself.
#[test]
fn an_undo_is_never_delayed_by_the_index_debounce() {
    let (mut app, mut ctx) = support::editor("oplog_follow_undo");
    support::settle_until_the_index_is_quiet(&mut app);
    // **The index cannot have noticed anything**: its sweep is ten minutes apart, and no frame is stepped between the operation and the undo.
    support::sweep_every(&app, Duration::from_secs(600));
    let before = reads(&app);
    let maps = ctx.project.as_ref().unwrap_or_else(|| panic!("a project")).maps_dir.clone();

    for name in ["east", "west"] {
        let id = make(&mut ctx, name);
        assert!(maps.join(format!("{name}.map.yaml")).exists(), "the operation made `{name}`");
        let started = Instant::now();
        ops::undo(&mut ctx, Some(&id)).unwrap_or_else(|e| panic!("the undo of the operation just made refused: {}", e.0));
        let took = started.elapsed();
        eprintln!("oplog undo of `{name}` made at once after its operation, the log's view not yet reading it: {took:?}");
        assert!(!maps.join(format!("{name}.map.yaml")).exists(), "the undo took `{name}` away");
        assert!(took < support::SEEN_WITHIN, "the undo took {took:?}");
    }
    // A frame now: the index has noticed nothing (its sweep is ten minutes apart), so the view still has not read the log, though the undo is done.
    app.update();
    assert_eq!(reads(&app), before, "the view has not read the log: the undo did not wait for it, or for the index");
}

#[test]
fn a_file_beside_a_log_that_is_not_there_makes_the_view_look_at_nothing() {
    let (mut app, ctx) = support::editor("oplog_follow_neighbour_missing");
    let beside = log_path(&ctx);
    assert!(!beside.exists(), "nothing has been logged in this project");
    support::a_neighbour_is_not_read(&mut app, &beside, reads);
}
