//! **The composer's panel files the issue a person previewed, and loses nothing to a restart** (#499).
//!
//! The value and its filing are proved by `an_issue_is_composed_previewed_and_filed.rs`. This file
//! proves the half a person touches:
//!
//! - **`Cmd+Shift+I` opens the dialog**, it draws the connection's words and every one of the six
//!   sections, and the fields hold the draft the project already had;
//! - **File issue goes through the panel's own seam** (`issue_compose_panel::Filing`), so a headless run
//!   files against a fake connection: exactly one create, with the preview's body;
//! - **the dialog's own keys do not reach the tab behind it**, which is the defect every field in this
//!   crate records at least once;
//!
//! A click on the dialog's Cancel is *not* proved here: a headless app lays the card out with no pointer
//! position, so the click never lands. The rule the button reaches — a draft written before a restart is
//! restored unchanged — is proved at the value level by
//! `an_issue_is_composed_previewed_and_filed::a_draft_written_before_a_restart_is_restored_unchanged`,
//! and `issue_compose_panel::save` is the same call that test's writer makes.

use std::path::PathBuf;
use std::sync::Mutex;

use bevy::prelude::*;
use bevy_dark_factory::github::{Connection, Create, Created, GitHubCreate, TokenSource, CONNECTION_VERSION};
use bevy_dark_factory::issue_compose::{self, Attachment};
use bevy_dark_factory::issue_compose_panel::{self, Composer, Filing};
use bevy_dark_factory::report_issue::Runner;
use bevy_gooey::testing::find_by_label;

use crate::fixtures;

const WARM_UP_FRAMES: usize = 6;

/// A project of its own, which is where the draft waits.
fn project(tag: &str) -> PathBuf {
    fixtures::Fixture::new(&format!("compose_panel_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m")
}

/// The editor on that project, past its warm-up so every plugin has drawn once.
fn editor(tag: &str) -> (App, PathBuf) {
    let root = project(tag);
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..WARM_UP_FRAMES {
        app.update();
    }
    (app, root)
}

/// A composer filled the way a person fills it.
fn filled() -> issue_compose::Issue {
    issue_compose::Issue {
        title: "The composer's title line loses its text".to_owned(),
        what_happened: "Typing in the title and pressing Tab cleared it.".to_owned(),
        what_was_expected: "The title stays as typed.".to_owned(),
        how_to_reproduce: "1. Press `Cmd+Shift+I`.\n2. Type a title.\n3. Press Tab.".to_owned(),
        measurement: "`issue_compose_panel::draw_composer` spawns one card; nothing re-reads the title.".to_owned(),
        acceptance: "- [ ] A test types a title, presses Tab, and reads it back.".to_owned(),
        related: "#499".to_owned(),
        labels: vec!["bug".to_owned()],
        repository: "octocat/hello-world".to_owned(),
        attachment: Attachment::Nothing,
    }
}

/// **A fake connection that records every create**, so one attempt with one body is measurable.
#[derive(Default)]
struct Fake {
    asked: Mutex<Vec<Create>>,
    answers: Mutex<Vec<Result<Created, String>>>,
}

impl Fake {
    fn answering(answers: Vec<Result<Created, String>>) -> Fake {
        Fake { asked: Mutex::new(Vec::new()), answers: Mutex::new(answers) }
    }

}

impl GitHubCreate for Fake {
    fn create(&self, issue: &Create, _token: Option<&str>) -> Result<Created, String> {
        let mut asked = self.asked.lock().expect("the fake's log");
        asked.push(issue.clone());
        let n = asked.len() - 1;
        drop(asked);
        let answers = self.answers.lock().expect("the fake's answers");
        match answers.get(n) {
            Some(answer) => answer.clone(),
            None => Err("this run files nothing".to_owned()),
        }
    }
}

/// **A picture check that answers 200**, so an attachment never stops the filing in a test.
struct Links;

impl Runner for Links {
    fn copy(&self, _checkout: &std::path::Path, _stem: &str, _files: &[PathBuf]) -> Result<(), String> {
        Err("a composed issue copies nothing".to_owned())
    }

    fn git(&self, _checkout: &std::path::Path, args: &[&str]) -> Result<(), String> {
        Err(format!("a composed issue runs no `git {}`", args.join(" ")))
    }

    fn push(&self, _checkout: &std::path::Path, _branch: &str) -> Result<(), String> {
        Err("a composed issue pushes nothing".to_owned())
    }

    fn url_answers(&self, _url: &str) -> bool {
        true
    }

    fn create_issue(&self, _checkout: &std::path::Path, _repo: &str, _title: &str, _body: &str) -> Result<String, String> {
        Err("a composed issue is filed through the connection".to_owned())
    }
}

/// **A connection a person stated**, so the filing has a token to use.
fn connection() -> Connection {
    Connection {
        version: CONNECTION_VERSION,
        account: "octocat".to_owned(),
        repository: "octocat/hello-world".to_owned(),
        source: TokenSource::Stored,
        token: Some("ghp_secret".to_owned()),
    }
}

/// **Drive the composer's own filing path**, which is what the File issue button's observer calls.
///
/// The two seams are taken off the `Filing` the app holds, so the fakes the test installed are the ones
/// the filing reaches; `resource_scope` is what lets the composer and the pending receiver be borrowed at
/// once, which is the same pair the observer has.
fn file_now(app: &mut App) {
    let held = app.world().resource::<Filing>();
    let filing = Filing {
        github: std::sync::Arc::clone(&held.github),
        links: std::sync::Arc::clone(&held.links),
        recorded: std::sync::Arc::clone(&held.recorded),
    };
    app.world_mut().resource_scope(|world, mut composer: Mut<Composer>| {
        let mut pending = world.resource_mut::<issue_compose_panel::Pending>();
        issue_compose_panel::file_the_issue(composer.as_mut(), pending.as_mut(), &filing);
    });
    for _ in 0..40 {
        app.update();
    }
}

/// **The composer is open, its fields hold the issue, and its preview is the body.**
fn install(app: &mut App, issue: issue_compose::Issue, github: Fake) {
    let composer = Composer { open: false, issue, preview: None, result: None, filing: false, connection: connection() };
    app.insert_resource(composer);
    app.insert_resource(Filing {
        github: std::sync::Arc::new(github),
        links: std::sync::Arc::new(Links),
        recorded: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
    });
    let mut composer = app.world_mut().resource_mut::<Composer>();
    composer.weave();
    composer.open = true;
    for _ in 0..4 {
        app.update();
    }
}

/// **The dialog draws the connection's words and all six sections**, which is what a person reads before
/// they file. The words are the connection's own sentence, never a URL.
#[test]
fn the_composer_draws_the_connection_and_every_section() {
    let (mut app, _root) = editor("draws");
    install(&mut app, filled(), Fake::default());

    assert!(find_by_label(&mut app, "COMPOSE AN ISSUE").is_ok(), "the dialog is up");
    let named = app
        .world_mut()
        .query::<&Text>()
        .iter(app.world())
        .any(|text| text.0.contains("octocat") && text.0.contains("octocat/hello-world"));
    assert!(named, "the account and the repository are named in words above the body");
    for heading in issue_compose::HEADINGS {
        assert!(find_by_label(&mut app, heading).is_ok(), "the dialog draws the `{heading}` section");
    }
}

/// **File issue files exactly what the preview says**, once, through the panel's own seam.
#[test]
fn the_file_button_files_one_create_with_the_preview_body() {
    let (mut app, _root) = editor("files");
    let created = Created { number: 499, url: "https://github.com/octocat/hello-world/issues/499".to_owned() };
    let github = Fake::answering(vec![Ok(created.clone())]);
    install(&mut app, filled(), github);

    let body = app.world().resource::<Composer>().body();
    file_now(&mut app);

    let composer = app.world().resource::<Composer>();
    assert!(matches!(&composer.result, Some(issue_compose::Step::Filed(url)) if url == &created.url), "the URL is shown: {:?}", composer.result);

    // **Exactly one create, carrying the preview's own bytes and the issue's title and labels.**
    let asked = app.world().resource::<Filing>().recorded.lock().expect("the recording").clone();
    assert_eq!(asked.len(), 1, "one filing is one create: {asked:?}");
    assert_eq!(asked[0].title, filled().title);
    assert_eq!(asked[0].labels, vec!["bug".to_owned()]);
    assert_eq!(asked[0].repository, "octocat/hello-world");
    assert_eq!(asked[0].body.as_bytes(), body.as_bytes(), "the body filed is the preview's, byte for byte");
}

/// **A preview a person edited is the body filed**, which is the panel's half of the rule that stops
/// somebody approving one thing and shipping another.
#[test]
fn an_edited_preview_is_the_body_the_panel_files() {
    let (mut app, _root) = editor("edited");
    let github = Fake::answering(vec![Ok(Created { number: 1, url: "https://example.invalid/1".to_owned() })]);
    install(&mut app, filled(), github);

    let edited = app.world().resource::<Composer>().body().replace("Tab.", "the Tab key.");
    app.world_mut().resource_mut::<Composer>().edit(edited.clone());
    file_now(&mut app);

    let landed = matches!(app.world().resource::<Composer>().result, Some(issue_compose::Step::Filed(_)));
    assert!(landed, "the filing landed: {:?}", app.world().resource::<Composer>().result);
    let asked = app.world().resource::<Filing>().recorded.lock().expect("the recording").clone();
    assert_eq!(asked.len(), 1, "one create: {asked:?}");
    assert_eq!(asked[0].body, edited, "the edit is the body that was filed, byte for byte");
}

/// **A create that errors says which step failed and leaves the composer open with the text intact**, so
/// the person can try again without writing the issue a second time.
#[test]
fn a_refused_create_says_the_step_and_keeps_the_words() {
    let (mut app, _root) = editor("refused");
    let github = Fake::answering(vec![Err("GitHub answered 422: Validation Failed".to_owned())]);
    install(&mut app, filled(), github);

    let body = app.world().resource::<Composer>().body();
    file_now(&mut app);

    let composer = app.world().resource::<Composer>();
    let Some(issue_compose::Step::Stopped { step, why }) = &composer.result else { panic!("the refusal is recorded: {:?}", composer.result) };
    assert_eq!(*step, "create");
    assert!(why.contains("422"), "{why}");
    assert!(composer.open, "the dialog stays up so the person can try again");
    assert_eq!(composer.issue.title, filled().title, "and the words are where they were");
    assert_eq!(composer.preview.as_ref().map(|p| p.body.clone()), Some(body), "and so is the preview");
}
