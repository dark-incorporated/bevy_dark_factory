//! **A bottom dock with nothing to report is its own tab bar** (#662).
//!
//! Measured on `bedroom` at 1600 x 1000 with no live game: the strip laid out 178 px tall (17.80% of
//! the window) for one dim line, `no game connected: the editor reads a game's verdicts only with
//! --live`, and the art got 32.30%. The strip's content is `reload_files::panel_lines`, and when every
//! line it draws is `Tone::Dim` nobody learns anything from it.
//!
//! What each test asserts is the laid-out box, which is what the issue measured itself:
//!
//! - with no news the strip is at most 40 px, and the tab bar is still drawn in it;
//! - a click on that tab bar opens it to the full 148-px row, which is the door the acceptance asks to
//!   keep reachable;
//! - with news the strip is at its full height, so nothing is lost when there is something to say;
//! - and the viewport's share of the window rises by what the strip gave back.
//!
//! # Why the tests write `ReloadNews` rather than a project file
//!
//! "Has news" is `refresh_reload_list`'s answer, read from the same walk that builds the lines. A test
//! that wrote a game verdict to disk and waited for the index could not tell a folded strip from a
//! strip that had not walked yet, and the assertion would pass for the wrong reason. Writing the
//! resource is writing the answer the panel publishes — the fold's *input* — which is exactly what a
//! layout test should hand the layout.

use bevy::prelude::*;
use bevy::ui::ComputedNode;

use bevy_dark_factory::chrome::{BottomDock, Frame};
use bevy_dark_factory::editor::ReloadNews;
use bevy_dark_factory::shell_view::ShellView;
use bevy_dark_factory::style::metrics;

use crate::fixtures;

/// The issue's own window, and the one its first two boxes are measured at.
const WIDE: (u32, u32) = (1600, 1000);
/// The issue's second size: the docks are the same width there, so the art's share differs.
const NARROW: (u32, u32) = (1100, 700);

fn editor(tag: &str, (width, height): (u32, u32), news: bool) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    bevy_dark_factory::harness::resize_surface(&mut app, width, height).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..12 {
        app.update();
    }
    // **The answer the panel publishes**, written after the first walk so it is not overwritten by it.
    app.world_mut().resource_mut::<ReloadNews>().0 = Some(news);
    for _ in 0..6 {
        app.update();
    }
    app
}

fn size_of(app: &App, entity: Entity) -> Vec2 {
    app.world().get::<ComputedNode>(entity).map_or(Vec2::ZERO, |n| n.size() * n.inverse_scale_factor)
}

fn dock(app: &App) -> Vec2 {
    size_of(app, app.world().resource::<Frame>().bottom)
}

fn viewport(app: &App) -> Vec2 {
    size_of(app, app.world().resource::<Frame>().viewport)
}

fn window(app: &App) -> Vec2 {
    size_of(app, app.world().resource::<Frame>().root)
}

/// **With no news the strip is at most 40 px, and the RELOAD tab is still drawn in it** — the issue's
/// first box and the first half of its second.
#[test]
fn a_dock_with_no_news_is_at_most_forty_pixels_and_keeps_its_tab() {
    for window in [WIDE, NARROW] {
        let mut app = editor("dock_empty", window, false);
        let strip = dock(&app);
        assert!(
            strip.y <= 40.0 && strip.y > 0.0,
            "at {window:?} a dock with no news is its tab bar, at most 40 px: {strip:?}"
        );
        assert!(
            (strip.y - metrics().layout.bottom_dock_tab_h).abs() <= 2.0,
            "and it stands at the sheet's own `bottom_dock_tab_h` ({}): {strip:?}",
            metrics().layout.bottom_dock_tab_h
        );
        // **The door back is still there**: the tab that opens the panel is drawn, which is what the
        // acceptance means by "the RELOAD tab is still reachable".
        let (tabs, visible) = {
            let world = app.world_mut();
            let tabs: Vec<Entity> = world.query_filtered::<Entity, With<bevy::ui_widgets::Tab>>().iter(world).collect();
            let shown = tabs.iter().filter(|entity| world.get::<ComputedNode>(**entity).is_some_and(|n| n.size().y > 0.0)).count();
            (tabs.len(), shown)
        };
        assert!(tabs > 0, "at {window:?} the dock draws no tab at all");
        assert!(visible > 0, "at {window:?} every tab of the folded dock is laid out to nothing: {tabs} tabs, {visible} drawn");
    }
}

/// **A click on the tab bar opens the strip to its full height**, and says so in the View menu's tick.
///
/// The acceptance asks that the RELOAD tab "is still reachable in that state (a click or chord raises
/// the strip to its full 148 px)". The observer takes a `PointerClick` on the tab strip; this sends one
/// and asserts the strip's own laid-out height, which is the thing a person sees.
#[test]
fn a_click_on_the_folded_strips_tab_opens_it_to_its_full_height() {
    let mut app = editor("dock_reopen", WIDE, false);
    let folded = dock(&app);
    assert!(folded.y <= 40.0, "the strip starts folded: {folded:?}");

    let tab = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy::ui_widgets::Tab>>().iter(world).next().expect("the dock's tab")
    };
    app.world_mut().trigger(bevy::picking::events::PointerClick {
        button: bevy::picking::pointer::PointerButton::Primary,
        pointer: bevy::picking::pointer::PointerId::Mouse,
        entity: tab,
        original_event_target: tab,
    }.into());
    for _ in 0..6 {
        app.update();
    }

    assert!(app.world().resource::<ShellView>().reload_shown, "the click asked for the panel, as the View menu's tick does");
    let opened = dock(&app);
    assert!(
        (opened.y - metrics().layout.bottom_dock_h).abs() <= 2.0,
        "the strip is the full `bottom_dock_h` ({}): {opened:?}",
        metrics().layout.bottom_dock_h
    );
}

/// **With news the strip is at its present height** — the issue's third box, "nothing is lost when
/// there is something to say".
#[test]
fn a_dock_with_news_is_the_full_height_it_always_was() {
    for window in [WIDE, NARROW] {
        let mut app = editor("dock_news", window, true);
        let strip = dock(&app);
        assert!(
            (strip.y - metrics().layout.bottom_dock_h).abs() <= 2.0,
            "at {window:?} a dock with news is the full `bottom_dock_h` ({}): {strip:?}",
            metrics().layout.bottom_dock_h
        );
    }
}

/// **The room the fold gives back is the viewport's** — the half of the issue's last box that is a
/// number rather than a picture.
///
/// The dock's full height is 148 px plus its tab bar; folding it to 29 px must give the body that
/// difference. Measured as a difference between two editors at the same window, so the docks' own
/// widths do not enter into it.
#[test]
fn the_folded_strip_gives_its_room_to_the_viewport() {
    let mut folded = editor("dock_room_folded", WIDE, false);
    let mut full = editor("dock_room_full", WIDE, true);
    let (folded_strip, full_strip) = (dock(&folded), dock(&full));
    let gave_back = full_strip.y - folded_strip.y;
    assert!(gave_back > 100.0, "the fold is worth more than a hairline: {folded_strip:?} against {full_strip:?}");

    let (folded_art, full_art) = (viewport(&folded), viewport(&full));
    assert!(
        folded_art.y >= full_art.y + gave_back - 2.0,
        "the viewport grew by what the strip gave back ({gave_back:.0} px): {full_art:?} to {folded_art:?}"
    );
    // And the share, which is what the issue's table is in.
    let share = |viewport: Vec2, window: Vec2| viewport.x * viewport.y / (window.x * window.y);
    let (before, after) = (share(full_art, window(&full)), share(folded_art, window(&folded)));
    assert!(
        after > before + 0.10,
        "at {WIDE:?} the art's share rises from {:.2}% to {:.2}% when the strip folds",
        before * 100.0,
        after * 100.0
    );
}

/// **A dim line is not news, and anything else is** — the predicate the fold reads, over the tones the
/// panel actually draws.
#[test]
fn only_a_dim_line_is_no_news() {
    use bevy_dark_factory::reload_files::{Tone, reports_nothing};
    let dim = |text: &str| (text.to_owned(), Tone::Dim);
    assert!(reports_nothing(&[dim("no game connected: the editor reads a game's verdicts only with --live (7 files)")]), "the header alone is nothing to report");
    assert!(reports_nothing(&[]), "an empty panel reports nothing");
    for tone in [Tone::Text, Tone::Accent, Tone::Danger] {
        assert!(
            !reports_nothing(&[dim("no game connected (7 files)"), ("site/wall.yaml · game refused: …".to_owned(), tone)]),
            "{tone:?} is a file's news, so the strip stays open"
        );
    }
}
