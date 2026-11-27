//! **The checks view reads `checks.yaml` when the project index says it moved, and no clock of its own** (#501, item 8:
//! `checks_view`'s `RECHECK_SECS`, 1.0 s, is retired).
//!
//! - an edit made while the editor is open is seen within the index's debounce, and what the view holds is what an independent read of
//!   the file says;
//! - the view reads its file again **only** when that file moved: a file beside it that is written and changed makes it read nothing
//!   (the retired timer read once a second whatever happened, so this count fails on it);
//! - a `checks.yaml` the index cannot follow (named by the project's `.gitignore`) is refused and says why, and is not read.

use std::time::Duration;

use bevy::prelude::*;
use bevy_dark_factory::checks_view::{ChecksView, checks_for};
use bevy_dark_factory::project::Project;
use serde_json::json;

use crate::index_view_support as support;
use crate::settle_support::Frames;

const FILE: &str = "checks.yaml";

/// **How many frames the editor is stepped after the ignored file is edited**, from a literal: 1.3 s at `support::STEP`, longer than
/// the retired one-second timer's period, so a view that still read on a clock of its own would read inside it.
const EDIT_WATCHED_FOR: Frames = Frames::covering(Duration::from_millis(1300), support::STEP);

fn good(threshold: f64) -> String {
    format!("{{checks: [{{name: \"mine\", question: \"Is it floating?\", threshold: {threshold}}}]}}")
}

fn root(app: &App) -> std::path::PathBuf {
    app.world().resource::<Project>().root.clone()
}

fn path(app: &App) -> std::path::PathBuf {
    root(app).join("assets/blueprint").join(FILE)
}

/// The threshold of the project's own check `mine` in the checks the editor would ask now.
fn mine(app: &App) -> Option<f64> {
    let checks = checks_for(app.world().get_resource::<ChecksView>(), &root(app)).ok()?;
    checks.get("mine").map(|c| c.threshold)
}

fn reads(app: &App) -> u64 {
    app.world().resource::<ChecksView>().reads()
}

#[test]
fn an_edit_made_while_the_editor_is_open_is_seen_within_the_index_debounce() {
    let (mut app, _ctx) = support::editor("checks_follow_edit");
    support::settle_until_the_index_is_quiet(&mut app);
    for threshold in [0.35, 0.8] {
        std::fs::write(path(&app), good(threshold)).unwrap_or_else(|e| panic!("{e}"));
        // The view holds the checks as the file defines them: it is asked through `checks_for`, which answers with the view's held checks.
        let took = support::until_seen(&mut app, &format!("`mine` at {threshold}"), |a| mine(a) == Some(threshold));
        assert!(took < support::SEEN_WITHIN, "the edit took {took:?} to be seen");
        // **Ground truth is a plain read of the file.**
        let on_disk = std::fs::read_to_string(path(&app)).unwrap_or_else(|e| panic!("{e}"));
        let said: f64 = on_disk.split("threshold: ").nth(1).and_then(|t| t.trim_end_matches("}]}").parse().ok()).unwrap_or_else(|| panic!("the file says a threshold: {on_disk}"));
        assert_eq!(mine(&app), Some(said), "the view holds what the file says");
        assert_eq!(support::verdict(&mut app, "checks")["outcome"], json!("applied"));
    }
}

#[test]
fn a_file_beside_the_checks_file_that_changes_makes_the_view_read_nothing() {
    let (mut app, _ctx) = support::editor("checks_follow_neighbour");
    std::fs::write(path(&app), good(0.5)).unwrap_or_else(|e| panic!("{e}"));
    support::until_seen(&mut app, "the checks read", |a| mine(a) == Some(0.5));
    let beside = path(&app);
    support::a_neighbour_is_not_read(&mut app, &beside, reads);
}

/// **A file the index cannot follow is refused, and said, and not read**: no event would ever tell the view it changed.
#[test]
fn a_checks_file_the_index_does_not_follow_is_refused_and_not_read() {
    let (mut app, _ctx) = support::editor("checks_follow_ignored");
    std::fs::write(path(&app), good(0.5)).unwrap_or_else(|e| panic!("{e}"));
    support::until_seen(&mut app, "the checks read", |a| mine(a) == Some(0.5));
    support::settle_until_the_index_is_quiet(&mut app);
    let before = reads(&app);

    // The project now names the file in its `.gitignore`: the index stops listing it, and so cannot say it changes.
    std::fs::write(root(&app).join(".gitignore"), "checks.yaml\n").unwrap_or_else(|e| panic!("{e}"));
    support::until_seen(&mut app, "the ignored file refused", |a| support::verdict(a, "checks")["outcome"] == json!("refused"));
    let v = support::verdict(&mut app, "checks");
    assert!(v["reason"].as_str().unwrap_or_default().contains(".gitignore"), "the refusal says why: {v}");
    // **One `.gitignore` change reaches the view as two publications**: the index publishes the `.gitignore` entry, then walks again and
    // publishes the file leaving the listing. A look made between them still finds the file listed and reads it, which is right at that
    // moment, so how many looks it took is not the property and is not counted. The property is what holds from the refusal on: the view
    // looked at least once (it is the `.gitignore` it read that says why), and the file is not read again, whatever happens to it.
    assert!(reads(&app) > before, "the view looked, and it is the `.gitignore` it read that says why");
    support::settle_until_the_index_is_quiet(&mut app);
    let refused_at = reads(&app);
    // **An edit no event announces**: the file is valid and different. A view that read on a clock would read inside this window.
    std::fs::write(path(&app), good(0.9)).unwrap_or_else(|e| panic!("{e}"));
    support::pump(&mut app, EDIT_WATCHED_FOR);
    assert_eq!(reads(&app), refused_at, "a file the index cannot follow is not read, though it changed");
    assert_eq!(mine(&app), Some(0.5), "the view holds the checks it last read, not an edit no event announced");
}

#[test]
fn a_file_beside_a_checks_file_that_is_not_there_makes_the_view_look_at_nothing() {
    let (mut app, _ctx) = support::editor("checks_follow_neighbour_missing");
    let beside = path(&app);
    assert!(!beside.exists(), "this project has no checks file");
    support::a_neighbour_is_not_read(&mut app, &beside, reads);
}

/// **A rule that names the file after the view looked is looked at again**, though the file's own stamp never moved because it
/// was not there: the root's `.gitignore` is one of the stamps a look is made at.
#[test]
fn a_rule_that_names_a_missing_checks_file_refuses_it_when_it_is_made() {
    let (mut app, _ctx) = support::editor("checks_follow_rule_later");
    support::settle_until_the_index_is_quiet(&mut app);
    assert!(!path(&app).exists());
    std::fs::write(root(&app).join(".gitignore"), "checks.yaml\n").unwrap_or_else(|e| panic!("{e}"));
    support::until_seen(&mut app, "the rule noticed", |a| support::verdict(a, "checks")["outcome"] == json!("refused"));
    let v = support::verdict(&mut app, "checks");
    assert!(v["reason"].as_str().unwrap_or_default().contains(".gitignore"), "{v}");
}

/// **A symbolic link is never listed, so a checks file that is one is refused**, not read once and then never again.
#[cfg(unix)]
#[test]
fn a_checks_file_that_is_a_symbolic_link_is_refused() {
    let (mut app, _ctx) = support::editor("checks_follow_symlink");
    let target = root(&app).join("assets/blueprint/real-checks.txt");
    std::fs::write(&target, good(0.5)).unwrap_or_else(|e| panic!("{e}"));
    std::os::unix::fs::symlink(&target, path(&app)).unwrap_or_else(|e| panic!("{e}"));
    // **A link that replaces a file that was not there is noticed at the next look**, which nothing in the link's own stamp causes (the
    // index lists no link): here the root's `.gitignore` changing is what makes the look.
    std::fs::write(root(&app).join(".gitignore"), "unrelated-pattern\n").unwrap_or_else(|e| panic!("{e}"));
    support::until_seen(&mut app, "the link refused", |a| support::verdict(a, "checks")["outcome"] == json!("refused"));
    let v = support::verdict(&mut app, "checks");
    assert!(v["reason"].as_str().unwrap_or_default().contains("symbolic link"), "{v}");
}
