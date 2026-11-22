//! **Help ▸ Keyboard shortcuts is the registry's own list, with each command's binding as it stands** (UI-91, #257).
//!
//! The reference is not a page somebody keeps in step: it is one row per command of `commands::all()`, in the registry's order,
//! each showing the chord that command answers to now. A command added to the registry has a row without anyone writing it, and
//! a command a person rebinds shows the new chord and no longer the old one.
//!
//! Contract update (2026-11-22): the editor's fixed chords (`chords::CHORDS`: the console, the palette, the terminal's own) are
//! settings now and rebound on the same screen, so their rows follow the commands', in the table's order.

use std::path::PathBuf;

use bevy::prelude::*;
use bevy_dark_factory::app_verbs::AppVerb;
use bevy_dark_factory::commands;
use bevy_dark_factory::keymap_screen::shown;
use bevy_dark_factory::settings::{SettingsLayer, UserSettingsFile};
use bevy_gooey::testing::find_by_label;
use bevy_gooey::{Activate, Binding, BindingValue, Modifiers, Shortcut};

use crate::fixtures;

/// **The rows the reference shows with nothing rebound but `rebound`**: every command in the registry's order, then every fixed
/// chord in its table's order, each on its default chord.
fn every_row(rebound: Option<(&str, Shortcut)>) -> Vec<Shortcut> {
    let moved = |id: &str, default: Shortcut| rebound.filter(|(r, _)| *r == id).map_or(default, |(_, chord)| chord);
    commands::all()
        .iter()
        .map(|c| moved(c.id, c.shortcut))
        .chain(bevy_dark_factory::chords::CHORDS.iter().map(|named| moved(named.id, (named.default)())))
        .collect()
}

fn updates(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

/// An editor whose user settings are a file of its own, with the reference not yet open.
fn editor(tag: &str) -> (App, PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let user = root.join(format!("user_config_{tag}_{}/settings.yaml", std::process::id()));
    std::fs::create_dir_all(user.parent().unwrap()).unwrap();
    app.insert_resource(UserSettingsFile(user.clone()));
    updates(&mut app, 6);
    (app, user)
}

/// The person chose Help ▸ Keyboard shortcuts: the row of the menu, activated.
fn open_the_reference(app: &mut App) {
    let row = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &AppVerb)>();
        q.iter(world).find(|(_, verb)| **verb == AppVerb::KeyboardShortcuts).map(|(e, _)| e).expect("Help offers Keyboard shortcuts")
    };
    app.world_mut().commands().trigger(Activate { entity: row });
    updates(app, 4);
}

/// The chord of every row the reference shows, in the order it shows them.
fn rows_shown(app: &mut App) -> Vec<Shortcut> {
    let world = app.world_mut();
    let mut q = world.query_filtered::<Entity, With<BindingValue>>();
    let rows: Vec<Entity> = q.iter(world).collect();
    rows.into_iter().filter_map(|row| shown(world, row)).collect()
}

fn narrow_to(app: &mut App, words: &str) {
    let row = app.world_mut().query_filtered::<Entity, With<bevy_gooey::TextHint>>().iter(app.world()).next().expect("the search row");
    app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange { source: row, value: words.to_owned(), is_final: false });
    updates(app, 3);
}

#[test]
fn the_reference_has_one_row_per_command_each_on_the_chord_it_answers_to() {
    let (mut app, _user) = editor("shortcuts_every");
    open_the_reference(&mut app);
    // The registry's own defaults, read off each command and each fixed chord, not through the screen's resolver.
    let commands_only = commands::all().len();
    assert!(commands_only > 50, "the registry is the editor's whole vocabulary of commands: {commands_only} of them");
    let want = every_row(None);
    let got = rows_shown(&mut app);
    assert_eq!(got.len(), want.len(), "one row for every command of the registry and every fixed chord, and no row for anything else");
    assert_eq!(got, want, "each row, in the registry's order and then the chords', shows the chord it answers to");
}

#[test]
fn a_rebinding_shows_on_the_command_and_the_old_chord_is_left() {
    let (mut app, user) = editor("shortcuts_rebound");
    open_the_reference(&mut app);
    let redo = commands::by_id("edit.redo").expect("Redo is a command");
    let elsewhere = Shortcut::new(Modifiers::PRIMARY | Modifiers::ALT, KeyCode::KeyJ);
    assert_ne!(redo.shortcut, elsewhere);

    narrow_to(&mut app, "redo");
    let row = find_by_label(&mut app, redo.title).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(shown(app.world(), row), Some(redo.shortcut), "Redo starts on its default chord");
    app.world_mut().commands().trigger(bevy::ui_widgets::ValueChange { source: row, value: Binding::Chord(elsewhere), is_final: true });
    updates(&mut app, 3);
    let layer = SettingsLayer::read("test", &std::fs::read_to_string(&user).expect("the rebinding was written")).unwrap();
    assert_eq!(layer.keymap.get("edit.redo"), Some(&elsewhere), "the person's file holds the rebinding");

    narrow_to(&mut app, "");
    // Every command and fixed chord on its default, except the one the person moved.
    let want = every_row(Some(("edit.redo", elsewhere)));
    let got = rows_shown(&mut app);
    assert_eq!(got, want, "the whole reference follows the file: the one rebound command shows its new chord, every other its own");

    narrow_to(&mut app, "redo");
    let row = find_by_label(&mut app, redo.title).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(shown(app.world(), row), Some(elsewhere), "Redo's own row shows the new chord, not the default it had");
}
