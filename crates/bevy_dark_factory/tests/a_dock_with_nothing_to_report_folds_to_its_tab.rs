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

use bevy_dark_factory::chrome::Frame;
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

/// **With no news the strip takes at most 40 px, and the RELOAD tab is still drawn in it** — the issue's
/// first box and the first half of its second.
///
/// **Two acceptable answers, and both are "not 178 px"**: the folded strip (29 px at 1600x1000) or no
/// dock row at all (0 px at 1100x700, where the editor's own room rule already hides it — the issue's
/// art table says 34.85% there, which is a window with no 178-px strip in it). What the acceptance
/// forbids is the bug: a strip taking 17.8% for one dim line.
///
/// The tab assertion is only made where the strip is drawn; a hidden dock's tab has no box to measure,
/// and its door is the View menu's row, which is unchanged.
#[test]
fn a_dock_with_no_news_is_at_most_forty_pixels_and_keeps_its_tab() {
    for window in [WIDE, NARROW] {
        let mut app = editor("dock_empty", window, false);
        let strip = dock(&app);
        assert!(
            strip.y <= 40.0,
            "at {window:?} a dock with no news takes at most 40 px, folded or hidden: {strip:?}"
        );
        if strip.y == 0.0 {
            // **Hidden, which is the editor's room rule and stricter than the fold**: no row at all is
            // less room than a 29-px tab bar, so there is nothing here to complain about.
            continue;
        }
        assert!(
            (strip.y - metrics().layout.bottom_dock_tab_h).abs() <= 2.0,
            "and where it is drawn it stands at the sheet's own `bottom_dock_tab_h` ({}): {strip:?}",
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

    // **The dock's own tab, not just any tab in the editor.** `bevy::ui_widgets::Tab` is on every
    // strip — the door strip, a panel's tabs, this dock's — so the one asked for here is the one whose
    // ancestors reach the bottom dock, the same walk `shell_view::under` makes.
    let tab = {
        use bevy_dark_factory::chrome::BottomDock;
        let world = app.world_mut();
        let bottom = world
            .query_filtered::<Entity, With<BottomDock>>()
            .single(world)
            .expect("the frame has a bottom dock");
        let mut ancestors = world.query::<&ChildOf>();
        let tabs: Vec<Entity> = world.query_filtered::<Entity, With<bevy::ui_widgets::Tab>>().iter(world).collect();
        let mut found = None;
        for candidate in tabs {
            let mut at = ancestors.get(world, candidate).ok().map(|c| c.parent());
            while let Some(entity) = at {
                if entity == bottom {
                    found = Some(candidate);
                    break;
                }
                at = ancestors.get(world, entity).ok().map(|c| c.parent());
            }
            if found.is_some() {
                break;
            }
        }
        found.expect("the bottom dock draws a tab to click")
    };
    // **The click a person's press produces**, built the way `a_lesson_is_kept_or_discarded_in_the_agent_block`
    // builds one: `PointerClick` carries a resolved `Pointer`, a `HitData` and a press count, not an entity and a
    // pointer id. The hit's entity is what the dock's observer walks upward to find its tab strip.
    app.world_mut().commands().trigger(bevy::picking::events::PointerClick {
        entity: tab,
        pointer: bevy::picking::events::Pointer::new(
            bevy::picking::pointer::PointerId::Mouse,
            bevy::picking::pointer::Location {
                target: bevy::camera::NormalizedRenderTarget::Image(bevy::camera::ImageRenderTarget {
                    handle: bevy::asset::Handle::default(),
                    scale_factor: 1.0,
                }),
                position: Vec2::new(40.0, 60.0),
            },
        ),
        button: bevy::picking::pointer::PointerButton::Primary,
        hit: bevy::picking::backend::HitData::new(tab, 0.0, None, None),
        duration: std::time::Duration::ZERO,
        count: 1,
    });
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

/// **Asked for, the strip is at its present height** — the issue's third box, "nothing is lost when
/// there is something to say".
///
/// **`reload_shown` rather than a written `ReloadNews`**: the news resource is *republished* by
/// `refresh_reload_list` on every walk, so a test that wrote `Some(true)` would have it overwritten
/// before the layout read it — measured, the strip came back at 29 px. The tick is the same door with
/// the same effect on the layout (it forces `Display::Flex` and the full height), and it is what a
/// person does when the panel has something they want to read.
///
/// At 1600x1000 only. At 1100x700 the dock is hidden entirely, which is
/// `the_bottom_dock_gives_way_in_the_reference_window`'s rule and not this change's: a 148-px row on a
/// 700-px window with the terminal open takes the workbench below its floor.
#[test]
fn a_dock_that_is_asked_for_is_the_full_height_it_always_was() {
    let mut app = editor("dock_news", WIDE, false);
    app.world_mut().resource_mut::<ShellView>().reload_shown = true;
    for _ in 0..6 {
        app.update();
    }
    let strip = dock(&app);
    assert!(
        (strip.y - metrics().layout.bottom_dock_h).abs() <= 2.0,
        "at {WIDE:?} a dock that is asked for is the full `bottom_dock_h` ({}): {strip:?}",
        metrics().layout.bottom_dock_h
    );
    // **And its page is back in the layout**, because a 148-px row with nothing drawn in it is the
    // other half of the same bug.
    let page = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_dark_factory::editor::ReloadPanel>>().iter(world).next().expect("the reload panel's page")
    };
    assert!(
        size_of(&app, page).y > 40.0,
        "the panel's body is drawn in the row: {:?}",
        size_of(&app, page)
    );
}

/// **The room the fold gives back is the viewport's** — the half of the issue's last box that is a
/// number rather than a picture.
///
/// **The assertion is the relationship, not a threshold I chose.** The viewport must grow by what the
/// strip gave back, and its share of the window must rise by the share of the window that gain is:
/// `Δshare == Δviewport_area / window_area`, to the pixel. A threshold would be a number invented here
/// and could be satisfied by a viewport that grew for another reason.
#[test]
fn the_folded_strip_gives_its_room_to_the_viewport() {
    let folded = editor("dock_room_folded", WIDE, false);
    let full = editor("dock_room_full", WIDE, true);
    let (folded_strip, full_strip) = (dock(&folded), dock(&full));
    let gave_back = full_strip.y - folded_strip.y;
    assert!(gave_back > 100.0, "the fold is worth more than a hairline: {folded_strip:?} against {full_strip:?}");

    let (folded_art, full_art) = (viewport(&folded), viewport(&full));
    assert!(
        folded_art.y >= full_art.y + gave_back - 2.0,
        "the viewport grew by what the strip gave back ({gave_back:.0} px): {full_art:?} to {folded_art:?}"
    );
    // **And the share is that same gain**: the viewport's area grew by the window's width times the
    // height the strip gave back, so its share rose by exactly that over the window's area.
    let share = |viewport: Vec2, window: Vec2| viewport.x * viewport.y / (window.x * window.y);
    let (before, after) = (share(full_art, window(&full)), share(folded_art, window(&folded)));
    let expected_gain = full_art.x * gave_back / (window(&full).x * window(&full).y);
    assert!(
        (after - before - expected_gain).abs() <= 0.01,
        "at {WIDE:?} the art's share rose from {:.2}% to {:.2}%; the strip's {gave_back:.0} px is {:.2} points of the window",
        before * 100.0,
        after * 100.0,
        expected_gain * 100.0
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
