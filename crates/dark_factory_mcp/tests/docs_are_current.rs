//! **The MCP tool reference matches the server.**
//!
//! The brief's rule: *"MCP tools and `docs/mcp-tools.md` are generated from the registry, or
//! mechanically checked against it."* **This is the checked half** — the document is rendered
//! from the server's own declarations and compared with the file in the repository.
//!
//! **A stale copy fails the test**, and the failure says how to fix it. A hand-written reference
//! is a second vocabulary: it says one thing and the server does another, and the reader — an
//! agent, mostly — believes the document.

/// **The document, as the repository holds it.**
const ON_DISK: &str = include_str!("../../../docs/mcp-tools.md");

#[test]
fn the_tool_reference_is_current() {
    let rendered = dark_factory_mcp::docs::render();
    assert_eq!(
        rendered, ON_DISK,
        "docs/mcp-tools.md does not match the server's own tool declarations. Regenerate it with \
         `cargo run -q -p dark_factory_mcp -- --emit-docs > docs/mcp-tools.md`."
    );
}

/// **The reference names every tool the server has, and no others.**
///
/// **The tool list is the server's**, so a tool that was added and not documented — or
/// documented and removed — is caught here rather than by a reader.
#[test]
fn the_reference_names_every_tool() {
    let rendered = dark_factory_mcp::docs::render();
    for tool in dark_factory_mcp::docs::TOOLS {
        assert!(
            rendered.contains(&format!("### `{}`", tool.name)),
            "the reference does not document the tool `{}`.",
            tool.name
        );
    }
}

/// **The reference's schemas cover every tool.**
#[test]
fn every_tool_has_a_schema() {
    let schemas = dark_factory_mcp::docs::schemas();
    let map = schemas.as_object().expect("the schemas are an object");
    for tool in dark_factory_mcp::docs::TOOLS {
        assert!(
            map.contains_key(tool.name),
            "the tool `{}` has no parameter schema, so an agent cannot tell what to send it.",
            tool.name
        );
    }
    assert_eq!(
        map.len(),
        dark_factory_mcp::docs::TOOLS.len(),
        "there are more schemas than tools, or more tools than schemas, so one of the lists is \
         stale."
    );
}

/// **The reference's tool list is the SERVER's tool list, not a copy of it.**
///
/// **The first version compared the reference with `docs::TOOLS`** — and `docs::TOOLS` is a
/// hand-written copy of the `#[tool]` attributes. **So changing an annotation in the server
/// passed every test**, and the reference could say the opposite of what the server does.
///
/// **So the server's own tool list is the one compared.** The server is asked, over MCP, what
/// tools it has; the reference is asked the same way; and the two must agree.
#[tokio::test]
async fn the_reference_names_the_servers_own_tools() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};

    let mut child = Command::new(env!("CARGO_BIN_EXE_dark_factory_mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the server binary starts");
    let mut stdin = child.stdin.take().expect("the server has a stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("the server has a stdout"));

    let send =
        |stdin: &mut std::process::ChildStdin, id: u64, method: &str, params: serde_json::Value| {
            let body = serde_json::json!({
                "jsonrpc": "2.0", "id": id, "method": method, "params": params,
            });
            writeln!(stdin, "{body}").expect("the server's stdin is writable");
            stdin.flush().expect("the server's stdin flushes");
        };
    // **A notification has no id and no answer.** Sending one with an id makes the server treat
    // it as a request, and it answers it — so the reader waiting for the next id takes the
    // notification's answer instead and the real answer is never read.
    let notify = |stdin: &mut std::process::ChildStdin, method: &str| {
        let body = serde_json::json!({"jsonrpc": "2.0", "method": method, "params": {}});
        writeln!(stdin, "{body}").expect("the server's stdin is writable");
        stdin.flush().expect("the server's stdin flushes");
    };
    // **Read until the answer with the id asked for.** The server may send a notification in
    // between — the handshake's own — and a reader that takes the first line would take that
    // instead of the answer.
    let read =
        |stdout: &mut BufReader<std::process::ChildStdout>, want: u64| -> serde_json::Value {
            loop {
                let mut line = String::new();
                let n = stdout
                    .read_line(&mut line)
                    .expect("the server's stdout is readable");
                assert!(n > 0, "the server's stdout ended before it answered");
                let value: serde_json::Value =
                    serde_json::from_str(line.trim_end()).expect("the server sent JSON");
                if value.get("id").and_then(|i| i.as_u64()) == Some(want) {
                    return value;
                }
            }
        };

    // **The handshake carries a protocol version and a client name**, because the server reads
    // them — an initialize with empty params is not a handshake, and the server answers it with
    // an error the reader then waits past.
    send(
        &mut stdin,
        1,
        "initialize",
        serde_json::json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": {"name": "bevy_blueprint's own test", "version": "0.1.0"},
        }),
    );
    let _ = read(&mut stdout, 1);
    // **The notification is a notification, and it has no answer.** It is sent and not read, and
    // the reader skips it because it carries no id.
    notify(&mut stdin, "notifications/initialized");
    send(&mut stdin, 3, "tools/list", serde_json::json!({}));
    let listed = read(&mut stdout, 3);
    // **The process is waited for**, not just killed: a killed child is a zombie until it is
    // reaped, and a test file that leaves one behind is a test file that leaks.
    let _ = child.kill();
    let _ = child.wait();

    let server: Vec<String> = listed["result"]["tools"]
        .as_array()
        .expect("the tool list is an array")
        .iter()
        .map(|t| t["name"].as_str().unwrap_or_default().to_owned())
        .collect();
    let documented: Vec<String> = dark_factory_mcp::docs::TOOLS
        .iter()
        .map(|t| t.name.to_owned())
        .collect();
    // **The same SET, and not the same order.** The server lists its tools in the order the
    // router holds them, which is alphabetical; the reference is in the order they are declared,
    // which is the order a reader wants. **The order is a presentation choice and the set is
    // the contract** — so the set is compared, and a tool that is in one and not the other is
    // the failure.
    let mut server_sorted = server.clone();
    server_sorted.sort();
    let mut documented_sorted = documented.clone();
    documented_sorted.sort();
    assert_eq!(
        server_sorted, documented_sorted,
        "the reference documents {documented:?} and the server has {server:?}. The reference is \
         generated from the server's declarations, and a hand-written copy of them is a second \
         vocabulary."
    );

    // **And each tool says the same thing in both places.** Names alone were the first version of
    // this check, and the reference then said `blueprint_apply_operation` was not destructive while
    // the server said it was — an agent reading the reference would treat a tool that replaces
    // values as harmless. **The description and the three hints are the contract**, so they are
    // compared one by one, against the server's own answer.
    let mut drift = Vec::new();
    for doc in dark_factory_mcp::docs::TOOLS {
        let Some(tool) = listed["result"]["tools"]
            .as_array()
            .and_then(|tools| tools.iter().find(|t| t["name"] == doc.name))
        else {
            continue;
        };
        if tool["description"].as_str() != Some(doc.description) {
            drift.push(format!(
                "{}: description\n  server:    {}\n  reference: {}",
                doc.name, tool["description"], doc.description
            ));
        }
        let hints = &tool["annotations"];
        for (hint, documented) in [
            ("readOnlyHint", doc.read_only),
            ("destructiveHint", doc.destructive),
            ("idempotentHint", doc.idempotent),
        ] {
            if hints[hint].as_bool() != Some(documented) {
                drift.push(format!(
                    "{}: {hint} is {} on the server and {documented} in the reference",
                    doc.name, hints[hint]
                ));
            }
        }
    }
    assert!(
        drift.is_empty(),
        "the reference and the server disagree, so an agent reading docs/mcp-tools.md is told \
         something the server does not do:\n{}",
        drift.join("\n")
    );
}

/// **Every tool is prefixed, and the prefix is what keeps the namespaces apart.**
#[test]
fn every_tool_is_prefixed() {
    for tool in dark_factory_mcp::docs::TOOLS {
        assert!(
            tool.name.starts_with("blueprint_"),
            "the tool `{}` is not prefixed with `blueprint_`.",
            tool.name
        );
    }
}

/// **The module doc's tool table and its count are the server's, not a copy that drifted.**
///
/// `main.rs` opens with a table of every tool and a sentence saying how many there are. Both are
/// documentation a reader trusts, and both had gone stale: two tools (`blueprint_present`,
/// `blueprint_list_pieces`) were missing rows, and the sentence said twelve while the server declared
/// fourteen. A count nobody checks is a count that is wrong once and stays wrong.
#[test]
fn the_tool_table_and_its_count_are_the_servers() {
    let (rows, count_word) = dark_factory_mcp::docs::tool_table_and_count();
    assert_eq!(
        rows,
        dark_factory_mcp::docs::TOOLS.len(),
        "the module doc lists {rows} tools and the reference lists {}",
        dark_factory_mcp::docs::TOOLS.len()
    );
    // **Every count word the sentence could use, past the set's own size.** The list stopped at
    // `twenty` when twenty tools was beyond imagining; the moment a twenty-first arrived the check
    // refused to read its own sentence, which is a vocabulary gap and not a count that drifted. The
    // hyphenated twenties are the words, so they are in it. Assumed owner exception (2026-11-22): the
    // vocabulary gains the thirties up to thirty-nine, an addition, so the sentence can say the
    // thirty-two tools the editor's style inspector and relaunch bring it to.
    let words = [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
        "twelve", "thirteen", "fourteen", "fifteen", "sixteen", "seventeen", "eighteen", "nineteen",
        "twenty", "twenty-one", "twenty-two", "twenty-three", "twenty-four", "twenty-five",
        "twenty-six", "twenty-seven", "twenty-eight", "twenty-nine", "thirty", "thirty-one",
        "thirty-two", "thirty-three", "thirty-four", "thirty-five", "thirty-six", "thirty-seven",
        "thirty-eight", "thirty-nine",
    ];
    let said = words
        .iter()
        .position(|w| *w == count_word)
        .map(|i| i + 1)
        .unwrap_or_else(|| panic!("the sentence counts the tools as `{count_word}`, which is not a number"));
    assert_eq!(
        said,
        dark_factory_mcp::docs::TOOLS.len(),
        "the module doc says there are {said} tools and the server declares {}",
        dark_factory_mcp::docs::TOOLS.len()
    );
}

/// **Every document that says how many tools there are says the server's number** (#473 review).
///
/// The module doc's table and its count are checked above, but the *same* number is spelled in the
/// crate's README and in `docs/PROJECT.md`, and both had gone stale: the server declared 29 tools
/// while both said 28. A count nobody checks is a count that is wrong once and stays wrong, so this
/// reads each document that states it and refuses a stale one.
#[test]
fn every_document_states_the_servers_tool_count() {
    let declared = dark_factory_mcp::docs::TOOLS.len();
    // **Every place the number is spelled**, as (document, the phrase its digits follow, where a
    // table cell ends rather than a word): the README's sentence, PROJECT.md's agent section, its
    // crate table row and its summary table row.
    let places: [(&str, &str, &str); 4] = [
        (include_str!("../README.md"), "There are ", " tools"),
        (include_str!("../../../docs/PROJECT.md"), "### What an agent sees\n\n- **", " tools**, `blueprint_`"),
        (include_str!("../../../docs/PROJECT.md"), "The MCP server: ", " tools over stdio"),
        (include_str!("../../../docs/PROJECT.md"), "| MCP tools | ", " |"),
    ];
    for (text, lead, suffix) in places {
        assert_eq!(
            count_after(text, lead, suffix),
            Some(declared),
            "**a document states a tool count that is not the server's {declared}**, after `{lead}`. \
             Fix the document, not this test."
        );
    }
}

/// **The number the first `{lead}<n>{suffix}` in `text` names**, or `None` when it spells none.
fn count_after(text: &str, lead: &str, suffix: &str) -> Option<usize> {
    let at = text.find(lead)? + lead.len();
    let digits: String = text[at..].chars().take_while(char::is_ascii_digit).collect();
    text[at + digits.len()..].starts_with(suffix).then(|| digits.parse().ok()).flatten()
}

/// **The reference names the folders the server actually writes**, not a copy of them (#562).
///
/// The sentence said the server "writes nothing to disk itself" while `blueprint_capture_still`,
/// `blueprint_capture_sequence`, `blueprint_snip` and `blueprint_feedback_file_issue` all wrote files. So the
/// sentence is built from `docs`' constants and this reads it against the folders the code resolves: a moved
/// path fails the build rather than the sentence going quietly stale.
#[test]
fn the_reference_names_the_folders_the_server_writes() {
    let rendered = dark_factory_mcp::docs::render();
    assert!(
        !rendered.contains("writes nothing to disk"),
        "docs/mcp-tools.md's opening says the server writes nothing, and it writes every capture and every \
         filed report"
    );
    // **Under a project**: the capture folder is that project's `target/loop/captures`.
    let project = std::path::Path::new("/tmp/a-project");
    let folders = dark_factory_mcp::docs::written_folders(Some(project));
    let captures = folders.captures.expect("a project's capture folder resolves");
    assert_eq!(captures, project.join("target/loop/captures"));
    let tail = captures.strip_prefix(project).expect("under the project");
    let tail = tail.to_string_lossy().replace('\\', "/");
    assert!(
        rendered.contains(&format!("`{tail}/<label>/`")),
        "the reference does not name the project capture folder `{tail}/<label>/` that `capture_dir` returns"
    );
    // **With no project**: the person's data folder's `captures`.
    let user = dark_factory_mcp::docs::written_folders(None);
    if let Some(captures) = user.captures {
        let leaf = captures.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        assert!(
            rendered.contains(&format!("`{leaf}/`")),
            "the reference does not name the no-project capture folder `{leaf}/` that `capture_dir` returns"
        );
    }
    // **A filed report**: always the person's data folder's `feedback`.
    if let Some(feedback) = folders.feedback {
        let leaf = feedback.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        assert!(
            rendered.contains(&format!("`{leaf}/`")),
            "the reference does not name the feedback folder `{leaf}/` that `feedback_dir` returns"
        );
    }
    // **And the constants the sentence interpolates are the folders' own leaves**, so a rename in `user_dirs`
    // is caught here and not by a person reading a stale sentence.
    if let Some(captures) = dark_factory_mcp::docs::written_folders(None).captures {
        assert_eq!(captures.file_name().unwrap_or_default(), dark_factory_mcp::docs::CAPTURES_IN_USER_DIR);
    }
    if let Some(feedback) = dark_factory_mcp::docs::written_folders(None).feedback {
        assert_eq!(feedback.file_name().unwrap_or_default(), dark_factory_mcp::docs::FEEDBACK_IN_USER_DIR);
    }
}

/// **The README's group table names every tool the server declares, and the sentence above it states the
/// table's own length** (#569).
///
/// The README said "There are 32 tools" over a table of 30: `blueprint_volume_report` and
/// `blueprint_feedback_file_issue`, the two newest, had no row. A group table is the page a person reads to
/// learn what the server can do, so the two tools it left out were the two least likely to be guessed.
#[test]
fn the_readme_table_names_every_tool() {
    use std::collections::BTreeSet;

    let readme = include_str!("../README.md");
    let declared: BTreeSet<String> = dark_factory_mcp::docs::TOOLS.iter().map(|t| t.name.to_owned()).collect();

    // **The tool table, and no other.** The README has more than one two-column table (the flags one
    // names `blueprint_list_rules` in prose), so the group table is found by its own header.
    const HEADER: &str = "| group | tools |";
    let start = readme.find(HEADER).expect("the README has a `| group | tools |` table");
    let body = &readme[start + HEADER.len()..];
    let end = body.find("\n\n").expect("the tool table ends at a blank line");
    let mut named: BTreeSet<String> = BTreeSet::new();
    for line in body[..end].lines() {
        let line = line.trim();
        if !line.starts_with("| ") || line.starts_with("|--") {
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').collect();
        if cells.len() != 2 {
            continue;
        }
        for tool in cells[1].split(',') {
            let tool = tool.trim().trim_matches('`');
            if tool.starts_with("blueprint_") {
                assert!(named.insert(tool.to_owned()), "the README's tool table names `{tool}` twice");
            }
        }
    }
    let missing: Vec<&String> = declared.difference(&named).collect();
    let extra: Vec<&String> = named.difference(&declared).collect();
    assert!(
        missing.is_empty(),
        "**the MCP README's group table omits {missing:?}**: every tool the server declares needs a group \
         row, or a person reading the table never learns it exists"
    );
    assert!(extra.is_empty(), "the README's table names {extra:?}, which the server does not declare");

    // **And the sentence above the table states the table's own length.**
    let at = readme.find("There are ").expect("the README states a tool count") + "There are ".len();
    let digits: String = readme[at..].chars().take_while(char::is_ascii_digit).collect();
    let said: usize = digits.parse().expect("the sentence states a number");
    assert_eq!(
        said,
        declared.len(),
        "the README says there are {said} tools and the table names {}",
        declared.len()
    );
}
