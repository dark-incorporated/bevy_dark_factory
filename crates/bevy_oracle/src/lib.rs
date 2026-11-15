//! Typed questions to a decision model (Cloudflare's Clef and clef-flash).
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
//! **One endpoint and one protocol per run, no fallback.** [`Config::protocol`] is System One (a local
//! llama.cpp server, Cloudflare) or OpenRouter Decisions (`docs/decide/openrouter.md`); [`Live::new`]
//! builds the transport the config names, and nothing is ever tried in turn. A wrong endpoint or a
//! refused request is an [`Error`] naming the endpoint, never a default answer.

mod b64;
mod calibrate;
mod check;
mod clip;
mod config;
mod openrouter;
mod run;
mod transport;
mod types;

pub use b64::data_url;
pub use calibrate::{Calibration, Sample, calibrate};
pub use check::{BUILTIN_CHECKS_YAML, CHECKS_VERSION, Check, PROJECT_CHECKS_FILE, Checks, Evidence, Finding};
pub use clip::{FRAMES, LABELLED_FILE, Labelled, clip_check, load_labelled, score_set};
pub use config::{Config, LOCAL_ENDPOINT, OPENROUTER_ENDPOINT, OPENROUTER_KEY_VAR, OPENROUTER_MODEL, PROTOCOL_VAR, Protocol};
pub use openrouter::{FREE_REQUESTS_PER_MINUTE, OpenRouterDecisions, OpenRouterFailure, wire_request};
pub use run::{Verdict, evidence_from, run_checks};
pub use transport::{Http, Live, Recorder, Replay, Transport};
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
    /// A `checks.yaml` that does not parse or breaks a rule (duplicate name, threshold outside 0..=1).
    Checks(String),
    /// A check was asked without an image or the facts it needs; it does not answer from less.
    MissingEvidence { check: String, missing: String },
    /// The provider declined one question (OpenRouter's `refusal`). Other questions of the request may
    /// have answers, but a check needs all of its own: a refusal is never a number.
    Refusal { endpoint: String, question: String },
    /// OpenRouter refused with a status that has its own meaning (402, 404, 429), carrying its reason.
    OpenRouter { endpoint: String, failure: OpenRouterFailure, reason: String },
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
            Error::MissingEvidence { check, missing } => {
                write!(f, "check `{check}` needs {missing}, which was not captured; it does not answer from less")
            }
            Error::Refusal { endpoint, question } => write!(
                f,
                "decision endpoint {endpoint} declined to answer question `{question}`; a refusal is not a verdict, so no number was made of it"
            ),
            Error::OpenRouter { endpoint, failure, reason } => match failure {
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
