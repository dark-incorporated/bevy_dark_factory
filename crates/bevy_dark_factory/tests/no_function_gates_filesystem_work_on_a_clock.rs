//! **No function gates filesystem work on a clock** (#538): the ratchet that reads what a function DOES, not what its constant is called.
//!
//! `no_module_polls_the_project_on_its_own_clock.rs` matches the NAMES of poll constants (`POLL_S`, `*READ_SECS`, `*LOOK_EVERY*`...). #501's
//! last pieces were found by reading code, because they were called `WATCH_SECS`, `REFRESH_S` and `COLLECTION_READOUT_EVERY`, and a new poll with a
//! new name passes a name matcher, which is the thing it was written to stop. This test reads every function in every workspace crate's `src/` and
//! **fails on one whose body (signature included) both uses a clock and touches the filesystem**: that is a poll, whatever it is called.
//!
//! # What is a clock, and what is the filesystem
//!
//! - **A clock**: `delta_secs`, `elapsed_secs` (and the `_f64` forms), `Instant`, `Timer`, or a `Local<f32>`/`Local<f64>` (the since-accumulator
//!   a poll keeps between frames).
//! - **The filesystem**: `read_dir`, `metadata` (and `symlink_metadata`), `exists`, `is_file`, `is_dir`, `read_to_string`, `fs::read`, `modified`,
//!   and the named helpers that do it for a caller: `Tree::read`, `areas_of_level`, `fingerprint`, `usage`.
//!
//! # How it reads
//!
//! **A line-and-brace scan, not a parser**: comments and the insides of strings and character literals are blanked, `#[cfg(test)]` modules and
//! `#[test]` functions are skipped, and a function's body is the text between the braces that follow its signature. A nested function or closure
//! belongs to the function that holds it, and is also read by itself if it is a `fn`. **It cannot see a clock and a read split across two
//! functions** (a system that ticks a timer and calls a helper that reads): the helper is named in the list above when it is one of ours
//! (`Tree::read`, `areas_of_level`, `fingerprint`, `usage`), and a new one is not seen until it is added there. It is a ratchet, not a proof.
//!
//! # The exceptions, each with its reason
//!
//! [`EXCEPTIONS`] is explicit and commented. A function on it is allowed; a function that is not and that the scan flags fails the test. **A stale
//! entry never fails it** (a function that was retired, renamed or no longer both uses a clock and reads), so a worker who migrates one does not have
//! to edit this file.

use std::path::{Path, PathBuf};

/// **A function the scan may flag, the file it is in, and why it is allowed.** The file is a path under the workspace's `crates/` folder.
type Exception = (&'static str, &'static str, &'static str);

/// **The functions that gate filesystem work on a clock on purpose**, each with a sentence.
const EXCEPTIONS: &[Exception] = &[
    // #501's two recorded exceptions (items 13 and 17 of the checklist). `STATE_POLL_S` reads a running process over a socket and touches no
    // file in the function the scan reads (`poll_the_game`), so it is not flagged; the log's two are.
    ("bevy_dark_factory/src/game_view.rs", "read_the_log", "the game's log is in the temporary directory, outside the project root, and a measured index would show a streamed log every two or three seconds, not every 0.25 s (#501 item 17)"),
    ("bevy_dark_factory/src/game_view.rs", "watch_the_game", "`LOG_CAP_CHECK_S`: the stat of the log's size that keeps it under its cap while the game runs (in `ops_play::enforce_log_cap`, which this scan cannot see into, so the entry is stale until it can); it belongs to the log (#501 item 17)"),
    // **Not polls of a file**: a clock and a file in one function, and the clock does not gate the file.
    ("bevy_dark_factory/src/editor.rs", "refresh_notes_list", "the 0.5 s tick compares the in-memory notes (which `notes_view` follows through the index) with what the list drew and returns when they are the same; the `fs::read` is of a resolved note's capture pictures, only when the list is rebuilt"),
    ("bevy_dark_factory/src/ops_brp.rs", "editor_lesson", "`elapsed_secs_f64` timestamps the agent's activity for the night-shift row; the `exists` is a check of the guide file for the lesson; the clock gates nothing"),
    ("bevy_dark_factory/src/settings.rs", "ask_the_login_shell", "`Instant` is the deadline of one question to a login shell (a child process whose output is read on a thread); nothing is looked at again on a clock"),
    // **Another process, not the editor's UI thread**: the benchmark harness and the MCP server run beside the editor.
    ("bevy_dark_factory_bench/src/run.rs", "start", "waits, with a deadline, for a child `Xvfb` to make its socket and lock file; a test harness's wait on a process, not a poll of a project's files"),
    ("bevy_dark_factory_bench/src/run.rs", "one_trial", "`Instant` times one benchmark trial; the files it reads and writes are the trial's own record, not watched"),
    ("dark_factory_mcp/src/main.rs", "capture_sequence", "`Instant` is the deadline for the editor's answer; the frames are read once, after it, and nothing is looked at again on a clock"),
];

const CLOCK_WORDS: &[&str] = &["delta_secs", "delta_secs_f64", "elapsed_secs", "elapsed_secs_f64", "Instant", "Timer"];
const CLOCK_PHRASES: &[&str] = &["Local<f32>", "Local<f64>"];
const FS_WORDS: &[&str] = &["read_dir", "metadata", "symlink_metadata", "exists", "is_file", "is_dir", "read_to_string", "modified", "areas_of_level", "fingerprint", "usage"];
const FS_PHRASES: &[&str] = &["fs::read(", "Tree::read("];

/// One function as the scan reads it: where it is and the text from its `fn` to its closing brace.
struct Func {
    name: String,
    line: usize,
    text: String,
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// **The source with comments and the insides of strings and character literals blanked** (newlines kept, so a line number survives).
fn blank(source: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut i = 0;
    let keep = |c: char| if c == '\n' { '\n' } else { ' ' };
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if c == '/' && next == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                out.push(' ');
                i += 1;
            }
        } else if c == '/' && next == Some('*') {
            let mut depth = 0;
            while i < chars.len() {
                if chars[i] == '/' && chars.get(i + 1) == Some(&'*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if chars[i] == '*' && chars.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(keep(chars[i]));
                    i += 1;
                }
            }
        } else if c == 'r' && matches!(next, Some('"' | '#')) && (i == 0 || !is_ident(chars[i - 1])) {
            // A raw string: r"..." or r#"..."#.
            let mut j = i + 1;
            let mut hashes = 0;
            while chars.get(j) == Some(&'#') {
                hashes += 1;
                j += 1;
            }
            if chars.get(j) != Some(&'"') {
                out.push(c);
                i += 1;
                continue;
            }
            out.push_str(&" ".repeat(j - i + 1));
            i = j + 1;
            while i < chars.len() {
                if chars[i] == '"' && (0..hashes).all(|k| chars.get(i + 1 + k) == Some(&'#')) {
                    out.push_str(&" ".repeat(hashes + 1));
                    i += hashes + 1;
                    break;
                }
                out.push(keep(chars[i]));
                i += 1;
            }
        } else if c == '"' {
            out.push(' ');
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    out.push(' ');
                    out.push(keep(chars.get(i + 1).copied().unwrap_or(' ')));
                    i += 2;
                } else if chars[i] == '"' {
                    out.push(' ');
                    i += 1;
                    break;
                } else {
                    out.push(keep(chars[i]));
                    i += 1;
                }
            }
        } else if c == '\'' {
            // A character literal ('a', '\n', '{') or a lifetime ('a): a literal closes within a few characters.
            let literal_end = if next == Some('\\') {
                chars.iter().skip(i + 2).take(8).position(|&x| x == '\'').map(|p| i + 2 + p)
            } else if chars.get(i + 2) == Some(&'\'') {
                Some(i + 2)
            } else {
                None
            };
            match literal_end {
                Some(end) => {
                    out.push_str(&" ".repeat(end - i + 1));
                    i = end + 1;
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            }
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// The index just past the brace that closes the one at `open`, or the end of the text.
fn matching_brace(text: &[char], open: usize) -> usize {
    let mut depth = 0;
    for (i, &c) in text.iter().enumerate().skip(open) {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
    }
    text.len()
}

/// **Blank every `mod tests { ... }` and every `#[test]` function**, so test code that times and reads is not a poll.
fn without_tests(blanked: &str) -> String {
    let mut chars: Vec<char> = blanked.chars().collect();
    let text: String = chars.iter().collect();
    let mut starts: Vec<usize> = Vec::new();
    for (at, _) in text.match_indices("mod tests") {
        starts.push(at);
    }
    for (at, _) in text.match_indices("#[test]") {
        starts.push(at);
    }
    // `match_indices` gives byte offsets; the text is mostly ASCII, but a character index is what the blanking needs.
    let char_at = |byte: usize| text[..byte].chars().count();
    for byte in starts {
        let at = char_at(byte);
        if let Some(open) = chars.iter().enumerate().skip(at).find(|(_, c)| **c == '{').map(|(i, _)| i) {
            let end = matching_brace(&chars, open);
            for c in &mut chars[at..end] {
                if *c != '\n' {
                    *c = ' ';
                }
            }
        }
    }
    chars.into_iter().collect()
}

/// **Every `fn` with a body in `source`**, nested ones included.
fn functions(source: &str) -> Vec<Func> {
    let blanked = without_tests(&blank(source));
    let chars: Vec<char> = blanked.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 < chars.len() {
        let starts_word = i == 0 || !is_ident(chars[i - 1]);
        if starts_word && chars[i] == 'f' && chars[i + 1] == 'n' && chars[i + 2] == ' ' {
            let mut j = i + 3;
            while chars.get(j) == Some(&' ') {
                j += 1;
            }
            let name_start = j;
            while chars.get(j).is_some_and(|c| is_ident(*c)) {
                j += 1;
            }
            let name: String = chars[name_start..j].iter().collect();
            // The body opens at the first `{` outside parentheses and angle brackets that comes before a `;`.
            let (mut paren, mut open) = (0i32, None);
            for (k, &c) in chars.iter().enumerate().skip(j) {
                match c {
                    '(' | '[' => paren += 1,
                    ')' | ']' => paren -= 1,
                    ';' if paren == 0 => break,
                    '{' if paren == 0 => {
                        open = Some(k);
                        break;
                    }
                    _ => {}
                }
            }
            if let (Some(open), false) = (open, name.is_empty()) {
                let end = matching_brace(&chars, open);
                let line = chars[..i].iter().filter(|c| **c == '\n').count() + 1;
                out.push(Func { name, line, text: chars[i..end].iter().collect() });
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    out
}

fn has_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + word.len()..].chars().next();
        !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
    })
}

/// **Whether a function's text uses a clock**, and whether it touches the filesystem: the two halves of a poll.
fn gates(text: &str) -> (Vec<&'static str>, Vec<&'static str>) {
    let clock = CLOCK_WORDS.iter().filter(|w| has_word(text, w)).chain(CLOCK_PHRASES.iter().filter(|p| text.contains(**p))).copied().collect();
    let fs = FS_WORDS.iter().filter(|w| has_word(text, w)).chain(FS_PHRASES.iter().filter(|p| text.contains(**p))).copied().collect();
    (clock, fs)
}

/// Every `.rs` file under `dir`.
fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// **Every function that uses a clock and touches the filesystem under `crates/*/src/`**, as `(file under crates/, function, line, clock, fs)`.
fn flagged(crates: &Path) -> Vec<(String, String, usize, Vec<&'static str>, Vec<&'static str>)> {
    let mut found = Vec::new();
    let mut members: Vec<PathBuf> = std::fs::read_dir(crates).unwrap_or_else(|e| panic!("{}: {e}", crates.display())).flatten().map(|e| e.path()).collect();
    members.sort();
    for member in members {
        let mut files = Vec::new();
        sources(&member.join("src"), &mut files);
        files.sort();
        for path in files {
            let source = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let rel = path.strip_prefix(crates).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            for func in functions(&source) {
                let (clock, fs) = gates(&func.text);
                if !clock.is_empty() && !fs.is_empty() {
                    found.push((rel.clone(), func.name, func.line, clock, fs));
                }
            }
        }
    }
    found
}

fn allowed(file: &str, name: &str) -> bool {
    EXCEPTIONS.iter().any(|(f, n, _)| *f == file && *n == name)
}

/// **No new function that uses a clock and touches the filesystem**: every one the scan finds is on the list.
#[test]
fn no_function_gates_filesystem_work_on_a_clock() {
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let found = flagged(&crates);
    for (file, name, line, clock, fs) in &found {
        println!("{file}:{line} fn {name}: clock {clock:?}, filesystem {fs:?}{}", if allowed(file, name) { "  [excepted]" } else { "" });
    }
    let new: Vec<String> = found
        .iter()
        .filter(|(file, name, ..)| !allowed(file, name))
        .map(|(file, name, line, clock, fs)| format!("  crates/{file}:{line}: `{name}` uses {clock:?} and touches the filesystem ({fs:?})"))
        .collect();
    assert!(
        new.is_empty(),
        "A function gates filesystem work on a clock, which is a poll whatever it is called:\n{}\n\nA reader of a project file subscribes to the project index \
         (`project_index::ProjectIndex`, or `OutsideFiles` for a folder outside the project; see `docs/guide/reload-contract.md`, \"The one watcher\"); it does \
         not keep its own timer. If this one is not a poll, add it to EXCEPTIONS with the reason in a sentence.",
        new.join("\n")
    );
}

/// **The scan can fail**: a function with a NEW name that polls is flagged, one that does not is not, and the pieces it is made of work.
#[test]
fn the_scan_flags_a_poll_with_a_new_name_and_only_a_poll() {
    // A poll nothing in the name matcher of the old ratchet would see: no constant, a name that is nobody's, a timer in a `Local`.
    let poll = r#"
        fn look_at_the_notes_now_and_then(time: Res<Time>, mut since: Local<f32>) {
            *since += time.delta_secs();
            if *since < 0.7 { return; }
            *since = 0.0;
            let _ = std::fs::read_to_string("notes.yaml");
        }
        fn a_pure_helper(a: u32) -> u32 { a + 1 }
        fn reads_without_a_clock() { let _ = std::fs::metadata("x"); }
        fn clock_without_a_read(time: Res<Time>) { let _ = time.delta_secs(); }
        /// fn in_a_comment() { let _ = std::fs::metadata("x"); let _ = Instant::now(); }
        fn in_a_string() { let _ = "std::fs::metadata Instant::now()"; }
        #[test]
        fn a_test_that_times_and_reads() { let _ = Instant::now(); let _ = std::fs::metadata("x"); }
        mod tests {
            fn helper() { let _ = Instant::now(); let _ = std::fs::metadata("x"); }
        }
        fn a_char_brace() -> char { '{' }
        fn after_the_brace(time: Res<Time>) { let _ = (time.delta_secs(), std::fs::metadata("y")); }
    "#;
    let mut names: Vec<String> = functions(poll)
        .into_iter()
        .filter(|f| {
            let (clock, fs) = gates(&f.text);
            !clock.is_empty() && !fs.is_empty()
        })
        .map(|f| f.name)
        .collect();
    names.sort();
    assert_eq!(names, ["after_the_brace", "look_at_the_notes_now_and_then"], "the poll with a new name is flagged, and nothing else is");

    // **The helpers that read for a caller are seen by name**: a timer and `fingerprint(..)` in one function is a poll.
    let helper = "fn follow(time: Res<Time>) { let _ = time.delta_secs(); let _ = fingerprint(root); }";
    let found = functions(helper);
    let (clock, fs) = gates(&found[0].text);
    assert!(!clock.is_empty() && fs == ["fingerprint"], "{clock:?} {fs:?}");

    // **A seeded poll goes red**: `no_function_gates_filesystem_work_on_a_clock` fails on a file that holds the function above and no exception.
    let dir = std::env::temp_dir().join(format!("no_function_gates_{}", std::process::id()));
    std::fs::create_dir_all(dir.join("seeded/src")).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(dir.join("seeded/src/lib.rs"), poll).unwrap_or_else(|e| panic!("{e}"));
    let seeded = flagged(&dir);
    std::fs::remove_dir_all(&dir).unwrap_or_else(|e| panic!("{e}"));
    let new: Vec<&str> = seeded.iter().filter(|(file, name, ..)| !allowed(file, name)).map(|(_, name, ..)| name.as_str()).collect();
    assert_eq!(new, ["look_at_the_notes_now_and_then", "after_the_brace"], "a fixture crate with a polling function is not excepted, so the ratchet would be red on it: {seeded:?}");
}
