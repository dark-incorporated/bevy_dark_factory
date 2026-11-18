//! **Each model's calibration on the clip set is its own file, and the local default is the one that
//! agrees best** (#455, DM-04).
//!
//! `fixtures/calibrations/<model>.yaml` is written by `decide_check --calibrate ... --model <model> --out`
//! against that model's own endpoint (the run refuses when the endpoint serves another). A test cannot make
//! the run, so it keeps the records honest: the file is named for its model, covers every sample of the
//! labelled set, and its agreement is its own counts' arithmetic. The committed
//! `clip_calibration.yaml` (clef-flash, 2026-11-03) is a record of the same shape, made before `score_set`
//! paired the samples with the right answer (#455), and is not compared with these.

use std::path::PathBuf;

#[derive(Debug, serde::Deserialize)]
struct Record {
    check: String,
    model: String,
    set_size: usize,
    agreement: f64,
    correct: usize,
    wrong: usize,
    threshold: f64,
    /// The samples this record says it could not measure (#455), each naming itself.
    #[serde(default)]
    excluded: Vec<Excluded>,
}

/// One named exclusion, read here only for its sample's name and reason.
#[derive(Debug, serde::Deserialize)]
struct Excluded {
    sample: String,
    why: String,
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn records() -> Vec<(String, Record)> {
    let mut out: Vec<(String, Record)> = std::fs::read_dir(fixtures().join("calibrations"))
        .expect("fixtures/calibrations exists")
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .map(|p| {
            let text = std::fs::read_to_string(&p).unwrap();
            let rec: Record = serde_yaml_ng::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            (p.file_stem().unwrap().to_string_lossy().into_owned(), rec)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn every_model_has_its_own_record_of_the_clip_set() {
    let set: Vec<serde_yaml_ng::Value> =
        serde_yaml_ng::from_str(&std::fs::read_to_string(fixtures().join("clip_samples.yaml")).unwrap()).unwrap();
    let records = records();
    let models: Vec<&str> = records.iter().map(|(_, r)| r.model.as_str()).collect();
    for wanted in ["clef-flash", "d3-flash"] {
        assert!(models.contains(&wanted), "no calibration record for {wanted}: {models:?}");
    }
    for (stem, r) in &records {
        assert_eq!(&r.model, stem, "{stem}.yaml holds a record labelled `{}`: a file is one model's", r.model);
        assert_eq!(r.check, "clip_shows_its_title", "{stem}");
        // **What it measured plus what it named is the set** (#455, owner's decision 2026-11-16). A record that
        // states fewer than the set is honest only when it says which samples it did not measure: `set_size`
        // alone equalling the set is the full-run case, and it is what the two local records are.
        assert_eq!(
            r.set_size + r.excluded.len(),
            set.len(),
            "{stem} covers {} samples and names {} it could not, the set holds {}",
            r.set_size,
            r.excluded.len(),
            set.len()
        );
        // **Each excluded id is a sample of the set, and each is excluded once** (#455, review G): the sum above
        // passes just as well for invented or repeated ids, and an exclusion naming a sample that does not exist
        // is a record that lost a different one.
        let ids: Vec<&str> = set.iter().filter_map(|s| s.get("id").and_then(|i| i.as_str())).collect();
        let mut seen = std::collections::BTreeSet::new();
        for e in &r.excluded {
            assert!(!e.sample.trim().is_empty() && !e.why.trim().is_empty(), "{stem}: an exclusion must name its sample and why");
            assert!(ids.contains(&e.sample.as_str()), "{stem}: `{}` is not a sample of the set", e.sample);
            assert!(seen.insert(e.sample.as_str()), "{stem}: `{}` is excluded twice", e.sample);
        }
        let _ = (&r.correct, &r.wrong, r.threshold);
        assert_eq!(r.correct + r.wrong, r.set_size, "{stem}: the counts do not add up");
        let measured = r.correct as f64 / r.set_size as f64;
        assert!((measured - r.agreement).abs() < 1e-9, "{stem}: agreement {} but counts give {measured}", r.agreement);
        assert!((0.0..=1.0).contains(&r.threshold), "{stem}: threshold {}", r.threshold);
    }
}

#[test]
fn the_local_default_is_the_local_model_that_agrees_best() {
    // **Only a local model can be the local default** (#455): pplx-decider agrees best on the set and runs on
    // OpenRouter, so `LOCAL_ENDPOINT` cannot serve it and it is not a candidate. The issue's decision names the
    // comparison: "whichever of d3-flash and clef-flash agrees better on the set".
    let remote = bevy_oracle::OPENROUTER_MODEL;
    let records: Vec<(String, Record)> =
        records().into_iter().filter(|(_, r)| !bevy_oracle::names_the_same_model(remote, &r.model)).collect();
    let best = records
        .iter()
        .max_by(|a, b| a.1.agreement.partial_cmp(&b.1.agreement).unwrap())
        .map(|(_, r)| r.model.clone())
        .unwrap();
    let endpoint = bevy_oracle::Config::default().endpoint;
    assert!(
        endpoint.contains(&best),
        "the default endpoint {endpoint} does not serve {best}, which agreed best on the clip set: {:?}",
        records.iter().map(|(_, r)| (&r.model, r.agreement)).collect::<Vec<_>>()
    );
}
