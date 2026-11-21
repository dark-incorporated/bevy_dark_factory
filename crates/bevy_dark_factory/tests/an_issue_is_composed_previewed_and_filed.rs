//! **An issue is composed in the editor, previewed, and filed without a capture** (#499).
//!
//! # What this file is about
//!
//! Before this, an issue reached GitHub only as a side effect of a capture (#495): press `Cmd+D`, point
//! at what is wrong, and the editor files it. That is the right shape for "here is what I saw" and the
//! wrong shape for a feature request, a design note with no screen to point at, or a body a person
//! wants to write. This is the composer: six named sections, a preview of the exact Markdown, and one
//! create through #498's connection.
//!
//! # The four things the issue's acceptance block asks for, in this file's order
//!
//! 1. **A headless test drives the composer**: fill the fields, preview, assert the Markdown carries
//!    every section in order, then file against a fake connection and assert exactly one create was
//!    attempted with that body.
//! 2. **A draft written before a restart is restored**, with its text unchanged.
//! 3. **The preview and the filed body are byte-identical** — the rule that stops a person approving
//!    one thing and shipping another.
//! 4. **The failure path**: a create that errors leaves no branch, no commit and no issue, and says
//!    which step failed.
//!
//! No test here asks GitHub: `AnswersOff` is the connection's own fake (#498), and the picture-link
//! check is `report_issue::Runner`'s, so a run that reached the network would fail on `ureq` never being
//! constructed rather than quietly passing.

use std::path::PathBuf;
use std::sync::Mutex;

use bevy_dark_factory::github::{AnswersOff, CONNECTION_VERSION, Connection, Created, Given, TokenSource};
use bevy_dark_factory::issue_compose::{self, Attachment, HEADINGS, Issue, Preview, Step};
use bevy_dark_factory::report_issue::Runner;

/// A scratch folder for one test, named for the test and the process so a shared test process cannot
/// collide with another test's.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("issue_compose_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **The connection #498 states**: an account, a repository, and a token it holds.
fn connection() -> Connection {
    Connection {
        version: CONNECTION_VERSION,
        account: "octocat".to_owned(),
        repository: "octocat/hello-world".to_owned(),
        source: TokenSource::Stored,
        token: Some("ghp_secret".to_owned()),
    }
}

/// **A composer filled in the way a person fills one**: a title, all six sections, a label, and the
/// repository it will go to.
fn filled() -> Issue {
    Issue {
        title: "The report box's note field loses its text on Cmd+J".to_owned(),
        what_happened: "Typing a note and pressing `Cmd+J` closed the box and the note was gone.\n\nThe second line says so.".to_owned(),
        what_was_expected: "The note stays as typed until the person saves it.".to_owned(),
        how_to_reproduce: "1. Press `Cmd+D`.\n2. Type a note.\n3. Press `Cmd+J`.".to_owned(),
        measurement: "`report::NoteField`'s observer writes `open.note` on every change; nothing reads it back after the box closes.".to_owned(),
        acceptance: "- [ ] A headless test types a note, presses `Cmd+J`, and reads the note back.".to_owned(),
        related: "#491 — a final change is not a save.".to_owned(),
        labels: vec!["bug".to_owned(), "ui".to_owned()],
        repository: "dark-incorporated/bevy_dark_factory".to_owned(),
        attachment: Attachment::Nothing,
    }
}

/// **The picture-link check, as a value**: `report_issue::Runner`'s own `url_answers`, plus four methods
/// a composed issue must never reach. Recording every call is what makes "no branch, no commit"
/// measurable rather than asserted by absence.
struct Links {
    log: Mutex<Vec<String>>,
    /// Whether the links answer 200.
    answers: bool,
}

impl Links {
    fn answering() -> Links {
        Links { log: Mutex::new(Vec::new()), answers: true }
    }

    fn refusing() -> Links {
        Links { log: Mutex::new(Vec::new()), answers: false }
    }

    fn calls(&self) -> Vec<String> {
        self.log.lock().expect("the fake's log").clone()
    }

    fn record(&self, what: impl Into<String>) {
        self.log.lock().expect("the fake's log").push(what.into());
    }
}

impl Runner for Links {
    fn copy(&self, _checkout: &std::path::Path, _stem: &str, _files: &[PathBuf]) -> Result<(), String> {
        self.record("copy");
        Err("a composed issue copies no files into a checkout".to_owned())
    }

    fn git(&self, _checkout: &std::path::Path, args: &[&str]) -> Result<(), String> {
        self.record(format!("git {}", args.join(" ")));
        Err("a composed issue makes no branch and no commit".to_owned())
    }

    fn push(&self, _checkout: &std::path::Path, branch: &str) -> Result<(), String> {
        self.record(format!("push {branch}"));
        Err("a composed issue pushes nothing".to_owned())
    }

    fn url_answers(&self, url: &str) -> bool {
        self.record(format!("get {url}"));
        self.answers
    }

    fn create_issue(&self, _checkout: &std::path::Path, _repo: &str, _title: &str, _body: &str) -> Result<String, String> {
        self.record("gh issue create");
        Err("a composed issue is filed through the connection, not gh".to_owned())
    }
}

/// **The issue one fake create answers with**, so a `Filed` step has a URL to name.
fn created() -> Created {
    Created { number: 499, url: "https://github.com/dark-incorporated/bevy_dark_factory/issues/499".to_owned() }
}

/// **The issue's first acceptance**: fill the fields, preview, read every section back in order, then
/// file exactly one create carrying that body.
#[test]
fn the_composer_fills_the_fields_previews_and_files_one_body() {
    let issue = filled();
    let preview = Preview::of(&connection(), &issue);

    // **Every section is there, and in the order the composer names them.**
    let positions: Vec<usize> = HEADINGS
        .iter()
        .map(|heading| {
            let wanted = format!("## {heading}");
            preview.body.find(&wanted).unwrap_or_else(|| panic!("`{wanted}` is not in the composed body:\n{}", preview.body))
        })
        .collect();
    assert!(positions.windows(2).all(|w| w[0] < w[1]), "the six sections keep their order: {positions:?}");

    // **And each section carries the words the person typed**, not a summary of them.
    for said in ["`Cmd+J` closed the box", "stays as typed until", "2. Type a note", "`report::NoteField`", "reads the note back", "#491"] {
        assert!(preview.body.contains(said), "the composed body carries `{said}`:\n{}", preview.body);
    }

    let links = Links::answering();
    let github = AnswersOff::answering(Vec::new()).creating(vec![Ok(created())]);
    let step = issue_compose::file(&preview, &Given(Some("ghp_secret")), &links, &github);
    let Step::Filed(url) = &step else { panic!("the filing landed: {step:?}") };
    assert_eq!(url, &created().url);

    // **Exactly one create was attempted, with the preview's own body and the issue's title and labels.**
    let attempted = github.created();
    assert_eq!(attempted.len(), 1, "one filing is one create: {attempted:?}");
    assert_eq!(attempted[0].title, issue.title);
    assert_eq!(attempted[0].body, preview.body);
    assert_eq!(attempted[0].labels, vec!["bug".to_owned(), "ui".to_owned()]);

    // **Nothing was committed, because a composed issue has no files to commit.**
    assert!(links.calls().is_empty(), "an issue with nothing attached asks the checkout for nothing: {:?}", links.calls());
}

/// **The issue's second acceptance**: a draft written before a restart comes back with its text
/// unchanged, so a half-written issue is not lost to a restart.
#[test]
fn a_draft_written_before_a_restart_is_restored_unchanged() {
    let dir = scratch("draft");
    let path = issue_compose::draft_file(&dir);
    let drafted = filled();
    issue_compose::save_draft(&path, &drafted).unwrap_or_else(|e| panic!("{e}"));

    // **A new process reads the file, not the value in memory** — `load_draft` is the only reader.
    let restored = issue_compose::load_draft(&path).unwrap_or_else(|e| panic!("{e}")).unwrap_or_else(|| panic!("a draft was written at {}", path.display()));
    assert_eq!(restored, drafted, "the draft comes back exactly as it was taken down, every field");
    assert_eq!(restored.what_happened, drafted.what_happened, "the multi-line text is unchanged, blank line included");

    // **A folder with no draft is no draft**, not an error: the composer opens empty the first time.
    assert_eq!(issue_compose::load_draft(&issue_compose::draft_file(&scratch("no_draft"))).unwrap_or_else(|e| panic!("{e}")), None);
}

/// **The issue's third acceptance**: what the person previewed is what was filed, byte for byte —
/// including the edit they made in the preview.
#[test]
fn the_preview_and_the_filed_body_are_byte_identical() {
    let issue = filled();
    let mut preview = Preview::of(&connection(), &issue);

    // **The person edits the composed Markdown in the preview**, which is the whole point of it being
    // editable: the edit is what ships, not the composer's first draft.
    let edited = preview.body.replace("The second line says so.", "And a third line the person added.");
    assert_ne!(edited, preview.body, "the edit changes the body");
    preview.edit(edited.clone());

    let github = AnswersOff::answering(Vec::new()).creating(vec![Ok(created())]);
    let step = issue_compose::file(&preview, &Given(Some("ghp_secret")), &Links::answering(), &github);
    assert!(matches!(step, Step::Filed(_)), "the edited preview files: {step:?}");

    let attempted = github.created();
    assert_eq!(attempted.len(), 1);
    assert_eq!(attempted[0].body.as_bytes(), preview.body.as_bytes(), "the filed body is the preview's, byte for byte");
    assert!(attempted[0].body.contains("a third line the person added"), "the person's edit is what shipped:\n{}", attempted[0].body);
}

/// **The issue's fourth acceptance**: a create that errors leaves no branch, no commit and no issue, and
/// the step that failed is named.
#[test]
fn a_create_that_errors_leaves_no_branch_and_says_which_step_failed() {
    let mut issue = filled();
    // **With a capture attached**, so the picture-link step is in play and the sequence really does
    // have something it could have committed.
    issue.attachment = Attachment::Capture { stem: "feedback-1794921159460".to_owned(), branch: "main".to_owned() };
    let preview = Preview::of(&connection(), &issue);

    let links = Links::answering();
    let github = AnswersOff::answering(Vec::new()).creating(vec![Err("GitHub answered 422: Validation Failed".to_owned())]);
    let step = issue_compose::file(&preview, &Given(Some("ghp_secret")), &links, &github);

    let Step::Stopped { step, why } = &step else { panic!("a refused create stops the filing: {step:?}") };
    assert_eq!(*step, "create", "the step that failed is named");
    assert!(why.contains("422"), "and its message is passed on: {why}");

    // **Nothing was left half-done on a repository.** The links were checked and nothing else: no copy,
    // no branch, no commit, no push, no `gh`.
    for call in links.calls() {
        assert!(call.starts_with("get "), "a composed issue reaches no checkout but the picture check: `{call}`");
    }
    assert_eq!(github.created().len(), 1, "one attempt, and no retry that files a second issue");
}

/// **A picture link that does not answer stops the filing before the issue exists** — #495's rule, which
/// the composer inherits rather than restates: an issue whose pictures 404 is worse than no issue.
#[test]
fn a_picture_that_does_not_answer_files_nothing() {
    let mut issue = filled();
    issue.attachment = Attachment::Capture { stem: "feedback-1794921159460".to_owned(), branch: "main".to_owned() };
    let preview = Preview::of(&connection(), &issue);

    let github = AnswersOff::answering(Vec::new()).creating(vec![Ok(created())]);
    let step = issue_compose::file(&preview, &Given(Some("ghp_secret")), &Links::refusing(), &github);

    let Step::Stopped { step, why } = &step else { panic!("a picture that 404s stops the filing: {step:?}") };
    assert_eq!(*step, "verify", "the step that failed is the picture check");
    assert!(why.contains("did not answer 200"), "and it says so: {why}");
    assert!(github.created().is_empty(), "no issue was created: {:?}", github.created());
}

/// **The preview names the account and the repository in words, above the Markdown** — a person should
/// never file an issue into a repository they were not told about.
#[test]
fn the_preview_names_the_account_and_the_repository_above_the_body() {
    let preview = Preview::of(&connection(), &filled());
    assert!(preview.named_above.contains("octocat"), "the account is named: {}", preview.named_above);
    assert!(preview.named_above.contains("dark-incorporated/bevy_dark_factory"), "the repository is named: {}", preview.named_above);
    assert_eq!(preview.repository, "dark-incorporated/bevy_dark_factory", "and it is the one the issue was composed for");

    // **A repository the issue does not name is the connection's**, stated once into the preview so the
    // filing uses exactly the string the person was shown.
    let mut issue = filled();
    issue.repository.clear();
    let preview = Preview::of(&connection(), &issue);
    assert_eq!(preview.repository, "octocat/hello-world", "the connection's repository is the one named and filed into");
    assert!(preview.named_above.contains("octocat/hello-world"), "{}", preview.named_above);
}

/// **Nothing but a token and a good body gets as far as a create**: a token the connection does not hold
/// and an issue with no title are both refused before anything is sent.
#[test]
fn a_draft_with_no_title_or_no_token_is_refused_before_it_is_sent() {
    let mut issue = filled();
    issue.title = "   ".to_owned();
    let preview = Preview::of(&connection(), &issue);
    let github = AnswersOff::answering(Vec::new()).creating(vec![Ok(created())]);
    let step = issue_compose::file(&preview, &Given(Some("ghp_secret")), &Links::answering(), &github);
    let Step::Stopped { step, why } = &step else { panic!("an issue with no title is refused: {step:?}") };
    assert_eq!(*step, "title", "{why}");
    assert!(github.created().is_empty(), "nothing was sent: {:?}", github.created());

    let github = AnswersOff::answering(Vec::new()).creating(vec![Ok(created())]);
    let step = issue_compose::file(&Preview::of(&connection(), &filled()), &Given(None), &Links::answering(), &github);
    let Step::Stopped { step, .. } = &step else { panic!("a filing with no token is refused: {step:?}") };
    assert_eq!(*step, "token");
    assert!(github.created().is_empty(), "nothing was sent: {:?}", github.created());
}
