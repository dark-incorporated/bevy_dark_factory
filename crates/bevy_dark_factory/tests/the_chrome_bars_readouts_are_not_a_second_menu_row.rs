//! **The chrome bar's readouts are not a second menu row** (#480, feedback-1794783606350: *"The debug text is
//! overlayed on the editor top menu like file, edit, view--we should make it distinct."*).
//!
//! Measured on the owner's frame: the menu row is `y=0..77` in `(5,5,5)`, the chrome bar `y=79..139` in
//! `(34,34,34)`, a 1 px rule at `y=78` between them. Both bands' words were drawn at `chrome::text::BODY`
//! (`chrome::spawn_chrome_bar`'s back chip and `WhereYouAre`), so the band's state words and the menu's commands
//! were one texture and the eye could not tell a verb from a readout.
//!
//! The fact is stated once, where both are spawned: the chrome bar's readouts are drawn at
//! [`chrome::BAR_READOUT`], a role **smaller** than the words the menu row draws, and in ink that is not the menu's.
//! Nothing about what either band says changes — the assertions here read the words back to prove it.

use bevy::prelude::*;
use bevy_dark_factory::chrome::{self, TypeRole, WhereYouAre};

use crate::fixtures;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("{tag}_{}", std::process::id())).descriptor("floor", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    app
}

/// **A `TextFont`'s size in logical px.** The field is a `FontSize`, which may be stated in a viewport unit; a label
/// this test cares about is always `Px`, and anything else would be a size nobody stated in the editor's scale.
fn px_of(size: bevy::text::FontSize) -> f32 {
    match size {
        bevy::text::FontSize::Px(px) => px,
        other => panic!("a label is drawn in logical px, not {other:?}"),
    }
}

/// **What a menu title's words are drawn at**: its `Text` child's font size, in logical px, and its ink.
///
/// Read off the `TextFont`, not off a role: gooey sizes a menu title from its own skin, so the menu's size is a
/// measurement rather than a constant this test could restate.
fn a_menu_titles_words(app: &mut App) -> (f32, Color) {
    let world = app.world_mut();
    let titles: Vec<Entity> = world.query_filtered::<Entity, With<bevy_gooey::MenuTitle>>().iter(world).collect();
    assert!(!titles.is_empty(), "the menu row carries titles");
    for title in &titles {
        let Some(kids) = world.get::<Children>(*title) else { continue };
        for kid in kids.iter() {
            if let (Some(font), Some(ink)) = (world.get::<TextFont>(kid), world.get::<TextColor>(kid)) {
                return (px_of(font.font_size), ink.0);
            }
        }
    }
    panic!("a menu title draws its words in a `TextFont`");
}

/// **A readout on the chrome bar**: its words, the role it is drawn at, its font size and its ink.
struct Readout {
    entity: Entity,
    words: String,
    role: TypeRole,
    size: f32,
    ink: Color,
}

/// **Every label under `Frame::chrome_bar` that carries a `chrome::TypeRole`** — the bar's readouts, and nothing the
/// frames elsewhere in the world happen to hold.
fn the_readouts(app: &mut App) -> Vec<Readout> {
    let world = app.world_mut();
    let bar = world.resource::<chrome::Frame>().chrome_bar;
    let mut under = Vec::new();
    let mut walk = vec![bar];
    while let Some(entity) = walk.pop() {
        if let Some(kids) = world.get::<Children>(entity) {
            walk.extend(kids.iter());
        }
        if let (Some(role), Some(font)) = (world.get::<TypeRole>(entity), world.get::<TextFont>(entity)) {
            under.push(Readout {
                entity,
                words: world.get::<Text>(entity).map_or_else(String::new, |text| text.0.clone()),
                role: *role,
                size: px_of(font.font_size),
                ink: world.get::<TextColor>(entity).map_or(Color::BLACK, |ink| ink.0),
            });
        }
    }
    assert!(!under.is_empty(), "the chrome bar draws readouts");
    under
}

/// **The menu's words and the bar's readouts are drawn at different sizes, and in different ink** — the fact stated
/// once, where both are spawned.
#[test]
fn the_menus_words_and_the_chrome_bars_readouts_differ_in_size_and_ink() {
    let mut app = editor("chrome_readouts");
    let (menu_size, menu_ink) = a_menu_titles_words(&mut app);
    assert!(menu_size > 0.0, "a menu title is drawn at {menu_size} px");
    for readout in the_readouts(&mut app) {
        assert!(
            readout.size < menu_size,
            "{:?} (`{}`) is drawn at {} px against the menu's {menu_size} px: the readout and the command are one \
             texture, which is what the owner read as debug text over the menu",
            readout.entity,
            readout.words,
            readout.size
        );
        assert!(
            readout.ink != menu_ink,
            "{:?} (`{}`) is drawn in the menu titles' own ink: the band reads as a second menu row",
            readout.entity,
            readout.words
        );
    }
}

/// **Every readout on the bar is drawn at [`chrome::BAR_READOUT`]**, so a word added to the band later is a readout
/// by construction rather than by whoever remembers to pick a small role.
#[test]
fn every_readout_on_the_bar_is_drawn_at_the_bars_own_role() {
    let mut app = editor("chrome_readouts_role");
    for readout in the_readouts(&mut app) {
        assert_eq!(
            readout.role.role().px(),
            chrome::BAR_READOUT.px(),
            "{:?} (`{}`) is drawn at {:?}, and a readout on the chrome bar is `chrome::BAR_READOUT` ({:?})",
            readout.entity,
            readout.words,
            readout.role.role().px(),
            chrome::BAR_READOUT.px()
        );
    }
}

/// **The bar's words are unchanged**: it still says where the person is, and the menu row still leads with File.
#[test]
fn the_bar_still_says_where_you_are() {
    let mut app = editor("chrome_readouts_words");
    let said: Vec<String> = {
        let world = app.world_mut();
        world.query_filtered::<&Text, With<WhereYouAre>>().iter(world).map(|text| text.0.clone()).collect()
    };
    assert_eq!(said.len(), 1, "one `WhereYouAre` on the bar: {said:?}");
    assert!(!said[0].is_empty(), "and it says where the person is");
    let titles: Vec<String> = {
        let world = app.world_mut();
        let titles: Vec<Entity> = world.query_filtered::<Entity, With<bevy_gooey::MenuTitle>>().iter(world).collect();
        titles.iter().flat_map(|title| world.get::<Children>(*title).map(|kids| kids.iter().collect::<Vec<_>>()).unwrap_or_default()).filter_map(|kid| world.get::<Text>(kid).map(|text| text.0.clone())).collect()
    };
    assert!(titles.iter().any(|words| words == "File"), "the menu row still leads with File: {titles:?}");
}
