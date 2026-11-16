//! **Quitting with several stuck terminals costs one wait, not one each** (#427, review).
//!
//! Dropping a session hangs its shell up, gives it `HANGUP_GRACE_MS`, and kills it. Dropped one after another, three shells that
//! ignore the hangup cost three graces, which is more than the exit watchdog's whole deadline (`EXIT_GRACE_S`). The `Terminal` ends
//! every child first (hang up on all, wait once, kill all), so the bound is per quit.
#![cfg(unix)]

use std::time::{Duration, Instant};

use bevy_cauldron::terminal::{HANGUP_GRACE_MS, ShellChoice, Terminal};

const STUCK_TABS: usize = 3;

#[test]
fn three_shells_that_ignore_the_hangup_are_waited_for_together() {
    let shell = ShellChoice { program: "/bin/sh".to_owned(), args: vec!["-c".to_owned(), "trap '' HUP; exec sleep 20".to_owned()] };
    let mut terminal = Terminal::default();
    terminal.set_shell(Some(shell));
    for _ in 0..STUCK_TABS {
        terminal.spawn(80, 24, &std::env::temp_dir());
    }
    // Long enough for each shell to have installed its trap, or the hangup ends it at once and nothing is waited for.
    std::thread::sleep(Duration::from_millis(700));

    let started = Instant::now();
    drop(terminal);
    let took = started.elapsed();

    assert!(took >= Duration::from_millis(HANGUP_GRACE_MS * 9 / 10), "the shells had their grace ({took:?})");
    assert!(
        took < Duration::from_millis(HANGUP_GRACE_MS * 2),
        "{STUCK_TABS} stuck shells took {took:?} to close, which is one grace each; the quit has one deadline, so they are waited for together"
    );
}
