//! **An app that ends puts the process-global text scale back** (#497).
//!
//! `chrome::TEXT_SCALE` is a `static`, so it outlives every `App` in the process. `ui::follow_the_text_size`
//! copies a person's setting into it every frame, and before this fix nothing wrote it back when the app
//! stopped — so a test binary running many apps in one process had each app inherit whatever the previous
//! one left. A layout test could then measure a scale it never staged, which is why the failure depended on
//! which tests shared a process: `c1 + c6` failed while either branch alone passed.
//!
//! The invariant these two tests hold: **after an app is dropped, `chrome::text_scale()` reads 1.0.**

use bevy::prelude::*;

use bevy_dark_factory::chrome;
use bevy_dark_factory::settings::UserSettingsFile;
use bevy_dark_factory::harness;

use crate::fixtures;

/// A project whose settings file states `text_scale: 1.5`, which is inside the slider's own range.
fn project_at(tag: &str, text_scale: f32) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id()))
        .descriptor("lamp_tall", "alpha")
        .build("m");
    let user = root.join(format!("user_settings_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(user.parent().expect("a settings folder")).expect("a settings folder");
    std::fs::write(
        &user,
        format!("version: 1\ntext_scale: {text_scale}\ninterface_scale: 1.0\n"),
    )
    .expect("a settings file");
    (root, user)
}

/// **An app at 1.5 leaves nothing behind.** Three halves: the app really did write 1.5 while it ran (or this
/// proves nothing), and the scale is back to 1.0 once it is dropped.
#[test]
fn an_app_at_one_and_a_half_leaves_one_behind() {
    let (root, settings) = project_at("text_scale_restored", 1.5);

    {
        let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
        app.insert_resource(UserSettingsFile(settings));
        for _ in 0..4 {
            app.update();
        }
        let while_running = chrome::text_scale();
        assert!(
            (while_running - 1.5).abs() < 0.01,
            "**the app wrote {while_running}, not the 1.5 its settings file states** — so this test is \
             not exercising the write at all and would pass even with no restore"
        );
        drop(app);
    }

    assert!(
        (chrome::text_scale() - 1.0).abs() < 0.001,
        "**the scale is {} after the app was dropped, not 1.0** (#497): `chrome::TEXT_SCALE` is a process \
         `static`, so the next app in this test binary inherits a size its own settings never asked for. \
         A layout test then measures a scale it did not stage.",
        chrome::text_scale()
    );
}

/// **A second app is not poisoned by the first.** The order dependence itself, stated as a test: build at
/// 1.5, drop, then build one whose settings say 1.0 and check it really measures 1.0.
#[test]
fn the_next_app_measures_its_own_scale_not_the_last_ones() {
    let (bright, bright_settings) = project_at("text_scale_first", 1.6);
    {
        let mut first = harness::build_headless(&bright, "m", None).unwrap_or_else(|e| panic!("{e}"));
        first.insert_resource(UserSettingsFile(bright_settings));
        for _ in 0..4 {
            first.update();
        }
        assert!(
            (chrome::text_scale() - 1.6).abs() < 0.01,
            "the first app wrote its own 1.6"
        );
    }

    let (plain, plain_settings) = project_at("text_scale_second", 1.0);
    let mut second = harness::build_headless(&plain, "m", None).unwrap_or_else(|e| panic!("{e}"));
    second.insert_resource(UserSettingsFile(plain_settings));
    for _ in 0..4 {
        second.update();
    }
    assert!(
        (chrome::text_scale() - 1.0).abs() < 0.01,
        "**the second app reads {} but its own settings say 1.0** (#497): it inherited the first app's \
         scale, which is the order dependence that made `c1 + c6` fail while either alone passed.",
        chrome::text_scale()
    );
}
