//! **The wait for a closed session's child** (#427): it reads the terminal at every look, kills when the grace has passed, and gives up
//! after the kill rather than waiting on a child nothing can end.
#![cfg(unix)]

use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use rustix::process::{Pid, Signal};

use super::pty::end_the_child;

const GRACE: Duration = Duration::from_millis(200);
const AFTER_KILL: Duration = Duration::from_millis(150);

/// A child that ignores the hangup and lives far longer than any bound here.
fn stubborn_child() -> Child {
    Command::new("/bin/sh")
        .args(["-c", "trap '' HUP; exec sleep 30"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("{e}"))
}

fn pid_of(child: &Child) -> Pid {
    Pid::from_raw(i32::try_from(child.id()).unwrap_or_else(|e| panic!("{e}"))).unwrap_or_else(|| panic!("a pid"))
}

#[test]
fn the_terminal_is_read_at_every_look_before_and_after_the_kill() {
    let mut child = stubborn_child();
    // `sh` has to have installed its trap before the hangup, or the hangup kills it and the grace is never used.
    std::thread::sleep(Duration::from_millis(300));
    let pid = pid_of(&child);
    let mut reads = 0usize;
    let mut sent = Vec::new();
    let started = Instant::now();
    let left = end_the_child(
        pid,
        GRACE,
        AFTER_KILL,
        &mut || reads += 1,
        &mut |signal| {
            sent.push(signal);
            let _ = rustix::process::kill_process(pid, signal);
        },
    );
    let took = started.elapsed();
    let _ = child.wait();

    assert!(left, "a killed child that is not held by anything leaves");
    assert_eq!(sent, [Signal::HUP, Signal::KILL], "hung up first, killed when the grace had passed");
    assert!(took >= GRACE * 9 / 10, "the child had its grace ({took:?})");
    assert!(reads >= 10, "the terminal was read at every look while the child was given its grace, not once: {reads} reads");
}

#[test]
fn a_child_that_cannot_be_ended_is_given_up_on_after_the_kill() {
    let mut child = stubborn_child();
    std::thread::sleep(Duration::from_millis(300));
    let pid = pid_of(&child);
    let mut sent = Vec::new();
    let started = Instant::now();
    // Signals that reach nothing: the child stays, as one held in the kernel does on macOS.
    let left = end_the_child(pid, GRACE, AFTER_KILL, &mut || {}, &mut |signal| sent.push(signal));
    let took = started.elapsed();
    let _ = rustix::process::kill_process(pid, Signal::KILL);
    let _ = child.wait();

    assert!(!left, "the child is still there, and the wait says so");
    assert_eq!(sent, [Signal::HUP, Signal::KILL]);
    assert!(took >= GRACE + AFTER_KILL - Duration::from_millis(30), "it waited the grace and the time after the kill ({took:?})");
    assert!(took < Duration::from_secs(5), "and then it stopped waiting ({took:?})");
}
