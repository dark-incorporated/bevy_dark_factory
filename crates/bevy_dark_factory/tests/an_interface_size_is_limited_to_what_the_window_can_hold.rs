//! **The interface size never takes more window than there is** (#537).
//!
//! The Interface size setting runs 50% to 300%, and on the capture's window (1280 x 802 logical) the bars and a panel's own heading fill the
//! window above 200% (text 100%): a list has no row to be read in. The editor now draws at the size the window can hold
//! (`surface::fit_interface_scale`), keeps the person's choice in the file, and says so beside the slider.
//!
//! # Where the expected values come from
//!
//! Not from the function. They are the table measured by running the census's own floors (a panel shows a text; its content box and its scroll
//! viewport are each at least one text row tall) in three window heights at four text sizes and eleven interface sizes, without any limit
//! (`COLLAPSE` measured on `fleet/w537` before the function existed): for each pair, the largest size at which the floors held, and the first at
//! which they failed.
//!
//! | window height (logical px) | text | holds up to | first fails at |
//! |---|---|---|---|
//! | 480  | 100% | 1.00 | 1.25 |
//! | 480  | 150% | 0.75 | 1.00 |
//! | 480  | 200% | 0.50 | 0.75 |
//! | 802 (the capture's) | 100% | 2.00 | 2.25 |
//! | 802  | 150% | 1.50 | 1.75 |
//! | 802  | 200% | 1.00 | 1.25 |
//! | 1200 | 100% | 2.75 | 3.00 |
//! | 1200 | 150% | 2.25 | 2.50 |
//! | 1200 | 200% | 1.75 | 2.00 |
//!
//! At text 300% the table is not a ladder: the shell shows the 148-px bottom dock once the body keeps 420 px beside it, and a panel at 300% needs
//! more than 420, so there is a band of heights where the dock takes the room the panel's list needed. A choice that lands in it is given the
//! size at its far end. At 802 px and text 300% the measured grid held at 0.50 and failed at 0.75 and above, and the function's answer for 3.0
//! (about 0.65) is checked in the real editor by `the_floors_hold_in_a_shorter_and_a_taller_window_at_the_top_of_the_range`, not by this table.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use bevy_dark_factory::settings::{EditorSettings, UserSettingsFile};
use bevy_dark_factory::settings_screen::OpenSettings;
use bevy_dark_factory::surface::{InterfaceFit, fit_interface_scale};

use crate::fixtures;

const WIDTH: f32 = 1280.0;

/// `(window height, text size, largest size the floors held at, first size they failed at)`: the measured table above. `None` where the measured
/// grid is not a ladder (text 300%), so only the lower bound is a measurement.
const MEASURED: [(f32, f32, f32, Option<f32>); 12] = [
    (480.0, 1.0, 1.0, Some(1.25)),
    (480.0, 1.5, 0.75, Some(1.0)),
    (480.0, 2.0, 0.5, Some(0.75)),
    (480.0, 3.0, 0.5, None),
    (802.0, 1.0, 2.0, Some(2.25)),
    (802.0, 1.5, 1.5, Some(1.75)),
    (802.0, 2.0, 1.0, Some(1.25)),
    (802.0, 3.0, 0.5, None),
    (1200.0, 1.0, 2.75, Some(3.0)),
    (1200.0, 1.5, 2.25, Some(2.5)),
    (1200.0, 2.0, 1.75, Some(2.0)),
    (1200.0, 3.0, 0.75, None),
];

#[test]
fn the_top_of_the_range_is_cut_to_what_the_measured_floors_held() {
    for (height, text, holds, fails) in MEASURED {
        let fit = fit_interface_scale(WIDTH, height, text, 3.0);
        assert!(fit.limited, "a window {height} px tall at text {text} cannot hold 300%: {fit:?}");
        assert!(fit.effective >= holds - 1e-3, "{height} px, text {text}: the floors held at {holds} but the limit is {}", fit.effective);
        if let Some(fails) = fails {
            assert!(fit.effective < fails, "{height} px, text {text}: the floors failed at {fails} but the limit is {}", fit.effective);
        }
    }
}

#[test]
fn a_size_the_window_holds_is_left_as_it_is() {
    for (height, text, holds, _) in MEASURED {
        let fit = fit_interface_scale(WIDTH, height, text, holds);
        assert_eq!(fit, InterfaceFit { effective: holds, limited: false }, "{height} px, text {text}: {holds} held in the measurement");
    }
}

#[test]
fn a_choice_in_the_dock_band_is_given_the_size_past_it() {
    // Measured at 802 px and text 300%: 0.75 failed (the dock was shown and the lists had no row), 0.50 held.
    let fit = fit_interface_scale(WIDTH, 802.0, 3.0, 0.75);
    assert!(fit.limited && fit.effective < 0.75 && fit.effective >= 0.5, "{fit:?}");
    assert_eq!(fit_interface_scale(WIDTH, 802.0, 3.0, 0.5), InterfaceFit { effective: 0.5, limited: false });
}

#[test]
fn a_window_too_short_for_the_floor_is_not_helped_and_does_not_say_it_limited_the_floor() {
    // Measured: at 0.5 and text 300% the floors fail from 240 to 420 logical px tall.
    let fit = fit_interface_scale(WIDTH, 300.0, 3.0, 3.0);
    assert_eq!(fit.effective, 0.5, "never below the setting's floor");
    assert!(fit.limited, "300% was asked for and 50% is what the window can give");
    assert_eq!(fit_interface_scale(WIDTH, 300.0, 3.0, 0.5), InterfaceFit { effective: 0.5, limited: false });
}

#[test]
fn a_hand_edited_value_outside_the_range_is_clamped_first() {
    assert_eq!(fit_interface_scale(WIDTH, 1200.0, 1.0, 0.01), InterfaceFit { effective: 0.5, limited: false });
    assert!(fit_interface_scale(WIDTH, 1200.0, 1.0, 100.0).effective <= 3.0);
}

fn rig(tag: &str, interface_scale: f32) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("site/wall", "alpha").build("m");
    let user = root.join("user_config/settings.yaml");
    std::fs::create_dir_all(user.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&user, format!("version: 1\ntext_scale: 1.0\ninterface_scale: {interface_scale}\n")).unwrap_or_else(|e| panic!("{e}"));
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(UserSettingsFile(user.clone()));
    app.world_mut().spawn((Window { resolution: WindowResolution::new(1280, 802), ..default() }, PrimaryWindow));
    for _ in 0..8 {
        app.update();
    }
    (app, user)
}

fn resize(app: &mut App, height: u32) {
    let world = app.world_mut();
    let mut windows = world.query_filtered::<&mut Window, With<PrimaryWindow>>();
    if let Ok(mut window) = windows.single_mut(world) {
        window.resolution.set_physical_resolution(1280, height);
    }
    for _ in 0..8 {
        app.update();
    }
}

/// The words of the line under the Interface size row, read off the rendered texts (empty when there is none).
fn note(app: &mut App) -> String {
    let world = app.world_mut();
    world.query::<&Text>().iter(world).map(|t| t.0.clone()).find(|t| t.starts_with("Limited to ")).unwrap_or_default()
}

#[test]
fn the_slider_says_when_the_window_limits_it_and_stops_when_it_does_not() {
    let (mut app, user) = rig("interface_note", 2.5);
    app.world_mut().write_message(OpenSettings);
    for _ in 0..6 {
        app.update();
    }
    // 802 px, text 100%: the measured floors held at 2.00 and failed at 2.25.
    let said = note(&mut app);
    let percent: u32 = said
        .strip_prefix("Limited to ")
        .and_then(|rest| rest.strip_suffix("% by this window."))
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("the line is not 'Limited to N% by this window.': {said:?}"));
    assert!((200..225).contains(&percent), "{said}");
    // The editor draws at that size, and the choice is still the person's.
    let drawn = app.world().resource::<UiScale>().0;
    assert!((drawn - 1.2 * percent as f32 / 100.0).abs() < 0.03, "UiScale {drawn} for {said}");
    assert_eq!(app.world().resource::<EditorSettings>().interface_scale, 2.5, "the clamp never rewrites the stored choice");
    assert!(std::fs::read_to_string(&user).unwrap_or_default().contains("interface_scale: 2.5"), "nor the file");

    // A taller window holds the whole choice: the line goes, and the full size is back.
    resize(&mut app, 1600);
    assert_eq!(note(&mut app), "", "a window that holds 250% says nothing");
    let drawn = app.world().resource::<UiScale>().0;
    assert!((drawn - 1.2 * 2.5).abs() < 1e-3, "UiScale {drawn}");
    assert_eq!(app.world().resource::<EditorSettings>().interface_scale, 2.5);

    // And a short one limits it again, without anyone touching the setting.
    resize(&mut app, 802);
    assert!(note(&mut app).starts_with("Limited to "), "{}", note(&mut app));
    assert!(std::fs::read_to_string(&user).unwrap_or_default().contains("interface_scale: 2.5"));
}
