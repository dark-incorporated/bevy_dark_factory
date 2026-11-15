use crate::{Config, Error, Request, Response};
use std::path::{Path, PathBuf};

/// Something that answers a [`Request`]. [`Http`] for a live endpoint, [`Replay`] for a recorded one.
pub trait Transport {
    fn endpoint(&self) -> String;
    fn send(&self, request: &Request) -> Result<Response, Error>;
}

/// A live endpoint over HTTP.
pub struct Http {
    pub config: Config,
}

impl Http {
    pub fn new(config: Config) -> Self {
        Http { config }
    }
}

impl Http {
    /// **The models this endpoint says it serves** (`GET /v1/models`, System One's sibling path on the
    /// same host: llama.cpp answers `{"data":[{"id": <alias>}]}`). A calibration run reads it before the
    /// first sample, so a record is never labelled with a model the endpoint is not serving (#455).
    pub fn served_models(&self) -> Result<Vec<String>, Error> {
        let endpoint = self.config.endpoint.clone();
        let url = match endpoint.rsplit_once('/') {
            Some((base, _)) => format!("{base}/models"),
            None => {
                return Err(Error::BadResponse { endpoint, detail: "the endpoint has no path to take /models from".into() });
            }
        };
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .timeout_global(Some(std::time::Duration::from_secs(self.config.timeout_secs)))
            .http_status_as_error(false)
            .build()
            .into();
        let mut req = agent.get(&url);
        if let Some(key) = &self.config.key {
            req = req.header("Authorization", &format!("Bearer {key}"));
        }
        let mut resp =
            req.call().map_err(|e| Error::Unreachable { endpoint: endpoint.clone(), detail: e.to_string() })?;
        let status = resp.status().as_u16();
        let text = resp
            .body_mut()
            .read_to_string()
            .map_err(|e| Error::Unreachable { endpoint: endpoint.clone(), detail: e.to_string() })?;
        if !(200..300).contains(&status) {
            return Err(Error::Refused { endpoint: url, status, body: text.chars().take(500).collect() });
        }
        #[derive(serde::Deserialize)]
        struct Model {
            id: String,
        }
        #[derive(serde::Deserialize)]
        struct List {
            data: Vec<Model>,
        }
        let list: List = serde_json::from_str(&text)
            .map_err(|e| Error::BadResponse { endpoint, detail: format!("{url}: {e}") })?;
        Ok(list.data.into_iter().map(|m| m.id).collect())
    }
}

/// Refuse a calibration labelled `label` when the endpoint's served models (see [`Http::served_models`])
/// do not include it.
pub fn verify_served(label: &str, endpoint: &str, served: &[String]) -> Result<(), Error> {
    if served.iter().any(|m| m == label) {
        Ok(())
    } else {
        Err(Error::ModelMismatch { endpoint: endpoint.to_owned(), label: label.to_owned(), served: served.to_vec() })
    }
}

/// **Refuses an answer from any model but the one a run is labelled with** (#455). Wraps the run's
/// transport: a response that names a different model is an [`Error::ModelMismatch`], never a sample
/// scored into a record labelled for another model. A response that names none passes: it is the
/// preflight ([`verify_served`]) that vouches for such an endpoint.
pub struct Expecting<'a> {
    inner: &'a dyn Transport,
    model: &'a str,
}

impl<'a> Expecting<'a> {
    pub fn new(inner: &'a dyn Transport, model: &'a str) -> Self {
        Expecting { inner, model }
    }
}

impl Transport for Expecting<'_> {
    fn endpoint(&self) -> String {
        self.inner.endpoint()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        let response = self.inner.send(request)?;
        match response.model.as_deref() {
            Some(answered) if answered != self.model => Err(Error::ModelMismatch {
                endpoint: self.inner.endpoint(),
                label: self.model.to_owned(),
                served: vec![answered.to_owned()],
            }),
            _ => Ok(response),
        }
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
