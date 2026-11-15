//! **With up to six tabs open the terminal's bar keeps every chip whole** (#448: with three tabs the `Start agent` chip read `Star|agent`, cut by the status LED).
//!
//! At 1024 x 640 and 1280 x 720 (and every width between 600 and 1500 px), with an agent tab and two or five shells open, every chip of the bar (the tabs, `Start agent`,
//! `+`, `x`) is drawn whole: its text fits inside its own box, no chip overlaps another or the status LED, and everything stands inside the bar. It is the tab strip that
//! gives way: a tab's name is cut with an ellipsis, never a chip's. The rects are read from the world, as drawn.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;

use bevy_cauldron::terminal::Terminal;
use bevy_cauldron::terminal_agent::AgentSetup;
use bevy_cauldron::terminal_pane::{
    TerminalAgentChip, TerminalAgentLed, TerminalBar, TerminalCdChip, TerminalCloseTab, TerminalInvokeButton, TerminalNewTab, TerminalTabChip,
};

use crate::fixtures;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor(tag: &str, size: (u32, u32), shell: &str, agent_command: &str) -> App {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/grimoire/editor.yaml");
    let settings = r#"{version: 1, terminal_agent: Custom, terminal_agent_command: "COMMAND", terminal_shell: "SHELL"}"#.replace("SHELL", shell).replace("COMMAND", agent_command);
    std::fs::write(&file, settings).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let files = std::env::temp_dir().join(format!("{tag}_agents"));
    let server = std::env::temp_dir().join(format!("{tag}_server"));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15799), server, files });
    bevy_cauldron::harness::resize_surface(&mut app, size.0, size.1).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    updates(&mut app, 12);
    app
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

fn until(app: &mut App, why: &str, test: impl Fn(&mut App) -> bool) {
    for _ in 0..200 {
        updates(app, 2);
        if test(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("{why}");
}

fn click(app: &mut App, at: Vec2) {
    let handle = app.world().get_resource::<bevy_cauldron::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    for action in [PointerAction::Move { delta: Vec2::ZERO }, PointerAction::Press(PointerButton::Primary), PointerAction::Release(PointerButton::Primary)] {
        let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle: handle.clone(), scale_factor: 1.0 }), position: at };
        app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
        app.update();
    }
}

fn centre_of<M: Component>(app: &mut App) -> Option<Vec2> {
    let world = app.world_mut();
    world.query_filtered::<&UiGlobalTransform, With<M>>().iter(world).next().map(|at| at.translation)
}

/// A rect in physical px: `(min, max)`.
type Rect = (Vec2, Vec2);

fn overlap(a: Rect, b: Rect) -> bool {
    // A pixel of slack for rounding: touching edges are not an overlap.
    a.0.x < b.1.x - 1.0 && b.0.x < a.1.x - 1.0 && a.0.y < b.1.y - 1.0 && b.0.y < a.1.y - 1.0
}

/// One drawn item of the bar: its name, its box and, for a chip, the width its text needs against the width it was given.
struct Item {
    name: String,
    word: Option<String>,
    rect: Rect,
    text: Option<(f32, f32)>,
}

fn items(app: &mut App) -> (Rect, Vec<Item>, Option<Rect>) {
    let world = app.world_mut();
    let bar = world.query_filtered::<Entity, With<TerminalBar>>().iter(world).next().expect("the bar");
    let mut chips: Vec<(String, Entity)> = Vec::new();
    for (entity, tab) in world.query::<(Entity, &TerminalTabChip)>().iter(world) {
        chips.push((format!("tab {}", tab.0), entity));
    }
    for (name, entities) in [
        ("Start agent", world.query_filtered::<Entity, With<TerminalAgentChip>>().iter(world).collect::<Vec<_>>()),
        ("+", world.query_filtered::<Entity, With<TerminalNewTab>>().iter(world).collect()),
        ("x", world.query_filtered::<Entity, With<TerminalCloseTab>>().iter(world).collect()),
        ("Go to", world.query_filtered::<Entity, With<TerminalCdChip>>().iter(world).collect()),
    ] {
        chips.extend(entities.into_iter().map(|e| (name.to_owned(), e)));
    }
    let led = world.query_filtered::<Entity, With<TerminalAgentLed>>().iter(world).next();
    let rect = |world: &World, entity: Entity| -> Option<Rect> {
        let node = world.get::<ComputedNode>(entity)?;
        let at = world.get::<UiGlobalTransform>(entity)?.translation;
        Some((at - node.size() / 2.0, at + node.size() / 2.0))
    };
    let bar_rect = rect(world, bar).expect("the bar is laid out");
    let mut out = Vec::new();
    for (name, chip) in chips {
        let Some(r) = rect(world, chip) else { continue };
        // The chip's word and the room the chip leaves it: the text's laid-out width against the chip's width less its padding and edge.
        let text = world.get::<Children>(chip).and_then(|kids| {
            kids.iter().find_map(|kid| {
                let layout = world.get::<bevy::text::TextLayoutInfo>(kid)?;
                let node = world.get::<ComputedNode>(kid)?;
                Some((layout.size.x, node.size().x, world.get::<bevy::ui::widget::Text>(kid)?.0.clone()))
            })
        });
        let inner = world.get::<ComputedNode>(chip).map(|n| (n.size().x - n.padding().min_inset.x - n.padding().max_inset.x - n.border().min_inset.x - n.border().max_inset.x));
        out.push(Item { name, rect: r, word: text.as_ref().map(|(_, _, word)| word.clone()), text: text.map(|(needs, _, _)| (needs, inner.unwrap_or(0.0))) });
    }
    (bar_rect, out, led.and_then(|led| rect(world, led)))
}

/// The agent tab, then shells: `tabs` in all.
fn six_tabs(app: &mut App, tabs: usize) {
    until(app, "the Start agent button", |app| centre_of::<TerminalInvokeButton>(app).is_some());
    let at = centre_of::<TerminalInvokeButton>(app).expect("the button");
    click(app, at);
    until(app, "the agent's tab", |app| app.world().resource::<Terminal>().len() == 1);
    // Five more tabs through the terminal's own call (what the `+` chip does), so the bar is what is under test and not a click's aim.
    for _ in 2..=tabs {
        app.world_mut().resource_mut::<Terminal>().spawn(80, 24, std::path::Path::new("/tmp"));
        updates(app, 4);
    }
    assert_eq!(app.world().resource::<Terminal>().len(), tabs, "an agent tab and shells");
    updates(app, 10);
}

const SHORT_AGENT: &str = "printf 'AGENT-%s\\n' UP";

/// What is wrong with the bar as drawn now, one line each; empty when every chip is whole.
fn faults(app: &mut App) -> Vec<String> {
    let (bar, chips, led) = items(app);
    let led = led.expect("the agent's LED is in the bar");
    let mut out = Vec::new();
    for item in &chips {
        let Item { name, rect, text, .. } = item;
        if rect.0.x < bar.0.x - 1.0 || rect.1.x > bar.1.x + 1.0 {
            out.push(format!("`{name}` {rect:?} is not inside the bar {bar:?}"));
        }
        if let Some((needs, room)) = text
            && *needs > room + 1.0
        {
            out.push(format!("`{name}`'s text needs {needs} px and its chip leaves {room}: the word is cut"));
        }
        if overlap(*rect, led) {
            out.push(format!("`{name}` {rect:?} overlaps the status LED {led:?}"));
        }
    }
    for (a, b) in chips.iter().enumerate().flat_map(|(i, a)| chips.iter().skip(i + 1).map(move |b| (a, b))) {
        if overlap(a.rect, b.rect) {
            out.push(format!("`{}` {:?} overlaps `{}` {:?}", a.name, a.rect, b.name, b.rect));
        }
    }
    // `+` and `x` are always there; `Start agent` goes first when the bar is narrow, by the bar's own order (BAR_GIVES_UP).
    for name in ["+", "x"] {
        if !chips.iter().any(|c| c.name == name) {
            out.push(format!("the `{name}` chip is gone: {:?}", chips.iter().map(|c| &c.name).collect::<Vec<_>>()));
        }
    }
    out
}

/// **Every window width from narrow to wide**, the two sizes of the issue among them: the plan of the bar is an estimate, so a width where the estimate is a pixel short is
/// where a chip is cut, and only a sweep finds it.
fn sweep(tabs: usize, agent_command: &str) {
    let mut app = editor(&format!("chips_{tabs}"), (1024, 640), "/bin/sh", agent_command);
    six_tabs(&mut app, tabs);
    // Up in steps and then down in steps of 3 px: the plan is made from a measured room, and a window dragged narrower by less than the measure's step is where it is stale.
    let mut widths: Vec<u32> = (600..=1500).step_by(20).collect();
    widths.extend((600..=1500).rev().step_by(3));
    widths.extend([1280, 1024]);
    let mut bad = Vec::new();
    // The drawn tab is the agent's (its LED is the control that brings the self test back) and then a shell's (the LED stands alone).
    for drawn in [0, tabs - 1] {
        app.world_mut().resource_mut::<Terminal>().select(drawn);
        for &width in &widths {
            let height = if width >= 1280 { 720 } else { 640 };
            bevy_cauldron::harness::resize_surface(&mut app, width, height).unwrap_or_else(|e| panic!("{e}"));
            updates(&mut app, 8);
            let mut faults = faults(&mut app);
            // At the two sizes of the issue the bar has room for every chip, so `Start agent` stays: it is the tab strip that gives way.
            if [1024, 1280].contains(&width) && !items(&mut app).1.iter().any(|c| c.name == "Start agent") {
                faults.push("the `Start agent` chip is gone".to_owned());
            }
            if !faults.is_empty() {
                bad.push(format!("tab {drawn} drawn, {width}x{height}: {}", faults.join("; ")));
            }
        }
    }
    assert!(bad.is_empty(), "{tabs} tabs: a chip is cut at {} of the widths:\n{}", bad.len(), bad.join("\n"));
}

#[test]
fn with_three_tabs_every_chip_is_whole_at_every_width() {
    sweep(3, SHORT_AGENT);
}

#[test]
fn with_six_tabs_every_chip_is_whole_at_every_width() {
    sweep(6, SHORT_AGENT);
}

const LONG_AGENT: &str = "/home/someone/.local/share/agents/bin/a-long-agent-program --flag";

/// **A tab named by a long path is cut with an ellipsis**: a custom agent's tab is named by the first word of its command, and that can be a whole path.
#[test]
fn a_long_tab_name_is_cut_with_an_ellipsis_and_no_chip_is() {
    sweep(3, LONG_AGENT);
    let mut app = editor("chips_long", (1024, 640), "/bin/sh", LONG_AGENT);
    six_tabs(&mut app, 3);
    let (_, chips, _) = items(&mut app);
    let word = chips.iter().find(|c| c.name == "tab 0").and_then(|c| c.word.clone()).expect("the agent's tab");
    assert!(word.ends_with('…') && word.chars().count() <= 16, "a long name is cut with an ellipsis to at most 16 characters: {word:?}");
}
