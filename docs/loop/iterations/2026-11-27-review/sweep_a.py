#!/usr/bin/env python3
"""Every workspace: does it open, what it costs at rest, how much of the window is the game's, one capture."""
import json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
TAG = sys.argv[1] if len(sys.argv) > 1 else "w"
WS = ["map", "world", "kit", "animation", "sound", "files", "game", "agent", "design", "project"]
out = {"window": scale_factor(), "rows": []}
say("window:", out["window"])
for ws in WS:
    row = {"ws": ws}
    try:
        t0 = time.perf_counter(); r = run(f"workspace.{ws}")
        row["outcome"] = (r or {}).get("outcome") if isinstance(r, dict) else str(r)[:60]
        row["why"] = (r or {}).get("why", "")[:140] if isinstance(r, dict) else ""
        t = time.time() + 5
        while time.time() < t:
            s = state()
            if (s.get("ui") or {}).get("workspace", "").lower() == ws or s.get("screen") != "editor":
                break
            time.sleep(0.01)
        row["switch_wall_ms"] = round(1000 * (time.perf_counter() - t0), 1)
        time.sleep(2.0)
        s = state()
        row["ui"] = s.get("ui"); row["screen"] = s.get("screen"); row["share"] = share(s); row["rects"] = s.get("rects")
        row["rest"] = busy(40)
        row["texts"] = len(widgets())
        row["tabindex"] = tabindex_census()
        row["problems"] = (s.get("editor") or {}).get("problems")
        row["shot"] = shot(f"{TAG}_{ws}")[0]
    except Exception as e:
        row["error"] = str(e)[:200]
    say(json.dumps({k: v for k, v in row.items() if k not in ("rects", "shot")}))
    out["rows"].append(row)
run("workspace.map")
json.dump(out, open(os.path.expanduser(f"~/fleetscratch/rv5/sweep_{TAG}.json"), "w"), indent=1)
