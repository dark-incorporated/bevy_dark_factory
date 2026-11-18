#!/usr/bin/env python3
"""A contact sheet of one scene and its engine-made defects, from `dark_factory-negatives sheet` (DL-05, #464).

usage: contact_sheet.py <sheet-dir> --sheet <out.png> [--frames <dir>] [--crop x0,y0,x1,y1] [--cell 480]

Reads <sheet-dir>/sheet.json and the pictures beside it. Writes a grid, the kept scene first and each defect after it, every cell captioned
with the defect, its size and what the engine measured on the saved scene (the floating piece's gap, the shared volume, the area test, the
angle, the trees, the peak's height). With --frames it also writes one captioned frame per picture, for `changelog.py clip --frames`.
A picture the renderer could not draw is a cell that says why, never a blank one.
"""
import argparse
import json
import os
import sys

from PIL import Image, ImageDraw, ImageFont

CAPTION_H = 64
PAD = 8


def font(size):
    for path in ("/System/Library/Fonts/Menlo.ttc", "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf", "/usr/share/fonts/dejavu/DejaVuSansMono.ttf"):
        if os.path.exists(path):
            return ImageFont.truetype(path, size)
    return ImageFont.load_default()


def measured(entry):
    """The one sentence of what the engine measured, from the record's own facts."""
    facts = entry.get("facts", {})
    a, b = facts.get("after", {}), facts.get("before", {})
    d = entry.get("defect")
    if d == "float":
        return f"gap {a['gap_m']:.2f} m (was {b['gap_m']:.2f}) on {a['piece']}"
    if d == "clip":
        return f"shared volume {a['overlap_volume_m3']:.3f} m3, {a['pair'][0]} in {a['pair'][1]}"
    if d == "outside_area":
        return f"{a['piece']} {a['distance_outside_m']:.2f} m outside {a['area']} (inside was {str(b['inside']).lower()})"
    if d == "delete_named":
        return f"{a['piece']} named by the request: on the map {str(b['present']).lower()} -> {str(a['present']).lower()}"
    if d == "turn_wrong":
        return f"{a['piece']} turned {a['turned_deg']:.0f} deg"
    if d == "thin_forest":
        return f"forest {b['trees']} -> {a['trees']} trees"
    if d == "flatten_peak":
        return f"peak {b['height_m']:.2f} -> {a['height_m']:.2f} m"
    return ""


def cell(directory, entry, size, crop):
    img = Image.new("RGB", (size, size * 10 // 16 + CAPTION_H), (24, 24, 28))
    draw = ImageDraw.Draw(img)
    title = entry["what"]
    if "refused" in entry:
        draw.text((PAD, PAD), f"{title}: refused", fill=(255, 190, 120), font=font(15))
        draw.text((PAD, 32), entry["refused"][:120], fill=(200, 200, 200), font=font(12))
        return img
    if entry.get("file") and os.path.exists(os.path.join(directory, entry["file"])) and entry.get("picture"):
        pic = Image.open(os.path.join(directory, entry["file"])).convert("RGB")
        if crop:
            pic = pic.crop(crop)
        pic.thumbnail((size, size * 10 // 16))
        img.paste(pic, (0, 0))
    else:
        draw.text((PAD, PAD), f"no picture: {entry.get('missing') or 'none was taken'}"[:90], fill=(255, 190, 120), font=font(12))
    y = size * 10 // 16 + 6
    draw.text((PAD, y), title, fill=(255, 255, 255), font=font(16))
    draw.text((PAD, y + 24), measured(entry)[:70] if entry.get("defect") else "the scene as kept", fill=(170, 210, 255), font=font(13))
    return img


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("directory")
    ap.add_argument("--sheet", required=True)
    ap.add_argument("--frames")
    ap.add_argument("--crop")
    ap.add_argument("--cell", type=int, default=480)
    args = ap.parse_args()
    crop = tuple(int(v) for v in args.crop.split(",")) if args.crop else None
    entries = json.load(open(os.path.join(args.directory, "sheet.json")))["pictures"]
    cells = [cell(args.directory, e, args.cell, crop) for e in entries]
    cols = 4
    rows = (len(cells) + cols - 1) // cols
    w, h = cells[0].size
    sheet = Image.new("RGB", (cols * w + (cols + 1) * PAD, rows * h + (rows + 1) * PAD), (10, 10, 12))
    for i, c in enumerate(cells):
        sheet.paste(c, (PAD + (i % cols) * (w + PAD), PAD + (i // cols) * (h + PAD)))
    sheet.save(args.sheet)
    print(f"{args.sheet}: {sheet.size[0]} x {sheet.size[1]}, {len(cells)} cells")
    if args.frames:
        os.makedirs(args.frames, exist_ok=True)
        for i, e in enumerate(entries):
            cell(args.directory, e, 900, crop).save(os.path.join(args.frames, f"{i:03d}.png"))


if __name__ == "__main__":
    sys.exit(main())
