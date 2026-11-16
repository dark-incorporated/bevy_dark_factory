//! **Prose is drawn in a proportional face and everything else stays FiraMono** (#469, `feedback-1794024825941`: *"it reads like a terminal"*).
//!
//! Decided as option 2 on 2026-11-15 (*recommend, record, proceed*): the sentences of the launcher's What's new page and of its Learn page draw in
//! Fira Sans (the same family and the same SIL OFL 1.1 licence as FiraMono, the licence text beside the face), and the headings, the buttons, the keycaps,
//! the Inspector, the tables and the terminal keep FiraMono. These tests read what the layout made:
//!
//! - every entry's sentence on the What's new page resolves to the prose face, and every other text on the page to the editor's own, FiraMono;
//! - the prose is actually drawn (the face is registered: a face that never registers draws nothing, which is how a font swap fails);
//! - a sentence is set at no more than 75 characters to a line, counted from the glyphs the layout placed (the earlier measure test counts a monospace
//!   advance, which a proportional face does not have);
//! - the prose face carries every glyph the page's strings use: every `char` of the changelog's "For you" sentences is in its `cmap`;
//! - in the editor itself no text but a [`Prose`] one resolves to the prose face.

use std::collections::BTreeMap;
use std::path::Path;

use bevy::prelude::*;
use bevy::text::{FontSource, TextLayoutInfo};
use bevy_cauldron::launcher::{ChangelogText, HomeTab, Launcher, LauncherPlugin, NEWS_SHOWN, NewsEntry, for_you};
use bevy_cauldron::prose::{PROSE_FONT_BYTES, Prose, ProseFont};
use bevy_cauldron::screen::Screen;

use crate::fixtures::Fixture;

/// The most characters a line of prose may carry: the top of the comfortable measure (Bringhurst, 45 to 75).
const MAX_MEASURE_CHARS: usize = 75;

/// Entries with a sentence as long as the real ones, mixing every kind of character the changelog's sentences use.
fn entries(count: usize) -> String {
    let mut text = String::from("# Changelog\n\n");
    for i in 0..count {
        text.push_str(&format!(
            "- **an-entry-{i}** `{i:07}` Something that changed *For you: entry {i} says what changed, in the two or three lines a person would actually \
             read, because every sentence on this page is set as body text and every one of them is as long as this one.*\n"
        ));
    }
    text
}

/// The launcher on `tab`, drawn.
fn the_page(tag: &str, tab: HomeTab) -> App {
    let root = Fixture::new(&format!("prose_{tag}_{}", std::process::id())).descriptor("bench", "props").build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(ChangelogText(entries(NEWS_SHOWN)));
    app.add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..6 {
        app.update();
    }
    app.world_mut().resource_mut::<Launcher>().tab = tab;
    for _ in 0..10 {
        app.update();
    }
    app
}

/// Every entity under `root`.
fn under(world: &World, root: Entity) -> Vec<Entity> {
    let mut found = vec![root];
    let mut next = 0;
    while next < found.len() {
        if let Some(children) = world.get::<Children>(found[next]) {
            found.extend(children.iter());
        }
        next += 1;
    }
    found
}

/// The face the editor installs over Bevy's default id: what every text that is not prose resolves to.
fn the_editors_face() -> FontSource {
    TextFont::default().font
}

/// The widest line of a laid-out text, in glyphs.
fn widest_line(info: &TextLayoutInfo) -> usize {
    let mut lines: BTreeMap<u32, usize> = BTreeMap::new();
    for glyph in &info.glyphs {
        *lines.entry(glyph.line_index).or_default() += 1;
    }
    lines.values().copied().max().unwrap_or(0)
}

/// **Each What's new entry's sentence is set in the prose face, and nothing else on the page is.**
#[test]
fn every_whats_new_sentence_is_in_the_prose_face_and_the_rest_of_the_page_is_not() {
    let mut app = the_page("news", HomeTab::WhatsNew);
    let prose = FontSource::Handle(app.world().resource::<ProseFont>().0.clone());
    let world = app.world_mut();

    let entries: Vec<Entity> = world.query_filtered::<Entity, With<NewsEntry>>().iter(world).collect();
    assert_eq!(entries.len(), NEWS_SHOWN, "the page lists {NEWS_SHOWN} entries");
    let mut sentences = Vec::new();
    for entry in &entries {
        for entity in under(world, *entry) {
            if world.get::<Text>(entity).is_some_and(|t| t.0.starts_with("entry ")) {
                sentences.push(entity);
            }
        }
    }
    assert_eq!(sentences.len(), NEWS_SHOWN, "one sentence for each entry");
    for entity in &sentences {
        let font = world.get::<TextFont>(*entity).expect("a text has a font");
        assert_eq!(font.font, prose, "an entry's sentence resolves to the prose face, not FiraMono");
        assert!(world.get::<Prose>(*entity).is_some(), "an entry's sentence is marked `Prose`");
        let info = world.get::<TextLayoutInfo>(*entity).expect("a text has a layout");
        assert!(!info.glyphs.is_empty(), "the prose face draws: a face that is not registered draws nothing");
        let widest = widest_line(info);
        assert!(widest <= MAX_MEASURE_CHARS, "a sentence is set {widest} characters to a line, over the {MAX_MEASURE_CHARS} of a comfortable measure");
    }

    // Everything else on the page is not in the prose face, and the page's heading and its button are in the editor's own, FiraMono. (The other pages'
    // sentences are on the page too, hidden: they are prose, and resolve to the prose face. The terminal's texts have a face of their own.)
    let mono = the_editors_face();
    let mut others = 0;
    let mut texts = world.query::<(Entity, &Text, &TextFont, Has<Prose>)>();
    for (entity, text, font, is_prose) in texts.iter(world) {
        if sentences.contains(&entity) {
            continue;
        }
        if is_prose {
            assert_eq!(font.font, prose, "`{}` is marked `Prose` and resolves to the prose face", text.0);
            continue;
        }
        others += 1;
        assert_ne!(font.font, prose, "`{}` is not prose and must not resolve to the prose face", text.0);
        if text.0 == "What's new" || text.0 == "See every change" {
            assert_eq!(font.font, mono, "`{}` resolves to the editor's own face, FiraMono", text.0);
        }
    }
    assert!(others >= 5, "the launcher draws its heading, sidebar and buttons ({others})");
}

/// **The Learn page's one sentence is prose too, and its buttons are not.**
#[test]
fn the_learn_pages_sentence_is_prose_and_its_buttons_are_not() {
    let mut app = the_page("learn", HomeTab::Learn);
    let prose = FontSource::Handle(app.world().resource::<ProseFont>().0.clone());
    let world = app.world_mut();
    let mut texts = world.query::<(&Text, &TextFont)>();
    let drawn: Vec<(String, FontSource)> = texts.iter(world).map(|(t, f)| (t.0.clone(), f.font.clone())).collect();
    let intro = drawn.iter().find(|(t, _)| t.starts_with("Start with a two-minute tour")).expect("the Learn page's introduction is drawn");
    assert_eq!(intro.1, prose, "the Learn page's sentence is set in the prose face");
    let keys = drawn.iter().find(|(t, _)| t == "Keyboard shortcuts").expect("the Keyboard shortcuts button is drawn");
    assert_eq!(keys.1, the_editors_face(), "a button's label is FiraMono");
}

/// **The prose face carries every glyph the changelog's "For you" sentences use**, and the page's own sentences: the same check `ui.rs` makes for
/// FiraMono. A codepoint the face does not map is a missing-glyph box, whatever the layout does.
#[test]
fn the_prose_face_maps_every_character_of_every_for_you_sentence() {
    let face = ttf_parser::Face::parse(PROSE_FONT_BYTES, 0).unwrap_or_else(|e| panic!("the prose face does not parse: {e}"));
    let changelog = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CHANGELOG.md")).unwrap_or_else(|e| panic!("CHANGELOG.md: {e}"));
    let mut sentences: Vec<String> = for_you(&changelog).into_iter().filter_map(|n| n.user).collect();
    assert!(sentences.len() > 10, "the changelog has For-you sentences to check ({})", sentences.len());
    sentences.extend(["Nothing to tell you about yet.".to_owned(), "Start with a two-minute tour of the example, or read the guide.".to_owned()]);
    let mut missing: BTreeMap<char, String> = BTreeMap::new();
    for sentence in &sentences {
        for c in sentence.chars().filter(|c| !c.is_control()) {
            if face.glyph_index(c).is_none() {
                missing.entry(c).or_insert_with(|| sentence.chars().take(50).collect());
            }
        }
    }
    assert!(
        missing.is_empty(),
        "the prose face has no glyph for: {}",
        missing.iter().map(|(c, s)| format!("{c:?} (U+{:04X}) in {s:?}", *c as u32)).collect::<Vec<_>>().join("; ")
    );
}

/// **In the editor itself no text but a prose one resolves to the prose face**: the headings, the keycaps, the Inspector and the rest stay FiraMono.
#[test]
fn the_editor_keeps_fira_mono_for_everything_that_is_not_prose() {
    let root = Fixture::new(&format!("prose_editor_{}", std::process::id())).descriptor("bench", "props").place("bench", (1.5, 1.5)).build("m");
    let mut app = bevy_cauldron::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..30 {
        app.update();
    }
    let prose = FontSource::Handle(app.world().resource::<ProseFont>().0.clone());
    let world = app.world_mut();
    let mut texts = world.query::<(&Text, &TextFont, Has<Prose>)>();
    let (mut mono, mut proportional) = (0, 0);
    for (text, font, is_prose) in texts.iter(world) {
        if font.font == prose {
            assert!(is_prose, "`{}` resolves to the prose face but is not marked `Prose`", text.0);
            proportional += 1;
        } else {
            assert!(!is_prose, "`{}` is marked `Prose` and does not resolve to the prose face", text.0);
            mono += 1;
        }
    }
    assert!(mono > 20, "the editor draws its panels in FiraMono ({mono} texts)");
    assert_eq!(proportional, 0, "the Map door of the editor holds no sentence of prose today");
}
