//! **Finding the settings screen's search field**, for the tests that narrow its list by sending a query (#501).
//!
//! Those tests used to take "the first text input whose value is empty" as the search field. It is not reliably the first: the editor holds
//! other empty fields (the panels' filters), and which one a query returns first depends on the order the world stores them in, which changes
//! with whatever else was spawned and despawned before. When another field came first the query went to it, the list was never narrowed, the
//! Terminal agent row stayed 2,800 px down a list that shows about nine rows, and a click aimed at it hit nothing (measured on `big` for
//! `a_file_with_custom_and_no_command_is_one_problem_naming_the_file_and_the_fix_sets_omp`: four empty fields, the search the last of them).
//! It had passed only because the order happened to put the search first.
//!
//! The search field is told apart by what it is: it carries the hint the screen gives it.

use bevy::prelude::*;
use bevy_gooey::{TextHint, TextInputValue};

/// How the hint of the settings screen's search field begins (`settings_screen`: `TextHint("a word, like grid")`).
const SEARCH_HINT_PREFIX: &str = "a word";

/// **The settings screen's search field**: the empty text input that carries the search's hint. Panics, naming the fields it found, when there
/// is not exactly one.
pub fn search_field(app: &mut App) -> Entity {
    let world = app.world_mut();
    let found: Vec<(Entity, String)> = world
        .query::<(Entity, &TextInputValue, Option<&TextHint>)>()
        .iter(world)
        .filter(|(_, value, _)| value.0.is_empty())
        .map(|(entity, _, hint)| (entity, hint.map(|h| h.0.to_string()).unwrap_or_default()))
        .collect();
    let search: Vec<Entity> = found.iter().filter(|(_, hint)| hint.starts_with(SEARCH_HINT_PREFIX)).map(|(entity, _)| *entity).collect();
    match search.as_slice() {
        [one] => *one,
        _ => panic!("expected one empty text input with the search hint {SEARCH_HINT_PREFIX:?}; the empty ones are {found:?}"),
    }
}
