#!/bin/bash
RUN=$HOME/fleetscratch/f677
[ -f $RUN/editor.pid ] && kill "$(cat $RUN/editor.pid)" 2>/dev/null; sleep 1
[ -f $RUN/xvfb.pid ] && kill "$(cat $RUN/xvfb.pid)" 2>/dev/null; rm -f $RUN/editor.pid $RUN/xvfb.pid; echo stopped
