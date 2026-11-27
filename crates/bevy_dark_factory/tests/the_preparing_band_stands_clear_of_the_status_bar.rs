//! **The "Preparing the view" band never covers the status bar** (#440). Found recording the #430 frames: the band, standing at the window's bottom-left
//! corner, was drawn over the status bar's "Baking previews" text. The band's rect and the status bar's rect must not intersect, at the smallest
//! window the editor supports (1024 x 640) and at 1280 x 720.

use std::time::Duration;

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_dark_factory::chrome::Frame;
use bevy_dark_factory::launcher::LauncherPlugin;
use bevy_dark_factory::loading::{LoadPhase, LoadRequest, LoadTiming, LoadingPlugin, PreparingBand, SettleBudget};
use bevy_dark_factory::screen::Screen;

use crate::fixtures::Fixture;
use crate::settle_support::Frames;

/// The windows the band is laid out in, in logical px.
const WINDOWS_PX: [(u32, u32); 2] = [(1024, 640), (1280, 720)];
/// How long the test lets the band stand, so it is never taken down by the settle rule while it is measured.
const BAND_STANDS: Duration = Duration::from_secs(600);
/// Frames run after the hand-over, for layout to settle.
const SETTLE_FRAMES: usize = 20;
/// The real pause between frames while the project loads.
const LOAD_PAUSE: Duration = Duration::from_millis(5);
/// How long the load has to reach the editor: frames covering 20 s at [`LOAD_PAUSE`], counted so a starved host cannot run it
/// out (#644).
const ENTERED_WITHIN: Frames = Frames::covering(Duration::from_secs(20), LOAD_PAUSE);

/// A node's rect in logical px: `(left, top, right, bottom)`.
fn rect(app: &App, entity: Entity) -> (f32, f32, f32, f32) {
    let computed = app.world().get::<ComputedNode>(entity).unwrap_or_else(|| panic!("{entity} is laid out"));
    let at = app.world().get::<UiGlobalTransform>(entity).unwrap_or_else(|| panic!("{entity} has a place")).translation;
    let scale = computed.inverse_scale_factor();
    let (w, h) = (computed.size().x * scale, computed.size().y * scale);
    let (cx, cy) = (at.x * scale, at.y * scale);
    (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0)
}

fn intersect(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    a.0 < b.2 && b.0 < a.2 && a.1 < b.3 && b.1 < a.3
}

/// **The editor, opened through the loading screen so the band is up**, at the window's size.
fn open(tag: &str, window: (u32, u32)) -> App {
    let root = Fixture::new(&format!("{tag}_{}_{}", window.0, std::process::id())).descriptor("bench", "props").build("m").canonicalize().unwrap_or_else(|e| panic!("{e}"));
    let other = Fixture::new(&format!("{tag}_host_{}_{}", window.0, std::process::id())).descriptor("bench", "props").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&other, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins((LauncherPlugin, LoadingPlugin))
        .insert_resource(LoadRequest(vec![root.to_string_lossy().into_owned(), "m".to_owned()]))
        .insert_resource(LoadTiming { spinner_after: Duration::from_secs(60), progress_after: Duration::from_secs(600) })
        .insert_resource(SettleBudget { frame: Duration::ZERO, quiet_frames: u32::MAX, cap: BAND_STANDS });
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Loading);
    let mut stepped = Frames(0);
    while *app.world().resource::<State<Screen>>().get() != Screen::Editor {
        assert!(stepped < ENTERED_WITHIN, "the editor was never entered (phase {:?})", app.world().get_resource::<LoadPhase>());
        app.update();
        std::thread::sleep(LOAD_PAUSE);
        stepped.0 += 1;
    }
    bevy_dark_factory::harness::resize_surface(&mut app, window.0, window.1).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    app
}

/// **At 1024 x 640 and 1280 x 720 the band's rect and the status bar's do not intersect.**
#[test]
fn the_preparing_band_and_the_status_bar_never_overlap() {
    for window in WINDOWS_PX {
        let mut app = open("band_over_status", window);
        let band = app.world_mut().query_filtered::<Entity, With<PreparingBand>>().single(app.world()).unwrap_or_else(|e| panic!("{window:?}: the band is up: {e}"));
        let status = app.world().resource::<Frame>().status;
        let (band_rect, status_rect) = (rect(&app, band), rect(&app, status));
        assert!(status_rect.3 > status_rect.1, "{window:?}: the status bar is laid out: {status_rect:?}");
        assert!(
            !intersect(band_rect, status_rect),
            "{window:?}: the 'Preparing the view' band {band_rect:?} covers the status bar {status_rect:?}"
        );
    }
}
