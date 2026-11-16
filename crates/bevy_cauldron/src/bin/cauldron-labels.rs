//! **`cauldron-labels`**: the commands that work on the turn ledger (DL-04, #463). Today one:
//!
//! ```text
//! cauldron-labels assess [--project <root>] [--limit <turns>] [--allow-remote]
//! ```
//!
//! It asks the person's chosen decision model (Local or Remote, as Settings and `settings.yaml` say; never the other one) to answer every question
//! of every turn this assessor version has not assessed, three times each, and appends the answers to the ledger as `assessment` lines. It runs in
//! its own process, so the editor's frame loop never waits for it. **Remote sends the pictures off this machine**, so it refuses unless
//! `--allow-remote` is given.

use std::process::ExitCode;

use bevy_cauldron::turn_assess::assess_ledger;
use bevy_cauldron::turn_store::Store;

const USAGE: &str = "usage: cauldron-labels assess [--project <root>] [--limit <turns>] [--allow-remote]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("assess") {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let (mut project, mut limit, mut allow_remote) = (None, None, false);
    let mut rest = args[1..].iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--project" => project = rest.next().map(std::path::PathBuf::from),
            "--limit" => limit = rest.next().and_then(|n| n.parse::<usize>().ok()),
            "--allow-remote" => allow_remote = true,
            other => {
                eprintln!("`{other}` is not an argument of assess.\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let Some(dir) = bevy_grimoire_core::user_dirs::labels_dir() else {
        eprintln!("this platform has no data directory, so there is no ledger to assess");
        return ExitCode::FAILURE;
    };
    let root = project.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
    let source = bevy_cauldron::decision_model::Source::process();
    let transport = source.asking(&root);
    let mut store = Store::new(dir);
    match assess_ledger(&mut store, transport.as_ref(), allow_remote, limit) {
        Ok(batch) => {
            println!("assessed {} turn(s); {} had been assessed by this version already", batch.assessed, batch.already);
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("cauldron-labels: {why}");
            ExitCode::FAILURE
        }
    }
}
