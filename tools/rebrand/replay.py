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

# Crates whose directory name changes. **These are git's own renames from `dbc9beed`,
# read out of the commit rather than guessed** — the rebrand put the core crate at
# `blueprint_model`, the rng at `blueprint_rng` and the mcp at `dark_factory_mcp`, none of
# which follow the package-name rule. Guessing produced two crates claiming one package.
CRATE_MAP: tuple[tuple[str, str], ...] = (
    ("crates/bevy_cauldron", "crates/bevy_dark_factory"),
    ("crates/bevy_cauldron_mcp", "crates/dark_factory_mcp"),
    ("crates/bevy_grimoire", "crates/bevy_blueprint"),
    ("crates/bevy_grimoire_anim", "crates/bevy_blueprint_anim"),
    ("crates/bevy_grimoire_core", "crates/blueprint_model"),
    ("crates/bevy_grimoire_rng", "crates/blueprint_rng"),
    ("crates/bevy_grimoire_snip", "crates/dark_factory_snip"),
    ("crates/bevy_grimoire_solver", "crates/blueprint_solver"),
    ("crates/bevy_grimoire_migrate", "crates/bevy_blueprint_migrate"),
    ("crates/bevy_grimoire_rhai", "crates/bevy_blueprint_rhai"),
    ("games/grimoire_sample_game", "games/blueprint_sample_game"),
)

# Package and identifier names, **read out of the rebrand commit's own Cargo.toml files**
# rather than derived from a rule. It drops the `bevy_` prefix on five crates
# (`blueprint_model`, `blueprint_rng`, `blueprint_solver`, `dark_factory_mcp`,
# `dark_factory_snip`), which no rule would predict — deriving it produced a crate whose
# directory said `blueprint_model` and whose package said `bevy_blueprint_core`.
# Longest first, so `bevy_grimoire_core` is consumed before `bevy_grimoire`.
RENAMES: tuple[tuple[str, str], ...] = (
    # The MCP server's compound name, before either of its words: `cauldron_grimoire`
    # appears in 98 files (the bench's transcript fixtures, the guide, the schemas), and
    # renaming `cauldron` and `grimoire` separately produced `dark_factory_blueprint` in
    # some places and `cauldron_blueprint` in others depending on which rule ran first.
    ("cauldron_grimoire", "dark_factory_blueprint"),
    ("CauldronGrimoire", "DarkFactoryBlueprint"),
    ("bevy_grimoire_core", "blueprint_model"),
    ("bevy_grimoire_rng", "blueprint_rng"),
    ("bevy_grimoire_solver", "blueprint_solver"),
    ("bevy_grimoire_snip", "dark_factory_snip"),
    ("bevy_grimoire_anim", "bevy_blueprint_anim"),
    ("bevy_grimoire_rhai", "bevy_blueprint_rhai"),
    ("bevy_grimoire_migrate", "bevy_blueprint_migrate"),
    ("bevy_grimoire", "bevy_blueprint"),
    ("bevy_cauldron_mcp", "dark_factory_mcp"),
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
    # A *quoted display name* is one word and capitalised, while an identifier is
    # snake_case: `mod familiar;` must become `mod night_shift;`, but a test compares a
    # **lowercased** status line, so its literal must be the lowercased display name —
    # `"the NightShift"` can never match `to_lowercase()`'s output.
    ('"the familiar', '"the nightshift'),
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
    """Every path to move: git's own recorded renames, plus the crate map for paths the
    rebrand never saw (files added after it forked). One pass, no overlap — a path that
    git already renamed is never rewritten again."""
    out = git("show", "dbc9beed", "-M", "--name-status", "--format=")
    pairs: list[tuple[str, str]] = []
    seen: set[str] = set()
    for line in out.splitlines():
        parts = line.split("\t")
        if len(parts) == 3 and parts[0].startswith("R"):
            pairs.append((parts[1], parts[2]))
            seen.add(parts[1])
            seen.add(parts[2])
    # Anything tracked today under a crate directory the map does not already cover.
    for path in git("ls-files").splitlines():
        if path in seen:
            continue
        for old, new in CRATE_MAP:
            if path == old or path.startswith(old + "/"):
                pairs.append((path, new + path[len(old):]))
                break
    return pairs


def substitute(text: str) -> str:
    for old, new in RENAMES:
        text = text.replace(old, new)
    return text


def rename_nested_grimoire(args_check: bool) -> int:
    """Every `…/assets/grimoire` directory, not only the repository root's.

    The bench fixture and the sample game each carry their own `assets/grimoire`, and the
    rebrand's rename list only covers the root one — so the bench's `bench_bedroom` and the
    greeting task's `courtyard` were left under a path their loader no longer reads.
    """
    moved = 0
    for path in sorted(pathlib.Path(".").rglob("grimoire")):
        if not path.is_dir() or "target" in path.parts or ".git" in path.parts:
            continue
        target = path.with_name("blueprint")
        if target.exists():
            continue
        if args_check:
            print(f"rename {path} -> {target}")
        else:
            subprocess.run(("git", "mv", str(path), str(target)), check=True)
        moved += 1
    return moved


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

    nested = rename_nested_grimoire(args.check)

    print(f"renamed {moved} tracked paths and {nested} nested asset directories, skipped "
          f"{skipped_source} missing sources and {skipped_target} occupied targets; "
          f"rewrote {edited} files", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
