#!/usr/bin/env python3
"""Drive the Dark Factory editor over BRP: the calls every review script is built from."""
import base64
import json
import os
import sys
import time
import urllib.request

PORT = int(os.environ.get("PORT", "15790"))
OUT = os.environ.get("OUT", os.path.expanduser("~/fleetscratch/rv5/crops"))
os.makedirs(OUT, exist_ok=True)


def brp(method, params=None, timeout=120):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}).encode()
    req = urllib.request.Request(
        f"http://127.0.0.1:{PORT}/", data=body, method="POST", headers={"Content-Type": "application/json"}
    )
    reply = json.loads(urllib.request.urlopen(req, timeout=timeout).read())
    if "error" in reply:
        raise RuntimeError(f"{method}: {reply['error']}")
    return reply.get("result")


def shot(name, region=None):
    """bevy_fairy/screenshot + capture; returns path. region = [x, y, w, h] in surface px."""
    p = {"source": "window"}
    if region:
        p["region"] = region
    asked = brp("bevy_fairy/screenshot", p)
    cid = asked.get("id")
    if cid is None:
        raise RuntimeError(f"screenshot gave no id: {asked}")
    deadline = time.time() + 20
    while time.time() < deadline:
        ans = brp("bevy_fairy/capture", {"id": cid})
        st = ans.get("status")
        if st == "done":
            png = base64.b64decode(ans["png_base64"])
            path = f"{OUT}/{name}.png"
            open(path, "wb").write(png)
            from PIL import Image

            with Image.open(path) as im:
                return path, im.size
        if st == "failed":
            raise RuntimeError(f"capture failed: {ans}")
        time.sleep(0.04)
    raise RuntimeError("capture did not arrive in 20 s")


def state():
    return brp("blueprint/state", {})


def term():
    return state().get("terminal")


def key(k, action="Tap"):
    return brp("bevy_fairy/input", {"kind": "Keyboard", "key": k, "action": action})


def tap(k, mods=(), gap=0.03):
    for m in mods:
        key(m, "Press"); time.sleep(gap)
    time.sleep(gap)
    key(k, "Tap"); time.sleep(gap)
    for m in reversed(mods):
        key(m, "Release"); time.sleep(gap)


def type_text(t):
    return brp("bevy_fairy/input", {"kind": "Keyboard", "text": t})


def move(x, y):
    return brp("bevy_fairy/input", {"kind": "Cursor", "x": float(x), "y": float(y)})


def click(x, y, button="Left"):
    move(x, y); time.sleep(0.15)
    brp("bevy_fairy/input", {"kind": "Mouse", "button": button, "action": "Press"}); time.sleep(0.09)
    brp("bevy_fairy/input", {"kind": "Mouse", "button": button, "action": "Release"})


def scale_factor():
    q = brp("world.query", {"data": {"components": ["bevy_window::window::Window"]},
                            "filter": {"with": ["bevy_window::window::PrimaryWindow"]}})
    r = q[0]["components"]["bevy_window::window::Window"]["resolution"]
    return r["scale_factor"], r["physical_width"], r["physical_height"]


def widgets(prefix=None):
    """Every drawn Text's words and surface-px rect."""
    q = brp("world.query", {"data": {"components": ["bevy_ui::widget::text::Text",
                                                   "bevy_ui::ui_node::ComputedNode",
                                                   "bevy_ui::ui_transform::UiGlobalTransform"]}})
    out = []
    for row in q or []:
        c = row.get("components", {})
        t = c.get("bevy_ui::widget::text::Text")
        if isinstance(t, dict):
            words = "".join(sec.get("value", "") for sec in (t.get("sections") or []))
        else:
            words = t or ""
        words = words.strip()
        if not words:
            continue
        raw = c.get("bevy_ui::ui_transform::UiGlobalTransform")
        if isinstance(raw, dict):
            tr = raw.get("translation") or [0, 0]
        elif isinstance(raw, list) and len(raw) >= 6:
            tr = [raw[4], raw[5]]
        elif isinstance(raw, list) and len(raw) == 2:
            tr = raw
        else:
            tr = [0, 0]
        sz = (c.get("bevy_ui::ui_node::ComputedNode") or {}).get("size") or [0, 0]
        rect = [tr[0] - sz[0] / 2, tr[1] - sz[1] / 2, tr[0] + sz[0] / 2, tr[1] + sz[1] / 2]
        out.append({"words": words, "rect": rect, "size": sz, "entity": row.get("entity")})
    if prefix:
        out = [w for w in out if w["words"].startswith(prefix)]
    out.sort(key=lambda w: (round(w["rect"][1]), w["rect"][0]))
    return out


def terminal_texts():
    """The terminal pane's own Text nodes: the body rows, the bar. Returns words sorted by y then x."""
    return [w for w in widgets() if w["rect"][1] > 200]


def find(words, exact=True):
    for w in widgets():
        if (w["words"] == words) if exact else (words in w["words"]):
            return w
    return None


def say(*a):
    line = " ".join(str(x) for x in a)
    print(line, flush=True)


if __name__ == "__main__":
    say("scale/physical:", scale_factor())
    say("terminal:", json.dumps(term()))
    for w in widgets():
        say("  text", json.dumps(w["words"])[:60], [round(v) for v in w["rect"]])
