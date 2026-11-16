#!/usr/bin/env python3
"""Draw one map in a real editor and write its picture: the renderer `cauldron-negatives` calls (DL-05, #464).

usage: render_scene.py <project-root> <map> <out.png>      (needs DISPLAY, a private Xvfb: never :0)

environment: EDITOR_BIN  the `bevy_cauldron` binary (default: `bevy_cauldron` beside this repository's target/debug)
             SETTLE_S    seconds to wait after the editor answers BRP, for the palette's thumbnails to bake (default 45)

It starts the editor on the project as `bevy_cauldron . <map>` with its own HOME and a free BRP port, takes the editor's own picture
(`bevy_fairy/screenshot`, source offscreen: the world and the interface, as the person sees them) and closes the editor. The project it is
given is changed by nothing but the editor's own start-up: `cauldron-negatives` hands it a scratch copy, never the person's project.
"""
import base64
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request


def brp(port, method, params=None):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}).encode()
    req = urllib.request.Request(f"http://127.0.0.1:{port}/", data=body, method="POST", headers={"Content-Type": "application/json"})
    reply = json.loads(urllib.request.urlopen(req, timeout=20).read())
    if "error" in reply:
        raise RuntimeError(f"{method}: {reply['error']}")
    return reply.get("result")


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    root, name, out = sys.argv[1:]
    if not os.environ.get("DISPLAY"):
        sys.exit("set DISPLAY to a private Xvfb, never :0")
    editor = os.environ.get("EDITOR_BIN") or shutil.which("bevy_cauldron")
    if not editor:
        sys.exit("set EDITOR_BIN to the bevy_cauldron binary")
    settle = float(os.environ.get("SETTLE_S", "45"))
    port = free_port()
    home = tempfile.mkdtemp(prefix="render-scene-home-")
    log = open(os.path.join(home, "editor.log"), "wb")
    proc = subprocess.Popen(
        [editor, ".", name], cwd=root, stdin=subprocess.DEVNULL, stdout=log, stderr=log,
        env={**os.environ, "HOME": home, "XDG_DATA_HOME": os.path.join(home, "data"), "XDG_CONFIG_HOME": os.path.join(home, "config"),
             "BEVY_BRP_PORT": str(port), "GRIMOIRE_BENCH_RUN": "1"},
    )
    try:
        for _ in range(120):
            if proc.poll() is not None:
                sys.exit(f"the editor stopped with {proc.returncode}: {open(log.name).read()[-400:]}")
            try:
                brp(port, "rpc.discover")
                break
            except Exception:
                time.sleep(1)
        else:
            sys.exit("the editor never answered BRP")
        time.sleep(settle)
        asked = brp(port, "bevy_fairy/screenshot", {"source": "offscreen"})
        deadline = time.time() + 30
        while time.time() < deadline:
            got = brp(port, "bevy_fairy/capture", {"id": asked["id"]})
            if got.get("status") == "done":
                with open(out, "wb") as f:
                    f.write(base64.b64decode(got["png_base64"]))
                return
            if got.get("status") == "failed":
                sys.exit(f"the capture failed: {got.get('error')}")
            time.sleep(0.1)
        sys.exit("the capture did not arrive in 30 s")
    finally:
        proc.terminate()
        try:
            proc.wait(10)
        except subprocess.TimeoutExpired:
            proc.kill()
        log.close()
        shutil.rmtree(home, ignore_errors=True)


if __name__ == "__main__":
    main()
