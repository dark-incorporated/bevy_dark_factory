//! **A prompt of one glyph still gives the terminal a measured cell** (#674).
//!
//! The cell was the step between two neighbouring glyphs' pens in a drawn row, and a row is trimmed of its trailing
//! blanks, so dash's `$ ` — Ubuntu's `/bin/sh`, and so CI's — is one glyph: no pair, no measurement. The cell stayed the
//! 0.6 em ratio (6.0 × 12.0 logical px where the face lays out 5.83 × 12.5), so the caret, the column count and the cell
//! under a click drifted a sixth of a pixel a column until the shell printed something longer. The shell here prints
//! exactly `$ ` and nothing else, on every host.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use bevy_dark_factory::terminal::Terminal;
use bevy_dark_factory::terminal_view::CellMetrics;

use crate::fixtures;

/// How many frames, 50 ms apart, the test waits for the shell and for the measurement.
const TRIES: usize = 60;

/// **`SHELL` is process-wide**, so the test holds this while it has it set.
static SHELL_ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct OneGlyphShell(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

/// The terminal's screen as text, trailing blanks and empty rows dropped, or `None` while no shell is open. The first shell
/// opens on the first frame the pane is raised after the settings are read (`terminal_view::pump_the_terminal`), and the
/// pump runs in `PreUpdate` while the settings are taken in `Update`, so the first frame has none.
fn screen_text(app: &mut App) -> Option<String> {
    let mut terminal = app.world_mut().resource_mut::<Terminal>();
    let screen = terminal.tab_mut()?.screen_mut();
    let lines: Vec<String> = screen.lines().iter().map(|line| line.cells.iter().map(|c| c.ch).collect()).collect();
    Some(lines.iter().map(|line| line.trim_end()).collect::<Vec<_>>().join("\n").trim_end().to_owned())
}

fn until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    for _ in 0..TRIES {
        app.update();
        if done(app) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    panic!("never: {what}");
}

#[test]
fn a_shell_whose_prompt_is_one_glyph_still_gets_a_measured_cell() {
    let root = fixtures::Fixture::new(&format!("one_glyph_prompt_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let shell = root.join("one-glyph-shell");
    std::fs::write(&shell, "#!/bin/sh\nprintf '$ '\nexec cat >/dev/null\n").unwrap_or_else(|e| panic!("{}: {e}", shell.display()));
    std::fs::set_permissions(&shell, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap_or_else(|e| panic!("{}: {e}", shell.display()));
    let guard = SHELL_ENV.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: the guard above is held for the test's whole run, and nextest runs each test in a process of its own.
    unsafe { std::env::set_var("SHELL", shell.as_os_str()) };
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<Terminal>().set_collapsed(false); // #424: the pane starts collapsed
    app.insert_non_send(OneGlyphShell(guard));
    app.world_mut().spawn((
        Window { resolution: bevy::window::WindowResolution::new(1280, 720), ..default() },
        PrimaryWindow,
    ));

    until(&mut app, "the shell's prompt on screen", |app| screen_text(app).as_deref() == Some("$"));
    until(&mut app, "a measured cell while the screen holds one glyph", |app| {
        app.world().resource::<CellMetrics>().is_measured()
    });
    assert_eq!(
        screen_text(&mut app).as_deref(),
        Some("$"),
        "the measurement came from the pane, not from anything the shell printed"
    );
}
