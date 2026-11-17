#!/usr/bin/env python3
"""Take `docs/media/editor.png`: the whole editor window after an agent changed the bedroom map.

usage: stills.py --port <brp-port> --project <project-root> --out <directory> [--server <dark_factory_mcp>]

The editor must already be running on the bedroom map of this repository's own project (a scratch copy
of `assets/`, started as `bevy_dark_factory . bedroom` with BEVY_BRP_PORT set to the port) and must have
finished baking its palette thumbnails (the status band stops counting). The script never launches or
closes the editor, like record.py.

An agent removes the bed (`placement_remove`) and turns the sofa 90 degrees (`placement_set_yaw`)
through the MCP server, the view is framed on `[-2.6, -2.1, 2.6, 2.1]` last (an agent's change moves the
view to it, so the framing goes after the changes), and the window is captured with
`blueprint_capture_still` (source `window`): `editor.png`, 1088 x 640 on a 1280 x 720 Xvfb.
"""
import argparse
import os
import shutil
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import record  # noqa: E402


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, required=True)
    ap.add_argument("--project", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--server", default=os.environ.get("BEVY_BLUEPRINT_MCP", "dark_factory_mcp"))
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    mc = record.McpClient(args.server, args.project, args.port, "stills")
    inj = record.Injector(args.port)

    def shot(label):
        reply = mc.json_call("blueprint_capture_still", {"source": "window", "label": label})
        shutil.copyfile(reply["path"], os.path.join(args.out, f"{label}.png"))
        print(f"{label}.png <- {reply['path']}")

    def apply(operation, parameters):
        print(operation, str(mc.json_call("blueprint_apply_operation", {"operation": operation, "parameters": parameters}))[:120])

    apply("placement_remove", {"id": "bed"})
    time.sleep(0.5)
    apply("placement_set_yaw", {"id": "sofa", "yaw": 90})
    time.sleep(0.5)
    mc.json_call("blueprint_frame_view", {"target": {"rect": [-2.6, -2.1, 2.6, 2.1]}})
    inj.cursor(1000.0, 600.0)
    time.sleep(2.0)
    shot("editor")


if __name__ == "__main__":
    main()
