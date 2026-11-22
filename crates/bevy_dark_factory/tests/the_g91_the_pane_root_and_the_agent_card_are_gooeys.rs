//! **The terminal's pane root and its self-test card are gooey's** (#260, UI-99, the last two raw `Node`s).
//!
//! The pane root is a `sized` column (`Sizing`, `RowLayout`, `BorderPx`) that follows its `display` (`ShownFollow`, closed when the person left the terminal collapsed) and whose geometry is the
//! host's once gooey has styled it (`HostGeometry`): the grip, the window's fit and the maximise write `height`, `position_type` and the insets every frame, and gooey must not write them back.
//! The card is a `placed` column whose width is its content's. This file asks a running editor what they are and what they measure; the card's numbers are the ones the inline test of
//! `agent_card_node()` pinned (Owner exception, 2026-11-14: that test went with the function; this one reads the same numbers from the world).

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy_gooey::{HostGeometry, ShownFollow};

use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_agent::AgentSetup;
use bevy_dark_factory::terminal_pane::{TerminalAgentCard, TerminalBody, TerminalInvokeButton, TerminalPaneRoot, TERMINAL_MIN_H};

use crate::fixtures;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn px(v: Val) -> f32 {
    match v {
        Val::Px(p) => p,
        other => panic!("expected pixels, got {other:?}"),
    }
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// The editor on a project (settings from `settings`), with the terminal collapsed or raised.
fn editor(tag: &str, settings: &str, collapsed: bool) -> App {
    let tag = format!("g91_{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/blueprint/editor.yaml");
    std::fs::write(&file, settings).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let files = std::env::temp_dir().join(format!("{tag}_agents"));
    let server = std::env::temp_dir().join(format!("{tag}_server"));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15799), server, files });
    app.world_mut().resource_mut::<Terminal>().set_collapsed(collapsed);
    updates(&mut app, 12);
    app
}

fn pane(app: &mut App) -> Entity {
    let world = app.world_mut();
    world.query_filtered::<Entity, With<TerminalPaneRoot>>().iter(world).next().expect("the terminal's pane")
}

/// **A raised pane is a window-wide column, its opening height tall, at least `TERMINAL_MIN_H`, never shrinking, with a 1 px top edge; the host owns its geometry after the first frame.**
#[test]
fn the_raised_pane_root_is_a_sized_column_the_host_owns() {
    let mut app = editor("raised", "{version: 1, terminal_agent: Off}", false);
    let root = pane(&mut app);
    let world = app.world();
    let n = world.get::<Node>(root).expect("a Node");
    assert_eq!(n.flex_direction, FlexDirection::Column);
    assert_eq!((n.width, n.min_height, n.flex_shrink), (Val::Percent(100.0), Val::Px(TERMINAL_MIN_H), 0.0));
    assert!(matches!(n.height, Val::Px(h) if h >= TERMINAL_MIN_H), "an opening height: {:?}", n.height);
    assert_eq!((n.border.top, n.border.left, n.display), (Val::Px(1.0), Val::ZERO, Display::Flex));
    assert!(world.get::<ShownFollow>(root).is_some(), "gooey follows the display the host writes");
    assert!(world.get::<HostGeometry>(root).is_some(), "and the geometry is the host's once gooey styled it");
    assert_ne!(world.get::<Visibility>(root), Some(&Visibility::Hidden));
    assert_eq!(world.get::<Pickable>(root), Some(&Pickable::default()), "the pane takes the pointer");
}

/// **A collapsed pane starts closed: no display, hidden from focus.**
#[test]
fn a_collapsed_pane_starts_closed() {
    let mut app = editor("collapsed", "{version: 1, terminal_agent: Off}", true);
    let root = pane(&mut app);
    let world = app.world();
    assert_eq!(world.get::<Node>(root).map(|n| n.display), Some(Display::None));
    assert_eq!(world.get::<Visibility>(root), Some(&Visibility::Hidden), "closed to focus too");
    assert!(world.get::<ShownFollow>(root).is_some());
}

/// **What the host writes is not written back**: a dragged height, the maximise's `Auto` height and absolute position, and a changed skin all stay as the host left them.
#[test]
fn what_the_host_writes_to_the_pane_stays_when_gooey_restyles() {
    let mut app = editor("host", "{version: 1, terminal_agent: Off}", false);
    let root = pane(&mut app);

    // A height the grip drags the pane to (it is the person's: `Terminal::set_height`, so the window's fit leaves it), then a skin change that restyles every node.
    app.world_mut().resource_mut::<Terminal>().set_height(250.0);
    app.world_mut().get_mut::<Node>(root).expect("a Node").height = Val::Px(250.0);
    let skin = app.world().resource::<bevy_gooey::GooeySkin>().clone();
    app.world_mut().insert_resource(skin);
    updates(&mut app, 4);
    assert_eq!(app.world().get::<Node>(root).map(|n| n.height), Some(Val::Px(250.0)), "a skin change does not reset the height the host wrote");

    // The maximise: absolute, insets, `Auto` height, over the frame; still so after more frames and a restyle.
    app.world_mut().resource_mut::<Terminal>().toggle_maximised();
    updates(&mut app, 4);
    let skin = app.world().resource::<bevy_gooey::GooeySkin>().clone();
    app.world_mut().insert_resource(skin);
    updates(&mut app, 4);
    let n = app.world().get::<Node>(root).expect("a Node");
    assert_eq!((n.position_type, n.height, n.left, n.right, n.bottom), (PositionType::Absolute, Val::Auto, Val::Px(0.0), Val::Px(0.0), Val::Px(0.0)), "maximised: over the frame, auto height");

    // And given back it is in the column again at its height.
    app.world_mut().resource_mut::<Terminal>().toggle_maximised();
    updates(&mut app, 4);
    let n = app.world().get::<Node>(root).expect("a Node");
    assert_eq!(n.position_type, PositionType::Relative);
    assert!(matches!(n.height, Val::Px(h) if h >= TERMINAL_MIN_H), "back to a pixel height: {:?}", n.height);
}

fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app.world().get_resource::<bevy_dark_factory::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }), position: at };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

/// **The self-test card is a placed column: its numbers are the ones `agent_card_node()` pinned.**
///
/// Top `BODY_INSET.y` and a margin in from the right, as wide as its content (no width of its own, at least 320, at most the terminal's body), its rows 3 px apart, 3 px in and a 1 px
/// edge; the pointer goes through it. The measured width is inside the body.
#[test]
fn the_self_test_card_is_a_placed_column_as_wide_as_its_content() {
    let mut app = editor("card", r#"{version: 1, terminal_agent: Custom, terminal_agent_command: "printf 'AGENT-%s\\n' UP"}"#, false);
    for _ in 0..200 {
        updates(&mut app, 2);
        let world = app.world_mut();
        if world.query_filtered::<Entity, With<TerminalInvokeButton>>().iter(world).next().is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let at = {
        let world = app.world_mut();
        world.query_filtered::<&UiGlobalTransform, With<TerminalInvokeButton>>().iter(world).next().expect("the invitation's button").translation
    };
    for action in [PointerAction::Move { delta: Vec2::ZERO }, PointerAction::Press(PointerButton::Primary), PointerAction::Release(PointerButton::Primary)] {
        point(&mut app, at, action);
    }
    let mut card = None;
    for _ in 0..200 {
        updates(&mut app, 2);
        let world = app.world_mut();
        card = world.query_filtered::<Entity, With<TerminalAgentCard>>().iter(world).next();
        if card.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let card = card.expect("the card is drawn once the agent starts");
    updates(&mut app, 4);
    let world = app.world_mut();
    let n = world.get::<Node>(card).expect("a Node").clone();
    assert_eq!(n.position_type, PositionType::Absolute, "it floats over the terminal");
    assert_eq!(n.top, Val::Px(bevy_dark_factory::terminal_view::body_inset().y));
    assert_eq!(n.right, Val::Px(bevy_dark_factory::chrome::space(bevy_dark_factory::chrome::STEP_PAD)));
    assert_eq!(n.width, Val::Auto, "its width is its content's, not a constant");
    assert_eq!(n.min_width, Val::Px(320.0), "and never less than the first row needs");
    assert_eq!(n.max_width, Val::Percent(100.0), "and never more than the terminal's body");
    assert_eq!(n.flex_direction, FlexDirection::Column);
    for (what, got) in [("row gap", px(n.row_gap)), ("padding", px(n.padding.left)), ("border", px(n.border.left))] {
        let want = if what == "border" { 1.0 } else { 3.0 };
        assert!((got - want).abs() < 0.05, "the card's {what} is {got}, not {want}");
    }
    assert_eq!(world.get::<Pickable>(card), Some(&Pickable::IGNORE), "the pointer goes through the card to the shell");
    let body = world.query_filtered::<Entity, With<TerminalBody>>().iter(world).next().expect("the body");
    let (card_w, body_w) = {
        let c = world.get::<ComputedNode>(card).expect("laid out");
        let b = world.get::<ComputedNode>(body).expect("laid out");
        (c.size().x * c.inverse_scale_factor(), b.size().x * b.inverse_scale_factor())
    };
    assert!(card_w >= 320.0 - 0.5 && card_w <= body_w + 0.5, "the card is {card_w} wide inside a {body_w} body");
}
