. /tmp/fleet/studyrun2-env.sh
export PATH=/tmp/studyrun-bin:$PATH
B=$CARGO_TARGET_DIR/debug
S="--project games/sample --suite games/sample/bench/tasks --editor /tmp/studyrun-bin/editor-seeded2 --server $B/dark_factory_mcp"
A=docs/research/2026-11-14-style-helps-an-agent-analyse.py
MODEL=anthropic/claude-sonnet-5-5
STATE=$(realpath "$HOME")/.local/state/dark_factory-bench
