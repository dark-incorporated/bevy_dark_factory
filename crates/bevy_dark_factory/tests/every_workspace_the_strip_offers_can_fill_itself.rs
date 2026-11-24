//! **Every workspace the tab strip offers can fill itself** (#600).
//!
//! Measured 2026-11-24 on a default build: the Sound workspace's whole content was
//! `This build has no audio panel. Build the editor with the bevy_siren feature to edit cues and buses`
//! — an instruction to rebuild the editor. The panel was honest and the build was not: a person opens
//! Sound to edit sound and is told the editor cannot.
//!
//! # The rule this pins
//!
//! **A tab the strip offers draws a panel, and that panel is never only an apology.** Two halves, and
//! both are asserted:
//!
//! - `bevy_siren` is a default feature (the owner's decision, 2026-11-24), so a default build's Sound
//!   workspace draws the audio panel's three lists — the buses, the cues and the log;
//! - a build that turns the feature off does not offer the Sound tab at all
//!   ([`Workspace::missing`](bevy_dark_factory::workspace::Workspace::missing)), because a tab that can
//!   only say "this build has no …" is the defect.
//!
//! The apology constant [`NO_AUDIO`](bevy_dark_factory::workspace_panels::NO_AUDIO) stays: the
//! `#[cfg(not(feature = "bevy_siren"))]` arm still has to draw something, and a person on a build that
//! reaches that arm by hand deserves the sentence. What is pinned is that no build *offers the tab*
//! and reaches it.
//!
//! # Why the three lists are read structurally
//!
//! A heading over three empty columns is the shape the panel fills every frame to avoid, so the test
//! reads the columns themselves — `BusList`, `CueChooser` and `LogTail` — and each one's drawn rows.
//! A `Text` that says `AUDIO` over nothing is exactly what an apology looks like from the outside.

use bevy::prelude::*;

use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;
use crate::settle_support;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("sound_home_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    app
}

fn switch_to(app: &mut App, workspace: Workspace) {
    let id = format!("workspace.{}", workspace.id());
    let command = commands::by_id(&id).unwrap_or_else(|| panic!("`{id}` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(app);
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "the `{id}` command did not open {}", workspace.title());
}

/// Is a heading with exactly these words drawn — visible and laid out with a box?
fn shows(app: &mut App, label: &str) -> bool {
    let world = app.world_mut();
    let mut q = world.query::<(&Text, &InheritedVisibility, &ComputedNode)>();
    q.iter(world).any(|(text, shown, node)| shown.get() && text.0.trim() == label && node.size().x > 0.0 && node.size().y > 0.0)
}

/// **The drawn rows under every entity marked `column`**: the words a person reads in that list.
///
/// A row is a `Text` the column owns, so this walks the column's descendants rather than the whole
/// screen — which is what makes "this list is filled" different from "this workspace draws something".
// **Only the `bevy_siren` arm calls this**: the Sound workspace's columns exist when the crate does, and the test's
// other arms read their own panels. Kept `cfg`-shaped rather than deleted so a build with the crate still checks them.
#[cfg_attr(not(feature = "bevy_siren"), allow(dead_code))]
fn rows_in<C: Component>(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    let columns: Vec<Entity> = world.query_filtered::<Entity, With<C>>().iter(world).collect();
    let mut found = Vec::new();
    for column in columns {
        let mut stack = vec![column];
        while let Some(at) = stack.pop() {
            if let Some(children) = world.get::<Children>(at) {
                stack.extend(children.iter());
            }
            if let Some((text, shown, node)) = world
                .get::<Text>(at)
                .zip(world.get::<InheritedVisibility>(at))
                .zip(world.get::<ComputedNode>(at))
                .map(|((text, shown), node)| (text, shown, node))
                && shown.get()
                && node.size().x > 0.0
                && node.size().y > 0.0
            {
                found.push(text.0.trim().to_owned());
            }
        }
    }
    found
}

/// **A default build's Sound workspace draws the buses, the cues and the log.**
#[cfg(feature = "bevy_siren")]
#[test]
fn a_default_build_draws_the_audio_panel() {
    let mut app = editor("sound_default");
    switch_to(&mut app, Workspace::Sound);
    assert!(shows(&mut app, "AUDIO"), "the Sound workspace does not draw its AUDIO panel");
    assert!(
        !shows(&mut app, bevy_dark_factory::workspace_panels::NO_AUDIO),
        "the Sound workspace says this build has no audio panel: the shipped build cannot edit sound",
    );

    #[cfg(feature = "bevy_siren")]
    {
        use bevy_dark_factory::audio_panel::{AudioPanel, BusList, LogTail};
        use bevy_dark_factory::audio_panel::CueChooser;

        // **The panel's own state names buses**, so a column that drew nothing would be a drawing fault
        // rather than an empty project.
        let buses = app.world().resource::<AudioPanel>().buses.clone();
        assert!(!buses.is_empty(), "the audio panel knows no buses at all, so its bus list cannot be drawn");

        let bus_rows = rows_in::<BusList>(&mut app);
        assert!(!bus_rows.is_empty(), "the bus list is drawn with no rows under it");
        assert!(
            bus_rows.iter().any(|row| row.starts_with(buses.first().map(String::as_str).unwrap_or_default())),
            "the bus list draws {bus_rows:?}, and none of them is the bus the panel says it has: {buses:?}",
        );

        let cue_rows = rows_in::<CueChooser>(&mut app);
        assert!(
            !cue_rows.is_empty(),
            "the cue list is drawn with no rows under it: an empty project must still say it has no cues",
        );

        let log_rows = rows_in::<LogTail>(&mut app);
        assert!(
            !log_rows.is_empty(),
            "the log tail is drawn with no rows under it: a project that has played nothing must say so",
        );
    }
}

/// **Every `Workspace` the tab strip offers draws something, and none of them is only an apology.**
///
/// The list is the strip's own ([`bevy_dark_factory::activity_bar::registered`]), so a workspace the
/// strip would offer and this test would skip cannot exist: offering a tab whose only content is
/// "this build has no …" is the defect #600 names, and the apology is the string it is caught by.
#[cfg(feature = "bevy_siren")]
#[test]
fn no_offered_workspace_draws_only_an_apology() {
    let mut app = editor("sound_no_apology");
    let offered = bevy_dark_factory::activity_bar::registered();
    let mut checked = 0;
    for workspace in offered.iter().copied() {
        switch_to(&mut app, workspace);
        let world = app.world_mut();
        let mut q = world.query::<(&Text, &InheritedVisibility, &ComputedNode)>();
        let drawn: Vec<String> = q
            .iter(world)
            .filter(|(_, shown, node)| shown.get() && node.size().x > 0.0 && node.size().y > 0.0)
            .map(|(text, _, _)| text.0.trim().to_owned())
            .filter(|text| !text.is_empty())
            .collect();
        assert!(!drawn.is_empty(), "the {} workspace draws no text at all", workspace.title());
        let apology = drawn.iter().find(|line| line.starts_with("This build has no")).cloned();
        assert!(
            apology.is_none(),
            "the {} workspace's content says {:?}: the strip offers a tab this build cannot fill",
            workspace.title(),
            apology.unwrap_or_default(),
        );
        checked += 1;
    }
    assert!(checked >= 8, "the strip offered only {checked} workspaces; this test proved nothing");
}
