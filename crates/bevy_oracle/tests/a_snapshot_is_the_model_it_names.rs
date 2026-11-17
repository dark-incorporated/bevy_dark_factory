//! **A dated snapshot is the model it names** (#455, DM-04).
//!
//! OpenRouter answers with the model's own full name: asked for `perplexity/pplx-decider-v1.3:free` it answers
//! `perplexity/pplx-decider-v1.3-20261112`. Those are one model named three ways — a vendor prefix, a billing
//! variant (`:free`) and a dated snapshot — and a calibration made from those answers belongs to it, not to
//! "another model". Comparing the strings as typed would refuse every sample of a remote calibration run.
//!
//! **A different model is still refused.** `clef-flash` answering for `d3-flash` is not a rename, and the
//! record-labelled-for-another-model defect DM-04 exists to prevent stays prevented.

use bevy_oracle::{Calibration, Check, Config, Error, Evidence, Expecting, Question, Request, Response, Transport, Usage, verify_served};
use std::collections::BTreeMap;

const SLUG: &str = "pplx-decider-v1.3";
const ANSWERED: &str = "perplexity/pplx-decider-v1.3-20261112";

/// A transport that names one model in every answer.
struct Named(&'static str);

impl Transport for Named {
    fn endpoint(&self) -> String {
        "named".into()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        let answers = request
            .questions
            .keys()
            .map(|id| (id.clone(), bevy_oracle::Answer::Noul { noul: 0.6 }))
            .collect();
        Ok(Response { model: Some(self.0.to_owned()), answers, usage: Some(Usage::default()) })
    }
}

fn request() -> Request {
    Request::new("s").with_question("q", Question::noul("?"))
}

/// **The three ways OpenRouter names one model are one model**, and a name of another model is not.
#[test]
fn a_vendor_prefix_a_free_tag_and_a_dated_snapshot_are_the_same_model() {
    for (label, answered) in [
        (SLUG, ANSWERED),
        (SLUG, "pplx-decider-v1.3"),
        ("perplexity/pplx-decider-v1.3:free", ANSWERED),
        ("perplexity/pplx-decider-v1.3:free", "perplexity/pplx-decider-v1.3:free"),
        ("d3-flash", "d3-flash"),
    ] {
        assert!(bevy_oracle::names_the_same_model(label, answered), "`{label}` and `{answered}` are one model");
    }
    for (label, answered) in [
        ("clef-flash", "d3-flash"),
        (SLUG, "perplexity/pplx-decider-v1.2"),
        (SLUG, "perplexity/sonar-pro"),
        // A snapshot date is stripped from the end only; a number inside the name is part of it.
        ("pplx-decider-v1.3", "pplx-decider-v4.3"),
    ] {
        assert!(
            !bevy_oracle::names_the_same_model(label, answered),
            "`{label}` must not be read as `{answered}`: a record for one model is never another's"
        );
    }
}

/// **A calibration run's guard accepts the snapshot its endpoint answers with**, so a remote record can be made.
#[test]
fn the_label_guard_accepts_the_snapshot_of_the_model_it_is_labelled_for() {
    let ok = Expecting::new(&Named(ANSWERED), SLUG).send(&request());
    assert!(ok.is_ok(), "a dated snapshot is the model the record is for: {ok:?}");

    match Expecting::new(&Named("perplexity/pplx-decider-v1.2"), SLUG).send(&request()) {
        Err(Error::ModelMismatch { label, served, .. }) => {
            assert_eq!(label, SLUG);
            assert_eq!(served, vec!["perplexity/pplx-decider-v1.2".to_owned()]);
        }
        other => panic!("another model's answer must still be refused: {other:?}"),
    }
}

/// **The preflight accepts the same three names**, so it does not refuse the run the guard would allow.
#[test]
fn the_preflight_reads_the_snapshot_as_the_model_it_lists() {
    let served = vec!["perplexity/pplx-decider-v1.3:free".to_owned()];
    verify_served(SLUG, "https://openrouter.example/decisions", &served).expect("the slug is what the remote endpoint serves");
    match verify_served("pplx-decider-v1.2", "https://openrouter.example/decisions", &served) {
        Err(Error::ModelMismatch { label, .. }) => assert_eq!(label, "pplx-decider-v1.2"),
        other => panic!("{other:?}"),
    }
}

/// **A finding is calibrated for a snapshot too**, so a remote answer is read with its own threshold instead of
/// falling back to the author's default as an uncalibrated one.
#[test]
fn a_check_calibrated_for_the_slug_is_calibrated_for_its_snapshot() {
    let check = Check {
        name: "floating".into(),
        question: "Is the piece floating?".into(),
        images: vec!["view".into()],
        facts: false,
        threshold: 0.5,
        after_edit: false,
        calibrations: vec![Calibration {
            check: "floating".into(),
            model: SLUG.into(),
            set_size: 10,
            agreement: 0.9,
            threshold: 0.4,
            date: "2026-11-16".into(),
            correct: 9,
            wrong: 1,
            excluded: Vec::new(),
        }],
    };
    let images = BTreeMap::from([("view".to_owned(), bevy_oracle::data_url(b"png"))]);
    let finding = check.run(&Named(ANSWERED), &Evidence { images, facts: None }).unwrap();
    assert!(finding.is_calibrated(), "the snapshot is the model the record names: {finding:?}");
    assert_eq!(finding.threshold, 0.4, "the threshold it earned, not the author's default");
    let _ = Config::default();
}
