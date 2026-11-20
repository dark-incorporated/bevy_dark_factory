//! **The editor holds a GitHub connection it can name, test and read through** (#498).
//!
//! The editor files a capture as an issue (#495) through one hard-coded repository and whatever `gh`
//! happens to be signed in as. This file proves the connection that replaces that implicitness:
//!
//! - **the settings round-trip and are validated**, and the panel's words name the account and the
//!   repository rather than a URL that fails silently;
//! - **the connection check tells four outcomes apart** — no token, a bad token, a wrong repository and
//!   a writable one — driven against a fake endpoint, so no test asks GitHub;
//! - **a read answers the issues already filed**, which is the half an integration that can only create
//!   does not have;
//! - **no token is written anywhere but the person's own file**, proved by reading back what was written
//!   rather than by trusting a comment.

use std::path::PathBuf;

use bevy_dark_factory::github::{
    Answer, AnswersOff, Connection, CONNECTION_VERSION, Credentials, GhOnPath, Given, GitHubHttp, Named, TokenSource, Verdict, check,
    connection_file, load, read_issues, store,
};

/// A scratch folder for one test, named for the test and the process so a shared test process cannot
/// collide with another file's.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("github_connection_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **The four bodies GitHub answers with**, one for each verdict's turn.
fn user_body(login: &str) -> String {
    format!(r#"{{"login":"{login}","id":1,"type":"User"}}"#)
}

fn repo_body(push: bool, admin: bool) -> String {
    format!(r#"{{"full_name":"someone/project","private":false,"permissions":{{"admin":{admin},"push":{push},"pull":true}}}}"#)
}

/// **A connection as a person states it**: an account, a repository, and a token it holds.
fn stored(account: &str, repository: &str, token: &str) -> Connection {
    Connection {
        version: CONNECTION_VERSION,
        account: account.to_owned(),
        repository: repository.to_owned(),
        source: TokenSource::Stored,
        token: Some(token.to_owned()),
    }
}

/// **The settings round-trip through the one YAML writer, and every field comes back.**
///
/// This is the issue's first acceptance: the connection is a value a file holds, read back by the same
/// reader every project file goes through, so a value written today reads tomorrow.
#[test]
fn a_connection_round_trips_through_the_file_it_is_written_to() {
    let dir = scratch("round_trip");
    let path = dir.join("github.yaml");
    let written = stored("octocat", "octocat/hello-world", "ghp_secret");
    store(&path, &written).unwrap_or_else(|e| panic!("{e}"));

    let read = load(&path).unwrap_or_else(|e| panic!("{e}")).expect("a file was written, so one reads");
    assert_eq!(read, written, "every field of the connection comes back exactly");
    assert_eq!(read.account, "octocat");
    assert_eq!(read.repository, "octocat/hello-world");
    assert_eq!(read.source, TokenSource::Stored);
    assert_eq!(read.token(), Some("ghp_secret"), "a token this connection holds is the token the check uses");

    // **And a file of nothing but the version** is the default connection, which is what #495 did: a
    // person who has never stated one keeps filing captures through `gh`.
    let bare = dir.join("bare.yaml");
    std::fs::write(&bare, "version: 1\n").unwrap_or_else(|e| panic!("{e}"));
    let default = load(&bare).unwrap_or_else(|e| panic!("{e}")).expect("a file was written");
    assert_eq!(default, Connection::default(), "a file that states nothing is the default connection");
    assert_eq!(default.source, TokenSource::GhOnPath, "which is `gh` on this machine");
    assert_eq!(default.repository_or_own(), "dark-incorporated/bevy_dark_factory", "filing into the editor's own repository");
    assert_eq!(default.token(), None, "and holding no token of its own");

    let _ = std::fs::remove_dir_all(&dir);
}

/// **Every wrong field is refused by name**, so a person knows which one to fix.
#[test]
fn a_connection_that_is_wrong_says_which_field_is_wrong() {
    let mut c = stored("octocat", "octocat/hello-world", "ghp_secret");
    assert_eq!(c.problem(), None, "a complete connection is a connection");

    c.repository = "octocat".to_owned();
    let said = c.problem().expect("a repository of one word is refused");
    assert!(said.contains("owner/name"), "the refusal says the shape a repository has: {said}");

    c.repository = "octocat/hello/world".to_owned();
    assert!(c.problem().is_some(), "a repository of three parts is refused");

    c.repository = "octocat/hello-world".to_owned();
    c.account = "two words".to_owned();
    let said = c.problem().expect("an account of two words is refused");
    assert!(said.contains("one word"), "the refusal says what an account is: {said}");

    c.account = "octocat".to_owned();
    c.source = TokenSource::Stored;
    c.token = None;
    let said = c.problem().expect("a stored token that is not set is refused");
    assert!(said.contains("none is set"), "the refusal says there is no token: {said}");

    let mut old = stored("octocat", "octocat/hello-world", "ghp_secret");
    old.version = 99;
    let said = old.problem().expect("another version is refused");
    assert!(said.contains("version 99"), "the refusal names the version: {said}");

    // **And a refused value is not written at all** — neither its token nor its account.
    let dir = scratch("refused");
    let path = dir.join("github.yaml");
    let mut bad = stored("two words", "octocat/hello-world", "ghp_secret");
    bad.account = "two words".to_owned();
    let why = store(&path, &bad).expect_err("a wrong connection is refused");
    assert!(why.contains("one word"), "the write says why: {why}");
    assert!(!path.exists(), "nothing was written for a value that does not read back");
    let _ = std::fs::remove_dir_all(&dir);
}

/// **The panel's words name the account and the repository**, never a URL (the issue's acceptance).
#[test]
fn the_connection_says_its_account_and_repository_in_words() {
    let said = stored("octocat", "octocat/hello-world", "ghp_secret").describe();
    assert!(said.contains("octocat"), "the account is named: {said}");
    assert!(said.contains("octocat/hello-world"), "the repository is named: {said}");
    assert!(!said.contains("https://"), "and neither is a URL that fails silently: {said}");
    assert!(!said.contains("ghp_secret"), "the token is never in the words a person reads: {said}");

    // **A person who has connected nothing** is told what will happen, not left with a blank.
    let said = Connection::default().describe();
    assert!(said.contains("gh"), "the default says it uses `gh`: {said}");
    assert!(said.contains("dark-incorporated/bevy_dark_factory"), "and which repository: {said}");
}

/// **No token reaches a log line, a project file, or the value a body is built from.**
///
/// The issue's own acceptance is *"no token is ever written into the project file, a log, or an issue
/// body — asserted by a test that reads what was written"*, so this reads what each writer produced.
#[test]
fn the_token_is_written_nowhere_but_the_persons_own_file() {
    const TOKEN: &str = "ghp_the_one_that_must_not_escape";
    let connection = stored("octocat", "octocat/hello-world", TOKEN);
    assert!(connection.carries_a_secret(), "this connection does carry a token");

    // A log line.
    let logged = connection.log_line();
    assert!(!logged.contains(TOKEN), "the log line carries no token: {logged}");

    // A project's settings file: refused outright, and the file is not made.
    let dir = scratch("project_refusal");
    let project = dir.join("assets/blueprint/editor.yaml");
    std::fs::create_dir_all(project.parent().unwrap_or(&dir)).unwrap_or_else(|e| panic!("{e}"));
    let why = store(&project, &connection).expect_err("a token is never written into a project's settings file");
    assert!(why.contains("never written into a project"), "the refusal says why: {why}");
    let written = std::fs::read_to_string(&project).unwrap_or_default();
    assert!(written.is_empty(), "the project's file was never made: {written:?}");

    // The value everything else is built from: what an issue body and an error see.
    let published = connection.without_the_token();
    assert!(!published.carries_a_secret(), "the published value carries nothing");
    assert_eq!(published.token(), None);
    let encoded = serde_json::to_string(&published).unwrap_or_else(|e| panic!("{e}"));
    assert!(!encoded.contains(TOKEN), "the value an issue is built from has no token in it: {encoded}");

    // **And the person's own file is the one place it does go**, which is what makes the rest a
    // guarantee rather than a deletion.
    let mine = dir.join("github.yaml");
    store(&mine, &connection).unwrap_or_else(|e| panic!("{e}"));
    let text = std::fs::read_to_string(&mine).unwrap_or_else(|e| panic!("{e}"));
    assert!(text.contains(TOKEN), "the person's own file holds it, which is the one home it has");

    let _ = std::fs::remove_dir_all(&dir);
}

/// **The four outcomes are told apart, each driven against a fake endpoint.**
///
/// The issue's second acceptance, and the heart of this increment: a person acts differently on each, so
/// a check that answered one failure for all three would leave them guessing.
#[test]
fn the_check_tells_no_token_from_a_bad_one_from_a_wrong_repository_from_a_writable_one() {
    let connection = stored("octocat", "octocat/hello-world", "ghp_secret");

    // **No token**: the machine has none, so there is nothing to ask GitHub.
    let verdict = check(&connection, &AnswersOff::answering(vec![]), &Given(None));
    assert_eq!(verdict, Verdict::NoToken);
    assert!(!verdict.is_writable());
    assert!(verdict.words().contains("No GitHub token"), "the words say what is missing: {}", verdict.words());

    // **A bad token**: GitHub refuses `/user` itself.
    let refused = AnswersOff::answering(vec![Answer { status: 401, body: r#"{"message":"Bad credentials"}"#.to_owned() }]);
    let verdict = check(&connection, &refused, &Given(Some("ghp_wrong")));
    assert_eq!(verdict, Verdict::BadToken);
    assert!(!verdict.is_writable());
    assert!(verdict.words().contains("refused"), "the words say the token was refused: {}", verdict.words());

    // **A wrong repository**: the token works and the repository is not there (or cannot be seen).
    let missing = AnswersOff::answering(vec![
        Answer { status: 200, body: user_body("octocat") },
        Answer { status: 404, body: r#"{"message":"Not Found"}"#.to_owned() },
    ]);
    let verdict = check(&connection, &missing, &Given(Some("ghp_secret")));
    match &verdict {
        Verdict::WrongRepository { repository, .. } => assert_eq!(repository, "octocat/hello-world", "the verdict names the repository asked for"),
        other => panic!("a repository GitHub has no such name for is a wrong repository, not {other:?}"),
    }
    assert!(!verdict.is_writable());
    assert!(verdict.words().contains("octocat/hello-world"), "the words name it: {}", verdict.words());

    // **A token that can read and cannot file** is the same verdict and a different sentence, because the
    // repository is there and the permission is what is missing.
    let readable = AnswersOff::answering(vec![
        Answer { status: 200, body: user_body("octocat") },
        Answer { status: 200, body: repo_body(false, false) },
    ]);
    let verdict = check(&connection, &readable, &Given(Some("ghp_secret")));
    assert!(!verdict.is_writable(), "reading is not filing: {verdict:?}");
    assert!(verdict.words().contains("may not file"), "and the words say exactly that: {}", verdict.words());

    // **Writable**: a token that works, a repository it can see, and permission to file into it.
    let writable = AnswersOff::answering(vec![
        Answer { status: 200, body: user_body("octocat") },
        Answer { status: 200, body: repo_body(true, false) },
    ]);
    let verdict = check(&connection, &writable, &Given(Some("ghp_secret")));
    assert_eq!(
        verdict,
        Verdict::Writable { account: "octocat".to_owned(), repository: "octocat/hello-world".to_owned() },
        "a full permission is the one answer that means a capture can be filed"
    );
    assert!(verdict.is_writable());
    assert!(verdict.words().contains("octocat"), "the words name the account: {}", verdict.words());
}

/// **The check asks the token a connection names**, so `gh` and a stored token are not two paths.
#[test]
fn the_credentials_a_connection_names_are_the_ones_the_check_uses() {
    // A stored token: the connection's own, without asking the machine. `gh` on this machine may not be
    // installed at all in a test process, and the check must not care.
    let connection = stored("octocat", "octocat/hello-world", "ghp_secret");
    let credentials = Named { connection: &connection, machine: GhOnPath };
    assert_eq!(credentials.token().unwrap_or_else(|e| panic!("{e}")).as_deref(), Some("ghp_secret"));

    // **A stored source with no token** says why rather than falling back to the machine: a person who
    // chose their own token did not ask to act as whoever `gh` is signed in as.
    let mut empty = connection.clone();
    empty.token = None;
    let credentials = Named { connection: &empty, machine: GhOnPath };
    let why = credentials.token().expect_err("a stored source with no token has none");
    assert!(why.contains("none is set"), "and says so: {why}");
    assert_eq!(check(&empty, &AnswersOff::answering(vec![]), &credentials), Verdict::NoToken);

    // And the file the connection lives in is the person's own, not a project's.
    let path = connection_file().expect("this platform has a config directory");
    assert!(path.file_name().is_some_and(|n| n == "github.yaml"), "it is its own file: {}", path.display());
    assert!(!path.to_string_lossy().contains("assets/blueprint"), "and never a project's: {}", path.display());
}

/// **A read answers what is already filed**, which is what an integration that can only create lacks.
#[test]
fn a_read_lists_the_issues_already_filed() {
    let connection = stored("octocat", "octocat/hello-world", "ghp_secret");
    let body = r#"[
        {"number": 12, "title": "the pane is too wide at 3.0", "html_url": "https://github.com/octocat/hello-world/issues/12"},
        {"number": 9, "title": "the divider is too thick", "html_url": "https://github.com/octocat/hello-world/issues/9"}
    ]"#;
    let http = AnswersOff::answering(vec![Answer { status: 200, body: body.to_owned() }]);
    let filed = read_issues(&connection, &http, &Given(Some("ghp_secret"))).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(filed.len(), 2, "both issues are listed");
    assert_eq!(filed[0].number, 12);
    assert_eq!(filed[0].title, "the pane is too wide at 3.0");
    assert_eq!(filed[1].url, "https://github.com/octocat/hello-world/issues/9");

    // **A refused read says so** rather than answering an empty list, which would read as "nothing is filed".
    let refused = AnswersOff::answering(vec![Answer { status: 403, body: String::new() }]);
    let why = read_issues(&connection, &refused, &Given(Some("ghp_secret"))).expect_err("a 403 is a refusal");
    assert!(why.contains("403"), "the refusal names the answer: {why}");

    // **And with no token there is nothing to read with**, said as such.
    let why = read_issues(&connection, &AnswersOff::answering(vec![]), &Given(None)).expect_err("no token reads nothing");
    assert!(why.contains("no GitHub token"), "the refusal says what is missing: {why}");
}

/// **A fake endpoint is a `GitHubHttp`**, which is how every test above asks without a network: this one
/// asserts the seam itself, so a check that quietly reached `ureq` would fail here.
#[test]
fn the_seam_is_the_only_way_the_check_asks_the_world() {
    struct Counting(std::sync::atomic::AtomicUsize);
    impl GitHubHttp for Counting {
        fn get(&self, _url: &str, _token: Option<&str>) -> Result<Answer, String> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(Answer { status: 401, body: String::new() })
        }
    }
    let counting = Counting(std::sync::atomic::AtomicUsize::new(0));
    let connection = stored("octocat", "octocat/hello-world", "ghp_secret");
    let verdict = check(&connection, &counting, &Given(Some("ghp_secret")));
    assert_eq!(verdict, Verdict::BadToken);
    assert_eq!(counting.0.load(std::sync::atomic::Ordering::SeqCst), 1, "a refused token stops after the one question that proved it");
}
