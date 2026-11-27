#!/usr/bin/env python3
"""Do World, Kit and Animation draw anything in the viewport, given time?"""
import json, os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
def meshes():
    try:
        q = brp("world.query", {"data": {"components": ["bevy_camera::visibility::ViewVisibility"], "has": ["bevy_mesh::components::Mesh3d"]}})
        rows = [r for r in q or [] if (r.get("has") or {}).get("bevy_mesh::components::Mesh3d")]
        return len(rows), sum(1 for r in rows if r["components"].get("bevy_camera::visibility::ViewVisibility"))
    except Exception as e:
        return str(e)[:100], None
for ws in ("map", "world", "kit", "animation", "map"):
    run(f"workspace.{ws}"); time.sleep(1.5); a = meshes(); shot(f"j_{ws}_1s")
    time.sleep(6.0); b = meshes(); shot(f"j_{ws}_8s")
    s = state(); e = s.get("editor") or {}
    say(f"{ws.upper():10s} Mesh3d entities (total, in view): after 1.5 s {a}, after 7.5 s {b} | map {e.get('map')} placements {e.get('placements') or e.get('placed')}")
