#!/bin/bash
# #677/#678 measurement: Xvfb :77 + the editor on a scratch copy of the review's project. Pidfiles, never pkill -f.
set -u
RUN=$HOME/fleetscratch/f677; D=77; PORT=15877
export DISPLAY=:$D
[ -f $RUN/editor.pid ] && kill "$(cat $RUN/editor.pid)" 2>/dev/null && sleep 1
[ -f $RUN/xvfb.pid ] && kill "$(cat $RUN/xvfb.pid)" 2>/dev/null && sleep 1
Xvfb :$D -screen 0 1600x1000x24 -nolisten tcp >$RUN/xvfb.log 2>&1 &
echo $! > $RUN/xvfb.pid; sleep 2
cd $RUN/proj || exit 1
HOME=$RUN/home XDG_DATA_HOME=$RUN/home/data XDG_CONFIG_HOME=$RUN/home/config BEVY_BRP_PORT=$PORT SHELL=/bin/bash \
  nohup $HOME/.cargo-target-frame677/debug/bevy_dark_factory . bedroom --workspace map > $RUN/run.log 2>&1 &
echo $! > $RUN/editor.pid
for i in $(seq 1 150); do
  if curl -sf -X POST http://127.0.0.1:$PORT -d '{"jsonrpc":"2.0","id":1,"method":"rpc.discover"}' 2>/dev/null | grep -q bevy; then
    echo "brp up after ${i}s (pid $(cat $RUN/editor.pid))"; exit 0
  fi
  sleep 1
done
echo "brp never came up"; tail -30 $RUN/run.log; exit 1
