//! **A document tab is `bevy_gooey`'s `edge_tab`** (#645), so it carries a gooey `Role` — and with it a hover
//! state and the skin's own ink, which the hand-built tab never had.
//!
//! The issue's measurement: `session::draw_the_tabs` builds each tab from a `row_with`, a literal `Spacing`, a
//! 3 px `BorderPx`, an `InkOverride` and two colours baked at spawn. **It carries no `Role`**, so gooey's
//! `apply_skin` never styles it, and because the colour is baked there is no hover state at all — the only
//! state it can show is `active`, decided on the frame the strip was last rebuilt.
//!
//! So the assertions are the two things that follow from carrying a `Role`: it is there, and it is
//! `Role::EdgeTab` specifically (not `Role::Row`, which a plain row would give it).

use bevy::prelude::*;
use bevy_dark_factory::session::{Doc, DocumentStrip, DocumentTab, Kind};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("doc_tab_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"))
}

fn frames(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// **Two documents in the Map workspace draw two tabs, and each carries gooey's `Role::EdgeTab`.**
#[test]
fn a_document_tab_is_gooeys_edge_tab() {
    let mut app = editor("edge_tab");
    frames(&mut app, 3);

    // **Opened through the session**, which is the door the strip reads, rather than by synthesising input.
    {
        let world = app.world_mut();
        let mut session = world.resource_mut::<bevy_dark_factory::session::Session>();
        for name in ["alpha", "beta"] {
            session
                .open(Workspace::Map, Doc { kind: Kind::Map, name: name.to_owned() })
                .unwrap_or_else(|e| panic!("opening `{name}`: {e}"));
        }
    }
    frames(&mut app, 6);

    let world = app.world_mut();
    let strips = world
        .query_filtered::<Entity, With<DocumentStrip>>()
        .iter(world)
        .count();
    assert_eq!(strips, 1, "the Map workspace draws one document strip");

    let tabs: Vec<(Entity, bool)> = world
        .query::<(Entity, Option<&bevy_gooey::Role>)>()
        .iter(world)
        .filter(|(entity, _)| world.get::<DocumentTab>(*entity).is_some())
        .map(|(entity, role)| (entity, matches!(role, Some(bevy_gooey::Role::EdgeTab))))
        .collect();

    assert_eq!(
        tabs.len(),
        2,
        "**two documents open and the strip drew {} tab(s)** (#645). If this is zero, check the strip's \\
         rebuild guard: `draw_the_tabs` returns early when `drawn == want`, so a test that opens documents \\
         without changing the tab set sees an empty strip",
        tabs.len()
    );
    let without_role: Vec<Entity> = tabs
        .iter()
        .filter(|(_, has_edge_tab)| !has_edge_tab)
        .map(|(entity, _)| *entity)
        .collect();
    assert!(
        without_role.is_empty(),
        "**{} of {} document tabs carry no `Role::EdgeTab`** (#645), so gooey's `apply_skin` never styles \\
         them and they have no hover state: {without_role:?}. The tab should be \\
         `bevy_gooey::edge_tab(name)` rather than a hand-built `row_with`",
        without_role.len(),
        tabs.len()
    );
}

/// **The selected tab is the open document, for files as for maps** (#688). `select_the_tab` used to
/// select the tab named for the open *map*, so in the Files workspace no tab was ever selected and two
/// open files drew identical tabs.
#[test]
fn the_open_files_tab_is_the_selected_one() {
    let mut app = editor("selected");
    frames(&mut app, 3);
    let files = ["design/notes.md", "design/style.md"];
    {
        let world = app.world_mut();
        world.insert_resource(Workspace::Files);
        let mut session = world.resource_mut::<bevy_dark_factory::session::Session>();
        for name in files {
            session
                .open(Workspace::Files, Doc { kind: Kind::File, name: name.to_owned() })
                .unwrap_or_else(|e| panic!("opening `{name}`: {e}"));
        }
    }
    frames(&mut app, 6);

    let selected = |app: &mut App| -> Vec<String> {
        let world = app.world_mut();
        world
            .query_filtered::<&DocumentTab, With<bevy::ui::Selected>>()
            .iter(world)
            .map(|tab| tab.0.name.clone())
            .collect()
    };
    let active = |app: &App| {
        app.world()
            .resource::<bevy_dark_factory::session::Session>()
            .active_doc(Workspace::Files)
            .map(|doc| doc.name.clone())
    };

    let want = active(&app).unwrap_or_else(|| panic!("two files were opened and the Files workspace has no active one"));
    assert_eq!(
        selected(&mut app),
        vec![want.clone()],
        "**with two files open, exactly one tab is `Selected` and it is the active document `{want}`** (#688)"
    );

    app.world_mut()
        .resource_mut::<bevy_dark_factory::session::Session>()
        .step(Workspace::Files, false);
    frames(&mut app, 4);
    let stepped = active(&app).unwrap_or_else(|| panic!("stepping the tabs left no active document"));
    assert_ne!(stepped, want, "stepping two tabs changes the active document");
    assert_eq!(selected(&mut app), vec![stepped.clone()], "**`Selected` follows the active document to `{stepped}`** (#688)");
}
