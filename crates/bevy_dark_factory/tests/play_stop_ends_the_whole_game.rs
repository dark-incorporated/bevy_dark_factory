//! **`play_stop` ends the game, not only the process `play` started** (#97).
//!
//! A game is usually started through a launcher: `play.yaml`'s own example is `cargo run -- {map}`.
//! Killing only the launcher's pid left the game running with no parent, still answering on its
//! port. This test's launcher is `sh -c "… & wait"`. (Correction, 2026-10-30: the orphaned
//! `game.py` processes found on bmb were not this. `play_starts_and_stops_the_game` panicked
//! before `play_stop`, because Homebrew's Python 3.15 fixture server never answered; see #121.)

use bevy_dark_factory::ops::{self, Applied, Ctx};
use bevy_dark_factory::ops_play::fairy_ready;
use serde_json::json;
use std::time::{Duration, Instant};

use crate::fixtures;

/// The game: answers `bevy_fairy/status` with `ready: true` on the port in its first argument.
const GAME: &str = r#"
import http.server, json, sys
class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers.get("content-length", 0)))
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"ready": True}}).encode()
        self.send_response(200); self.send_header("content-length", str(len(body))); self.end_headers(); self.wfile.write(body)
    def log_message(self, *a): pass
http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), H).serve_forever()
"#;

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()
}

#[test]
fn stop_ends_a_game_started_through_a_launcher() {
    stop_ends_the_game_behind_a_launcher("play_launcher");
}

/// **`play_stop` ends the group itself, whatever `kill` is on `PATH`** (#697). It used to run the `kill` program,
/// and procps-ng 4.0.4's — Ubuntu 24.04's `/usr/bin/kill` — signals minus the group's *first digit*: the game
/// behind the launcher kept answering, and a group id starting with `1` was `kill(-1)`, every process the person
/// owns. A `kill` that does nothing stands first on `PATH` here, so a stop that goes back to the program fails on
/// every host, not only on one whose `kill` misreads it.
#[test]
fn stop_ends_the_game_whatever_kill_is_on_the_path() {
    let bin = std::env::temp_dir().join(format!("blueprint-697-kill-{}", std::process::id()));
    std::fs::create_dir_all(&bin).unwrap_or_else(|e| panic!("{}: {e}", bin.display()));
    let kill = bin.join("kill");
    std::fs::write(&kill, "#!/bin/sh\nexit 0\n").unwrap_or_else(|e| panic!("{}: {e}", kill.display()));
    std::fs::set_permissions(&kill, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap_or_else(|e| panic!("{}: {e}", kill.display()));
    let _path = PathFirst::new(&bin);
    stop_ends_the_game_behind_a_launcher("play_launcher_path");
    let _ = std::fs::remove_dir_all(&bin);
}

/// **`PATH` with `dir` in front of it, put back when dropped.** `PATH` is process-wide, and nextest runs each test
/// in a process of its own, so only this test sees the change.
struct PathFirst(Option<std::ffi::OsString>);

impl PathFirst {
    fn new(dir: &std::path::Path) -> Self {
        let old = std::env::var_os("PATH");
        let dirs = std::iter::once(dir.to_path_buf()).chain(old.iter().flat_map(std::env::split_paths));
        let path = std::env::join_paths(dirs).unwrap_or_else(|e| panic!("{e}"));
        // SAFETY: a test's own process; see the type's doc.
        unsafe { std::env::set_var("PATH", path) };
        PathFirst(old)
    }
}

impl Drop for PathFirst {
    fn drop(&mut self) {
        // SAFETY: as in `new`.
        match &self.0 {
            Some(old) => unsafe { std::env::set_var("PATH", old) },
            None => unsafe { std::env::remove_var("PATH") },
        }
    }
}

/// Plays a game started through a launcher, stops it, and asserts the game is gone with the launcher.
fn stop_ends_the_game_behind_a_launcher(tag: &str) {
    let port = free_port();
    let root = fixtures::Fixture::new(&format!("{tag}{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("town");
    std::fs::write(root.join("game.py"), GAME).unwrap();
    // A launcher that starts the game as its child and waits on it, as `cargo run` does.
    std::fs::write(
        root.join("play.yaml"),
        format!("version: 1\ncommand: [\"sh\", \"-c\", \"{} game.py {port} & wait\"]\nport: {port}\n", fixtures_python()),
    )
    .unwrap();
    let trust_file = Some(fixtures::trusted_store(&root));
    let mut c = Ctx { root: Some(root), session: "test".to_owned(), trust_file, ..Ctx::default() };

    let Applied::Done { outcome, .. } =
        ops::dispatch(&mut c, "play", &json!({"map": "town", "wait_s": 20.0})).unwrap()
    else {
        panic!("play was refused")
    };
    let after = outcome.after.unwrap();
    let _cleanup = crate::play_starts_and_stops_the_game::KillGroupOnDrop(after["pid"].as_u64().unwrap());
    assert_eq!(after["ready"], true, "the game behind the launcher answered: {}", outcome.summary);

    let Applied::Done { .. } = ops::dispatch(&mut c, "play_stop", &json!({})).unwrap() else { panic!("stop refused") };
    // **A wait on the thing asserted**: the port is asked until it stops answering, for at most five seconds, one step at a
    // time; the step is how often it is asked, not how long the test waits.
    const ASK_EVERY: Duration = Duration::from_millis(20);
    let deadline = Instant::now() + Duration::from_secs(5);
    while fairy_ready(port) && Instant::now() < deadline {
        std::thread::sleep(ASK_EVERY);
    }
    assert!(
        !fairy_ready(port),
        "the game still answers on port {port} after play_stop: the launcher died and its child, the game, did not"
    );
}

/// The interpreter the fixture game runs on: the system's `/usr/bin/python3` where there is one.
/// Homebrew's Python 3.15.2 on bmb never answered the fixture's POST (#121).
fn fixtures_python() -> &'static str {
    if std::path::Path::new("/usr/bin/python3").exists() { "/usr/bin/python3" } else { "python3" }
}
