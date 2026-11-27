//! **A workspace with one panel folds its tab into the chrome bar, and the tab row is gone** (#670; the owner chose
//! "fold", 2026-11-27).
//!
//! The review measured the MAP workspace naming itself three times in the top 153 px at 1600×1000: the chrome bar's
//! `MAP: bedroom`, then a 42 px row (35 logical px at the editor's 1.2 scale) holding one tab, `MAP`, beside
//! `Switch to Kit`. Nine of the ten workspaces have one panel ([`Workspace::panels`]), so that row held one word the
//! bar already drew. Now a one-panel workspace's tab stands in the chrome bar, after the way out; the bar's far end
//! says the subject alone, because the tab beside it names the workspace; and the row lays out to nothing. Kit, the
//! one workspace with three panels, keeps its row, so the rule is the count of panels and not a list of names.

use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use bevy_dark_factory::chrome::{Frame, WhereYouAre};
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::{Mode, OtherWorkspaceMark, Tab};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// Frames the editor is given to lay out after it opens and after a resize.
const SETTLE_FRAMES: usize = 12;

/// The issue's window, 1600×1000 logical, in physical texels at the harness's `UiScale` of 1.2.
const ISSUE_WINDOW: (u32, u32) = (1920, 1200);

/// The narrow window the acceptance also names, 1100×700 logical.
const NARROW_WINDOW: (u32, u32) = (1320, 840);

/// A rectangle's edges may disagree by this much: layout rounds to physical pixels.
const SLACK_PX: f32 = 0.5;

/// The map the fixture opens, which is the subject the chrome bar names.
const MAP: &str = "m";

fn editor_on(workspace: Workspace, window: (u32, u32)) -> App {
    let root = fixtures::Fixture::new(&format!("fold_{}_{}x{}_{}", workspace.id(), window.0, window.1, std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build(MAP);
    let mut app = harness::build_headless_at(&root, MAP, None, workspace.opens_on()).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, window.0, window.1).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    assert_eq!(app.world().resource::<Workspace>(), &workspace, "the editor did not open on {}", workspace.title());
    app
}

/// A laid-out node's rectangle in logical px, `None` when it has no size.
fn rect(app: &App, entity: Entity) -> Option<Rect> {
    let node = app.world().get::<ComputedNode>(entity)?;
    let at = app.world().get::<UiGlobalTransform>(entity)?;
    let scale = node.inverse_scale_factor();
    let size = node.size() * scale;
    (size.x > 0.0 && size.y > 0.0).then(|| Rect::from_center_size(at.translation * scale, size))
}

fn inside(inner: Rect, outer: Rect) -> bool {
    inner.min.x >= outer.min.x - SLACK_PX
        && inner.min.y >= outer.min.y - SLACK_PX
        && inner.max.x <= outer.max.x + SLACK_PX
        && inner.max.y <= outer.max.y + SLACK_PX
}

/// The words of every laid-out text above the body's top edge.
fn words_above_the_body(app: &mut App) -> Vec<String> {
    let body = app.world().resource::<Frame>().body;
    let top = rect(app, body).expect("the body is laid out").min.y;
    let texts: Vec<(Entity, String)> = {
        let world = app.world_mut();
        world.query::<(Entity, &Text)>().iter(world).map(|(e, t)| (e, t.0.clone())).collect()
    };
    texts
        .into_iter()
        .filter(|(e, words)| !words.is_empty() && rect(app, *e).is_some_and(|r| r.max.y <= top + SLACK_PX))
        .map(|(_, words)| words)
        .collect()
}

/// **The MAP workspace names itself once above the body**, at the issue's window and the narrow one: the acceptance's
/// own count (laid-out texts that are exactly `MAP`) and the one a reader makes (any text that starts with the name,
/// which is how the bar's `MAP: m` said it a second time).
///
/// Fails before the change: the bar said `MAP: m` beside the row's `MAP`, two names for one workspace.
#[test]
fn the_map_workspace_names_itself_once_above_the_body() {
    for window in [ISSUE_WINDOW, NARROW_WINDOW] {
        let mut app = editor_on(Workspace::Map, window);
        let words = words_above_the_body(&mut app);
        let exactly = words.iter().filter(|w| w.as_str() == "MAP").count();
        let naming = words.iter().filter(|w| w.as_str() == "MAP" || w.starts_with("MAP:")).count();
        assert_eq!(exactly, 1, "at {window:?} the texts exactly `MAP` above the body: {words:?}");
        assert_eq!(naming, 1, "at {window:?} the workspace is named {naming} times above the body: {words:?}");
        let said: Vec<String> = {
            let world = app.world_mut();
            world.query_filtered::<&Text, With<WhereYouAre>>().iter(world).map(|t| t.0.clone()).collect()
        };
        assert_eq!(said, vec![MAP.to_owned()], "the bar's far end says the map alone, beside the tab that says MAP");
    }
}

/// **Every one-panel workspace's tab stands in the chrome bar and its row lays out to nothing; Kit keeps a row.**
///
/// Fails before the change: every workspace laid out its tab row, 35 logical px, under the chrome bar.
#[test]
fn a_one_panel_workspace_has_no_tab_row_and_kit_keeps_its_own() {
    for workspace in Workspace::ALL {
        let mut app = editor_on(workspace, ISSUE_WINDOW);
        let (chrome_bar, strip, body) = {
            let frame = app.world().resource::<Frame>();
            (frame.chrome_bar, frame.door_strip, frame.body)
        };
        let bar = rect(&app, chrome_bar).expect("the chrome bar is laid out");
        let tabs: Vec<(Mode, Rect)> = {
            let world = app.world_mut();
            let found: Vec<(Entity, Mode)> = world.query::<(Entity, &Tab)>().iter(world).map(|(e, t)| (e, t.0)).collect();
            found.into_iter().filter_map(|(e, mode)| rect(&app, e).map(|r| (mode, r))).collect()
        };
        // By label and sorted: a query walks archetypes, and the selected tab carries a component the others do not.
        let mut drawn: Vec<&str> = tabs.iter().map(|(mode, _)| mode.label()).collect();
        let mut wanted: Vec<&str> = workspace.panels().iter().map(|mode| mode.label()).collect();
        drawn.sort_unstable();
        wanted.sort_unstable();
        assert_eq!(drawn, wanted, "the {} workspace's tabs", workspace.title());
        let row = rect(&app, strip).map_or(0.0, |r| r.height());
        let body_top = rect(&app, body).expect("the body is laid out").min.y;
        if workspace.folds_its_tabs_into_the_chrome_bar() {
            assert!(row <= SLACK_PX, "{} has one panel and still lays out a tab row {row} px tall", workspace.title());
            for (mode, tab) in &tabs {
                assert!(inside(*tab, bar), "{}'s `{}` tab at {tab:?} is not inside the chrome bar {bar:?}", workspace.title(), mode.label());
            }
            // The way to the other workspace folds with the tab, so a click on its name still crosses.
            let mark = {
                let world = app.world_mut();
                world.query_filtered::<Entity, With<OtherWorkspaceMark>>().iter(world).next()
            };
            let mark = mark.and_then(|m| rect(&app, m)).unwrap_or_else(|| panic!("{} draws no way to the other workspace", workspace.title()));
            assert!(inside(mark, bar), "{}'s way to the other workspace at {mark:?} is not in the chrome bar {bar:?}", workspace.title());
            // **And nothing stands between the bands and the body**: it starts where the toolbar ends on a workspace
            // that has one (#690), and where the chrome bar ends on one that has not.
            let toolbar = {
                let world = app.world_mut();
                world.query_filtered::<Entity, With<bevy_dark_factory::toolbar::Toolbar>>().iter(world).next()
            };
            let above = toolbar.and_then(|t| rect(&app, t)).map_or(bar.max.y, |t| t.max.y);
            assert!(
                (body_top - above).abs() <= SLACK_PX,
                "{}'s body starts at {body_top}, {} px below the last band above it",
                workspace.title(),
                body_top - above
            );
        } else {
            assert!(row > SLACK_PX, "{} has {} panels and lost its tab row", workspace.title(), workspace.panels().len());
            let row_rect = rect(&app, strip).expect("the row is laid out");
            for (mode, tab) in &tabs {
                assert!(inside(*tab, row_rect), "{}'s `{}` tab is not in its row", workspace.title(), mode.label());
            }
        }
    }
}
