//! **The Settings screen's collection readout follows the turn ledger through the second watched root, and not on a clock** (#538;
//! `settings_screen::COLLECTION_READOUT_EVERY` retired).
//!
//! The readout under "Learn from my use" (`N turns kept, X MB of Y MB used.`) used to read the ledger and sum the labels folder every two
//! seconds. It is recomputed when the ledger file's stamp moves: the labels folder is outside the project, so `OutsideFiles` follows
//! `turns.jsonl` there. **The assumed decision** (recorded on #538): the ledger's stamp is what the readout follows, so a picture written
//! without a ledger line does not move it, and the byte count can lag such a picture until the ledger moves. This test writes to the ledger
//! itself, from outside the editor, and waits real time, a counted number of frames apart, for the readout to say what an independent
//! walk of the folder says.
//! The clock is frozen and the index's sweep at its floor, so a readout that still waited for two seconds of `Time` could not pass.

use std::path::Path;
use std::time::Duration;

use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy_dark_factory::project_index::{MIN_SWEEP, OutsideFiles};
use bevy_dark_factory::settings_screen::OpenSettings;
use bevy_dark_factory::turn_store::LEDGER_FILE;

use crate::a_turn_is_recorded_for_learning::editor;
use crate::settle_support::Frames;

/// The pause between frames while the readout is waited for.
const PAUSE: Duration = Duration::from_millis(10);
/// How long a test waits for the readout to follow the ledger: frames covering 30 s at [`PAUSE`], counted rather than timed so a
/// starved host cannot run it out (#644).
const WAIT: Frames = Frames::covering(Duration::from_secs(30), PAUSE);

/// How many bytes of lines the ledger's reader skips the test appends: enough that the readout's one decimal of a megabyte changes.
const APPENDED_BYTES: usize = 400_000;

/// **Every byte under `dir`, by a plain walk**: what the readout says it used.
fn bytes_under(dir: &Path) -> u64 {
    let mut total = 0;
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        let meta = std::fs::metadata(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        total += if meta.is_dir() { bytes_under(&path) } else { meta.len() };
    }
    total
}

/// The readout's text, when the screen shows one.
fn readout(app: &mut App) -> Option<String> {
    let world = app.world_mut();
    let mut texts = world.query::<&Text>();
    texts.iter(world).map(|t| t.0.clone()).find(|t| t.contains(" used."))
}

#[test]
fn the_collection_readout_follows_the_ledger_file_with_the_clock_stopped() {
    let (mut app, labels) = editor("collection_readout_follows");
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    app.world_mut().write_message(OpenSettings);
    for _ in 0..6 {
        app.update();
    }
    let before = readout(&mut app).unwrap_or_else(|| panic!("the Settings screen shows the collection readout"));
    app.world().resource::<OutsideFiles>().set_sweep(&labels, MIN_SWEEP);

    // The ledger grows from outside the editor: lines its reader skips, so the turn count stays and the bytes move.
    let junk = "not a ledger line, written by another process\n".repeat(APPENDED_BYTES / 46);
    // The labels folder is made by the editor's first write, and nothing has written yet: a person's other process makes it.
    std::fs::create_dir_all(&labels).unwrap_or_else(|e| panic!("{e}"));
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(labels.join(LEDGER_FILE)).unwrap_or_else(|e| panic!("{e}"));
    std::io::Write::write_all(&mut file, junk.as_bytes()).unwrap_or_else(|e| panic!("{e}"));
    drop(file);
    let used = format!("{:.1} MB of", bytes_under(&labels) as f64 / 1_000_000.0);
    assert!(!before.contains(&used), "the readout did not already say {used}: {before}");

    let mut stepped = Frames(0);
    loop {
        app.update();
        let now = readout(&mut app).unwrap_or_default();
        if now.contains(&used) {
            break;
        }
        assert!(stepped < WAIT, "never happened within {WAIT:?}: the readout says {now:?}, not {used}");
        std::thread::sleep(PAUSE);
        stepped.0 += 1;
    }
}
