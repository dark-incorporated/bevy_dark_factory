//! **The composer's panel: six fields, a preview, and one filing** (#499).
//!
//! # Where this sits beside the capture path
//!
//! `Cmd+D` (`crate::report`) points at what is wrong and files it — the right shape for *"here is what I
//! saw"*. This is the other shape, opened by **`Cmd+Shift+I`** from any screen: a title, the six
//! sections, the labels, the repository, what is attached, and a preview of the exact Markdown that will
//! be filed. The two share no chord, no resource and no file.
//!
//! # The preview is a field, because it is the body
//!
//! The issue's third acceptance is that the preview and the filed body are byte-identical, and the only
//! way to hold that over an edit is for the edit to *be* the body: [`issue_compose::Preview`] holds it,
//! the text area writes it back through [`Composer::edit`], and [`issue_compose::file`] sends that same
//! field. Nothing here recomposes behind a person's back — [`Composer::reweave`] leaves an edited body
//! alone and the dialog says so.
//!
//! # The draft survives
//!
//! `Save draft` writes the fields to the project's `.dark_factory/issue-draft.yaml`, and so does closing
//! the dialog, so a restart never loses one. Opening the composer reads it back through
//! [`issue_compose::load_draft`]; a project that never had one opens an empty composer, not an error.
//!
//! # The filing is a thread
//!
//! `curl` and GitHub are seconds long and network-bound, so [`file_the_issue`] starts a thread and
//! [`land_the_filing`] takes its answer, exactly as [`crate::report_issue`] does for a capture.

use bevy::prelude::*;
use bevy_gooey::{Shortcut, TextHint};

use crate::github::Connection;
use crate::issue_compose::{self, Attachment, Issue, Preview, Step};

/// **The dialog's width**, in body text sizes: wide enough for a paragraph of the Markdown, no wider
/// than the launcher's own dialog.
pub const CARD_TEXT_SIZES: f32 = 48.0;

/// **The rows the Markdown preview is given**, which is what a person reads before they file.
pub const PREVIEW_ROWS: usize = 18;

/// **The rows each of the six sections is given**: a paragraph, not a page — the preview below is where
/// the whole body is read.
pub const SECTION_ROWS: usize = 4;

/// **The longest title the field keeps**, in characters, which is GitHub's own limit on an issue title.
pub const TITLE_MAX_CHARS: usize = 256;

/// **How many lines of the editor's log the attachment takes**, which is `report.rs`'s tail: the same
/// number, so an issue and a capture attach the same thing.
pub const LOG_TAIL_LINES: usize = 200;

/// **The labels the composer offers**, in the order `docs/loop/captures/ISSUE-FORMAT.md` names them.
pub const LABELS: [&str; 5] = ["bug", "enhancement", "ui", "owner", "debug"];

/// **The Markdown preview's field**, so the body can be told from the other areas in the dialog.
#[derive(Component)]
pub struct PreviewField;

/// **The dialog's root**, so the panel is rebuilt when what it draws changes and taken down when it
/// closes.
#[derive(Component)]
pub struct ComposerRoot;

/// **The composer is open**, which is what the keyboard guard reads: while a field has the keyboard, no
/// other key fires — the same reason `token_prompt`'s prompt is in `editor::not_typing`'s list.
#[derive(Component)]
pub struct ComposerShown;

/// **One line of the dialog**, so one system fills every sentence in it from the state rather than
/// respawning a card under somebody who is typing in it.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// The connection's own words: the account and the repository.
    Connection,
    /// What the last filing said.
    Result,
    /// What is attached.
    Attachment,
    /// Which sections were given nothing.
    Missing,
    /// Whether the Markdown is the composer's own or the person's edit.
    Preview,
}

/// **The composer's state**: the issue being written, the preview of it, and where the last filing got
/// to — one resource, because the three are one act.
#[derive(Resource, Default)]
pub struct Composer {
    /// **Whether the dialog is up.** A field of this resource rather than a query, because the keyboard
    /// guard must answer it in `Phase::Sense`, before anything is drawn.
    pub open: bool,
    /// The title, the six sections, the labels, the repository, the attachment.
    pub issue: Issue,
    /// **The preview**, composed by [`Composer::weave`]. `None` until the composer is first opened.
    pub preview: Option<Preview>,
    /// What the last filing answered.
    pub result: Option<Step>,
    /// A filing is on its own thread.
    pub filing: bool,
    /// **The connection the preview names and the filing goes through**, read when the dialog opens.
    pub connection: Connection,
}

impl Composer {
    /// **Compose the preview from the fields as they stand.**
    pub fn weave(&mut self) {
        self.preview = Some(Preview::of(&self.connection, &self.issue));
    }

    /// **A field above the preview changed.** The preview follows — unless the person edited it, in
    /// which case the edit is theirs and stands, and [`Line::Preview`] keeps saying so.
    pub fn reweave(&mut self) {
        if self.preview.as_ref().is_some_and(Preview::was_edited) {
            return;
        }
        self.weave();
    }

    /// **The person edited the Markdown in the preview**, which is the body that ships.
    pub fn edit(&mut self, markdown: String) {
        if self.preview.is_none() {
            self.weave();
        }
        if let Some(preview) = self.preview.as_mut() {
            preview.edit(markdown);
        }
    }

    /// **The preview's body**, or empty when the composer has not composed one yet.
    pub fn body(&self) -> String {
        self.preview.as_ref().map(|p| p.body.clone()).unwrap_or_default()
    }
}

/// **The six sections as fields**: their heading, the text to start with, and the setter that writes the
/// value back into the issue.
///
/// One list, so adding a section means adding it here and in [`issue_compose::HEADINGS`], and the panel's
/// order is the filed order.
/// **One editable section**: its heading, the text as it stands, and the setter that writes it back.
///
/// Named because the three-tuple reads as noise inline and clippy is right that it is a type.
type SectionField = (&'static str, String, fn(&mut Issue, String));

fn section_fields(issue: &Issue) -> Vec<SectionField> {
    use issue_compose::HEADINGS;
    vec![
        (HEADINGS[0], issue.what_happened.clone(), |i: &mut Issue, v: String| i.what_happened = v),
        (HEADINGS[1], issue.what_was_expected.clone(), |i: &mut Issue, v: String| i.what_was_expected = v),
        (HEADINGS[2], issue.how_to_reproduce.clone(), |i: &mut Issue, v: String| i.how_to_reproduce = v),
        (HEADINGS[3], issue.measurement.clone(), |i: &mut Issue, v: String| i.measurement = v),
        (HEADINGS[4], issue.acceptance.clone(), |i: &mut Issue, v: String| i.acceptance = v),
        (HEADINGS[5], issue.related.clone(), |i: &mut Issue, v: String| i.related = v),
    ]
}

/// **The editor's log tail**, as [`crate::report`] reads it: the last [`LOG_TAIL_LINES`] lines of the log
/// file, pasted into the body. Empty when there is no log, and the attachment then says so.
pub fn log_tail() -> String {
    let Some(path) = blueprint_model::user_dirs::log_file() else {
        return String::new();
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => blueprint_model::feedback::last_lines(&text, LOG_TAIL_LINES),
        Err(_) => String::new(),
    }
}

/// **Which of the six sections the person was given nothing for**, so they are told before they file
/// rather than after somebody reads it. Empty when every one is filled.
pub fn missing_sections(issue: &Issue) -> Vec<&'static str> {
    issue_compose::HEADINGS
        .iter()
        .copied()
        .zip([
            issue.what_happened.as_str(),
            issue.what_was_expected.as_str(),
            issue.how_to_reproduce.as_str(),
            issue.measurement.as_str(),
            issue.acceptance.as_str(),
            issue.related.as_str(),
        ])
        .filter(|(_, text)| text.trim().is_empty())
        .map(|(heading, _)| heading)
        .collect()
}

/// **What one line of the dialog says**, from the composer as it stands.
pub fn line_text(line: Line, composer: &Composer) -> String {
    match line {
        Line::Connection => match composer.preview.as_ref() {
            Some(preview) => preview.named_above.clone(),
            None => composer.connection.describe(),
        },
        Line::Result => match (&composer.result, composer.filing) {
            (Some(Step::Filed(url)), _) => format!("filed: {url}"),
            (Some(Step::Stopped { step, why }), _) => format!("not filed (at {step}): {why}"),
            (None, true) => "filing…".to_owned(),
            (None, false) => "not filed yet".to_owned(),
        },
        Line::Attachment => format!("attached: {}", composer.issue.attachment.name()),
        Line::Missing => {
            let missing = missing_sections(&composer.issue);
            if missing.is_empty() {
                "every section has text".to_owned()
            } else {
                format!("no text for: {}", missing.join(", "))
            }
        }
        Line::Preview => match composer.preview.as_ref() {
            Some(preview) if preview.was_edited() => "preview: the person's edit, and this is what will be filed".to_owned(),
            _ => "preview: composed from the fields, exactly as it will be filed".to_owned(),
        },
    }
}

/// **Where the draft is saved for the open project**, or `None` when no project is open — in which case
/// a composed issue is still filed, it just has nowhere to wait between sessions.
pub fn draft_path(project: Option<&crate::project::Project>) -> Option<std::path::PathBuf> {
    project.map(|p| issue_compose::draft_file(&p.root))
}

/// **Write the draft, saying so**, and never silently: a save that failed is a draft a person thinks is
/// safe.
pub fn save(composer: &Composer, path: Option<&std::path::Path>, commands: &mut Commands) {
    let Some(path) = path else {
        commands.spawn(bevy_gooey::toast_alert("no project is open, so the issue draft has nowhere to wait").sticky());
        return;
    };
    match issue_compose::save_draft(path, &composer.issue) {
        Ok(()) => {
            commands.spawn(bevy_gooey::toast(format!("issue draft saved to {}", path.display())));
        }
        Err(why) => {
            commands.spawn(bevy_gooey::toast_alert(format!("the issue draft was not saved: {why}")).sticky());
        }
    }
}

pub struct IssueComposePlugin;

impl Plugin for IssueComposePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Composer>()
            .init_resource::<Pending>()
            .init_resource::<Filing>()
            .add_systems(
                Update,
                // **The keys are the composer's while it is open**, and gooey's own fields do the typing:
                // the chord that opened the dialog is drained rather than typed, the same guard
                // `token_prompt` needs.
                (open_composer.in_set(crate::keys::Phase::Act), composer_keys.in_set(crate::keys::Phase::Act)).chain(),
            )
            .add_systems(Update, (sync_composer.after(crate::keys::Phase::Text), refill_preview.after(sync_composer), land_the_filing));
    }
}

// ---------------------------------------------------------------------------------------------
// Opening, drawing and closing
// ---------------------------------------------------------------------------------------------

/// **`Cmd+Shift+I` opens the composer**, with the project's draft read back and the connection named.
///
/// The draft is read here rather than in the drawing system, so a file that does not read is one problem
/// said once, at the moment a person asked for it, not one toast per frame.
fn open_composer(
    keys: Res<ButtonInput<KeyCode>>,
    mut composer: ResMut<Composer>,
    project: Option<Res<crate::project::Project>>,
    mut commands: Commands,
) {
    if composer.open || !crate::chords::chord(crate::chords::Fixed::IssueCompose).just_pressed(&keys) {
        return;
    }
    // **The connection comes from the person's own file**, never a project's (#498's rule), and a
    // connection that does not read leaves the default in place — `gh` on this machine — rather than
    // refusing to open a composer.
    if let Some(path) = crate::github::connection_file() {
        match crate::github::load(&path) {
            Ok(Some(connection)) => composer.connection = connection,
            Ok(None) => {}
            Err(why) => {
                commands.spawn(bevy_gooey::toast_alert(format!("the GitHub connection did not read: {why}")).sticky());
            }
        }
    }
    if let Some(path) = draft_path(project.as_deref()) {
        match issue_compose::load_draft(&path) {
            Ok(Some(issue)) => composer.issue = issue,
            Ok(None) => {}
            Err(why) => {
                commands.spawn(bevy_gooey::toast_alert(format!("the issue draft did not read: {why}")).sticky());
            }
        }
    }
    composer.result = None;
    composer.filing = false;
    composer.weave();
    composer.open = true;
}

/// **One dialog while the composer is open, none otherwise**, and its sentences read the state.
///
/// The card is gooey's `dialog` and `sized_panel`, which is what every other dialog in the editor is, so
/// there is no second UI vocabulary here. What this system does not do is rebuild the card: the fields
/// keep their own text, and only the [`Line`]s and the preview are refilled as the state moves — a card
/// respawned under a person typing in it would take the keyboard away after one character
/// (`project_page::fill_the_guide_buttons` records the same defect).
fn sync_composer(mut commands: Commands, composer: Res<Composer>, standing: Query<Entity, With<ComposerRoot>>) {
    if !composer.open {
        for entity in &standing {
            commands.entity(entity).try_despawn();
        }
        return;
    }
    if standing.is_empty() {
        draw_composer(&mut commands, &composer);
    }
}

/// **Build the composer's dialog**, once, from the state as it stands.
fn draw_composer(commands: &mut Commands, composer: &Composer) {
    let issue = composer.issue.clone();
    let body = composer.body();
    commands
        .spawn((bevy_gooey::dialog(), ComposerRoot, ComposerShown, Name::new("compose an issue")))
        .insert(GlobalZIndex(crate::style::metrics().z.modal))
        .with_children(|scrim| {
            scrim.spawn((bevy_gooey::sized_panel(bevy_gooey::CardWidth::Text(CARD_TEXT_SIZES)), bevy::picking::hover::Hovered::default())).with_children(|card| {
                card.spawn(bevy_gooey::heading("COMPOSE AN ISSUE"));
                for line in [Line::Connection, Line::Missing, Line::Attachment, Line::Result] {
                    card.spawn((bevy_gooey::label(line_text(line, composer)), line));
                }

                // **The title and the repository**, one line each.
                card.spawn((
                    bevy_gooey::text_input("Title", issue.title.clone()),
                    bevy_gooey::TextLimit(TITLE_MAX_CHARS),
                    bevy_gooey::observe(|change: On<bevy_gooey::ValueChange<String>>, mut composer: ResMut<Composer>| {
                        composer.issue.title = change.value.clone();
                        composer.reweave();
                    }),
                ));
                card.spawn((
                    bevy_gooey::text_input("Repository", issue.repository.clone()),
                    TextHint("owner/name, or empty for the connection's own".to_owned()),
                    bevy_gooey::observe(|change: On<bevy_gooey::ValueChange<String>>, mut composer: ResMut<Composer>| {
                        composer.issue.repository = change.value.clone();
                        composer.reweave();
                    }),
                ));
                card.spawn((
                    bevy_gooey::text_input("Labels", issue.labels.join(", ")),
                    TextHint(LABELS.join(", ")),
                    bevy_gooey::observe(|change: On<bevy_gooey::ValueChange<String>>, mut composer: ResMut<Composer>| {
                        composer.issue.labels = change.value.split(',').map(|l| l.trim().to_owned()).filter(|l| !l.is_empty()).collect();
                        composer.reweave();
                    }),
                ));

                // **The six sections, in the order they are filed.**
                for (heading, text, set) in section_fields(&issue) {
                    card.spawn((
                        bevy_gooey::text_area(heading, text, SECTION_ROWS),
                        bevy_gooey::observe(move |change: On<bevy_gooey::ValueChange<String>>, mut composer: ResMut<Composer>| {
                            set(&mut composer.issue, change.value.clone());
                            composer.reweave();
                        }),
                    ));
                }

                // **The preview is editable, and the edit is the body**: the observer writes the value
                // straight into `Preview::body`, which is the one field `file` sends. Nothing recomposes
                // it afterwards — `Composer::reweave` leaves an edited body alone — so what a person
                // approves here is what is filed, byte for byte.
                card.spawn((
                    bevy_gooey::text_area("The Markdown, exactly as it will be filed", body, PREVIEW_ROWS),
                    PreviewField,
                    bevy_gooey::observe(|change: On<bevy_gooey::ValueChange<String>>, mut composer: ResMut<Composer>| {
                        composer.edit(change.value.clone());
                    }),
                ));
                card.spawn((bevy_gooey::label(line_text(Line::Preview, composer)), Line::Preview));

                card.spawn((bevy_gooey::row(0.0), crate::chrome::Gap::Group.class())).with_children(|row| {
                    row.spawn((
                        bevy_gooey::button("Attach the log tail"),
                        bevy_gooey::observe(|_: On<bevy_gooey::Activate>, mut composer: ResMut<Composer>| {
                            composer.issue.attachment = Attachment::Log { text: log_tail() };
                            composer.weave();
                        }),
                    ));
                    row.spawn((
                        bevy_gooey::button("Attach nothing"),
                        bevy_gooey::observe(|_: On<bevy_gooey::Activate>, mut composer: ResMut<Composer>| {
                            composer.issue.attachment = Attachment::Nothing;
                            composer.weave();
                        }),
                    ));
                    row.spawn((
                        bevy_gooey::text_input("Picture URL", String::new()),
                        TextHint("a picture already published, verified 200 before filing".to_owned()),
                        bevy_gooey::observe(|change: On<bevy_gooey::ValueChange<String>>, mut composer: ResMut<Composer>| {
                            let url = change.value.trim().to_owned();
                            composer.issue.attachment = if url.is_empty() { Attachment::Nothing } else { Attachment::Picture { url } };
                            composer.weave();
                        }),
                    ));
                });

                card.spawn((bevy_gooey::row(0.0), crate::chrome::Gap::Group.class())).with_children(|row| {
                    row.spawn((
                        bevy_gooey::button("Save draft"),
                        bevy_gooey::observe(|_: On<bevy_gooey::Activate>, composer: Res<Composer>, project: Option<Res<crate::project::Project>>, mut commands: Commands| {
                            let path = draft_path(project.as_deref());
                            save(&composer, path.as_deref(), &mut commands);
                        }),
                    ));
                    row.spawn((
                        bevy_gooey::button("File issue"),
                        bevy_gooey::observe(|_: On<bevy_gooey::Activate>, mut composer: ResMut<Composer>, mut pending: ResMut<Pending>, filing: Res<Filing>| {
                            file_the_issue(composer.as_mut(), pending.as_mut(), &filing);
                        }),
                    ));
                    row.spawn((
                        bevy_gooey::button("Cancel"),
                        bevy_gooey::observe(|_: On<bevy_gooey::Activate>, mut composer: ResMut<Composer>, project: Option<Res<crate::project::Project>>, mut commands: Commands| {
                            let path = draft_path(project.as_deref());
                            save(&composer, path.as_deref(), &mut commands);
                            composer.open = false;
                        }),
                    ));
                });
            });
        });
}

/// **The composer's own keys**: `Esc` closes and saves the draft, `Cmd+S` saves it without closing, and
/// `Cmd+Enter` files. Nothing else reaches the tab behind the dialog, because a field in it holds the
/// keyboard (`bevy_gooey::TextEditing`) and the census stands down while one does.
fn composer_keys(
    keys: Res<ButtonInput<KeyCode>>,
    mut composer: ResMut<Composer>,
    mut pending: ResMut<Pending>,
    filing: Res<Filing>,
    project: Option<Res<crate::project::Project>>,
    mut commands: Commands,
) {
    if !composer.open {
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        let path = draft_path(project.as_deref());
        save(&composer, path.as_deref(), &mut commands);
        composer.open = false;
        return;
    }
    if Shortcut::new(bevy_gooey::Modifiers::PRIMARY, KeyCode::KeyS).just_pressed(&keys) {
        let path = draft_path(project.as_deref());
        save(&composer, path.as_deref(), &mut commands);
        return;
    }
    if Shortcut::new(bevy_gooey::Modifiers::PRIMARY, KeyCode::Enter).just_pressed(&keys) {
        file_the_issue(composer.as_mut(), pending.as_mut(), &filing);
    }
}

/// **The preview refilled, and the sentences above it**, when what they say moves.
///
/// The area's text is written only when it differs, so a person's caret stays where it was: gooey puts a
/// new [`bevy_gooey::TextInputValue`] into the field, and rewriting it every frame would fight the typist.
fn refill_preview(
    composer: Res<Composer>,
    mut lines: Query<(&Line, &mut Text)>,
    mut areas: Query<&mut bevy_gooey::TextInputValue, With<PreviewField>>,
) {
    if !composer.is_changed() {
        return;
    }
    for (line, mut text) in &mut lines {
        let want = line_text(*line, &composer);
        if text.0 != want {
            text.0 = want;
        }
    }
    let want = composer.body();
    for mut area in &mut areas {
        if area.0 != want {
            area.0.clone_from(&want);
        }
    }
}

/// **The filing runs on its own thread**, and its `Receiver` waits in the composer.
///
/// `curl` and GitHub are seconds long and network-bound, so the button starts a thread and
/// [`land_the_filing`] takes the answer: a frame is never held for a network.
#[derive(Resource, Default)]
pub struct Pending {
    rx: Option<std::sync::Mutex<std::sync::mpsc::Receiver<Step>>>,
}

/// **The two things a filing asks of the world**, as values the caller hands in.
///
/// The defaults are the real ones — [`crate::github::UreqCreate`] and
/// [`crate::report_issue::SystemRunner`] — so the shipped path is one path. A test inserts its own pair,
/// which is how the panel's own File button is proved without a network: the seam is a value, never a
/// branch in the code that ships (the shape `github::AnswersOff` established for #498).
#[derive(Resource)]
pub struct Filing {
    /// The write half of the connection.
    pub github: std::sync::Arc<dyn crate::github::GitHubCreate + Send + Sync>,
    /// The picture-link check, `report_issue::Runner`'s.
    pub links: std::sync::Arc<dyn crate::report_issue::Runner + Send + Sync>,
    /// **What the last filing asked of the connection**, recorded by [`file_the_issue`] so the panel can
    /// say what it filed and a test can read the one create rather than trusting a count elsewhere.
    pub recorded: std::sync::Arc<std::sync::Mutex<Vec<crate::github::Create>>>,
}

impl Default for Filing {
    fn default() -> Self {
        Filing {
            github: std::sync::Arc::new(crate::github::UreqCreate),
            links: std::sync::Arc::new(crate::report_issue::SystemRunner),
            recorded: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

/// **Start the filing**, once, from the preview as it stands.
///
/// **One press is one filing**: a second press while one is in flight says so and does nothing, because a
/// create retried is a second issue.
pub fn file_the_issue(composer: &mut Composer, pending: &mut Pending, filing: &Filing) {
    if composer.filing {
        return;
    }
    let Some(preview) = composer.preview.clone() else { return };
    let credentials = Credentialed(composer.connection.clone());
    let github = std::sync::Arc::new(Recording { inner: std::sync::Arc::clone(&filing.github), log: std::sync::Arc::clone(&filing.recorded) });
    let links = std::sync::Arc::clone(&filing.links);
    let (tx, rx) = std::sync::mpsc::channel();
    composer.filing = true;
    composer.result = None;
    *pending = Pending { rx: Some(std::sync::Mutex::new(rx)) };
    std::thread::spawn(move || {
        let step = issue_compose::file(&preview, &credentials, links.as_ref(), github.as_ref());
        let _ = tx.send(step);
    });
}

/// **A connection wrapper that writes down every create**, so the panel can say what it filed and a test
/// reads the one body that went out. The same shape as the fakes `github::AnswersOff` establishes: the
/// recording is around the seam, never a branch in the code that ships.
struct Recording {
    inner: std::sync::Arc<dyn crate::github::GitHubCreate + Send + Sync>,
    log: std::sync::Arc<std::sync::Mutex<Vec<crate::github::Create>>>,
}

impl crate::github::GitHubCreate for Recording {
    fn create(&self, issue: &crate::github::Create, token: Option<&str>) -> Result<crate::github::Created, String> {
        self.log.lock().map(|mut log| log.push(issue.clone())).ok();
        self.inner.create(issue, token)
    }
}

/// **The token the connection names**, as the credentials a filing asks for: the stored one, or `gh` on
/// this machine — the same pair `github::Named` reads, wrapped so the thread can own it.
struct Credentialed(Connection);

impl crate::github::Credentials for Credentialed {
    fn token(&self) -> Result<Option<String>, String> {
        crate::github::Named { connection: &self.0, machine: crate::github::GhOnPath }.token()
    }
}

/// **Take a finished filing's answer and say it**: one URL, or the step that stopped it and why — never
/// silence, which is the defect a filed issue exists to avoid.
fn land_the_filing(mut composer: ResMut<Composer>, mut pending: ResMut<Pending>, mut commands: Commands) {
    let Some(rx) = pending.rx.as_ref() else { return };
    let Ok(step) = rx.lock().map(|r| r.try_recv()).unwrap_or(Err(std::sync::mpsc::TryRecvError::Empty)) else { return };
    pending.rx = None;
    composer.filing = false;
    match &step {
        Step::Filed(url) => {
            commands.spawn(bevy_gooey::toast(format!("issue filed: {url}")));
        }
        Step::Stopped { step, why } => {
            commands.spawn(bevy_gooey::toast_alert(format!("the issue was not filed (at {step}): {why}")).sticky());
        }
    }
    composer.result = Some(step);
}
