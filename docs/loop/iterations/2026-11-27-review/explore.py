#!/usr/bin/env python3
"""First contact: what the running editor says about itself."""
import json, os, sys, time
os.environ["PORT"] = "15791"; os.environ["OUT"] = os.path.expanduser("~/fleetscratch/rv5/crops")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from drive import brp, shot, state, say, scale_factor
s = state()
say("top-level keys:", sorted(s.keys()))
for k in ("workspace", "screen", "ui", "rects", "frame", "terminal", "shell", "editor"):
    if k in s:
        say(f"--- {k}:", json.dumps(s[k])[:700])
say("scale/physical:", scale_factor())
try:
    brp("blueprint/run_command", {"id": "nope.nope"})
except Exception as e:
    say("run_command refusal:", str(e)[:900])
methods = brp("rpc.discover", {})
names = sorted(m["name"] for m in methods.get("methods", []))
say("methods:", [n for n in names if not n.startswith("world.") and not n.startswith("rpc.")])
p, size = shot("first")
say("shot:", p, size)
