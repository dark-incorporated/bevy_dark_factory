//! **OpenRouter Decisions as a second protocol** (#453): `POST https://openrouter.ai/api/alpha/decisions`.
//!
//! Why `GRIMOIRE_DECIDE_ENDPOINT` alone cannot reach it, and what this module does about each reason:
//!
//! 1. The path differs: [`Config::from_lookup`] defaults the endpoint for `GRIMOIRE_DECIDE_PROTOCOL=openrouter`.
//! 2. OpenRouter rejects a top-level `images` field with 400. Images go **inside `state`**, as
//!    `{"type": "image_url", ...}` parts after the text: [`wire_request`]. [`Request`] keeps its one shape
//!    (text in `state`, images in `images`), so [`Replay`](crate::Replay) and [`Recorder`](crate::Recorder)
//!    wrap this transport exactly as they wrap [`Http`](crate::Http).
//! 3. An answer can be a `refusal`: it is an [`Error::Refusal`] naming the question, never a number.
//! 4. A `provider` block pins who may see a screenshot (`perplexity/*` models only; no other model's
//!    provider is known, so none is invented).
//! 5. The free tier is paced (20 a minute). **Pacing is the caller's**: this transport never sleeps and
//!    never retries; 402, 404 and 429 each come back as a named [`Error::OpenRouter`] with OpenRouter's reason.
//!
//! The key is `OPENROUTER_API_KEY` from the process environment (see [`Config`]). It is sent as the bearer
//! token and appears in no error, no `Debug` output and no recorded fixture.
//!
//! Facts: <https://openrouter.ai/docs/guides/features/decisions>, the Perplexity adapter in
//! <https://openrouter.ai/docs/guides/community/multimodal-decisions>, and `docs/decide/openrouter.md`.

use crate::transport::{Reply, post_json};
use crate::{Answer, Config, Error, OPENROUTER_MODEL, Protocol, Request, Response, Transport, Usage};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// OpenRouter's `:free` models allow this many requests a minute; the calibration run paces to it.
/// (<https://openrouter.ai/docs/api-reference/limits>)
pub const FREE_REQUESTS_PER_MINUTE: u32 = 20;

/// Models with this prefix are served by Perplexity and carry the [`PROVIDER_SLUG`] pin.
const PERPLEXITY_PREFIX: &str = "perplexity/";
/// OpenRouter's provider slug for Perplexity (the `only` list takes slugs).
const PERPLEXITY_SLUG: &str = "perplexity";
/// What the response's `provider` is called when Perplexity answered.
const PERPLEXITY_NAME: &str = "Perplexity";

/// Images the adapter reads are base64 `data:` URLs of these types; remote URLs are not fetched.
const IMAGE_DATA_URL_PREFIXES: [&str; 3] =
    ["data:image/png;base64,", "data:image/jpeg;base64,", "data:image/webp;base64,"];

/// Longest piece of a refusal body quoted when it is not OpenRouter's `{"error": ...}` shape.
const MAX_QUOTED_BODY_CHARS: usize = 500;

/// Why OpenRouter said no, for the statuses that each mean something different to the person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenRouterFailure {
    /// 402: out of credits, or the key's own limit (`error.metadata.limit_source` says which).
    CreditsExhausted,
    /// 404: no endpoint satisfies the `provider` preferences (`only`, `zdr`, `data_collection`) or the
    /// account's privacy settings. **The filter is never relaxed.**
    NoEndpoint,
    /// 429: the rate limit. `retry_after` is the `Retry-After` header as sent, if there was one.
    RateLimited { retry_after: Option<String> },
}

/// The body OpenRouter wants for `request`, as JSON: `{model, state, questions, provider?}`.
///
/// * `state` is the plain string when there are no images (the documented text-only form), otherwise
///   `[text, {"type": "image_url", "image_url": {"url": <data URL>}}, ...]`, in the order the images
///   were added. There is **no top-level `images`**: OpenRouter answers 400 to one.
/// * `provider` is present for `perplexity/*` models and absent for any other.
/// * An image that is not a base64 `data:image/{png,jpeg,webp}` URL is an [`Error::Invalid`] naming its
///   position, before anything is sent: OpenRouter does not fetch remote URLs.
///
/// `model` is the one to send ([`OpenRouterDecisions`] resolves it from the request, then the config).
pub fn wire_request(request: &Request, model: &str) -> Result<Value, Error> {
    for (i, image) in request.images.iter().enumerate() {
        if !IMAGE_DATA_URL_PREFIXES.iter().any(|p| image.starts_with(p)) {
            return Err(Error::Invalid(format!(
                "image {} is not a base64 data URL of a PNG, JPEG or WebP (`data:image/png;base64,...`); OpenRouter does not fetch remote URLs",
                i + 1
            )));
        }
    }
    let state = if request.images.is_empty() {
        Value::String(request.state.clone())
    } else {
        let mut parts = vec![Value::String(request.state.clone())];
        parts.extend(
            request.images.iter().map(|url| json!({ "type": "image_url", "image_url": { "url": url } })),
        );
        Value::Array(parts)
    };
    let mut body = json!({
        "model": model,
        "state": state,
        "questions": request.questions,
    });
    if model.starts_with(PERPLEXITY_PREFIX) {
        body["provider"] = json!({
            "only": [PERPLEXITY_SLUG],
            "allow_fallbacks": false,
            "data_collection": "deny",
            "zdr": true,
        });
    }
    Ok(body)
}

/// OpenRouter's Decisions endpoint, over HTTP. Build it with [`Live::new`](crate::Live::new) so the config's
/// protocol picks it; it refuses a config whose protocol is not `openrouter`.
pub struct OpenRouterDecisions {
    pub config: Config,
}

impl OpenRouterDecisions {
    pub fn new(config: Config) -> Self {
        OpenRouterDecisions { config }
    }
}

impl Transport for OpenRouterDecisions {
    fn endpoint(&self) -> String {
        self.config.endpoint.clone()
    }

    fn send(&self, request: &Request) -> Result<Response, Error> {
        if self.config.protocol != Protocol::OpenRouter {
            return Err(Error::Invalid(format!(
                "`OpenRouterDecisions` speaks openrouter only, but the config's protocol is `{}`; one protocol per run: build the transport with `Live::new`",
                self.config.protocol
            )));
        }
        let endpoint = self.endpoint();
        if self.config.key.is_none() {
            return Err(Error::Invalid(format!(
                "{} is not set; OpenRouter needs a key, and it is read from the environment only",
                crate::OPENROUTER_KEY_VAR
            )));
        }
        let model = request.model.as_deref().or(self.config.model.as_deref()).unwrap_or(OPENROUTER_MODEL);
        let body = wire_request(request, model)?;
        let reply = post_json(&self.config, &body)?;
        if !(200..300).contains(&reply.status) {
            return Err(refusal_of(&endpoint, reply));
        }
        let response = parse_response(&endpoint, &reply.body)?;
        check_provider(&endpoint, model, &response)?;
        Ok(response)
    }
}

/// A `perplexity/*` request was pinned to Perplexity (`allow_fallbacks: false`); an answer from anyone
/// else means the pin did not hold, which is a privacy failure, not a detail.
fn check_provider(endpoint: &str, model: &str, response: &Response) -> Result<(), Error> {
    let served_by = response.usage.as_ref().and_then(|u| u.provider.as_deref());
    match served_by {
        Some(p) if model.starts_with(PERPLEXITY_PREFIX) && !p.eq_ignore_ascii_case(PERPLEXITY_NAME) => {
            Err(Error::BadResponse {
                endpoint: endpoint.to_owned(),
                detail: format!("asked for provider {PERPLEXITY_NAME} only, but `{p}` answered"),
            })
        }
        _ => Ok(()),
    }
}

/// A non-2xx reply as the error that names it: 402, 404 and 429 each carry OpenRouter's own reason;
/// anything else is the generic [`Error::Refused`] with the body quoted.
pub(crate) fn refusal_of(endpoint: &str, reply: Reply) -> Error {
    let reason = reason_of(&reply.body);
    let named = |failure| Error::OpenRouter { endpoint: endpoint.to_owned(), failure, reason: reason.clone() };
    match reply.status {
        402 => named(OpenRouterFailure::CreditsExhausted),
        404 => named(OpenRouterFailure::NoEndpoint),
        429 => named(OpenRouterFailure::RateLimited { retry_after: reply.retry_after }),
        status => Error::Refused { endpoint: endpoint.to_owned(), status, body: reply.body.chars().take(MAX_QUOTED_BODY_CHARS).collect() },
    }
}

/// OpenRouter's `error.message`, with `error.metadata` (`limit_source`, `provider_name`, `raw`...) after it
/// when there is any; the body itself (cut short) when it is not that shape.
fn reason_of(body: &str) -> String {
    let error = serde_json::from_str::<Value>(body).ok().and_then(|v| v.get("error").cloned());
    let Some(error) = error else {
        return body.chars().take(MAX_QUOTED_BODY_CHARS).collect();
    };
    let message = error.get("message").and_then(Value::as_str).unwrap_or("(no message)");
    match error.get("metadata").filter(|m| !m.is_null()) {
        Some(metadata) => {
            let metadata = metadata.to_string();
            format!("{message} (metadata: {})", metadata.chars().take(MAX_QUOTED_BODY_CHARS).collect::<String>())
        }
        None => message.to_owned(),
    }
}

#[derive(Deserialize)]
struct WireResponse {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    answers: BTreeMap<String, Value>,
    #[serde(default)]
    usage: Option<WireUsage>,
}

#[derive(Deserialize, Default)]
struct WireUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
    #[serde(default)]
    cost: Option<f64>,
}

/// Read a 2xx body. The extra fields (`id`, `provider`, `usage.cost`) are kept in [`Usage`]. Every answer
/// is typed: `noul`, `choice` and `score` parse; `refusal` becomes [`Answer::Refusal`] (which
/// [`ask`](crate::ask) turns into an error); any other `type` (OpenRouter's `box` among them), a missing
/// `type` or a malformed answer is an [`Error::BadResponse`] naming the question.
pub(crate) fn parse_response(endpoint: &str, body: &str) -> Result<Response, Error> {
    let bad = |detail: String| Error::BadResponse { endpoint: endpoint.to_owned(), detail };
    let wire: WireResponse = serde_json::from_str(body).map_err(|e| bad(e.to_string()))?;
    let mut answers = BTreeMap::new();
    for (question, value) in wire.answers {
        let kind = value.get("type").and_then(Value::as_str).map(str::to_owned);
        let answer = match kind.as_deref() {
            Some("refusal") => Answer::Refusal,
            Some(kind @ ("noul" | "choice" | "score")) => serde_json::from_value(value)
                .map_err(|e| bad(format!("question `{question}` has a malformed {kind} answer: {e}")))?,
            Some(other) => return Err(bad(format!("question `{question}` was answered with the unexpected type `{other}`"))),
            None => return Err(bad(format!("question `{question}` has an answer with no `type`"))),
        };
        answers.insert(question, answer);
    }
    let usage = wire.usage.unwrap_or_default();
    let usage = Usage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cost: usage.cost,
        provider: wire.provider,
        id: wire.id,
    };
    Ok(Response { model: wire.model, answers, usage: Some(usage) })
}
