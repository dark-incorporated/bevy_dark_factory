//! **Closing a terminal keeps reading the master while it waits for the shell to leave** (#427).
//!
//! Measured on a real macOS build (bmb, 0.6.8, 2026-11-15): with an agent tab open, a Cmd+Q left the process alive for five minutes
//! and more. The stack of the main thread (an abort report taken while it hung) was `World::clear_all` <- `app_will_terminate` <-
//! `Terminal` <- `TerminalTab` <- `Pty::drop` <- `Child::wait` <- `wait4`, and the shell it waited for, and every process of its
//! group, stayed in the `E` (exiting) state through `SIGKILL`. **On macOS a process cannot finish exiting while the terminal holds
//! output nobody has read**, and the master's only reader is the editor's main thread, which was the thread waiting for the shell:
//! each waited for the other. The same two processes in a bare Python `pty.fork` reproduce it, and reading the master (or closing it)
//! frees them at once.
//!
//! So closing a session reads, and drops, what its children write while it waits. This test is that shape: a shell that ignores the
//! hangup, a subshell of it writing far more than a terminal holds, and the shell itself waiting. A close that does not read never
//! returns on macOS (this test is red there without the fix); on Linux a kill ends the shell whatever is written, so it guards the
//! close from regressing into a wait that is not bounded.
#![cfg(unix)]

use std::sync::mpsc;
use std::time::Duration;

use bevy_cauldron::terminal::{HANGUP_GRACE_MS, ShellChoice, TerminalSession};

/// How many 50 ms looks the test gives the child to say it is ready.
const TRIES: usize = 200;

/// Far more than a terminal holds (Linux 64 KiB, macOS a few KiB): the writer blocks until someone reads the master.
const WRITE_KIB: u32 = 4_096;

/// The most the close may take: the grace, the wait after the kill and slack for a loaded machine. A close that waits for the shell
/// without reading it never returns, so any bound tells it apart.
const MOST_A_CLOSE_MAY_TAKE: Duration = Duration::from_millis(HANGUP_GRACE_MS + 8_000);

#[test]
fn a_child_blocked_writing_to_the_terminal_does_not_hold_the_close() {
    // The shell ignores the hangup, says ready, starts a writer that also ignores it and writes `WRITE_KIB` KiB to the terminal, and
    // then waits (it is not `exec`ed into `sleep`: the shell must be the session leader that cannot leave while the writer is stuck).
    let script = format!("trap '' HUP; echo ready; ( exec 2>&1; trap '' HUP; dd if=/dev/zero bs=1024 count={WRITE_KIB} 2>/dev/null ) & sleep 20");
    let shell = ShellChoice { program: "/bin/sh".to_owned(), args: vec!["-c".to_owned(), script] };
    let mut session = TerminalSession::spawn_with(80, 24, &std::env::temp_dir(), &[], Some(&shell)).unwrap_or_else(|e| panic!("{e}"));
    let mut seen = Vec::new();
    for _ in 0..TRIES {
        session.drain(&mut seen).unwrap_or_else(|e| panic!("{e}"));
        if String::from_utf8_lossy(&seen).contains("ready") {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(String::from_utf8_lossy(&seen).contains("ready"), "the child said ready: {:?}", String::from_utf8_lossy(&seen));
    // Give the writer time to fill what the terminal holds.
    std::thread::sleep(Duration::from_millis(500));

    // The close runs on a thread, so a close that never returns fails this test and does not hang the run.
    let (closed, done) = mpsc::channel();
    std::thread::spawn(move || {
        drop(session);
        let _ = closed.send(());
    });
    assert!(
        done.recv_timeout(MOST_A_CLOSE_MAY_TAKE).is_ok(),
        "closing the session did not return in {MOST_A_CLOSE_MAY_TAKE:?}: it waited for a shell that cannot leave while what its children wrote \
         to the terminal is unread (on macOS the editor's quit then waits for ever)"
    );
}
