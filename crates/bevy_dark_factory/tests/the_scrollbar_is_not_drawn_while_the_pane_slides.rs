//! **No scrollbar is drawn over a pane that is sliding** (#517, `feedback-1795094031429`).
//!
//! # What the owner saw
//!
//! *"The scroll bar here stays visible during the animation that kicks off when I hit 'Cmd+B'"*. The
//! pick is the PROJECT page's scroll viewport — `Sized > FillRegion > Split > SplitPane > Sized >
//! Sized` — and the bar is drawn over the pane on every frame of the slide.
//!
//! # Why gooey's own rule cannot see it
//!
//! `update_lists` shows a rail exactly while `content > size` (`bevy_gooey/src/list.rs:99`), read off
//! the **last frame's** layout. That is right for a list whose rows came and went, and blind to a pane
//! changing size on a spring: measured on this build at the capture's window (2560x1406, `ui_scale`
//! 2.4), the left pane's rail was `Display::Flex` with `content` 877 against a `size` of 553 on **all
//! 29 frames** of a shut and on all 10 sampled frames of a restore. The numbers never move, because a
//! full-height pane changes *width* as it slides and the rail's rule is a *height* comparison — so the
//! engine's rule answers a question about the pane that was there before the chord.
//!
//! # What this file holds
//!
//! - **No rail is drawn on any viewport under a pane while the shell's split glides**, sampled every
//!   frame from the chord to the landing — both directions, shut and open.
//! - **When it settles the rail is drawn again only if the content really does not fit**, at two
//!   windows: the capture's, where the inspector's 877 px of rows in a 553 px box must show a bar, and
//!   a window tall enough to fit them, where it must not. That is gooey's rule restored, not a second
//!   copy of it, and it is what stops the fix from being "never draw a scrollbar".

use std::time::Duration;

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::{ComputedNode, UiScale};
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use bevy_dark_factory::chrome::{Frame, ShellSplit};
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::Mode;
use bevy_gooey::{MotionSettings, Role, SplitGlide};

use crate::fixtures;

/// One frame of the test clock, seconds. The spring settles in about a quarter of a second.
const FRAME: f32 = 1.0 / 60.0;
/// Frames that settle a glide on the clock above, with room to spare.
const SETTLE_FRAMES: usize = 90;
/// The capture's own window, physical px (1088 x 703 logical at the display's 2x).
const CAPTURE_SURFACE: (u32, u32) = (2560, 1406);
/// The capture's own interface scale.
const CAPTURE_UI_SCALE: f32 = 2.4;

/// **A rail is drawn**: its node takes room and its visibility does not hide it. Both halves are
/// read because either one alone is a way to hide a node — `Display::None` is what gooey's own
/// `update_lists` writes, and `Visibility::Hidden` is what a host writes — and the point of this file
/// is that *nothing draws it*, whichever lever was used.
fn drawn(node: Option<&Node>, visibility: Option<&Visibility>) -> bool {
    let shown_by_layout = node.is_some_and(|n| n.display != Display::None);
    let shown_by_visibility = !matches!(visibility, Some(Visibility::Hidden));
    shown_by_layout && shown_by_visibility
}

/// The editor on the Map door — which is the door whose left pane holds a list taller than its box at
/// the capture's window, so the measurement is of a bar that has somewhere to go — settled at `surface`.
fn editor(tag: &str, surface: (u32, u32)) -> App {
    let root = fixtures::Fixture::new(&format!("p517_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((
        Window { resolution: WindowResolution::new(surface.0, surface.1), ..default() },
        PrimaryWindow,
    ));
    harness::resize_surface(&mut app, surface.0, surface.1).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<UiScale>().0 = CAPTURE_UI_SCALE;
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(FRAME)));
    // Layout first, then the motion back on: the headless harness starts with motion off so every
    // other test reads a result in the frame it is made, and this file is about the motion itself.
    for _ in 0..30 {
        app.update();
    }
    app.insert_resource(MotionSettings::default());
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    app
}

/// **Cmd+B, pressed and let go**: a frame down, a frame up.
fn cmd_b(app: &mut App) {
    let keys = [bevy_dark_factory::keys::MOD_KEYS[0], KeyCode::KeyB];
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for key in keys {
            let logical_key = if key == KeyCode::KeyB {
                Key::Character("b".into())
            } else {
                Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified)
            };
            app.world_mut().write_message(KeyboardInput { key_code: key, logical_key, state, text: None, repeat: false, window: Entity::PLACEHOLDER });
        }
        app.update();
    }
}

/// One rail under the left dock, and the viewport it is a bar for.
struct Under {
    /// The rail's own drawn state.
    drawn: bool,
    /// How tall the viewport is and how tall its contents are, logical px: gooey's own comparison, and
    /// the only honest test of "does the bar have somewhere to go".
    size: f32,
    content: f32,
}

/// **Every scroll rail under the frame's left dock**, with the geometry of the viewport it is for.
///
/// **Rails, not viewports.** gooey spawns a rail three ways — `scroll_viewport` puts
/// [`bevy_gooey::ScrollRail`] on the viewport, and `list()` and `scroll_region()` spawn a rail of their
/// own a host cannot switch off. Measured on this build: the Map door's inspector is the second kind,
/// so a helper keyed on the marker found no bar to watch and this file failed on its own precondition
/// rather than on the defect. Every one of the three is a `Role::ListRail` whose `Scrollbar.target` is
/// its viewport, which is the one thing they share, and what the fix itself walks.
///
/// Walked up from the viewport to the frame's left dock, not by a list of panel names: a panel added
/// tomorrow is covered, and a rail somewhere else in the window (the PIECES list, the terminal) is not
/// counted because nothing in this file is about it.
fn under_the_left_dock(app: &mut App) -> Vec<Under> {
    let world = app.world_mut();
    let Some(left) = world.get_resource::<Frame>().map(|frame| frame.left) else { return Vec::new() };
    let rails: Vec<(Entity, Entity)> = world
        .query::<(&Role, &bevy::ui_widgets::Scrollbar, Entity)>()
        .iter(world)
        .filter(|(role, _, _)| **role == Role::ListRail)
        .map(|(_, bar, rail)| (rail, bar.target))
        .collect();
    let mut out = Vec::new();
    for (rail, view) in rails {
        let mut at = world.get::<ChildOf>(view).map(|c| c.parent());
        let mut found = false;
        while let Some(up) = at {
            if up == left {
                found = true;
                break;
            }
            at = world.get::<ChildOf>(up).map(|c| c.parent());
        }
        if !found {
            continue;
        }
        let (size, content) = world
            .get::<ComputedNode>(view)
            .map(|node| (node.size().y * node.inverse_scale_factor(), node.content_size().y * node.inverse_scale_factor()))
            .unwrap_or((0.0, 0.0));
        let drawn = drawn(world.get::<Node>(rail), world.get::<Visibility>(rail));
        out.push(Under { drawn, size, content });
    }
    out
}

/// The left pane's width, logical px, and whether a pane of the shell's split is on its way anywhere.
///
/// A pane is "on its way" when the split carries gooey's [`SplitGlide`]; the width is read to prove the
/// sampling really did cover the slide rather than a settled frame either side of it.
fn pane(app: &mut App) -> (f32, bool) {
    let world = app.world_mut();
    let Some(split) = world.try_query_filtered::<Entity, With<ShellSplit>>().and_then(|mut q| q.single(world).ok()) else {
        return (-1.0, false);
    };
    let sliding = world.get::<SplitGlide>(split).is_some();
    let wide = world
        .get::<Children>(split)
        .and_then(|kids| kids.iter().find(|kid| world.get::<Role>(*kid) == Some(&Role::SplitPane)))
        .and_then(|first| world.get::<ComputedNode>(first))
        .map(|node| node.size().x * node.inverse_scale_factor())
        .unwrap_or(-1.0);
    (wide, sliding)
}

/// **A bar is drawn under the left dock while the pane holding it is sliding** — the defect.
fn bars_drawn_while_sliding(app: &mut App, direction: &str, into: &mut Vec<String>) {
    let mut sampled = 0usize;
    let mut moved = false;
    let mut last = pane(app).0;
    for frame in 0..SETTLE_FRAMES {
        let (wide, sliding) = pane(app);
        if sliding {
            sampled += 1;
            if (wide - last).abs() > 0.5 {
                moved = true;
            }
            last = wide;
            for under in under_the_left_dock(app) {
                if under.drawn {
                    into.push(format!(
                        "{direction} f{frame:02}: a bar over a pane {wide:.0} px wide, its viewport {:.0} tall for {:.0} of rows",
                        under.size, under.content
                    ));
                }
            }
        }
        app.update();
    }
    assert!(sampled >= 8, "{direction}: the slide was sampled on more than a handful of frames ({sampled}), or this test is about a pane that does not move");
    assert!(moved, "{direction}: the pane's width changed across the sampled frames, so they are the slide and not a settle");
}

#[test]
fn no_scrollbar_is_drawn_while_the_left_pane_slides() {
    for (tag, surface, overflows) in [
        // The capture's window: the inspector's rows do not fit, so a settled pane must show a bar.
        ("capture", CAPTURE_SURFACE, true),
        // A window tall enough to hold them: a settled pane must show no bar at all.
        ("tall", (2560, 2400), false),
    ] {
        let mut app = editor(tag, surface);
        let at_rest = under_the_left_dock(&mut app);
        let tall = at_rest.iter().map(|u| u.content).fold(0.0f32, f32::max);
        let fits = |u: &Under| u.content > u.size + 1.0;
        let overflowing = at_rest.iter().any(fits);
        assert_eq!(
            overflowing, overflows,
            "{tag}: this window was chosen because the left dock's contents {} fit; measured content {:.0} px against the tallest viewport",
            if overflows { "do not" } else { "" },
            tall
        );
        assert!(!at_rest.is_empty(), "{tag}: the left dock has scroll rails, or there is no bar to hide");
        // **The capture's window is the one with a bar to take away**, and the other is the control
        // case: same slide, same rails, and nothing that was ever due to be drawn. A slide that drew a
        // bar in either would be the defect.
        assert_eq!(
            at_rest.iter().any(|u| u.drawn),
            overflows,
            "{tag}: a bar is on screen at rest exactly when the rows do not fit, or this window is not the case it is here for"
        );

        // **The defect.** Shut, then open again: every frame of both slides is sampled.
        let mut offences = Vec::new();
        cmd_b(&mut app);
        bars_drawn_while_sliding(&mut app, "shutting", &mut offences);
        for _ in 0..SETTLE_FRAMES {
            app.update();
        }
        cmd_b(&mut app);
        bars_drawn_while_sliding(&mut app, "opening", &mut offences);
        for _ in 0..SETTLE_FRAMES {
            app.update();
        }
        assert!(offences.is_empty(), "{tag}: {count} frame(s) drew a scrollbar over a pane that was mid-air:\n  {}", offences.join("\n  "), count = offences.len());
    }
}

#[test]
fn a_settled_pane_draws_a_scrollbar_exactly_when_its_contents_do_not_fit() {
    for (tag, surface, overflows) in [("capture", CAPTURE_SURFACE, true), ("tall", (2560, 2400), false)] {
        let mut app = editor(tag, surface);
        // **Open, at rest**: what gooey's own rule says, which is the state the fix must return to.
        for under in under_the_left_dock(&mut app) {
            if under.content > under.size + 1.0 {
                assert!(under.drawn, "{tag}: {:.0} px of rows in a {:.0} px viewport and no bar to say so", under.content, under.size);
            } else {
                assert!(!under.drawn, "{tag}: a bar over a viewport whose {:.0} px of rows fit in {:.0}", under.content, under.size);
            }
        }
        assert!(under_the_left_dock(&mut app).iter().any(|u| u.content > u.size + 1.0) == overflows, "{tag}: the window was chosen for whether its contents fit");

        // **Shut, and settled**: the pane is gone, so nothing under it may claim a bar is needed.
        cmd_b(&mut app);
        for _ in 0..(2 * SETTLE_FRAMES) {
            app.update();
        }
        let (wide, sliding) = pane(&mut app);
        assert!(!sliding && wide < 0.5, "{tag}: the pane has finished sliding shut ({wide:.1} px, gliding {sliding})");
        for under in under_the_left_dock(&mut app) {
            assert!(!under.drawn, "{tag}: a bar is drawn under a pane that is shut");
        }

        // **Open again, settled**: the same answer the open pane gave before the chord, on gooey's own
        // rule — a bar for rows that do not fit, and nothing for rows that do.
        cmd_b(&mut app);
        for _ in 0..(2 * SETTLE_FRAMES) {
            app.update();
        }
        let (wide, sliding) = pane(&mut app);
        assert!(!sliding && wide > 100.0, "{tag}: the pane has finished sliding open ({wide:.1} px, gliding {sliding})");
        let back = under_the_left_dock(&mut app);
        assert!(!back.is_empty(), "{tag}: the rails under the left dock are still there after the restore, so one can be drawn again");
        let mut checked = 0usize;
        for under in &back {
            if under.size <= 0.0 {
                // A viewport whose panel is not the one on show lays out to nothing.
                continue;
            }
            checked += 1;
            if under.content > under.size + 1.0 {
                assert!(under.drawn, "{tag}: the bar is back where the rows do not fit ({:.0} in {:.0})", under.content, under.size);
            } else {
                assert!(!under.drawn, "{tag}: no bar where the rows fit ({:.0} in {:.0})", under.content, under.size);
            }
        }
        assert!(checked > 0, "{tag}: at least one viewport under the left dock is laid out after the restore");
    }
}
