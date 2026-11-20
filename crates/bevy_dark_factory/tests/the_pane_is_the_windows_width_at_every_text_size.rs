//! **The frame never lays out wider than the window, at any text size** (#497).
//!
//! The owner's capture measured the terminal pane **1569 px wide in a 1536 px window** at text scale
//! 3.0, with its Invoke button drawn past the right edge so a press at the button's centre never
//! reached it. Two earlier attempts to fix it at the *pane* — a `Percent(100.0)` width and a
//! `max_width: Percent(100.0)` on its `Sizing` — changed nothing at 3.0, and this file's measurement
//! says why.
//!
//! # What the measurement found
//!
//! `node.width` on the pane was already `Percent(100.0)` on the frame it was first laid out, and
//! `HostGeometry` was already on it, so neither a missing width nor a dropped one was the cause. Ten
//! frames at 1280 x 720, text scale written into the fixture's `editor.yaml`:
//!
//! | text scale | the pane | the frame root | widest terminal row |
//! |---|---|---|---|
//! | 1.0 | 1280 | 1280 | 1252 |
//! | 1.6 | 1280 | 1280 | 1252 |
//! | 3.0 | **1569** | **1569** | **1541** |
//!
//! **The frame root was as wide as the pane, so no pane was what stretched it.** Hiding the terminal
//! pane left the root at 1569. The cause is a flex item's *automatic least width*: the terminal's body
//! holds one unwrapped `Text` per line of output (`TextLayout::no_wrap`, so a shell's 200-column
//! `cargo` line is one `Text`), and at 3.0 that line's own width is 1541 px. A least width travels up
//! the chain, so the body, the pane, the bars and the root each took the line's width while their own
//! `width: Percent(100.0)` said "the parent's" — a `Percent` width says how wide a node is and does not
//! stop its content claiming more.
//!
//! # The fix
//!
//! The frame root's `Sizing` states `min_width: Extent::Px(0.0)`, which is gooey's own spelling of
//! "this box's content decides nothing about how wide it is". Measured after: root, pane and body are
//! 1280 at 3.0 and unchanged at 1.0 and 1.6.

use bevy::prelude::*;
use bevy::ui::ComputedNode;
use bevy::window::{PrimaryWindow, Window};

use bevy_dark_factory::chrome::Frame;
use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_pane::{TerminalBody, TerminalPaneRoot};

use crate::fixtures;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// **The window the owner's capture was taken in**, in logical px.
const WINDOW: (u32, u32) = (1280, 720);

/// Frames the pane is given to reach its resting layout — the first restyle, then the fit, then the
/// re-wrap. Ten is what the measurement table was taken over, and the number is unchanged after.
const SETTLE_FRAMES: usize = 10;

/// The editor on a project whose settings choose the text size `scale`, with a window.
fn editor(tag: &str, scale: f32) -> App {
    let tag = format!("pane_width_{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).sized_descriptor("fence", "alpha", 2.064, 0.12).build("m");
    let file = root.join("assets/blueprint/editor.yaml");
    std::fs::write(&file, format!("{{version: 1, text_scale: {scale}}}")).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.world_mut().spawn((
        Window { resolution: bevy::window::WindowResolution::new(WINDOW.0, WINDOW.1), ..default() },
        PrimaryWindow,
    ));
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
    app
}

/// The laid-out width of the one entity carrying `M`, in the UI's own pixels.
fn width_of<M: Component>(app: &mut App, what: &str) -> f32 {
    let world = app.world_mut();
    let found: Vec<f32> = world.query_filtered::<&ComputedNode, With<M>>().iter(world).map(|n| n.size().x).collect();
    assert_eq!(found.len(), 1, "one {what} is on screen, got {}", found.len());
    found[0]
}

/// The frame root's laid-out width.
fn frame_width(app: &mut App) -> f32 {
    let root = app.world().get_resource::<Frame>().map(|frame| frame.root).expect("the editor's frame");
    app.world().get::<ComputedNode>(root).map_or(f32::NAN, |node| node.size().x)
}

/// **The frame, the pane and the body are the window's width, at every text size the panel offers.**
///
/// The three scales are the ones #497's table measured: the two that were already right, and the one
/// that was 1569. A fix that righted 3.0 by breaking 1.6 is not a fix, so all three are asserted.
#[test]
fn the_frame_is_the_windows_width_at_every_text_size() {
    for scale in [1.0f32, 1.6, 3.0] {
        let mut app = editor(&format!("s{}", (scale * 10.0) as u32), scale);
        let root = frame_width(&mut app);
        let pane = width_of::<TerminalPaneRoot>(&mut app, "terminal pane");
        let body = width_of::<TerminalBody>(&mut app, "terminal body");
        let window = WINDOW.0 as f32;
        assert!(
            (root - window).abs() < 0.5,
            "at text scale {scale} the frame root is {root} px in a {window} px window: a descendant's least \
             width reached the root, and the whole window scrolls sideways"
        );
        assert!(
            (pane - window).abs() < 0.5,
            "at text scale {scale} the pane is {pane} px in a {window} px window, so its right-hand controls are \
             past the edge and a press on them never lands"
        );
        assert!(
            (body - window).abs() < 0.5,
            "at text scale {scale} the body is {body} px in a {window} px window, so the rows are drawn outside it"
        );
    }
}

/// **The pane's own width is still stated as its parent's**, which is the half the frame's clamp does not
/// say: a pane sized by its content would be as wide as its longest row the moment the frame stopped
/// stretching it.
#[test]
fn the_pane_is_as_wide_as_its_parent_never_as_wide_as_its_content() {
    let mut app = editor("pane_states_its_width", 3.0);
    let world = app.world_mut();
    let panes: Vec<Val> = world.query_filtered::<&Node, With<TerminalPaneRoot>>().iter(world).map(|node| node.width).collect();
    assert_eq!(panes.len(), 1, "one terminal pane is on screen");
    assert_eq!(
        panes[0],
        Val::Percent(100.0),
        "the pane states its width as its parent's, never as its content's — at text scale 3.0 its longest \
         unwrapped row is 1541 px in a 1280 px window"
    );
}
