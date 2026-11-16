//! **`cauldron-labels`**: the commands that work on the turn ledger. Two:
//!
//! ```text
//! cauldron-labels assess [--project <root>] [--limit <turns>] [--allow-remote]
//! cauldron-labels export --out <dir> [--labels <dir>] [--oldest <version>] [--include-old-builds] [--share <n>]
//! ```
//!
//! `assess` (DL-04, #463) asks the person's chosen decision model (Local or Remote, as Settings and `settings.yaml` say; never the other one) to
//! answer every question of every turn this assessor version has not assessed, three times each, and appends the answers to the ledger as
//! `assessment` lines. It runs in its own process, so the editor's frame loop never waits for it. **Remote sends the pictures off this machine**,
//! so it refuses unless `--allow-remote` is given.
//!
//! `export` (DL-06, #465) resolves the tiers and writes the training set as Valen's JSONL, split by scene: `<out>/train.jsonl`,
//! `<out>/eval.jsonl`, `<out>/holdout.jsonl`, the pictures under `<out>/images/` and the counts in `<out>/summary.json`. It is derived: nothing is
//! written into the labels folder, and two exports of one ledger are byte-identical.

use std::path::PathBuf;
use std::process::ExitCode;

use bevy_cauldron::turn_assess::assess_ledger;
use bevy_cauldron::turn_export::{self, Options};
use bevy_cauldron::turn_store::Store;

const USAGE: &str = "usage:\n  cauldron-labels assess [--project <root>] [--limit <turns>] [--allow-remote]\n  cauldron-labels export --out <dir> [--labels <dir>] [--oldest <version>] [--include-old-builds] [--share <n>]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("assess") => assess(&args[1..]),
        Some("export") => export(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// **The flags of one command**: what follows each flag, and which switches are there.
struct Args {
    flags: Vec<String>,
    values: Vec<(String, String)>,
}

impl Args {
    /// **Read the flags a command takes.** A flag the command does not name is an error saying so, never ignored: a typo that silently exports
    /// every build is worse than one that stops.
    fn read(rest: &[String], takes_a_value: &[&str], switches: &[&str], command: &str) -> Result<Args, String> {
        let mut args = Args { flags: Vec::new(), values: Vec::new() };
        let mut at = 0;
        while let Some(arg) = rest.get(at) {
            if let Some(flag) = switches.iter().find(|flag| **flag == arg.as_str()) {
                args.flags.push((*flag).to_owned());
                at += 1;
            } else if let Some(flag) = takes_a_value.iter().find(|flag| **flag == arg.as_str()) {
                let Some(value) = rest.get(at + 1) else {
                    return Err(format!("{flag} needs a value, and none follows it.\n{USAGE}"));
                };
                args.values.push(((*flag).to_owned(), value.clone()));
                at += 2;
            } else {
                return Err(format!("`{arg}` is not an argument of {command}.\n{USAGE}"));
            }
        }
        Ok(args)
    }

    fn get(&self, flag: &str) -> Option<&str> {
        self.values.iter().find(|(name, _)| name == flag).map(|(_, value)| value.as_str())
    }

    fn has(&self, flag: &str) -> bool {
        self.flags.iter().any(|name| name == flag)
    }
}

/// **The labels folder**, or the reason there is none on this platform.
fn labels_dir() -> Result<PathBuf, String> {
    bevy_grimoire_core::user_dirs::labels_dir().ok_or_else(|| "this platform has no data directory, so there is no ledger".to_owned())
}

fn assess(rest: &[String]) -> ExitCode {
    let args = match Args::read(rest, &["--project", "--limit"], &["--allow-remote"], "assess") {
        Ok(args) => args,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::from(2);
        }
    };
    let project = args.get("--project").map(PathBuf::from);
    let limit = match args.get("--limit").map(str::parse::<usize>) {
        Some(Ok(limit)) => Some(limit),
        Some(Err(_)) => {
            eprintln!("--limit is a number of turns.\n{USAGE}");
            return ExitCode::from(2);
        }
        None => None,
    };
    let allow_remote = args.has("--allow-remote");
    let dir = match labels_dir() {
        Ok(dir) => dir,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::FAILURE;
        }
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

fn export(rest: &[String]) -> ExitCode {
    let flags = ["--out", "--labels", "--oldest", "--share"];
    let args = match Args::read(rest, &flags, &["--include-old-builds"], "export") {
        Ok(args) => args,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::from(2);
        }
    };
    let Some(out) = args.get("--out").map(PathBuf::from) else {
        eprintln!("export needs --out, the folder to write the set into.\n{USAGE}");
        return ExitCode::from(2);
    };
    let labels = args.get("--labels").map(PathBuf::from);
    let mut options = Options { include_old_builds: args.has("--include-old-builds"), ..Options::default() };
    if let Some(oldest) = args.get("--oldest") {
        options.oldest_build = oldest.to_owned();
    }
    if let Some(share) = args.get("--share") {
        match share.parse::<u32>() {
            Ok(share) if share > 0 => options.eval_share = share,
            _ => {
                eprintln!("--share is one scene in how many, so it is a positive number.\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    let labels = match labels.map_or_else(labels_dir, Ok) {
        Ok(dir) => dir,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::FAILURE;
        }
    };
    match turn_export::export(&labels, &out, &options) {
        Ok(exported) => {
            println!(
                "wrote {} train, {} eval and {} holdout record(s) with {} picture(s) to {}; parked {} question(s), {} disagreement(s), left out {} record(s)",
                exported.train,
                exported.eval,
                exported.holdout,
                exported.images,
                out.display(),
                exported.parked.len(),
                exported.disagreements.len(),
                exported.left_out.len()
            );
            for (kind, count) in exported.parked_by_kind() {
                println!("  parked {count}: {kind}");
            }
            for (kind, count) in exported.left_out_by_kind() {
                println!("  left out {count}: {kind}");
            }
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("cauldron-labels: {why}");
            ExitCode::FAILURE
        }
    }
}
