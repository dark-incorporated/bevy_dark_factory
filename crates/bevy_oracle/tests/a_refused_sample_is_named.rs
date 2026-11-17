//! **A sample that could not be scored is named** (#455, DM-04).
//!
//! A calibration run makes 122 requests. When one of them is refused — OpenRouter's content filter answered 403
//! to one clip's images on 2026-11-16 — the run stops, and the one thing the person needs is *which* sample. A
//! bare `status 403` is a run nobody can act on: the set has to be read again by hand, with a second copy of the
//! wire shape, which is exactly the duplicate the crate refuses elsewhere.
//!
//! No test here calls a model: the stand-in refuses the sample it is told to.

use bevy_oracle::{Check, Error, Labelled, Question, Request, Response, Transport, clip_check, score_set};
use std::path::PathBuf;

/// The id of the sample the stand-in refuses.
const REFUSED: &str = "b";
/// A word only that sample's title holds, so the stand-in can tell its request from the other one's.
const REFUSED_TITLE: &str = "second";

/// A transport that answers `noul 0.5` to everything and refuses one sample's request.
struct Refuses(&'static str);

impl Transport for Refuses {
    fn endpoint(&self) -> String {
        "https://openrouter.example/decisions".into()
    }

    fn send(&self, request: &Request) -> Result<Response, Error> {
        // The title rides in the question's instructions (see `clip::request`), not in `state`.
        let title_is_here = request.questions.values().any(|q| match q {
            Question::Noul { instructions } => instructions.contains(self.0),
            _ => false,
        });
        if title_is_here {
            return Err(Error::Refused {
                endpoint: self.endpoint(),
                status: 403,
                body: r#"{"error":{"message":"Request blocked by content filter","code":403}}"#.into(),
            });
        }
        let answers = request
            .questions
            .keys()
            .map(|id| (id.clone(), bevy_oracle::Answer::Noul { noul: 0.5 }))
            .collect();
        Ok(Response { model: Some("a-model".into()), answers, usage: None })
    }
}

/// Two samples whose titles the stand-in can tell apart; the directories need not exist, because the check is
/// asked with no images (a `facts` check) so nothing is read from disk.
fn samples() -> Vec<Labelled> {
    vec![
        Labelled { id: "a".into(), title: "the first clip".into(), dir: PathBuf::from("/nonexistent/a"), truth: true },
        Labelled { id: REFUSED.into(), title: "the second clip".into(), dir: PathBuf::from("/nonexistent/b"), truth: false },
    ]
}

fn check() -> Check {
    Check {
        name: "clip_shows_its_title".into(),
        question: "Do the images fail to show it?".into(),
        images: Vec::new(),
        facts: false,
        threshold: 0.5,
        after_edit: false,
        calibrations: Vec::new(),
    }
}

/// **The error names the sample and still carries the endpoint's own refusal**, so one line says what to fix.
#[test]
fn a_refused_sample_is_named_beside_the_refusal_that_stopped_the_run() {
    let e = score_set(&check(), &samples(), &Refuses(REFUSED_TITLE)).expect_err("the second sample is refused");
    let said = e.to_string();
    assert!(said.contains("`b`"), "the sample's id is named: {said}");
    assert!(said.contains("403") && said.contains("content filter"), "the reason is kept: {said}");
    match e {
        Error::Sample { id, .. } => assert_eq!(id, REFUSED),
        other => panic!("a refused sample is its own error, not a bare status: {other:?}"),
    }
}

/// **A run that loses a sample does not return a partial set**: a calibration from 121 of 122 samples would be a
/// record whose `set_size` says 122 and whose counts do not.
#[test]
fn a_refused_sample_yields_no_samples_at_all() {
    assert!(score_set(&check(), &samples(), &Refuses(REFUSED_TITLE)).is_err(), "a half-scored set is not a set");
}

/// The set that is not refused scores in order, and each sample's truth is the right answer to the question.
#[test]
fn a_set_that_is_answered_scores_every_sample() {
    let scored = score_set(&check(), &samples(), &Refuses("nothing matches this")).expect("nothing is refused");
    assert_eq!(scored.len(), 2);
    assert!(scored.iter().all(|s| (s.p_yes - 0.5).abs() < 1e-12));
    assert_eq!((scored[0].truth, scored[1].truth), (false, true), "a match's right answer is no; a mismatch's is yes");
    let _ = clip_check("clip_shows_its_title");
}
