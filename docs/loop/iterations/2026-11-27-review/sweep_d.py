#!/usr/bin/env python3
"""A clean A/B of the workspace switch on a freshly started editor, with a clock that is not the editor's own;
then whether a focused button answers the keyboard; then the inspector's blank band by name."""
import json, os, statistics, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
def med(xs): return round(statistics.median(xs), 1)
def focus(): return (brp("world.get_resources", {"resource": "bevy_input_focus::InputFocus"}) or {}).get("value", {}).get("current_focus")

def hitch(action, calls=10):
    """The independent clock: wall time of each of the BRP calls after the action. A frame that takes 176 ms
    makes one of them wait that long. Also what the editor reports for the same frames."""
    f0 = (state().get("frame") or {}).get("frame_count")
    t = time.perf_counter(); action(); longest = 1000 * (time.perf_counter() - t)
    worst = 0.0
    for _ in range(calls):
        t = time.perf_counter(); f = state().get("frame") or {}; dt = 1000 * (time.perf_counter() - t)
        longest = max(longest, dt)
        if f.get("frame_count") != f0: worst = max(worst, f.get("busy_ms") or 0)
    return round(longest, 1), round(worst, 1)

def switches(label, n=10):
    wall, busy = [], []
    for i in range(n):
        for target in ("kit", "map"):
            w, b = hitch(lambda: run(f"workspace.{target}")); wall.append(w); busy.append(b); time.sleep(0.25)
    wall.sort(); busy.sort()
    say(f"{label}: longest BRP wait p50 {med(wall)} ms, worst {wall[-1]} ms | editor's busy_ms p50 {med(busy)}, worst {busy[-1]}  (n={len(wall)})")
    return med(wall), med(busy)

say("load:", open("/proc/loadavg").read().split()[:3], " profile:", (state().get("frame") or {}).get("profile_recording"))
idle = sorted(hitch(lambda: None)[0] for _ in range(10)); say(f"at rest: longest BRP wait p50 {med(idle)} ms, worst {idle[-1]} ms")
a = switches("A  fresh editor, profile off")
chord("KeyF", ("ControlLeft", "ShiftLeft")); time.sleep(0.6)
say("   profile recording now:", (state().get("frame") or {}).get("profile_recording"))
b = switches("B  profile on")
chord("KeyF", ("ControlLeft", "ShiftLeft")); time.sleep(0.6)
c = switches("C  profile off again")
for _ in range(3): chord("Tab"); time.sleep(0.15)
d = switches("D  after three Tab presses")
say("ui:", json.dumps(state().get("ui")), " focus:", focus())

# does a button that a click focused then answer Enter or Space?
run("workspace.map"); time.sleep(1.0)
for yaw in (90, 180, 270):
    brp("blueprint/operation_apply", {"operation": "placement_set_yaw", "parameters": {"id": "sofa", "yaw": yaw}}); time.sleep(0.3)
def depths():
    e = state().get("editor") or {}; return e.get("undo_depth"), e.get("redo_depth")
say("undo/redo depth after three operations:", depths())
undo_btn = None
try:
    q = brp("bevy_fairy/ui_text", {})
    say("ui_text keys:", list(q.keys())[:6] if isinstance(q, dict) else type(q))
except Exception as e:
    say("ui_text:", str(e)[:120])
click(411, 88); time.sleep(0.4); d1 = depths(); f1 = focus()
say(f"clicked the toolbar's undo arrow at (411,88): depth {d1}, InputFocus {f1}")
key("Enter", "Press"); time.sleep(0.08); key("Enter", "Release"); time.sleep(0.4); d2 = depths()
key("Space", "Press"); time.sleep(0.08); key("Space", "Release"); time.sleep(0.4); d3 = depths()
say(f"then Enter: depth {d2}   then Space: depth {d3}   -> a focused button {'ANSWERS' if (d2 != d1 or d3 != d2) else 'does not answer'} the keyboard")
while (depths()[0] or 0) > 0: run("edit.undo"); time.sleep(0.2)

# what is the inspector's blank band, and what is the divider
for name, (x, y) in {"inspector hole": (200, 540), "inspector hole lower": (200, 600), "divider": (402, 300), "toolbar": (700, 88), "tab row": (700, 131), "crumb bar": (700, 48)}.items():
    try:
        r = brp("blueprint/ui_style_inspect", {"x": x, "y": y})
        node = r.get("node") or r
        say(f"   {name} @({x},{y}):", json.dumps({k: node.get(k) for k in ("classes", "size", "role", "label", "kind") if k in node})[:200] or json.dumps(r)[:200])
    except Exception as e:
        say(f"   {name}: {str(e)[:160]}")
shot("d_map")
