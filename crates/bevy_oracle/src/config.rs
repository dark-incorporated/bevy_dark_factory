use crate::Error;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The wire protocol a run speaks. **One protocol per run**: a request is sent in exactly one of them,
/// and a failure is never retried in the other. It is not a setting of its own: the person's
/// [`DecisionModel`] chooses it ([`DecisionModel::protocol`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Protocol {
    /// System One, `POST /v1/systemone`: a llama.cpp server serving d3-flash on the LAN.
    #[default]
    SystemOne,
    /// OpenRouter Decisions, `POST /api/alpha/decisions`: images ride in the `state` array, and a
    /// `provider` block pins who may see them. See `docs/decide/openrouter.md`.
    OpenRouter,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::SystemOne => f.write_str("systemone"),
            Protocol::OpenRouter => f.write_str("openrouter"),
        }
    }
}

impl Protocol {
    /// **Where a request in this protocol goes**: System One is the local model's, OpenRouter the remote's. A [`Config`]
    /// is built from the person's choice ([`Endpoints::config`]), so the protocol is the choice, and a transport tells an
    /// answer where it ran by it rather than by comparing addresses (a LAN address chosen as Local is `local`).
    pub fn site(self) -> DecisionModel {
        match self {
            Protocol::SystemOne => DecisionModel::Local,
            Protocol::OpenRouter => DecisionModel::Remote,
        }
    }
}

/// **Which decision model the person chose** (DM-03, #454): the one setting every surface reads, in the editor's
/// Settings and in the `decision_model` key of a settings file.
///
/// **One endpoint per run, no fallback chain.** `Local` asks d3-flash on the LAN and nothing leaves it; `Remote` asks
/// Perplexity's model through OpenRouter, and **screenshots of the project go there**. Remote is the person's own
/// choice, made in their own settings file only ([`crate::decision_model`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionModel {
    #[default]
    Local,
    Remote,
}

impl DecisionModel {
    /// The word the answer's `where` says and the settings file spells: `local` or `remote`.
    pub fn where_ran(self) -> &'static str {
        match self {
            DecisionModel::Local => "local",
            DecisionModel::Remote => "remote",
        }
    }

    /// The wire protocol this choice speaks.
    pub fn protocol(self) -> Protocol {
        match self {
            DecisionModel::Local => Protocol::SystemOne,
            DecisionModel::Remote => Protocol::OpenRouter,
        }
    }

    /// **The model's name as a person reads it**, before any answer has named it: the status LED's word.
    pub fn name(self) -> &'static str {
        match self {
            DecisionModel::Local => LOCAL_MODEL_NAME,
            DecisionModel::Remote => OPENROUTER_MODEL.split_once('/').map_or(OPENROUTER_MODEL, |(_, name)| name),
        }
    }
}

impl fmt::Display for DecisionModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.where_ran())
    }
}

/// Where decisions are asked. **One endpoint, one protocol per run**: there is no fallback chain. A `Config` is
/// built from the person's [`DecisionModel`] by [`Endpoints::config`]; nothing reads it from the environment.
///
/// `Debug` shows whether a key is set, never the key.
#[derive(Clone, PartialEq)]
pub struct Config {
    /// Which wire protocol [`endpoint`](Self::endpoint) speaks.
    pub protocol: Protocol,
    /// The full URL a request is POSTed to.
    pub endpoint: String,
    /// Sent as `model` when set. A llama.cpp server has one model and needs none; OpenRouter
    /// always gets one ([`OPENROUTER_MODEL`]).
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

/// d3-flash on this LAN, the same endpoint from every editor: big's llama-swap (`:8081`) starts the model
/// on the first request and proxies `/v1/systemone` through `/upstream/d3-flash`. `big.local` is the name
/// every LAN host resolves (mDNS); a bare `big` resolves only on big itself. `docs/decide/serving-d3-flash.md`.
pub const LOCAL_ENDPOINT: &str = "http://big.local:8081/upstream/d3-flash/v1/systemone";

/// What the local model is called before an answer has named it.
pub const LOCAL_MODEL_NAME: &str = "d3-flash";

/// OpenRouter's Decisions endpoint (alpha: the shape may change).
pub const OPENROUTER_ENDPOINT: &str = "https://openrouter.ai/api/alpha/decisions";

/// The remote decision model: Perplexity's, on OpenRouter's free tier (20 requests/min).
pub const OPENROUTER_MODEL: &str = "perplexity/pplx-decider-v1.3:free";

/// The environment variable holding the OpenRouter key. **It is not a setting**: read from the process environment
/// only, never from a settings file, a project's `.env` or omp's database, and never logged or written.
pub const OPENROUTER_KEY_VAR: &str = "OPENROUTER_API_KEY";

/// How long a request waits for an answer, in seconds.
pub const TIMEOUT_SECS: u64 = 60;

/// **The sentence Remote is refused with when no key is set**: it names the variable, says nothing was sent and
/// that the local model was not asked instead. The Settings row says the same words before a decision is asked.
pub const REMOTE_WITHOUT_KEY: &str = "Remote is chosen but OPENROUTER_API_KEY is not set in the environment, so no decision was asked. Nothing was sent to the local model instead: set the variable and restart, or choose Local in Settings";

impl Default for Config {
    fn default() -> Self {
        Config { protocol: Protocol::SystemOne, endpoint: LOCAL_ENDPOINT.into(), model: None, key: None, timeout_secs: TIMEOUT_SECS }
    }
}

/// **Where each value of the [`DecisionModel`] setting goes, and the key Remote needs.** A value, so a test points
/// the two at stub servers and no process environment is involved; [`Endpoints::from_env`] is what the editor and the MCP
/// server use.
///
/// `Debug` shows whether a key is set, never the key.
#[derive(Clone, PartialEq)]
pub struct Endpoints {
    /// The System One endpoint `Local` asks.
    pub local: String,
    /// The OpenRouter Decisions endpoint `Remote` asks.
    pub remote: String,
    /// `OPENROUTER_API_KEY`, when the environment has one.
    pub key: Option<String>,
}

impl fmt::Debug for Endpoints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Endpoints")
            .field("local", &self.local)
            .field("remote", &self.remote)
            .field("key", &self.key.as_ref().map(|_| "<set>"))
            .finish()
    }
}

impl Default for Endpoints {
    /// The shipped endpoints and no key.
    fn default() -> Self {
        Endpoints { local: LOCAL_ENDPOINT.into(), remote: OPENROUTER_ENDPOINT.into(), key: None }
    }
}

impl Endpoints {
    /// The shipped endpoints with the key `get` finds for [`OPENROUTER_KEY_VAR`] (an empty value is unset).
    pub fn with_key_from(get: impl Fn(&str) -> Option<String>) -> Self {
        Endpoints { key: get(OPENROUTER_KEY_VAR).filter(|k| !k.trim().is_empty()), ..Endpoints::default() }
    }

    /// The shipped endpoints with the key from the process environment: the only thing the environment says.
    pub fn from_env() -> Self {
        Endpoints::with_key_from(|k| std::env::var(k).ok())
    }

    /// **The configuration of one choice.** `Local` is System One at [`Endpoints::local`] with no key; `Remote` is
    /// OpenRouter at [`Endpoints::remote`] with [`OPENROUTER_MODEL`] and the key, and **with no key it is refused**
    /// ([`Error::RemoteWithoutKey`]): never answered by the local model instead.
    pub fn config(&self, choice: DecisionModel) -> Result<Config, Error> {
        match choice {
            DecisionModel::Local => Ok(Config { endpoint: self.local.clone(), ..Config::default() }),
            DecisionModel::Remote => {
                let key = self.key.clone().ok_or(Error::RemoteWithoutKey)?;
                Ok(Config {
                    protocol: Protocol::OpenRouter,
                    endpoint: self.remote.clone(),
                    model: Some(OPENROUTER_MODEL.to_owned()),
                    key: Some(key),
                    timeout_secs: TIMEOUT_SECS,
                })
            }
        }
    }
}
