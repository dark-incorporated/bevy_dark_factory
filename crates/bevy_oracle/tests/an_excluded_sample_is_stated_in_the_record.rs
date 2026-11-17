//! **A record that measured 121 of 122 samples says so** (#455, DM-04).
//!
//! A calibration record's `set_size` is the number of samples its threshold was actually measured on, and
//! `excluded` names the ones that could not be. The dishonest case is a record that claims the set's nominal
//! count while fewer samples answered — `set_size: 122` beside counts that came from 121 — which reads to
//! anyone who later compares the rows as a full run. The nominal count is not something the record declares
//! (a dishonest record would simply declare it too): it is `fixtures/clip_samples.yaml`, so this test derives
//! it and does the arithmetic against what the record says.
//!
//! No test here calls a model.

use std::path::PathBuf;

use bevy_oracle::{Calibration, Exclusion, Sample, calibrate};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// The number of labelled samples the committed set holds.
fn nominal() -> usize {
    let text = std::fs::read_to_string(fixtures().join("clip_samples.yaml")).expect("the set is in the repo");
    let samples: Vec<serde_yaml_ng::Value> =
        serde_yaml_ng::from_str(&text).expect("the set parses as YAML");
    samples.len()
}

fn record(excluded: Vec<Exclusion>) -> Result<Calibration, String> {
    let samples: Vec<Sample> =
        (0..4).map(|i| Sample { p_yes: if i % 2 == 0 { 0.9 } else { 0.1 }, truth: i % 2 == 0 }).collect();
    let mut c = calibrate("clip_shows_its_title", "pplx-decider-v1.3", "2026-11-16", &samples)
        .map_err(|e| e.to_string())?;
    c.excluded = excluded;
    Ok(c)
}

/// Two of the four samples measured well enough to calibrate; the count is what was measured, not what was asked.
#[test]
fn a_record_s_stated_size_is_the_number_it_measured() {
    let c = record(Vec::new()).expect("four samples with both labels calibrate");
    assert_eq!(c.set_size, 4, "no exclusions: the measured count is the set");
    assert_eq!(c.correct + c.wrong, c.set_size, "the counts add up to what was measured");
    assert!(c.excluded.is_empty(), "a complete run excludes nothing");
}

/// **The exclusion is carried in the record and states itself**, so a reader of the file learns why the row is
/// one short without a second document.
#[test]
fn an_exclusion_names_the_sample_the_frame_and_the_reason() {
    let c = record(vec![Exclusion {
        sample: "103-light-buttons".into(),
        frame: Some("frame1.png".into()),
        why: "the provider's secret scanner refuses these bytes: 403 SECRET:bitcoin-wif-compressed".into(),
    }])
    .expect("the measured samples still calibrate");

    assert_eq!(c.excluded.len(), 1);
    let e = &c.excluded[0];
    assert_eq!(e.sample, "103-light-buttons");
    assert_eq!(e.frame.as_deref(), Some("frame1.png"));
    assert!(e.why.contains("403"), "the reason carries the status that proved it: {}", e.why);
    assert_eq!(c.set_size, 4, "the measured count is unaffected by what was excluded");
}

/// **A record that claims the nominal count while excluding something is refused.** This is the arithmetic the
/// committed records are held to: `set_size + excluded` is the nominal set, or the record is lying about how
/// much of it answered.
#[test]
fn a_record_claiming_the_whole_set_while_excluding_a_sample_is_refused() {
    let nominal = nominal();
    let honest = Calibration {
        check: "clip_shows_its_title".into(),
        model: "pplx-decider-v1.3".into(),
        set_size: nominal - 1,
        agreement: 0.8,
        threshold: 0.5,
        date: "2026-11-16".into(),
        correct: 0,
        wrong: 0,
        excluded: vec![Exclusion {
            sample: "103-light-buttons".into(),
            frame: Some("frame1.png".into()),
            why: "403 SECRET:bitcoin-wif-compressed".into(),
        }],
    };
    assert!(honest.states_the_set_it_measured(nominal), "121 measured plus 1 named is 122 asked: {}", honest.set_size);

    let dishonest = Calibration { set_size: nominal, ..honest.clone() };
    assert!(
        !dishonest.states_the_set_it_measured(nominal),
        "a record that says {} measured and also names an exclusion is claiming {} answered {}",
        dishonest.set_size,
        dishonest.set_size,
        nominal
    );
}

/// A record whose exclusions are unnamed is a record with a hole in it: one sample of the set answered nothing and
/// nothing says which.
#[test]
fn an_exclusion_must_say_which_sample_and_why() {
    let mut c = record(vec![Exclusion { sample: String::new(), frame: None, why: String::new() }])
        .expect("the measured samples still calibrate");
    assert!(!c.exclusions_are_named(), "an empty exclusion is not an exclusion: {c:?}");
    c.excluded.clear();
    assert!(c.exclusions_are_named(), "no exclusion at all is fine");
    c.excluded.push(Exclusion { sample: "a-clip".into(), frame: None, why: "403".into() });
    assert!(c.exclusions_are_named(), "a named exclusion with a reason is complete");
}

/// The record round-trips through YAML with its exclusion, and an older record with no `excluded` key still reads.
#[test]
fn the_exclusion_round_trips_and_an_older_record_still_loads() {
    let c = record(vec![Exclusion {
        sample: "103-light-buttons".into(),
        frame: Some("frame1.png".into()),
        why: "403 SECRET:bitcoin-wif-compressed".into(),
    }])
    .expect("calibrates");
    let text = serde_yaml_ng::to_string(&c).expect("writes");
    let back: Calibration = serde_yaml_ng::from_str(&text).expect("reads");
    assert_eq!(back, c);

    let older = "check: clip_shows_its_title\nmodel: d3-flash\nset_size: 122\nagreement: 0.86\n\
                 threshold: 0.7\ndate: '2026-11-15'\ncorrect: 105\nwrong: 17\n";
    let read: Calibration = serde_yaml_ng::from_str(older).expect("a record from before `excluded` still reads");
    assert!(read.excluded.is_empty(), "no key means nothing was excluded, which is what those records meant");
}
