# The Rust server behind the Files editor

**Issues #611 and #698.** What the project's rust-analyzer adds to a Rust file in the Files workspace, what the editor's
own tokenizer keeps doing with or without it, and how a test measures both without rust-analyzer installed. The code is
`crates/bevy_dark_factory/src/files_lsp.rs`; its module comment is the reference, and this page is the overview.

## One server, per project

- **One program, `rust-analyzer`, for `.rs` files only** (owner, 2026-11-27). It is started once per project, from the
  project's root, when the root holds a `Cargo.toml` and a Rust file is open; it is killed and reaped when the project
  closes or another opens.
- **Off the main thread.** Three threads own its pipes (frames in, frames out, the last line of its error stream); the
  frame drains their channel once per frame in `PreUpdate` and never waits on the server.
- **Absent is said once.** A machine without it gets one status problem naming the install command
  (`rustup component add rust-analyzer`); a server that stops is said once with the last thing it wrote, and is not
  restarted for that project.

## What it adds

| The person does | The editor sends | What comes back, and where it is drawn |
| --- | --- | --- |
| opens a Rust file | `textDocument/didOpen` | findings (`publishDiagnostics`) join the editor's own on their lines |
| types | `textDocument/didChange`, the one edit that makes the change (incremental sync; the whole text to a server that does not take edits) | new findings when the server publishes |
| saves | `textDocument/didSave`, when the server asked for saves | rust-analyzer runs `cargo check` again, and publishes what it found |
| asks for completion (typing a letter, or Ctrl+Space) | `textDocument/completion` at the caret | the editor's completion list at the start of the word, in the server's order |
| rests the pointer on code | `textDocument/hover`, asked as plain text | the note over the bytes the server named |
| closes the file | `textDocument/didClose` | its findings are dropped |

**An answer arrives on a later frame**, and is put on the editor only while it still shows the file and the text
revision it asked about; an answer about text that has moved on is dropped. A newer question cancels the older one of
its kind from the same editor (`$/cancelRequest`), so a fast typist leaves no queue behind.

**Completion items the list cannot apply as the server means them are left out**: an item whose edit starts before the
word (rust-analyzer's postfix rewrites, which replace the expression), an item that brings edits elsewhere (an
auto-import), and a snippet. The editor's list writes one text over the word from its start to the caret, so these
would write something other than what the server offered.

## What the tokenizer keeps

The tokenizer (`files_lang`) colours, folds and outlines every file, a Rust file included, **whether a server runs or
not**: that is the baseline, not a fallback. It knows Rust's comments and nothing else, so it offers nothing to
complete and nothing to say on hover in a Rust file: without a server, a Rust file opens no completion list and no
note, and keeps its colours, its folds and the editor's own findings. With a server, the server's findings are drawn
beside the editor's own, never instead of them.

## How it is measured

The server is started from one setting, the program `RustAnalyzer::with_program` names. A test names
`dark_factory-lsp-stub` there (`crates/bevy_dark_factory/src/bin/dark_factory-lsp-stub.rs`), a fixed peer that answers
completion and hover with answers the test knows and publishes, after every open, change and save, a finding that says
its version, the saves it was told of and the whole text it holds. It goes through the same spawn, threads and frames
as rust-analyzer, so **no test skips when rust-analyzer is absent**:

- `tests/a_rust_files_completion_and_hover_come_from_its_server.rs`: a typed letter opens the server's items at the
  caret, Enter writes the server's edit, the hover note is the server's text over its range, edits reach the server as
  edits, and a save is told.
- `tests/a_rust_files_compiler_errors_are_marked_by_rust_analyzer.rs`: the real rust-analyzer marks a type error on its
  line (this one needs rust-analyzer, and fails naming the install command without it), and an absent server is said
  once while the file stays coloured and editable.
