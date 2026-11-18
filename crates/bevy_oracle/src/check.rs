use crate::{Calibration, Error, Question, Request, Transport, ask};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The editor's own checks, as shipped (`fixtures/checks.yaml`). A project's `checks.yaml` has the same shape.
pub const BUILTIN_CHECKS_YAML: &str = include_str!("../fixtures/checks.yaml");

/// The project's own checks, beside `kits.yaml` and `rules.yaml`.
pub const PROJECT_CHECKS_FILE: &str = "checks.yaml";

/// A named yes/no question about what an operation changed, with what it needs and what it earned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub name: String,
    /// The yes/no question: yes means something is wrong.
    pub question: String,
    /// Names of the images it needs (`before`, `after`, `closeup`, `view`, ...), in the order sent.
    #[serde(default)]
    pub images: Vec<String>,
    /// Whether it needs the JSON facts (operations run, pieces, camera).
    #[serde(default)]
    pub facts: bool,
    /// P(yes) at or above this is a finding **for a model this check has no calibration for**: the
    /// author's default. A model with a record in [`Check::calibrations`] uses the threshold it earned.
    pub threshold: f64,
    /// Run after each agent operation (never while a person is editing).
    #[serde(default)]
    pub after_edit: bool,
    /// **What a labelled set of real captures measured, one record per model** (#455). A calibration
    /// belongs to the model whose probabilities it was measured on: another model's P(yes) is on another
    /// scale, so a record is never lent to a model it does not name (`Calibration::model`).
    #[serde(default)]
    pub calibrations: Vec<Calibration>,
}

/// The format version of `checks.yaml` (#202). A file with no `version` is the first.
///
/// This crate has no `blueprint_model` dependency (it is engine-free and no game links it), so the two
/// small functions below say what `blueprint_model::format_version` says for the formats core owns.
pub const CHECKS_VERSION: u32 = 1;

/// The version of a file from before the version existed: the first.
fn first_version() -> u32 {
    1
}

/// A `version` that must be [`CHECKS_VERSION`]: any other number is refused, saying which version this
/// build reads, so an author knows a file from a newer release is not damaged.
fn read_version<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let found = u32::deserialize(deserializer)?;
    if found == CHECKS_VERSION {
        Ok(found)
    } else {
        Err(serde::de::Error::custom(format!("version {found} is not the {CHECKS_VERSION} this build reads")))
    }
}

/// The `checks.yaml` file: checks by name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checks {
    /// The format version: [`CHECKS_VERSION`]. A file from before it existed has none and reads as the
    /// first; a file of another version is refused, naming the one this build reads.
    #[serde(default = "first_version", deserialize_with = "read_version")]
    pub version: u32,
    pub checks: Vec<Check>,
}

impl Default for Checks {
    fn default() -> Self {
        Checks { version: CHECKS_VERSION, checks: Vec::new() }
    }
}

impl Checks {
    /// **Read a `checks.yaml`**, the one reader this kind has since the cutover (GD-03, #365).
    pub fn from_yaml(text: &str) -> Result<Self, Error> {
        // **The one-calibration form is refused, not ignored** (#455): a `calibration:` that no field reads
        // would quietly strip the check of the authority it earned. An empty one (`null`) says nothing.
        let raw: serde_yaml_ng::Value = serde_yaml_ng::from_str(text).map_err(|e| Error::Checks(e.to_string()))?;
        let stale = raw
            .get("checks")
            .and_then(|c| c.as_sequence())
            .into_iter()
            .flatten()
            .find(|c| c.get("calibration").is_some_and(|v| !v.is_null()))
            .map(|c| c.get("name").and_then(|n| n.as_str()).unwrap_or("?"));
        if let Some(name) = stale {
            return Err(Error::Checks(format!(
                "check `{name}` has a `calibration`; a calibration belongs to one model, so the field is now \
                 `calibrations`, a list with one record per model"
            )));
        }
        let c: Checks = serde_yaml_ng::from_str(text).map_err(|e| Error::Checks(e.to_string()))?;
        let mut seen = std::collections::BTreeSet::new();
        for check in &c.checks {
            if !seen.insert(check.name.as_str()) {
                return Err(Error::Checks(format!("two checks are named `{}`", check.name)));
            }
            if !(0.0..=1.0).contains(&check.threshold) {
                return Err(Error::Checks(format!("check `{}`: threshold {} is outside 0..=1", check.name, check.threshold)));
            }
            if check.images.len() > crate::MAX_IMAGES {
                return Err(Error::Checks(format!("check `{}` needs more than {} images", check.name, crate::MAX_IMAGES)));
            }
        }
        Ok(c)
    }

    /// The file as the one YAML writer writes it.
    pub fn to_yaml(&self) -> Result<String, Error> {
        serde_yaml_ng::to_string(self).map_err(|e| Error::Checks(e.to_string()))
    }

    /// The editor's own checks.
    pub fn builtin() -> Self {
        Self::from_yaml(BUILTIN_CHECKS_YAML).expect("fixtures/checks.yaml parses (a test reads it)")
    }

    /// The editor's checks with the project's own `assets/blueprint/checks.yaml` over them, read fresh so an
    /// edit takes effect at once. No file is the editor's checks alone; an unreadable or invalid one is an error
    /// naming the path.
    pub fn for_project(project: &std::path::Path) -> Result<Self, String> {
        let path = project.join("assets").join("blueprint").join(PROJECT_CHECKS_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => Ok(Self::builtin().merged(Self::from_yaml(&text).map_err(|e| format!("{}: {e}", path.display()))?)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::builtin()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Check> {
        self.checks.iter().find(|c| c.name == name)
    }

    /// `other`'s checks over these: a game's check of the same name replaces the editor's.
    pub fn merged(mut self, other: Checks) -> Self {
        for c in other.checks {
            match self.checks.iter_mut().find(|m| m.name == c.name) {
                Some(slot) => *slot = c,
                None => self.checks.push(c),
            }
        }
        self
    }
}

/// What a check is run on: the captures by name (data URLs) and the JSON facts.
#[derive(Debug, Clone, Default)]
pub struct Evidence {
    pub images: BTreeMap<String, String>,
    pub facts: Option<serde_json::Value>,
}

/// One check's result. A finding is advisory unless its check earned the right to block **for the model
/// that answered**.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub check: String,
    pub p_yes: f64,
    /// The threshold applied: the answering model's calibrated one, else the check's own default.
    pub threshold: f64,
    /// `p_yes >= threshold`.
    pub found: bool,
    /// The model that answered, as the response names it. `None` when the endpoint did not say, which no
    /// record can be matched to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The agreement this check measured **for that model**; `None` when it is not calibrated for it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agreement: Option<f64>,
}

impl Finding {
    /// "floating 0.93 > 0.70", as the journal writes it; `None` when nothing was found.
    pub fn message(&self) -> Option<String> {
        self.found.then(|| format!("{} {:.2} > {:.2}", self.check, self.p_yes, self.threshold))
    }

    /// Whether the check is calibrated for the model that answered.
    pub fn is_calibrated(&self) -> bool {
        self.agreement.is_some()
    }

    /// `uncalibrated for <model>` when this check has no record for the model that answered, else `None`.
    pub fn uncalibrated_note(&self) -> Option<String> {
        (!self.is_calibrated()).then(|| match &self.model {
            Some(m) => format!("uncalibrated for {m}"),
            None => "uncalibrated for an unnamed model".to_owned(),
        })
    }

    /// **Whether this finding may fail the loop's gate**: it found something, and its check earned
    /// agreement of at least `bar` on the model that answered. An uncalibrated finding never does.
    pub fn is_blocking(&self, bar: f64) -> bool {
        self.found && self.agreement.is_some_and(|a| a >= bar)
    }
}

/// **The id every check's question goes on the wire under.** `Check::request` writes it and a reader of the
/// answer must use the same one; `clip::score_set` learned that the hard way (#112), so it is public now
/// and there is one copy of it.
pub const QUESTION_ID: &str = "check";

impl Check {
    /// The request for this check. A check that needs an image or the facts and was not given them is an
    /// error naming what is missing: it never answers from less.
    pub fn request(&self, evidence: &Evidence) -> Result<Request, Error> {
        let mut missing: Vec<&str> = self
            .images
            .iter()
            .filter(|n| !evidence.images.contains_key(*n))
            .map(String::as_str)
            .collect();
        if self.facts && evidence.facts.is_none() {
            missing.push("facts");
        }
        if !missing.is_empty() {
            return Err(Error::MissingEvidence { check: self.name.clone(), missing: missing.join(", ") });
        }
        let mut state = format!("Check `{}`.", self.name);
        if !self.images.is_empty() {
            state += &format!(" Images, in order: {}.", self.images.join(", "));
        }
        if let (true, Some(f)) = (self.facts, &evidence.facts) {
            state += &format!(" Facts: {f}");
        }
        let mut req = Request::new(state).with_question(QUESTION_ID, Question::noul(self.question.clone()));
        for n in &self.images {
            req = req.with_image(evidence.images[n].clone());
        }
        Ok(req)
    }

    /// Ask `transport` this check's question and compare P(yes) with the threshold.
    pub fn run(&self, transport: &dyn Transport, evidence: &Evidence) -> Result<Finding, Error> {
        let response = ask(transport, &self.request(evidence)?)?;
        let p_yes = response.answers[QUESTION_ID].p_yes().expect("ask checked the answer is a noul");
        let model = response.model;
        let calibration = self.calibration_for(model.as_deref());
        let threshold = calibration.map_or(self.threshold, |c| c.threshold);
        Ok(Finding {
            check: self.name.clone(),
            p_yes,
            threshold,
            found: p_yes >= threshold,
            agreement: calibration.map(|c| c.agreement),
            model,
        })
    }

    /// The record this check earned for `model`, if any. A model nobody named has none.
    pub fn calibration_for(&self, model: Option<&str>) -> Option<&Calibration> {
        let model = model?;
        // **A record named for the slug reads for its dated snapshot** (#455): OpenRouter answers
        // `perplexity/pplx-decider-v1.3:free` with `perplexity/pplx-decider-v1.3-20261112`, and a finding that
        // fell back to the author's default there would be an uncalibrated reading of a calibrated model.
        self.calibrations.iter().find(|c| crate::transport::names_the_same_model(&c.model, model))
    }

    /// Whether this check may block the loop's gate when asked of `model`: it earned a calibration for
    /// that model at or above `bar`.
    pub fn may_block(&self, model: Option<&str>, bar: f64) -> bool {
        self.calibration_for(model).is_some_and(|c| c.is_blocking(bar))
    }

    /// Keep `calibration` as this check's record for its model, replacing the one it had for that model.
    pub fn record(&mut self, calibration: Calibration) {
        match self.calibrations.iter_mut().find(|c| c.model == calibration.model) {
            Some(slot) => *slot = calibration,
            None => self.calibrations.push(calibration),
        }
    }
}
