//! **Rust**: a game's own source, when its crate sits in the project. The editor knows how it writes a comment, so `Cmd+/`
//! works on it (#612), and nothing else: it is drawn plain, with no folds and no findings, as it was before it had a row.

use super::{CommentSyntax, Lang, Language};

/// The language of `.rs` files.
pub struct Rust;

impl Language for Rust {
    fn lang(&self) -> Lang {
        Lang::Rust
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn comment(&self) -> Option<CommentSyntax> {
        Some(CommentSyntax::Line("//"))
    }
}
