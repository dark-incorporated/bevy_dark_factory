#!/usr/bin/env python3
"""#677/#678/#541: both frame clocks and the schedule split, settled and across 20 Map<->Kit and 20 Map<->World switches."""
import json, os, statistics, sys, time, urllib.request

PORT = int(os.environ.get("PORT", "15877"))


def brp(method, params=None, timeout=60):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}).encode()
    req = urllib.request.Request(f"http://127.0.0.1:{PORT}/", data=body, method="POST", headers={"Content-Type": "application/json"})
    r = json.loads(urllib.request.urlopen(req, timeout=timeout).read())
    if "error" in r:
        raise RuntimeError(f"{method}: {r['error']}")
    return r.get("result")


def frame():
    return (brp("blueprint/state") or {}).get("frame") or {}


def med(xs):
    return round(statistics.median(xs), 2)


def p90(xs):
    xs = sorted(xs)
    return round(xs[int(0.9 * (len(xs) - 1))], 2)


def split(f, top=4):
    s = sorted(f.get("schedules") or [], key=lambda s: -s["ms"])[:top]
    return ", ".join(f"{x['schedule']} {x['ms']:.1f}" for x in s)


st = brp("blueprint/state")
print("jobs:", json.dumps(st.get("jobs"))[:160], "| workspace:", (st.get("ui") or {}).get("workspace"))
os.system("uptime")

# 1. Settled: 50 distinct frames in the MAP workspace.
seen, rows = set(), []
while len(rows) < 50:
    f = frame()
    c = f.get("frame_count")
    if c not in seen and f.get("busy_ms") is not None and f.get("frame_ms") is not None:
        seen.add(c)
        rows.append(f)
    time.sleep(0.02)
fm = [f["frame_ms"] for f in rows]
bm = [f["busy_ms"] for f in rows]
gap = [a - b for a, b in zip(fm, bm)]
print(f"SETTLED n={len(rows)} frame_ms p50 {med(fm)} p90 {p90(fm)} | busy_ms p50 {med(bm)} p90 {p90(bm)} | frame-busy p50 {med(gap)} min {min(gap):.2f} | equal within 0.1 ms: {sum(1 for g in gap if abs(g) < 0.1)}/{len(gap)}")
per = {}
for f in rows:
    for s in f.get("schedules") or []:
        per.setdefault(s["schedule"], []).append(s["ms"])
print("SETTLED split p50:", ", ".join(f"{k} {med(v)}" for k, v in sorted(per.items(), key=lambda kv: -statistics.median(kv[1]))[:6]))

# 2. Switches: 20 Map <-> Kit, then 20 Map <-> World (OTHERS overrides), each through the command the chord runs (door_open).
def p50_split(fs):
    per = {}
    for f in fs:
        for s in f.get("schedules") or []:
            per.setdefault(s["schedule"], []).append(s["ms"])
    return ", ".join(f"{k} {med(v)}" for k, v in sorted(per.items(), key=lambda kv: -statistics.median(kv[1]))[:6])


def switches(other):
    out = {other: [], "map": []}
    for i in range(20):
        target = other if i % 2 == 0 else "map"
        f0 = frame().get("frame_count")
        r = brp("blueprint/run_command", {"id": f"workspace.{target}"})
        outcome = (r or {}).get("outcome") if isinstance(r, dict) else r
        frames = {}
        deadline = time.time() + 1.5
        while time.time() < deadline:
            f = frame()
            c = f.get("frame_count")
            if c != f0 and c not in frames and f.get("busy_ms") is not None:
                frames[c] = f
            time.sleep(0.003)
        worst = max(frames.values(), key=lambda f: f["busy_ms"])
        ws = (brp("blueprint/state").get("ui") or {}).get("workspace")
        out[target].append(worst)
        print(f"switch {i+1:2d} -> {target:5s} {str(outcome)[:12]:12s} now {ws}: worst frame frame_ms {worst['frame_ms']:.1f} busy_ms {worst['busy_ms']:.1f} | {split(worst)}")
        time.sleep(0.4)
    both = out[other] + out["map"]
    for label, fs in ((f"map<->{other}", both), (f"-> {other}", out[other]), ("-> map", out["map"])):
        wf = [f["frame_ms"] for f in fs]
        wb = [f["busy_ms"] for f in fs]
        print(f"SWITCH {label} n={len(fs)} worst-frame frame_ms p50 {med(wf)} p90 {p90(wf)} max {max(wf):.1f} | busy_ms p50 {med(wb)} p90 {p90(wb)} max {max(wb):.1f} | over 100 ms busy: {sum(1 for b in wb if b > 100)}/{len(wb)}")
        print(f"SWITCH {label} split p50:", p50_split(fs))
    os.system("uptime")


for other in os.environ.get("OTHERS", "kit world").split():
    switches(other)
