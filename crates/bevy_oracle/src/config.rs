use std::fmt;

/// The wire protocol a run speaks. **One protocol per run**: a request is sent in exactly one of them,
/// and a failure is never retried in the other.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Protocol {
    /// System One, `POST /v1/systemone`: a llama.cpp server (clef-flash, d3-flash) or Cloudflare.
    #[default]
    SystemOne,
    /// OpenRouter Decisions, `POST /api/alpha/decisions`: images ride in the `state` array, and a
    /// `provider` block pins who may see them. See `docs/decide/openrouter.md`.
    OpenRouter,
    /// `GRIMOIRE_DECIDE_PROTOCOL` named something else. It is kept, not defaulted to System One, so the
    /// first request fails loudly naming the value: a typo must never quietly send screenshots to the
    /// other endpoint kind.
    Unrecognised(String),
}

impl Protocol {
    /// Read the value of `GRIMOIRE_DECIDE_PROTOCOL` (case-insensitive, trimmed).
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "systemone" => Protocol::SystemOne,
            "openrouter" => Protocol::OpenRouter,
            _ => Protocol::Unrecognised(value.trim().to_owned()),
        }
    }
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::SystemOne => f.write_str("systemone"),
            Protocol::OpenRouter => f.write_str("openrouter"),
            Protocol::Unrecognised(v) => write!(f, "{v}"),
        }
    }
}

/// Where decisions are asked. **One endpoint, one protocol per run**: there is no fallback chain.
///
/// Local clef-flash or d3-flash over System One is the default. Clef 27B (bmb), Cloudflare Workers AI
/// and OpenRouter are values of the same fields. **Cloudflare and OpenRouter send screenshots off the
/// LAN, so they are opt-in:** only by setting `GRIMOIRE_DECIDE_PROTOCOL=openrouter` (or an endpoint).
///
/// `Debug` shows whether a key is set, never the key.
#[derive(Clone, PartialEq)]
pub struct Config {
    /// Which wire protocol [`endpoint`](Self::endpoint) speaks.
    pub protocol: Protocol,
    /// The full URL a request is POSTed to.
    pub endpoint: String,
    /// Sent as `model` when set. A llama.cpp server has one model and needs none; OpenRouter
    /// always gets one ([`OPENROUTER_MODEL`] unless named).
    pub model: Option<String>,
    /// Sent as a bearer token when set. For [`Protocol::OpenRouter`] this is `OPENROUTER_API_KEY`
    /// from the process environment, nothing else.
    pub key: Option<String>,
    pub timeout_secs: u64,
}

impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("protocol", &self.protocol)
            .field("endpoint", &self.endpoint)
            .field("model", &self.model)
            .field("key", &self.key.as_ref().map(|_| "<set>"))
            .field("timeout_secs", &self.timeout_secs)
            .finish()
    }
}

/// clef-flash on this LAN: `tools/decide/serve-clef-flash.sh` on big.
pub const LOCAL_ENDPOINT: &str = "http://127.0.0.1:8093/v1/systemone";

/// OpenRouter's Decisions endpoint (alpha: the shape may change).
pub const OPENROUTER_ENDPOINT: &str = "https://openrouter.ai/api/alpha/decisions";

/// The remote decision model: Perplexity's, on OpenRouter's free tier (20 requests/min).
pub const OPENROUTER_MODEL: &str = "perplexity/pplx-decider-v1.3:free";

/// The environment variable holding the OpenRouter key. Read from the process environment only: never
/// from a project's `.env`, a settings file or omp's database, and never logged or written.
pub const OPENROUTER_KEY_VAR: &str = "OPENROUTER_API_KEY";

/// Which protocol the run speaks.
pub const PROTOCOL_VAR: &str = "GRIMOIRE_DECIDE_PROTOCOL";

impl Default for Config {
    fn default() -> Self {
        Config {
            protocol: Protocol::SystemOne,
            endpoint: LOCAL_ENDPOINT.into(),
            model: None,
            key: None,
            timeout_secs: 60,
        }
    }
}

impl Config {
    /// `GRIMOIRE_DECIDE_PROTOCOL` (`systemone`, the default, or `openrouter`), `GRIMOIRE_DECIDE_ENDPOINT`,
    /// `GRIMOIRE_DECIDE_MODEL`, `GRIMOIRE_DECIDE_KEY`, `GRIMOIRE_DECIDE_TIMEOUT_SECS` over the local
    /// default; `get` is the environment, so tests pass a map.
    ///
    /// With `openrouter` the endpoint defaults to [`OPENROUTER_ENDPOINT`], the model to
    /// [`OPENROUTER_MODEL`], and the key is `OPENROUTER_API_KEY` (`GRIMOIRE_DECIDE_KEY` is not read:
    /// one variable per protocol, so a Cloudflare token is never sent to OpenRouter).
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Self {
        let d = Config::default();
        let set = |k: &str| get(k).filter(|v| !v.trim().is_empty());
        let protocol = set(PROTOCOL_VAR).map(|v| Protocol::parse(&v)).unwrap_or_default();
        let remote = protocol == Protocol::OpenRouter;
        Config {
            endpoint: set("GRIMOIRE_DECIDE_ENDPOINT")
                .unwrap_or_else(|| if remote { OPENROUTER_ENDPOINT.into() } else { d.endpoint.clone() }),
            model: set("GRIMOIRE_DECIDE_MODEL").or_else(|| remote.then(|| OPENROUTER_MODEL.to_owned())),
            key: if remote { set(OPENROUTER_KEY_VAR) } else { set("GRIMOIRE_DECIDE_KEY") },
            timeout_secs: set("GRIMOIRE_DECIDE_TIMEOUT_SECS").and_then(|v| v.parse().ok()).unwrap_or(d.timeout_secs),
            protocol,
        }
    }
    pub fn from_env() -> Self {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    /// The environment first, then a project's own file (`env` and `file` are lookups, so tests pass
    /// maps). **`OPENROUTER_API_KEY` is read from `env` alone**: a key must never come from a file a
    /// project carries, and so never from a repository. An empty value counts as unset.
    pub fn from_env_then_file(
        env: impl Fn(&str) -> Option<String>,
        file: impl Fn(&str) -> Option<String>,
    ) -> Self {
        Self::from_lookup(|k| {
            let from_env = env(k).filter(|v| !v.is_empty());
            if k == OPENROUTER_KEY_VAR { from_env } else { from_env.or_else(|| file(k)) }
        })
    }
}
