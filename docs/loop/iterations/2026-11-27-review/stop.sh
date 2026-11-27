#!/bin/bash
RUN=$HOME/fleetscratch/rv5
for f in editor xvfb; do [ -f $RUN/$f.pid ] && { kill "$(cat $RUN/$f.pid)" 2>/dev/null && echo "stopped $f $(cat $RUN/$f.pid)"; rm -f $RUN/$f.pid; }; done
