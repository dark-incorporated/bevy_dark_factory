#!/usr/bin/env bash
# **A release tag whose commit has no gate record is refused** (#579). 7 of the last 9 release commits on `main` reached
# it with no `gate.log` line on any host that keeps one, because the pre-push hook runs only where `core.hooksPath` names
# it and two of the three clones that push `main` did not. A convention cannot be enforced from the pushing side; this is
# the reading side, and it runs where the tag becomes a DMG.
#
#   bash tools/loop/require-gate-record.sh <sha>
#
# Exit 0 when `refs/notes/gated` holds a note for the commit (the record the hook writes where the gate passes, #575), and
# exit 1 naming the commit when it does not. `--allow <reason>` records an owner exception and prints it: a release that
# must ship without a record says so in the log rather than passing silently.
set -uo pipefail
notes_ref=refs/notes/gated
allow=""
usage() { echo "usage: $0 [--allow <reason>] <sha>" >&2; exit 64; }
if [ "${1:-}" = "--allow" ]; then
  [ $# -ge 3 ] || usage
  allow=$2
  shift 2
fi
[ $# -ge 1 ] || usage
sha=$1
if ! git cat-file -e "$sha^{commit}" 2>/dev/null; then
  echo "require-gate-record: $sha is not a commit in this clone" >&2
  exit 2
fi
# The notes travel with the commits; a clone that has never fetched them is asking a question it can answer.
git fetch --quiet origin "+$notes_ref:$notes_ref" >/dev/null 2>&1 || true
if git notes --ref "$notes_ref" show "$sha" >/dev/null 2>&1; then
  echo "require-gate-record: $sha carries its gate record: $(git notes --ref "$notes_ref" show "$sha" | head -1)"
  exit 0
fi
if [ -n "$allow" ]; then
  echo "require-gate-record: OWNER EXCEPTION: $sha has no record in $notes_ref ($allow)"
  exit 0
fi
echo "require-gate-record: REFUSED: $sha has no record in $notes_ref, so the push that put it on main ran no gate. Gate it from a clone with 'git config --local core.hooksPath tools/loop/hooks' (tools/loop/check-gate-hook.sh lists which have it), or pass --allow <reason> to say why this one ships without." >&2
exit 1
