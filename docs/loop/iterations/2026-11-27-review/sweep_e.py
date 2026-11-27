#!/usr/bin/env python3
"""Which act ends the slow workspace switch? One fresh editor, one candidate at a time."""
import json, os, statistics, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
def med(xs): return round(statistics.median(xs), 1)
def focus(): return (brp("world.get_resources", {"resource": "bevy_input_focus::InputFocus"}) or {}).get("value", {}).get("current_focus")
def hitch(action, calls=8):
    f0 = (state().get("frame") or {}).get("frame_count")
    t = time.perf_counter(); action(); longest = 1000 * (time.perf_counter() - t); worst = 0.0
    for _ in range(calls):
        t = time.perf_counter(); f = state().get("frame") or {}; longest = max(longest, 1000 * (time.perf_counter() - t))
        if f.get("frame_count") != f0: worst = max(worst, f.get("busy_ms") or 0)
    return round(longest, 1), round(worst, 1)
def switches(label, n=4):
    wall, busy = [], []
    for i in range(n):
        for target in ("kit", "map"):
            w, b = hitch(lambda: run(f"workspace.{target}")); wall.append(w); busy.append(b); time.sleep(0.25)
    say(f"{label:44s} wait p50 {med(wall):6.1f} ms  worst {max(wall):6.1f} | busy_ms p50 {med(busy):6.1f}  worst {max(busy):6.1f}  jobs {len((state().get('jobs') or []))}")
    return med(busy)

t0 = time.time()
say("jobs at start:", json.dumps(state().get("jobs"))[:200])
switches("1 fresh")
time.sleep(75); say(f"   (+{time.time()-t0:.0f}s) jobs:", json.dumps(state().get("jobs"))[:120])
switches("2 after 75 s idle")
run("palette.open"); time.sleep(0.5); chord("Escape"); time.sleep(0.5)
switches("3 after the palette opened and closed")
for _ in range(3): chord("KeyJ", ("ControlLeft",)); time.sleep(0.5)
switches("4 after Ctrl+J three times")
chord("KeyM", ("ControlLeft", "ShiftLeft")); time.sleep(0.7); chord("KeyM", ("ControlLeft", "ShiftLeft")); time.sleep(0.7)
switches("5 after Zen on and off")
chord("KeyB", ("ControlLeft",)); time.sleep(0.7); chord("KeyB", ("ControlLeft",)); time.sleep(0.7)
switches("6 after Ctrl+B on and off")
chord("KeyF", ("ControlLeft", "ShiftLeft")); time.sleep(0.6); chord("KeyF", ("ControlLeft", "ShiftLeft")); time.sleep(0.6)
switches("7 after the profile on and off")

# a focused button and the keyboard, on an editor nobody pressed Tab in
run("workspace.map"); time.sleep(1.0)
for yaw in (90, 180, 270):
    r = brp("blueprint/operation_apply", {"operation": "placement_set_yaw", "parameters": {"id": "sofa", "yaw": yaw}}); time.sleep(0.3)
say("last operation_apply answer:", json.dumps(r)[:160])
def depths():
    e = state().get("editor") or {}; return e.get("undo_depth"), e.get("redo_depth")
d0 = depths(); click(411, 88); time.sleep(0.5); d1 = depths(); f1 = focus()
key("Enter", "Press"); time.sleep(0.08); key("Enter", "Release"); time.sleep(0.5); d2 = depths()
key("Space", "Press"); time.sleep(0.08); key("Space", "Release"); time.sleep(0.5); d3 = depths()
say(f"undo depth: before {d0} | after clicking the toolbar's undo arrow {d1} (InputFocus {f1}) | after Enter {d2} | after Space {d3}")
while (depths()[0] or 0) > 0: run("edit.undo"); time.sleep(0.2)

r = brp("blueprint/ui_style_inspect", {"x": 200, "y": 540}); say("inspector hole:", json.dumps(r)[:700])
