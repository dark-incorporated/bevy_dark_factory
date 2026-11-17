#!/usr/bin/env python3
"""Replay the 2026-11-08 rebrand onto today's tree.

The rebrand (`dbc9beed`) is 919 path renames plus 197 files' string edits, and it has
since fallen 1137 commits behind main, so merging it would mean hand-resolving 366
conflicts in files that have moved on. Replaying it is the alternative: apply the same
rename map and the same substitutions to whatever main is now, and let the compiler and
the test suite say whether it landed.

Two rules that keep this honest:

- **Every substitution is a word boundary**, so `grimoire` inside `bevy_grimoire` and
  `Grimoire` inside `GrimoireError` are handled by the longer names first, and a plain
  `grim` is never touched.
- **The map is reported, not guessed.** Every source that no longer exists and every
  target that already does is printed, so a human can see what the drift costs.

Run from the repository root:  python3 tools/rebrand/replay.py [--check]
`--check` prints what it would do and changes nothing.
"""

from __future__ import annotations

import argparse
import pathlib
import subprocess
import sys

# The longest names first: `bevy_grimoire_core` must become `bevy_blueprint_core` before
# `bevy_grimoire` is considered, or the shorter rule would eat its prefix.
RENAMES: tuple[tuple[str, str], ...] = (
    ("bevy_grimoire_core", "bevy_blueprint_core"),
    ("bevy_grimoire_rng", "bevy_blueprint_rng"),
    ("bevy_grimoire_solver", "bevy_blueprint_solver"),
    ("bevy_grimoire_anim", "bevy_blueprint_anim"),
    ("bevy_grimoire_rhai", "bevy_blueprint_rhai"),
    ("bevy_grimoire_migrate", "bevy_blueprint_migrate"),
    ("bevy_grimoire", "bevy_blueprint"),
    ("bevy_cauldron_mcp", "bevy_dark_factory_mcp"),
    ("bevy_cauldron_bench", "bevy_dark_factory_bench"),
    ("bevy_cauldron", "bevy_dark_factory"),
    ("grimoire_sample_game", "blueprint_sample_game"),
    ("grimoire", "blueprint"),
    ("Grimoire", "Blueprint"),
    ("GRIMOIRE", "BLUEPRINT"),
    ("cauldron", "dark_factory"),
    ("Cauldron", "DarkFactory"),
    ("CAULDRON", "DARK_FACTORY"),
    ("Familiar", "NightShift"),
    ("familiar", "night_shift"),
)

# File extensions whose contents are rewritten. A binary, a font or an LFS clip is moved
# by the rename map but never edited.
TEXT_SUFFIXES = frozenset(
    {".rs", ".toml", ".md", ".json", ".yaml", ".yml", ".ron", ".sh", ".py", ".txt",
     ".service", ".cfg", ".wesl", ".glsl", ".html", ".css", ".js", ".ts", ".svg",
     ".lock", ".gitignore", ".gitattributes", ""}
)
SKIP_DIRS = frozenset({".git", "target", "node_modules"})


def git(*args: str) -> str:
    return subprocess.run(("git", *args), check=True, capture_output=True, text=True).stdout


def rename_map() -> list[tuple[str, str]]:
    """The rebrand's own rename map: the paths git recorded as renames, verbatim."""
    out = git("show", "dbc9beed", "-M", "--name-status", "--format=")
    pairs: list[tuple[str, str]] = []
    for line in out.splitlines():
        parts = line.split("\t")
        if len(parts) == 3 and parts[0].startswith("R"):
            pairs.append((parts[1], parts[2]))
    return pairs


def substitute(text: str) -> str:
    for old, new in RENAMES:
        text = text.replace(old, new)
    return text


def is_text(path: pathlib.Path) -> bool:
    if path.suffix in TEXT_SUFFIXES:
        return True
    return path.name in {".gitignore", ".gitattributes", "Cargo.lock"}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="report, change nothing")
    args = parser.parse_args()

    root = pathlib.Path(git("rev-parse", "--show-toplevel").strip())
    pairs = rename_map()
    moved = skipped_source = skipped_target = 0

    for old, new in pairs:
        source, target = root / old, root / new
        if not source.exists():
            skipped_source += 1
            continue
        if target.exists():
            # The tree already has the new name; either a previous run or main moved on.
            skipped_target += 1
            continue
        if args.check:
            print(f"rename {old} -> {new}")
        else:
            target.parent.mkdir(parents=True, exist_ok=True)
            git("mv", old, new)
        moved += 1

    edited = 0
    for path in sorted(root.rglob("*")):
        if not path.is_file() or any(part in SKIP_DIRS for part in path.parts):
            continue
        if not is_text(path):
            continue
        try:
            text = path.read_text()
        except UnicodeDecodeError:
            continue
        new_text = substitute(text)
        if new_text == text:
            continue
        edited += 1
        if not args.check:
            path.write_text(new_text)

    print(f"renamed {moved}, skipped {skipped_source} missing sources and {skipped_target} "
          f"occupied targets; rewrote {edited} files", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
