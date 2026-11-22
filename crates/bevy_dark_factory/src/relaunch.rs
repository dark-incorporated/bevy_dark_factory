//! **The editor relaunches itself on a new build, for the editor development agent** (`blueprint/relaunch_editor`).
//!
//! The editor development agent (`terminal_agent::AgentKind::EditorDev`) changes the editor's own Rust, builds it, and asks to see
//! it running. A relaunch is that: the editor saves the way `Cmd+S` does, starts the binary that was just built on the same
//! arguments plus `--restore-dev-agent <session>`, and quits; the new editor opens the agent's omp session again in its
//! terminal (`omp --restore`), so the agent continues the turn that asked.
//!
//! **Refused, saying why, before anything is saved**: a token that is not an editor development tab's, an agent that has not
//! said which omp session it is in yet, and a binary on disk that is not newer than this process (no new build to run).
//!
//! **The new editor waits for this one to be gone.** A small shell, in its own process group so it outlives this one, polls
//! this process's id and then `exec`s the new binary: two editors never hold one project's files at once.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bevy::prelude::*;

use crate::terminal_agent::AgentKind;

/// **When this editor started**, so a binary built since can be told from the one running. Inserted by `main`.
#[derive(Resource, Clone, Copy, Debug)]
pub struct StartedAt(pub SystemTime);

/// **A relaunch asked for and not yet carried out**: the save has been queued and the frames it needs are counted down.
#[derive(Resource, Clone, Debug)]
pub struct PendingRelaunch {
    /// The binary to start: this process's own path, which `cargo build` wrote the new build over.
    pub exe: PathBuf,
    /// The arguments to start it with, `--restore-dev-agent <session>` last.
    pub args: Vec<String>,
    /// Frames left before the relaunch: the save's chord goes down and up first.
    pub frames_left: u32,
}

/// **Frames between the request and the relaunch**: the queued save chord goes down one frame and up the next
/// (`command_run::Chords`, one chord per two frames), and the save is written on the frame its key goes down. Six leaves the
/// save two frames to land with nothing else queued ahead of it.
const RELAUNCH_AFTER_FRAMES: u32 = 6;

/// **The shell that starts the new editor once this one is gone**: `$1` is this process's id, the rest the command to `exec`.
/// It polls every tenth of a second, the interval `update.rs`'s swap waits on.
const TRAMPOLINE_SH: &str = r#"pid="$1"; shift; while kill -0 "$pid" 2>/dev/null; do sleep 0.1; done; exec "$@""#;

/// **Ask for a relaunch on behalf of the agent tab with `token`**: the checks, then the save queued and the relaunch counted down.
pub fn request(world: &mut World, token: &str) -> Result<serde_json::Value, String> {
    let asker = {
        let mut terminal = world.get_resource_mut::<crate::terminal::Terminal>().ok_or("the editor has no terminal")?;
        terminal.agent_with_token_mut(token).map(|post| (post.kind, post.session.clone()))
    };
    let started = world.get_resource::<StartedAt>().ok_or("this editor did not record when it started, so a new build cannot be told from it")?.0;
    let exe = std::env::current_exe().map_err(|e| format!("this editor's own binary cannot be found: {e}"))?;
    let built = std::fs::metadata(&exe).and_then(|meta| meta.modified()).map_err(|e| format!("{}: {e}", exe.display()))?;
    let session = session_to_restore(asker, token, started, built, &exe)?;
    // **Saved the way `Cmd+S` saves**: the command's own chord, queued as an agent's, so the save is the one path a person's press takes.
    let save = crate::commands::by_id("file.save").ok_or("the editor has no file.save command")?;
    let live = world.get_resource::<crate::keys::Live>().copied().ok_or("the editor is not ready for keys yet, so it cannot save")?;
    world
        .get_resource_mut::<crate::command_run::Chords>()
        .ok_or("the editor is not ready for keys yet, so it cannot save")?
        .queue_from(&save, live, crate::ops::Source::Agent);
    world.insert_resource(PendingRelaunch { exe, args: relaunch_args(std::env::args().skip(1).collect(), &session), frames_left: RELAUNCH_AFTER_FRAMES });
    Ok(serde_json::json!({
        "relaunching": true,
        "session": session,
        "note": "this session ends now and is restored by the new editor",
    }))
}

/// **The session a relaunch restores, or why there is none**: the asking tab (its kind and the omp session it said, when a tab has
/// the token) is an editor development agent that has said its session, and the binary at `exe` was built after this editor
/// started at `started`.
fn session_to_restore(
    asker: Option<(AgentKind, Option<String>)>,
    token: &str,
    started: SystemTime,
    built: SystemTime,
    exe: &Path,
) -> Result<String, String> {
    let (kind, session) = asker.ok_or_else(|| format!("no agent tab of this editor has the token {token}"))?;
    if kind != AgentKind::EditorDev {
        return Err("only the editor dev agent relaunches the editor: this tab is the project's agent".to_owned());
    }
    let session = session.ok_or("no omp session recorded for this agent yet")?;
    if built <= started {
        let since = started.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        return Err(format!(
            "no new build: {} is older than this editor (started at unix time {since}); run cargo build -p bevy_dark_factory first",
            exe.display()
        ));
    }
    Ok(session)
}

/// **The arguments the new editor starts with**: this one's, without any earlier `--restore-dev-agent` pair, and the session last.
pub fn relaunch_args(argv: Vec<String>, session: &str) -> Vec<String> {
    let mut out = Vec::with_capacity(argv.len() + 2);
    let mut it = argv.into_iter();
    while let Some(arg) = it.next() {
        if arg == crate::args::RESTORE_DEV_AGENT {
            // The pair's value goes with it.
            it.next();
            continue;
        }
        out.push(arg);
    }
    out.push(crate::args::RESTORE_DEV_AGENT.to_owned());
    out.push(session.to_owned());
    out
}

/// **Carry a pending relaunch out when its frames are up**: start the waiting shell, then quit.
fn relaunch_when_due(pending: Option<ResMut<PendingRelaunch>>, mut exit: MessageWriter<AppExit>, mut commands: Commands) {
    let Some(mut pending) = pending else { return };
    if pending.frames_left > 0 {
        pending.frames_left -= 1;
        return;
    }
    commands.remove_resource::<PendingRelaunch>();
    let started = start_the_trampoline(&pending);
    match started {
        Ok(()) => {
            info!("relaunching on {} {:?}", pending.exe.display(), pending.args);
            exit.write(AppExit::Success);
        }
        Err(why) => error!("the editor could not relaunch, so it keeps running: {why}"),
    }
}

/// Start [`TRAMPOLINE_SH`] in its own process group, so it outlives this process.
#[cfg(unix)]
fn start_the_trampoline(pending: &PendingRelaunch) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;
    std::process::Command::new("/bin/sh")
        .arg("-c")
        .arg(TRAMPOLINE_SH)
        .arg("relaunch")
        .arg(std::process::id().to_string())
        .arg(&pending.exe)
        .args(&pending.args)
        .process_group(0)
        .stdin(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("/bin/sh: {e}"))
}

/// A relaunch waits for this process through `/bin/sh`, which this platform has not got.
#[cfg(not(unix))]
fn start_the_trampoline(_pending: &PendingRelaunch) -> Result<(), String> {
    Err("a relaunch waits for this editor through /bin/sh, which this platform has not got".to_owned())
}

/// **What carries a relaunch out.** Only `main` adds it, as the exit watchdog: a test editor that asks for a relaunch is told it
/// will happen and goes on.
pub struct RelaunchPlugin;

impl Plugin for RelaunchPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, relaunch_when_due);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The new editor gets this one's arguments and the session**, and an earlier relaunch's pair is not carried twice.
    #[test]
    fn a_relaunch_keeps_the_arguments_and_names_the_session_once() {
        let argv = vec!["/game".to_owned(), "--live".to_owned(), "--restore-dev-agent".to_owned(), "old-1".to_owned(), "harbour".to_owned()];
        assert_eq!(relaunch_args(argv, "new-2"), ["/game", "--live", "harbour", "--restore-dev-agent", "new-2"]);
    }

    /// **A relaunch is the editor development agent's, once it has a session, onto a newer binary**; every other ask is refused
    /// naming why, so an agent is never killed for an editor that comes back the same.
    #[test]
    fn a_relaunch_is_refused_for_another_tab_no_session_or_no_new_build() {
        let started = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000);
        let newer = started + std::time::Duration::from_secs(1);
        let exe = Path::new("/bin/editor");
        let dev = |session: Option<&str>| Some((AgentKind::EditorDev, session.map(str::to_owned)));

        assert_eq!(session_to_restore(dev(Some("s-1")), "t", started, newer, exe), Ok("s-1".to_owned()));
        let refused = |asker, built| session_to_restore(asker, "t", started, built, exe).err().unwrap_or_default();
        assert!(refused(None, newer).contains("no agent tab of this editor has the token t"));
        assert!(refused(Some((AgentKind::Project, Some("s-1".to_owned()))), newer).contains("only the editor dev agent"));
        assert!(refused(dev(None), newer).contains("no omp session recorded"));
        let same = refused(dev(Some("s-1")), started);
        assert!(same.starts_with("no new build: /bin/editor"), "a binary as old as the editor is not new: {same}");
    }
}
