# A panel's authoring format

**Issue #488** (`feedback-1794784713125`). Research, with measurements, not a design handed down.

The owner's note, in full:

> *"Can we replace this with a mix of custom widgets, but also markdown? My idea is to have generic widgets to generate markdown. But also parsing markdown (proper markdown) back into those control widgets. And those that don't map directly, get a custom widget wrapper. Or maybe this is dumb and needs to be YAML and properly parsed with an LSP. Let's research this thoroughly as what fits best. I feel like probably a mix of markdown and YAML, with a parser and LSP to guide users and agents."*

**Status: research and a recommendation. Nothing was re-implemented.** Issue #488's third acceptance bullet — "No panel is re-implemented before this is answered" — is honoured: this branch changes no file under `crates/`.

Everything below was measured in this worktree at `4ed4e330`, `bevy_gooey` at the pinned rev `f1904be`.

---

## 1. The option table, verbatim from #488

| # | Option |
|---|---|
| 1 | **Markdown with embedded directive blocks**, parsed back into gooey widgets by a directive parser, custom types falling back to a wrapper |
| 2 | **YAML as the data, Markdown as the presentation**, with an LSP giving completion and diagnostics over the YAML |
| 3 | **A typed schema that generates both** — the panel draws it, and a text form (Markdown or YAML) is a projection of it, so neither is the source of truth |
| 4 | **None of these**, with the reasoning written down |

The note also says "I feel like probably a mix of markdown and YAML, with a parser and LSP". §5 answers that directly: the mix exists in the editor already, but it is split by **what the thing is**, not by panel.

---

## 2. What a panel is, measured

The shell registers five dock panels in `ui.rs::register_the_shell_panels` (`inspector`, `viewport`, `place`, `status`, `reload`), but that is only the dock's naming. A panel a person actually reads is built by `chrome::panel_root(...)`, and it is called **16 times**, from 13 spawn functions in 11 files:

| Panel | File | Spawn function | Lines in that function | Root call line |
|---|---|---|---|---|
| AGENT | `agent_view.rs` | `spawn_agent_panel` | 19 | 188 |
| ANIMATION | `anim_tab.rs` | `spawn_panels` | 67 (both panels) | 363 |
| RIGS | `anim_tab.rs` | `spawn_panels` | — | 406 |
| COMPOSE | `compose.rs` | `spawn_compose_panel` | 65 | 620 |
| DESIGN | `design_view.rs` | `spawn_the_design_workspace` | 38 (both panels) | 442 |
| BOARD | `design_view.rs` | `spawn_the_design_workspace` | — | 472 |
| INSPECTOR (Map) | `editor.rs` | `spawn_panel` | **300** | 3732 |
| PLACE | `editor.rs` | `spawn_palette_panel` | 28 | 4076 |
| FILES | `files.rs` | `spawn_the_files_workspace` | 36 | 694 |
| GAME | `game_view.rs` | `spawn_game_panel` | 36 | 564 |
| LIBRARY | `library_browser.rs` | `toggle_the_library` | 44 | 182 |
| PROJECT | `project_page.rs` | `spawn_the_workspace_page` | 33 | 397 |
| CONTROLS | `tiles.rs` | `spawn_tiles_panel` | 141 (both panels) | 4558 |
| TILES | `tiles.rs` | `spawn_tiles_panel` | — | 4630 |
| INSPECTOR (World) | `workspace_panels.rs` | `spawn_world_panel` | 47 | 73 |
| INSPECTOR (Sound) | `workspace_panels.rs` | `spawn_sound_panel` | 28 | 122 |

Two more families sit under these and are spawned into them rather than being roots of their own: `world_lists.rs` (396 lines, the WATER/FOREST/LIGHTS/CUES lists) and `audio_panel.rs` (229 lines).

**What a panel declares.** The largest by far is the Map INSPECTOR (`editor.rs` `spawn_panel`, lines 3731–4030, 300 lines): **9** `chrome::section` headings (`MAP SIZE  (m)`, `FAMILY SETTINGS`, `CROWD`, `DATA`, `SCRIPTS`, `NOTES`, `PLAN`, `PROBLEMS`, `COMPOSITIONS`), a 12-variant `Field` enum drawn as label/value readout rows, three `text_field`s for map size, and a live list of every section's controls. (`TERRAIN` is a tenth heading a person sees in the same panel, but it is built by `spawn_terrain_section` at `editor.rs:4039` and called from **both** this panel and the World panel, `workspace_panels.rs:99` — the one section two panels share.) The rest are small: AGENT is 19 lines, PLACE 28, SOUND 28.

**The whole production `spawn_*` surface** (functions outside `#[cfg(test)]` modules): **74 functions** across 42 files, **3,219 lines** of body, and — the number that decides this document — only **251 authored string literals**. That is **12.8 lines of Rust per string a person reads.**

That ratio is the finding. A panel is not mostly data. A panel is mostly *procedure*: read the project, filter it, decide which of four states to draw, wire an observer, and place the result. Extracting the 251 strings into a text file would leave ~3,000 lines of behaviour behind, and the text file would need a way to express everything the procedure does or the panel breaks.

### Where a panel's rows come from

| Panel | Rows read from |
|---|---|
| INSPECTOR (Map) | `map` YAML + **live editor state** (undo stack, cursor, selection) |
| PLACE | the open kit's library, in memory |
| CONTROLS / TILES | kit library + `map` YAML |
| ANIMATION / RIGS | `assets/blueprint/rigs.yaml` + measured GLB clips |
| COMPOSE | `compositions.yaml` |
| WORLD | `map` YAML, `water_plan`, forest, `bevy_lights` family values |
| SOUND | `family/bevy_siren.yaml` |
| GAME | **a running game over BRP**, and its log file |
| AGENT | **the live turn ledger** |
| DESIGN / BOARD | `design/**/*.md` |
| FILES | the project's text files |
| PROJECT | `project.yaml`, the kits, the agent guide |
| LIBRARY | a kit's `library.yaml` |

**Two of the sixteen draw live runtime state that no file holds** — GAME (a read over BRP) and AGENT (the turn ledger) — and three more mix computed values into an otherwise-authored panel or list (INSPECTOR's undo/cursor readouts, PLACE's armed-operation state, CONTROLS' armed tool). A text format cannot be the source of truth for a value the editor computed this frame from where the pointer is, which is why those get no text form rather than a fake one.

**The sixteen, sorted by what a text form could be:**

| Class | Count | Panels |
|---|---|---|
| **Tool** — a viewer or editor of something already structured (a tree, a grid, a code editor, picture cards) | **6** | FILES, DESIGN, BOARD, CONTROLS, TILES, PLACE |
| **File-backed** — rows are a projection of a project file that already has a schema | **7** | ANIMATION, RIGS, COMPOSE, LIBRARY, PROJECT, INSPECTOR (World), INSPECTOR (Sound) |
| **Live runtime** — no file exists at all | **2** | GAME, AGENT |
| **Hybrid** — authored fields plus computed readouts in one panel | **1** | INSPECTOR (Map) |

That split, not a slogan, is why the recommendation is scoped: a text form is meaningful for the 7 file-backed panels and the authored half of the 1 hybrid — and for the other 8 it would be describing something that is not a document.

### What the editor already does in text

This is the part the note does not mention and it changes the answer.

`design_view.rs` (1,299 lines) renders Markdown pages, and those pages are **not free prose**: they are the owner's own named-field documents. `crates/bevy_dark_factory/src/design_templates/` holds six templates, `style.md` with 9 fixed `##` sections and five topic kinds (dialogue, interaction, names, sound, ui) with 3–4 each — **26 named fields** across the six, each one a heading with one italic prompt line under it:

```markdown
## Voice

*Who speaks here and how they sound, in a line or two.*
```

That is exactly the screenshot the note was taken on: DESIGN on the left, `design/dialogue.md` in the middle with Voice / Register / Never said / Sample lines, and a toggle labelled **"As an agent reads it"**.

So the editor has already shipped option 1's shape — *Markdown carrying named fields* — for **pages**, and it works. Three facts about it matter here:

1. **The fields are convention, not schema.** `STYLE_H2S` (`design_templates.rs:34`) names the nine style headings, and its only readers are two tests (`a_new_project_starts_with_a_style.rs`, `new_topics_start_from_templates_in_the_design_workspace.rs`) checking that a *new* page has them. Nothing validates an *edited* page: `files_lang::markdown::diagnose` checks only that a cited picture's path exists (`design::cites`). A person can delete `## Voice` and the page still renders, and the agent's `blueprint_style` still serves it.
2. **Markdown here has no directive parser, and the parser underneath offers none.** `files_lang/markdown.rs` is 302 lines: a tokenizer for colour, folds, the picture/board completion, a citation diagnostic. `pulldown-cmark` 0.13.6 is the parser (pinned in `Cargo.lock`; gooey's `markdown` feature uses the same crate). Its `Options` bitflags are, read from `~/.cargo/registry/src/*/pulldown-cmark-0.13.6/src/lib.rs:651-735`: tables, footnotes, strikethrough, tasklists, smart punctuation, heading attributes, YAML-style and pluses metadata blocks, old footnotes, math, GFM, definition lists, superscript, subscript, wikilinks. **There is no directive flag**, and the single `directive` occurrence in the crate's source is `scanners.rs:1503`, an XML processing instruction.
3. **gooey's own Markdown reader has no block for it either.** `bevy_gooey/src/markdown.rs:184` `enum Block` has Heading, Paragraph, Code, Quote, List, Table, Rule, Html — and nothing that could carry a widget.

### What gooey actually offers the editor

| Measured | Count |
|---|---|
| `Role` variants (the skin's vocabulary, `skin.rs:1512`) | **240** |
| Public constructor functions across `src/*.rs` | 341 |
| `src/widgets.rs` alone | 106 fns, 4,994 lines |
| Modules in `src/` | 89, 58,092 lines |
| Distinct `bevy_gooey::` items the 15 panel files name | **32** |
| `chrome::` helpers the crate calls, of 67 defined | 53 |

The panels use **32 of gooey's 341 constructors** — column, row, label, button, cell, toggle, text_input, text_area, card_grid, table, tree, code_editor, markdown_view, inspector and so on. The editor's own `chrome.rs` (4,598 lines, 67 public helpers) is the layer that actually binds a panel to the skin, and **53 distinct helpers** of its 67 are called (842 call sites across the crate).

**Any text format is defined against `chrome`'s 53 used helpers and gooey's 32 used items, not against 240 roles** — and many `chrome` helpers are layout (a margin, a gap, a row), not a construct a text format would name. That is the good news for all four options: the surface to describe is ~30 constructs, not 240.

### The schema apparatus already shipped

| Thing | Measured |
|---|---|
| `blueprint_model::reload::KINDS` (file kinds the loader knows) | **38** `kind(...)` entries |
| `blueprint_model::schema::KINDS` (kinds with a generated JSON Schema) | **24** |
| `schema::for_kind` | one arm per kind, `schemars`-generated |
| Schema location | `<root>/.dark_factory/schema/<kind>.schema.json`, rewritten on every open |
| Schema hint line | `# yaml-language-server: $schema=…` (`schema.rs:55`), written by `with_header` |
| Schema proof | `crates/blueprint_model/tests/schema_describes_every_file_kind.rs`, 10 tests |
| `files.rs` schema consumer | `schema_for` (`files.rs:216`), called 5 times, caching one `Arc<Value>` per kind |
| Completion / hover / diagnose over that schema | `files_lang/yaml.rs` — 807 lines, `complete_yaml` at 667, `hover_yaml` at 716 |
| `yaml_surgery` (byte-preserving leaf edits) | 1,035 lines, 8 public fns, a 595-line proof test with 33 `#[test]`s |
| `setting_catalogue` | **517 lines, 17 rows**, 5 categories, 3 `Kind`s |
| `setting_catalogue`'s hand-written words | `label`, `category`, `description` per row |
| `gooey::inspector` (reflect → rows) already used | **2 sites**: `inspector_view.rs` (`PoseView`, 4 fields), `family_panel.rs` (`FamilyView`, 14 fields) |

The last row is the most important thing in this document, and the prior write-up missed it.

**The editor already has option 3, working, twice.** `PoseView` (4 fields, `inspector_view.rs:103`) and `FamilyView` (14 fields, `family_panel.rs:336`) are `#[derive(Reflect)]` structs that `bevy_gooey::inspector` turns into a row per field, and writes edits back through an observer that dispatches the operation. `inspector_view.rs`'s header states the rule that makes it safe: the view is the editor's own type, never the model's, so there is one writer per fact and the write goes through the operation that owns it.

**And the mapping from type to widget is mechanical, not hand-written.** `bevy_gooey/src/inspector.rs` (`build_inspectors`, line 524) chooses the control from the field's `Reflect` type — measured at its spawn sites:

| Field's type | The control gooey spawns |
|---|---|
| `bool` | `toggle` (line 595) |
| a number | `number_field`, with range and step from `#[reflect(@RangeInclusive…)]` and `#[reflect(@InspectStep)]` (602) |
| a unit enum | `choice`, its options from the variant names (612) |
| `Vec2` / `Vec3` / `Vec4` | `vec_field` (623–630) |
| `Srgba` / `Color` | `color_field` (642) |
| `String` | `text_input` (648) |
| a nested struct | `section` of its own fields (659) |

**A field's doc comment is its tooltip** — `docs_of` at `inspector.rs:320` joins the comment's lines into one, and the type's own doc says so verbatim: *"A field's doc comment is its tooltip"*. That is the same text `schema.rs` puts in a JSON Schema's `description` ("A field's doc comment as its `description`, which is what hover shows a person and what an agent reads"). **One comment already serves both readers** once the type is the panel.

So "a typed struct, drawn as rows, with its own text form" is not a proposal; it is shipped twice in miniature — and `family_panel.rs` is 665 lines around a 14-field table, which is the cost of the *behaviour* that a text format would still not express.

### The one gap between the two halves

This is the honest cost of option 3, measured: **`gooey::inspector` reads `Reflect`, and `schema::for_kind` writes `schemars`, and `bevy_dark_factory` has neither a `JsonSchema` derive nor a `schemars` dependency at all** (0 hits for `JsonSchema`, 0 for `schemars::`, and no `schemars` line in `crates/bevy_dark_factory/Cargo.toml`; the derives live only in `blueprint_model`, for example `board.rs`, `composition.rs`). A panel type that must be *both* a drawn panel and a schema'd file therefore needs `#[derive(Reflect, JsonSchema)]` on one struct, a `register_type`, **and `schemars` added to the editor's `Cargo.toml`** — the dependency is already in the workspace (`Cargo.toml`, used by `blueprint_model`), so it is one line. Two derive lines, one registration and one dependency line per panel kind: not a new mechanism, but real, and the only genuinely new work option 3 requires.

---

## 3. What the field has settled on

| Approach | What it is | Where | Why it matters here |
|---|---|---|---|
| Generic directives | `:name[text]{key=val}`, `::leaf`, `:::container` | remark-directive (JS), `markdown-office/commonmark-generic-directives` (PHP) | **Still a proposal, not CommonMark.** Latest spec is 0.31.2 (2024-01-28); the priority is core elements, and extensions wait. **No Rust parser implements it**, and `pulldown-cmark` has no flag — §2 measured its `Options`. |
| MDX | Markdown + JSX, compiled to a JS module | Next.js, Astro, Docusaurus (opt-in) | Compiles to **code**, needs a JS runtime. Not expressible in a Bevy UI tree without an embedded JS engine. |
| Nuxt MDC | `::component{props}` with YAML props and `#slot` blocks | Nuxt Content only | Closest to the note's first idea, **vendor-locked**, and its props are *already* YAML. |
| LSP over typed data | Schema declares the shape; a server completes, hovers, diagnoses | VS Code + `yaml-language-server`, `taplo`, `vscode-json-languageserver` | The settled answer where a person and a machine edit one file. Note the editor **doesn't need a server**: `files_lang` is an in-process implementation of the same idea, reading the same schema. |
| Markdown prose + data file | Prose in Markdown, structure in a data file | Hugo, Zola, mdBook, Sphinx | The common split: prose is what Markdown is good at, structure is a data file. |

The pattern: every "widgets inside Markdown" system either owns a JS compiler (MDX), is vendor-locked (MDC), or is an unimplemented proposal (directives). The pattern for "a machine and a person edit one structured file over years" is a **schema plus a reader that guides both** — and this repository has that reader, in-process, at 807 lines of `files_lang/yaml.rs`.

---

## 4. Measured cost per option

Cost is code a pass must write **and keep**. Numbers are from the table in §2; where an estimate is a range it is stated as one, because a prototype that does not exist cannot be measured.

### Option 1 — Markdown with directive blocks

| What it changes | Measured cost |
|---|---|
| A directive grammar | **New, and nothing to lean on.** `pulldown-cmark` has no directive flag (measured, §2), so this is a parser: container nesting by colon count, attribute parsing, inline vs leaf vs container, an error for an unterminated block. Realistically 600–900 lines plus tests, in a new `files_lang` module or beside gooey's `markdown.rs`. |
| `files.rs` (1,632 lines, 68 fns) | A second Markdown mode: directives highlit, completed, diagnosed. `schema_for` becomes irrelevant to pages. ~150 lines, and a second `Lang` path that the 5 existing `schema_for` call sites do not share. |
| `design_view.rs` (1,299 lines) | The renderer must decide which constructs become widgets, and pages and panels converge on one renderer. ~200 lines, plus the widget-embed path through `markdown_view`. |
| `setting_catalogue.rs` (517 lines) | No change: a panel's rows are not settings. |
| Panels migrated | **16 of 16, all at once** — a text format is only a format if it is the truth. And the 8 that are tools or draw live runtime state (§2) each need a directive that means "computed, not authored". |
| Two paths? | **Yes, by construction.** The note says it: "those that don't map directly, get a custom widget wrapper". A wrapper widget's contents cannot be diffed, cannot be completed, and cannot be diagnosed — so the panels most worth editing are the ones a diff describes worst. |

**The killer, measured:** a panel is 12.8 lines of Rust per authored string (§2). The directive file would have to express the *procedure*, not the string, or the panel cannot be rebuilt from it.

### Option 2 — YAML data + Markdown presentation, with an LSP

| What it changes | Measured cost |
|---|---|
| An LSP server | **Zero lines, because the editor does not need one.** `files_lang/yaml.rs` is 807 lines of completion/hover/diagnose reading `schema::for_kind`, already wired to `code_editor`. A *separate* process would be a second implementation of 807 lines to keep in step. If the owner wants outside editors (VS Code), `schema.rs` already writes the hint line and the schema files. |
| `files.rs` | **Unchanged.** `schema_for` already serves the editor. |
| `setting_catalogue.rs` | A panel's YAML needs the same one-row-per-key-with-words the catalogue does. Either a second catalogue (a fact stated twice) or `Setting` generalised: ~120 lines + a schema-derivation walk over 17 rows. |
| `design_view.rs` | Unchanged; prose stays Markdown, the panel is a view of the YAML. |
| Panels migrated | Only those whose rows **are** data: ANIMATION + RIGS (67), COMPOSE (65), INSPECTOR World (47), INSPECTOR Sound (28), LIBRARY (44), PROJECT (33) — **7 of 16**, 284 lines of spawn bodies. The other 9 are 6 tools, 2 live-runtime and 1 hybrid, and a YAML file cannot be their truth. |
| Two paths? | **No, as long as nobody writes the YAML and the panel to disagree.** This is the option's real risk: it makes the schema a *convention* unless something enforces the panel reads only the file. |

### Option 3 — A typed schema that generates both

| What it changes | Measured cost |
|---|---|
| A schema per panel kind | One `schemars` root each, added to `schema::KINDS` (24 → N). Cheap to derive; **expensive to word**, because the label and description per field must come from doc comments, exactly as `FamilyView`'s 14 fields already do. |
| The panel drawn from it | `bevy_gooey::inspector` already does this — **2 shipped sites**, `PoseView` 4 fields and `FamilyView` 14 fields. A panel that is a field table becomes a `Reflect` struct + an observer. `inspector_view.rs` (331 lines) is the whole pattern, including the "view, not the model" rule that keeps one writer per fact. |
| `setting_catalogue.rs` | Its 17 hand-kept rows become one *reader* of the schema rather than a table. This is the only place the option is a net **deletion** of a fact stated twice — and `the_settings_screen_searches_and_resets.rs:237` already asserts `keys == CATALOGUE.len()`, so the guard exists to move. |
| `files.rs` | Unchanged: the text form is YAML, and `schema_for` already completes it. |
| `design_view.rs` | Unchanged. |
| Panels migrated | The **field-table** panels: INSPECTOR's `Field` rows, FAMILY (14 fields, already gooey's), POSE (4, already gooey's), MAP SIZE (3 fields), plus any new panel born this way. **Not** the 2 live-runtime panels (GAME, AGENT) and **not** the 6 tool panels (FILES' code editor, DESIGN's tree, BOARD's cards, CONTROLS' and TILES' grids, PLACE's list) — none is a field table, and COMPOSE, though file-backed, reads and analyses rather than presenting a form. Forcing any of them into one would be a second path. |
| Two paths? | **No**: one struct, one drawer, one text projection. A field cannot be added to the panel without adding it to the type, and cannot be in the type without the YAML schema offering it. |

### Option 4 — None of these

| What it changes | Measured cost |
|---|---|
| Nothing | Zero. |
| What it costs | An agent still cannot propose a panel change as a diff; a person still edits a panel only by pointer. But §2 measured that only 7 of 16 panels are file-backed and 1 is a hybrid, and the two field-table blocks already inside an INSPECTOR panel (FAMILY, 14 fields; POSE, 4) **already** have a typed, agent-reachable surface through `gooey::inspector` — and the *data* those panels edit (`family/*.yaml`, `map/*.yaml`) is **already** YAML with a schema and completion. |

**Option 4 is much stronger than it first looks**, and this is the finding that separates this document from the prior write-up. The gap the note points at — "an agent cannot propose a panel change as a diff" — is **already closed for the panels whose content is data**, because that content is a project YAML file the agent edits with `yaml_surgery`, and the panel re-reads it. What is *not* closed is the panel's **shape and wording**: which rows exist, in what order, called what. That is the only genuinely missing thing, and it is a much smaller question than "what is a panel's authoring format".

---

## 5. Recommendation

**Option 3, scoped narrowly: a panel's *field table* is a typed `Reflect` struct drawn by `bevy_gooey::inspector`, and its text form is the YAML schema the same struct generates. Panels that are tools (a code editor, a tree, a grid) or that draw live runtime state are not given a text form at all — and that is the recommended answer, not a deferral.**

The reasoning, in the order the measurements support it:

1. **It is already shipped twice** (`PoseView` 4 fields, `FamilyView` 14 fields). The recommendation is to finish a pattern the editor uses, not to invent one. That is the cheapest possible version of "no second path".
2. **The schema half is shipped too**: `schema::for_kind` answers 24 kinds, `files.rs::schema_for` completes them, `files_lang/yaml.rs` (807 lines) reads them, and `yaml_surgery` (1,035 lines, 33-test proof) writes one leaf without moving a comment. Option 3's "generate both" needs only the derive and the inspector call.
3. **It matches the repository's own rule** that a view is the editor's type and one writer owns each fact (`inspector_view.rs`, stated in its header and proved by `pose_edits_are_operations`).
4. **Option 1 is refused on a measurement, not a taste**: 12.8 lines of Rust per authored string means a directive file must express procedure, and a wrapper widget for what does not map is a diff nobody can read — two paths for one panel by construction. There is also no Rust directive parser to build on (§3, measured against `pulldown-cmark` 0.13.6's own `Options`).
5. **Option 2 is three-quarters of this** and is refused only because it leaves the schema as a convention: if the panel may read the YAML *or* compute the row, nothing stops them disagreeing. Option 3 makes the struct the one declaration.
6. **Option 4's honest version is a real answer for 8 of 16 panels (6 tools, 2 live runtime)** — which is why the recommendation is scoped rather than total. Applying a text format to GAME (a live BRP read) or FILES (a code editor) would be inventing a worse format for something that is not a document.

### What an agent gains, per form

| | Which form, and why |
|---|---|
| **An agent can read it** | **The YAML**, for a field-table panel's *content* — one line per value, and the panel's own file is the same file the game loads (`family/bevy_siren.yaml` is "the file a person edits here is the file a game ships", `audio_panel.rs`). For a panel's *shape*, **the Rust struct's doc comments**: `schema.rs` turns each into the JSON Schema's `description`, which is what hover shows and what an agent reads. |
| **An agent can diff it** | **The YAML, one leaf at a time.** `yaml_surgery::set` replaces one scalar's bytes and leaves every other byte — comments, blank lines, key order, alignment — as it was (1,035 lines; 33 tests in `yaml_surgery_keeps_every_comment.rs`). A diff of a panel edit is a diff of the value that changed. A *shape* change is a diff to a Rust struct, which is a reviewed code change, not a data tweak — correctly harder. |
| **A person can edit it** | **The panel**, for the shape (a typed control per field, as `FamilyView`'s 14 fields show), and **the YAML in the Files workspace** for content, with the schema completing, hovering and diagnosing as they type (`GD-11` #373, `GD-12` #374). A person who wants prose writes prose — in Markdown, on a page, where it already works (§2). |

That is the answer to "a mix of markdown and YAML, with a parser and LSP to guide users and agents": **the mix is Markdown for prose and YAML for values, the split is by *what the thing is* rather than by panel, and the "LSP" is already in-process** — 807 lines of `files_lang`, driven by a schema generated from the Rust type. What this issue adds is the last step: the *shape* of a field-table panel declared in that same type.

### Its costs, stated

1. **Wording migrates by hand.** 17 `setting_catalogue` rows carry hand-written `label`/`category`/`description`; moving them to doc comments means writing the words again, with care, because hover text is what an agent reads. This is the one place the option costs more than it deletes.
2. **A `Reflect` field table is only as good as its types.** A row that is a sentence assembled at runtime (the INSPECTOR's receipt line, "the NightShift placed bed@2") is not a field and stays hand-spawned. Expect field tables to be the *minority*: 2 blocks today (FAMILY, POSE), plus the INSPECTOR (Map)'s `Field` rows and MAP SIZE's 3 fields as the next candidates.
3. **No widget inside a page's prose.** If the owner ever wants `::callout` mid-paragraph, that is option 1's problem and stays unbuilt. This document recommends against building it, not that it is impossible.

---

## 6. What is NOT decided here

Every one of these is the owner's, and each is named so it can be answered on #488 rather than discovered in a prototype.

1. **Whether the editor's pages should gain a validated field set.** `design_templates`' 26 named fields are convention: `STYLE_H2S` is checked for *new* pages by two tests and for edited pages by nothing. Turning that into a diagnostic (a page missing `## Voice` says so) is a small, separate, high-value change — and it is a *decision*, because it would make a person's hand-written page fail.
2. **Whether `setting_catalogue`'s 17 rows move to schema-derived words.** The recommendation says yes; the wording is the owner's voice and cannot be derived from a type.
3. **Which panels are in scope first.** This document measures 4 candidates (FAMILY, POSE — both already done — plus INSPECTOR's field rows and MAP SIZE). The choice of the first new one is the owner's.
4. **Whether outside editors matter.** If a person edits `family/*.yaml` in VS Code, the schema and its hint line already work today and nothing is needed. If they must edit a *panel's shape* in VS Code, that is a generated file and a different question.
5. **Whether the DESIGN page's "As an agent reads it" toggle should show the YAML instead of composed Markdown** — a presentation question this research raises and does not answer.
6. **What a panel is for.** `docs/ui/vision.md` §4 says a workspace is a job and a panel is its region; nothing states that a panel must be editable as text at all. The owner's note assumes it. That assumption is the premise of this issue and is not re-litigated here beyond the measurement that 8 of 16 panels are tools or live runtime and would gain nothing.

---

## 7. Why not the other three, in one line each

- **Option 1** — two paths for one panel by construction (a wrapper widget is a diff nobody can read), and no Rust directive parser exists to build it on; measured, `pulldown-cmark` 0.13.6 has no directive flag.
- **Option 2** — three quarters of the recommendation, refused only because it leaves the schema a convention: a panel that may read the YAML *or* compute the row can drift from its file.
- **Option 4** — genuinely right for **8 of 16 panels** (6 tools, 2 live runtime), which is why the recommendation is scoped rather than total; refused as a *complete* answer only because the shape and wording of a field table has no agent-reachable form at all today.

---

## Appendix: how each number was measured

| Number | How |
|---|---|
| 16 `panel_root` calls, 13 spawn fns, 11 files | `grep -rn "panel_root(" crates/` then the enclosing `fn` per hit |
| spawn-body line counts | Python: regex the `fn`, brace-match to its close, count lines between |
| 74 production `spawn_*` fns / 3,219 lines / 251 literals | Python over `crates/bevy_dark_factory/src/**/*.rs`, skipping spans inside `#[cfg(test)]` modules, same brace match, string literals matching `[A-Za-z]` |
| 240 `Role` variants | `skin.rs:1512` to its closing brace, block comments stripped, top-level identifiers at depth 0 (dupe-checked) |
| 341 public constructors, 89 modules, 58,092 lines | `grep -cE '^pub fn '` and `ls src/*.rs` in the pinned gooey checkout |
| 32 distinct `bevy_gooey::` items used by panels | regex `bevy_gooey::([a-z_0-9]+)\s*[\(:]` over the 15 panel files, counted |
| 38 file kinds, 24 schema kinds | `reload.rs` `KINDS` `kind(...)` entries; `schema.rs` `KINDS` list |
| 17 catalogue rows, 5 categories | the `CATALOGUE` literal, `key:` rows counted |
| `pulldown-cmark`'s `Options` | read from `~/.cargo/registry/src/index.crates.io-*/pulldown-cmark-0.13.6/src/lib.rs:651-735` |
| 26 page fields across 6 templates | `grep -c '^## '` per file in `src/design_templates/` |
| 12.8 lines per authored string | 3,219 ÷ 251 |

Two corrections to `docs/design/2026-11-17-a-panels-authoring-format.md` (the earlier write-up of this issue), both measured above and neither affecting its recommendation: it states gooey's `Role` has **76** variants (measured: **240**), and it states the settings catalogue has **16** rows (measured: **17**, across 5 categories). It also misses the two shipped `gooey::inspector` sites, which are the strongest evidence *for* its own recommendation.
