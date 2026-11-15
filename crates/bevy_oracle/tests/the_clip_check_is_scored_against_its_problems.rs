//! **A model that is right about the clip check calibrates to 100 %, not to a coin toss** (#455).
//!
//! The clip check asks *"do these frames FAIL to show what the line says?"*: yes means a problem, as for
//! every check in this crate. A labelled sample is a match (its own line: the right answer is **no**) or a
//! mismatch (another line's title: the right answer is **yes**). `score_set` once handed `calibrate` the
//! label "is a match" beside P(yes = fails to show), so a model that was perfectly right scored at best
//! 50 % (the all-yes threshold) and one that was perfectly wrong scored 100 %: measured with d3-flash,
//! whose P(yes) was higher on 8 of 8 sampled mismatches than on their matches, and which "agreed" 50.0 %.
//!
//! The stand-in below is a perfect model: it answers 0.95 when the title in the request is the mismatch's
//! and 0.05 when it is the match's. Nothing leaves the process.

use bevy_oracle::{Answer, Error, Labelled, Request, Response, Transport, calibrate, clip_check, score_set};

const MATCH_TITLE: &str = "A bench the stallkeeper sells from";
const MISMATCH_TITLE: &str = "A torch burns on a lamp at 23:00";
/// P(yes) of a model that knows the clip does not show the line.
const SAYS_YES: f64 = 0.95;
/// P(yes) of a model that knows the clip shows the line.
const SAYS_NO: f64 = 0.05;

struct Perfect;

impl Transport for Perfect {
    fn endpoint(&self) -> String {
        "perfect".into()
    }
    fn send(&self, request: &Request) -> Result<Response, Error> {
        let asked = request.questions.values().map(|q| format!("{q:?}")).collect::<String>();
        let p = if asked.contains(MISMATCH_TITLE) { SAYS_YES } else { SAYS_NO };
        let answers = request.questions.keys().map(|id| (id.clone(), Answer::Noul { noul: p })).collect();
        Ok(Response { model: Some("perfect".into()), answers, usage: None })
    }
}

fn sample(dir: &std::path::Path, id: &str, title: &str, truth: bool) -> Labelled {
    let d = dir.join(id);
    std::fs::create_dir_all(&d).unwrap();
    for name in ["still", "frame0", "frame1", "frame2"] {
        std::fs::write(d.join(format!("{name}.png")), b"\x89PNG not really").unwrap();
    }
    Labelled { id: id.into(), title: title.into(), dir: d, truth }
}

#[test]
fn a_model_that_is_right_about_every_clip_agrees_one_hundred_percent() {
    let dir = std::env::temp_dir().join(format!("clip_polarity_{}", std::process::id()));
    let samples = vec![
        sample(&dir, "a", MATCH_TITLE, true),
        sample(&dir, "a-mismatch", MISMATCH_TITLE, false),
        sample(&dir, "b", MATCH_TITLE, true),
        sample(&dir, "b-mismatch", MISMATCH_TITLE, false),
    ];
    let scored = score_set(&clip_check("clip_shows_its_title"), &samples, &Perfect).unwrap();
    let record = calibrate("clip_shows_its_title", "perfect", "2026-11-15", &scored).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert_eq!(
        record.agreement, 1.0,
        "a model that says yes exactly where the clip fails its line is in perfect agreement, not at {}",
        record.agreement
    );
    assert!(record.threshold > SAYS_NO && record.threshold <= SAYS_YES, "threshold {}", record.threshold);
}
