#!/usr/bin/env bash
# **Every clone that can push `main` carries the gate** (#579). The pre-push hook runs only where
# `core.hooksPath` names it; a clone without it pushes ungated and says nothing. Measured 2026-11-23: 7 of the last
# 9 release commits on `main` had no `gate.log` line on any host that keeps one, and the two hosts the releases are
# pushed from answered for themselves — `mac_air` had the setting, `bmb` had it set to `/dev/null`.
#
#   bash tools/loop/check-gate-hook.sh              every clone below, exit 1 when one is wrong
#   bash tools/loop/check-gate-hook.sh --fix        set it on each clone that is wrong (local config only)
#
# The list is the clones this repository is pushed from, by the name each is reachable at from `big`; a clone on this
# host is checked by path. Exit 0 only when every one reads `tools/loop/hooks`.
set -uo pipefail
want=tools/loop/hooks
fix=0
[ "${1:-}" = "--fix" ] || [ $# -eq 0 ] || { echo "usage: $0 [--fix]" >&2; exit 64; }
[ "${1:-}" = "--fix" ] && fix=1
# One line per clone: `<name> <path on that host>`. `big` is this host. The Macs keep the clone in different homes, and
# ssh resolves each name from `~/.ssh/config` as `ssh bmb` and `ssh mac_air` do interactively.
# `GATE_HOOK_CLONES` replaces the list, which is how a test drives it without contacting another machine.
default_clones="big $HOME/dark-incorporated/bevy_dark_factory
bmb /Users/rebekahbrittain/dark-incorporated/bevy_dark_factory
mac_air /Users/ladvien/dark-incorporated/bevy_dark_factory"
clones=${GATE_HOOK_CLONES:-$default_clones}
bad=0
while read -r host repo; do
  [ -n "$host" ] || continue
  if [ "$host" = "$(uname -n)" ] || [ "$host" = "big" ] && [ "$(uname -n)" = big ]; then
    got=$(git -C "$repo" config --get core.hooksPath 2>/dev/null) || got=""
    if [ "$got" != "$want" ]; then
      if [ "$fix" = 1 ]; then git -C "$repo" config --local core.hooksPath "$want" && got=$want; fi
    fi
  else
    got=$(timeout 20 ssh -o BatchMode=yes -o ConnectTimeout=5 "$host" </dev/null "git -C '$repo' config --get core.hooksPath" 2>/dev/null) || got=""
    if [ "$got" != "$want" ] && [ "$fix" = 1 ]; then
      timeout 20 ssh -o BatchMode=yes -o ConnectTimeout=5 "$host" </dev/null "git -C '$repo' config --local core.hooksPath '$want'" && got=$want
    fi
  fi
  if [ "$got" = "$want" ]; then
    echo "ok    $host:$repo  core.hooksPath=$got"
  else
    echo "WRONG $host:$repo  core.hooksPath='${got:-<unset>}' (want $want)" >&2
    bad=1
  fi
done < <(printf "%s\n" "$clones" | grep -v '^[[:space:]]*#')
exit "$bad"
