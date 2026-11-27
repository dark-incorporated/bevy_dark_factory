#!/usr/bin/env python3
"""#667's own setup: terminal at a quarter, then Ctrl+B and Zen."""
import json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
def term(): return (state().get("terminal") or {}).get("placement")
run("workspace.map"); time.sleep(1.0); move(700, 450)
say(f"start: terminal {term()}, share {share()}%")
n = 0
while term() != "quarter" and n < 4: chord("KeyJ", ("ControlLeft",)); time.sleep(0.6); n += 1
say(f"terminal {term()}: share {share()}%  rect {json.dumps((state().get('rects') or {}).get('viewport'))}")
chord("KeyB", ("ControlLeft",)); time.sleep(0.8); say(f"   after Ctrl+B: share {share()}%  rect {json.dumps((state().get('rects') or {}).get('viewport'))}")
chord("KeyB", ("ControlLeft",)); time.sleep(0.8)
chord("KeyM", ("ControlLeft", "ShiftLeft")); time.sleep(0.8); say(f"   after Zen:    share {share()}%  rect {json.dumps((state().get('rects') or {}).get('viewport'))}  terminal {term()}"); shot("h_zen_quarter")
chord("KeyM", ("ControlLeft", "ShiftLeft")); time.sleep(0.8); say(f"   restored:     share {share()}%  terminal {term()}")
n = 0
while term() != "collapsed" and n < 4: chord("KeyJ", ("ControlLeft",)); time.sleep(0.6); n += 1
say(f"end: terminal {term()}, share {share()}%")
