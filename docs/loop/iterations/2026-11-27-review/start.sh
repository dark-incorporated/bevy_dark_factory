#!/bin/bash
# Review pass 2026-11-27: Xvfb + the editor on a scratch copy of the repo's own project. Pidfile, never pkill -f.
set -u
RUN=$HOME/fleetscratch/rv5; W=${1:-1600}; H=${2:-1000}; D=${3:-98}
export DISPLAY=:$D
if [ -f $RUN/editor.pid ]; then kill "$(cat $RUN/editor.pid)" 2>/dev/null; sleep 1; fi
if [ -f $RUN/xvfb.pid ]; then kill "$(cat $RUN/xvfb.pid)" 2>/dev/null; sleep 1; fi
Xvfb :$D -screen 0 ${W}x${H}x24 -nolisten tcp >$RUN/xvfb.log 2>&1 &
echo $! > $RUN/xvfb.pid; sleep 2
cd $RUN/proj || exit 1
HOME=$RUN/home XDG_DATA_HOME=$RUN/home/data XDG_CONFIG_HOME=$RUN/home/config \
  BEVY_BRP_PORT=15791 SHELL=/bin/bash \
  nohup $RUN/bin/bevy_dark_factory . bedroom --workspace map > $RUN/run.log 2>&1 &
echo $! > $RUN/editor.pid
for i in $(seq 1 150); do
  if curl -sf -X POST http://127.0.0.1:15791 -d '{"jsonrpc":"2.0","id":1,"method":"rpc.discover"}' 2>/dev/null | grep -q bevy; then
    echo "brp up after ${i}s (pid $(cat $RUN/editor.pid), ${W}x${H} on :$D)"; exit 0
  fi
  sleep 1
done
echo "brp never came up"; tail -30 $RUN/run.log; exit 1
