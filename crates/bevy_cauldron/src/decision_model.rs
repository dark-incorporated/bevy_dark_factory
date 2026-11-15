//! **The editor's side of the decision-model setting** (DM-03, #454).
//!
//! Which decision model is asked is one setting, `decision_model`: Local (d3-flash on the LAN) or Remote (Perplexity's, through
//! OpenRouter). Everything that asks one reads it through [`Source`], which reads the settings layers off disk with
//! [`bevy_oracle::Endpoints::resolve`], the same resolver the MCP server calls, so the editor and the agent cannot choose
//! two models. Nothing reads an endpoint from the process environment or a project's `.env`; the one environment variable left is
//! `OPENROUTER_API_KEY`, which is a key and not a setting.
//!
//! **The status LED.** The band at the foot of the window carries one LED with the model's name: [`LedState::Live`] when the last
//! request was answered, `Busy` while one is out, `Failed` with the reason in its tooltip, and `Off` when nothing has been asked
//! of the model that is chosen now. Every request the editor makes goes through a [`Tracked`] transport, which writes the
//! [`DecisionLink`] the LED reads, so the light is a record of round trips that happened, never a guess. Remote chosen with no key
//! is the one failure known without a request, and the LED says it at once.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy_gooey::{Led, LedState, LedWord, Tooltip};
use bevy_oracle::{Config, DecisionModel, Endpoints, Error, Live, REMOTE_WITHOUT_KEY, Request, Resolved, Response, Transport};

use crate::settings::{EditorSettings, UserSettingsFile};

/// What the last requests found out, shared by every thread that asks (a labeller's task, an after-edit check's thread).
#[derive(Clone, Debug, Default)]
pub struct DecisionLink(Arc<Mutex<LinkState>>);

#[derive(Debug, Default)]
struct LinkState {
    /// Requests out now.
    in_flight: usize,
    /// How the last one ended.
    last: Option<Outcome>,
}

#[derive(Clone, Debug, PartialEq)]
struct Outcome {
    /// The choice the request was made under; `None` when the choice itself could not be read.
    site: Option<DecisionModel>,
    /// The model the answer named.
    model: Option<String>,
    /// Why it failed, or `None` when it was answered.
    failure: Option<String>,
}

/// **What the light says now**, for the model chosen now.
#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub light: LedState,
    /// The model that last answered, when it answered.
    pub model: Option<String>,
    /// Why the last request failed.
    pub failure: Option<String>,
}

impl DecisionLink {
    fn state(&self) -> MutexGuard<'_, LinkState> {
        // A thread that panicked mid-request leaves the light as it was: poisoning is not a reason to stop painting it.
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// A request goes out.
    pub fn begin(&self) {
        self.state().in_flight += 1;
    }

    /// The request was answered, by `model` when the answer named it.
    pub fn answered(&self, site: DecisionModel, model: Option<String>) {
        let mut s = self.state();
        s.in_flight = s.in_flight.saturating_sub(1);
        s.last = Some(Outcome { site: Some(site), model, failure: None });
    }

    /// The request ended without an answer, for `reason`.
    pub fn failed(&self, site: DecisionModel, reason: String) {
        let mut s = self.state();
        s.in_flight = s.in_flight.saturating_sub(1);
        s.last = Some(Outcome { site: Some(site), model: None, failure: Some(reason) });
    }

    /// A request was refused before it was made (no key, an unreadable settings file): no request to count.
    pub fn refused(&self, site: Option<DecisionModel>, reason: String) {
        self.state().last = Some(Outcome { site, model: None, failure: Some(reason) });
    }

    /// **The light for `chosen`**: busy while a request is out; otherwise how the last one ended, if it was made under
    /// this choice. After a switch from Local to Remote the light is `Off` until Remote has been asked, and never Local's
    /// answer.
    pub fn seen(&self, chosen: DecisionModel) -> Seen {
        let s = self.state();
        if s.in_flight > 0 {
            return Seen { light: LedState::Busy, model: None, failure: None };
        }
        match &s.last {
            Some(o) if o.site.is_none_or(|site| site == chosen) => match &o.failure {
                Some(why) => Seen { light: LedState::Failed, model: None, failure: Some(why.clone()) },
                None => Seen { light: LedState::Live, model: o.model.clone(), failure: None },
            },
            _ => Seen { light: LedState::Off, model: None, failure: None },
        }
    }
}

/// **The editor's decision state**: where each choice goes and the light. Inserted by `main` with the key from the environment; a
/// test's editor has the shipped endpoints and no key, or inserts its own (stand-in servers).
#[derive(Resource, Clone, Debug, Default)]
pub struct DecisionState {
    pub endpoints: Endpoints,
    pub link: DecisionLink,
}

/// **Everything a request needs to know which model to ask**, as a value a task or a thread can carry.
#[derive(Clone, Debug)]
pub struct Source {
    /// The person's settings file; `None` in a test's editor, which then has the built-in default (Local).
    pub user_file: Option<PathBuf>,
    pub endpoints: Endpoints,
    pub link: DecisionLink,
}

impl Source {
    /// **Which model is chosen for the project at `root`, and its configuration.** The refusal ([`Error::RemoteWithoutKey`] or
    /// an unreadable settings file) lights the LED as failed and is returned; it is never answered by the other model.
    pub fn resolve(&self, root: &Path) -> Result<Resolved, Error> {
        let resolved = self.endpoints.resolve(self.user_file.as_deref(), Some(&bevy_oracle::project_file(root)));
        if let Err(e) = &resolved {
            let site = matches!(e, Error::RemoteWithoutKey).then_some(DecisionModel::Remote);
            self.link.refused(site, e.to_string());
        }
        resolved
    }

    /// **The process's own source**: the person's real settings file, the shipped endpoints and the key in the environment, with
    /// a light of its own. What the binary builds; an operation dispatched with no editor in hand ([`crate::ops::Ctx::decisions`] is
    /// `None`) asks through it.
    pub fn process() -> Source {
        Source { user_file: crate::settings::user_file(), endpoints: Endpoints::from_env(), link: DecisionLink::default() }
    }

    /// The configuration for the project at `root`.
    pub fn config(&self, root: &Path) -> Result<Config, Error> {
        self.resolve(root).map(|r| r.config)
    }

    /// **A transport for the project at `root` that always works as a value**: [`Tracked`] when the choice can be asked, and one that
    /// refuses every request with the reason when it cannot (Remote with no key, a settings file that does not read). For a thread
    /// that runs a whole batch of checks and reports each one's own failure, instead of one error before any.
    pub fn asking(&self, root: &Path) -> Box<dyn Transport + Send> {
        match self.transport(root) {
            Ok(tracked) => Box::new(tracked),
            Err(e) => Box::new(Refusing(e)),
        }
    }

    /// **The transport for the project at `root`**: [`Live`] for the chosen model, writing the light as it goes.
    pub fn transport(&self, root: &Path) -> Result<Tracked, Error> {
        Ok(Tracked { inner: Live::new(self.config(root)?), link: self.link.clone() })
    }
}

/// A [`Live`] transport that writes the [`DecisionLink`]: busy when a request goes out, then answered or failed.
pub struct Tracked {
    inner: Live,
    link: DecisionLink,
}

impl Transport for Tracked {
    fn endpoint(&self) -> String {
        self.inner.endpoint()
    }

    fn site(&self) -> DecisionModel {
        self.inner.site()
    }

    fn send(&self, request: &Request) -> Result<Response, Error> {
        let site = self.inner.site();
        self.link.begin();
        let sent = self.inner.send(request);
        match &sent {
            Ok(response) => self.link.answered(site, response.model.clone()),
            Err(e) => self.link.failed(site, e.to_string()),
        }
        sent
    }
}

/// **A transport for a choice that cannot be asked**: every request is refused with the reason, and nothing is sent anywhere.
pub struct Refusing(Error);

impl Transport for Refusing {
    fn endpoint(&self) -> String {
        "the chosen decision model".to_owned()
    }

    fn send(&self, _: &Request) -> Result<Response, Error> {
        Err(match &self.0 {
            Error::RemoteWithoutKey => Error::RemoteWithoutKey,
            Error::Settings(problem) => Error::Settings(problem.clone()),
            other => Error::Invalid(other.to_string()),
        })
    }
}

/// **The source, for a system**: the state resource and the person's settings file, which a test's editor does not have.
#[derive(SystemParam)]
pub struct Decisions<'w> {
    state: Option<Res<'w, DecisionState>>,
    user: Option<Res<'w, UserSettingsFile>>,
}

impl Decisions<'_> {
    pub fn source(&self) -> Source {
        let state = self.state.as_deref().cloned().unwrap_or_default();
        Source { user_file: self.user.as_ref().map(|f| f.0.clone()), endpoints: state.endpoints, link: state.link }
    }
}

/// The word on the LED: the model that answered when one did (without its provider), else the chosen model's own name.
fn led_name(chosen: DecisionModel, seen: &Seen) -> String {
    match &seen.model {
        Some(model) => model.rsplit_once('/').map_or(model.as_str(), |(_, name)| name).to_owned(),
        None => chosen.name().to_owned(),
    }
}

/// **What the status LED shows**, from the person's choice, whether a key is set, and what the link saw.
#[derive(Clone, Debug, PartialEq)]
pub struct LedView {
    pub state: LedState,
    /// The model's name, the word beside the dot.
    pub name: String,
    /// Words for the hover: the state, where it runs and the reason a failure gives.
    pub tooltip: String,
}

/// The LED for `chosen`. **Remote chosen with no key is failed at once**, with the sentence a decision is refused with.
pub fn led_view(chosen: DecisionModel, key_set: bool, seen: &Seen) -> LedView {
    let name = led_name(chosen, seen);
    let (state, said) = if chosen == DecisionModel::Remote && !key_set {
        (LedState::Failed, format!("failed: {REMOTE_WITHOUT_KEY}"))
    } else {
        match seen.light {
            LedState::Busy => (LedState::Busy, "is answering a request".to_owned()),
            LedState::Live => (LedState::Live, "answered the last request".to_owned()),
            LedState::Failed => (LedState::Failed, format!("failed: {}", seen.failure.as_deref().unwrap_or("no reason was given"))),
            LedState::Off => (LedState::Off, "has not been asked yet".to_owned()),
        }
    };
    let tooltip = format!("Decision model: {name} ({chosen}) {said}. Choose it in Settings, Decisions.");
    LedView { state, name, tooltip }
}

/// The LED's wrapper, which carries the tooltip: an LED itself takes no pointer.
#[derive(Component)]
pub struct DecisionLedChip;

/// **The status band's decision-model LED** (DM-03, #454): [`Led`] on it, the model's name as its label.
#[derive(Component)]
pub struct DecisionLed;

/// The LED and its hover, off (nothing has been asked), named for the shipped default until [`paint_the_led`] reads the setting.
pub fn chip() -> impl Bundle {
    (
        bevy_gooey::row_with(crate::chrome::GAP_TIGHT_STEPS, bevy_gooey::RowLayout { align: Some(AlignItems::Center), ..default() }),
        Pickable::default(),
        DecisionLedChip,
        Tooltip::new(String::new()),
        Name::new("decision model"),
        children![(bevy_gooey::led(DecisionModel::default().name(), LedState::Off), DecisionLed)],
    )
}

/// **Keep the LED what the setting and the link say**, each write only on a difference.
pub fn paint_the_led(
    settings: Option<Res<EditorSettings>>,
    state: Option<Res<DecisionState>>,
    mut chips: Query<&mut Tooltip, With<DecisionLedChip>>,
    mut leds: Query<(&mut Led, &Children), With<DecisionLed>>,
    mut words: Query<&mut Text, Without<LedWord>>,
) {
    let chosen = settings.map_or_else(DecisionModel::default, |s| s.decision_model);
    let state = state.as_deref().cloned().unwrap_or_default();
    let view = led_view(chosen, state.endpoints.key.is_some(), &state.link.seen(chosen));
    for mut tip in &mut chips {
        if tip.text != view.tooltip {
            tip.text.clone_from(&view.tooltip);
        }
    }
    for (mut led, children) in &mut leds {
        if led.0 != view.state {
            led.0 = view.state;
        }
        for child in children.iter() {
            if let Ok(mut text) = words.get_mut(child)
                && text.0 != view.name
            {
                text.0.clone_from(&view.name);
            }
        }
    }
}

/// Registers the state and paints the LED.
pub struct DecisionModelPlugin;

impl Plugin for DecisionModelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DecisionState>().add_systems(Update, paint_the_led.after(crate::settings::sync_settings));
    }
}
