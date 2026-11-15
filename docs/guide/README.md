# Guide

Read these in order the first time through. The editor opens every page below from its Help menu, and a panel's header carries a `?` that opens the page covering what that panel holds.

- [Getting started](getting-started.md): install, open the editor on the project, and make your first map.
- [Building maps](building-maps.md): place, fill and undo pieces on a map.
- [Kits and labels](kits-and-labels.md): group meshes into kits and label each piece.
- [Agents](agents.md): let an agent drive the editor alongside you, with snips, a shared undo, a style to follow, and a hand-back you keep or take back.
- [Your game](your-game.md): use the crates in your own Bevy game.
- [Keys](keys.md): every key by context, the ten workspaces, the finder and command palette, and the terminal.
- [Data files](data-files.md): the files a project keeps and a game loads, the design folder and the editor's settings.
- [Behaviours](behaviours.md): the components a game gets from `bevy_grimoire` — triggers, doors, health, countdowns, paths and state machines — and the YAML that sets them.
- [Scripts](scripts.md): behaviour a component's fields cannot describe, as a `.rhai` file a game runs on an entity, reloaded when you save it.
- [The reload contract](reload-contract.md): every kind of file the editor writes, and what a running program does when one changes.

Reference:

- [MCP tools](../mcp-tools.md): every tool an agent can call, with its arguments.
- [The benchmark](../../bench/README.md): measure a real agent against a real editor, and [the committed runs](../bench/README.md).
- [The development loop](loop.md): how the project is built, one iteration at a time.
