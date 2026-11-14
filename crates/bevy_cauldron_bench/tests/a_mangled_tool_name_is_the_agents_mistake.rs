//! **A tool name that does not exist is the agent's mistake, not the harness's** (#441, BM-11).
//!
//! The style study (#409) lost 18 of its 48 runs to one thing: the model's first call was a tool named
//! `xd__mcp__cauldron_grimoire_guide` (or `xd___mcp__...`), omp answered `Tool xd__mcp__cauldron_grimoire_guide not
//! found. Did you mean mcp__cauldron_grimoire_guide?`, the agent recovered and finished the task, and
//! `read_transcript` then discarded the whole trial as "the profile was not isolated". The cause is in the prompt omp
//! writes, not in the runner (the write-up is on the issue): omp mounts the editor's tools as `xd://` devices, lists
//! them as `xd://mcp__cauldron_grimoire_guide`, and the server's own instructions say "Call grimoire_guide first", so
//! the model turns the device path into a function name.
//!
//! The isolation check's job is a **leak**: a tool the agent actually ran that the Dark Factory did not hand out. A
//! name omp refused ("not found") ran nothing, so it is recorded as an agent error and the trial is scored. Two real
//! transcripts, trimmed to the calls that matter (`tests/fixtures/`, recorded on bmb 2026-11-14 from the study's
//! without-style greeting trial 1 and with-style greeting trial 3), and the same shapes written out for the leaks.

use bevy_cauldron_bench::run::{read_transcript, AgentRun};
use serde_json::{json, Value};

const SERVER: &str = "cauldron";

/// The recovery through the device path the prompt documents: the mangled call, then `write xd://mcp__..._guide`.
const THROUGH_THE_DEVICE: &str = include_str!("fixtures/a_mangled_tool_name.jsonl");
/// The recovery through omp's own suggestion: the mangled call, then a direct `mcp__cauldron_grimoire_guide` call.
const THROUGH_THE_HINT: &str = include_str!("fixtures/a_mangled_name_then_a_direct_call.jsonl");

fn scratch(test: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("grimoire-bench-mangled-{}-{test}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

/// **Read a transcript the way a trial does**, with the tool list omp's extension records beside it.
fn read(test: &str, transcript: &str) -> Result<AgentRun, String> {
    let dir = scratch(test);
    std::fs::write(dir.join("transcript.jsonl"), transcript).unwrap_or_else(|e| panic!("{e}"));
    let offered: Vec<String> = ["read", "write", "bash", "glob", "grep", "edit"]
        .iter()
        .map(|t| (*t).to_owned())
        .chain(
            ["guide", "state", "list_operations", "apply_operation", "present", "style", "style_propose"]
                .iter()
                .map(|t| format!("mcp__{SERVER}_grimoire_{t}")),
        )
        .collect();
    std::fs::write(dir.join("tools-offered.jsonl"), json!({"why": "before_agent_start", "tools": offered}).to_string())
        .unwrap_or_else(|e| panic!("{e}"));
    let run = read_transcript(&dir.join("transcript.jsonl"), &dir, SERVER);
    let _ = std::fs::remove_dir_all(&dir);
    run
}

/// **A turn that spoke**, so a transcript made of the lines below counts as one the model answered.
fn turn() -> Value {
    json!({"type": "message_end", "message": {"role": "assistant", "stopReason": "stop",
        "usage": {"input": 5, "output": 7, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 12}}})
}

/// **The model asking for a tool by `name`**, as omp's stream writes it.
fn asks(id: &str, name: &str, args: Value) -> Value {
    json!({"type": "message_update", "assistantMessageEvent": {"type": "toolcall_end", "contentIndex": 0,
        "toolCall": {"type": "toolCall", "id": id, "name": name, "arguments": args}}})
}

/// **omp refusing a name it has no tool for**, in the words it wrote in every real transcript.
fn not_found(id: &str, name: &str, suggestion: Option<&str>) -> Value {
    let said = match suggestion {
        Some(s) => format!("Tool {name} not found. Did you mean {s}?"),
        None => format!("Tool {name} not found"),
    };
    json!({"type": "tool_execution_end", "toolCallId": id, "toolName": name, "isError": true,
        "result": {"content": [{"type": "text", "text": said}], "details": {"isError": true, "error": said}}})
}

/// **A tool that ran and answered**, whatever its name.
fn ran(id: &str, name: &str) -> Value {
    json!({"type": "tool_execution_end", "toolCallId": id, "toolName": name, "isError": false,
        "result": {"content": [{"type": "text", "text": "{\"ok\":true}"}], "details": {"serverName": "elsewhere", "mcpToolName": "paper_search"}}})
}

/// **The mangled call, as the study's trials made it, is one agent error and the trial is read.**
#[test]
fn a_mangled_tool_name_is_an_agent_error_and_the_trial_is_read() {
    let run = read("device", THROUGH_THE_DEVICE).unwrap_or_else(|e| panic!("an agent's mistake is not a harness error: {e}"));
    assert_eq!(run.agent_errors.len(), 1, "one call to a tool that does not exist: {:?}", run.agent_errors);
    let error = &run.agent_errors[0];
    assert_eq!(error.tool, "xd__mcp__cauldron_grimoire_guide", "the name the model used, so a report can say which");
    assert!(error.said.contains("not found"), "omp's own words are kept: {}", error.said);
    assert_eq!(run.tool_calls, ["grimoire_guide"], "the recovery through `write xd://` is the one editor call, and the mistake is no call");
    assert_eq!(run.tool_log.len(), 1, "an agent error is not in the tool log");
    assert!(run.usage.is_some(), "the trial's tokens are read as for any scored trial");
}

/// **omp's suggestion is a route that works**, and the runner counts the calls made through it.
///
/// After "Did you mean mcp__cauldron_grimoire_guide?" five of the study's agents called the `mcp__` name directly; omp ran it.
/// `read_transcript` knew two routes (`write xd://` and `eval`), so those calls were not in the trial at all, and the hand-back
/// one of them made would have been scored "no hand-back was made".
#[test]
fn a_direct_editor_call_after_the_hint_is_counted_with_its_arguments() {
    let run = read("hint", THROUGH_THE_HINT).unwrap_or_else(|e| panic!("the trial reads: {e}"));
    assert_eq!(run.agent_errors.len(), 1, "{:?}", run.agent_errors);
    for tool in ["grimoire_guide", "grimoire_state", "grimoire_style", "grimoire_list_operations", "grimoire_present"] {
        assert!(run.tool_calls.iter().any(|c| c == tool), "`{tool}` was called directly and ran: {:?}", run.tool_calls);
    }
    assert!(run.tool_calls.iter().all(|c| !c.starts_with("mcp__")), "names carry no `mcp__<server>_` prefix: {:?}", run.tool_calls);
    assert_eq!(run.handbacks.len(), 1, "the direct `grimoire_present` carried a hand-back");
    assert!(!run.handbacks[0].title.is_empty(), "and its words: {:?}", run.handbacks[0]);
    let style = run.tool_log.iter().find(|c| c.tool == "grimoire_style").unwrap_or_else(|| panic!("the style call is logged"));
    assert!(style.refused, "its isError is the editor's refusal ('no map was named')");
}

/// **A name with no suggestion is refused the same way**, and so is an editor tool the arm hides.
#[test]
fn any_name_omp_refused_is_an_agent_error() {
    let events = [
        turn(),
        asks("a", "list_operations_placeholder", json!({})),
        not_found("a", "list_operations_placeholder", None),
        asks("b", "mcp__cauldron_grimoire_style", json!({})),
        not_found("b", "mcp__cauldron_grimoire_style", Some("mcp__cauldron_grimoire_style_propose")),
    ];
    let text = events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let run = read("refused", &text).unwrap_or_else(|e| panic!("two refused names are two mistakes: {e}"));
    let names: Vec<&str> = run.agent_errors.iter().map(|e| e.tool.as_str()).collect();
    assert_eq!(names, ["list_operations_placeholder", "mcp__cauldron_grimoire_style"]);
    assert!(run.tool_calls.is_empty(), "nothing ran: {:?}", run.tool_calls);
}

/// **A tool that RAN and is not the Dark Factory's is a leak**, and stays a harness error.
#[test]
fn a_tool_outside_the_set_that_ran_is_still_a_harness_error() {
    let events = [
        turn(),
        asks("leak", "mcp__home-still_paper_search", json!({"query": "x"})),
        ran("leak", "mcp__home-still_paper_search"),
    ];
    let text = events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let said = read("leak", &text).expect_err("the agent reached a server the profile should not have");
    assert!(said.contains("mcp__home-still_paper_search") && said.contains("not isolated"), "{said}");
}

/// **A call that never got an answer cannot be called a mere mistake**: the runner cannot tell it did not run.
#[test]
fn an_outside_tool_with_no_answer_is_a_harness_error() {
    let events = [turn(), asks("silent", "web_fetch", json!({"url": "https://example.com"}))];
    let text = events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let said = read("silent", &text).expect_err("no answer, so no proof that nothing ran");
    assert!(said.contains("web_fetch") && said.contains("not isolated"), "{said}");
}

/// **"not found" about one name does not excuse another**: the answer must be the refused call's own.
#[test]
fn a_not_found_for_another_name_does_not_excuse_a_leak() {
    let events = [
        turn(),
        asks("c", "web_fetch", json!({})),
        // The answer under this id is a success of a different tool, and a "not found" of a third name is not it.
        ran("c", "web_fetch"),
        not_found("d", "something_else", None),
    ];
    let text = events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let said = read("other", &text).expect_err("web_fetch ran");
    assert!(said.contains("web_fetch"), "{said}");
}

/// **A built-in's own output is not omp's refusal** (review of #441): a `bash` whose output says `Tool bash not found` ran, and
/// is no call to a tool that does not exist.
#[test]
fn a_built_ins_output_is_not_an_agent_error() {
    let said = "Tool bash not found in this image";
    let events = [
        turn(),
        asks("e", "bash", json!({"command": "which tool"})),
        json!({"type": "tool_execution_end", "toolCallId": "e", "toolName": "bash", "isError": true,
            "result": {"content": [{"type": "text", "text": said}]}}),
    ];
    let text = events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let run = read("builtin", &text).unwrap_or_else(|e| panic!("a bash call reads: {e}"));
    assert!(run.agent_errors.is_empty(), "{:?}", run.agent_errors);
}

/// **Another server whose name starts with ours is not the editor** (review of #441): `mcp__cauldron_x_search` shares the prefix
/// `mcp__cauldron_`, is in no list the Dark Factory offered, and when it ran it is a leak.
#[test]
fn a_server_whose_name_starts_with_ours_is_a_leak_when_it_ran() {
    let events = [turn(), asks("x", "mcp__cauldron_x_search", json!({})), ran("x", "mcp__cauldron_x_search")];
    let text = events.iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
    let said = read("sibling", &text).expect_err("the agent reached a server that is not the editor's");
    assert!(said.contains("mcp__cauldron_x_search") && said.contains("not isolated"), "{said}");
}

/// **An agent error is no editor call, but the live tool-call budget still counts it** (it was a call the agent made).
#[test]
fn the_budget_counts_an_agent_error_as_a_call() {
    let end = not_found("a", "xd__mcp__cauldron_grimoire_guide", None);
    assert_eq!(bevy_cauldron_bench::run::omp_calls(&end), 1);
}
