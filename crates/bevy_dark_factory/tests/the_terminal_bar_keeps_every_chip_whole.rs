//! **With up to six tabs open the terminal's bar keeps every chip whole** (#448: with three tabs the `Start agent` chip read `Star|agent`, cut by the status LED).
//!
//! At 1024 x 640 and 1280 x 720 (and every width between 600 and 1500 px), with an agent tab and two or five shells open, every chip of the bar (the tabs, `Start agent`,
//! `+`, `x`) is drawn whole: its text fits inside its own box, no chip overlaps another or the status LED, and everything stands inside the bar. It is the tab strip that
//! gives way: a tab's name is cut with an ellipsis, never a chip's. The rects are read from the world, as drawn.
//!
//! **The width list is cut into [`CHIP_SWEEPS`] tests, and every width is still measured** (#672).
//! One test held 349 widths × 2 drawn tabs × 8 frames — 5,584 `App::update` calls in a single thread,
//! 29.5 s that nextest could not schedule beside anything else. Splitting is a scheduling change and
//! not a coverage change: `sweep_widths` is the one list, `chunk_bounds` cuts it into contiguous runs,
//! and [`every_width_is_measured_once`] asserts that the runs concatenate back to it — no width
//! dropped, none measured twice, the two sizes of the issue still among them. Each part keeps the
//! same `faults()` assertion and the same `updates(8)` per width.
//!
//! **This is the second fix the issue names, and it is not the first one.** The same file's `sweep`
//! looks like a search for the width at which a chip starts to wrap, which would make a binary search
//! the fix. It is not a search: the predicate is *every* width, and measured on this file's three
//! chip sets it never goes false. Scanning all 901 px from 600 to 1500 and back, the bar faults at
//! **no width at all**, and the only thing that changes across the range is whether the `Go to` chip
//! is drawn. There is no threshold to bisect, so the cost is the enumeration itself, and cutting it
//! across processes is the only fix that keeps every width.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;

use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_agent::AgentSetup;
use bevy_dark_factory::terminal_pane::{
    TerminalAgentChip, TerminalAgentLed, TerminalBar, TerminalCdChip, TerminalCloseTab, TerminalInvokeButton, TerminalNewTab, TerminalTabChip,
};

use crate::fixtures;

/// **`SHELL` is process-wide**, so the tests here take turns; the guard lives in the `App`.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct PosixShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

fn editor(tag: &str, size: (u32, u32), shell: &str, agent_command: &str) -> App {
    let tag = format!("{tag}_{}", std::process::id());
    let root = fixtures::Fixture::new(&tag).descriptor("floor", "alpha").build("m");
    let file = root.join("assets/blueprint/editor.yaml");
    let settings = r#"{version: 1, terminal_agent: Custom, terminal_agent_command: "COMMAND", terminal_shell: "SHELL"}"#.replace("SHELL", shell).replace("COMMAND", agent_command);
    std::fs::write(&file, settings).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
    let files = std::env::temp_dir().join(format!("{tag}_agents"));
    let server = std::env::temp_dir().join(format!("{tag}_server"));
    std::fs::write(&server, "").unwrap_or_else(|e| panic!("{}: {e}", server.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    unsafe { std::env::set_var("SHELL", "/bin/sh") };
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(PosixShell(guard));
    app.insert_resource(AgentSetup { port: Some(15799), server, files });
    bevy_dark_factory::harness::resize_surface(&mut app, size.0, size.1).unwrap_or_else(|e| panic!("{e}"));
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
    let handle = app.world().get_resource::<bevy_dark_factory::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
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
        let inner = world.get::<ComputedNode>(chip).map(|n| n.size().x - n.padding().min_inset.x - n.padding().max_inset.x - n.border().min_inset.x - n.border().max_inset.x);
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

/// **The widths the bar is measured at**, in the order the original sweep visited them: up in steps
/// of 20, then down in steps of 3, then the two sizes of the issue.
///
/// **One list, in one place**, so the tests that split it and the test that checks the split cannot
/// drift apart. `sweep_widths` is what the sweep always was; `every_width_is_measured_once` proves the
/// split covers it exactly.
fn sweep_widths() -> Vec<u32> {
    let mut widths: Vec<u32> = (600..=1500).step_by(20).collect();
    widths.extend((600..=1500).rev().step_by(3));
    widths.extend([1280, 1024]);
    widths
}

/// The height the sweep pairs with a width: a wide window is measured tall, a narrow one short.
fn height_for(width: u32) -> u32 {
    if width >= 1280 { 720 } else { 640 }
}

/// **`len` widths cut into `parts` contiguous runs**, in order, last one short. The runs are a
/// partition: concatenated they are the input, so no width is dropped and none is measured twice.
fn chunk_bounds(len: usize, parts: usize) -> Vec<(usize, usize)> {
    let size = len.div_ceil(parts);
    (0..len).step_by(size).map(|start| (start, (start + size).min(len))).collect()
}

/// **How many processes the sweep is cut across** (#672). No width moves; only the schedule does.
///
/// Six is measured, not chosen. The loop's cost is its `App::update` calls, and the split's cost adds
/// one editor per part (~0.9 s of fixture, measured by `a_long_tab_name_is_cut_with_an_ellipsis_and_no_chip_is`)
/// on top of a fixed 16,752 updates. On this machine's 10 cores, cutting the three chip sets into
/// `n` parts each gives one part's work every `ceil(3n / 10)` rounds, so too few parts leaves cores
/// idle and too many adds a third round. Measured all three:
///
/// | parts | tests | one test, longest | sum of test-seconds | wall |
/// |---|---|---|---|---|
/// | 3 | 12 | 26.31 s | 242.5 s | 26.48 s |
/// | **6** | **21** | **12.64 s** | **212.4 s** | **22.81 s** |
/// | 9 | 30 | 10.48 s | 248.3 s | 26.45 s |
///
/// Six is best on both, and it is where the serial loop's 29.84 s wall and 29.53 s one-test figure
/// both come down. Nine is faster per test than six but pays a third scheduling round for it.
const CHIP_SWEEPS: usize = 6;

/// **One contiguous run of the sweep**: one editor, both drawn tabs, every width of `widths` at
/// `updates(8)` apiece, the same `faults` assertion the whole sweep made.
///
/// `prime` is the width the original sweep stood on immediately before this run, so the run begins
/// from the history it would have had. `measure_the_bar` keeps its cached room in a 16-logical-px
/// staircase, and a run that started cold would be a different experiment — see
/// `a_wider_or_narrower_history_changes_no_verdict` for the measurement that says it would not, in
/// fact, move one.
fn check_widths(tabs: usize, agent_command: &str, tag: &str, prime: Option<u32>, widths: &[u32]) -> Vec<String> {
    let mut app = editor(tag, (1024, 640), "/bin/sh", agent_command);
    six_tabs(&mut app, tabs);
    let mut bad = Vec::new();
    // The drawn tab is the agent's (its LED is the control that brings the self test back) and then a shell's (the LED stands alone).
    for drawn in [0, tabs - 1] {
        app.world_mut().resource_mut::<Terminal>().select(drawn);
        if let Some(width) = prime {
            bevy_dark_factory::harness::resize_surface(&mut app, width, height_for(width)).unwrap_or_else(|e| panic!("{e}"));
            updates(&mut app, 8);
        }
        for &width in widths {
            let height = height_for(width);
            bevy_dark_factory::harness::resize_surface(&mut app, width, height).unwrap_or_else(|e| panic!("{e}"));
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
    bad
}

/// One part of the sweep, asserted. `tag` names the part so its fixture and its message say which.
fn sweep_part(tabs: usize, agent_command: &str, tag: &str, part: usize) {
    let all = sweep_widths();
    let bounds = chunk_bounds(all.len(), CHIP_SWEEPS);
    let (start, end) = bounds[part];
    // The width the sweep stood on just before this run: the one before the run's first, or nothing
    // for the first run, which the editor is already built at (1024 x 640).
    let prime = start.checked_sub(1).map(|before| all[before]);
    let bad = check_widths(tabs, agent_command, &format!("chips_{tag}_{part}"), prime, &all[start..end]);
    assert!(
        bad.is_empty(),
        "{tabs} tabs, part {part} of {CHIP_SWEEPS} ({} of {} widths): a chip is cut at {} of the widths:\n{}",
        end - start,
        all.len(),
        bad.len(),
        bad.join("\n")
    );
}

/// **Every width the sweep names is measured exactly once**, across the parts. This is the split's
/// whole contract: splitting is a scheduling change, so the union of the parts must be the original
/// list — same elements, same order, none dropped, none measured twice.
///
/// A part that quietly lost a width would still pass its own assertion, and the suite would have
/// traded coverage for time without saying so. This is the test that makes that impossible.
#[test]
fn every_width_is_measured_once() {
    let all = sweep_widths();
    let bounds = chunk_bounds(all.len(), CHIP_SWEEPS);
    assert_eq!(bounds.len(), CHIP_SWEEPS, "the sweep is cut into the number of parts it claims: {bounds:?}");
    let mut covered = Vec::new();
    let mut previous_end = 0usize;
    for (part, &(start, end)) in bounds.iter().enumerate() {
        assert_eq!(start, previous_end, "part {part} starts where the one before it ended, with no gap: {bounds:?}");
        assert!(start < end, "part {part} measures at least one width: {bounds:?}");
        covered.extend_from_slice(&all[start..end]);
        previous_end = end;
    }
    assert_eq!(previous_end, all.len(), "the last part ends at the last width: {bounds:?}");
    assert_eq!(covered, all, "the parts together are the sweep, in its order, with nothing dropped and nothing repeated");
    // The two sizes of the issue are in the list, or the "`Start agent` stays" check inside the loop
    // would never run.
    for size in [1024, 1280] {
        assert!(all.contains(&size), "the {size} px size of the issue is measured: the sweep holds {}", all.len());
    }
}

/// One test per part of one chip set. The names say which part, so a red one points at its widths.
macro_rules! chip_sweep {
    ($tabs:expr, $agent:expr, $slug:literal, [$($part:expr => $name:ident),+ $(,)?]) => {
        $(
            #[test]
            fn $name() {
                sweep_part($tabs, $agent, $slug, $part);
            }
        )+
    };
}

chip_sweep!(3, SHORT_AGENT, "three", [
    0 => with_three_tabs_every_chip_is_whole_part_0,
    1 => with_three_tabs_every_chip_is_whole_part_1,
    2 => with_three_tabs_every_chip_is_whole_part_2,
    3 => with_three_tabs_every_chip_is_whole_part_3,
    4 => with_three_tabs_every_chip_is_whole_part_4,
    5 => with_three_tabs_every_chip_is_whole_part_5,
]);

chip_sweep!(6, SHORT_AGENT, "six", [
    0 => with_six_tabs_every_chip_is_whole_part_0,
    1 => with_six_tabs_every_chip_is_whole_part_1,
    2 => with_six_tabs_every_chip_is_whole_part_2,
    3 => with_six_tabs_every_chip_is_whole_part_3,
    4 => with_six_tabs_every_chip_is_whole_part_4,
    5 => with_six_tabs_every_chip_is_whole_part_5,
]);

const LONG_AGENT: &str = "/home/someone/.local/share/agents/bin/a-long-agent-program --flag";

chip_sweep!(3, LONG_AGENT, "long", [
    0 => a_long_agent_every_chip_is_whole_part_0,
    1 => a_long_agent_every_chip_is_whole_part_1,
    2 => a_long_agent_every_chip_is_whole_part_2,
    3 => a_long_agent_every_chip_is_whole_part_3,
    4 => a_long_agent_every_chip_is_whole_part_4,
    5 => a_long_agent_every_chip_is_whole_part_5,
]);

/// **A verdict does not depend on the width the bar was dragged from.** This is the precondition the
/// whole split rests on, and it is stated as a test rather than assumed.
///
/// `measure_the_bar` caches the bar's room only once it has moved by `BAR_ROOM_STEP` (16 logical px,
/// `terminal_pane.rs:955`), so the room the plan is made from is a staircase whose height depends on
/// the *history*: a descent and a climb disagree about it at most widths. What matters is that the
/// **answer** does not move — the same chips drawn, `faults()` the same — or a part of the sweep
/// starting from a different history would be a different experiment.
///
/// Sampled every 37th px rather than swept: the full 901 px both ways is two more sweeps, and the
/// point is the shape of the disagreement, which 25 samples show as well as 901. Measured on this
/// machine across all 901 the two traversals disagreed about the cached room at 722 widths and about
/// the verdict at none; the sample below must reproduce both halves of that.
#[test]
fn a_wider_or_narrower_history_changes_no_verdict() {
    const STRIDE: u32 = 37;
    let widths: Vec<u32> = (600..=1500u32).step_by(STRIDE as usize).collect();
    assert!(widths.len() >= 20, "the sample is wide enough to mean something: {}", widths.len());
    let mut app = editor("chips_history", (1500, 720), "/bin/sh", SHORT_AGENT);
    six_tabs(&mut app, 3);
    app.world_mut().resource_mut::<Terminal>().select(0);
    // What the bar is at a width: the room the plan is made from, the chips it drew, and whether any
    // chip is cut there.
    let snapshot = |app: &mut App| -> (Option<f32>, Vec<String>, bool) {
        let room = app.world().resource::<Terminal>().bar_room();
        let faulty = !faults(app).is_empty();
        let names: Vec<String> = items(app).1.iter().map(|c| c.name.clone()).collect();
        (room, names, faulty)
    };
    let mut descending = std::collections::HashMap::new();
    for width in widths.iter().rev() {
        bevy_dark_factory::harness::resize_surface(&mut app, *width, height_for(*width)).unwrap_or_else(|e| panic!("{e}"));
        updates(&mut app, 8);
        descending.insert(*width, snapshot(&mut app));
    }
    let mut rooms_differ = Vec::new();
    let mut verdict_differ = Vec::new();
    for width in &widths {
        bevy_dark_factory::harness::resize_surface(&mut app, *width, height_for(*width)).unwrap_or_else(|e| panic!("{e}"));
        updates(&mut app, 8);
        let climbing = snapshot(&mut app);
        let from_above = descending[width].clone();
        if climbing.0 != from_above.0 {
            rooms_differ.push(*width);
        }
        if (climbing.1.clone(), climbing.2) != (from_above.1.clone(), from_above.2) {
            verdict_differ.push((*width, from_above.1.clone(), climbing.1.clone()));
        }
    }
    assert!(
        verdict_differ.is_empty(),
        "a width reached from above draws the same bar, with the same verdict, as one reached from \
         below; {} of {} widths disagree: {:?}",
        verdict_differ.len(),
        widths.len(),
        &verdict_differ[..verdict_differ.len().min(8)]
    );
    // The staircase is real. Without this the test above could pass because the cached room is a
    // pure function of the width, and then it would be proving a dependency away that does not exist
    // — it should fail loudly rather than pass for the wrong reason.
    assert!(
        !rooms_differ.is_empty(),
        "the cached room is a staircase, so the two traversals must disagree about it somewhere; \
         none of {} widths did, and this test is then not measuring what it claims",
        widths.len()
    );
}

/// **A tab named by a long path is cut with an ellipsis**: a custom agent's tab is named by the first word of its command, and that can be a whole path.
#[test]
fn a_long_tab_name_is_cut_with_an_ellipsis_and_no_chip_is() {
    let mut app = editor("chips_long", (1024, 640), "/bin/sh", LONG_AGENT);
    six_tabs(&mut app, 3);
    let (_, chips, _) = items(&mut app);
    let word = chips.iter().find(|c| c.name == "tab 0").and_then(|c| c.word.clone()).expect("the agent's tab");
    assert!(word.ends_with('…') && word.chars().count() <= 16, "a long name is cut with an ellipsis to at most 16 characters: {word:?}");
}
