# The loop's measuring tools

Four scripts, one purpose: the loop stopped measuring because the tools lived outside the repo. They
live here now, are Python 3 standard library only, and never write to a person's project — every run
copies an `assets` directory to `/tmp` first and closes the editor it launched by its own pid.

**`common.py`** — what the others share: the repo path, `binary(name)` (a workspace binary looked up
through `cargo metadata`'s target directory, so whatever `CARGO_TARGET_DIR` is set to on the host is
found rather than assumed), `free_port()`, `make_scratch(source, dest)`, `class Editor` (one editor
process, `wait_ready(pred)` polling `blueprint/state`, closed by pid), and `class Client` (the demo
recorder's `McpClient` plus `tools/list`). It imports `McpClient` and `brp` from `tools/demo/record.py`
rather than writing a second MCP client.

```
python3 tools/loop/accept.py --source ~/sanctuary/assets --out target/loop/<date>-<sha>/accept.json
python3 -m unittest tools/loop/test_accept.py
```

**`accept.py`** — the loop's headline metric. An outside MCP client is given the brief's own sentence
(*"a mossy ruined courtyard with two exits"*) and nothing else, and ten steps are recorded: tools
published, map made, map opened, pieces placed, layout proposed, still captured, layout kept, map
saved, result presented, result verified. Each step is `pass`, `fail` (with the refusal's own text) or
`skip` (its dependency did not pass — a step nobody could attempt has not been shown to be broken).
The report carries `reached`, `of` and `first_failure`. `--glb <path>` adds an eleventh step that
imports a mesh and places it, for measuring the Blender bridge. Exit 0 only when every step passes.

**`perf.py`** — responsiveness, in the five numbers the reports quote: `frame_ms_median` (from
`bevy_debugger/status`'s frame counter over five 2 s windows), `key_p95_ms` (a keystroke until
`blueprint/state` says the armed tool changed), `file_ms` (a `yaw:` rewritten in the map file until the
watched placement's transform carries it), `mcp_ms` (the same value through `blueprint_apply_operation`),
and `load_avg_1m` — because every number above is a statement about the machine as much as the code.
Exit 1 if a latency came back `None`: a report with a hole in it gets quoted as if it were a
measurement.

**`backlog.py`** — `check` (unique ids across the working list and the archive; `kind` in
`bug`/`enhancement`/`series`; `status` in the seven the brief lists; a series carrying its goal,
`approved_by`, acceptance steps and steps; exit 1 naming each violation), `next-id B|E|S` (one past
the highest number for that prefix across both files), `archive` (move every `fixed`/`shipped`/
`rejected` item to the end of the archive, in order).

**`check-gate-hook.sh`** — every clone that can push `main` reads `core.hooksPath=tools/loop/hooks`; `--fix` sets it
where it does not. `bmb` pushes releases and had it set to `/dev/null`, which is why 7 of 9 release commits reached
`main` with no gate record (#579). `require-gate-record.sh <sha>` is the reading side, run by the release workflow:
a tag whose commit carries no `refs/notes/gated` note is refused before a DMG is built.

**`prune_gate_target.sh`** — keeps the newest test executable of each name in the push gate's target and deletes the
older ones (`CEILING_GB`, default 120, is what it is held to; over it exits 4). `gate-prune.timer` runs it daily via
`install-gate-prune.sh`; nothing ran it before, and the target reached 283 GB with 129.4 GB of superseded executables
cargo could never reuse (#578).

**`wall_clock_filter.py`** — the `threads-required = "num-cpus"` overrides in `.config/nextest.toml` name the tests
that measure the host, and this check fails when a clock-reading test is unnamed or a named test reads no clock. It
replaced six module patterns that took 28 tests, 9 of them reading no clock (#577).

**`test_accept.py`** — `summarize()`'s arithmetic, which is what the loop grades itself on: a skip is
neither a pass nor a failure, `first_failure` is the first failure in step order, a skip ahead of a
failure does not become the first failure, and a skip never satisfies a dependent.
