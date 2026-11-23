# Review captures, 2026-11-23 (lenses 1 and 2)

Staged, not a person's Cmd+D. The editor ran headless on big
(`~/.cargo-target-shared/global/debug/bevy_dark_factory . bedroom` under `xvfb-run -a`,
`BEVY_BRP_PORT=15712`, 1360x850 surface), driven over BRP `bevy_fairy/input`, captured with
`bevy_fairy/screenshot` (`source: window`). Source at `0c74e140`: the binary was built from
`f3143e62`, whose entire difference from `0c74e140` is the version string in
`Cargo.toml`/`Cargo.lock`.

- `files-editor-opened.png` — the Files workspace with `assets/blueprint/maps/bedroom.map.yaml`
  open (opened through Cmd+P, typed, Enter).
- `minimap-after-typing.png` — that editor's minimap lane, cropped x 1195..1345,
  y 235..800 and scaled 2x.
- `header-row-2x.png` — that editor's header row, cropped y 178..210, scaled 2x.
