#!/usr/bin/env python3
"""Interactions on a quiet host: what each costs in the editor's own busy_ms, and what each does to the art's share."""
import collections, json, os, statistics, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
R = {}
def med(xs): return round(statistics.median(xs), 1) if xs else None

run("workspace.map"); time.sleep(1.5)
say("host load:", open("/proc/loadavg").read().split()[:3])

# 1. who carries a TabIndex
try:
    q = brp("world.query", {"data": {"components": ["bevy_input_focus::tab_navigation::TabIndex"], "option": ["bevy_gooey::skin::Role"]}})
    roles = collections.Counter()
    for row in q or []:
        r = (row.get("components") or {}).get("bevy_gooey::skin::Role")
        roles[r if isinstance(r, str) else json.dumps(r)[:30]] += 1
    say("TabIndex by Role on MAP:", roles.most_common(12), "total", len(q or []))
    R["tabindex_roles"] = roles.most_common(12)
except Exception as e:
    say("tabindex role query:", str(e)[:200])
try:
    say("InputFocus before Tab:", json.dumps(brp("world.get_resources", {"resource": "bevy_input_focus::InputFocus"}))[:160])
    for _ in range(3):
        chord("Tab"); time.sleep(0.15)
    say("InputFocus after 3 Tab:", json.dumps(brp("world.get_resources", {"resource": "bevy_input_focus::InputFocus"}))[:160])
    say("ui after Tab:", json.dumps(state().get("ui")))
except Exception as e:
    say("focus probe:", str(e)[:200])

# 2. workspace switches, 10 round trips each
for other in ("kit", "files", "animation", "game"):
    worst, wall = [], []
    for i in range(10):
        for target in (other, "map"):
            t0 = time.perf_counter()
            s = spike(lambda: run(f"workspace.{target}"), frames=8)
            worst.append(s["worst_busy_ms"]); wall.append(s["call_ms"])
            time.sleep(0.25)
    worst.sort()
    R[f"switch_{other}"] = {"busy_p50": med(worst), "busy_worst": worst[-1], "n": len(worst), "call_p50": med(wall)}
    say(f"switch map<->{other}: worst-frame busy_ms p50 {med(worst)}  worst {worst[-1]}  (n={len(worst)}); BRP call p50 {med(wall)} ms")

# 3. palette
try:
    s = spike(lambda: run("palette.open")); say("palette.open:", s); time.sleep(0.4)
    per = []
    for ch in "placement":
        per.append(spike(lambda: type_text(ch), frames=6)["worst_busy_ms"]); time.sleep(0.08)
    say("palette typing 'placement', worst busy_ms per key:", per)
    shot("b_palette")
    chord("Escape"); time.sleep(0.4)
    R["palette"] = {"open": s, "keys": per}
except Exception as e:
    say("palette:", str(e)[:200])

# 4. an operation, undo, redo
try:
    ap = spike(lambda: brp("blueprint/operation_apply", {"operation": "placement_set_yaw", "parameters": {"id": "sofa", "yaw": 90}}))
    say("placement_set_yaw:", ap)
    u, r = [], []
    for i in range(6):
        u.append(spike(lambda: run("edit.undo"), frames=6)["worst_busy_ms"]); time.sleep(0.15)
        r.append(spike(lambda: run("edit.redo"), frames=6)["worst_busy_ms"]); time.sleep(0.15)
    say("undo worst busy_ms:", u, " redo:", r)
    run("edit.undo"); R["undo"] = u; R["redo"] = r
except Exception as e:
    say("ops:", str(e)[:200])

# 5. the terminal and the two controls that give the window back
def snap(tag):
    s = state(); t = s.get("terminal") or {}
    say(f"   {tag}: share {share(s)}%  viewport {[round(v) for v in (s.get('rects') or {}).get('viewport') or []]}  terminal {t.get('placement')}")
    return share(s)
base = snap("baseline")
for i in range(3):
    sp = spike(lambda: chord("KeyJ", ("ControlLeft",)), frames=10); time.sleep(0.6)
    snap(f"Ctrl+J #{i+1} (worst busy {sp['worst_busy_ms']} ms)")
    if i == 0: shot("b_terminal1")
chord("KeyM", ("ControlLeft", "ShiftLeft")); time.sleep(0.8); z = snap("Shift+Ctrl+M (Zen)"); shot("b_zen")
chord("KeyM", ("ControlLeft", "ShiftLeft")); time.sleep(0.8); snap("Shift+Ctrl+M again")
chord("KeyB", ("ControlLeft",)); time.sleep(0.8); lp = snap("Ctrl+B (left panel)"); shot("b_leftpanel")
chord("KeyB", ("ControlLeft",)); time.sleep(0.8); snap("Ctrl+B again")
R["share"] = {"baseline": base, "zen": z, "left_panel": lp}
json.dump(R, open(os.path.expanduser("~/fleetscratch/rv5/sweep_b.json"), "w"), indent=1)
