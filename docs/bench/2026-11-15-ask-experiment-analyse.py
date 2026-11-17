"""Ask behaviour and its effect, from the two ask-*-guide run directories (DM-06, #457). Prints markdown."""
import json, glob, os, re, sys, random, collections, math
ROOT = os.path.expanduser("~/.local/state/dark_factory-bench")
MUT = {"blueprint_apply_operation"}          # the calls that change what the person sees
MIN_Q = 4

def wilson(k, n, z=1.96):
    if n == 0: return (0, 0)
    p = k / n; d = 1 + z*z/n
    c = (p + z*z/(2*n)) / d; h = z*math.sqrt(p*(1-p)/n + z*z/(4*n*n)) / d
    return (max(0, c-h), min(1, c+h))

def load(arm):
    """The arm's trials: the final spliced directory (ask-<arm>-guide-final) when it exists, else the raw run."""
    out = []
    run = f"ask-{arm}-guide-final" if os.path.isdir(f"{ROOT}/ask-{arm}-guide-final") else f"ask-{arm}-guide"
    for f in sorted(glob.glob(f"{ROOT}/{run}/*/trial-*/trial.json")):
        t = json.load(open(f)); t["_dir"] = os.path.dirname(f); out.append(t)
    return out

def nq(a):
    q = a.get("questions") if isinstance(a, dict) else None
    return len(q) if isinstance(q, (dict, list)) else 0

def _balanced(code, i):
    """The text of the (...) call whose '(' is at code[i], respecting string quotes."""
    depth, q, j = 0, None, i
    while j < len(code):
        ch = code[j]
        if q:
            if ch == "\\": j += 1
            elif ch == q: q = None
        elif ch in "\"'`": q = ch
        elif ch == "(": depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0: return code[i:j+1]
        j += 1
    return code[i:]

def calls_of(d):
    """Every editor tool call the agent made, in order, from the transcript alone:
    dict(tool, nq, view, refused, text). Routes: write xd://mcp__dark_factory_<tool>, a direct mcp__dark_factory_<tool>, and eval's tool.mcp__dark_factory_<tool>(...)."""
    out, pending = [], {}
    try: lines = open(os.path.join(d, "transcript.jsonl"))
    except OSError: return out
    for l in lines:
        if '"tool_execution_' not in l: continue
        try: e = json.loads(l)
        except Exception: continue
        kind, tid, name = e.get("type"), e.get("toolCallId"), e.get("toolName") or ""
        if kind == "tool_execution_start":
            a = e.get("args") or {}
            items = []
            if name == "write" and str(a.get("path", "")).startswith("xd://mcp__dark_factory_"):
                tool = a["path"].split("mcp__dark_factory_", 1)[1]
                try: args = json.loads(a.get("content") or "{}")
                except Exception: args = {}
                items.append((tool, args))
            elif name.startswith("mcp__dark_factory_"):
                items.append((name.split("mcp__dark_factory_", 1)[1], a))
            elif name == "eval":
                code = a.get("code") or ""
                for m in re.finditer(r"tool\.mcp__dark_factory_(\w+)\(", code):
                    body = _balanced(code, m.end() - 1)
                    args = {"questions": {"?%d" % k: 1 for k in range(len(re.findall(r"instructions", body)))}, "view": bool(re.search(r"view\s*:\s*true", body))} if m.group(1) == "blueprint_decide" else {}
                    items.append((m.group(1), args))
            if items: pending[tid] = (name, items)
        elif kind == "tool_execution_end" and tid in pending:
            name, items = pending.pop(tid)
            text = " ".join(c.get("text", "") for c in (e.get("result") or {}).get("content", []) if isinstance(c, dict))
            err = bool(e.get("isError")) or text.lstrip().startswith(("Error", '{"refused"'))
            for tool, args in items:
                if "Check your work by asking" in text: text = ""
                out.append(dict(tool=tool, nq=nq(args), view=isinstance(args, dict) and args.get("view") is True,
                                refused=err and name != "eval", text=text, route=name))
    return out

def decide_outcomes(calls):
    res = []
    for c in calls:
        if c["tool"] != "blueprint_decide": continue
        t = c["text"]
        m = re.search(r'"latency_ms":\s*(\d+)', t); w = re.search(r'"where":\s*"(\w+)"', t); mo = re.search(r'"model":\s*"([^"]+)"', t)
        res.append(dict(ok=bool(m), latency=int(m.group(1)) if m else None, where=w.group(1) if w else None,
                        model=mo.group(1) if mo else None, text=t[:400], down=("did not answer" in t) or ("Unreachable" in t)))
    return res

def arm_stats(arm):
    ts = load(arm)
    ok = [t for t in ts if t["outcome"] != "harness_error"]
    elig, asked, asked1, anyask, qs, view_calls, ncalls = [], [], [], [], [], 0, 0
    lat, wheres, models, failed, down = [], collections.Counter(), collections.Counter(), [], 0
    per_task = collections.defaultdict(lambda: [0, 0, 0, 0])
    void_trials = []
    for t in ok:
        calls = calls_of(t["_dir"])
        muts = [i for i, c in enumerate(calls) if c["tool"] in MUT and not c["refused"]]
        dec = [(i, c) for i, c in enumerate(calls) if c["tool"] == "blueprint_decide"]
        is_elig = bool(muts); last = muts[-1] if muts else -1
        after = [c for i, c in dec if i > last and c["nq"] >= MIN_Q]
        pt = per_task[t["task"]]; pt[0] += 1; pt[1] += t["outcome"] == "pass"
        if is_elig: elig.append(t); pt[2] += 1
        if is_elig and after: asked.append(t); pt[3] += 1
        if is_elig and any(i > last and c["nq"] >= 1 for i, c in dec): asked1.append(t)
        if dec: anyask.append(t)
        for i, c in dec:
            ncalls += 1; qs.append(c["nq"]); view_calls += c["view"]
        outs = decide_outcomes(calls)
        for r in outs:
            if r["ok"]: lat.append(r["latency"]); wheres[r["where"]] += 1; models[r["model"]] += 1
            else:
                failed.append((t["task"], r["text"]))
                if r["down"]: down += 1
        if any(r["down"] for r in outs): void_trials.append(t)
    return dict(ts=ts, ok=ok, elig=elig, asked=asked, asked1=asked1, anyask=anyask, calls=ncalls, qs=qs, view=view_calls,
                lat=lat, wheres=wheres, models=models, failed=failed, down=down, void_trials=void_trials, per_task=per_task)

def pct(a, b): return f"{a}/{b} = {100*a/b:.0f}%" if b else "n/a"

if __name__ == "__main__":
    A = arm_stats("with"); B = arm_stats("without")
    for name, s in (("with the DM-05 guide", A), ("without it", B)):
        n = len(s["ok"]); e = len(s["elig"]); a = len(s["asked"]); lo, hi = wilson(a, e)
        succ = sum(t["outcome"] == "pass" for t in s["ok"]); slo, shi = wilson(succ, n)
        print(f"## {name}")
        print(f"- trials scored {n} of {len(s['ts'])} ({len(s['ts'])-n} harness errors); success {pct(succ, n)} (Wilson 95% [{100*slo:.0f}%, {100*shi:.0f}%])")
        print(f"- eligible trials (a visible operation applied): {e}; asked after the last visible operation with >= {MIN_Q} questions: {pct(a, e)} (Wilson 95% [{100*lo:.0f}%, {100*hi:.0f}%])")
        vis = [t for t in s["elig"] if "data" not in (t.get("tags") or [])]
        v4 = sum(1 for t in vis if t in s["asked"]); v1 = sum(1 for t in vis if t in s["asked1"])
        print(f"- asked after the last visible operation with >= 1 question: {pct(len(s['asked1']), e)}; excluding the `data` task (an invisible result): >= {MIN_Q} questions {pct(v4, len(vis))}, >= 1 question {pct(v1, len(vis))}")
        print(f"- trials that called blueprint_decide at all: {pct(len(s['anyask']), n)}; calls {s['calls']} (view:true {s['view']}); mean questions per call {sum(s['qs'])/len(s['qs']):.2f}" if s["qs"] else f"- calls 0")
        if s["lat"]:
            L = sorted(s["lat"]); print(f"- decision calls answered {len(L)}; latency ms median {L[len(L)//2]}, mean {sum(L)/len(L):.0f}, p90 {L[int(.9*(len(L)-1))]}, max {L[-1]}; where {dict(s['wheres'])}; model {dict(s['models'])}")
        print(f"- failed or refused decision calls: {len(s['failed'])}, of which the endpoint did not answer (down): {s['down']}, in {len(s['void_trials'])} trial(s)")
        for t, x in s["failed"][:5]: print(f"    - {t}: {x[:160]}")
        print()
    # what asking changed, within the with-guide arm: success of eligible trials that asked vs did not
    for name, s in (("with", A), ("without", B)):
        asked_ids = {id(t) for t in s["asked"]}
        yes = [t for t in s["elig"] if id(t) in asked_ids]; no = [t for t in s["elig"] if id(t) not in asked_ids]
        f = lambda xs: pct(sum(t["outcome"] == "pass" for t in xs), len(xs))
        mean = lambda xs, k: (sum((t.get(k) or 0) for t in xs) / len(xs)) if xs else float("nan")
        print(f"- {name} arm, eligible trials that asked: success {f(yes)}, mean wall {mean(yes,'wall_s'):.0f} s, mean tool calls {mean(yes,'tool_call_count'):.1f}; did not ask: success {f(no)}, mean wall {mean(no,'wall_s'):.0f} s, mean tool calls {mean(no,'tool_call_count'):.1f}")
    print()
    print("| task | with: n pass eligible asked | without: n pass eligible asked |")
    print("|---|---|---|")
    for task in sorted(set(A["per_task"]) | set(B["per_task"])):
        a = A["per_task"].get(task, [0]*4); b = B["per_task"].get(task, [0]*4)
        print(f"| `{task}` | {a[0]} {a[1]} {a[2]} {a[3]} | {b[0]} {b[1]} {b[2]} {b[3]} |")
