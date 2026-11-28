//! **A raised terminal shows its shell's words within the frames measured for it** (#541).
//!
//! #541's ceiling is 0.1 s for raising the terminal from a Map with one open shell. A wall clock is the host's as much as
//! the editor's, so this counts the work instead, as `a_switch_frame_names_its_systems_and_keeps_its_budget` does for a
//! switch: the **frames** from the raise to the first frame a row of the shell's words is laid out, and the **text nodes**
//! the raise spawns. A lowered pane keeps its rows (`Display::None` on the pane, `terminal_pane`), so a raise is a layout
//! and not a rebuild. `--no-capture` prints each raise's counts.

use std::collections::HashSet;

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_pane::TerminalRow;

use crate::fixtures;

/// How many frames, 50 ms apart, the test waits for the shell to print.
const TRIES: usize = 80;

/// **The most frames a raise may take** to lay out the shell's words: the raise is read in the frame it lands in and the
/// pane is laid out in that frame's `PostUpdate`, so the words are in the tree on the first frame after the raise.
const FRAME_BUDGET: usize = 1;

/// **The most text nodes a raise may spawn**: a lowered pane keeps its rows, so none.
const TEXT_BUDGET: usize = 0;

/// The words the shell prints, so a laid-out row is known to be the shell's and not the cell probe's.
const WORDS: &str = "raised words";

/// **`SHELL` is process-wide**, so the test holds this while it has it set.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct OurShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// Whether a terminal row laid out with a height holds the shell's words. A row's `Text` is empty and its words are its
/// `TextSpan` children, one per run of same-coloured cells (`terminal_view`).
fn words_laid_out(app: &mut App) -> bool {
    let world = app.world_mut();
    let laid_out: HashSet<Entity> = world
        .query_filtered::<(Entity, &ComputedNode), With<TerminalRow>>()
        .iter(world)
        .filter(|(_, node)| node.size().y > 0.0)
        .map(|(row, _)| row)
        .collect();
    let mut spans = world.query::<(&TextSpan, &ChildOf)>();
    spans.iter(world).any(|(span, parent)| laid_out.contains(&parent.parent()) && span.0.contains(WORDS))
}

#[test]
fn a_raise_lays_out_the_shells_words_on_the_next_frame_and_spawns_no_text() {
    let root = fixtures::Fixture::new(&format!("raised_words_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let shell = root.join("words-shell");
    std::fs::write(&shell, format!("#!/bin/sh\nprintf '{WORDS}\\n$ '\nexec cat >/dev/null\n"))
        .unwrap_or_else(|e| panic!("{}: {e}", shell.display()));
    std::fs::set_permissions(&shell, std::os::unix::fs::PermissionsExt::from_mode(0o755))
        .unwrap_or_else(|e| panic!("{}: {e}", shell.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: the guard above is held for the test's whole run, and nextest runs each test in a process of its own.
    unsafe { std::env::set_var("SHELL", shell.as_os_str()) };
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_non_send(OurShell(guard));
    app.world_mut().spawn((Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() }, PrimaryWindow));
    // **One open shell**: raised once, and its words on screen.
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
    let mut shown = false;
    for _ in 0..TRIES {
        app.update();
        if words_laid_out(&mut app) {
            shown = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(shown, "the shell's words were never laid out in the raised pane");

    for raise in 1..=5 {
        app.world_mut().resource_mut::<Terminal>().set_collapsed(true);
        for _ in 0..3 {
            app.update();
        }
        assert!(!words_laid_out(&mut app), "lowered, the pane lays out no words (raise {raise})");
        let texts: HashSet<Entity> = {
            let world = app.world_mut();
            world.query_filtered::<Entity, Or<(With<Text>, With<TextSpan>)>>().iter(world).collect()
        };
        app.world_mut().resource_mut::<Terminal>().set_collapsed(false);
        let mut frames = 0;
        while !words_laid_out(&mut app) {
            assert!(frames < 10, "raise {raise}: the shell's words were not laid out within 10 frames");
            app.update();
            frames += 1;
        }
        let spawned = {
            let world = app.world_mut();
            world.query_filtered::<Entity, Or<(With<Text>, With<TextSpan>)>>().iter(world).filter(|e| !texts.contains(e)).count()
        };
        println!("raise {raise}: words laid out after {frames} frame(s), {spawned} text node(s) spawned");
        assert!(frames <= FRAME_BUDGET, "raise {raise} took {frames} frames to lay out the shell's words, over {FRAME_BUDGET}");
        assert!(spawned <= TEXT_BUDGET, "raise {raise} spawned {spawned} text nodes, over {TEXT_BUDGET}: the rows were rebuilt");
    }
}
