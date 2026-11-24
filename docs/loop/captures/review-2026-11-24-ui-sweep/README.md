# Review captures, 2026-11-24 (the UI sweep)

Staged, not a person's `Cmd+D`. This is the sweep the owner asked for on 2026-11-24: every workspace, panel and
overlay, driven headless on `big` and captured from the editor's own swapchain.

- **Editor:** `/tmp/uireview/bin/bevy_dark_factory` (a copy, so a concurrent gate could not rewrite it), driven over
  BRP on port 15771, Xvfb `:87` at 1600x1000.
- **Fixture:** a copy of this repo's `assets/` (65 MB) plus a `data.yaml` registering a 80-leaf tuning file, a
  200-character script line, 40 `scripts/fNN.yaml`, and one library id long enough to overflow any panel:
  `a_very_long_piece_name_that_runs_off_the_end_of_any_panel_it_is_listed_in`. Library: 91 pieces; map `bedroom`:
  12 placements; 16 rigs.
- **Sizes:** every workspace at **1600x1000** and at **1100x700** (`Window.resolution.*` over BRP).
- **Capture:** `bevy_fairy/screenshot source=window`, so coordinates are surface pixels with no window manager.

## The files here

| File | What it shows |
|---|---|
| `wide-ws-animation.png` | the Animation workspace at 1600x1000 — the PLOTS legend runs past the inspector (#596) |
| `crop-legend-overflow.png` | that legend at 2x, x 280..420, y 640..700 |
| `crop-status-chips.png` | the status band's left group — two unlabelled `off` radios (#597) |
| `F-files-1600.png` | the Files workspace at 1600x1000 |
| `crop-empty-box.png` | the unlabelled 153x29 focusable box in that pane (#599) |
| `wide-ws-sound.png` | the Sound workspace, whose whole content is "this build has no audio panel" (#600) |
| `chk-j0.png`, `chk-j1.png` | the terminal maximised and collapsed, after three `Cmd+J` presses that changed nothing (#598) |

The issues that cite these carry the measurements in their bodies; these are the evidence, not the report.
