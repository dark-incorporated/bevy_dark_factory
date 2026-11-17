#!/usr/bin/env python3
"""Make a scene every defect of `dark_factory-negatives` can be made on, from the repository's bedroom (DL-05, #464).

usage: sheet_scene.py <repo> <out-project>

Copies `<repo>/assets` to `<out-project>/assets` and adds, for the `bedroom` map: a design area over its north half, a forest of ten
trees along its south edge and a terrain with one peak in its south-east corner. A scratch project for a contact sheet, never a person's.
"""
import os
import shutil
import sys

repo, out = sys.argv[1:3]
shutil.copytree(os.path.join(repo, "assets"), os.path.join(out, "assets"), symlinks=False)
kit = os.path.join(out, "assets", "blueprint", "furniture")
area = os.path.join(out, "design", "levels", "bedroom", "areas", "north_half")
os.makedirs(area)
open(os.path.join(area, "area.yaml"), "w").write("version: 1\nrect:\n- -3.0\n- -2.5\n- 3.0\n- 0.0\n")
os.makedirs(os.path.join(kit, "forest"), exist_ok=True)
trees = "".join(
    f"- id: {i}\n  species: oak\n  at:\n  - {-2.25 + i * 0.5}\n  - 2.2\n  ground_m: 0.0\n  yaw: 0.0\n  scale: 1.0\n  variant: 0\n" for i in range(10)
)
open(os.path.join(kit, "forest", "bedroom.yaml"), "w").write("version: 1\ntrees:\n" + trees)
os.makedirs(os.path.join(kit, "terrain"), exist_ok=True)
w, d = 13, 11
heights = ["0.0"] * (w * d)
heights[9 * w + 11] = "1.0"
open(os.path.join(kit, "terrain", "bedroom.yaml"), "w").write(
    "version: 1\norigin:\n- -3.0\n- -2.5\ncell_m: 0.5\nwidth: 13\ndepth: 11\nheights:\n" + "".join(f"- {h}\n" for h in heights)
)
