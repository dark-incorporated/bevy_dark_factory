#!/usr/bin/env python3
"""What changes in the UI tree when the frame profile is toggled once?"""
import collections, json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
def census(tag):
    ws = widgets()
    laid = [w for w in ws if w["size"][0] > 0 and w["size"][1] > 0]
    onwin = [w for w in laid if w["rect"][2] > 0 and w["rect"][0] < 1360 and w["rect"][3] > 0 and w["rect"][1] < 850]
    fp = [w for w in ws if w["words"].startswith("FRAME PROFILE")]
    say(f"{tag}: texts {len(ws)}, with a size {len(laid)}, inside the window {len(onwin)} | FRAME PROFILE title: {[(w['size'], [round(v) for v in w['rect']]) for w in fp]}")
    return {w["entity"]: w for w in laid}
def nodes(tag):
    q = brp("world.query", {"data": {"components": ["bevy_ui::ui_node::ComputedNode"], "option": ["bevy_ui::ui_node::Node"]}})
    n = len(q or []); sized = 0; disp = collections.Counter()
    for r in q or []:
        c = r["components"]; sz = (c.get("bevy_ui::ui_node::ComputedNode") or {}).get("size") or [0, 0]
        if sz[0] > 0 and sz[1] > 0: sized += 1
        d = (c.get("bevy_ui::ui_node::Node") or {}).get("display"); disp[d] += 1
    say(f"{tag}: UI nodes {n}, with a size {sized}, display {dict(disp)}")
    return n, sized
def one_switch():
    out = []
    for target in ("kit", "map", "kit", "map"):
        f0 = (state().get("frame") or {}).get("frame_count"); run(f"workspace.{target}"); worst = 0
        for _ in range(8):
            f = state().get("frame") or {}
            if f.get("frame_count") != f0: worst = max(worst, f.get("busy_ms") or 0)
        out.append(round(worst, 1)); time.sleep(0.25)
    return out
a = census("fresh      "); na = nodes("fresh      "); say("   switch busy_ms:", one_switch())
chord("KeyF", ("ControlLeft", "ShiftLeft")); time.sleep(0.8)
b = census("profile on "); nb = nodes("profile on ")
chord("KeyF", ("ControlLeft", "ShiftLeft")); time.sleep(0.8)
c = census("profile off"); nc = nodes("profile off"); say("   switch busy_ms:", one_switch())
gone = [a[e] for e in a if e not in c]; new = [c[e] for e in c if e not in a]
say(f"texts that had a size before the toggle and have none after: {len(gone)}; the reverse: {len(new)}")
tally = collections.Counter(w["words"][:26] for w in gone)
say("   lost their size:", tally.most_common(14))
say("   where they were:", collections.Counter((round(w["rect"][0], -2), round(w["rect"][1], -2)) for w in gone).most_common(8))
