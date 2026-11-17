use crate::Error;
use serde::{Deserialize, Serialize};

/// One labelled example: the model's P(yes) and the truth a person labelled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub p_yes: f64,
    pub truth: bool,
}

/// **One sample of a labelled set that could not be scored, and why** (#455, DM-04).
///
/// A calibration run makes one request per sample; when the endpoint refuses one, the run has a choice between
/// three lies — drop the sample silently, record the nominal set size, or report nothing — and this type is the
/// honest fourth: name it. `sample` is the manifest's id, `frame` the one image that was refused when the
/// refusal was isolated to an image ([`crate::Error::Sample`] names the sample; this names the picture), and
/// `why` the endpoint's own words.
///
/// The measured case (`tests/an_excluded_sample_is_stated_in_the_record.rs`): OpenRouter's secret scanner
/// answered 403 `SECRET:bitcoin-wif-compressed` for `103-light-buttons`' `frame1.png` alone — the title, the
/// still, `frame0` and `frame2` all answered — so pplx-decider's record is 121 of 122 with that one named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Exclusion {
    /// The labelled sample's id in `clip_samples.yaml`.
    pub sample: String,
    /// The one image that was refused, when the refusal was isolated to it; `None` when the whole request was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
    /// What the endpoint said, in its own words, including the status that proved it.
    pub why: String,
}

/// What a check earned. Written beside the check; only a check at or above the bar may block the gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Calibration {
    pub check: String,
    pub model: String,
    /// **The number of samples the threshold was measured on**, never the set's nominal count: a record that
    /// says 122 while 121 answered reads, to anyone comparing rows, as a full run. `set_size` plus
    /// `excluded.len()` is the nominal set ([`Calibration::states_the_set_it_measured`]).
    pub set_size: usize,
    /// Fraction of the set the threshold classifies as labelled, 0..=1.
    pub agreement: f64,
    /// P(yes) at or above this is a yes.
    pub threshold: f64,
    pub date: String,
    /// **The counts the agreement is made of** (#112). `agreement` alone is a number beside nothing:
    /// the record is what a test and a gate read, and two typed numbers that disagree are
    /// indistinguishable from a measurement. Defaulted so a hand-written `checks.yaml` that predates
    /// this field still loads; a calibration run always fills both.
    #[serde(default)]
    pub correct: usize,
    /// Samples the threshold got wrong.
    #[serde(default)]
    pub wrong: usize,
    /// **The samples of the set that were not measured, each naming itself** (#455). Empty for a full run, and
    /// defaulted so a record written before this field existed still reads — those records excluded nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub excluded: Vec<Exclusion>,
}

impl Calibration {
    /// Whether this check may block the loop's gate: its agreement meets `bar`.
    pub fn is_blocking(&self, bar: f64) -> bool {
        self.agreement >= bar
    }

    /// **Whether this record accounts for the whole set of `nominal` samples**: the ones it measured plus the ones
    /// it names. A record that says `set_size: 122` *and* names an exclusion claims both that 122 answered and
    /// that one did not, which is the dishonest case this exists to refuse.
    pub fn states_the_set_it_measured(&self, nominal: usize) -> bool {
        self.set_size + self.excluded.len() == nominal
    }

    /// Whether every exclusion says which sample and why. An exclusion that names nothing is a hole in the set
    /// with no way to find it.
    pub fn exclusions_are_named(&self) -> bool {
        self.excluded.iter().all(|e| !e.sample.trim().is_empty() && !e.why.trim().is_empty())
    }
}

/// Pick the threshold that best separates yes from no on a labelled set.
///
/// Candidates are 0, 1 (stricter than any P) and the midpoints between neighbouring distinct scores;
/// the one with the most agreements wins. Ties go to the candidate nearest 0.5, so a set that any
/// threshold in a range separates equally gets the one least sensitive to drift. A set with no
/// samples, a NaN score or only one label cannot calibrate: a threshold from one class is meaningless.
pub fn calibrate(
    check: &str,
    model: &str,
    date: &str,
    samples: &[Sample],
) -> Result<Calibration, Error> {
    if samples.is_empty() {
        return Err(Error::Calibration(format!("check `{check}` has no labelled samples")));
    }
    if samples.iter().any(|s| !(0.0..=1.0).contains(&s.p_yes)) {
        return Err(Error::Calibration(format!("check `{check}` has a P(yes) outside 0..=1")));
    }
    let yes = samples.iter().filter(|s| s.truth).count();
    if yes == 0 || yes == samples.len() {
        return Err(Error::Calibration(format!(
            "check `{check}`: every sample is labelled {}; a threshold needs both yes and no",
            if yes == 0 { "no" } else { "yes" }
        )));
    }
    let mut ps: Vec<f64> = samples.iter().map(|s| s.p_yes).collect();
    ps.sort_by(|a, b| a.partial_cmp(b).expect("range-checked, so not NaN"));
    ps.dedup();
    let mut candidates = vec![0.0];
    candidates.extend(ps.windows(2).map(|w| (w[0] + w[1]) / 2.0));
    candidates.push(1.0 + f64::EPSILON);
    let agree = |t: f64| samples.iter().filter(|s| (s.p_yes >= t) == s.truth).count();
    let best = candidates
        .into_iter()
        .map(|t| (agree(t), t))
        .max_by(|a, b| a.0.cmp(&b.0).then((b.1 - 0.5).abs().partial_cmp(&(a.1 - 0.5).abs()).unwrap()))
        .expect("at least two candidates");
    Ok(Calibration {
        check: check.into(),
        model: model.into(),
        set_size: samples.len(),
        agreement: best.0 as f64 / samples.len() as f64,
        threshold: best.1.min(1.0),
        date: date.into(),
        correct: best.0,
        wrong: samples.len() - best.0,
        // A run fills this only when it had to skip a sample; `calibrate` sees the samples it was given and
        // cannot know what the set held, so an excluder sets it after.
        excluded: Vec::new(),
    })
}
