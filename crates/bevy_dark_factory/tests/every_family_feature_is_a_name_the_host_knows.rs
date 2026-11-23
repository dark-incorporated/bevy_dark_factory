//! **Every family crate the editor can be built with is a name `family_host::KNOWN` has** (#556).
//!
//! `KNOWN` is what decides whether a project's `family.yaml` is *understood*: a crate missing from it
//! reads as one the editor cannot host, so the project cannot declare it and `project_config` cannot offer
//! the switch. Its own doc asks for the name to arrive "in the commit that adds the cargo feature", and
//! `bevy_light_grid` was added as a feature and left out of the list for days — this reads the manifest,
//! which is the only place the features actually are, so it cannot drift the same way twice.

use std::collections::BTreeSet;

/// **The family crate features the manifest declares**, read from `Cargo.toml`'s `[features]` table.
///
/// Every `bevy_*` feature that is not a switch over other features (`family_all` and `family_none` are
/// groups) is a crate the editor can host. Read from the file rather than listed here, so a crate added to
/// one and not the other fails.
fn family_features() -> BTreeSet<String> {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest).unwrap_or_else(|e| panic!("{manifest:?}: {e}"));
    let features = text
        .split("[features]")
        .nth(1)
        .expect("Cargo.toml has a [features] table")
        .split("\n[")
        .next()
        .expect("the table ends");
    features
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, _)| name.trim().to_owned())
        .filter(|name| name.starts_with("bevy_"))
        .collect()
}

/// **The crates that are not a switch**: `bevy_gooey` is the editor's own UI and always built (a `KNOWN` name with
/// no feature to turn on), and `bevy_devshot` is a frame-on-demand tool, not a game's family crate. Both sides of the
/// comparison drop them, so the test asks about the crates a project can actually declare.
const NOT_A_SWITCH: &[&str] = &["bevy_gooey", "bevy_devshot"];

#[test]
fn every_family_feature_is_a_name_the_host_knows() {
    let known: BTreeSet<String> = bevy_dark_factory::family_host::KNOWN
        .iter()
        .map(|k| (*k).to_owned())
        .filter(|k| !NOT_A_SWITCH.contains(&k.as_str()))
        .collect();
    let features = family_features();
    let unknown: Vec<&String> = features.difference(&known).collect();
    assert!(
        unknown.is_empty(),
        "**these features are family crates `family_host::KNOWN` does not name**: {unknown:?}. A project cannot \
         declare one, so the crate is a feature nothing can switch on (#556)."
    );
    let orphaned: Vec<&String> = known.difference(&features).collect();
    assert!(
        orphaned.is_empty(),
        "**`family_host::KNOWN` names crates the manifest has no feature for**: {orphaned:?}. Either the feature goes \
         with the name or the name goes."
    );
}
