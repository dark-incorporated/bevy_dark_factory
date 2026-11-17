"""The two arms of the look-then-ask question, read from the runs' own files (#457, DM-06). Prints markdown.

**Every number comes from a file a run wrote**: each trial's `trial.json` (outcome, tool calls, wall time, cost) and
each trial's `transcript.jsonl` (the `grimoire_decide` calls, their questions and their `latency_ms`). Nothing is
typed in by hand, and nothing is estimated.

Usage, on the host that holds the runs:

    python3 docs/bench/2026-11-17-ask-two-arms-analyse.py <with-run> <without-run>

The run names are directories under `~/.local/state/cauldron-bench`. Its default is the two names this issue's run
used, so a reader who has them runs it with no arguments.
"""
import collections
import glob
import json
import math
import os
import re
import sys

ROOT = os.path.expanduser("~/.local/state/cauldron-bench")

# **The calls that change what the person sees.** `grimoire_apply_operation` is the one the bench's tasks use for
# every visible change; `note_resolve` answers a note, which is also a result the person sees.
VISIBLE = {"grimoire_apply_operation", "note_resolve"}

# **The issue's own threshold** (`#457`): a call with four or more questions after the last visible operation.
# Reported beside the one-question reading, because the revised guide no longer asks for a count and the two numbers
# then answer two different questions: "does it ask at all" and "does it ask as #457 wrote it".
MIN_QUESTIONS = 4


def wilson(k, n, z=1.96):
    """The Wilson score interval for `k` of `n`, which a small sample needs more than the normal one."""
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / d
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (max(0.0, centre - half), min(1.0, centre + half))


def trials(run):
    """Every trial of `run`, with the directory it wrote, newest run first."""
    out = []
    for path in sorted(glob.glob(os.path.join(ROOT, run, "*", "trial-*", "trial.json"))):
        with open(path) as f:
            record = json.load(f)
        record["_dir"] = os.path.dirname(path)
        out.append(record)
    return out


def _balanced(code, i):
    """The text of the `(...)` call whose `(` is at `code[i]`, respecting string quotes."""
    depth, quote, j = 0, None, i
    while j < len(code):
        ch = code[j]
        if quote:
            if ch == "\\":
                j += 1
            elif ch == quote:
                quote = None
        elif ch in "\"'`":
            quote = ch
        elif ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                return code[i : j + 1]
        j += 1
    return code[i:]


def _questions_in(args):
    """How many questions an ad-hoc `grimoire_decide` call asked, from its arguments."""
    q = args.get("questions") if isinstance(args, dict) else None
    if isinstance(q, dict):
        return len(q)
    if isinstance(q, list):
        return len(q)
    return 0


def calls_of(directory):
    """Every editor tool call the agent made, in order, from `directory`'s transcript alone.

    **Three routes reach an MCP tool through omp**, and all three are read, because a reader that knows only one
    counts nothing: `write` to `xd://mcp__cauldron_<tool>`; a direct `mcp__cauldron_<tool>` call; and `eval` code
    that calls `tool.mcp__cauldron_<tool>(...)`. Each entry is `dict(tool, questions, view, refused, text)`.
    """
    out, pending = [], {}
    try:
        lines = open(os.path.join(directory, "transcript.jsonl"))
    except OSError:
        return out
    for line in lines:
        if '"tool_execution_' not in line:
            continue
        try:
            event = json.loads(line)
        except Exception:
            continue
        kind, call_id = event.get("type"), event.get("toolCallId")
        name = event.get("toolName") or ""
        if kind == "tool_execution_start":
            args = event.get("args") or {}
            items = []
            if name == "write" and str(args.get("path", "")).startswith("xd://mcp__cauldron_"):
                tool = args["path"].split("mcp__cauldron_", 1)[1]
                try:
                    body = json.loads(args.get("content") or "{}")
                except Exception:
                    body = {}
                items.append((tool, body))
            elif name.startswith("mcp__cauldron_"):
                items.append((name.split("mcp__cauldron_", 1)[1], args))
            elif name == "eval":
                code = args.get("code") or ""
                for match in re.finditer(r"tool\.mcp__cauldron_(\w+)\(", code):
                    body = _balanced(code, match.end() - 1)
                    # **An `eval` cell's arguments are not in the transcript as JSON**, so the questions are
                    # counted from the text: one `instructions` per question, and `view: true` when it is set.
                    if match.group(1) == "grimoire_decide":
                        n = len(re.findall(r"instructions", body))
                        items.append((
                            match.group(1),
                            {"questions": {"?%d" % k: 1 for k in range(n)}, "view": bool(re.search(r"view\s*:\s*true", body))},
                        ))
                    else:
                        items.append((match.group(1), {}))
            if items:
                pending[call_id] = (name, items)
        elif kind == "tool_execution_end" and call_id in pending:
            name, items = pending.pop(call_id)
            text = " ".join(
                c.get("text", "") for c in (event.get("result") or {}).get("content", []) if isinstance(c, dict)
            )
            failed = bool(event.get("isError")) or text.lstrip().startswith(("Error", '{"refused"'))
            for tool, args in items:
                # **An answer that quotes the guide is not a call's answer**, and would look like a call of its own.
                if "Check your work by asking" in text:
                    text = ""
                out.append({
                    "tool": tool,
                    "questions": _questions_in(args),
                    "view": isinstance(args, dict) and args.get("view") is True,
                    "refused": failed and name != "eval",
                    "text": text,
                })
    return out


def decide_answers(calls):
    """What each `grimoire_decide` call answered: whether it answered, how long it took, and the model that did."""
    answers = []
    for call in calls:
        if call["tool"] != "grimoire_decide":
            continue
        text = call["text"]
        latency = re.search(r'"latency_ms":\s*(\d+)', text)
        where = re.search(r'"where":\s*"(\w+)"', text)
        model = re.search(r'"model":\s*"([^"]+)"', text)
        answers.append({
            "answered": bool(latency),
            "latency_ms": int(latency.group(1)) if latency else None,
            "where": where.group(1) if where else None,
            "model": model.group(1) if model else None,
            "text": text[:400],
            "down": ("did not answer" in text) or ("Unreachable" in text),
        })
    return answers


def arm(run):
    """One arm's numbers, each with the trials it came from."""
    records = trials(run)
    scored = [t for t in records if t["outcome"] != "harness_error"]
    eligible, asked4, asked1, any_call = [], [], [], []
    per_task = collections.defaultdict(lambda: [0, 0, 0, 0])
    questions, view_calls, decide_calls = [], 0, 0
    latencies, wheres, models, refused, down, void_trials = [], collections.Counter(), collections.Counter(), [], 0, []
    for record in scored:
        calls = calls_of(record["_dir"])
        visible = [i for i, c in enumerate(calls) if c["tool"] in VISIBLE and not c["refused"]]
        last = visible[-1] if visible else -1
        decide = [(i, c) for i, c in enumerate(calls) if c["tool"] == "grimoire_decide"]
        bucket = per_task[record["task"]]
        bucket[0] += 1
        bucket[1] += record["outcome"] == "pass"
        if visible:
            eligible.append(record)
            bucket[2] += 1
        # **Asked after the last visible operation**, which is the moment the guide's rule is about: a question
        # asked before the change cannot be about the change.
        if visible and any(i > last and c["questions"] >= MIN_QUESTIONS for i, c in decide):
            asked4.append(record)
            bucket[3] += 1
        if visible and any(i > last and c["questions"] >= 1 for i, c in decide):
            asked1.append(record)
        if decide:
            any_call.append(record)
        for _, call in decide:
            decide_calls += 1
            questions.append(call["questions"])
            view_calls += call["view"]
        answers = decide_answers(calls)
        for answer in answers:
            if answer["answered"]:
                latencies.append(answer["latency_ms"])
                wheres[answer["where"]] += 1
                models[answer["model"]] += 1
            else:
                refused.append((record["task"], answer["text"]))
                if answer["down"]:
                    down += 1
        if any(a["down"] for a in answers):
            void_trials.append(record)
    successes = sum(1 for t in scored if t["outcome"] == "pass")
    return {
        "run": run,
        "records": records,
        "scored": scored,
        "successes": successes,
        "eligible": eligible,
        "asked4": asked4,
        "asked1": asked1,
        "any_call": any_call,
        "per_task": per_task,
        "questions": questions,
        "view_calls": view_calls,
        "decide_calls": decide_calls,
        "latencies": latencies,
        "wheres": wheres,
        "models": models,
        "refused": refused,
        "down": down,
        "void_trials": void_trials,
        "cost": sum(t.get("cost_reported_usd") or 0.0 for t in scored),
        "walls": sorted(t.get("wall_s") or 0.0 for t in scored),
    }


def pct(k, n):
    return f"{k}/{n} = {100 * k / n:.0f}%" if n else "n/a"


def interval(k, n):
    lo, hi = wilson(k, n)
    return f"[{100 * lo:.0f}%, {100 * hi:.0f}%]"


def median(xs):
    xs = sorted(xs)
    return xs[len(xs) // 2] if xs else None


def main():
    with_run = sys.argv[1] if len(sys.argv) > 1 else "ask-two-arms-with"
    without_run = sys.argv[2] if len(sys.argv) > 2 else "ask-two-arms-without"
    arms = [("with the guide", arm(with_run)), ("without the section", arm(without_run))]

    print("## The four numbers\n")
    print("| | with the guide | without the section |")
    print("|---|---|---|")
    for label, key, fmt in [
        ("trials scored", "scored", lambda a: str(len(a["scored"]))),
        ("harness errors", "records", lambda a: str(len(a["records"]) - len(a["scored"]))),
        ("success", "successes", lambda a: f'{pct(a["successes"], len(a["scored"]))} {interval(a["successes"], len(a["scored"]))}'),
        ("eligible trials (a visible change applied)", "eligible", lambda a: str(len(a["eligible"]))),
        ("**asked ≥4 questions after the last visible change**", "asked4",
         lambda a: f'**{pct(len(a["asked4"]), len(a["eligible"]))}** {interval(len(a["asked4"]), len(a["eligible"]))}'),
        ("asked ≥1 question after the last visible change", "asked1",
         lambda a: f'{pct(len(a["asked1"]), len(a["eligible"]))} {interval(len(a["asked1"]), len(a["eligible"]))}'),
        ("trials that called `grimoire_decide` at all", "any_call", lambda a: pct(len(a["any_call"]), len(a["scored"]))),
        ("`grimoire_decide` calls", "decide_calls", lambda a: f'{a["decide_calls"]} (view: true {a["view_calls"]})'),
        ("mean questions per call", "questions",
         lambda a: f'{sum(a["questions"]) / len(a["questions"]):.2f}' if a["questions"] else "no calls"),
        ("answered calls", "latencies", lambda a: str(len(a["latencies"]))),
        ("latency of an answer, ms (median / mean / max)", "latencies",
         lambda a: (f'{median(a["latencies"])} / {sum(a["latencies"]) / len(a["latencies"]):.0f} / {max(a["latencies"])}'
                    if a["latencies"] else "none")),
        ("`where` / `model` reported", "models",
         lambda a: f'{dict(a["wheres"])} / {dict(a["models"])}' if a["latencies"] else "none"),
        ("calls the endpoint did not answer (void cells)", "down", lambda a: str(a["down"])),
        ("cost (as the harness reported it)", "cost", lambda a: f'${a["cost"]:.4f}'),
        ("median wall s", "walls", lambda a: f'took {median(a["walls"]):.0f}' if a["walls"] else "n/a"),
    ]:
        cells = []
        for _, a in arms:
            try:
                cells.append(fmt(a))
            except (KeyError, TypeError, ZeroDivisionError):
                cells.append("n/a")
        print(f'| {label} | {cells[0]} | {cells[1]} |')

    print("\n## By task\n")
    print("| task | with: trials, passed, eligible, asked≥4, asked≥1 | without: trials, passed, eligible, asked≥4, asked≥1 |")
    print("|---|---|---|")
    a, b = arms[0][1], arms[1][1]
    for task in sorted(set(a["per_task"]) | set(b["per_task"])):
        wa = a["per_task"].get(task, [0] * 4)
        wb = b["per_task"].get(task, [0] * 4)
        print(f'| `{task}` | {wa[0]} {wa[1]} {wa[2]} {wa[3]} | {wb[0]} {wb[1]} {wb[2]} {wb[3]} |')

    print("\n## Trials that asked a decision model, and did not\n")
    for label, arm_data in arms:
        asked_ids = {id(t) for t in arm_data["asked4"]}
        yes = [t for t in arm_data["eligible"] if id(t) in asked_ids]
        no = [t for t in arm_data["eligible"] if id(t) not in asked_ids]

        def line(xs):
            if not xs:
                return "none"
            passed = sum(1 for t in xs if t["outcome"] == "pass")
            calls = sum(t.get("tool_call_count") or 0 for t in xs) / len(xs)
            wall = sum(t.get("wall_s") or 0.0 for t in xs) / len(xs)
            return f'{pct(passed, len(xs))}, {calls:.1f} calls, {wall:.0f} s'

        print(f"- **{label}**: asked ≥4: {line(yes)}; did not: {line(no)}")

    print("\n## Decision calls that were refused or unanswered\n")
    for label, arm_data in arms:
        print(f"- **{label}**: {len(arm_data['refused'])} refused/unanswered, {arm_data['down']} of them the endpoint not answering, in {len(arm_data['void_trials'])} trial(s)")
        for task, text in arm_data["refused"][:6]:
            print(f"  - `{task}`: {text[:160]}")


    print("\n## What was read\n")
    for label, arm_data in arms:
        print(f'- **{label}**: `{len(arm_data["records"])}` trial records under `~/.local/state/cauldron-bench/{arm_data["run"]}/`')
        print(f'  - {len({t["task"] for t in arm_data["records"]})} task(s)')
        print(f'  - transcripts read: `{arm_data["run"]}/<task>/trial-<n>/transcript.jsonl` ({sum(1 for _ in glob.glob(os.path.join(ROOT, arm_data["run"], "*", "trial-*", "transcript.jsonl")))} files)')


if __name__ == "__main__":
    main()
