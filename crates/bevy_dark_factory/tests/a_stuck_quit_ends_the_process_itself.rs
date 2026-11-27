//! **A quit that does not finish ends the process itself, and a second press ends it at once** (#427), measured on a real process.
//!
//! Two other files hold the watchdog's decisions: `a_quit_that_does_not_finish_is_ended` the deadline, `a_second_press_of_the_quit_key_ends_a_quit_that_is_stuck`
//! the named waits, the record and the second press. Both hand the watchdog an action that *counts*, because the binary's own action ends
//! the process the test runs in.
//!
//! A count is exactly what cannot prove this bug: the report is a **hang**, so what a person needs is that **the process is gone**. These two
//! tests therefore drive the editor's own library code in a child process — this test binary, started again with a role in its environment, the
//! way `blueprint_model/tests/atomic_writes.rs` starts its writer — and then ask the operating system whether that child is still there:
//!
//! 1. The child holds a **real shell that ignores `SIGHUP`** and hangs it up (what dropping a pane's session does), writes `AppExit` through the real
//!    [`ExitWatchdogPlugin`] at the shipped grace, and then does what a teardown stuck on that shell does: waits on it. The parent requires the child
//!    to be **gone inside the deadline**, ended by the watchdog's `SIGKILL` rather than exiting on its own, and the record the child wrote first to
//!    name the wait that was cut short.
//! 2. The child arms the real watch with a second press and a deadline far past the test, then blocks. The parent presses after the debounce and
//!    requires the child to be **gone at once** — far inside the deadline that is the only other thing that could end it.
//!
//! The children write what the watchdog recorded to their scratch folder before they go, so a pass also says which wait was cut short.
//!
//! **The child is ended if the test fails**: the parent kills it in a guard, and the shell it left behind is killed by the pid the child recorded.
#![cfg(unix)]

use std::io::BufRead;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use bevy::prelude::*;
use bevy_dark_factory::exit_watchdog::{
    Cut, EXIT_GRACE_S, ExitWatchdog, ExitWatchdogPlugin, SecondPress, arm_exit, end_this_process, last_cut, waiting,
};

use crate::settle_support::Frames;

/// The environment variable that tells this same binary it is the child, and which role to play.
const ROLE_ENV: &str = "DARK_FACTORY_QUIT_ROLE";

/// The environment variable naming the child's scratch folder: what it was told to do, and what it records.
const SCRATCH_ENV: &str = "DARK_FACTORY_QUIT_SCRATCH";

/// The role that sticks on a shell which ignores `SIGHUP`, with the shipped deadline.
const DEADLINE_ROLE: &str = "stuck-on-a-shell";

/// The role whose exit is ended by a second press, with a deadline far past the test.
const SECOND_PRESS_ROLE: &str = "second-press";

/// The role that holds a **real terminal session** whose shell ignores `SIGHUP`, and then hangs: what a person has running when they quit.
const TERMINAL_ROLE: &str = "a-terminal-whose-shell-ignores-hangup";

/// The one test the child runs: the parent's filter, so the child runs this and nothing else.
const CHILD_TEST: &str = "the_child_that_is_a_stuck_quit";

/// Written by the child once its `AppExit` is written, so the parent times the deadline from it.
const EXITING_FILE: &str = "exiting";

/// Written by the child with the pid of the shell it is stuck on, so the parent can end it after the child is gone.
const SHELL_PID_FILE: &str = "shell.pid";

/// Written by the child once its watch is armed and its stuck wait is listed, so the parent presses at the right moment.
const STUCK_FILE: &str = "stuck";

/// The press the second-press child's probe sees: the parent makes it, the child's watchdog reads it.
const PRESS_FILE: &str = "press";

/// What the watchdog recorded when it ended the child. The record is process-global state, so it is written out before the process goes.
const RECORD_FILE: &str = "record.txt";

/// How long the shell the child is stuck on lives if nothing ends it, seconds: far past every bound below, so only the watchdog can end the wait.
const SHELL_LIFE_S: u64 = 600;

/// How long the exit watchdog in the child calls the hang-up wait overdue after, milliseconds: short, so the record names it as past its bound.
const HANGUP_WAIT_BOUND_MS: u64 = 300;

/// The deadlock the second-press child is stuck on, seconds: longer than the test, so only a press can end it.
const NEVER_S: u64 = 600;

/// The second-press child's deadline, seconds: far past the test, so a pass cannot be the deadline arriving.
const SECOND_PRESS_DEADLINE_S: u64 = 60;

/// The debounce the second-press child is given, milliseconds: long enough that the parent can press inside it and be sure it did, then wait it
/// out and press again.
const SECOND_PRESS_DEBOUNCE_MS: u64 = 1_500;

/// How long a press is held down, milliseconds: long enough for the child's probe to see it, short enough to fit inside the debounce.
const PRESS_HELD_MS: u64 = 150;

/// How often the second-press child's probe looks, milliseconds.
const SECOND_PRESS_POLL_MS: u64 = 10;

/// `SIGKILL`, the signal the watchdog ends a process with: `exit` would run the very hooks a stuck process is stuck in.
const SIGKILL: i32 = 9;

/// How long the watchdog may spend recording where the process is stuck **before** it kills it, milliseconds: the watchdog writes `exit-hang.txt`
/// with `sample` first, so a Mac's `sample` is part of the wait a person sees. The process is required to be gone inside the deadline *plus* this.
#[cfg(target_os = "macos")]
const RECORD_BOUND_MS: u64 = bevy_dark_factory::exit_watchdog::SAMPLE_PATIENCE_MS;

/// Elsewhere there is nothing to record with, so the record is free.
#[cfg(not(target_os = "macos"))]
const RECORD_BOUND_MS: u64 = 0;

/// How far past a deadline a process may be ended and the test still call it inside it, seconds: a loaded machine, and the look that follows.
const SLACK_S: u64 = 2;

/// How long one look for a child's file sleeps.
const LOOK: Duration = Duration::from_millis(5);

/// The most looks a test gives a child to write a file: at least forty seconds at [`LOOK`], counted in looks so a starved
/// host is not read as a child that never got there (#644). Past every deadline below and far inside [`SHELL_LIFE_S`].
const PATIENCE: Frames = Frames::covering(Duration::from_secs(40), LOOK);

/// How long one look for the terminal's shell to be gone sleeps.
const SHELL_LOOK: Duration = Duration::from_millis(10);

/// The looks the kernel is given to deliver the group's kill to the shell: at least [`SLACK_S`] at [`SHELL_LOOK`] (#644).
const SHELL_GONE_WITHIN: Frames = Frames::covering(Duration::from_secs(SLACK_S), SHELL_LOOK);

/// How long a second press may take to end the process, seconds: at once, not at a deadline twenty times as far.
const MOST_A_SECOND_PRESS_MAY_TAKE_S: u64 = 5;

/// How many 50 ms looks the child gives its shell to say it is ready.
const TRIES: usize = 100;

// ---------------------------------------------------------------------------------------------------------------
// The child: this binary, started again with a role. Run bare (no role) it is not a test of anything, as
// `atomic_writes::a_writer_to_kill` is not.
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn the_child_that_is_a_stuck_quit() {
    let Some(role) = std::env::var(ROLE_ENV).ok() else { return };
    let scratch = scratch_dir();
    match role.as_str() {
        DEADLINE_ROLE => stuck_on_a_shell_that_ignores_hangup(&scratch),
        SECOND_PRESS_ROLE => ended_by_a_second_press(&scratch),
        TERMINAL_ROLE => a_terminal_whose_shell_ignores_hangup(&scratch),
        other => panic!("the child was given a role there is no child for: {other}"),
    }
}

/// **The child of the first test**: a real shell that ignores `SIGHUP` and is hung up on, `AppExit` written through the real plugin, and then the
/// wait that hangs.
fn stuck_on_a_shell_that_ignores_hangup(scratch: &Path) {
    // A shell that ignores the hangup and keeps running: what a pane's shell can be when the pane's session is dropped. `exec` keeps the pid
    // and the ignored disposition, so the process hung up on is the one waited for.
    let script = format!("trap '' HUP; echo ready; exec sleep {SHELL_LIFE_S}");
    let mut shell = Command::new("/bin/sh")
        .args(["-c", &script])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("a shell to hold: {e}"));
    let mut ready = String::new();
    std::io::BufReader::new(shell.stdout.take().expect("a piped stdout"))
        .read_line(&mut ready)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(ready.trim(), "ready", "the shell ignores the hangup before anything is sent to it");
    let shell_pid = i32::try_from(shell.id()).ok().and_then(rustix::process::Pid::from_raw).expect("a pid this system can name");
    write_file(scratch, SHELL_PID_FILE, &shell.id().to_string());

    // The editor's own quit: `AppExit` written, the watchdog armed with the shipped grace and the binary's own action.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(ExitWatchdogPlugin {
        watchdog: ExitWatchdog { grace: Duration::from_secs(EXIT_GRACE_S), then: record_then_end },
    });
    app.world_mut().write_message(AppExit::Success);
    app.update();
    write_file(scratch, EXITING_FILE, "");

    // The teardown that does not finish: the shell is hung up on, as dropping its pane does, and waited for. Its wait is listed by name so the
    // record can say what was cut short. The hangup is ignored by the shell, which is the whole point: nothing but the watchdog ends this.
    let _wait = waiting("a shell that ignores SIGHUP", Duration::from_millis(HANGUP_WAIT_BOUND_MS));
    let _ = rustix::process::kill_process(shell_pid, rustix::process::Signal::HUP);
    let _ = shell.wait();
    // Reaching here means the quit finished: the child exits by itself, which is the failure the parent reports.
    panic!("the quit finished on its own: the watchdog never ended this process");
}

/// **The child of the second test**: the real watch with a second press, a deadline far past the test, and a wait that never ends.
fn ended_by_a_second_press(scratch: &Path) {
    let second = SecondPress {
        debounce: Duration::from_millis(SECOND_PRESS_DEBOUNCE_MS),
        poll: Duration::from_millis(SECOND_PRESS_POLL_MS),
        down: the_press_is_up,
    };
    arm_exit(Duration::from_secs(SECOND_PRESS_DEADLINE_S), Some(second), record_then_end).unwrap_or_else(|e| panic!("the watch did not start: {e}"));
    let _stuck = waiting("a wait that never ends", Duration::from_secs(NEVER_S));
    write_file(scratch, STUCK_FILE, "");
    loop {
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// **The child of the third test**: a real [`TerminalSession`] whose shell ignores `SIGHUP`, then a hang so it cannot be closed in the
/// ordinary course. Its shell is what a person has running (`cargo build`, `vim`, an agent) when they quit.
fn a_terminal_whose_shell_ignores_hangup(scratch: &Path) {
    let shell = bevy_dark_factory::terminal::ShellChoice {
        program: "/bin/sh".to_owned(),
        args: vec!["-c".to_owned(), format!("trap '' HUP; echo ready; exec sleep {SHELL_LIFE_S}")],
    };
    let mut session = bevy_dark_factory::terminal::TerminalSession::spawn_with(80, 24, &std::env::temp_dir(), &[], Some(&shell))
        .unwrap_or_else(|e| panic!("a terminal session: {e}"));
    // Read until the shell has said its trap is in place, so the parent knows the session is really up before it is ended.
    let mut seen = Vec::new();
    for _ in 0..TRIES {
        session.drain(&mut seen).unwrap_or_else(|e| panic!("{e}"));
        if String::from_utf8_lossy(&seen).contains("ready") {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(String::from_utf8_lossy(&seen).contains("ready"), "the shell never said ready: {:?}", String::from_utf8_lossy(&seen));
    write_file(scratch, SHELL_PID_FILE, &session.pid().to_string());

    // The editor's own quit, and then a main thread that never finishes: only the watchdog can end this, and it must take the shell too.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(ExitWatchdogPlugin {
        watchdog: ExitWatchdog { grace: Duration::from_secs(EXIT_GRACE_S), then: record_then_end },
    });
    app.world_mut().write_message(AppExit::Success);
    app.update();
    write_file(scratch, EXITING_FILE, "");
    let _stuck = waiting("a terminal session held open by a hang", Duration::from_secs(NEVER_S));
    // The session is deliberately leaked: a real quit is stuck on the main thread with it still alive, not dropping it first.
    std::mem::forget(session);
    loop {
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// **Whether the parent has pressed the quit key again**, as the watchdog's probe sees it: the file the parent makes.
fn the_press_is_up() -> bool {
    std::env::var_os(SCRATCH_ENV).is_some_and(|dir| PathBuf::from(dir).join(PRESS_FILE).exists())
}

/// **What the watchdog recorded, then the process ends**: the binary's own action, the record written first because the record is memory.
fn record_then_end() {
    let scratch = scratch_dir();
    let said = match last_cut() {
        Some(cut) => format!("why={:?}\nwaits={}\n", cut.why, cut.waits.join("; ")),
        None => "why=none\n".to_owned(),
    };
    write_file(&scratch, RECORD_FILE, &said);
    end_this_process();
}

fn scratch_dir() -> PathBuf {
    std::env::var_os(SCRATCH_ENV).map(PathBuf::from).unwrap_or_else(|| panic!("the child was given no {SCRATCH_ENV}"))
}

fn write_file(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap_or_else(|e| panic!("writing {name}: {e}"));
}

// ---------------------------------------------------------------------------------------------------------------
// The parent: start the child, time it, and ask the operating system whether it is gone.
// ---------------------------------------------------------------------------------------------------------------

/// A child that is ended if the test fails part-way, so a failure does not leave a stuck process behind.
struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A scratch folder of this test's own: tests share a process, so the name carries the test and the pid.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("dark_factory-stuck-quit-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    dir
}

/// **This binary again, in `role`**, with `scratch` as its folder: the editor's own code in a process a test may watch die.
fn start_child(role: &str, scratch: &Path) -> KillOnDrop {
    let exe = std::env::current_exe().unwrap_or_else(|e| panic!("{e}"));
    let child = Command::new(exe)
        .args([CHILD_TEST, "--test-threads=1", "--nocapture"])
        .env(ROLE_ENV, role)
        .env(SCRATCH_ENV, scratch)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("the child did not start: {e}"));
    KillOnDrop(child)
}

/// Waits for the child to write `name`, failing if it goes first: a child that is gone before it is stuck proved nothing. Says
/// when it saw the file, on the wall clock, which is what the deadlines below are measured from.
fn wait_for_file(child: &mut Child, dir: &Path, name: &str) -> Instant {
    for _ in 0..PATIENCE.0 {
        if dir.join(name).exists() {
            return Instant::now();
        }
        if let Some(status) = child.try_wait().unwrap_or_else(|e| panic!("{e}")) {
            panic!("the child went before it wrote `{name}`: {status}; record: {}", record(dir));
        }
        std::thread::sleep(LOOK);
    }
    panic!("the child never wrote `{name}` within {PATIENCE:?}: {}", record(dir));
}

/// **How long the child took to be gone**, and how it went.
fn wait_until_gone(child: &mut Child, dir: &Path, patience: Duration) -> (Duration, std::process::ExitStatus) {
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().unwrap_or_else(|e| panic!("{e}")) {
            return (started.elapsed(), status);
        }
        assert!(started.elapsed() < patience, "the process is still there {} s after it should have gone: record: {}", patience.as_secs(), record(dir));
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn record(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(RECORD_FILE)).unwrap_or_else(|_| "(the child recorded nothing)".to_owned())
}

/// The signal a process was ended by, if it was ended by one.
fn signal_of(status: std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

/// Whether `pid` is still there.
fn alive(pid: u32) -> bool {
    let Some(pid) = i32::try_from(pid).ok().and_then(rustix::process::Pid::from_raw) else { return false };
    rustix::process::test_kill_process(pid).is_ok()
}

/// **Ends the shell the child left behind**, which is its own process and outlives the child that held it.
fn end_the_leftover_shell(dir: &Path) {
    let Ok(text) = std::fs::read_to_string(dir.join(SHELL_PID_FILE)) else { return };
    let Ok(pid) = text.trim().parse::<u32>() else { return };
    if let Some(pid) = i32::try_from(pid).ok().and_then(rustix::process::Pid::from_raw) {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
    }
}

#[test]
fn a_quit_stuck_on_a_shell_that_ignores_hangup_ends_the_process_itself_inside_the_deadline() {
    let dir = scratch("deadline");
    let mut child = start_child(DEADLINE_ROLE, &dir);
    // From here the exit has begun: the watchdog was armed as `AppExit` was written.
    let asked = wait_for_file(&mut child.0, &dir, EXITING_FILE);
    let deadline = Duration::from_secs(EXIT_GRACE_S) + Duration::from_millis(RECORD_BOUND_MS) + Duration::from_secs(SLACK_S);
    let (after, status) = wait_until_gone(&mut child.0, &dir, deadline);
    end_the_leftover_shell(&dir);
    let took = asked.elapsed();

    assert_eq!(
        signal_of(status),
        Some(SIGKILL),
        "the child went with {status}, not the `SIGKILL` the watchdog ends a stuck quit with: {} s after its exit began",
        after.as_secs_f32()
    );
    assert!(
        took >= Duration::from_secs(EXIT_GRACE_S),
        "the child was ended {took:?} after its exit began, before the deadline of {EXIT_GRACE_S} s: the shell was killed early, not the watchdog's doing"
    );
    assert!(took <= deadline, "the child outlived its deadline by {took:?}: the deadline is {EXIT_GRACE_S} s and the record at most {RECORD_BOUND_MS} ms more");

    let said = record(&dir);
    assert!(said.contains("why=Deadline"), "the watchdog recorded the deadline as what ended it: {said}");
    assert!(said.contains("a shell that ignores SIGHUP"), "the record names the wait it cut short: {said}");
}

#[test]
fn a_second_press_after_the_debounce_ends_the_process_itself_at_once() {
    let dir = scratch("second-press");
    let mut child = start_child(SECOND_PRESS_ROLE, &dir);
    // The watch is armed and the stuck wait is listed: the quit is in progress, and the debounce counts from here.
    let armed = wait_for_file(&mut child.0, &dir, STUCK_FILE);

    // **A press inside the debounce is the first one bouncing, not a second press**: it must not end the quit. Without this the test could pass on a
    // watchdog that ended every press, which would kill people who fat-finger the chord.
    // **The test's own pacing, named**: "it did not end the quit" can only be asked after the watchdog has had time to end it
    // wrongly, which is a person's press against a real debounce — a wall-clock span, not a wait for a view.
    const A_WRONG_ENDING_WOULD_SHOW_WITHIN: Duration = Duration::from_millis(100);
    press(&dir);
    std::thread::sleep(A_WRONG_ENDING_WOULD_SHOW_WITHIN);
    assert!(alive(child.0.id()), "a press inside the debounce of {SECOND_PRESS_DEBOUNCE_MS} ms ended the quit, which is the first press bouncing");

    // The debounce runs out, and a press now is meant: the person has waited.
    let wait_out = Duration::from_millis(SECOND_PRESS_DEBOUNCE_MS).saturating_sub(armed.elapsed());
    std::thread::sleep(wait_out + A_WRONG_ENDING_WOULD_SHOW_WITHIN);
    assert!(alive(child.0.id()), "the child ended without a second press: {}", record(&dir));

    let pressed = Instant::now();
    write_file(&dir, PRESS_FILE, "");
    let (_, status) = wait_until_gone(&mut child.0, &dir, Duration::from_secs(MOST_A_SECOND_PRESS_MAY_TAKE_S));
    let took = pressed.elapsed();

    assert_eq!(
        signal_of(status),
        Some(SIGKILL),
        "the child went with {status}, not the `SIGKILL` a second press ends a stuck quit with; it would have lived {SECOND_PRESS_DEADLINE_S} s until its deadline"
    );
    assert!(took <= Duration::from_secs(MOST_A_SECOND_PRESS_MAY_TAKE_S), "the press ended the child after {took:?}, not at once");

    let said = record(&dir);
    assert!(said.contains(&format!("why={:?}", Cut::SecondPress)), "the record says a second press ended it, not a deadline: {said}");
    assert!(said.contains("a wait that never ends"), "the record names the wait the press cut short: {said}");
}

/// **Presses the quit key and lets it go**, as a finger does: the child's probe reads the file, and the watchdog's rising edge needs it up again
/// before the next press counts.
fn press(dir: &Path) {
    write_file(dir, PRESS_FILE, "");
    std::thread::sleep(Duration::from_millis(PRESS_HELD_MS));
    std::fs::remove_file(dir.join(PRESS_FILE)).unwrap_or_else(|e| panic!("releasing the press: {e}"));
    // The probe polls, so it must see the key up before anything reads a second press.
    std::thread::sleep(Duration::from_millis(SECOND_PRESS_POLL_MS * 4));
}

#[test]
fn a_quit_that_has_to_end_the_process_ends_the_terminals_shell_too() {
    // **Measured 2026-11-18 (macOS 26.6, bmb): a shell that ignores `SIGHUP`, in a PTY as a session leader, outlives a `SIGKILL`
    // of its parent as an orphan** (its parent reparented to 1). The shell here is a real `TerminalSession`'s, so this fails if the
    // watchdog ends only itself: `cargo build` left running with no window anywhere drawing it is exactly the report.
    let dir = scratch("terminal-shell");
    let mut child = start_child(TERMINAL_ROLE, &dir);
    let asked = wait_for_file(&mut child.0, &dir, EXITING_FILE);
    let shell_pid = std::fs::read_to_string(dir.join(SHELL_PID_FILE)).expect("the child wrote its shell's pid").trim().parse::<u32>().expect("a pid");
    assert!(alive(shell_pid), "the shell was not running before the quit");

    let deadline = Duration::from_secs(EXIT_GRACE_S) + Duration::from_millis(RECORD_BOUND_MS) + Duration::from_secs(SLACK_S);
    let (_, status) = wait_until_gone(&mut child.0, &dir, deadline);
    assert_eq!(signal_of(status), Some(SIGKILL), "the child was not ended by the watchdog: {status}");
    let took = asked.elapsed();

    // The shell is its own process group, and the watchdog ends the group: give the kernel a moment to deliver, then require it gone.
    for _ in 0..SHELL_GONE_WITHIN.0 {
        if !alive(shell_pid) {
            break;
        }
        std::thread::sleep(SHELL_LOOK);
    }
    if alive(shell_pid) {
        end_the_leftover_shell(&dir);
        panic!("the shell {shell_pid} outlived the editor by {took:?}: it is an orphan (`ps -o ppid` says 1), which is the shell a person's `cargo build` runs in");
    }
}
