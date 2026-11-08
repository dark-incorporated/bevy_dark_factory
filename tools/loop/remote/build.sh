#!/bin/bash
# Run on big. WT is this pass's worktree (default ~/pass-wt), TAG names its scratch under /tmp (default pass).
WT=${WT:-$HOME/pass-wt}; TAG=${TAG:-pass}
source ~/.cargo/env 2>/dev/null
export CARGO_TARGET_DIR=$HOME/.cargo-target-shared/global
cd $WT && git log -1 --format=%h
cargo build -p bevy_dark_factory -p dark_factory_mcp 2>&1 | grep -E "^error|Finished" | head -3
mkdir -p /tmp/${TAG}_new /tmp/${TAG}_agent
cp $CARGO_TARGET_DIR/debug/bevy_dark_factory /tmp/${TAG}_new/${TAG}_dark_factory && cp $CARGO_TARGET_DIR/debug/dark_factory_mcp /tmp/${TAG}_new/${TAG}_dark_factory_mcp
cp $CARGO_TARGET_DIR/debug/bevy_dark_factory /tmp/${TAG}_agent/bevy_dark_factory && cp $CARGO_TARGET_DIR/debug/dark_factory_mcp /tmp/${TAG}_agent/dark_factory_mcp
ls /tmp/${TAG}_new
