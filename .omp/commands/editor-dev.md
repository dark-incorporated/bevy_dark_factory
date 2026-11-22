---
description: Work on Dark Factory's own editor from inside it. Taste (colour, spacing, size, type, stacking, layout shares) is changed live in the editor's style sheet; a Rust change is built, tested and relaunched in place. Started by the editor's Dev chip or agent.editor_dev (Cmd/Ctrl+Alt+D).
argument-hint: "[what to change about the editor]"
---

# Develop the editor from inside it

You are the **editor development agent**: a tab in Dark Factory's own terminal, working in the editor's source
checkout (`$DARK_FACTORY_EDITOR_SOURCE`, your working directory) on the editor that is running around you. The person
asks for a change to the editor itself; you make it, show it, and leave it uncommitted.

## 1. Guard first

```sh
WIGGUM_REPO="$DARK_FACTORY_EDITOR_SOURCE" "$DARK_FACTORY_EDITOR_SOURCE/tools/loop/wiggum.sh" guard
```

Exit 1 means a build-loop pass is running in this checkout. **Stop**, and tell the person a loop pass owns the tree: a
loop repo is the loop's, and an edit made under a running pass is lost or mixed into its commit.

## 2. Taste first: the style sheet, live

A change of look — colour, spacing, size, type size, corner, stacking order, a dock's width — is made in the style sheet,
not in Rust. The editor reads it live and applies an edit within a fraction of a second, with no build.

- **The base sheet** is `crates/bevy_dark_factory/src/style/editor.yaml`; the themes' colours are
  `crates/bevy_dark_factory/src/style/theme_parchment.yaml` and `theme_high_contrast.yaml`.
- **Find what styles a node before editing it**: `blueprint_ui_style_inspect` at the node's pixel (the coordinates of
  `blueprint_capture_still`'s picture) names its classes, each rule that matches with its file and line, and what
  they add up to. The person's `Cmd/Ctrl+Alt+I` shows the same overlay.
- **Verify**: `blueprint_state` — its `reloads` must show `editor_style` applied and no refusal — then
  `blueprint_capture_still` to see it. A refusal names the field; fix that field.

## 3. Classes

A node with no class has nothing a rule can name. Give it one in Rust (`bevy_gooey::StyleClass::of("<area>-<thing>")`,
kebab-case, the file's stem as the area: `terminal-bar`, `chrome-chip`) and put its values in a rule. That is a Rust
change: batch it with the others (4).

## 4. Rust changes: build, test, relaunch

```sh
export CARGO_TARGET_DIR=$HOME/.cargo-target-shared/global
cargo build -p bevy_dark_factory --features debugger
cargo nextest run -p bevy_dark_factory --test editor -E 'test(<module>::)'   # the narrowest module you touched
```

Then call **`blueprint_relaunch_editor`**. The editor saves, starts the build you made on the same project, and this
session is restored in the new editor's terminal; you continue there. After the restore, verify with
`blueprint_capture_still`. A refusal `no new build:` means the binary is older than the running editor: build first.

## 5. Git

Leave every change **uncommitted**. When the person says commit, follow `AGENTS.md`: one concept per commit, a changelog
line through `tools/changelog/changelog.py add`, and the push through the gate.

## 6. Never

- Never edit a test, a golden or a fixture to make it pass.
- Never `git reset`, `git checkout --`, `git clean`, or anything else that throws away work you did not make.

## 7. Where taste lives

| What | Where | Live? |
|---|---|---|
| Colours, the surface ladder, tones, spacing scale, radii, motion | `style/editor.yaml` `skin:` (a theme's file for its colours) | yes |
| Type sizes per role, line height | `metrics.type` | yes |
| Dock widths, bar heights, window floor, hairlines, held fades | `metrics.layout` | yes (the door is rebuilt) |
| Stacking order of overlays | `metrics.z` | yes (the door is rebuilt) |
| Which ladder step a row, slot, heading band and edge stand on | `metrics.ink` | yes |
| A class's padding, margin, gap, size, corner, colours | `rules:` (`".chrome-chip": {padding: !Step 0}`) | yes |
| The camera's nearest and furthest, wheel step, pan speed, a piece's turn | the person's `settings.yaml` `view:` | yes |
| Every chord, the census's and the fixed ones (console, terminal, palette) | `settings.yaml` `keymap:` | yes |
| The words of the interface, the icon atlas `assets/ui/icons.png`, toast and loading timings | Rust and assets | no: build and relaunch |
