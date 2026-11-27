#!/usr/bin/env python3
"""A narrow window: what is cut off, per workspace."""
import json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
s = state(); win = s.get("window") or {}
say("window:", json.dumps(win)[:200])
W = H = None
q = brp("world.query", {"data": {"components": ["bevy_window::window::Window"]}})
try:
    r = q[0]["components"]["bevy_window::window::Window"]["resolution"]; W, H = r["physical_width"], r["physical_height"]; sc = r.get("scale_factor")
    say(f"window {W}x{H} physical, scale {sc}")
except Exception as e:
    say("window query:", str(e)[:120])
for ws in ("map", "kit", "animation", "files"):
    run(f"workspace.{ws}"); time.sleep(1.3); move(W // 2, H // 2); time.sleep(0.3)
    vp = (state().get("rects") or {}).get("viewport") or [0, 0, 0, 0]
    art = 100 * vp[2] * vp[3] * 1.44 / (W * H)
    texts = [w for w in widgets() if w["size"][0] > 0 and w["size"][1] > 0 and w["rect"][1] < H and w["rect"][3] > 0 and w["rect"][0] < W]
    cut = [w for w in texts if w["rect"][2] > W + 1 or w["rect"][0] < -1]
    say(f"{ws.upper():10s} art {art:5.2f}%  viewport {[round(v) for v in vp]}  texts on screen {len(texts)}, crossing the window's edge {len(cut)}: {[w['words'][:22] for w in cut][:6]}")
    shot(f"n_{ws}")
run("workspace.map"); time.sleep(1.0)
