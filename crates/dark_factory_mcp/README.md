# dark_factory_mcp

> ⚠️ **Vibe Coded.** Written by AI agents working from a human's direction, covered by tests rather than a line-by-line human review. Read the code before you rely on it for something you care about.

The Dark Factory world editor over the Model Context Protocol (MCP), for people who want an AI agent to build and review a map in the editor they are looking at: an MCP server that lets the agent drive the editor through typed operations, the same ones a person at the keyboard uses, instead of synthesising keystrokes.

**Status:** experimental · MSRV 1.88

**Why:** an agent's change should be the same kind of change as yours, through the same named operations and the same undo history. The server speaks MCP over stdio and reaches the running editor over the Bevy Remote Protocol (BRP); the editor is a separate process that owns the world, and the server never writes to the project directory itself.

## Quick start

Start the editor on a project, then the server beside it. The editor's BRP transport is on by default (the `debugger` feature), and `BEVY_BRP_PORT` (default 15702) names the same socket on both sides.

```sh
# The editor.
cargo run -p bevy_dark_factory -- <project>

# The server, on stdio.
cargo run -p dark_factory_mcp -- --project <project>
```

Point an MCP client at the built binary, with absolute paths:

```json
{
  "mcpServers": {
    "bevy_blueprint": {
      "command": "/absolute/path/to/bevy_dark_factory/target/debug/dark_factory_mcp",
      "args": ["--project", "/absolute/path/to/your/project"],
      "env": { "BEVY_BRP_PORT": "15702" }
    }
  }
}
```

Leave out `env` unless you moved the port. The server does not start the editor: with none running, a tool that needs it answers with a sentence saying so and changes nothing, and the agent's first call should be `blueprint_guide`, which says in what order to use the rest.

| flag | meaning |
|---|---|
| `--project <dir>` | the project the editor is working on; a change made while the editor has another one open is refused. **Without it the server follows the editor**: whatever it has open, including none (the Projects home), and `blueprint_open_project` opens one. Tools that read project files from disk (`blueprint_decide`, `blueprint_list_rules`, `blueprint_list_snips`, `blueprint_project_feedback_list`) need it. |
| `--hide-tools <a,b>` | tool names not served at all, so an agent's tool list does not hold them; a benchmark arm, not a permission. A name this build has no tool for is refused at start. |
| `--emit-docs` | print the generated tool reference to stdout and exit. |
| `--help` | print the usage and exit. |

## An installed Dark Factory

Every macOS release carries this server beside the editor, signed with it: `Dark Factory.app/Contents/MacOS/dark_factory_mcp`. An agent on another machine attaches over ssh with no `--project`, so it follows the installed app from its first frame:

```sh
ssh <mac> "'/Applications/Dark Factory.app/Contents/MacOS/dark_factory_mcp'"
```

`blueprint_read_log` reads the editor's own log (`~/Library/Application Support/Blueprint/dark_factory.log` on macOS) from disk, so it answers when the editor does not, and `blueprint_capture_still` with `"source": "window"` shows what the window itself shows rather than what the editor drew.

## The tools

There are 30 tools, and the set is small on purpose: an agent pays for every tool it is offered, on every turn. The operation tools are a discovery surface. `blueprint_list_operations` says what the editor can do (`query` searches, `operation` answers for one with its parameters and an example), `blueprint_preview_operation` shows what one would change, `blueprint_apply_operation` runs it and records it in the operation log, and `blueprint_undo_operation` takes one back.

| group | tools |
|---|---|
| Orient | `blueprint_guide`, `blueprint_self_test`, `blueprint_state`, `blueprint_read_oplog`, `blueprint_read_log`, `blueprint_schema`, `blueprint_list_pieces`, `blueprint_list_rules` |
| Change | `blueprint_list_operations`, `blueprint_preview_operation`, `blueprint_apply_operation`, `blueprint_undo_operation`, `blueprint_run_command`, `blueprint_open_project`, `blueprint_jobs_stop` |
| See | `blueprint_capture_still`, `blueprint_capture_sequence`, `blueprint_frame_view`, `blueprint_snip`, `blueprint_list_snips` |
| Style and checks | `blueprint_style`, `blueprint_style_propose`, `blueprint_decide` |
| Hand back | `blueprint_present`, `blueprint_wish`, `blueprint_lesson` |
| Reports | `blueprint_feedback_list`, `blueprint_project_feedback_list` |

`blueprint_guide` is the one prose tool: the shipped guide, [`src/guide.md`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/dark_factory_mcp/src/guide.md), compiled in. It is the same text [`docs/mcp-tools.md`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/mcp-tools.md) is rendered from, so an agent and a person read one guide. That reference is generated, and a test fails when it does not match the server:

```sh
cargo run -q -p dark_factory_mcp -- --emit-docs > docs/mcp-tools.md
```

## Learn more

- [The MCP tool reference](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/mcp-tools.md): every tool, with its arguments.
- [Working with an agent](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/docs/guide/agents.md): setting the server up, starting an agent in the editor's terminal, snips and undo.
- [`bevy_dark_factory`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/crates/bevy_dark_factory/README.md), the editor this serves.
- [Changelog](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/CHANGELOG.md) of the repository this crate lives in.
- Part of Dark Inc.: the [family table](https://github.com/dark-incorporated) lists the other crates, and [the book page for this repository](https://dark-incorporated.github.io/crates/bevy_blueprint.html) says how they fit together.

## Who and licence

Maintained by [@Ladvien](https://github.com/Ladvien). Issues go to the [issue tracker](https://github.com/dark-incorporated/bevy_dark_factory/issues); see the organisation's [`CONTRIBUTING.md`](https://github.com/dark-incorporated/.github/blob/main/CONTRIBUTING.md).

MIT OR Apache-2.0, at your option: [`LICENSE-MIT`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/dark-incorporated/bevy_dark_factory/blob/main/LICENSE-APACHE).
