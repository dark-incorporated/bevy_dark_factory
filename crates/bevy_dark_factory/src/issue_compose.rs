//! **An issue is composed in the editor, previewed, and filed without a capture** (#499).
//!
//! # Why a composer at all
//!
//! Before this, an issue reached GitHub only as a side effect of a capture (#495): press `Cmd+D`, point
//! at what is wrong, and the editor files it. That is exactly right for "here is what I saw" and wrong
//! for everything else — a feature request, a design note with no screen to point at, six issues from
//! one observation that wants splitting, or a body a person wants to write rather than have generated.
//!
//! # Six sections, because most of every issue here is the same six
//!
//! That is what makes this *procedural* rather than a text box: [`HEADINGS`] is the shape every issue
//! this repository receives already has, and the composer fills each from its own field. The Markdown
//! is composed by [`body`], which is engine-free and pure, so the shape is tested without a window and a
//! person can preview the exact bytes before they are filed.
//!
//! # Previewed, then filed, and the two are one value
//!
//! [`Preview`] holds the composed body, the connection's words above it, and the edit a person made.
//! [`file`] sends **`preview.body`** — not a recomposition — so a person cannot approve one thing and
//! ship another; there is no second path that builds a body. [`Preview::edit`] is the only writer of the
//! body after composition, which is what makes that rule a fact about the types rather than a promise.
//!
//! # What a composed issue does not do
//!
//! It makes **no branch, no commit and no push**: those exist in the capture path (#495) because a
//! capture's pictures must be on the branch its issue links. A composed issue attaches either nothing,
//! a capture already published (#495's own folder, whose links are still verified 200 first), a picture
//! already published elsewhere, or the editor's log **pasted into the body** — text needs no branch. So
//! a failed filing leaves nothing half-done on a repository, which is the issue's requirement 5.
//!
//! # The seams are values
//!
//! [`crate::github::GitHubCreate`] is the write half of #498's connection, [`crate::report_issue::Runner`]
//! is #495's own picture-link check, and both are handed in — a test files against a fake connection and
//! no suite asks the network.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::github::{Connection, Create, Credentials, GitHubCreate};
use crate::report_issue::Runner;

/// **The six sections, in the order a filed issue carries them.**
///
/// Named once and used by both the composer and its test, so the shape and the proof of the shape cannot
/// drift: a section added here is a section the order test reads.
pub const HEADINGS: [&str; 6] = ["What happened", "What was expected", "How to reproduce", "The measurement", "Acceptance", "Related work"];

/// **The version every draft file is written at.**
pub const DRAFT_VERSION: u32 = 1;

/// **The file a draft waits in**, under the project's own `.dark_factory/` folder: a half-written issue
/// is authoring state, so it travels with the project the way `.dark_factory/AGENTS.md` does.
pub const DRAFT_FILE: &str = "issue-draft.yaml";

/// **Where the draft lives for a project at `root`.** One function, so the writer and the reader name
/// the same file.
pub fn draft_file(root: &Path) -> PathBuf {
    root.join(".dark_factory").join(DRAFT_FILE)
}

/// **What is attached to a composed issue**, which is the issue's requirement 3.
///
/// Nothing, a capture this session took, a picture already published (a rendered frame), or the editor's
/// log tail. The first three can name a link, and every link is verified 200 before the issue exists —
/// #495's rule, inherited rather than restated. [`Attachment::Log`] is text and is pasted, so it can
/// never 404.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "what", rename_all = "kebab-case")]
pub enum Attachment {
    /// No attachment: the issue is the person's words and nothing else.
    #[default]
    Nothing,
    /// **A capture this session took**, embedded on the two raw URLs #495 files its pictures at.
    Capture {
        /// The capture's stem, which names its folder.
        stem: String,
        /// The branch the pictures are on: `main` is the better home, and a branch deleted after merge
        /// leaves the issue's pictures dead.
        branch: String,
    },
    /// **A picture already published** — a rendered frame, or a screenshot the person put somewhere
    /// that answers 200.
    Picture { url: String },
    /// **The editor's log tail**, pasted in a fenced block: text needs no branch and no link.
    Log { text: String },
}

impl Attachment {
    /// **The links this attachment embeds**, which are the ones [`file`] verifies before the issue
    /// exists. Empty for [`Attachment::Nothing`] and [`Attachment::Log`], whose content is the body.
    pub fn links(&self) -> Vec<String> {
        match self {
            Attachment::Nothing | Attachment::Log { .. } => Vec::new(),
            Attachment::Capture { stem, branch } => vec![
                blueprint_model::feedback::issue::raw_url(branch, stem, &blueprint_model::feedback::picks_picture_file(stem)),
                blueprint_model::feedback::issue::raw_url(branch, stem, &blueprint_model::feedback::picture_file(stem)),
            ],
            Attachment::Picture { url } => vec![url.clone()],
        }
    }

    /// **The Markdown this attachment contributes**, or empty when there is nothing to say.
    pub fn markdown(&self) -> String {
        match self {
            Attachment::Nothing => String::new(),
            Attachment::Capture { stem, branch } => format!(
                "**Attached**: the capture `{stem}` — ![the thing the note points at]({})\n\nFrame: ![whole window]({})",
                blueprint_model::feedback::issue::raw_url(branch, stem, &blueprint_model::feedback::picks_picture_file(stem)),
                blueprint_model::feedback::issue::raw_url(branch, stem, &blueprint_model::feedback::picture_file(stem))
            ),
            Attachment::Picture { url } => format!("**Attached**: ![the picture]({url})"),
            Attachment::Log { text } => format!("**Attached**: the editor's log tail.\n\n```text\n{}\n```", text.trim_end()),
        }
    }

    /// **This attachment's name**, for the panel's row.
    pub fn name(&self) -> String {
        match self {
            Attachment::Nothing => "nothing".to_owned(),
            Attachment::Capture { stem, .. } => format!("capture `{stem}`"),
            Attachment::Picture { url } => format!("picture `{url}`"),
            Attachment::Log { text } => format!("the log tail ({} lines)", text.lines().count()),
        }
    }
}

/// **An issue as a person composes one**: a title, the six sections, optional labels, the repository it
/// goes to, and whatever is attached.
///
/// Every field is a plain string, because that is what a person types, and the Markdown is composed from
/// them by [`body`] rather than assembled by hand.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Issue {
    /// The issue's title, as it reads in the list.
    pub title: String,
    /// Section 1: what happened.
    pub what_happened: String,
    /// Section 2: what was expected instead.
    pub what_was_expected: String,
    /// Section 3: how to reproduce it.
    pub how_to_reproduce: String,
    /// Section 4: the measurement — the numbers, the file and the symbol, so a pass does not have to
    /// find them again.
    pub measurement: String,
    /// Section 5: the boxes a test can prove.
    pub acceptance: String,
    /// Section 6: what this sits against.
    pub related: String,
    /// The labels the issue is filed with: `bug`, `enhancement`, `ui`, `owner`.
    pub labels: Vec<String>,
    /// **The repository it goes to**, `owner/name`; empty means the connection's own.
    pub repository: String,
    /// What is attached; see [`Attachment`].
    pub attachment: Attachment,
}

impl Issue {
    /// **The six sections' text, in [`HEADINGS`]' order**, which is the one place the mapping between a
    /// field and a section lives.
    fn sections(&self) -> [&str; 6] {
        [
            &self.what_happened,
            &self.what_was_expected,
            &self.how_to_reproduce,
            &self.measurement,
            &self.acceptance,
            &self.related,
        ]
    }

    /// **The repository this issue will be filed into**: its own when it names one, the connection's
    /// otherwise. One answer, so the preview names and the filing uses the same string.
    pub fn repository_into(&self, connection: &Connection) -> String {
        let named = self.repository.trim();
        if named.is_empty() {
            connection.repository_or_own().to_owned()
        } else {
            named.to_owned()
        }
    }
}

/// **The composed Markdown for `issue`, filed into `repository`**, which is the body a person previews
/// and the body that is filed.
///
/// Deterministic and pure: the same issue and repository give the same bytes, which is what lets
/// [`Preview`] be the single source of both the preview and the filing.
pub fn body(issue: &Issue, repository: &str) -> String {
    let mut out = format!("**Composed in the editor** — filing into `{repository}`{}.\n", labels_phrase(&issue.labels));
    let attached = issue.attachment.markdown();
    if !attached.is_empty() {
        out.push('\n');
        out.push_str(&attached);
        out.push('\n');
    }
    for (heading, text) in HEADINGS.iter().zip(issue.sections()) {
        out.push_str(&format!("\n## {heading}\n\n{}\n", filled_or_said(text)));
    }
    out
}

/// **What the provenance line says about labels**, or nothing when the issue carries none.
fn labels_phrase(labels: &[String]) -> String {
    let named: Vec<&str> = labels.iter().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    if named.is_empty() {
        String::new()
    } else {
        format!(", labelled {}", named.join(", "))
    }
}

/// **A section's text, or a plain statement that the composer was given nothing for it** — never an
/// empty heading, which reads as a section somebody forgot rather than one that does not apply.
fn filled_or_said(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        "*(the composer was given nothing for this section)*".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// **The issue as a person sees it before it is filed**: the composed Markdown, the connection and
/// repository named above it, and the title and labels the filing will use.
///
/// `body` is the only field [`file`] sends, and [`Preview::edit`] is the only writer after
/// composition — the two facts together are the rule that a person cannot approve one thing and ship
/// another.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    /// The account and the repository, in words, as [`Connection::describe_into`] says them.
    pub named_above: String,
    /// The repository the issue will be filed into.
    pub repository: String,
    /// The issue's title.
    pub title: String,
    /// The labels the filing will use.
    pub labels: Vec<String>,
    /// **The Markdown exactly as it will be filed**, editable.
    pub body: String,
    /// **The composer's first draft**, kept so a person can tell an edit from the composition.
    pub composed: String,
    /// What is attached, so the picture links can be verified at filing time.
    pub attachment: Attachment,
}

impl Preview {
    /// **Compose the preview from a connection and an issue.** The body is [`body`]'s, and the words
    /// above it are the connection's own sentence about the repository that will receive it.
    pub fn of(connection: &Connection, issue: &Issue) -> Preview {
        let repository = issue.repository_into(connection);
        let composed = body(issue, &repository);
        Preview {
            named_above: connection.describe_into(&repository),
            repository,
            title: issue.title.trim().to_owned(),
            labels: issue.labels.clone(),
            body: composed.clone(),
            composed,
            attachment: issue.attachment.clone(),
        }
    }

    /// **The person edits the Markdown**, which is what ships: this is the one writer of [`Self::body`]
    /// after composition.
    pub fn edit(&mut self, markdown: String) {
        self.body = markdown;
    }

    /// **Whether the body on screen is still the composer's own**, which the panel says so a person can
    /// tell what they are about to file.
    pub fn was_edited(&self) -> bool {
        self.body != self.composed
    }

    /// **The links the filing has to verify before the issue exists**, from the attachment.
    pub fn links(&self) -> Vec<String> {
        self.attachment.links()
    }
}

/// **Where a filing is up to**, in the state machine's own words, so the panel and a test read the same
/// thing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// The issue exists, at this URL.
    Filed(String),
    /// A step failed; nothing was filed, and this says which step and why.
    Stopped { step: &'static str, why: String },
}

/// **File a composed issue**, once, through the connection.
///
/// The order is the contract, and each step stops the filing: a title → a body → a token → every
/// attached picture answering 200 → **one** create. Nothing is written to a repository at any point, so
/// a failure leaves nothing half-done; a create is never retried, because a retry that succeeds after a
/// timeout is a second issue.
pub fn file(preview: &Preview, credentials: &dyn Credentials, links: &dyn Runner, github: &dyn GitHubCreate) -> Step {
    let stop = |step: &'static str, why: String| Step::Stopped { step, why };

    if preview.title.trim().is_empty() {
        return stop("title", "the issue has no title: a title is what the issue list shows".to_owned());
    }
    if preview.body.trim().is_empty() {
        return stop("body", "the issue has no body: there is nothing to file".to_owned());
    }
    let token = match credentials.token() {
        Ok(Some(token)) => token,
        Ok(None) => return stop("token", "no GitHub token: sign in with `gh`, or paste a token in the connection".to_owned()),
        Err(why) => return stop("token", why),
    };
    // **Every attached picture answers before the issue exists** (#495's rule).
    for url in preview.links() {
        if !links.url_answers(&url) {
            return stop("verify", format!("{url} did not answer 200"));
        }
    }
    let create = Create {
        repository: preview.repository.clone(),
        title: preview.title.clone(),
        body: preview.body.clone(),
        labels: preview.labels.clone(),
    };
    match github.create(&create, Some(&token)) {
        Ok(created) => Step::Filed(created.url),
        Err(why) => stop("create", why),
    }
}

/// **A draft as it is written to a project**: the version, then the issue.
///
/// The version is out here rather than in [`Issue`] so an issue stays exactly the fields a person fills;
/// a file of another version is refused by the reader rather than read as this one's.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    issue: Issue,
}

/// **Write the draft to the project's own file**, atomically, so a restart cannot leave half of one.
pub fn save_draft(path: &Path, issue: &Issue) -> Result<(), String> {
    let text = blueprint_model::yaml::to_string(&Stored { version: DRAFT_VERSION, issue: issue.clone() }).map_err(|p| p.line())?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    blueprint_model::atomic_file::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// **Read a draft back**, or `None` when the project has never had one — which is the composer opening
/// empty the first time, not an error.
///
/// A file that does not read, or that is another version, is refused **by name** rather than treated as
/// absent: a draft a person wrote and cannot see is worse than one that says why.
pub fn load_draft(path: &Path) -> Result<Option<Issue>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let name = path.display().to_string();
    let stored: Stored = blueprint_model::yaml::from_str(&text, &name).map_err(|p| p.line())?;
    if stored.version != DRAFT_VERSION {
        return Err(format!("{name} is a draft of version {}, and this editor writes {DRAFT_VERSION}; nothing was loaded", stored.version));
    }
    Ok(Some(stored.issue))
}
