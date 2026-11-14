#!/usr/bin/env python3
"""A SECOND, POST-HOC analysis of #409, for the replication (docs/research/2026-11-14-style-helps-an-agent-results-2.md).

It is NOT the registered script and does not replace it: the registered script (`2026-11-14-style-helps-an-agent-analyse.py`, unedited) is run first and its output is
reported verbatim.  This script was written AFTER the first run's data was seen (that run lost 58% of the without-style arm to a harness error the runner has since fixed,
BM-11 #441), and it differs from the registered one in exactly these ways, nothing else:

  1. tool use is read from the trial record's `tool_log` (every editor call the runner saw: through `xd://` and by the tool's own direct name `mcp__cauldron_grimoire_*`),
     not only from the `xd://` transcript events; a design page opened by a generic tool (`read`, `bash`, ...) is still read from the transcript with the registered reader;
  2. a `followed` citation is read from the agent's own `grimoire_present` call in the transcript, through either route;
  3. it reports the agent errors per arm (`agent_errors` in `trial.json`: a call to a tool omp refused, which the agent recovered from) and a Fisher exact test of the arms.

The statistic is the registered one, imported from the registered script and not re-implemented: lines satisfied per run, D = with - without summed over the tasks, a 95% percentile bootstrap
of D, an exact permutation p-value, the route stratification and the 70% claim rule, per-task cells.

usage:  analyse-2.py <without run>[,<run>...] <with run>[,<run>...]      |      analyse-2.py --selftest
"""
import importlib.util, json, math, sys, tempfile
from fractions import Fraction
from pathlib import Path

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("registered", HERE / "2026-11-14-style-helps-an-agent-analyse.py")
reg = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reg)

DIRECT = "mcp__cauldron_"          # the editor tool called by its own name (the registered reader sees only the xd:// route)


def calls_of(trial_dir, record):
    """(tool_log names in order, transcript events as the registered reader reads them, direct-name calls in the transcript)."""
    log = [c["tool"] for c in record.get("tool_log", [])]
    transcript = trial_dir / "transcript.jsonl"
    events = reg.events(transcript)
    direct = []
    if transcript.exists():
        for row in transcript.read_text().splitlines():
            try:
                e = json.loads(row)
            except ValueError:
                continue
            inner = e.get("assistantMessageEvent") or {}
            if e.get("type") == "message_update" and inner.get("type") == "toolcall_end":
                call = inner.get("toolCall") or {}
                if str(call.get("name", "")).startswith(DIRECT):
                    direct.append((call["name"][len(DIRECT):], call.get("arguments") or {}))
    return log, events, direct


def route_of(log, events):
    tool = "grimoire_style" in log
    file = any(kind == "generic" and reg.re.search(r"design[/\\]", json.dumps(args)) for kind, _, args in events)
    return {(True, False): "tool", (False, True): "file", (True, True): "both", (False, False): "neither"}[(tool, file)]


def cell(path, record, guards):
    log, events, direct = calls_of(path.parent, record)
    present_args = [a for k, n, a in events if k == "mcp" and n == "grimoire_present"] + [a for n, a in direct if n == "grimoire_present"]
    first = next((n for n in log if n in ("grimoire_style", "grimoire_apply_operation")), None)
    return {
        "lines": sum(c["passed"] for i, c in enumerate(record["checks"]) if i not in guards),
        "route": route_of(log, events),
        "followed": any(isinstance(a, dict) and a.get("followed") for a in present_args),
        "style_before_edit": first == "grimoire_style",
        "wishes": record["wishes"], "offered": record["tools_offered"],
        "hidden_writes": sum(1 for k, n, a in events if k == "device" and n == "write" and reg.re.search(r"grimoire_style(?!_)", str(a.get("path", "")))),
        "hidden_reads": 0,
        "direct_calls": len(direct),
        "agent_errors": record.get("agent_errors", 0),
        "agent_error_tools": [c.get("tool") for c in record.get("agent_error_calls", [])],
    }


def load(runs, state=None):
    """{task: [cells]} for scored runs, and the harness-error count."""
    cells, harness = {t: [] for t in reg.GUARDS}, 0
    for run in runs.split(","):
        for task, guards in reg.GUARDS.items():
            for path in sorted(((state or reg.STATE) / run / task).glob("trial-*/trial.json")):
                record = json.loads(path.read_text())
                if record["outcome"] == "harness_error":
                    harness += 1
                    continue
                assert len(record["checks"]) == len(guards) + reg.LINES_PER_TASK[task], f"{path}: the task's checks are not the registered ones"
                cells[task].append(cell(path, record, guards))
    return cells, harness


def fisher_two_sided(a, b, c, d):
    """Exact two-sided Fisher p of the 2x2 table [[a, b], [c, d]] (the sum of the tables as or less likely than the observed)."""
    n, r1, c1 = a + b + c + d, a + b, a + c

    def p(x):
        return Fraction(math.comb(r1, x) * math.comb(n - r1, c1 - x), math.comb(n, c1))
    lo, hi = max(0, c1 - (n - r1)), min(r1, c1)
    observed = p(a)
    return float(sum(p(x) for x in range(lo, hi + 1) if p(x) <= observed))


def main(without_runs, with_runs):
    without, h0 = load(without_runs)
    with_, h1 = load(with_runs)
    a = reg.analyse(without, with_, h0 + h1)
    print("=== SECOND (post-hoc) ANALYSIS: tool use read from trial.json tool_log, direct and xd:// calls ===")
    reg.report(a)
    for arm, cells in (("without-style", without), ("with-style", with_)):
        runs = [c for t in cells for c in cells[t]]
        err = sum(c["agent_errors"] > 0 for c in runs)
        calls = sum(c["agent_errors"] for c in runs)
        names = sorted({n for c in runs for n in c["agent_error_tools"] if n})
        print(f"agent errors, {arm}: {err} of {len(runs)} scored runs had at least one ({calls} calls); direct-name editor calls counted: {sum(c['direct_calls'] for c in runs)}; tools refused: {', '.join(names) or 'none'}")
    w = [c["agent_errors"] > 0 for t in without for c in without[t]]
    v = [c["agent_errors"] > 0 for t in with_ for c in with_[t]]
    if w and v:
        a_, b_, c_, d_ = sum(v), len(v) - sum(v), sum(w), len(w) - sum(w)
        print(f"Fisher exact, runs with an agent error (with-style {a_} of {len(v)} against without-style {c_} of {len(w)}), two-sided p = {fisher_two_sided(a_, b_, c_, d_):.4f}")
    print(f"harness errors (not scored): without-style {h0}, with-style {h1}")


def selftest():
    assert abs(fisher_two_sided(3, 1, 1, 3) - 0.4857142857) < 1e-9
    assert abs(fisher_two_sided(0, 5, 5, 0) - 2 / 252) < 1e-12
    assert fisher_two_sided(2, 2, 2, 2) == 1.0
    with tempfile.TemporaryDirectory() as d:
        t = Path(d) / "trial-1"
        t.mkdir()
        rows = [
            {"type": "message_update", "assistantMessageEvent": {"type": "toolcall_end", "toolCall": {"name": "mcp__cauldron_grimoire_present", "arguments": {"followed": [{"path": "design/style.md", "line": 3}]}}}},
            {"type": "message_update", "assistantMessageEvent": {"type": "toolcall_end", "toolCall": {"name": "read", "arguments": {"path": "design/style.md"}}}},
        ]
        (t / "transcript.jsonl").write_text("\n".join(json.dumps(r) for r in rows))
        record = {"checks": [{"passed": True}] * 7, "wishes": [], "tools_offered": [], "tool_log": [{"tool": "grimoire_style", "args": "{}", "refused": False}, {"tool": "grimoire_apply_operation", "args": "{}", "refused": False}],
                  "agent_errors": 1, "agent_error_calls": [{"tool": "xd__mcp__cauldron_grimoire_guide", "said": "unknown"}]}
        c = cell(t / "trial.json", record, {0})
        assert c["route"] == "both" and c["followed"] and c["style_before_edit"] and c["lines"] == 6 and c["agent_errors"] == 1 and c["direct_calls"] == 1, c
        record["tool_log"] = []
        assert cell(t / "trial.json", record, {0})["route"] == "file"
    print("selftest passed")


if __name__ == "__main__":
    if sys.argv[1:] == ["--selftest"]:
        selftest()
    else:
        main(sys.argv[1], sys.argv[2])
