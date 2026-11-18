# A panel's authoring format: Markdown with widgets, YAML with an LSP, or a schema that emits both?

**Issue #488** (feedback-1794784713125). The owner, 2026-11-15, in full:

> *"Can we replace this with a mix of custom widgets, but also markdown? My idea is to have generic widgets to generate markdown. But also parsing markdown (proper markdown) back into those control widgets. And those that don't map directly, get a custom widget wrapper. Or maybe this is dumb and needs to be YAML and properly parsed with an LSP. Let's research this thoroughly as what fits best. I feel like probably a mix of markdown and YAML, with a parser and LSP to guide users and agents."*

The owner asks for research before an answer. This is that research: what the editor already has, what the field has settled on for "a document with embedded widgets", the measured cost of each option against this repository, and one recommendation with its reason and its costs.

**Status: recommendation, not a decision made.** Nothing was re-implemented to produce this; "No panel is re-implemented before this is answered" is honoured. The prototype is a separate issue that cites this one.

---

## 1. The question, stated so it can be answered

The note points at a panel — the scene pick at map cell `[-292, 15]` — and asks whether a panel's content could be **text**, so that a person and an agent edit the same thing.

Today a panel is a tree of typed widgets (`bevy_gooey` roles) with **no text form at all**, and a prose page is Markdown with no widgets. A single fact therefore has two representations, and only one is editable as text. The gap is not "can we render Markdown" (the Design workspace does, #469/#399) and not "can we edit YAML with completion" (the Files workspace does, GD-11/#373, GD-12/#374). The gap is: **which form is the source of truth for a panel?**

Four candidate answers, as the issue states them:

1. **Markdown with embedded directive blocks**, parsed back into widgets; a construct that does not map becomes a wrapper widget.
2. **YAML as the data, Markdown as the presentation**, with an LSP over the YAML.
3. **A typed schema that emits both**: the panel draws it, and a text form is a projection, so neither text form is the source of truth.
4. None of these, with the reasoning written down.

## 2. What the editor already has

Measured in this worktree at `e82d1f3f`. These are the surfaces the decision must not duplicate.

| Surface | What it is | Size (lines) | What reads it |
|---|---|---|---|
| Prose pages | Markdown, rendered by gooey's `markdown_view`, prose face since #469 (`aee8076e`), pictures as plain Markdown (SG-00, #399) | `design_view.rs` 1,299 | `blueprint_model::design::Tree` |
| Files workspace | gooey's `code_editor`: YAML/JSON syntax, folds, minimap, completion and hover **from the kit's JSON Schema**, diagnostics (GD-11, #373; GD-12, #374) | `files.rs` 1,632 (68 fns) | every file kind in `blueprint_model::reload_check` |
| Game data | YAML, live while the game runs (GD-00, #362), read by one YAML reader (GD-01, #363) | `blueprint_model::data_files` | the running game |
| Settings | A typed catalogue, one row per key, read and written through it | `setting_catalogue.rs` 472 (16 rows) | `settings.rs`, the settings screen |
| Panels | Typed widgets (`bevy_gooey` roles) with **no text form** | `bevy_gooey` `Role` has 76 variants | the editor's 68 spawn sites |

Two facts from that table carry the whole decision:

- **The schema already exists, and it already drives completion and diagnostics.** `blueprint_model::schema::for_kind` answers a JSON Schema for **24 file kinds** (`kits`, `vocab`, `library`, `policy`, `rules`, `compositions`, `standards`, `repetition`, `data_registry`, `rigs`, `map`, `notes`, `plan`, `board`, `relations`, `terrain`, `water_plan`, `water_field`, `volume`, and more). `files.rs`'s `schema_for` caches one per kind, and `files_lang::complete`/`hover`/`diagnose_in` read it. That is option 3's "a typed schema" — built, shipped, and proved by GD-12 (#374).
- **Markdown has no directive parser here.** `files_lang::markdown` offers highlights, completion from the dream board, and hover for a picture's caption; it has **no** notion of a directive block, and cannot complete one. The page renderer's parser, `pulldown-cmark` **0.13.6** (already a `blueprint_model` dependency, `design.rs`), has no generic-directive extension either: its `Options` carries strikethrough, tables, footnotes, task lists, math, GFM blocks and metadata blocks, and nothing for `:::` — the single `directive` hit in its source is an XML processing instruction in its scanner. Option 1 would add a parser this repository does not have, to build on a crate that does not offer one.

## 3. What the field has settled on

| Approach | What it is | Where it is used | Why it matters here |
|---|---|---|---|
| **Generic directives** (CommonMark proposal) | `:name[text]{key=val}` inline, `::name` leaf, `:::name` container; three colon counts by nesting | remark-directive (JS), `markdown-office/commonmark-generic-directives` (PHP, league/commonmark 2.x, 2026) | **Still a proposal, not in CommonMark.** No parser in Rust with first-class support; `pulldown-cmark` has no directive extension. Adopting it means writing the parser. |
| **MDX** | Markdown plus JSX components and ESM imports, compiled to a JavaScript module | Next.js, Astro docs, Docusaurus (opt-in) | Compiles to *code*, not a document. **Stricter than CommonMark** — a raw `<` in prose or a certain HTML block throws. Needs a JS runtime; unusable as a Bevy-side format. |
| **Nuxt MDC** | `::component{props}` with YAML props and `#slot` blocks | Nuxt Content only | The closest thing to the owner's first idea, and **vendor-locked to Nuxt**: migrating means rewriting the content. Its props are *already* YAML. |
| **LSP over a typed data file** | A schema declares the shape; the editor completes, hovers and diagnoses through the schema | VS Code + `yaml-language-server`, `taplo` (TOML), `vscode-json-languageserver` | The settled answer where **machines and people must both edit one file**. The schema is the one description; the language server is a reading of it. |
| **Prose in Markdown, data in YAML** | Two files, one presentation and one truth | Hugo, Zola, mdBook (`SUMMARY.md` + TOML front matter), Sphinx (`index.rst` + `conf.py`) | The common split in static-site tooling: prose is Markdown because prose is what Markdown is good at; structure and data are a data file. |

**The pattern in that table.** Every system that embeds widgets in Markdown either (a) owns a JavaScript compiler (MDX), (b) is a framework-specific plugin (MDC), or (c) is a **proposal with no reference implementation in the host language** (generic directives). Every system where *a machine and a person edit the same structured file over a long life* uses a **typed schema read by a language server**. The settled answer for "an agent must edit it correctly and a person must edit it comfortably" is not "richer Markdown" — it is "a schema, and a reader that guides both".

## 4. Measured cost per option

What each option changes, and how much of it. "Cost" is the code a pass must write and keep, not the code it reads once.

| | Markdown with directives | YAML + Markdown, with an LSP | Schema emits both | None |
|---|---|---|---|---|
| **Markdown directive parser** | **New.** `files_lang/markdown.rs` grows a directive grammar (nested containers, props, slots), a completion source for it, and a diagnostic for an unknown directive. There is no Rust crate to lean on; §3 measured that. Est. 600–900 lines plus tests. | none | none — the text form is *generated*, and read back by the existing `files_lang` YAML reader | none |
| **`files.rs`** | Gains a second Markdown mode (directives highlit, completed, diagnosed). ~150 lines, and `schema_for` becomes irrelevant to it. | **Unchanged.** The schema path already completes, hovers and diagnoses YAML. | **Unchanged**, and the same schema is what generates the text. | unchanged |
| **`setting_catalogue.rs`** | No change: a panel's rows are not settings. | A panel's YAML needs the same "one row per key, with words" the catalogue already does — either a second catalogue (a fact stated twice) or `Setting` generalised to a non-settings table. ~120 lines and a schema-derivation walk. | The catalogue's `Setting` becomes one *reader* of the schema rather than a hand-written table; 16 rows move from by-hand to derived. ~200 lines, a net deletion of hand-kept rows. | unchanged |
| **`design_view.rs`** | A rendered page gains widget blocks, so the renderer must decide which constructs become widgets. ~200 lines, and the page and the panel now share one renderer. | Unchanged: prose stays Markdown, the panel's data stays YAML, and the page is a *view* of the YAML. | The page's widgets are drawn from the same schema the panel draws from. ~250 lines, and one renderer for both. | unchanged |
| **An agent can diff it?** | Yes, but only for what the directive parser round-trips: anything falling back to a wrapper widget is a diff of an opaque blob. | **Yes, fully.** YAML is a line-oriented file, and `blueprint_model::yaml_surgery` already writes a leaf without reflowing the file. | Yes: the text projection is YAML (or Markdown generated from the schema), and the existing surgery writes it. | — |
| **A person can edit it?** | Yes, in the Files workspace with directive completion — *once the parser exists*. | Yes today: `code_editor` with schema completion already edits `data.yaml`. | Yes, and with the schema's own hover text beside each field. | — |
| **Two paths?** | **The risk.** A widget that maps and one that falls back are two rendering paths for one panel, and the fallback is where a diff stops meaning anything. | One path: the YAML is the truth, the panel draws it, the page renders it. | One path: the schema is the truth, both forms are projections of it. | One path, and no new format. |

The row that decides it is the last one. The repository's own rule is *one path per feature, no fallbacks* (`AGENTS.md`, "One path per feature"). **Option 1 is two paths for one panel by construction** — the note itself says "those that don't map directly, get a custom widget wrapper", and a wrapper is the fallback whose contents no diff can describe.

## 5. Recommendation

**Option 3, with option 2's file layout: the typed schema is the source of truth; a panel draws it; the text form is a YAML projection, edited in the Files workspace where the schema already completes, hovers and diagnoses it. Markdown stays prose.**

Concretely:

- **The schema is `blueprint_model::schema::for_kind`**, which already answers 24 kinds and already drives completion and diagnostics. A *panel's* schema is a `schemars` root like every other kind, so the panel's shape is declared once and nowhere else.
- **The panel draws the schema**, as the settings screen draws `setting_catalogue`'s rows — which becomes a reader of the schema rather than a hand-kept table, deleting a fact stated twice.
- **The text form is YAML**, written and read by the paths that exist: `files_lang`'s YAML completion from the schema, `reload_check`'s diagnostics, `yaml_surgery`'s leaf writes, and the reload contract that already hot-reloads a game's data.
- **Markdown stays what it is**: prose pages, the Design workspace's renderer, a picture as a caption and a licence. It is not the authoring format for a panel, because a panel is a *thing with fields*, and YAML is the format this project already has one reader for.

### What an agent gains, by form

| | Which form |
|---|---|
| **An agent can read it** | The **YAML**: one line-oriented file per panel's data, already what `blueprint_style` and the game's `data.yaml` are. An agent that can read `deadite_caverns`'s style reads a panel the same way. |
| **An agent can diff it** | The **YAML**, and only the lines it changed: `yaml_surgery` replaces a leaf's bytes and leaves the rest of the file untouched, so a proposal is a real diff rather than a rewritten blob. |
| **A person can edit it** | The **panel** for the shape (a typed control per field, the settings screen's own widgets), and the **YAML in the Files workspace** for the content, with the schema completing and diagnosing as they type — GD-11/GD-12, shipped. |

That is the answer to the note's "a mix of markdown and YAML, with a parser and LSP to guide users and agents": the mix is **Markdown for prose, YAML for a panel's data**, and the guidance is **already built** as the schema-driven reader rather than a new language server.

### Its costs, stated

1. **A schema per panel kind is a `schemars` root each**, and `schema::for_kind`'s 24 arms grow. Writing a `JsonSchema` derive is cheap; getting the *words* right (a field's label, its description) means `schemars` doc comments doing what `setting_catalogue`'s hand-written `label`/`description` do today. That is a real migration of 16 rows, and it is the one place this option costs more than it deletes.
2. **A panel's Markdown presentation is not Markdown-*authored*.** A person who wants to write a paragraph *inside* a panel must write it in a YAML string, where Markdown's syntax is not highlit. That is the trade this recommendation makes deliberately: a widget-free paragraph is prose, and prose belongs on a page.
3. **No directive syntax means no Nuxt-style `::callout`.** If the owner wants a widget *inside a page's prose* — not a panel — that is option 1's problem and remains unbuilt. This document recommends against building it, not that it is impossible.

### What is not answered here

- **The schema's wording is not designed.** Which fields a panel kind has, and what each is called, is the prototype's work, cited to this issue.
- **The `setting_catalogue` migration is not costed line by line.** The estimate above is a range from reading the file, not a measurement of a prototype that does not exist.

## 6. Why not the other three

- **Option 1 (Markdown with directives)** — two paths for one panel (§4), and no Rust parser to build on (§3). Its one real strength, a widget inside a page's prose, is not what the note is about: the note points at a panel.
- **Option 2 (YAML + Markdown, with an LSP)** — this is three quarters of the recommendation. It is *not* taken as stated only because it treats the YAML as the source of truth and the panel as a view, which makes the schema a convention rather than the declaration; option 3 is the same file layout with the schema in charge, so a panel cannot drift from its data.
- **Option 4 (none)** — refused because the gap is real and measured: a panel has no text form at all, an agent cannot propose a panel change as a diff, and the settings screen already proves a typed catalogue can draw a control per field. The cost of doing nothing is a panel that only a pointer can change.
