//! Typed questions to a decision model (d3-flash on the LAN, or Perplexity's through OpenRouter).
//!
//! A decision model answers questions about a state (text, JSON, up to [`MAX_IMAGES`] images) with a
//! probability for every allowed answer and generates nothing, so there is nothing to parse. Three
//! question types: [`Question::Noul`] (yes or no), [`Question::Choice`] (named options) and
//! [`Question::Score`] (levels on a scale).
//!
//! **Authoring time only.** Nothing here ships in a game. Tests never call a live model: they replay
//! recorded exchanges ([`Replay`]); a live call happens only through [`Live`], in calibration runs and
//! smoke checks.
//!
//! **One endpoint and one protocol per run, no fallback.** The person's `decision_model` setting (`local` or `remote`,
//! [`DecisionModel`]) chooses between System One on the LAN and OpenRouter Decisions (`docs/decide/openrouter.md`);
//! [`Endpoints::resolve`] reads the settings layers and builds the [`Config`], [`Live::new`] builds the transport it
//! names, and nothing is ever tried in turn. A wrong endpoint or a refused request is an [`Error`] naming the
//! endpoint, never a default answer.

mod b64;
mod calibrate;
mod check;
mod clip;
mod config;
mod decision_model;
mod openrouter;
mod run;
mod transport;
mod types;

pub use b64::data_url;
pub use calibrate::{Calibration, Exclusion, Sample, calibrate};
pub use check::{BUILTIN_CHECKS_YAML, CHECKS_VERSION, Check, PROJECT_CHECKS_FILE, Checks, Evidence, Finding};
pub use clip::{FRAMES, LABELLED_FILE, Labelled, clip_check, load_labelled, score_set, score_set_recording_exclusions};
pub use config::{
    Config, DecisionModel, Endpoints, LOCAL_ENDPOINT, LOCAL_MODEL_NAME, OPENROUTER_ENDPOINT, OPENROUTER_KEY_VAR, OPENROUTER_MODEL, Protocol,
    REMOTE_WITHOUT_KEY, TIMEOUT_SECS,
};
pub use decision_model::{Answered, first_ipv4, Chosen, Resolved, SETTING_KEY, SettingsProblem, choose, probe, project_file, project_may_not_choose, user_file};
pub use openrouter::{FREE_REQUESTS_PER_MINUTE, OpenRouterDecisions, OpenRouterFailure, wire_request};
pub use run::{Verdict, evidence_from, run_checks};
pub use transport::{Expecting, Http, Live, Paced, Recorder, Replay, Transport, names_the_same_model, verify_served};
pub use types::{Answer, MAX_IMAGES, MAX_QUESTIONS, Question, Request, Response, Usage};

use std::fmt;

/// Why a decision could not be made. Every variant that involves the network names the endpoint.
#[derive(Debug)]
pub enum Error {
    /// The request breaks the API's limits before it is sent (0 or more than 64 questions, more than 4 images).
    Invalid(String),
    /// The endpoint did not answer (connection refused, DNS, timeout).
    Unreachable { endpoint: String, detail: String },
    /// The endpoint answered with a non-2xx status.
    Refused { endpoint: String, status: u16, body: String },
    /// The endpoint answered 2xx with a body that is not the documented response, or that lacks a question's answer.
    BadResponse { endpoint: String, detail: String },
    /// A fixture could not be read, written, or does not match the request asked.
    Fixture(String),
    /// A calibration was asked of a set that cannot calibrate.
    Calibration(String),
    /// **One sample of a labelled set could not be scored** (#455), and which one is named. `cause` is what the
    /// transport said (a refusal, an unreachable endpoint, an answer for another model); a bare status leaves the
    /// person re-running the set by hand to find the sample that stopped it.
    Sample { id: String, cause: Box<Error> },
    /// A `checks.yaml` that does not parse or breaks a rule (duplicate name, threshold outside 0..=1).
    Checks(String),
    /// A calibration was labelled for one model and the endpoint serves, or answered as, another. The
    /// record would name a model that did not answer (#455), so the run is refused.
    ModelMismatch { endpoint: String, label: String, served: Vec<String> },
    /// A check was asked without an image or the facts it needs; it does not answer from less.
    MissingEvidence { check: String, missing: String },
    /// The provider declined one question (OpenRouter's `refusal`). Other questions of the request may
    /// have answers, but a check needs all of its own: a refusal is never a number.
    Refusal { endpoint: String, question: String },
    /// OpenRouter refused with a status that has its own meaning (402, 404, 429), carrying its reason.
    OpenRouter { endpoint: String, failure: OpenRouterFailure, reason: String },
    /// Remote is chosen and `OPENROUTER_API_KEY` is not in the environment. It is refused, never answered by the
    /// local model: the sentence is [`REMOTE_WITHOUT_KEY`].
    RemoteWithoutKey,
    /// The person's settings file does not read, so which model decides is not known and nothing is asked.
    Settings(SettingsProblem),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Invalid(m) => write!(f, "invalid decision request: {m}"),
            Error::Unreachable { endpoint, detail } => {
                write!(f, "decision endpoint {endpoint} did not answer: {detail}")
            }
            Error::Refused { endpoint, status, body } => {
                write!(f, "decision endpoint {endpoint} refused the request with status {status}: {body}")
            }
            Error::BadResponse { endpoint, detail } => {
                write!(f, "decision endpoint {endpoint} sent an unusable response: {detail}")
            }
            Error::Fixture(m) => write!(f, "decision fixture: {m}"),
            Error::Calibration(m) => write!(f, "decision calibration: {m}"),
            Error::Checks(m) => write!(f, "checks.yaml: {m}"),
            Error::Sample { id, cause } => write!(f, "sample `{id}` could not be scored: {cause}"),
            Error::ModelMismatch { endpoint, label, served } => write!(
                f,
                "the run is labelled `{label}` but decision endpoint {endpoint} serves {}; a calibration belongs to one \
                 model, so point the endpoint at `{label}` or label the run with the model that answers",
                if served.is_empty() { "no named model".to_owned() } else { served.join(", ") }
            ),
            Error::MissingEvidence { check, missing } => {
                write!(f, "check `{check}` needs {missing}, which was not captured; it does not answer from less")
            }
            Error::Refusal { endpoint, question } => write!(
                f,
                "decision endpoint {endpoint} declined to answer question `{question}`; a refusal is not a verdict, so no number was made of it"
            ),
            Error::OpenRouter { endpoint, failure, reason } => match failure {
                OpenRouterFailure::ContentFiltered { detail } => write!(
                    f,
                    "{endpoint} answered {status}, its content filter refused this request's picture: {detail}",
                    status = crate::openrouter::CONTENT_FILTER_STATUS
                ),
                OpenRouterFailure::CreditsExhausted => write!(
                    f,
                    "{endpoint} answered 402, the account or key is out of credits or at its limit: {reason}. Nothing was retried."
                ),
                OpenRouterFailure::NoEndpoint => write!(
                    f,
                    "{endpoint} answered 404, no provider satisfies the request's provider preferences (only/zdr/data_collection): {reason}. The privacy filter is never relaxed for you; change the model or the account's privacy settings."
                ),
                OpenRouterFailure::RateLimited { retry_after } => write!(
                    f,
                    "{endpoint} answered 429, rate limited (retry after {}): {reason}. Nothing was retried; pace the requests (free models allow {} a minute).",
                    retry_after.as_deref().unwrap_or("an unstated time"),
                    FREE_REQUESTS_PER_MINUTE
                ),
            },
            Error::RemoteWithoutKey => f.write_str(REMOTE_WITHOUT_KEY),
            Error::Settings(problem) => write!(f, "which decision model is chosen cannot be read, so nothing was asked: {problem}"),
        }
    }
}

impl std::error::Error for Error {}

/// Ask `request` of `transport`, and check the response answers every question with the type asked.
pub fn ask(transport: &dyn Transport, request: &Request) -> Result<Response, Error> {
    request.validate()?;
    let response = transport.send(request)?;
    response.check_answers(request, &transport.endpoint())?;
    Ok(response)
}
