#!/usr/bin/env python3
"""Today's two merges up close: the band's refused-reload item (#647) and the document tab strip (#645)."""
import glob, json, os, shutil, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
RUN = os.path.expanduser("~/fleetscratch/rv5")
def band(): return [(w["words"][:44], [round(v) for v in w["rect"]]) for w in widgets() if 815 < w["rect"][1] < 850 and w["size"][1] > 0 and w["rect"][3] <= 850]
run("workspace.map"); time.sleep(1.0); move(700, 450)
say("band before:", [b[0] for b in band()])
r0 = (state().get("reloads") or {}); say("reloads keys:", list(r0.keys()), " counts:", {k: v for k, v in r0.items() if k != "kinds"})
# a style refusal is the case #647 names; the editor's own sheet is live-read, so break a project file the reload lane watches
kit = RUN + "/proj/assets/blueprint/furniture/library.yaml"
shutil.copy(kit, kit + ".bak"); open(kit, "a").write("\n: : this is not yaml [\n"); time.sleep(4.0)
r1 = state().get("reloads") or {}
say("reloads after breaking library.yaml:", {k: v for k, v in r1.items() if k != "kinds"})
for k in r1.get("kinds") or []:
    if k.get("outcome") != "applied": say("   not applied:", json.dumps(k)[:260])
say("   kinds named:", [(os.path.basename(k.get("file") or ""), k.get("kind"), k.get("outcome")) for k in (r1.get("kinds") or [])])
say("band after:", band()); shot("i_band_refused")
shutil.move(kit + ".bak", kit); time.sleep(4.0)
say("band after restore:", [b[0] for b in band()])
r2 = state().get("reloads") or {}; say("   kinds:", [(os.path.basename(k.get("file") or ""), k.get("outcome")) for k in (r2.get("kinds") or [])])

# the document tab strip
run("workspace.files"); time.sleep(1.3)
def rows(): return [w for w in widgets() if w["rect"][0] < 340 and 150 < w["rect"][1] < 800 and w["size"][1] > 0]
def at(w): return (w["rect"][0] + w["rect"][2]) / 2, (w["rect"][1] + w["rect"][3]) / 2
say("tree:", [w["words"] for w in rows()])
a = next((w for w in rows() if w["words"].strip().endswith("assets")), None)
if a:
    x, y = at(a); click(a["rect"][0] - 8, y); time.sleep(0.6)
    if len(rows()) < 5: click(x, y); time.sleep(0.15); click(x, y); time.sleep(0.6)
    if len(rows()) < 5: chord("ArrowRight"); time.sleep(0.6)
say("tree after opening assets:", [w["words"] for w in rows()][:14])
for name in ("blueprint", "furniture"):
    f = next((w for w in rows() if w["words"].strip().endswith(name)), None)
    if f:
        x, y = at(f); click(f["rect"][0] - 8, y); time.sleep(0.6)
files = [w for w in rows() if w["words"].strip().endswith((".yaml", ".md", ".rhai", ".json", ".jsonl"))]
say("files offered:", [w["words"] for w in files][:8])
for w in files[:2]:
    x, y = at(w); click(x, y); time.sleep(0.2); click(x, y); time.sleep(1.0)
move(700, 600); time.sleep(0.4)
top = [(w["words"][:30], [round(v) for v in w["rect"]]) for w in widgets() if 150 < w["rect"][1] < 215 and w["rect"][0] > 380 and w["size"][1] > 0]
say("above the document:", top[:8]); shot("i_tabs_rest")
if top:
    r = top[0][1]; move((r[0] + r[2]) / 2, (r[1] + r[3]) / 2); time.sleep(0.6); shot("i_tabs_hover")
run("workspace.map"); time.sleep(0.8)
