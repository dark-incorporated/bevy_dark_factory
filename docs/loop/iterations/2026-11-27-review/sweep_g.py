#!/usr/bin/env python3
"""Which text entities survive a workspace round trip, before and after the profile is toggled once?"""
import os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
def ids():
    ws = widgets(); return {w["entity"] for w in ws}, {w["entity"] for w in ws if w["size"][0] > 0 and w["size"][1] > 0}
def nodes():
    q = brp("world.query", {"data": {"components": ["bevy_ui::ui_node::ComputedNode"]}}); return {r["entity"] for r in q or []}
def trip():
    worst = []
    for target in ("kit", "map"):
        f0 = (state().get("frame") or {}).get("frame_count"); run(f"workspace.{target}"); w = 0
        for _ in range(8):
            f = state().get("frame") or {}
            if f.get("frame_count") != f0: w = max(w, f.get("busy_ms") or 0)
        worst.append(round(w, 1)); time.sleep(0.4)
    return worst
def step(label, act):
    t0, s0 = ids(); n0 = nodes(); out = act(); time.sleep(0.6); t1, s1 = ids(); n1 = nodes()
    say(f"{label:34s} nodes {len(n0)} -> {len(n1)} (kept {len(n0 & n1)}, new {len(n1 - n0)}, gone {len(n0 - n1)}) | texts {len(t0)} -> {len(t1)} (kept {len(t0 & t1)}) | sized texts kept {len(s0 & s1)} of {len(s0)} | {out if out else ''}")
run("workspace.map"); time.sleep(1.0)
step("nothing, 1 s", lambda: time.sleep(1.0))
step("round trip map-kit-map", trip)
step("round trip again", trip)
step("profile on", lambda: chord("KeyF", ("ControlLeft", "ShiftLeft")))
step("profile off", lambda: chord("KeyF", ("ControlLeft", "ShiftLeft")))
step("round trip after the toggle", trip)
step("round trip again", trip)
