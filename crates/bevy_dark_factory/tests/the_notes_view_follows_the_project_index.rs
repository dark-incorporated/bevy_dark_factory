//! **The notes view reads its file when the project index says it moved, and no clock of its own** (#501, item 6: `notes_view`'s
//! `RECHECK_SECS`, 1.0 s, is retired).
//!
//! - an edit made while the editor is open is seen within the index's debounce, with the sweep set short so the view cannot be
//!   revealed by a clock of the view's own (there is none);
//! - the view reads its file again **only** when that file moved: a file beside it that is written and changed makes it read nothing
//!   (the retired timer read once a second whatever happened, so this count fails on it);
//! - a note the editor itself adds shows with no sweep at all, because the operation says it has written the file
//!   (`project_index::note_written`).

use std::time::Duration;

use bevy_dark_factory::editor::{NoteDraft, note_add_params};
use bevy_dark_factory::notes_view::NotesView;
use bevy_dark_factory::ops;
use bevy_dark_factory::project::OpenMap;
use blueprint_model::notes::{self, Notes, Target};

use crate::index_view_support as support;

fn some_notes(texts: &[&str]) -> String {
    let mut n = Notes::default();
    for (i, t) in texts.iter().enumerate() {
        n.add(vec![Target::Cell(i as i32, 0)], t).unwrap_or_else(|e| panic!("{e}"));
    }
    n.to_yaml().unwrap_or_else(|e| panic!("{e}"))
}

fn first_text(app: &bevy::prelude::App) -> Option<String> {
    app.world().resource::<NotesView>().notes().notes.first().map(|n| n.text.clone())
}

fn path_of(app: &bevy::prelude::App) -> std::path::PathBuf {
    notes::path_for_map(&app.world().resource::<OpenMap>().map_path)
}

#[test]
fn an_edit_made_while_the_editor_is_open_is_seen_within_the_index_debounce() {
    let (mut app, _ctx) = support::editor("notes_follow_edit");
    support::settle_until_the_index_is_quiet(&mut app);
    let path = path_of(&app);
    for marker in ["first-marker-edit", "second-marker-edit"] {
        std::fs::write(&path, some_notes(&[marker])).unwrap_or_else(|e| panic!("{e}"));
        let took = support::until_seen(&mut app, &format!("the notes view holds `{marker}`"), |a| first_text(a).as_deref() == Some(marker));
        assert!(took < support::SEEN_WITHIN, "the edit took {took:?} to be seen");
        // **Ground truth is a plain read of the file**: the `text:` line of the one note in it, not anything this crate derives.
        let on_disk = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{e}"));
        let said = on_disk.lines().find_map(|l| l.trim().strip_prefix("text:")).map(|t| t.trim().trim_matches(|c| c == '"' || c == '\'').to_owned());
        assert_eq!(said.as_deref(), Some(marker), "the file says `{marker}`: {on_disk}");
        assert_eq!(app.world().resource::<NotesView>().notes().notes.len(), 1, "one note in the file, one in the view");
    }
}

#[test]
fn a_file_beside_the_notes_file_that_changes_makes_the_view_read_nothing() {
    let (mut app, _ctx) = support::editor("notes_follow_neighbour");
    let path = path_of(&app);
    std::fs::write(&path, some_notes(&["standing"])).unwrap_or_else(|e| panic!("{e}"));
    support::until_seen(&mut app, "the notes read", |a| first_text(a).as_deref() == Some("standing"));
    support::a_neighbour_is_not_read(&mut app, &path, |a| a.world().resource::<NotesView>().reads());
}

#[test]
fn a_note_the_editor_adds_shows_with_no_sweep_to_reveal_it() {
    let (mut app, mut ctx) = support::editor("notes_follow_own_write");
    support::settle_until_the_index_is_quiet(&mut app);
    // **A sweep ten minutes apart cannot reveal anything in this test**: only the operation's own word that it wrote can.
    support::sweep_every(&app, Duration::from_secs(600));
    let draft = NoteDraft { area: [0, 0, 2, 2], editing: None, text: "added by the editor".into(), pieces: Vec::new(), auto: false };
    ops::dispatch(&mut ctx, "note_add", &note_add_params(&draft, None)).unwrap_or_else(|e| panic!("{e:?}"));
    let took = support::until_seen(&mut app, "the note the operation wrote", |a| first_text(a).as_deref() == Some("added by the editor"));
    assert!(took < support::SEEN_WITHIN, "the note took {took:?} to show");
    assert!(std::fs::read_to_string(path_of(&app)).unwrap_or_else(|e| panic!("{e}")).contains("added by the editor"));
}

#[test]
fn a_file_beside_a_notes_file_that_is_not_there_makes_the_view_look_at_nothing() {
    let (mut app, _ctx) = support::editor("notes_follow_neighbour_missing");
    let beside = path_of(&app);
    assert!(!beside.exists(), "this map has no notes file");
    support::a_neighbour_is_not_read(&mut app, &beside, |a| a.world().resource::<NotesView>().reads());
}
