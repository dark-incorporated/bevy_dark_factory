//! **Stamps the build with the commit it was made from** (DL-01, #460), as `BUILD_COMMIT`.
//!
//! The turn ledger records which build wrote each label, because a label made by an editor that has since
//! changed is a label about different behaviour. A release build has no git beside it, so the commit can
//! also be named by `GRIMOIRE_BUILD_COMMIT`, which wins; with neither, the stamp is `unknown` and the
//! ledger says so rather than inventing one.
//!
//! The script reruns when `HEAD` or the branch it names moves, so the stamp is the commit of the build
//! and not of whenever the script last ran.

use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned()).filter(|s| !s.is_empty())
}

fn main() {
    println!("cargo:rerun-if-env-changed=GRIMOIRE_BUILD_COMMIT");
    println!("cargo:rerun-if-changed=build.rs");
    // **`--git-path`, not `.git/HEAD`**: in a worktree `.git` is a file, and the real `HEAD` is elsewhere.
    if let Some(head) = git(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head}");
    }
    if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"])
        && let Some(reference) = git(&["rev-parse", "--git-path", &branch])
    {
        println!("cargo:rerun-if-changed={reference}");
    }
    let commit = std::env::var("GRIMOIRE_BUILD_COMMIT")
        .ok()
        .filter(|c| !c.trim().is_empty())
        .or_else(|| git(&["rev-parse", "HEAD"]))
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=BUILD_COMMIT={commit}");
}
