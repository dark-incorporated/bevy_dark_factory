//! **A report counts the agent's calls to tools that do not exist, per arm** (#441, BM-11).
//!
//! An agent error is a scored trial, so it is in no "Harness errors" row and does not make a run unfit. It is a field of
//! `trial.json` (`agent_errors`, a count, with the calls under `agent_error_calls`), a per-task count and a per-tool count in
//! the report, a section of the markdown, and a line of `compare` for each arm. Built from recorded `trial.json` fixtures
//! as the other report tests are: no model, no editor.

use std::path::PathBuf;

use bevy_dark_factory_bench::compare::compare;
use bevy_dark_factory_bench::report::{Header, Report, RunData, REPORT_VERSION, RUN_FILE};
use bevy_dark_factory_bench::run::{AgentError, TrialRecord};
use bevy_dark_factory_bench::Outcome;
use serde_json::{json, Value};

const MANGLED: &str = "xd__mcp__dark_factory_blueprint_guide";

fn scratch(test: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("bench-report-agent-errors-{test}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}

fn header(label: &str) -> Header {
    Header {
        version: REPORT_VERSION,
        label: label.to_owned(),
        commit: "0123456789abcdef0123456789abcdef01234567".to_owned(),
        guide_sha256: "ab".repeat(32),
        agent: "omp".to_owned(),
        agent_version: "omp/18.16.0".to_owned(),
        model: Some("anthropic/claude-sonnet-5-5".to_owned()),
        variant: Some(label.to_owned()),
        suite: "bench/tasks".to_owned(),
        trials: 2,
        host: "bmb".to_owned(),
        date: "2026-11-14".to_owned(),
        price: None,
    }
}

/// **A run of two scored trials of one task**; `errors` gives each trial's `agent_errors` (`None` leaves the field out, as a
/// `trial.json` written before it was).
fn report_with(test: &str, label: &str, errors: [Option<u64>; 2]) -> Report {
    let dir = scratch(test);
    std::fs::write(dir.join(RUN_FILE), serde_json::to_string_pretty(&header(label)).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    for (n, count) in (1..=2u32).zip(errors) {
        let d = dir.join("write-the-gatekeepers-greeting").join(format!("trial-{n}"));
        std::fs::create_dir_all(&d).unwrap_or_else(|e| panic!("{e}"));
        let mut value = json!({
            "task": "write-the-gatekeepers-greeting", "trial": n, "title": "t", "split": "dev", "tags": ["style"],
            "outcome": "pass", "reason": "", "checks": [], "tool_calls": ["blueprint_guide"], "tool_log": [],
            "refusals": 0, "refusals_by_tool": {}, "undos": 0, "edits_behind": [], "wall_s": 20.0, "wishes": [],
        });
        if let (Some(count), Some(object)) = (count, value.as_object_mut()) {
            object.insert("agent_errors".to_owned(), json!(count));
            let calls: Vec<Value> = (0..count).map(|_| json!({"tool": MANGLED, "said": format!("Tool {MANGLED} not found")})).collect();
            object.insert("agent_error_calls".to_owned(), json!(calls));
        }
        std::fs::write(d.join("trial.json"), serde_json::to_string_pretty(&value).unwrap_or_default()).unwrap_or_else(|e| panic!("{e}"));
    }
    let report = Report::build(&RunData::read(&dir).unwrap_or_else(|e| panic!("the run reads: {e}")));
    let _ = std::fs::remove_dir_all(&dir);
    report
}

/// **A trial with an agent error is scored**, and the report counts the error beside it.
#[test]
fn an_agent_error_is_counted_in_a_scored_run() {
    let report = report_with("counted", "without-style", [Some(1), Some(0)]);
    assert_eq!(report.harness_errors, 0, "an agent's mistake is no harness error");
    assert!(report.unfit.is_none(), "and does not make the run unfit: {:?}", report.unfit);
    assert_eq!(report.dev.trials, 2, "both trials are scored");
    assert_eq!(report.agent_errors, 1, "one call to a tool that does not exist in the run");
    assert_eq!(report.agent_errors_by_tool.get(MANGLED), Some(&1), "by the name the model used");
    let task = &report.tasks[0];
    assert_eq!((task.agent_errors, task.agent_error_trials.clone()), (1, vec![1]), "and in the task, with the trial that made it");
    let md = report.markdown();
    assert!(md.contains("## Agent errors"), "a section says so:\n{md}");
    assert!(md.contains(MANGLED) && md.contains("`write-the-gatekeepers-greeting` trial 1"), "names the tool and the trial:\n{md}");
}

/// **A run with none has no such section**, and a `trial.json` or report from before the field reads as none.
#[test]
fn no_agent_error_no_section_and_an_old_file_reads() {
    for (name, errors) in [("zero", [Some(0), Some(0)]), ("absent", [None, None])] {
        let report = report_with(name, "with-style", errors);
        assert!(!report.markdown().contains("## Agent errors"), "{name}:\n{}", report.markdown());
        assert_eq!(report.agent_errors, 0, "{name}");
    }
    let report = report_with("json", "with-style", [Some(2), None]);
    let mut json: Value = serde_json::from_str(&report.json().unwrap_or_default()).unwrap_or_default();
    if let Some(object) = json.as_object_mut() {
        for key in ["agent_errors", "agent_errors_by_tool"] {
            object.remove(key);
        }
    }
    let old = Report::from_json(&json.to_string()).unwrap_or_else(|e| panic!("an older report reads: {e}"));
    assert_eq!(old.agent_errors, 0, "a report with no such field has none");
}

/// **A comparison says each arm's count**, and compares two arms that made them: the mistake is not a refusal to compare.
#[test]
fn compare_lists_each_arms_agent_errors() {
    let without = report_with("cmp-a", "without-style", [Some(1), Some(1)]);
    let with = report_with("cmp-b", "with-style", [Some(0), None]);
    let comparison = compare(&without, &with, true).unwrap_or_else(|e| panic!("two scored arms compare: {e}"));
    assert_eq!((comparison.a_agent_errors, comparison.b_agent_errors), (2, 0));
    let md = comparison.markdown();
    assert!(md.contains("Agent errors") && md.contains("`a` 2") && md.contains("`b` 0"), "the answer carries both counts:\n{md}");
    let quiet = compare(&with, &with, true).unwrap_or_else(|e| panic!("{e}"));
    assert!(!quiet.markdown().contains("Agent errors"), "and says nothing when neither arm made one:\n{}", quiet.markdown());
}

/// **`trial.json` carries the count and the calls.**
#[test]
fn a_trial_record_writes_its_agent_errors() {
    let record = |held_out: bool| TrialRecord {
        task: "t".to_owned(),
        trial: 1,
        agent: "omp".to_owned(),
        model: None,
        variant: None,
        outcome: Outcome::Pass,
        reason: String::new(),
        checks: Vec::new(),
        tool_calls: Vec::new(),
        tool_log: Vec::new(),
        tokens: None,
        cost_reported_usd: None,
        edits_behind: Vec::new(),
        wall_s: 0.0,
        tools_offered: Vec::new(),
        wishes: Vec::new(),
        dir: PathBuf::new(),
        held_out,
        call_budget_exhausted: false,
        agent_errors: vec![AgentError { tool: MANGLED.to_owned(), said: format!("Tool {MANGLED} not found") }],
    };
    let json = record(false).to_json();
    assert_eq!(json.get("agent_errors").and_then(Value::as_u64), Some(1), "{json}");
    assert_eq!(json.pointer("/agent_error_calls/0/tool").and_then(Value::as_str), Some(MANGLED), "{json}");
}
