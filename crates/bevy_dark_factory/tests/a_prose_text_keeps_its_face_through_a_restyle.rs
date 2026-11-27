//! **A `Prose` text keeps the prose face through a restyle** (#559, `bevy_gooey#118`).
//!
//! gooey's `apply_skin` is the one writer of a text's face: it writes `GooeyFonts::body` from the role
//! on every restyle (a state change, a move, a skin change, the first frame's real render target). Until
//! `TextFace` (v0.60.0, `678b008a`) a host had no way to say "this one text is in my face", so
//! `prose.rs` ran `set_the_prose_face` after `GooeySystems::Style` every frame — two writers of one
//! component, the defect the crate's own guide names. The spawn sites now pin a `TextFace` and that
//! system is gone; this file holds that the face survives what used to overwrite it.
//!
//! Two things are proved: **the face survives a restyle** (a skin change moves the role's size and must
//! not move the host's face), and **the editor no longer runs a second writer** (a ratchet over `src/`).
//! The restyle is the one that used to lose the face, so a version of `prose.rs` that spawns `Prose`
//! without the `TextFace` — or that keeps the after-style system but drops it — fails here.

use bevy::prelude::*;
use bevy::text::FontSource;
use bevy_dark_factory::harness;
use bevy_dark_factory::launcher::{ChangelogText, HomeTab, Launcher, LauncherPlugin, NEWS_SHOWN, NewsEntry};
use bevy_dark_factory::prose::{Prose, ProseFont};
use bevy_dark_factory::screen::Screen;
use bevy_gooey::TextFace;

use crate::fixtures;

/// Entries with a sentence as long as the real ones, so the page draws its prose.
fn entries(count: usize) -> String {
    let mut text = String::from("# Changelog\n\n");
    for i in 0..count {
        text.push_str(&format!(
            "- **an-entry-{i}** `{i:07}` Something that changed *For you: entry {i} says what changed, in the two or three lines a person would \
             actually read, because every sentence on this page is set as body text and every one of them is as long as this one.*\n"
        ));
    }
    text
}

/// The launcher on the What's new page, drawn.
fn the_page(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("prose_face_{tag}_{}", std::process::id())).descriptor("bench", "props").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(ChangelogText(entries(NEWS_SHOWN)));
    app.add_plugins(LauncherPlugin);
    app.world_mut().resource_mut::<NextState<Screen>>().set(Screen::Launcher);
    for _ in 0..6 {
        app.update();
    }
    app.world_mut().resource_mut::<Launcher>().tab = HomeTab::WhatsNew;
    for _ in 0..10 {
        app.update();
    }
    app
}

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

/// The face a text is drawn in.
fn face_of(world: &World, text: Entity) -> FontSource {
    world.get::<TextFont>(text).expect("a text has a font").font.clone()
}

/// The prose face's handle.
fn the_prose_face(app: &App) -> FontSource {
    FontSource::Handle(app.world().resource::<ProseFont>().0.clone())
}

/// Every entry's sentence on the What's new page, in page order.
fn the_sentences(app: &App) -> Vec<Entity> {
    let world = app.world();
    let entries: Vec<Entity> = {
        let mut query = world.try_query_filtered::<Entity, With<NewsEntry>>().expect("`NewsEntry` is registered");
        query.iter(world).collect()
    };
    assert_eq!(entries.len(), NEWS_SHOWN, "the page lists {NEWS_SHOWN} entries");
    let mut sentences = Vec::new();
    for entry in &entries {
        for entity in under(world, *entry) {
            if world.get::<Text>(entity).is_some_and(|t| t.0.starts_with("entry ")) {
                sentences.push(entity);
            }
        }
    }
    assert_eq!(sentences.len(), NEWS_SHOWN, "one sentence each entry");
    sentences
}

/// **Every prose text is spawned with the face it draws, and gooey writes that face, not the role's.**
///
/// The assertion is gooey's own rule read back: `apply_skin` writes the role's `GooeyFonts::body` over any
/// `TextFont` a host sets, and takes a `TextFace` instead (`skin.rs`: *the host's face wins over the
/// role's*). So the face this test reads on the opening frame can only be there because the component is
/// — take it off, and the next restyle must put the monospace face back. That is the two-writer defect the
/// old after-style system existed to paper over, proved from the other side.
#[test]
fn a_prose_texts_face_is_gooeys_to_paint_from_the_text_face() {
    let mut app = the_page("face");
    let prose = the_prose_face(&app);
    let sentences = the_sentences(&app);
    for &text in &sentences {
        assert!(app.world().get::<Prose>(text).is_some(), "an entry's sentence is marked `Prose`");
        assert!(app.world().get::<TextFace>(text).is_some(), "and spawned with a `TextFace`, which is how a host pins one");
        assert_eq!(face_of(app.world(), text), prose, "an entry's sentence is drawn in the prose face");
    }

    // **Taking the component off gives the role's face back**, which is gooey's removal restyle and not
    // something this crate runs. A face that stayed would be a second writer still alive somewhere.
    let mono = FontSource::Handle(Default::default());
    let one = sentences[0];
    app.world_mut().entity_mut(one).remove::<TextFace>();
    for _ in 0..6 {
        app.update();
    }
    assert_ne!(face_of(app.world(), one), prose, "the prose face stayed after its `TextFace` was removed, so something here still writes it");
    assert_eq!(face_of(app.world(), one), mono, "gooey gave the text back its role's face");
    for &text in &sentences[1..] {
        assert_eq!(face_of(app.world(), text), prose, "the sentence beside it is untouched");
    }
}

/// **A prose text moved under a different parent keeps its face.** A `ChildOf` write is a restyle
/// trigger in gooey, and this moves a real page entry's sentence rather than a stand-in.
#[test]
fn a_moved_prose_text_keeps_its_face() {
    let mut app = the_page("move");
    let prose = the_prose_face(&app);
    let sentence = the_sentences(&app)[0];
    assert_eq!(face_of(app.world(), sentence), prose);

    let entry = app.world().get::<ChildOf>(sentence).expect("a parent").parent();
    let host = app.world().get::<ChildOf>(entry).expect("a grandparent").parent();
    let new_home = app.world_mut().spawn((Node { flex_direction: FlexDirection::Column, ..default() }, ChildOf(host))).id();
    app.world_mut().entity_mut(new_home).add_child(sentence);
    for _ in 0..6 {
        app.update();
    }
    assert_eq!(face_of(app.world(), sentence), prose, "a moved prose text is still drawn in the prose face");
}

/// **No `src/` file writes a `TextFont` on a `Role` text any more** (#559's ratchet).
///
/// A second writer of the face is the defect, and `set_the_prose_face` was the one this crate ran. The
/// writers that remain are listed by name; each is a text with no `Role`, which gooey never restyles.
#[test]
fn no_source_file_writes_a_role_texts_face() {
    /// The writers that remain, with why each is not a second writer of a role's face.
    const ALLOWED: [(&str, &str); 3] = [
        ("terminal_view.rs", "the terminal's rows and caret have no `Role`: gooey does not restyle them"),
        ("terminal_pane.rs", "the pane's own glyph rows, likewise"),
        ("terminal_agent.rs", "the agent transcript's texts, likewise"),
    ];
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    for path in walk(&src) {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
        if !name.ends_with(".rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
        // A system that takes a `&mut TextFont` writes a face; a `TextFont` read is not a writer.
        let writes_a_font = text.contains("Query<&mut TextFont");
        if writes_a_font && !ALLOWED.iter().any(|(file, _)| *file == name) {
            offenders.push(name);
        }
    }
    assert!(
        offenders.is_empty(),
        "these files take a `&mut TextFont` from a query, which is a second writer of the face gooey's `apply_skin` owns \
         (a `TextFace` is how a host pins one): {offenders:?}"
    );
}

/// Every file under `dir`, recursively.
fn walk(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return found };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(walk(&path));
        } else {
            found.push(path);
        }
    }
    found
}
