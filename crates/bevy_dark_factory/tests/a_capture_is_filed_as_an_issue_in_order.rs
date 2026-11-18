//! **A capture is filed in the order the format requires, and a failed step files nothing** (#495).
//!
//! The whole sequence — copy → branch → commit → push → verify both pictures → create — is driven
//! through `report_issue::Runner`, so the order and the failure rule are proved here with no network,
//! no checkout and no GitHub token. The real runner (`SystemRunner`) is the same calls against the
//! programs on `PATH`.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use bevy_dark_factory::report_issue::{self, Runner, Step};
use blueprint_model::feedback::{
    self, Audience, Element, PathStep, Pick, Picture, Report, Subject, WindowRecord,
};

/// **A stand-in for `git`, `push`, `curl` and `gh`**, which records every call in order.
struct Stub {
    log: Mutex<Vec<String>>,
    /// Which step fails, if any: `"copy"`, `"push"`, `"verify"` or `"create"`.
    fails: Option<&'static str>,
}

impl Stub {
    fn new() -> Stub {
        Stub { log: Mutex::new(Vec::new()), fails: None }
    }

    fn failing(step: &'static str) -> Stub {
        Stub { log: Mutex::new(Vec::new()), fails: Some(step) }
    }

    fn calls(&self) -> Vec<String> {
        self.log.lock().expect("the stub's log").clone()
    }

    fn record(&self, what: impl Into<String>) {
        self.log.lock().expect("the stub's log").push(what.into());
    }
}

impl Runner for Stub {
    fn copy(&self, _checkout: &Path, stem: &str, _files: &[PathBuf]) -> Result<(), String> {
        self.record("copy");
        if self.fails == Some("copy") {
            return Err("the disk is full".to_owned());
        }
        self.record(format!("copied {stem}"));
        Ok(())
    }

    fn git(&self, _checkout: &Path, args: &[&str]) -> Result<(), String> {
        self.record(format!("git {}", args.join(" ")));
        Ok(())
    }

    fn push(&self, _checkout: &Path, branch: &str) -> Result<(), String> {
        self.record(format!("push {branch}"));
        if self.fails == Some("push") {
            return Err("the remote refused: no permissions".to_owned());
        }
        Ok(())
    }

    fn url_answers(&self, url: &str) -> bool {
        self.record(format!("get {url}"));
        self.fails != Some("verify")
    }

    fn create_issue(&self, _checkout: &Path, repo: &str, title: &str, _body: &str) -> Result<String, String> {
        self.record(format!("gh issue create -R {repo} --title {title}"));
        if self.fails == Some("create") {
            return Err("gh: not logged in".to_owned());
        }
        Ok(format!("https://github.com/{repo}/issues/495"))
    }
}

/// **A record as the editor writes one.**
fn report() -> Report {
    Report {
        version: feedback::FEEDBACK_VERSION,
        stem: "feedback-1794921159460".to_owned(),
        taken_at_ms: 1_794_921_159_460,
        audience: Audience::Developers,
        app_version: "0.7.0".to_owned(),
        screen: "editor".to_owned(),
        project: Some("/p/lavender_hills".to_owned()),
        map: Some("liches_lair".to_owned()),
        tool: Some("Place".to_owned()),
        window: WindowRecord { physical: [2560, 1406], logical: [1280.0, 703.0], scale_factor: 2.0, ui_scale: 2.4 },
        picks: vec![Pick {
            subject: Subject::Element(Element {
                entity: 1,
                rect_px: [0.0, 0.0, 10.0, 10.0],
                path: vec![PathStep { name: Some("terminal".to_owned()), role: None, control: None, text: None }],
            }),
            at: [5.0, 5.0],
        }],
        relations: Vec::new(),
        note: "The edge is cut.".to_owned(),
        picture: Picture { file: feedback::picture_file("feedback-1794921159460"), size: [2560, 1406] },
        picks_picture: Some(Picture { file: feedback::picks_picture_file("feedback-1794921159460"), size: [2560, 1406] }),
        log: Some(feedback::log_file("feedback-1794921159460")),
    }
}

/// **The order is copy → commit → push → verify both pictures → create.**
#[test]
fn a_capture_is_filed_in_the_order_the_format_requires() {
    let stub = Stub::new();
    let repo = report();
    let step = report_issue::file(&stub, Path::new("/checkout"), feedback::issue::REPO, "capture/feedback-1794921159460", &repo, &[]);
    let Step::Filed(url) = &step else { panic!("the filing landed: {step:?}") };
    assert!(url.contains("/issues/"), "{url}");

    let calls = stub.calls();
    let at = |needle: &str| calls.iter().position(|c| c.contains(needle)).unwrap_or_else(|| panic!("`{needle}` was never called: {calls:?}"));
    assert!(at("copy") < at("git checkout -b"), "{calls:?}");
    assert!(at("git checkout -b") < at("git commit"), "{calls:?}");
    assert!(at("git commit") < at("push"), "{calls:?}");
    assert!(at("push") < at("get https://raw.githubusercontent.com/"), "{calls:?}");
    assert!(at("_picks.png") < at("gh issue create"), "{calls:?}");
    // Both pictures, not one.
    assert_eq!(calls.iter().filter(|c| c.starts_with("get ")).count(), 2, "{calls:?}");
    assert_eq!(calls.iter().filter(|c| c.starts_with("gh issue create")).count(), 1, "{calls:?}");
}

/// **A push that fails creates no issue**, and the answer names the step and the reason.
#[test]
fn a_push_failure_files_no_issue() {
    let stub = Stub::failing("push");
    let step = report_issue::file(&stub, Path::new("/checkout"), feedback::issue::REPO, "capture/x", &report(), &[]);
    let Step::Stopped { step: which, why } = &step else { panic!("nothing was filed: {step:?}") };
    assert_eq!(*which, "push", "{why}");
    assert!(why.contains("permissions"), "the reason is the push's: {why}");
    let calls = stub.calls();
    assert!(!calls.iter().any(|c| c.starts_with("gh issue create")), "no issue was created: {calls:?}");
    assert!(!calls.iter().any(|c| c.starts_with("get ")), "and the pictures were not checked: {calls:?}");
}

/// **A picture that does not answer 200 creates no issue**, which is the one rule the format says
/// cannot break.
#[test]
fn a_picture_that_404s_files_no_issue() {
    let stub = Stub::failing("verify");
    let step = report_issue::file(&stub, Path::new("/checkout"), feedback::issue::REPO, "capture/x", &report(), &[]);
    let Step::Stopped { step: which, why } = &step else { panic!("nothing was filed: {step:?}") };
    assert_eq!(*which, "verify", "{why}");
    assert!(why.contains("200"), "the reason is the URL's: {why}");
    assert!(!stub.calls().iter().any(|c| c.starts_with("gh issue create")), "{:?}", stub.calls());
}

/// **A copy that fails never reaches git.**
#[test]
fn a_copy_failure_never_reaches_git() {
    let stub = Stub::failing("copy");
    let step = report_issue::file(&stub, Path::new("/checkout"), feedback::issue::REPO, "capture/x", &report(), &[]);
    assert!(matches!(step, Step::Stopped { step: "copy", .. }), "{step:?}");
    assert!(!stub.calls().iter().any(|c| c.starts_with("git ")), "{:?}", stub.calls());
}
