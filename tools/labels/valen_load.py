#!/usr/bin/env python3
"""Load an export with Valen's own reader (DL-06, #465).

usage: valen_load.py <export-dir> [<export-dir>...]

`dark_factory-labels export` writes `<out>/train.jsonl`, `<out>/eval.jsonl` and `<out>/holdout.jsonl` in Valen's training
format, with the pictures under `<out>/images/`. This runs `valen.data.schema.read_jsonl`, the reader Valen's own
training uses, over each file that exists, so "the exported JSONL loads with Valen's own data loader" is a command a
person can run rather than a claim. Nothing here re-implements the contract: a record the reader refuses prints the
reader's own sentence and the exit status is 1.

The `valen` package must be importable (`pip install -e` a Valen checkout, or `PYTHONPATH=<checkout>`), because the
contract is the package's and a copy of it here would be a second one to drift.
"""
import sys
from pathlib import Path

FILES = ("train.jsonl", "eval.jsonl", "holdout.jsonl")


def main(argv):
    if len(argv) < 2:
        print(__doc__.strip(), file=sys.stderr)
        return 2
    try:
        from valen.data.schema import read_jsonl
    except ImportError as why:
        print(f"the valen package is not importable, so its reader cannot be asked: {why}", file=sys.stderr)
        print("put a Valen checkout on PYTHONPATH (its `valen/` folder's parent).", file=sys.stderr)
        return 2
    failures = 0
    for directory in map(Path, argv[1:]):
        for name in FILES:
            path = directory / name
            if not path.is_file():
                print(f"{path}: not written (no record of that side)")
                continue
            try:
                records = read_jsonl(path)
            except ValueError as why:
                print(f"{path}: Valen's reader refused it: {why}", file=sys.stderr)
                failures += 1
                continue
            images = sum(1 for record in records for asset in record.get("assets", []))
            labelled = sum(
                1
                for record in records
                for target in record.get("targets", {}).values()
                if target.get("probabilities") is not None
            )
            print(f"{path}: {len(records)} record(s), {labelled} labelled question(s), {images} asset(s), all taken by Valen's reader")
            # A `group_id` is what the split is by: printed once so a person can see no scene is in two files by hand.
            print(f"  group_ids: {', '.join(sorted({record['group_id'] for record in records}))}")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
