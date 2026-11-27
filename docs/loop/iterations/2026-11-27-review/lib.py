"""Review 2026-11-27 drive helpers, on top of drive.py beside this file."""
import json, os, statistics, sys, time
os.environ.setdefault("PORT", "15791"); os.environ.setdefault("OUT", os.path.expanduser("~/fleetscratch/rv5/crops"))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from drive import brp, shot, state, say, widgets, key, move, click, type_text, scale_factor  # noqa

def run(cmd):
    return brp("blueprint/run_command", {"id": cmd})

def chord(k, mods=()):
    """Press/Release, never Tap (bevy_fairy#16: Tap answers success and delivers nothing)."""
    for m in mods:
        key(m, "Press"); time.sleep(0.04)
    key(k, "Press"); time.sleep(0.06); key(k, "Release"); time.sleep(0.04)
    for m in reversed(mods):
        key(m, "Release"); time.sleep(0.04)

def share(s=None):
    r = (s or state()).get("rects") or {}
    v, w = r.get("viewport"), r.get("window")
    if not v or not w:
        return None
    return round(100.0 * v[2] * v[3] / (w[2] * w[3]), 2)

def busy(n=40, gap=0.03):
    """busy_ms over n distinct frames: (p50, p90, max, frames)."""
    seen, out = set(), []
    deadline = time.time() + 12
    while len(out) < n and time.time() < deadline:
        f = state().get("frame") or {}
        c, b = f.get("frame_count"), f.get("busy_ms")
        if c not in seen and b is not None:
            seen.add(c); out.append(b)
        time.sleep(gap)
    if not out:
        return None
    out.sort()
    return {"p50": round(statistics.median(out), 2), "p90": round(out[int(0.9 * (len(out) - 1))], 2), "max": round(out[-1], 2), "n": len(out)}

def spike(action, frames=14, gap=0.012):
    """Run action(), then report the worst busy_ms over the next frames and the wall time until the first new frame."""
    f0 = (state().get("frame") or {}).get("frame_count")
    t0 = time.perf_counter(); action(); t1 = time.perf_counter()
    seen, worst, heavy = set(), 0.0, None
    deadline = time.time() + 6
    while len(seen) < frames and time.time() < deadline:
        f = state().get("frame") or {}
        c = f.get("frame_count")
        if c is not None and c != f0 and c not in seen:
            seen.add(c)
            if (f.get("busy_ms") or 0) > worst:
                worst = f.get("busy_ms") or 0; heavy = f.get("heaviest_span")
        time.sleep(gap)
    return {"call_ms": round(1000 * (t1 - t0), 1), "worst_busy_ms": round(worst, 2), "heaviest": heavy}

def tabindex_census():
    for path in ("bevy_input_focus::tab_navigation::TabIndex", "bevy_input_focus::tabnav::TabIndex", "bevy_ui::focus::TabIndex"):
        try:
            q = brp("world.query", {"data": {"components": [path]}})
            return path, len(q or [])
        except Exception as e:
            last = str(e)[:120]
    return None, last
