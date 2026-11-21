"""**Did the revised guide raise asking?** (#457, DM-06, 2026-11-20). Prints markdown, reads trial.json only.

The 2026-11-15 run measured the guide that demanded four or five questions: 22/43 eligible trials (51%) asked
>= 4 after their last visible operation, mean 3.33 questions per call, 35/43 asked at least one. The guide
was then rewritten to stop demanding a count and to ask only questions whose answer would change the next
operation. This script answers the question that revision left open, on today's main: what share ask at all,
and how many questions per call.

Two shares are reported, because the target changed with the text:
  * **>= 1 question** -- the only count the revised guide supports, since it no longer asks for four;
  * **>= 4 questions** -- kept solely so the 2026-11-15 figure has something to be compared against.
"""
import json, glob, os, re, sys, collections, math, subprocess

ROOT = os.environ.get("BENCH_ROOT") or os.path.expanduser("~/.local/state/dark_factory-bench")
# **The MCP tool prefix, in either era.** Runs before the rebrand say `mcp__cauldron_grimoire_`, today's say
# `mcp__dark_factory_`; one analyser reads both, so the 2026-11-15 figure stays comparable.
PREFIX = os.environ.get("TOOL_PREFIX") or "mcp__cauldron_grimoire_"
# **Tool names, normalized across the rebrand**: a call is `blueprint_apply_operation` today and
# `apply_operation` in the pre-rebrand runs, so the leading `blueprint_` is dropped before anything compares.
MUT = {"apply_operation"}   # what changes a map the person sees
MIN_Q = 4                             # the 2026-11-15 bar, kept for comparison only


def bare(tool):
    """A tool name without the era's `blueprint_` prefix, so one comparison reads both runs."""
    return tool[len("blueprint_"):] if tool.startswith("blueprint_") else tool


def wilson(k, n, z=1.96):
    if n == 0:
        return (0.0, 0.0)
    p = k / n
    d = 1 + z * z / n
    c = (p + z * z / (2 * n)) / d
    h = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return (max(0.0, c - h), min(1.0, c + h))


def _balanced(code, i):
    """The `(...)` group starting at `code[i]`, or the rest of the string."""
    depth, j, quote = 0, i, None
    while j < len(code):
        ch = code[j]
        if quote:
            if ch == quote:
                quote = None
            elif ch == "\\":
                j += 1
        elif ch in "\"'`":
            quote = ch
        elif ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                return code[i:j + 1]
        j += 1
    return code[i:]


def calls_of(d):
    """Every editor tool call the agent made, in order, from the transcript alone.

    dict(tool, nq, view, refused, text) -- `text` is the call's result, which is where `latency_ms`, `where`
    and `model` live: `trial.json`'s `tool_calls` is a list of tool *names*, so the answer's own numbers are
    only in the transcript. Same routes as the 2026-11-15 analyser: `xd://mcp__dark_factory_<tool>`, a direct
    `mcp__dark_factory_<tool>`, and eval's `tool.mcp__dark_factory_<tool>(...)`."""
    out, pending = [], {}
    try:
        lines = open(os.path.join(d, "transcript.jsonl"))
    except OSError:
        return out
    for line in lines:
        if '"tool_execution_' not in line:
            continue
        try:
            e = json.loads(line)
        except Exception:
            continue
        kind, tid, name = e.get("type"), e.get("toolCallId"), e.get("toolName") or ""
        if kind == "tool_execution_start":
            a = e.get("args") or {}
            items = []
            if name == "write" and str(a.get("path", "")).startswith("xd://" + PREFIX):
                tool = a["path"].split(PREFIX, 1)[1]
                try:
                    args = json.loads(a.get("content") or "{}")
                except Exception:
                    args = {}
                items.append((tool, args))
            elif name.startswith(PREFIX):
                items.append((name.split(PREFIX, 1)[1], a))
            elif name == "eval":
                code = a.get("code") or ""
                for m in re.finditer(r"tool\." + PREFIX + r"(\w+)\(", code):
                    body = _balanced(code, m.end() - 1)
                    nq = len(re.findall(r'"instructions"', body)) or len(re.findall(r"instructions", body))
                    args = {"questions": nq, "view": bool(re.search(r"view\s*:\s*true", body))}
                    items.append((m.group(1), args))
            if items:
                pending[tid] = (name, items)
        elif kind == "tool_execution_end" and tid in pending:
            name, items = pending.pop(tid)
            text = " ".join(c.get("text", "") for c in (e.get("result") or {}).get("content", [])
                            if isinstance(c, dict))
            err = bool(e.get("isError")) or text.lstrip().startswith(("Error", '{"refused"'))
            for tool, args in items:
                q = args.get("questions")
                nq = len(q) if isinstance(q, dict) else (q if isinstance(q, int) else 0)
                out.append({"tool": bare(tool), "nq": nq, "view": bool(args.get("view")),
                            "refused": err and name != "eval", "text": text})
    return out


def decide_outcomes(calls):
    """Each decide call's own reported numbers, read from its result text."""
    res = []
    for c in calls:
        if c["tool"] != "decide":
            continue
        text = c.get("text", "")
        m = re.search(r'"latency_ms":\s*(\d+)', text)
        w = re.search(r'"where":\s*"(\w+)"', text)
        mo = re.search(r'"model":\s*"([^"]+)"', text)
        res.append({"ok": bool(m), "latency": int(m.group(1)) if m else None,
                    "where": w.group(1) if w else None, "model": mo.group(1) if mo else None,
                    "text": text[:400],
                    "down": ("did not answer" in text) or ("Unreachable" in text)})
    return res


def load(run):
    out = []
    for f in sorted(glob.glob(f"{ROOT}/{run}/*/trial-*/trial.json")):
        t = json.load(open(f))
        t["_dir"] = os.path.dirname(f)
        out.append(t)
    return out


def pct(a, b):
    return f"{a}/{b} = {100 * a / b:.0f}%" if b else "n/a"


def summary(run):
    ts = load(run)
    ok = [t for t in ts if t.get("outcome") != "harness_error"]
    elig, asked4, asked1, anyask = [], [], [], []
    qs, lat, wheres, models, failed, void = [], [], collections.Counter(), collections.Counter(), [], []
    per_task = collections.defaultdict(lambda: [0, 0, 0, 0])
    for t in ok:
        calls = calls_of(t["_dir"])
        muts = [i for i, c in enumerate(calls) if c["tool"] in MUT and not c["refused"]]
        dec = [(i, c) for i, c in enumerate(calls) if c["tool"] == "decide"]
        last = muts[-1] if muts else -1
        pt = per_task[t["task"]]
        pt[0] += 1
        pt[1] += t.get("outcome") == "pass"
        if muts:
            elig.append(t)
            pt[2] += 1
            if any(i > last and c["nq"] >= MIN_Q for i, c in dec):
                asked4.append(t)
                pt[3] += 1
            if any(i > last and c["nq"] >= 1 for i, c in dec):
                asked1.append(t)
        if dec:
            anyask.append(t)
        for _, c in dec:
            qs.append(c["nq"])
        outs = decide_outcomes(calls)
        for r in outs:
            if r["ok"] and r["latency"]:
                lat.append(r["latency"])
                wheres[r["where"]] += 1
                models[r["model"]] += 1
            elif not r["ok"]:
                failed.append((t["task"], r["text"]))
                if r["down"]:
                    void.append(t)
    return dict(run=run, ts=ts, ok=ok, elig=elig, asked4=asked4, asked1=asked1, anyask=anyask,
                qs=qs, lat=lat, wheres=wheres, models=models, failed=failed, void=void, per_task=per_task)


def main():
    runs = sys.argv[1:] or ["ask-revised-guide"]
    for run in runs:
        s = summary(run)
        n = len(s["ts"])
        e = len(s["elig"])
        print(f"## {run}")
        succ = sum(1 for t in s["ok"] if t.get("outcome") == "pass")
        print(f"- trials {n}; scored {len(s['ok'])} ({n - len(s['ok'])} harness errors); success {pct(succ, len(s['ok']))}")
        print(f"- eligible trials (applied at least one visible operation): {e}")
        lo, hi = wilson(len(s["asked1"]), e)
        print(f"- **asked >= 1 question after the last visible operation: {pct(len(s['asked1']), e)}**"
              f" (Wilson 95% [{100*lo:.0f}%, {100*hi:.0f}%])")
        lo4, hi4 = wilson(len(s["asked4"]), e)
        print(f"- asked >= {MIN_Q} questions after the last visible operation: {pct(len(s['asked4']), e)}"
              f" (Wilson 95% [{100*lo4:.0f}%, {100*hi4:.0f}%]) -- the 2026-11-15 bar, for comparison")
        print(f"- trials that called decide at all: {pct(len(s['anyask']), len(s['ok']))}; calls {len(s['qs'])}")
        if s["qs"]:
            print(f"- mean questions per call {sum(s['qs'])/len(s['qs']):.2f}"
                  f" (histogram {dict(sorted(collections.Counter(s['qs']).items()))})")
        if s["lat"]:
            L = sorted(s["lat"])
            print(f"- decided answers with a latency: {len(L)}; median {L[len(L)//2]} ms, mean {sum(L)/len(L):.0f},"
                  f" p90 {L[int(.9*(len(L)-1))]}, max {L[-1]}; where {dict(s['wheres'])}; model {dict(s['models'])}")
        print(f"- failed/refused decision calls: {len(s['failed'])}; endpoint-down trials (void): {len(s['void'])}")
        for task, r in s["failed"][:5]:
            print(f"    - {task}: {str(r)[:160]}")
        print()
        print("| task | n | pass | eligible | asked>=1 |")
        print("|---|---|---|---|---|")
        for task, v in sorted(s["per_task"].items()):
            print(f"| `{task}` | {v[0]} | {v[1]} | {v[2]} | {v[3]} |")
        print()


if __name__ == "__main__":
    main()
