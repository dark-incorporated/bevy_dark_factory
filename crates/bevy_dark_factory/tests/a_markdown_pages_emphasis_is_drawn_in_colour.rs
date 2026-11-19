//! **A Markdown page's emphasis is drawn in the editor's own colours** (#525,
//! `feedback-1795094391027`: *"...look at how they do markdown rendering and text editing, as well as syntax
//! highlighting... and we probably need an LSP at some point"*).
//!
//! # What the editor did before
//!
//! The Markdown tokenizer coloured a heading (`Keyword`), a fence (`Operator`), a list marker (`Operator`), inline
//! `code` (`String`), a `[link](path)` (`Reference`) and a quote (`Comment`). It coloured **neither `*em*` nor `**strong**`**
//! — measured: `"*The three things.*"` produced `[]` highlight spans and `"a **bold** word"` produced `[]`, so the two
//! emphases a page is mostly made of were plain text, indistinguishable from a paragraph.
//!
//! # What this measures
//!
//! A page carrying both emphases draws them as coloured runs, each in the one role its kind takes, and drawn — not
//! merely stored: the spans the tokenizer returns and the `TextSpan` runs the editor spawns are read together, so a
//! highlight that never reaches the screen fails here.

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_dark_factory::command_run::Chords;
use bevy_dark_factory::commands;
use bevy_dark_factory::files_lang::{self, Lang};
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::Live;
use bevy_gooey::{CodeEditor, CodeEditorValue, CodePos, Role, SyntaxRole};

use crate::fixtures;

const SETTLE: usize = 14;

/// The page the two emphases sit on: a star emphasis and a star-strong, one line each, so a hover or a fold does not
/// enter into it.
const PAGE: &str = "# A page\n\n*Emphasised words.*\n\nA **strong** phrase.\n";

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn page(tag: &str) -> (App, Entity) {
    let root = fixtures::Fixture::new(&format!("md_emphasis_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (1.5, 1.5))
        .build("m");
    let rel = "design/style.md";
    let full = root.join(rel);
    std::fs::create_dir_all(full.parent().unwrap_or(&root)).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(&full, PAGE).unwrap_or_else(|e| panic!("{e}"));
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    harness::resize_surface(&mut app, 1400, 900).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle(&mut app);
    bevy_dark_factory::files::open_file(app.world_mut(), rel).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    let editor = app.world_mut().query_filtered::<Entity, With<CodeEditor>>().single(app.world_mut()).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().resource_mut::<InputFocus>().set(editor, FocusCause::Navigated);
    settle(&mut app);
    (app, editor)
}

/// **The tokenizer gives `*em*` and `**strong**` a colour each, and the two are different.** Before the change both were
/// `None`: neither produced a span.
#[test]
fn the_tokenizer_colours_both_emphases_and_not_alike() {
    let spans = files_lang::highlights(Lang::Markdown, PAGE, None);
    let role_of = |needle: &str| {
        let at = PAGE.find(needle).unwrap_or_else(|| panic!("{needle} is on the page"));
        spans.iter().find(|(r, _)| r.start == at).map(|(_, role)| *role)
    };
    let em = role_of("*Emphasised words.*");
    let strong = role_of("**strong**");
    assert!(em.is_some(), "`*Emphasised words.*` is drawn as a coloured run, not plain text; spans: {spans:?}");
    assert!(strong.is_some(), "`**strong**` is drawn as a coloured run, not plain text; spans: {spans:?}");
    assert_ne!(em, strong, "an emphasis and a strong emphasis are told apart by colour, not drawn the same");
    for role in [em, strong].into_iter().flatten() {
        assert!(
            !matches!(role, SyntaxRole::Operator | SyntaxRole::Comment | SyntaxRole::Error),
            "{role:?} is a mark's own colour, not an emphasis run's"
        );
    }
}

/// **The colours reach the screen**: the editor's own `TextSpan` runs carry the roles the tokenizer gave, so what is
/// stored and what is drawn are one thing.
#[test]
fn the_emphasis_is_drawn_as_a_coloured_run() {
    let (mut app, editor) = page("drawn");
    // Put the caret away from the page's first line so no caret-reveal preedit rewrites the rows.
    let Some(mut widget) = app.world_mut().get_mut::<CodeEditor>(editor) else { panic!("the editor is open") };
    widget.select(CodePos::new(0, 0), CodePos::new(0, 0));
    settle(&mut app);
    let world = app.world_mut();
    let mut q = world.query::<(&TextSpan, &Role)>();
    let drawn: Vec<(String, Role)> = q
        .iter(world)
        .filter(|(t, _)| !t.0.trim().is_empty())
        .map(|(t, role)| (t.0.clone(), *role))
        .collect();
    let lit = |needle: &str| {
        drawn.iter().any(|(run, role)| run.contains(needle) && matches!(role, Role::CodeSyntax(_)))
    };
    assert!(lit("Emphasised"), "the emphasis's run is drawn with a syntax role; runs: {drawn:?}");
    assert!(lit("strong"), "the strong emphasis's run is drawn with a syntax role; runs: {drawn:?}");
    // And the two are not drawn in one role, so a reader can tell them apart.
    let role_of = |needle: &str| {
        drawn.iter().find(|(run, _)| run.contains(needle)).map(|(_, role)| *role)
    };
    assert_ne!(role_of("Emphasised"), role_of("strong"), "emphasis and strong are two colours: {drawn:?}");
}

/// **A star that is not an emphasis stays plain**: a bullet's `*`, a lone star, and a star inside a code span are not
/// drawn as an emphasis, so the colours do not leak onto a page that did not ask for one.
#[test]
fn a_star_that_is_not_an_emphasis_stays_plain() {
    for (label, text) in [
        ("a star bullet", "* one\n* two\n"),
        ("a lone star", "just a * in a line\n"),
        ("a star in a code span", "`a * b`\n"),
        ("an unclosed emphasis", "an *opening with no close\n"),
    ] {
        let spans = files_lang::highlights(Lang::Markdown, text, None);
        let emphases = spans.iter().filter(|(_, role)| matches!(role, SyntaxRole::Function | SyntaxRole::Type)).count();
        assert_eq!(emphases, 0, "{label} draws no emphasis run; spans: {spans:?}");
    }
    // The value the editor shows is the page's own text: a colour is a drawing, never an edit.
    let (mut app, _editor) = page("plain");
    let world = app.world_mut();
    let shown = world.query::<&CodeEditorValue>().single(world).map(|v| v.text().to_owned()).unwrap_or_default();
    assert_eq!(shown, PAGE, "colouring the emphases changed no character of the page");
}
