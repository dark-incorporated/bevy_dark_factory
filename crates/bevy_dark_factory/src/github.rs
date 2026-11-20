//! **The editor's GitHub connection: who it acts as, which repository it files into, and a token it
//! holds itself — or a stated reason it holds none** (#498).
//!
//! # Why a connection and not a hard-coded path
//!
//! Filing a capture (#495) reached GitHub through one hard-coded repository
//! (`blueprint_model::feedback::issue::REPO`) and whatever `gh` happened to be signed in as on the
//! machine. Nothing said which account that was, nothing let a person choose another repository, and
//! nothing could read what was already filed — so the integration could create and never see. This
//! module is the connection every later GitHub feature builds on: **an account, a repository, and
//! either a token or a stated reason there is none**.
//!
//! # The token is never in a project file, a log or an issue
//!
//! A project is a folder a person owns and shares, so a token written into `editor.yaml` would be a
//! secret handed to everyone the folder is handed to. The value therefore lives in **the person's own
//! file** (`<config_dir>/Blueprint/github.yaml`, beside `recent.yaml` and `trusted.yaml`), and
//! [`store`] refuses a path named for a project's settings. Nothing else carries it: [`Connection::describe`]
//! is what a person and the status band read, [`Connection::log_line`] is what a log gets, and neither
//! holds the token — `every_way_a_connection_is_written_says_nothing_of_the_token` asserts both.
//!
//! A person who has connected nothing is not broken: [`TokenSource::GhOnPath`] is the default and is
//! exactly what #495 did, so the capture path keeps working while this is built. The issue says so
//! itself, and it is the reason this is additive rather than a replacement.
//!
//! # The four answers a connection test tells apart
//!
//! [`check`] answers one [`Verdict`], and the four failures stay distinct because a person can act on
//! each and on none of the others:
//!
//! | answer | what the person does |
//! |---|---|
//! | [`Verdict::NoToken`] | sign in with `gh`, or paste a token |
//! | [`Verdict::BadToken`] | the token was refused: it is expired, revoked, or mistyped |
//! | [`Verdict::WrongRepository`] | the token works and cannot see — or cannot file into — this repository |
//! | [`Verdict::Writable`] | nothing: this is the answer that means "a capture can be filed" |
//!
//! **Read access is the same request as write access** (the issue's requirement 4): GitHub answers both
//! in the repository object's `permissions.push`, so [`read_issues`] and [`check`] share one reader of
//! the repository and differ only in which field they take from the answer.
//!
//! # Both seams are the resource shape the codebase already uses
//!
//! [`crate::wish_view::WishSearchOff`] is how a test — and the headless harness, which has no network —
//! answers a GitHub question without a network. [`GitHubHttp`] is that shape for this module: a test
//! hands in the four answers it wants and asserts the verdict, and no suite asks GitHub. `ureq` reaches
//! the network only through [`UreqHttp`], which is what the editor's binary uses.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// **How long GitHub is given to answer**, in seconds. A connection test a person pressed must answer
/// or fail in the time a person will stand still for.
pub const TIMEOUT_S: u64 = 8;

/// **How many issues a read lists**: enough to find a duplicate, few enough to read.
pub const READ_MAX: usize = 30;

/// **The version every connection file is written at.**
pub const CONNECTION_VERSION: u32 = 1;

/// The file the connection lives in, beside `recent.yaml` and `trusted.yaml`.
const FILE: &str = "github.yaml";

const USER_URL: &str = "https://api.github.com/user";
const REPOS_URL: &str = "https://api.github.com/repos";

/// **Where the connection lives for this user**, or `None` on a platform with no config directory.
pub fn connection_file() -> Option<PathBuf> {
    blueprint_model::user_dirs::config_dir().map(|d| d.join(FILE))
}

/// **Where a token the connection uses comes from.**
///
/// Two, and only two, because the issue allows exactly two: a token the editor holds, or `gh` on the
/// machine, named as such. A third — an environment variable — would be a second, invisible way for the
/// editor to act as somebody, and the point of this issue is that the account is *stated*.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TokenSource {
    /// **`gh` on this machine, signed in as somebody**: #495's behaviour, and the default, so a person
    /// who connects nothing keeps filing captures exactly as they do today.
    #[default]
    GhOnPath,
    /// **A token the editor holds**, in the person's own file and never in a project's.
    Stored,
}

impl TokenSource {
    /// The names the panel offers, in declaration order.
    pub const NAMES: [&'static str; 2] = ["`gh` on this machine", "a token the editor holds"];

    /// This source's name, as `NAMES` spells it.
    pub fn name(self) -> &'static str {
        match self {
            TokenSource::GhOnPath => Self::NAMES[0],
            TokenSource::Stored => Self::NAMES[1],
        }
    }

    /// The index of this source among [`TokenSource::NAMES`], so a control can show it.
    pub fn index(self) -> usize {
        match self {
            TokenSource::GhOnPath => 0,
            TokenSource::Stored => 1,
        }
    }

    /// The source at `index`, or `None` past the end.
    pub fn at(index: usize) -> Option<TokenSource> {
        match index {
            0 => Some(TokenSource::GhOnPath),
            1 => Some(TokenSource::Stored),
            _ => None,
        }
    }
}

/// **The connection as a person states it once.**
///
/// Every field but `version` has a default, so a file written before a key existed reads as "using `gh`
/// on this machine, filing into the editor's own repository" — which is what #495 did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Connection {
    /// The file format's version, so a file of another version is refused by name rather than read as
    /// this one's.
    pub version: u32,
    /// **The account the editor acts as**, stated so a person can read it: `octocat`, or empty for
    /// "whoever `gh` says".
    pub account: String,
    /// **The repository it files into**, `owner/name`. Empty means the editor's own.
    pub repository: String,
    /// Where the token comes from.
    pub source: TokenSource,
    /// **The token itself, when the source is [`TokenSource::Stored`].** Kept in the person's file and
    /// nowhere else; see the module note.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl Default for Connection {
    fn default() -> Self {
        Connection {
            version: CONNECTION_VERSION,
            account: String::new(),
            repository: String::new(),
            source: TokenSource::default(),
            token: None,
        }
    }
}

impl Connection {
    /// **The repository this connection files into**, or the editor's own when none is stated.
    pub fn repository_or_own(&self) -> &str {
        if self.repository.trim().is_empty() {
            blueprint_model::feedback::issue::REPO
        } else {
            self.repository.trim()
        }
    }

    /// **The token, when one is stated.** A source of [`TokenSource::GhOnPath`] has none, whatever the
    /// file holds: the two are one fact, and a token left behind by a person who went back to `gh` must
    /// not be used silently.
    pub fn token(&self) -> Option<&str> {
        match self.source {
            TokenSource::Stored => self.token.as_deref().map(str::trim).filter(|t| !t.is_empty()),
            TokenSource::GhOnPath => None,
        }
    }

    /// **Whether this value carries a secret**, which is the one question a writer asks before it
    /// writes anything: a value that does may reach the person's own file and nowhere else.
    pub fn carries_a_secret(&self) -> bool {
        self.token().is_some()
    }

    /// **What a person reads**: the account and the repository, in words, never as a URL that fails
    /// silently (the issue's own acceptance). Every part says what is missing rather than leaving a gap.
    pub fn describe(&self) -> String {
        let account = match (self.account.trim(), self.source) {
            ("", TokenSource::GhOnPath) => "whoever `gh` on this machine is signed in as",
            ("", TokenSource::Stored) => "the account this token names",
            (name, _) => name,
        };
        let token = match self.source {
            TokenSource::GhOnPath => "using `gh` on this machine",
            TokenSource::Stored if self.token().is_some() => "with a token this editor holds",
            TokenSource::Stored => "with no token yet: paste one, or use `gh` on this machine",
        };
        format!("Acting as {account}, filing into {}, {token}.", self.repository_or_own())
    }

    /// **What a log line says about this connection**, which is [`Self::describe`] and never the token:
    /// the one string a logger is given, so a log cannot grow a secret by copying a field.
    pub fn log_line(&self) -> String {
        self.describe()
    }

    /// **The value with the token taken out**, for anything that is not the person's own file — a log,
    /// a project file, an issue body, an error. This is what a writer calls instead of `clone` when it
    /// is about to write somewhere a secret must not be.
    pub fn without_the_token(&self) -> Connection {
        Connection { token: None, source: TokenSource::GhOnPath, ..self.clone() }
    }

    /// **Why this value is not a connection a person can file through**, or `None` when it is.
    ///
    /// The validation the issue asks for, and it names the field that is wrong rather than saying
    /// "invalid": a repository is `owner/name`, and an account is one word of GitHub's alphabet.
    pub fn problem(&self) -> Option<String> {
        if self.version != CONNECTION_VERSION {
            return Some(format!("the connection is version {}, and this editor reads {CONNECTION_VERSION}", self.version));
        }
        let account = self.account.trim();
        if account.chars().any(|c| c.is_whitespace() || c == '/') {
            return Some(format!("`{account}` is not an account: an account is one word, with no space and no `/`"));
        }
        let repository = self.repository.trim();
        if !repository.is_empty() {
            let mut parts = repository.split('/');
            let owner = parts.next().unwrap_or_default();
            let name = parts.next().unwrap_or_default();
            if parts.next().is_some() || owner.is_empty() || name.is_empty() {
                return Some(format!("`{repository}` is not a repository: say it as `owner/name`, like `octocat/hello`"));
            }
        }
        if self.source == TokenSource::Stored && self.token().is_none() {
            return Some("this connection uses a token it holds, and none is set: paste one, or choose `gh` on this machine".to_owned());
        }
        None
    }
}

/// **What a connection test answers**, one of four outcomes and nothing else.
///
/// The four are the issue's own list, kept apart because each is a different thing for a person to do.
/// [`Verdict::Writable`] is the only success: a connection that can read and cannot write would file
/// nothing, so "readable" is not a pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing to try with: no token stored, and no `gh` signed in.
    NoToken,
    /// A token was offered and GitHub refused it: expired, revoked, or mistyped.
    BadToken,
    /// The token works, and cannot read or file into this repository — a typo in the name, or no access.
    WrongRepository { repository: String, why: String },
    /// A token that works, a repository it can read, and permission to file into it.
    Writable { account: String, repository: String },
}

impl Verdict {
    /// **What the status band says**, in the words a person acts on. Never a URL, and never the token.
    pub fn words(&self) -> String {
        match self {
            Verdict::NoToken => "No GitHub token: sign in with `gh`, or paste a token in the connection.".to_owned(),
            Verdict::BadToken => "GitHub refused the token: it is expired, revoked, or mistyped.".to_owned(),
            Verdict::WrongRepository { repository, why } => format!("The token works and cannot reach `{repository}`: {why}"),
            Verdict::Writable { account, repository } => format!("Connected as {account}, and may file issues in {repository}."),
        }
    }

    /// **Whether this is the answer that means a capture can be filed.**
    pub fn is_writable(&self) -> bool {
        matches!(self, Verdict::Writable { .. })
    }
}

/// **One HTTP answer, as the check reads it.** The status and the body, and nothing a transport adds:
/// a test hands this in, and [`UreqHttp`] is the only place a request is made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    pub status: u16,
    pub body: String,
}

/// **The one thing the check asks the world for**: a `GET` with a token. A seam, exactly as
/// `crate::wish_view::WishSearchOff` is, so a test drives all four outcomes without a network.
pub trait GitHubHttp {
    /// `GET url` with `token` as its bearer, or with no header when there is none.
    fn get(&self, url: &str, token: Option<&str>) -> Result<Answer, String>;
}

/// **GitHub over the network**, as `crate::wish_view` and `crate::update` already reach it.
pub struct UreqHttp;

impl GitHubHttp for UreqHttp {
    fn get(&self, url: &str, token: Option<&str>) -> Result<Answer, String> {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(TIMEOUT_S)))
            // **A refusal is an answer**, not a transport error: 401 and 404 are two of the four
            // verdicts, and an `Err` for them would lose the distinction the issue asks for.
            .http_status_as_error(false)
            .build()
            .into();
        let mut request = agent.get(url).header("User-Agent", &format!("DarkFactory/{}", env!("CARGO_PKG_VERSION"))).header("Accept", "application/vnd.github+json");
        if let Some(token) = token {
            request = request.header("Authorization", &format!("Bearer {token}"));
        }
        let mut response = request.call().map_err(|e| match e {
            ureq::Error::Timeout(_) => format!("GitHub did not answer within {TIMEOUT_S} s"),
            other => format!("GitHub could not be reached: {other}"),
        })?;
        let status = response.status().as_u16();
        let body = response.body_mut().read_to_string().map_err(|e| format!("reading GitHub's answer failed: {e}"))?;
        Ok(Answer { status, body })
    }
}

/// **A run that asks GitHub nothing**, and answers every request with what it is given.
///
/// A test inserts one to prove each of the four verdicts, and the harness inserts the failing answer,
/// because a test process has no business asking the network. `crate::wish_view::WishSearchOff`'s shape:
/// the seam is a value the caller hands in, never a branch in the code that ships.
pub struct AnswersOff {
    /// One answer per request, **in the order the check asks**; the last is repeated for any beyond it.
    pub answers: Vec<Answer>,
    /// How many requests have been answered, so the next one takes the next answer.
    pub asked: std::sync::atomic::AtomicUsize,
}

impl AnswersOff {
    /// A run answering these, in this order.
    pub fn answering(answers: Vec<Answer>) -> Self {
        AnswersOff { answers, asked: std::sync::atomic::AtomicUsize::new(0) }
    }
}

impl GitHubHttp for AnswersOff {
    fn get(&self, _url: &str, _token: Option<&str>) -> Result<Answer, String> {
        let n = self.asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let answer = self.answers.get(n).or_else(|| self.answers.last());
        answer.cloned().ok_or_else(|| "this run asks GitHub nothing".to_owned())
    }
}

/// **What the check needs from the machine**: the token that will be used.
///
/// A connection of [`TokenSource::GhOnPath`] has no token of its own, so the machine is asked — `gh auth
/// token` — and the answer is what makes the account real rather than stated.
pub trait Credentials {
    /// **The token to file with**, or `None` when there is none. A source that cannot answer says why
    /// (the issue's requirement 2: a token, or a stated reason it has none).
    fn token(&self) -> Result<Option<String>, String>;
}

/// **`gh` on this machine**, the way #495 had it: `gh auth token`.
pub struct GhOnPath;

impl Credentials for GhOnPath {
    fn token(&self) -> Result<Option<String>, String> {
        let out = std::process::Command::new("gh").args(["auth", "token"]).output().map_err(|e| format!("`gh` could not be run: {e}"))?;
        if !out.status.success() {
            return Ok(None);
        }
        let token = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        Ok((!token.is_empty()).then_some(token))
    }
}

/// **A token the editor holds**, or the machine's: the credentials the connection itself names.
pub struct Named<'a> {
    pub connection: &'a Connection,
    pub machine: GhOnPath,
}

impl Credentials for Named<'_> {
    fn token(&self) -> Result<Option<String>, String> {
        match self.connection.source {
            TokenSource::Stored => match self.connection.token() {
                Some(token) => Ok(Some(token.to_owned())),
                None => Err("this connection uses a token it holds, and none is set".to_owned()),
            },
            TokenSource::GhOnPath => self.machine.token(),
        }
    }
}

/// **A token the test states**, so no outcome needs `gh` installed.
pub struct Given(pub Option<&'static str>);

impl Credentials for Given {
    fn token(&self) -> Result<Option<String>, String> {
        Ok(self.0.map(str::to_owned))
    }
}

/// **Ask GitHub whether this connection can file issues, and say which of the four answers it is.**
///
/// One request per question, in the issue's order: is there a token, does it work, which account is it,
/// may it file into this repository. A no at each step stops there, because the next question cannot be
/// answered without the one before it.
pub fn check(connection: &Connection, http: &dyn GitHubHttp, credentials: &dyn Credentials) -> Verdict {
    let token = match credentials.token() {
        Ok(Some(token)) => token,
        // **A source that cannot even say** is "no token", which is the same thing to a person: there is
        // nothing to file with.
        Ok(None) | Err(_) => return Verdict::NoToken,
    };
    let repository = connection.repository_or_own().to_owned();
    // **The account.** `/user` is the cheapest question that proves a token is a token; GitHub answers
    // 401 for a refused one, and that is the whole difference between the first two verdicts.
    let user = match http.get(USER_URL, Some(&token)) {
        Ok(answer) if answer.status == 200 => answer,
        // 401 and 403 are a refused token; anything else says GitHub could not be asked about it, which
        // a person acts on the same way.
        Ok(_) => return Verdict::BadToken,
        Err(why) => return Verdict::WrongRepository { repository, why },
    };
    let account = login_of(&user.body).or_else(|| Some(connection.account.trim()).filter(|a| !a.is_empty()).map(str::to_owned));
    let Some(account) = account else {
        // A 200 with no `login` is not a `/user` answer; treating it as a good token would be inventing one.
        return Verdict::BadToken;
    };
    // **The repository, and the permission to file into it.** `permissions.push` is GitHub's own answer
    // to "may this token write here", and it is the same request that proves the repository is there.
    let url = format!("{REPOS_URL}/{repository}");
    let answer = match http.get(&url, Some(&token)) {
        Ok(answer) => answer,
        Err(why) => return Verdict::WrongRepository { repository, why },
    };
    if answer.status == 404 {
        return Verdict::WrongRepository { repository, why: "GitHub has no such repository, or this token cannot see it".to_owned() };
    }
    if answer.status != 200 {
        return Verdict::WrongRepository { repository, why: format!("GitHub answered {}", answer.status) };
    }
    match may_push(&answer.body) {
        Some(true) => Verdict::Writable { account, repository },
        Some(false) => Verdict::WrongRepository { repository, why: "the token can read it and may not file into it".to_owned() },
        // **A body with no `permissions`** is GitHub saying nothing about the token, which is not a yes.
        None => Verdict::WrongRepository { repository, why: "GitHub said nothing about this token's permission to file".to_owned() },
    }
}

/// **Read what is already filed**, the issue's requirement 4: an integration that can only create cannot
/// avoid a duplicate or link a capture to the issue it became.
pub fn read_issues(connection: &Connection, http: &dyn GitHubHttp, credentials: &dyn Credentials) -> Result<Vec<Filed>, String> {
    let token = credentials.token()?.ok_or_else(|| "no GitHub token to read with".to_owned())?;
    let repository = connection.repository_or_own();
    let url = format!("{REPOS_URL}/{repository}/issues?state=all&per_page={READ_MAX}");
    let answer = http.get(&url, Some(&token))?;
    if answer.status != 200 {
        return Err(format!("GitHub answered {} reading {repository}", answer.status));
    }
    issues_of(&answer.body)
}

/// **One issue already filed**, which is what a read answers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filed {
    pub number: u64,
    pub title: String,
    pub url: String,
}

/// The `login` of a `/user` answer, which is the account GitHub says the token belongs to.
fn login_of(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value.get("login")?.as_str().map(str::to_owned)
}

/// **Whether the token may file into the repository**, as the repository object's own `permissions.push`.
fn may_push(body: &str) -> Option<bool> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    value.get("permissions")?.get("push")?.as_bool()
}

/// The issues a list answer carries, taken from the fields a reader acts on.
fn issues_of(body: &str) -> Result<Vec<Filed>, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|e| format!("GitHub's answer is not JSON: {e}"))?;
    let items = value.as_array().cloned().unwrap_or_default();
    Ok(items
        .iter()
        .filter_map(|item| {
            Some(Filed {
                number: item.get("number")?.as_u64()?,
                title: item.get("title")?.as_str()?.to_owned(),
                url: item.get("html_url")?.as_str()?.to_owned(),
            })
        })
        .collect())
}

/// **Read the connection**, or `None` when the person has never stated one.
pub fn load(path: &Path) -> Result<Option<Connection>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let name = path.display().to_string();
    let connection: Connection = blueprint_model::yaml::from_str(&text, &name).map_err(|p| p.line())?;
    Ok(Some(connection))
}

/// **Write the connection to the person's own file**, and refuse any path named for a project's.
///
/// **This is the only writer**, and the refusal is the guarantee the issue asks for: a token reaching
/// `editor.yaml` would be a secret handed out with the folder, so a `store` aimed there says why and
/// writes nothing at all — neither the token nor the account.
pub fn store(path: &Path, connection: &Connection) -> Result<(), String> {
    let file = path.to_string_lossy().replace('\\', "/");
    if connection.carries_a_secret() && path.file_name().is_some_and(|name| name == blueprint_model::reload_check::PROJECT_SETTINGS_FILE) {
        return Err(format!("{file} is a project's settings file: a token is never written into a project, nothing was saved"));
    }
    if let Some(problem) = connection.problem() {
        return Err(format!("{file}: {problem}; nothing was saved"));
    }
    let text = blueprint_model::yaml::to_string(connection).map_err(|p| p.line())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    blueprint_model::atomic_file::write(path, text).map_err(|e| format!("{file}: {e}"))
}
