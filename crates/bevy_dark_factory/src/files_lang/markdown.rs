//! **Markdown**: the pages of the design tree and the project's notes. Coloured by a small tokenizer; its errors are
//! the design reader's; completion and hover are the place's dream board.

use std::ops::Range;
use std::path::Path;

use bevy_gooey::{CodeSeverity, SyntaxRole};
use blueprint_model::board::{self, Board, Entry};
use blueprint_model::design;
use serde_json::Value;

use super::{Completion, Env, Finding, Heading, Lang, Language, Lines};

/// **The most project files a Markdown completion offers after the board's pictures**, so the list stays one a person
/// reads and a big project does not make a menu of thousands.
const COMPLETION_FILES_MAX: usize = 200;

/// The language of `.md` and `.markdown` files.
pub struct Markdown;

impl Language for Markdown {
    fn lang(&self) -> Lang {
        Lang::Markdown
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["md", "markdown"]
    }

    fn highlights(&self, text: &str, _schema: Option<&Value>) -> Vec<(Range<usize>, SyntaxRole)> {
        markdown_highlights(text)
    }

    fn folds(&self, text: &str) -> Vec<Range<usize>> {
        markdown_folds(text)
    }

    /// **A link or a picture whose path is not in the project is an error on the path.** The reader is
    /// [`design::cites`], the one behind `blueprint_state.design`, so the editor and that list are one problem.
    fn diagnose(&self, path: &str, text: &str, env: &Env<'_>) -> Vec<Finding> {
        let Some(root) = env.root else { return Vec::new() };
        let mut out = Vec::new();
        for cite in design::cites(text, path) {
            let Some(rel) = &cite.file else { continue };
            let there = if cite.image { root.join(rel).is_file() } else { root.join(rel).exists() };
            if there {
                continue;
            }
            let what = if cite.image { "picture" } else { "page" };
            // **No text of the page**: this finding reaches an agent through the open file's diagnostics, and a destination
            // may be any prose. Its range sits on the destination, which is where the person looks.
            let message = format!("this {what} is not there. Put it there, or name one that is.");
            out.push(Finding { range: cite.range, severity: CodeSeverity::Error, message });
        }
        out
    }

    /// **After `](` the place's board first, then the project's files; after `![` the board's pictures with what to
    /// take from each**, so a person writing a page cites what is on the board.
    fn complete(&self, path: &str, text: &str, offset: usize, _schema: Option<&Value>, env: &Env<'_>) -> Completion {
        let Some(root) = env.root else { return Completion::default() };
        let Some(at) = caret_in(text, offset) else { return Completion::default() };
        let from = folder_of(path);
        let boards = chain_boards(root, path);
        match at {
            Caret::Alt { anchor } => {
                let items = boards
                    .iter()
                    .map(|e| {
                        let rel = relative_to(from, &e.path);
                        (e.caption.clone(), rel.clone(), Some(format!("{}]({rel})", e.caption)))
                    })
                    .collect();
                Completion { anchor, items }
            }
            Caret::Destination { anchor } => {
                let typed = text[anchor..offset].to_lowercase();
                let mut items: Vec<(String, String, Option<String>)> = Vec::new();
                for e in &boards {
                    let rel = relative_to(from, &e.path);
                    items.push((rel, board_detail(e), None));
                }
                let offered: Vec<String> = items.iter().map(|(label, _, _)| label.clone()).collect();
                let mut files = (env.files)();
                files.sort();
                for file in files {
                    if file == path || file.rsplit('/').next().is_some_and(|name| name.starts_with('.')) {
                        continue;
                    }
                    let rel = relative_to(from, &file);
                    if offered.contains(&rel) || !rel.to_lowercase().contains(&typed) {
                        continue;
                    }
                    if items.len() >= boards.len() + COMPLETION_FILES_MAX {
                        break;
                    }
                    items.push((rel, "file".to_owned(), None));
                }
                Completion { anchor, items }
            }
        }
    }

    /// **A picture's path says what the board says of it**: its caption and licence.
    fn hover(&self, path: &str, text: &str, offset: usize, _schema: Option<&Value>, env: &Env<'_>) -> Option<(Range<usize>, String)> {
        let root = env.root?;
        let cite = design::cites(text, path).into_iter().find(|c| c.range.start <= offset && offset <= c.range.end)?;
        let rel = cite.file?;
        let entry = chain_boards(root, path)
            .into_iter()
            .chain(board::places_with_a_board(root).iter().filter_map(|place| Board::of_place(root, place).ok()).flat_map(|b| b.pictures))
            .find(|e| e.path == rel)?;
        Some((cite.range, say(&entry)))
    }

    /// **The headings of the page**, in order (see [`design::headings`]).
    fn outline(&self, text: &str) -> Vec<Heading> {
        design::headings(text)
    }
}

/// Where the caret stands in a link or a picture being written.
enum Caret {
    /// After `![`: the alt text, which a board picture's caption fills.
    Alt { anchor: usize },
    /// After `](`: the destination.
    Destination { anchor: usize },
}

/// **What the caret is inside**, judged on its own line: a destination that is open (`](` and no `)` yet), or alt text
/// that is open (`![` and no `]` yet).
fn caret_in(text: &str, offset: usize) -> Option<Caret> {
    let start = text[..offset].rfind('\n').map_or(0, |at| at + 1);
    let before = &text[start..offset];
    if let Some(open) = before.rfind("](")
        && !before[open..].contains(')')
    {
        let skip = usize::from(before[open + 2..].starts_with('<'));
        return Some(Caret::Destination { anchor: start + open + 2 + skip });
    }
    let open = before.rfind("![")?;
    (!before[open..].contains(']')).then_some(Caret::Alt { anchor: start + open + 2 })
}

/// **The folder of a project-relative file**, `""` for one at the root.
fn folder_of(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

/// **A project-relative path as `from`'s files cite it**: relative to the folder `from`.
fn relative_to(from: &str, target: &str) -> String {
    let from: Vec<&str> = from.split('/').filter(|p| !p.is_empty()).collect();
    let target: Vec<&str> = target.split('/').collect();
    let common = from.iter().zip(&target).take_while(|(a, b)| a == b).count().min(target.len().saturating_sub(1));
    let mut parts: Vec<&str> = vec![".."; from.len() - common];
    parts.extend(&target[common..]);
    parts.join("/")
}

/// **The places a file is under that have a board, the nearest first**: the place of the file, then the broader ones.
fn places_of(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut dir = folder_of(path);
    loop {
        if board::is_place(dir) {
            out.push(dir.to_owned());
        }
        match dir.rsplit_once('/') {
            Some((parent, _)) => dir = parent,
            None => break,
        }
    }
    out
}

/// **The pictures on the boards of the place a page is in**, the nearest place first, in each board's order.
fn chain_boards(root: &Path, path: &str) -> Vec<Entry> {
    places_of(path).iter().filter_map(|place| Board::of_place(root, place).ok()).flat_map(|b| b.pictures).collect()
}

fn board_detail(e: &Entry) -> String {
    if e.restricted { format!("{} (restricted: never ships)", e.caption) } else { e.caption.clone() }
}

/// **What a hover says of a board picture**: what to take from it, then where it came from and what it may be used under.
fn say(e: &Entry) -> String {
    let mut out = e.caption.clone();
    out.push_str(&format!("\n\nLicence: {}", e.licence.as_deref().unwrap_or("not recorded")));
    if let Some(creator) = &e.creator {
        out.push_str(&format!("\nCreator: {creator}"));
    }
    if let Some(source) = &e.source {
        out.push_str(&format!("\nSource: {source}"));
    }
    if e.restricted {
        out.push_str("\nRestricted: looked at, never shipped");
    }
    out
}

// ── Markdown ─────────────────────────────────────────────────────────────────

fn fence_of(line: &str) -> Option<&str> {
    let t = line.trim_start();
    ["```", "~~~"].into_iter().find(|f| t.starts_with(f))
}

fn heading_level(line: &str) -> Option<usize> {
    let hashes = line.bytes().take_while(|b| *b == b'#').count();
    (1..=6).contains(&hashes).then_some(()).and_then(|()| (line[hashes..].starts_with(' ')).then_some(hashes))
}

/// **An emphasis span at the start of `rest`**: its whole length in bytes and the role it takes, or
/// `None` when the run does not open one.
///
/// `**strong**` (two marks) takes [`SyntaxRole::Type`] and `*em*` (one) takes
/// [`SyntaxRole::Function`] — two hues the markdown editor did not use for anything else, and both
/// distinct from the `String` a code span takes and the `Reference` a link does, so no two kinds of
/// markdown text are drawn alike. `__strong__` and `_em_` are the underscore spellings, matched the
/// same way. A run with no close in it is plain text, and a mark inside a word (`a*b`) is not an
/// opener, which is CommonMark's rule for the underscore forms and a fair one for the star here.
///
/// The emphasis is a *token*, not a parse: like every other colour in this module it names what a mark
/// looks like, and a reader who wants what a page means reads `blueprint_model::design` instead.
fn emphasis_at(rest: &str) -> Option<(usize, SyntaxRole)> {
    let (mark, role) = match rest.as_bytes().first()? {
        b'*' => ('*', SyntaxRole::Function),
        b'_' => ('_', SyntaxRole::Function),
        _ => return None,
    };
    // Two of the same mark is strong, and it takes the other hue.
    let double = rest.as_bytes().get(1) == Some(&(mark as u8));
    let (open, role) = if double { (2, SyntaxRole::Type) } else { (1, role) };
    // A mark at the start of a word: the closer has to be a mark too, so the run between them is not
    // empty and the opener is not the last thing on the line.
    let body = &rest[open..];
    let close = if double {
        body.find(&mark.to_string().repeat(2))?
    } else {
        body.find(mark)?
    };
    // An empty run (`**` alone, `__`) is not an emphasis.
    if close == 0 {
        return None;
    }
    Some((open + close + open, role))
}

fn markdown_highlights(text: &str) -> Vec<(Range<usize>, SyntaxRole)> {
    let lines = Lines::new(text);
    let mut spans = Vec::new();
    let mut fenced = false;
    for n in 0..lines.count() {
        let range = lines.range(n);
        let line = &text[range.clone()];
        if fence_of(line).is_some() {
            spans.push((range, SyntaxRole::Operator));
            fenced = !fenced;
            continue;
        }
        if fenced {
            if !line.is_empty() {
                spans.push((range, SyntaxRole::String));
            }
            continue;
        }
        if heading_level(line).is_some() {
            spans.push((range, SyntaxRole::Keyword));
            continue;
        }
        if line.starts_with('>') {
            spans.push((range, SyntaxRole::Comment));
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        let body = &line[indent..];
        let marker = if body.starts_with("- ") || body.starts_with("* ") || body.starts_with("+ ") {
            1
        } else {
            let digits = body.bytes().take_while(u8::is_ascii_digit).count();
            if digits > 0 && body[digits..].starts_with(". ") { digits + 1 } else { 0 }
        };
        if marker > 0 {
            spans.push((range.start + indent..range.start + indent + marker, SyntaxRole::Operator));
        }
        // Inline: `code` is a string, `[text](target)` a link, `**strong**` and `*em*` /
        // `__strong__` and `_em_` are the two emphases. A link or a code span is taken whole first,
        // so a `*` inside one is that span's text and not an emphasis mark.
        // Past the indent and the list marker: a `*` bullet is a mark, not an emphasis, and nothing
        // in the indentation is drawn.
        let mut at = indent + marker;
        while at < line.len() {
            let rest = &line[at..];
            if let Some(inner) = rest.strip_prefix('`') {
                match inner.find('`') {
                    Some(end) => {
                        spans.push((range.start + at..range.start + at + end + 2, SyntaxRole::String));
                        at += end + 2;
                    }
                    None => at += 1,
                }
            } else if rest.starts_with('[') {
                let link = rest.find("](").and_then(|mid| rest[mid..].find(')').map(|close| mid + close + 1));
                match link {
                    Some(end) => {
                        spans.push((range.start + at..range.start + at + end, SyntaxRole::Reference));
                        at += end;
                    }
                    None => at += 1,
                }
            } else if let Some((len, role)) = emphasis_at(rest) {
                spans.push((range.start + at..range.start + at + len, role));
                at += len;
            } else {
                at += rest.chars().next().map_or(1, char::len_utf8);
            }
        }
    }
    spans
}

fn markdown_folds(text: &str) -> Vec<Range<usize>> {
    let lines = Lines::new(text);
    let count = lines.count();
    let last_content = (0..count).rev().find(|&n| !lines.text(n).trim().is_empty()).map_or(0, |n| n + 1);
    let mut out = Vec::new();
    // **A section runs to the next heading as shallow as its own** (or to the last line with anything on it), and the headings are
    // the outline's: one reader, so a setext heading folds as it lists.
    let headings = design::headings(text);
    for (at, heading) in headings.iter().enumerate() {
        let end = headings[at + 1..].iter().find(|next| next.level <= heading.level).map_or(last_content, |next| next.line);
        out.push(heading.line..end.max(heading.line + 1));
    }
    let mut fence_start: Option<usize> = None;
    for n in 0..count {
        if fence_of(lines.text(n)).is_some() {
            match fence_start.take() {
                Some(start) => out.push(start..n),
                None => fence_start = Some(n),
            }
        }
    }
    out.sort_by_key(|r| (r.start, std::cmp::Reverse(r.end)));
    out
}
