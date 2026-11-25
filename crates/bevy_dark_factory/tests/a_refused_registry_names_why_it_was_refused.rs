//! **A `data.yaml` the loader refuses says why, rather than drawing the same words as an absent one**
//! (#631).
//!
//! **The measured failure.** A review found `blueprint/state.project.data_files` empty beside a
//! `data.yaml` that was present and valid — `version: 1`, naming `../data/tuning.yaml`, which existed
//! (2,002 lines). The DATA table drew *"no data files registered"*, the same sentence a project with
//! no registry at all draws. Three unlike states were indistinguishable:
//!
//! 1. no `data.yaml` — a project with no data files;
//! 2. a `data.yaml` this build refuses — malformed, wrong version, a repeated id, or an entry whose
//!    file is not there;
//! 3. a registry whose every file is unreadable.
//!
//! `DataFiles::load` already returned a precise sentence for (2). `ops_data::listing` threw it away
//! (`let Ok(Some(reg)) = … else { return Vec::new() }`), and the panel only had rows to draw from.
//! These tests hold the sentence: every refusal names the fault, and an absent registry stays silent.

use std::path::Path;

fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("df-data-refusal-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch blueprint dir");
    dir
}

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).expect("the scratch file");
}

#[test]
fn a_refused_registry_names_why_it_was_refused() {
    use bevy_dark_factory::ops_data::listing;

    // 1. **No registry is not a fault**, and it says nothing: a project with no data files is a
    // project with no data files, and a sentence there would be noise on every new project.
    let empty = scratch("absent");
    let (rows, why) = listing(&empty);
    assert!(rows.is_empty(), "no registry means no rows");
    assert_eq!(why, None, "an absent registry is not a refusal");

    // 2. **A version this build does not know** — the case a real project hits after a schema move
    // — names the version it found. This is the sentence a person needs and did not have.
    let wrong = scratch("version");
    write(&wrong, "data.yaml", "version: 99\nfiles:\n  - id: tuning\n    path: ../data/tuning.yaml\n");
    let (rows, why) = listing(&wrong);
    assert!(rows.is_empty(), "a refused registry yields no rows");
    let why = why.expect("a refused registry says why");
    assert!(
        why.contains("99"),
        "the refusal names the version it found, so the reader can see what moved: {why}"
    );

    // 3. **An entry naming a file that is not there** names the entry — the case the review hit,
    // where the registry itself was valid and one `path` pointed at nothing.
    let missing = scratch("missing");
    write(
        &missing,
        "data.yaml",
        "version: 1\nfiles:\n  - id: tuning\n    path: ../data/tuning.yaml\n",
    );
    let (rows, why) = listing(&missing);
    assert!(rows.is_empty(), "a registry whose only file is absent yields no rows");
    let why = why.expect("a missing entry's file says why");
    assert!(
        why.contains("tuning") || why.contains("tuning.yaml"),
        "the refusal names the entry or the path, not just that something failed: {why}"
    );

    // 4. **A valid registry and its file produce rows and no refusal** — the control, so the
    // assertions above are about refusal rather than about `listing` never working.
    let good = scratch("good");
    std::fs::create_dir_all(good.join("../data")).ok();
    let data = good.parent().expect("a parent").join("data");
    std::fs::create_dir_all(&data).expect("the data dir");
    std::fs::write(data.join("tuning.yaml"), "mob:\n  speed: 1.50\n").expect("the data file");
    write(&good, "data.yaml", "version: 1\nfiles:\n  - id: tuning\n    path: ../data/tuning.yaml\n");
    let (rows, why) = listing(&good);
    assert_eq!(why, None, "a valid registry is not a refusal");
    assert_eq!(rows.len(), 1, "the one leaf is listed: {rows:?}");
    assert_eq!(rows[0].path, "mob.speed");

    for dir in [empty, wrong, missing, good] {
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(dir.parent().expect("a parent").join("data"));
    }
}
