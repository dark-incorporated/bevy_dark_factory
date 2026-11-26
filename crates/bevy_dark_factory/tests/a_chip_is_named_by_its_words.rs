//! **The editor's chip is gooey's chip: it carries `Role::Chip`, and a screen reader hears its words** (#551).
//!
//! `chrome::chip` built a clickable tag out of a raw `bevy::ui_widgets::Button`, a `RowRest` fill, the
//! editor's own border and a bare `Text` child, and it carried **no `Role`**. Two things follow from a
//! missing role, and neither is visible in a screenshot:
//!
//! 1. **gooey's `apply_skin` never styles it.** The one question every styling arm asks is what a node
//!    *is*, so a chip's box was the editor's own `StyleClass` and `BorderPx` and nothing else.
//! 2. **gooey's accessibility pass never names it.** `access.rs`'s spec is one exhaustive `match` over
//!    `Role`, and a node with no role is not in the query at all. `Role::Chip`'s arm is the one that
//!    reads *"A chip is a button named by its words"*, so the arm that would have named the chip could
//!    never run on it: the editor's chip was a button a screen reader met as an unnamed box.
//!
//! That is what these tests pin. **The name is read off the `AccessibilityNode`**, the node AccessKit
//! sends to the platform, and not off the `Text` — asking the `Text` what it says would prove that the
//! string exists, not that anything a person without a screen could ever reach it.
//!
//! The third test is the other half of an adoption: **it is not a redesign.** A chip's fill and edge
//! are the ones its caller asked for, painted by the one system that paints every row and chip
//! (`chrome::style_list_rows`), and that assertion is green before this change as well as after it.

use bevy::prelude::*;
use bevy_dark_factory::chrome;
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::Mode;
use bevy_gooey::Role;

use crate::fixtures;

/// The words on the chip under test. Not a word any panel draws, so a failure that quotes it is
/// quoting this test.
const WORDS: &str = "held";

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("chip551_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .place("site/wall", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Map).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..10 {
        app.update();
    }
    app
}

/// **A screen reader is connected.** `bevy_gooey`'s `update_accessibility` returns at once unless one
/// is (`AccessibilityRequested`), and a headless app has none — so the pass that names every widget has
/// to be asked for, exactly as a real assistive technology asks for it. Without this the tree is not
/// built and a green result would mean "nothing asked", which is the shape of proof this file exists to
/// avoid.
fn a_screen_reader_connects(app: &mut App) {
    app.world_mut().init_resource::<bevy::a11y::AccessibilityRequested>();
    app.world_mut().resource_mut::<bevy::a11y::AccessibilityRequested>().set(true);
}

/// **One chip, spawned the way every panel spawns one**, and the frames it takes to be laid out and
/// painted. The `accent` edge is the one a held tag chip carries (B-047's shape).
fn a_chip(app: &mut App) -> Entity {
    let holder = app.world_mut().spawn(Node::default()).id();
    let mut chip = Entity::PLACEHOLDER;
    app.world_mut().commands().entity(holder).with_children(|parent| {
        chip = chrome::chip(
            parent,
            (),
            WORDS,
            chrome::text::CONTROL,
            chrome::ink().text,
            chrome::ink().row_bg,
            chrome::ink().accent,
        )
        .id();
    });
    for _ in 0..4 {
        app.update();
    }
    chip
}

/// **The words the chip draws**: the `Text` of its own child, which is the string a person reads on it.
fn drawn_words(world: &World, chip: Entity) -> Option<String> {
    let children = world.get::<Children>(chip)?;
    children
        .iter()
        .filter_map(|child| world.get::<Text>(child).map(|text| text.0.clone()))
        .find(|words| !words.trim().is_empty())
}

/// **The name a screen reader gets for `entity`**, as `bevy_gooey` published it.
///
/// `AccessibilityNode` is a plain newtype over `accesskit::Node` in Bevy 0.20, so the node is read
/// directly and `label()` is accesskit's own accessor — the same field `access.rs` writes from
/// `Wanted::name` and the platform is handed. This is the assertion the issue is about: the editor's
/// chip had no `AccessibilityNode` of its own at all, so it read `None`.
fn published_name(world: &World, entity: Entity) -> Option<String> {
    let node = world.get::<bevy::a11y::AccessibilityNode>(entity)?;
    node.0.label().map(str::to_owned).filter(|name| !name.trim().is_empty())
}

/// **A chip has a name a screen reader can read, and it is the words the chip shows** (#551).
///
/// This is the consumer-visible half of the missing `Role`. A person who cannot see the panel met the
/// tag chips as unnamed buttons: the words were drawn, but nothing carried them into the accessibility
/// tree, because the arm that names a chip (`Role::Chip`) is reached through the role the editor never
/// put on the node.
#[test]
fn a_chip_is_named_by_the_words_its_own_text_shows() {
    let mut app = editor("name");
    a_screen_reader_connects(&mut app);
    let chip = a_chip(&mut app);
    let drawn = drawn_words(app.world(), chip).unwrap_or_else(|| panic!("a chip draws its words"));
    assert_eq!(drawn, WORDS, "the chip draws the words it was given");
    assert_eq!(
        published_name(app.world(), chip).as_deref(),
        Some(WORDS),
        "a screen reader is handed the chip's words as its name. `None` here is the defect #551 \
         records: a chip is a button named by its words, and an unnamed button in a row of tags is \
         indistinguishable from the one beside it"
    );
}

/// **A chip is gooey's chip, not the editor's second one** (#551): the node carries `Role::Chip`.
///
/// The role is the whole mechanism. `bevy_gooey`'s `apply_skin` styles a node by what its role says it
/// is, and its accessibility pass is one exhaustive `match` over the same role, so a node without one
/// is styled by neither. `Role::Chip` is what makes the two arms — the chip's box, and *"a button named
/// by its words"* — reachable at all.
#[test]
fn a_chip_carries_gooeys_chip_role() {
    let mut app = editor("role");
    let chip = a_chip(&mut app);
    assert_eq!(
        app.world().get::<Role>(chip),
        Some(&Role::Chip),
        "a chip is gooey's chip; without the role neither the skin nor the accessibility pass can \
         see it"
    );
}

/// **A chip's fill and edge are the ones its caller asked for, and adopting the role did not repaint
/// it** (#551's third acceptance item: *"removing the editor's own paint leaves the chips looking the
/// same"*).
///
/// Measured on the shipped skin: a chip spawned with `row_bg` and `accent` is drawn in `row_bg` with an
/// `accent` edge, and its words in the caller's ink.
#[test]
fn a_chip_keeps_the_fill_and_edge_its_caller_asked_for() {
    let mut app = editor("look");
    let chip = a_chip(&mut app);
    assert_eq!(
        app.world().get::<BackgroundColor>(chip).map(|colour| colour.0),
        Some(chrome::ink().row_bg),
        "a chip at rest is drawn in the fill its caller named"
    );
    assert_eq!(
        app.world().get::<BorderColor>(chip).map(|colour| colour.top),
        Some(chrome::ink().accent),
        "and its edge is the colour its caller named, not the skin's outline"
    );
    let words = app
        .world()
        .get::<Children>(chip)
        .and_then(|children| children.iter().find(|child| app.world().get::<Text>(*child).is_some()))
        .expect("the chip's words");
    assert_eq!(
        app.world().get::<TextColor>(words).map(|colour| colour.0),
        Some(chrome::ink().text),
        "and its words are drawn in the ink its caller named"
    );
}
