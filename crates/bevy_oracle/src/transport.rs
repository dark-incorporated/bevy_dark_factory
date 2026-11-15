use crate::{Config, Error, Request, Response};
use std::path::{Path, PathBuf};

/// Something that answers a [`Request`]. [`Http`] for a live endpoint, [`Replay`] for a recorded one.
pub trait Transport {
    fn endpoint(&self) -> String;
    fn send(&self, request: &Request) -> Result<Response, Error>;
}

/// What a gateway in front of the model answers when it is up but cannot run the model: 502, 503 and 504
/// from any proxy, and the 500 llama-swap itself sends (`"src":"llama-swap"`) when the model's launch script
/// exits, as `tools/decide/serve-d3-flash.sh` does while a GPU lease holds the card (measured 2026-11-15:
/// HTTP 500 in 0.25 s, `{"error":"unspecific error: upstream command exited prematurely","src":"llama-swap"}`).
/// The request was not refused; nothing served it, so it is [`Error::Unreachable`], not [`Error::Refused`].
const GATEWAY_STATUSES: [u16; 3] = [502, 503, 504];

/// The marker llama-swap puts in its own error bodies, to tell its 500 from the model server's.
const LLAMA_SWAP_MARK: &str = "\"src\":\"llama-swap\"";

fn gateway_cannot_serve(status: u16, body: &str) -> bool {
    GATEWAY_STATUSES.contains(&status) || (status == 500 && body.contains(LLAMA_SWAP_MARK))
}

/// What to do about [`GATEWAY_STATUSES`] on big's local endpoint (`docs/decide/serving-d3-flash.md`): the
/// model is evicted while a GPU lease or the coding model holds the card, and it loads again on the first
/// request after the card is free and home-still's olmocr is back, so the remedy is to look at who holds the
/// card and ask again after.
const GATEWAY_REMEDY: &str = "the server is up but its model is not loaded: a GPU lease or the coding model \
    evicts it, and it reloads on the first request after the card is free and home-still's olmocr is back (a \
    minute or two); on big, `gpu-tenant status` names the holder and `gpu-tenant board -n 10` says since when";

/// A live endpoint over HTTP.
pub struct Http {
    pub config: Config,
}

impl Http {
    pub fn new(config: Config) -> Self {
        Http { config }
    }
}

impl Transport for Http {
    fn endpoint(&self) -> String {
        self.config.endpoint.clone()
    }

    fn send(&self, request: &Request) -> Result<Response, Error> {
        let endpoint = self.config.endpoint.clone();
        let mut body = request.clone();
        if body.model.is_none() {
            body.model = self.config.model.clone();
        }
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(self.config.timeout_secs)))
            // `big.local` also resolves to IPv6 addresses big's firewall drops: 56.9 s of the 60 s timeout.
            .ip_family(ureq::config::IpFamily::Ipv4Only)
            // A refusal carries the endpoint's own complaint in its body.
            .http_status_as_error(false)
            .build()
            .into();
        let mut req = agent.post(&endpoint);
        if let Some(key) = &self.config.key {
            req = req.header("Authorization", &format!("Bearer {key}"));
        }
        let mut resp = req
            .send_json(&body)
            .map_err(|e| Error::Unreachable { endpoint: endpoint.clone(), detail: e.to_string() })?;
        let status = resp.status().as_u16();
        let text = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| Error::Unreachable { endpoint: endpoint.clone(), detail: e.to_string() })?;
        if gateway_cannot_serve(status, &text) {
            let said: String = text.chars().take(500).collect();
            return Err(Error::Unreachable { endpoint, detail: format!("status {status}: {said}; {GATEWAY_REMEDY}") });
        }
        if !(200..300).contains(&status) {
            return Err(Error::Refused { endpoint, status, body: text.chars().take(500).collect() });
        }
        serde_json::from_str(&text).map_err(|e| Error::BadResponse { endpoint, detail: e.to_string() })
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
