#!/usr/bin/env bash
# Install the gate-target prune as a systemd user timer (`tools/loop/gate-prune.{service,timer}`), which is what
# makes it schedule (#578): the script existed and was correct, and nothing ran it, so `/home` on big reached 99% full
# (64 GB free) with a 283 GB gate target; 195 superseded test executables in it were 129.4 GB cargo could never reuse.
#
#   bash tools/loop/install-gate-prune.sh            install and start the timer
#   bash tools/loop/install-gate-prune.sh --dry-run  print the unit files and the commands, install nothing
#
# Linux with a systemd user manager only, and it says so rather than half-installing. The unit runs the script from
# this checkout, so the checkout must stay where it is; the unit's path is written from `git rev-parse --show-toplevel`.
set -euo pipefail
repo=$(git rev-parse --show-toplevel)
here="$repo/tools/loop"
units="$HOME/.config/systemd/user"

if [ "${1:-}" = "--dry-run" ]; then
  echo "# would write $units/gate-prune.service and $units/gate-prune.timer, with %h -> $HOME and the repo -> $repo"
  sed "s|%h/dark-incorporated/bevy_dark_factory|$repo|" "$here/gate-prune.service"
  echo "---"
  cat "$here/gate-prune.timer"
  echo "---"
  echo "systemctl --user daemon-reload && systemctl --user enable --now gate-prune.timer"
  exit 0
fi

command -v systemctl >/dev/null 2>&1 || { echo "install-gate-prune: systemctl is not on PATH; this installs a systemd user timer, so it runs on big (Linux) only" >&2; exit 1; }
systemctl --user show-environment >/dev/null 2>&1 || { echo "install-gate-prune: no systemd user manager here; run this on big" >&2; exit 1; }
mkdir -p "$units"
# `%h` is systemd's home specifier, right for any user; the repo path is this checkout's, so it is written out.
sed "s|%h/dark-incorporated/bevy_dark_factory|$repo|" "$here/gate-prune.service" > "$units/gate-prune.service"
cp "$here/gate-prune.timer" "$units/gate-prune.timer"
systemctl --user daemon-reload
systemd-analyze --user verify "$units/gate-prune.service" "$units/gate-prune.timer"
systemctl --user enable --now gate-prune.timer
systemctl --user list-timers gate-prune.timer --no-pager
