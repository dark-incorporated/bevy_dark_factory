//! **What the file editor knows about the languages a project holds** (GD-11, #373, GD-12, #374, and SG-04, #403).
//!
//! Engine-free: text in, byte ranges out. [`crate::files`] is what turns these answers into what
//! `bevy_gooey`'s `code_editor` draws (`Highlights`, `Diagnostics`, `FoldRanges`, `CodeCompletions`,
//! `CodeHover`), so a rule about a language is stated once, here, and a test reads it without a window.
//!
//! # One module per language, behind one trait
//!
//! A language is a type that implements [`Language`]: `highlights`, `diagnose`, `folds`, `complete`, `hover` and
//! `outline`. `yaml`, `ron`, `rhai` and `markdown` are one each, and [`REGISTRY`] is the one place that
//! maps a file's extension to its language. **The trait is the extension point**: an external LSP client would be
//! one more implementation and one more row. None is built, and nothing here starts a process. The free functions
//! in this module ([`highlights`], [`diagnose`], [`folds`] and the rest) are the editor's way in: each looks the
//! language up and asks it.
//!
//! # Colours and errors cannot disagree, because they are read from the same place
//!
//! - **YAML** is scanned line by line in the one shape the project's writer emits (block style, a key a
//!   line, an item a line: `blueprint_model::yaml`'s "written style"), and *its errors are the
//!   loader's*: a file of a known kind is refused by [`blueprint_model::reload_check::check`], the
//!   reader the game loads it with, and any other file by the event walk
//!   ([`blueprint_model::yaml::to_json`]) that refuses a duplicate key, an anchor, a tab and a second
//!   document. A key colour is the key the walk would name.
//! - **A reference is a link.** A field the kind's JSON Schema marks `x-dark-factory-ref` (a placement's
//!   `descriptor` names a library piece) is drawn as `SyntaxRole::Reference`; the schema is the one
//!   [`blueprint_model::schema`] generates from the Rust type, so nothing here knows which fields
//!   name another file's value.
//! - **Completion and hover are the schema's**: the field names a mapping may hold, the values an enum
//!   may take, the ids a reference may point at, and each field's doc comment.
//! - **Markdown's errors are the design tree's.** A link or a picture whose path is not in the project is read by
//!   [`blueprint_model::design::cites`], the reader behind `blueprint_state.design`, so a broken picture in the
//!   file editor and in that list are one problem. Its completion and hover are the place's dream board
//!   ([`blueprint_model::board`]).
//!
//! # Not a parser, and why that is a decision and not a shortcut
//!
//! The colours of `.ron` files and Rhai come from a small tokenizer each (no new dependency: the
//! owner's call of 2026-11-12). Their *errors* are not guessed by it: a `.ron` file's (no project file is
//! RON any more since GD-03, #365, so it is only a game's own) and Rhai's are only what a scan can be sure of (a bracket that
//! never closes, a string that never ends: the engine's own compile is a crate the editor does not link).
//! Markdown's colours are `bevy_gooey`'s own (`CodeMarkdown`, #525), and its errors are the paths the design reader names.

use std::ops::{Range, RangeInclusive};
use std::path::Path;

use bevy_gooey::{CodeSeverity, SyntaxRole};
use serde_json::Value;

mod code;
mod markdown;
mod rhai;
mod ron;
mod rust;
mod yaml;

pub use blueprint_model::design::Heading;
pub use yaml::{CUE_FIELD, CUE_KIND, Reference, Schema, Seg, reference_at, references};

/// **The languages the editor colours**, by the file's extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    Yaml,
    Ron,
    Rhai,
    Markdown,
    /// A game's own Rust source, when its crate sits in the project: the editor knows its comment and nothing else.
    Rust,
    /// Anything else the project holds as text: no colour, no folds, no findings.
    Plain,
}

impl Lang {
    /// The language of a path: the registry's row for its extension.
    pub fn of(path: &str) -> Lang {
        let ext = path.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
        let Some(ext) = ext else { return Lang::Plain };
        REGISTRY.iter().find(|l| l.extensions().contains(&ext.as_str())).map_or(Lang::Plain, |l| l.lang())
    }

    /// The implementation that answers for this language.
    pub fn language(self) -> &'static dyn Language {
        REGISTRY.iter().copied().find(|l| l.lang() == self).unwrap_or(&Plain)
    }
}

/// **The one registry from an extension to a language.** A file of an extension no row names is [`Lang::Plain`].
pub static REGISTRY: [&dyn Language; 5] = [&yaml::Yaml, &ron::Ron, &rhai::Rhai, &markdown::Markdown, &rust::Rust];

/// **How a language writes a comment**, which `Cmd+/` puts on lines and takes off them (#612, [`toggle_comment`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentSyntax {
    /// A mark that comments the rest of its line: `#` in YAML, `//` in `.ron`, Rhai and Rust.
    Line(&'static str),
    /// A pair that encloses what it comments: `<!--` and `-->` in Markdown, which has no line comment.
    Wrap(&'static str, &'static str),
}

impl CommentSyntax {
    /// Whether `body` (a line with its indent taken off) is already commented in this syntax.
    fn marks(self, body: &str) -> bool {
        match self {
            CommentSyntax::Line(mark) => body.starts_with(mark),
            CommentSyntax::Wrap(open, close) => {
                let body = body.trim_end_matches([' ', '\t']);
                body.len() >= open.len() + close.len() && body.starts_with(open) && body.ends_with(close)
            }
        }
    }
}

/// What a language is told about the project around the file it reads: the folder the paths are relative to, and
/// the two lists a completion draws on. Nothing here is read until a language asks.
pub struct Env<'a> {
    /// The project's root, when a project is open: what a path in a file is looked for under.
    pub root: Option<&'a Path>,
    /// The ids a reference field may name, by kind (`library`, `compositions`).
    pub refs: &'a dyn Fn(&str) -> Vec<String>,
    /// The project's files, relative to the root and `/`-separated (the file tree's listing).
    pub files: &'a dyn Fn() -> Vec<String>,
}

fn no_refs(_: &str) -> Vec<String> {
    Vec::new()
}

fn no_files() -> Vec<String> {
    Vec::new()
}

impl Env<'static> {
    /// A file with no project around it: nothing is looked for, nothing is offered.
    pub fn alone() -> Env<'static> {
        Env { root: None, refs: &no_refs, files: &no_files }
    }
}

/// **A language the file editor reads.** Every method has an answer for "nothing to say", so a language implements
/// what it has: `Plain` implements none.
pub trait Language: Sync {
    /// The key the editor knows this language by.
    fn lang(&self) -> Lang;

    /// The lower-case extensions (without the dot) of the files this language reads.
    fn extensions(&self) -> &'static [&'static str];

    /// The spans the text is coloured in, by byte range. `schema` is the file's kind's JSON Schema when it has one.
    fn highlights(&self, _text: &str, _schema: Option<&Value>) -> Vec<(Range<usize>, SyntaxRole)> {
        Vec::new()
    }

    /// What is wrong with the file at `path` (relative to the project, `/`-separated).
    fn diagnose(&self, _path: &str, _text: &str, _env: &Env<'_>) -> Vec<Finding> {
        Vec::new()
    }

    /// Where the text can fold, as `start..end` over lines (the header stays drawn, `start + 1..end` hides).
    fn folds(&self, _text: &str) -> Vec<Range<usize>> {
        Vec::new()
    }

    /// What the editor may offer at the caret (a byte offset on a char boundary).
    fn complete(&self, _path: &str, _text: &str, _offset: usize, _schema: Option<&Value>, _env: &Env<'_>) -> Completion {
        Completion::default()
    }

    /// What to say about the text at `offset`, and the bytes it is about.
    fn hover(&self, _path: &str, _text: &str, _offset: usize, _schema: Option<&Value>, _env: &Env<'_>) -> Option<(Range<usize>, String)> {
        None
    }

    /// The headings of a page, in order. A language without headings has none.
    fn outline(&self, _text: &str) -> Vec<Heading> {
        Vec::new()
    }

    /// How the language writes a comment, for `Cmd+/`. A language with none leaves the key doing nothing.
    fn comment(&self) -> Option<CommentSyntax> {
        None
    }
}

/// **Any other text the project holds**: no colour, no folds, no findings.
struct Plain;

impl Language for Plain {
    fn lang(&self) -> Lang {
        Lang::Plain
    }
    fn extensions(&self) -> &'static [&'static str] {
        &[]
    }
}

/// **Whether a file is one the editor shows read-only**: the files in `trust::COMMAND_FILES` (`agent`
/// and `play`, which name a program to start; the guide, which tells an agent what to do) and the YAML
/// spelling of the first two. Changing them is a decision with its own page and its own trust check, so
/// an edit here must not be a way round it.
pub fn is_command_file(rel: &str) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    blueprint_model::trust::COMMAND_FILES.contains(&name)
        || rel == blueprint_model::agent_guide::PROJECT_PATH
}

/// The sentence a read-only file's tab says, so nobody wonders why the keys do nothing.
pub const READ_ONLY_NOTE: &str = "Read-only here: this file is changed on its own page, so an edit cannot bypass its trust check";

// ── Lines ────────────────────────────────────────────────────────────────────

/// **A text's lines**, as byte ranges: a line's range excludes its `\n` and its `\r`.
pub struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    pub fn new(text: &'a str) -> Lines<'a> {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(at, _)| at + 1));
        Lines { text, starts }
    }

    /// How many lines there are: a text that ends in `\n` has an empty last one, as the editor counts.
    pub fn count(&self) -> usize {
        self.starts.len()
    }

    /// The bytes of line `n` (0-based), without its line break.
    pub fn range(&self, n: usize) -> Range<usize> {
        let start = self.starts.get(n).copied().unwrap_or(self.text.len());
        let end = self.starts.get(n + 1).map_or(self.text.len(), |next| next - 1);
        let end = end.max(start);
        if self.text[start..end].ends_with('\r') { start..end - 1 } else { start..end }
    }

    /// The line a byte offset is on.
    pub fn line_of(&self, offset: usize) -> usize {
        self.starts.partition_point(|&s| s <= offset).saturating_sub(1)
    }

    pub fn text(&self, n: usize) -> &'a str {
        &self.text[self.range(n)]
    }
}

/// **The largest text the editor colours and folds**, in bytes. Past this a file is data a program wrote: it is shown
/// plain, because the colours are recomputed on every revision.
pub const COLOUR_LIMIT_BYTES: usize = 1024 * 1024;

// ── The editor's way in ──────────────────────────────────────────────────────

/// **The spans a file's text is coloured in**, by byte range. `schema` is the file's kind's JSON Schema
/// when it has one: YAML values it marks as references are links.
pub fn highlights(lang: Lang, text: &str, schema: Option<&Value>) -> Vec<(Range<usize>, SyntaxRole)> {
    if text.len() > COLOUR_LIMIT_BYTES {
        return Vec::new();
    }
    lang.language().highlights(text, schema)
}

/// **Where a file can fold**, as `start..end` over lines (the header stays drawn, `start + 1..end` hides):
/// every YAML mapping and sequence, every Rhai or `.ron` block, every Markdown section and code fence.
pub fn folds(lang: Lang, text: &str) -> Vec<Range<usize>> {
    if text.len() > COLOUR_LIMIT_BYTES {
        return Vec::new();
    }
    lang.language().folds(text)
}

/// **The headings of a page**, in order: a Markdown page's, and nothing for a language without any.
pub fn outline(lang: Lang, text: &str) -> Vec<Heading> {
    if text.len() > COLOUR_LIMIT_BYTES {
        return Vec::new();
    }
    lang.language().outline(text)
}

/// **What `Cmd+/` does to lines `lines` of `text`** (#612): the edits, as byte ranges of `text` and what replaces each, that comment
/// the lines in `lang`'s own syntax, or that take the comment off when every line with words on it already carries one. `None` for a
/// language with no comment syntax ([`Lang::Plain`]).
///
/// The rule is the one VS Code keeps: a blank line is left as it is; the mark goes at the shallowest indent of the lines, so a block
/// keeps its shape and comes back to it; and the mark is followed by one space, which is taken off with it. A range past the text's
/// end stops at its last line.
pub fn toggle_comment(lang: Lang, text: &str, lines: RangeInclusive<usize>) -> Option<Vec<(Range<usize>, String)>> {
    let syntax = lang.language().comment()?;
    let all = Lines::new(text);
    let last = (*lines.end()).min(all.count().saturating_sub(1));
    let first = (*lines.start()).min(last);
    let indent = |line: &str| line.len() - line.trim_start_matches([' ', '\t']).len();
    let worded: Vec<(usize, &str)> = (first..=last).map(|n| (all.range(n).start, all.text(n))).filter(|(_, line)| !line.trim().is_empty()).collect();
    let commented = !worded.is_empty() && worded.iter().all(|(_, line)| syntax.marks(&line[indent(line)..]));
    let mut edits = Vec::new();
    if commented {
        for &(start, line) in &worded {
            let lead = indent(line);
            let body = &line[lead..];
            let (open, close) = match syntax {
                CommentSyntax::Line(mark) => (mark, None),
                CommentSyntax::Wrap(open, close) => (open, Some(close)),
            };
            let cut = open.len() + usize::from(body[open.len()..].starts_with(' '));
            edits.push((start + lead..start + lead + cut, String::new()));
            if let Some(close) = close {
                let end = line.trim_end_matches([' ', '\t']).len();
                let from = end - close.len();
                let from = if from > lead + cut && line[..from].ends_with(' ') { from - 1 } else { from };
                edits.push((start + from..start + end, String::new()));
            }
        }
    } else {
        let at = worded.iter().map(|(_, line)| indent(line)).min().unwrap_or(0);
        for &(start, line) in &worded {
            match syntax {
                CommentSyntax::Line(mark) => edits.push((start + at..start + at, format!("{mark} "))),
                CommentSyntax::Wrap(open, close) => {
                    let end = start + line.trim_end_matches([' ', '\t']).len();
                    edits.push((start + at..start + at, format!("{open} ")));
                    edits.push((end..end, format!(" {close}")));
                }
            }
        }
    }
    Some(edits)
}

/// **One thing wrong in a file**, by byte range.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub range: Range<usize>,
    pub severity: CodeSeverity,
    pub message: String,
}

/// **A finding on line `line` (1-based)**: the line's text, or the word the message names in backticks
/// when the line holds it (an unknown key points at the key).
fn finding_on(lines: &Lines<'_>, text: &str, line: usize, message: String, severity: CodeSeverity) -> Finding {
    let n = line.saturating_sub(1).min(lines.count().saturating_sub(1));
    let range = lines.range(n);
    let line_text = &text[range.clone()];
    let indent = line_text.len() - line_text.trim_start().len();
    let named = message.split('`').nth(1).filter(|w| !w.is_empty() && !w.contains(' ')).and_then(|w| line_text.find(w).map(|at| at..at + w.len()));
    let range = match named {
        Some(w) => range.start + w.start..range.start + w.end,
        None => range.start + indent..range.end.max(range.start + indent),
    };
    // An empty line (a missing field at the end of a file) still needs a range the editor can draw.
    let range = if range.is_empty() { range.start..(range.start + 1).min(text.len()).max(range.start) } else { range };
    Finding { range, severity, message }
}

/// **What is wrong with a file**, read the way the game reads it (see the module note): a project file by
/// its kind's own reader, any other YAML by the event walk, `.ron` and Rhai by a bracket scan.
///
/// `path` is the file's path under the project, with `/`. With no project around it ([`Env::alone`]): a Markdown
/// page's paths are not looked for. [`diagnose_in`] is the call that has one.
pub fn diagnose(path: &str, text: &str) -> Vec<Finding> {
    diagnose_in(&Env::alone(), path, text)
}

/// **[`diagnose`] with the project around the file**, which is what a Markdown page's links and pictures are
/// looked for in.
pub fn diagnose_in(env: &Env<'_>, path: &str, text: &str) -> Vec<Finding> {
    Lang::of(path).language().diagnose(path, text, env)
}

// ── Completion and hover ─────────────────────────────────────────────────────

/// **What the editor may offer at the caret**: the start of the word being completed, and the rows.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Completion {
    pub anchor: usize,
    /// `(label, detail, text to insert instead of the label)`.
    pub items: Vec<(String, String, Option<String>)>,
}

fn word_before(text: &str, offset: usize) -> usize {
    let head = &text[..offset];
    head.rfind(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '/' | '@'))).map_or(0, |at| at + head[at..].chars().next().map_or(1, char::len_utf8))
}

/// **Complete the file at `path` at `offset`**: YAML from the kind's schema (at a key, the fields the mapping may
/// hold; at a value, the enum's values, a boolean's two, or the ids a reference may point at), Markdown from the
/// place's dream board and then the project's files. Nothing for a file with nothing to offer.
pub fn complete(path: &str, text: &str, offset: usize, schema: Option<&Value>, env: &Env<'_>) -> Completion {
    if !text.is_char_boundary(offset) {
        return Completion::default();
    }
    Lang::of(path).language().complete(path, text, offset, schema, env)
}

/// **What to say about the text at `offset`**: a YAML key's doc comment (and that it is a reference, to what), a
/// Markdown picture's caption and licence from the board. The range is the bytes it is about.
pub fn hover(path: &str, text: &str, offset: usize, schema: Option<&Value>, env: &Env<'_>) -> Option<(Range<usize>, String)> {
    if !text.is_char_boundary(offset) {
        return None;
    }
    Lang::of(path).language().hover(path, text, offset, schema, env)
}
