#!/usr/bin/env python3
"""Close-ups: which system owns the switch frame, does a click move focus, the divider, the inspector's blank band,
Zen on the Game workspace, the document tab strip, and the status band's mark."""
import collections, glob, json, os, shutil, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
RUN = os.path.expanduser("~/fleetscratch/rv5")
def focus(): return (brp("world.get_resources", {"resource": "bevy_input_focus::InputFocus"}) or {}).get("value", {}).get("current_focus")

run("workspace.map"); time.sleep(1.2)
# 1. the frame profile on, then switch
chord("KeyF", ("ControlLeft", "ShiftLeft")); time.sleep(0.6)
say("profile recording:", (state().get("frame") or {}).get("profile_recording"))
heavy = collections.Counter(); rows = []
for i in range(6):
    for target in ("kit", "map"):
        s = spike(lambda: run(f"workspace.{target}"), frames=8)
        h = s.get("heaviest") or {}
        heavy[h.get("system")] += 1; rows.append((target, s["worst_busy_ms"], h.get("system"), round(h.get("ms") or 0, 2)))
        time.sleep(0.3)
say("switch with the profile on (target, worst busy_ms, heaviest instrumented span, its ms):")
for r in rows[:6]: say("   ", r)
say("heaviest span tally:", dict(heavy))
shot("c_profile")
chord("KeyF", ("ControlLeft", "ShiftLeft")); time.sleep(0.4)

# 2. does a click move InputFocus?
f0 = focus(); click(106, 88); time.sleep(0.3); f1 = focus(); click(43, 88); time.sleep(0.3); f2 = focus()
say(f"InputFocus: before {f0}  after clicking toolbar Move {f1}  after clicking Place {f2}  moved: {f0 != f1 or f1 != f2}")
say("tool after clicks:", (state().get("editor") or {}).get("tool"))

# 3. the split divider as laid out
try:
    q = brp("world.query", {"data": {"components": ["bevy_gooey::skin::Role", "bevy_ui::ui_node::ComputedNode", "bevy_ui::ui_transform::UiGlobalTransform"]}})
    for row in q or []:
        c = row["components"]
        if c.get("bevy_gooey::skin::Role") == "SplitDivider":
            sz = (c.get("bevy_ui::ui_node::ComputedNode") or {}).get("size")
            tr = c.get("bevy_ui::ui_transform::UiGlobalTransform")
            say("   SplitDivider size", [round(v, 1) for v in sz], "centre", [round(v) for v in (tr[4:6] if isinstance(tr, list) else [0, 0])])
except Exception as e:
    say("divider query:", str(e)[:160])

# 4. the inspector's column of text, top to bottom
ws = [w for w in widgets() if w["rect"][0] < 380 and w["rect"][2] > 40 and w["size"][1] > 0 and 150 < w["rect"][1] < 850]
say("inspector texts (y0-y1, words):")
prev = None
for w in ws:
    y0, y1 = round(w["rect"][1]), round(w["rect"][3])
    gap = (y0 - prev) if prev is not None else 0
    if gap > 40: say(f"      ---- {gap} px with no text ----")
    if w["rect"][0] < 80: say(f"   {y0:4d}-{y1:4d}  {w['words'][:38]!r}")
    prev = max(prev or 0, y1)

# 5. Zen on the Game workspace
run("workspace.game"); time.sleep(1.2); g0 = share()
chord("KeyM", ("ControlLeft", "ShiftLeft")); time.sleep(0.8); g1 = share(); shot("c_game_zen")
chord("KeyM", ("ControlLeft", "ShiftLeft")); time.sleep(0.8)
say(f"GAME share {g0}% -> Zen {g1}%")

# 6. two files open: the document tab strip
run("workspace.files"); time.sleep(1.2)
def row(words):
    for w in widgets():
        if w["words"] == words and w["size"][1] > 0: return w
opened = []
a = row("assets")
if a: click((a["rect"][0] + a["rect"][2]) / 2, (a["rect"][1] + a["rect"][3]) / 2); time.sleep(0.6)
b = row("blueprint")
if b: click((b["rect"][0] + b["rect"][2]) / 2, (b["rect"][1] + b["rect"][3]) / 2); time.sleep(0.6)
names = [w for w in widgets() if w["rect"][0] < 330 and w["words"].endswith((".yaml", ".md", ".rhai", ".jsonl")) and w["size"][1] > 0]
say("files in the tree:", [w["words"] for w in names][:10])
for w in names[:2]:
    click((w["rect"][0] + w["rect"][2]) / 2, (w["rect"][1] + w["rect"][3]) / 2); time.sleep(0.9); opened.append(w["words"])
say("opened:", opened, " ui:", json.dumps(state().get("ui")))
move(700, 500); time.sleep(0.4); shot("c_tabs_rest")
tabs = [w for w in widgets() if w["words"] in opened and w["rect"][1] < 170]
say("tab labels drawn:", [(w["words"], [round(v) for v in w["rect"]]) for w in tabs])
if tabs:
    t = tabs[0]; move((t["rect"][0] + t["rect"][2]) / 2, (t["rect"][1] + t["rect"][3]) / 2); time.sleep(0.5); shot("c_tabs_hover")
    try:
        q = brp("world.query", {"data": {"components": ["bevy_gooey::skin::Role", "bevy_ui::ui_node::ComputedNode", "bevy_ui::ui_node::BackgroundColor"], "option": ["bevy_picking::hover::Hovered"]}})
        for r in q or []:
            c = r["components"]
            if c.get("bevy_gooey::skin::Role") == "EdgeTab":
                say("   EdgeTab", [round(v) for v in (c.get("bevy_ui::ui_node::ComputedNode") or {}).get("size", [])], "bg", json.dumps(c.get("bevy_ui::ui_node::BackgroundColor"))[:90], "hovered", c.get("bevy_picking::hover::Hovered"))
    except Exception as e:
        say("edge tab query:", str(e)[:160])

# 7. a refused reload: does the status band's item draw its mark?
run("workspace.map"); time.sleep(1.0); move(700, 500)
cands = sorted(glob.glob(RUN + "/proj/assets/blueprint/**/*.yaml", recursive=True))
kit = next((c for c in cands if c.endswith("library.yaml") or "/kit" in c), cands[0] if cands else None)
say("breaking:", kit.replace(RUN, "") if kit else None)
if kit:
    shutil.copy(kit, kit + ".bak")
    open(kit, "a").write("\n: : this is not yaml [\n")
    time.sleep(4.0)
    s = state()
    say("reloads:", json.dumps(s.get("reloads"))[:300])
    shot("c_band_warn")
    band = [w for w in widgets() if w["rect"][1] > 815 and w["size"][1] > 0]
    say("status band texts:", [(w["words"][:30], [round(v) for v in w["rect"]]) for w in band])
    shutil.move(kit + ".bak", kit); time.sleep(4.0)
    say("after restore, band texts:", [w["words"][:30] for w in widgets() if w["rect"][1] > 815 and w["size"][1] > 0])
    shot("c_band_clean")
