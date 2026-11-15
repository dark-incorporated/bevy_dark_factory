use crate::{Config, DecisionModel, Error, OpenRouterDecisions, Protocol, Request, Response};
use std::path::{Path, PathBuf};

/// Something that answers a [`Request`]. [`Live`] for a live endpoint, [`Replay`] for a recorded one.
pub trait Transport {
    fn endpoint(&self) -> String;

    /// **Where this transport's answers are made**: [`DecisionModel::Local`] when nothing leaves the LAN,
    /// [`DecisionModel::Remote`] when pictures may have. `grimoire_decide` says it in its answer's `where`.
    ///
    /// It is the person's choice, not a guess from an address, so a LAN address chosen as Local reads `local`. **The
    /// default is Remote**: a transport that does not say is read as the one that may have sent pictures away, never
    /// the other way round.
    fn site(&self) -> DecisionModel {
        DecisionModel::Remote
    }

    fn send(&self, request: &Request) -> Result<Response, Error>;
}

/// A System One endpoint over HTTP (`POST /v1/systemone`). It speaks no other protocol: build the
/// transport for a run with [`Live::new`], which picks by [`Config::protocol`].
pub struct Http {
    pub config: Config,
}

impl Http {
    pub fn new(config: Config) -> Self {
        Http { config }
    }
}

/// What a POST came back with, before anyone decided what it means.
pub(crate) struct Reply {
    pub status: u16,
    /// The `Retry-After` header as sent (seconds or an HTTP date), if any.
    pub retry_after: Option<String>,
    pub body: String,
}

/// POST `body` as JSON to the config's endpoint, with the bearer token when one is set. **No retry**:
/// whatever the endpoint says comes back as it was said. Non-2xx statuses are a [`Reply`], not an error,
/// because the endpoint's own complaint is in the body.
pub(crate) fn post_json(config: &Config, body: &impl serde::Serialize) -> Result<Reply, Error> {
    let endpoint = &config.endpoint;
    let unreachable = |e: ureq::Error| Error::Unreachable { endpoint: endpoint.clone(), detail: e.to_string() };
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(std::time::Duration::from_secs(config.timeout_secs)))
        .http_status_as_error(false)
        .build()
        .into();
    let mut req = agent.post(endpoint);
    if let Some(key) = &config.key {
        req = req.header("Authorization", &format!("Bearer {key}"));
    }
    let mut resp = req.send_json(body).map_err(unreachable)?;
    let status = resp.status().as_u16();
    let retry_after = resp.headers().get("retry-after").and_then(|v| v.to_str().ok()).map(str::to_owned);
    let body = resp.body_mut().read_to_string().map_err(unreachable)?;
    Ok(Reply { status, retry_after, body })
}

impl Transport for Http {
    fn endpoint(&self) -> String {
        self.config.endpoint.clone()
    }

    fn site(&self) -> DecisionModel {
        self.config.protocol.site()
    }

    fn send(&self, request: &Request) -> Result<Response, Error> {
        if self.config.protocol != Protocol::SystemOne {
            return Err(Error::Invalid(
                "`Http` speaks systemone only, but the config's protocol is `openrouter`; one protocol per run: build the transport with `Live::new`".into(),
            ));
        }
        let endpoint = self.config.endpoint.clone();
        let mut body = request.clone();
        if body.model.is_none() {
            body.model = self.config.model.clone();
        }
        let reply = post_json(&self.config, &body)?;
        if !(200..300).contains(&reply.status) {
            return Err(Error::Refused { endpoint, status: reply.status, body: reply.body.chars().take(500).collect() });
        }
        serde_json::from_str(&reply.body).map_err(|e| Error::BadResponse { endpoint, detail: e.to_string() })
    }
}

/// The live transport a [`Config`] names: **one protocol per run, chosen by the config, never tried in
/// turn**. Callers build this instead of [`Http`] or [`OpenRouterDecisions`] so that the person's
/// `decision_model` setting is honoured everywhere.
pub enum Live {
    SystemOne(Http),
    OpenRouter(OpenRouterDecisions),
}

impl Live {
    pub fn new(config: Config) -> Self {
        match config.protocol {
            Protocol::OpenRouter => Live::OpenRouter(OpenRouterDecisions::new(config)),
            Protocol::SystemOne => Live::SystemOne(Http::new(config)),
        }
    }
}

impl Transport for Live {
    fn endpoint(&self) -> String {
        match self {
            Live::SystemOne(t) => t.endpoint(),
            Live::OpenRouter(t) => t.endpoint(),
        }
    }
    fn site(&self) -> DecisionModel {
        match self {
            Live::SystemOne(t) => t.site(),
            Live::OpenRouter(t) => t.site(),
        }
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        match self {
            Live::SystemOne(t) => t.send(request),
            Live::OpenRouter(t) => t.send(request),
        }
    }
}

/// A recorded exchange, answered with no network. It answers only the request it was recorded for:
/// a different request is an error, never the recorded answer.
pub struct Replay {
    request: Request,
    response: Response,
}

impl Replay {
    pub fn new(request: Request, response: Response) -> Self {
        Replay { request, response }
    }

    /// Read `request.json` and `response.json` from a fixture directory.
    pub fn load(dir: &Path) -> Result<Self, Error> {
        fn read<T: serde::de::DeserializeOwned>(p: PathBuf) -> Result<T, Error> {
            let text = std::fs::read_to_string(&p).map_err(|e| Error::Fixture(format!("{}: {e}", p.display())))?;
            serde_json::from_str(&text).map_err(|e| Error::Fixture(format!("{}: {e}", p.display())))
        }
        Ok(Replay { request: read(dir.join("request.json"))?, response: read(dir.join("response.json"))? })
    }
}

impl Transport for Replay {
    fn endpoint(&self) -> String {
        "replay".into()
    }
    /// A recording makes no request at all, so nothing leaves the machine.
    fn site(&self) -> DecisionModel {
        DecisionModel::Local
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        if *request != self.request {
            return Err(Error::Fixture("the request is not the one this fixture was recorded for".into()));
        }
        Ok(self.response.clone())
    }
}

/// Wraps a live transport and writes each exchange to `dir` as `request.json` and `response.json`
/// (the last exchange wins). Used by smoke runs that make a new fixture.
pub struct Recorder<T: Transport> {
    inner: T,
    dir: PathBuf,
}

impl<T: Transport> Recorder<T> {
    pub fn new(inner: T, dir: impl Into<PathBuf>) -> Self {
        Recorder { inner, dir: dir.into() }
    }
}

impl<T: Transport> Transport for Recorder<T> {
    fn endpoint(&self) -> String {
        self.inner.endpoint()
    }
    fn site(&self) -> DecisionModel {
        self.inner.site()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        let response = self.inner.send(request)?;
        let write = |name: &str, v: String| {
            std::fs::create_dir_all(&self.dir)
                .and_then(|_| std::fs::write(self.dir.join(name), v))
                .map_err(|e| Error::Fixture(format!("{}: {e}", self.dir.join(name).display())))
        };
        let to_s = |e: serde_json::Error| Error::Fixture(e.to_string());
        let r = write("request.json", serde_json::to_string_pretty(request).map_err(to_s)?)
            .and_then(|_| write("response.json", serde_json::to_string_pretty(&response).map_err(to_s)?));
        r?;
        Ok(response)
    }
}
