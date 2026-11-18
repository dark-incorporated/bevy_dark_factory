//! **A `Cmd+D` capture becomes a GitHub issue, pictures included** (#495).
//!
//! A capture today ends at four files in `~/Library/Application Support/DarkFactory/feedback/`, and
//! the only way they reach anyone is `python3 tools/loop/captures.py sync` in a checkout. The fleet
//! reads GitHub issues, so a capture that is not an issue does not exist to it: on 2026-11-15 thirty
//! captures sat `unread` in the ledger, and each hand-filing is one careful pass of reading the record,
//! cropping the pick and writing the issue.
//!
//! # One state machine, one path, and no issue without pictures
//!
//! The body is built by [`blueprint_model::feedback::issue`] — engine-free, so the format is tested
//! without a window. This module is the filing: after [`crate::report::save`] has written the four
//! files, the capture is copied into the configured checkout's `docs/loop/captures/<stem>/`, committed
//! on a branch named for the stem, pushed, **both raw URLs are verified to answer 200**, and only then
//! is `gh issue create` run. Any step failing stops the sequence and says what failed, with no issue
//! created: an issue whose pictures 404 is worse than no issue.
//!
//! # Why a job and not a frame
//!
//! `git` and `gh` are seconds long and network-bound. They run on a thread ([`Filing`] carries the
//! result), never inside a system, so a save cannot stall a frame.
//!
//! # The runner is a value
//!
//! Every process goes through [`Runner`], so a test drives the whole sequence with a stub `git`, `gh`
//! and `curl` and asserts the order and the failure rule without a network, a checkout or a GitHub
//! token. The real one is [`SystemRunner`], which spawns the programs on `PATH`.

use std::path::{Path, PathBuf};
use std::process::Command;

use bevy::prelude::*;

use blueprint_model::feedback::{self, Report};

/// **What one capture's filing asks of the world**, in the order the sequence runs it.
///
/// A test's implementation records these; the real one runs them.
pub trait Runner: Send + Sync {
    /// Copy the capture's files into `<checkout>/docs/loop/captures/<stem>/`.
    fn copy(&self, checkout: &Path, stem: &str, files: &[PathBuf]) -> Result<(), String>;
    /// Run `git` in `checkout` and answer whether it succeeded, with its error output when not.
    fn git(&self, checkout: &Path, args: &[&str]) -> Result<(), String>;
    /// Run `git push` for `branch`.
    fn push(&self, checkout: &Path, branch: &str) -> Result<(), String>;
    /// **Does this URL answer 200?** The check a picture link has to pass before the issue exists.
    fn url_answers(&self, url: &str) -> bool;
    /// `gh issue create`, answering the issue's URL.
    fn create_issue(&self, checkout: &Path, repo: &str, title: &str, body: &str) -> Result<String, String>;
}

/// **The runner the editor uses**: the programs on `PATH`.
pub struct SystemRunner;

impl Runner for SystemRunner {
    fn copy(&self, checkout: &Path, stem: &str, files: &[PathBuf]) -> Result<(), String> {
        let dest = checkout.join(feedback::issue::capture_dir(stem));
        std::fs::create_dir_all(&dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        for file in files {
            let Some(name) = file.file_name() else { continue };
            let to = dest.join(name);
            let bytes = std::fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
            blueprint_model::atomic_file::write(&to, &bytes)
                .map_err(|e| format!("{} -> {}: {e}", file.display(), to.display()))?;
        }
        Ok(())
    }

    fn git(&self, checkout: &Path, args: &[&str]) -> Result<(), String> {
        let out = Command::new("git")
            .current_dir(checkout)
            .args(args)
            .output()
            .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("git {}: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim()))
        }
    }

    fn push(&self, checkout: &Path, branch: &str) -> Result<(), String> {
        let out = Command::new("git")
            .current_dir(checkout)
            .args(["push", "origin", &format!("HEAD:refs/heads/{branch}")])
            .output()
            .map_err(|e| format!("git push: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("git push {branch}: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    }

    fn url_answers(&self, url: &str) -> bool {
        Command::new("curl")
            .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", url])
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).trim() == "200")
            .unwrap_or(false)
    }

    fn create_issue(&self, checkout: &Path, repo: &str, title: &str, body: &str) -> Result<String, String> {
        let dir = std::env::temp_dir().join(format!("dark_factory_issue_{}.md", std::process::id()));
        blueprint_model::atomic_file::write(&dir, body).map_err(|e| format!("{}: {e}", dir.display()))?;
        let out = Command::new("gh")
            .current_dir(checkout)
            .args(["issue", "create", "-R", repo, "--title", title, "--body-file"])
            .arg(&dir)
            .output()
            .map_err(|e| format!("gh issue create: {e}"))?;
        let _ = std::fs::remove_file(&dir);
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
        } else {
            Err(format!("gh issue create: {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    }
}

/// **Where a filing is up to**, as the state machine's own words, so the status line and a test read
/// the same thing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Copied,
    Committed,
    Pushed,
    Verified,
    Filed(String),
    /// A step failed; nothing was filed, and this says which step and why.
    Stopped { step: &'static str, why: String },
}

/// **File one capture as an issue**, in the order the format requires, on `branch`.
///
/// `checkout` is the repository that receives captures (the settings row names it); `files` are the
/// capture's files as they were written. `stem` names the branch and the folder.
///
/// **The order is the contract**: copy → commit → push → verify both pictures → create. A failure at
/// any step stops and answers [`Step::Stopped`], and no issue exists.
pub fn file(
    runner: &dyn Runner,
    checkout: &Path,
    repo: &str,
    branch: &str,
    report: &Report,
    files: &[PathBuf],
) -> Step {
    let stem = report.stem.as_str();
    let stop = |step: &'static str, why: String| Step::Stopped { step, why };

    if let Err(why) = runner.copy(checkout, stem, files) {
        return stop("copy", why);
    }
    if let Err(why) = runner.git(checkout, &["checkout", "-b", branch]) {
        return stop("branch", why);
    }
    // **The ledger row is the same copy the tool makes**, so `captures.py` adopts what the editor
    // filed rather than adding a second row for a stem it has not got (its own rule: files and the row
    // are each made once, and neither stands in for the other).
    if let Err(why) = runner.git(checkout, &["add", &feedback::issue::capture_dir(stem)]) {
        return stop("add", why);
    }
    let message = format!("capture: {stem}\n\nFiled from the editor (feedback {stem}).");
    if let Err(why) = runner.git(checkout, &["commit", "-m", &message]) {
        return stop("commit", why);
    }
    if let Err(why) = runner.push(checkout, branch) {
        return stop("push", why);
    }

    // **Both picture links have to answer before the issue exists.**
    for url in [
        feedback::issue::raw_url(branch, stem, &feedback::picks_picture_file(stem)),
        feedback::issue::raw_url(branch, stem, &feedback::picture_file(stem)),
    ] {
        if !runner.url_answers(&url) {
            return stop("verify", format!("{url} did not answer 200"));
        }
    }

    let title = feedback::issue::title(report);
    let body = feedback::issue::body(report);
    match runner.create_issue(checkout, repo, &title, &body) {
        Ok(url) => Step::Filed(url),
        Err(why) => stop("create", why),
    }
}

/// **What the switch says when it is on and no checkout is named.**
pub const NO_CHECKOUT: &str = "File captures as issues is on, and no Captures checkout is set: nothing was filed and the capture is saved here only.";

/// **A filing on its own thread.** `git` and `gh` are seconds long and network-bound, and a save must
/// not stall a frame: the save starts one and [`land_the_filing`] takes its answer when it arrives.
///
/// A [`std::sync::mpsc::Receiver`] is not `Sync`, so the receiver is behind a `Mutex`: the sending
/// thread only sends, and the one reader takes the lock for the length of a `try_recv`.
#[derive(Resource, Default)]
pub struct Pending {
    pub rx: Option<std::sync::Mutex<std::sync::mpsc::Receiver<Step>>>,
}

/// **The answer of the last filing**, for the status band.
#[derive(Resource, Default)]
pub struct Filed(pub Option<Step>);

/// **Take a finished filing's answer and say it.** One issue's URL, or the step that stopped it and
/// why — never silence, which is the defect a filed issue exists to avoid.
pub fn land_the_filing(mut pending: ResMut<Pending>, mut filed: ResMut<Filed>, mut state: Option<ResMut<crate::editor::EditorState>>, mut commands: Commands) {
    let Some(rx) = pending.rx.as_ref() else { return };
    let Ok(step) = rx.lock().map(|r| r.try_recv()).unwrap_or(Err(std::sync::mpsc::TryRecvError::Empty)) else { return };
    pending.rx = None;
    if let Some(state) = state.as_deref_mut() {
        match &step {
            Step::Filed(url) => state.status.note(format!("capture filed: {url}")),
            Step::Stopped { step, why } => state.status.problem(format!("the capture was not filed (at {step}): {why}")),
            _ => {}
        }
    }
    match &step {
        Step::Filed(url) => {
            commands.spawn(bevy_gooey::toast(format!("capture filed: {url}")));
        }
        Step::Stopped { step, why } => {
            commands.spawn(bevy_gooey::toast_alert(format!("the capture was not filed (at {step}): {why}")).sticky());
        }
        _ => {}
    }
    filed.0 = Some(step);
}

pub struct ReportIssuePlugin;

impl Plugin for ReportIssuePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Pending>().init_resource::<Filed>().add_systems(Update, land_the_filing);
    }
}

/// **The four files one capture wrote**, in the order `report::save` writes them.
pub struct Capture {
    pub stem: String,
    pub files: Vec<PathBuf>,
    pub report: Report,
}

/// **The files a capture is made of**, for a caller that has a record and its folder.
pub fn capture_files(dir: &Path, report: &Report) -> Vec<PathBuf> {
    let stem = report.stem.as_str();
    let mut out = vec![dir.join(&report.picture.file), dir.join(feedback::record_file(stem))];
    if let Some(picks) = report.picks_picture.as_ref() {
        out.push(dir.join(&picks.file));
    }
    if let Some(log) = report.log.as_deref() {
        out.push(dir.join(log));
    }
    out.retain(|p| p.is_file());
    out
}
