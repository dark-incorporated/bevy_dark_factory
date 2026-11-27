//! **The editor's chip is gooey's chip: it carries `Role::Chip`, a screen reader hears its words as a
//! button's name, and a click still reaches the panel behind it** (#551).
//!
//! `chrome::chip` built a clickable tag out of a raw `bevy::ui_widgets::Button`, a `RowRest` fill, the
//! editor's own border and a bare `Text` child, and it carried **no `Role`**. Two things follow from a
//! missing role, and neither is visible in a screenshot:
//!
//! 1. **gooey's `apply_skin` never styles it.** The one question every styling arm asks is what a node
//!    *is*, so a chip's box was the editor's own `StyleClass` and `BorderPx` and nothing else.
//! 2. **gooey's accessibility pass never names it.** `access.rs`'s spec is one exhaustive `match` over
//!    `Role`, and the arm that reads *"A chip is a button named by its words"* never ran on a node whose
//!    role was a layout role: the editor's chip was a button a screen reader met as an unnamed box.
//!
//! **The name and the role are read off the `AccessibilityNode`**, the node AccessKit hands the
//! platform, and not off the `Text` — asking the `Text` what it says would prove that the string exists,
//! not that a person without a screen could ever reach it.
//!
//! The last two tests are the other half of an adoption: **it is not a redesign.** A chip's fill and edge
//! are the ones its caller asked for, painted by the one system that paints every row and chip
//! (`chrome::style_list_rows`), and a click through the pointer chain still reaches the `On<Activate>`
//! observer behind a tag chip. Both are green before this change as well as after it; the second is here
//! because gooey's `quiet_chip` alone is not on the `Activate` bus, and every other test that "clicks" a
//! chip triggers `Activate` itself, so it would not notice a chip that stopped answering the pointer.

use bevy::a11y::AccessibilityNode;
use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_dark_factory::chrome;
use bevy_dark_factory::harness;
use bevy_dark_factory::project::Project;
use bevy_dark_factory::tiles::{self, Mode};
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
        chip = chrome::chip(parent, (), WORDS, chrome::text::CONTROL, chrome::ink().text, chrome::ink().row_bg, chrome::ink().accent).id();
    });
    for _ in 0..4 {
        app.update();
    }
    chip
}

/// **The words the chip draws**: the `Text` of its own child, which is the string a person reads on it.
fn drawn_words(world: &World, chip: Entity) -> Option<String> {
    world.get::<Children>(chip)?.iter().filter_map(|child| world.get::<Text>(child).map(|text| text.0.clone())).find(|words| !words.trim().is_empty())
}

/// **The node `bevy_gooey` published for `entity`.** `AccessibilityNode` is a plain newtype over
/// `accesskit::Node` in Bevy 0.20, so its `label()` and `role()` are accesskit's own accessors — the
/// fields `access.rs` writes and the platform is handed.
fn published(world: &World, entity: Entity) -> Option<&accesskit::Node> {
    world.get::<AccessibilityNode>(entity).map(|node| &node.0)
}

/// **A chip is gooey's chip, not the editor's second one** (#551): the node carries `Role::Chip`.
///
/// The role is the whole mechanism. `bevy_gooey`'s `apply_skin` styles a node by what its role says it
/// is, and its accessibility pass is one exhaustive `match` over the same role, so a node with a layout
/// role is styled and named as a box. `Role::Chip` is what makes the two arms — the chip's box, and *"a
/// button named by its words"* — reachable at all.
#[test]
fn a_chip_carries_gooeys_chip_role() {
    let mut app = editor("role");
    let chip = a_chip(&mut app);
    assert_eq!(app.world().get::<Role>(chip), Some(&Role::Chip), "a chip is gooey's chip; without the role neither the skin nor the accessibility pass can see it");
}

/// **A chip has a name a screen reader can read, and it is the words the chip shows** (#551).
///
/// The consumer-visible half of the missing `Role`. A person who cannot see the panel met the tag chips
/// as unnamed buttons: the words were drawn, but nothing carried them into the accessibility tree.
#[test]
fn a_chip_is_named_by_the_words_its_own_text_shows() {
    let mut app = editor("name");
    a_screen_reader_connects(&mut app);
    let chip = a_chip(&mut app);
    let drawn = drawn_words(app.world(), chip).unwrap_or_else(|| panic!("a chip draws its words"));
    assert_eq!(drawn, WORDS, "the chip draws the words it was given");
    assert_eq!(
        published(app.world(), chip).and_then(accesskit::Node::label),
        Some(WORDS),
        "a screen reader is handed the chip's words as its name. `None` here is the defect #551 records: a chip is a button named by \
         its words, and an unnamed button in a row of tags is indistinguishable from the one beside it"
    );
}

/// **And it hears a button** (#551, gooey's own spec: *"A chip is a button named by its words"*).
///
/// gooey's `Role::Chip` arm writes the name and leaves the AccessKit role to the node it finds: Bevy's
/// `Button` requires one as `accesskit::Role::Button`, and a chip with no `Button` — `quiet_chip` as gooey
/// ships it — is given a generic container. Before #551 the layout role's arm wrote the container over the
/// button's node.
#[test]
fn a_screen_reader_hears_a_chip_as_a_button() {
    let mut app = editor("button");
    a_screen_reader_connects(&mut app);
    let chip = a_chip(&mut app);
    assert_eq!(
        published(app.world(), chip).map(accesskit::Node::role),
        Some(accesskit::Role::Button),
        "a chip is announced as a button, so a person knows it can be pressed"
    );
}

/// **A chip's fill and edge are the ones its caller asked for, and adopting the role did not repaint
/// it** (#551: *"removing the editor's own paint leaves the chips looking the same"*).
///
/// Measured on the shipped skin: a chip spawned with `row_bg` and `accent` is drawn in `row_bg` with an
/// `accent` edge, and its words in the caller's ink.
#[test]
fn a_chip_keeps_the_fill_and_edge_its_caller_asked_for() {
    let mut app = editor("look");
    let chip = a_chip(&mut app);
    assert_eq!(app.world().get::<BackgroundColor>(chip).map(|colour| colour.0), Some(chrome::ink().row_bg), "a chip at rest is drawn in the fill its caller named");
    assert_eq!(app.world().get::<BorderColor>(chip).map(|colour| colour.top), Some(chrome::ink().accent), "and its edge is the colour its caller named, not the skin's outline");
    let words = app.world().get::<Children>(chip).and_then(|children| children.iter().find(|child| app.world().get::<Text>(*child).is_some())).expect("the chip's words");
    assert_eq!(app.world().get::<TextColor>(words).map(|colour| colour.0), Some(chrome::ink().text), "and its words are drawn in the ink its caller named");
}

/// **A click through the pointer chain on a `kind` tag chip holds the token** — the chip is still on the
/// `Activate` bus that `tiles::on_tag_chip` listens on.
///
/// `tiles::click_tag_chip_for_test` triggers `Activate` itself, which is the right tool for the tests that
/// are about the label write and the wrong one for this: a chip that no longer sent `Activate` on a click
/// would pass every one of them. So this presses and releases the primary button over the chip's centre,
/// as the window would.
#[test]
fn a_click_on_a_tag_chip_holds_its_token() {
    let root = fixtures::Fixture::new(&format!("chip551_click_{}", std::process::id())).descriptor("crate", "alpha").kind_tokens(&["container", "decor"]).build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Meshes).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let id = app.world().resource::<Project>().library.descriptors.first().map(|d| d.id.clone()).unwrap_or_else(|| panic!("the fixture carries a descriptor"));
    app.world_mut().resource_mut::<tiles::ImportState>().selected_library_id = Some(id);
    for _ in 0..6 {
        app.update();
    }
    let kinds = |app: &App| app.world().resource::<Project>().measured.descriptors.first().map(|d| d.kind.clone()).unwrap_or_default();
    assert!(!kinds(&app).iter().any(|k| k == "decor"), "the piece starts without `decor`: {:?}", kinds(&app));
    let mut chips = app.world_mut().query::<(Entity, &chrome::RowRest, &UiGlobalTransform, &ComputedNode)>();
    let world = app.world();
    let decor: Vec<(Vec2, Vec2)> = chips
        .iter(world)
        .filter(|(chip, ..)| drawn_words(world, *chip).as_deref() == Some("decor"))
        .map(|(_, _, at, node)| (at.translation, node.size()))
        .collect();
    assert_eq!(decor.len(), 1, "one `decor` chip is on screen: {decor:?}");
    let (at, size) = decor[0];
    assert!(size.x > 0.0 && size.y > 0.0, "the `decor` chip is laid out: {size:?}");
    let handle = app.world().get_resource::<bevy_dark_factory::surface::Surface>().map(|s| s.image.clone()).unwrap_or_default();
    for action in [PointerAction::Move { delta: Vec2::ZERO }, PointerAction::Press(PointerButton::Primary), PointerAction::Release(PointerButton::Primary)] {
        let location = Location { target: NormalizedRenderTarget::Image(ImageRenderTarget { handle: handle.clone(), scale_factor: 1.0 }), position: at };
        app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
        app.update();
    }
    for _ in 0..4 {
        app.update();
    }
    assert!(kinds(&app).iter().any(|k| k == "decor"), "a click on the `decor` chip holds `decor`: {:?}", kinds(&app));
}
