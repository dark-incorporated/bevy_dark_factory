//! **A fold draws a chevron, and the chevron itself is what folds** (#493).
//!
//! The owner's capture `feedback-1794921635359` reads *"The "v" should collapse the text."* — the PLACE palette's category
//! header drew its disclosure as the ASCII letter `v` (`>` when folded), which reads as a character in a code listing
//! rather than as a control, and gave no sign that the mark is the thing to press.
//!
//! Both halves are checked here against the editor a person sees, not against a constant:
//!
//! 1. the mark a fold draws is one of the two chevrons `docs/design/icon-vocabulary.md` chose, and never the letter;
//! 2. a **press at the mark's own rect** folds the group — driven through `bevy_picking`'s own pointer input, exactly as
//!    a mouse reaches the editor, so the press has to survive the whole pointer chain to the button that owns the fold.
//!
//! # Why the marks are U+25BC and U+25BA
//!
//! They are the two the shipped face, `assets/fonts/FiraMono-Regular.ttf`, actually has. `▾` (U+25BE) and `▸` (U+25B8),
//! which most editors use, are **not** in its `cmap` and would draw a box; `docs/design/icon-vocabulary.md` holds that
//! measurement and the rule it justifies.

use bevy::camera::{ImageRenderTarget, NormalizedRenderTarget};
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::*;
use bevy::ui::ComputedNode;

use bevy_dark_factory::editor::Folded;
use bevy_dark_factory::harness;

use crate::fixtures;

/// **The chevrons a fold draws**, closed then open — the pair `docs/design/icon-vocabulary.md` names.
const CLOSED: char = '\u{25ba}';
const OPEN: char = '\u{25bc}';

/// The letters the marks were, so a regression is named rather than merely caught.
const WAS: [char; 2] = ['v', '>'];

/// Frames for a rebuild to land.
const SETTLE_FRAMES: usize = 10;

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// The Map door, holding two pieces of two kinds, so the PLACE palette draws two category headers.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("fold_chevron_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .descriptor("site/gate", "beta")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    harness::resize_surface(&mut app, 1600, 1200).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

/// **One fold mark**: the character drawn, the button whose `Activate` is the fold, and where the mark stands on screen.
///
/// A mark is read from the layout the editor built — a one-character `Text` that is a chevron or the letter it used to be —
/// so this cannot pass while the code draws something else, and there is no list of call sites to drift from.
struct Mark {
    drawn: char,
    /// The mark's own node, which a press at its rect has to land on.
    entity: Entity,
    /// The mark's parent: the button a press on the mark has to reach.
    control: Entity,
    /// The mark's own centre, in the physical texels `PointerInput` speaks.
    at: Vec2,
}

fn marks(app: &mut App) -> Vec<Mark> {
    let world = app.world_mut();
    let mut query = world.query::<(Entity, &Text, &ComputedNode, &UiGlobalTransform, &ChildOf)>();
    query
        .iter(world)
        .filter_map(|(entity, text, node, at, parent)| {
            let mut chars = text.0.chars();
            let one = chars.next().filter(|_| chars.next().is_none())?;
            if !(one == OPEN || one == CLOSED || WAS.contains(&one)) {
                return None;
            }
            assert!(node.size().x > 0.0 && node.size().y > 0.0, "`{one}` (entity {entity:?}) is laid out with no box, so nothing of it is on screen");
            Some(Mark { drawn: one, entity, control: parent.parent(), at: at.translation })
        })
        .collect()
}

/// One pointer input, on the image the interface is drawn into — the target `bevy_ui`'s picking backend reads.
fn point(app: &mut App, at: Vec2, action: PointerAction) {
    let handle = app
        .world()
        .get_resource::<bevy_dark_factory::surface::Surface>()
        .map(|s| s.image.clone())
        .unwrap_or_default();
    let location = Location {
        target: NormalizedRenderTarget::Image(ImageRenderTarget { handle, scale_factor: 1.0 }),
        position: at,
    };
    app.world_mut().write_message(PointerInput::new(PointerId::Mouse, location, action));
    app.update();
}

/// **The entities the mouse is over** — `bevy_picking`'s own answer, which is what a press targets.
fn hovered_entities(app: &App) -> Vec<Entity> {
    app.world()
        .get_resource::<bevy::picking::hover::HoverMap>()
        .and_then(|map| map.0.get(&PointerId::Mouse))
        .map(|hits| hits.iter().map(|(entity, _)| *entity).collect())
        .unwrap_or_default()
}

fn hovered(app: &App, entity: Entity) -> bool {
    hovered_entities(app).contains(&entity)
}

/// **A fold's mark is a chevron and never the letter it was.**
#[test]
fn a_folds_mark_is_a_chevron_and_never_a_letter() {
    let mut app = editor("shape");
    let drawn: Vec<char> = marks(&mut app).into_iter().map(|m| m.drawn).collect();
    assert!(
        !drawn.is_empty(),
        "the PLACE palette draws a category header's mark; this found none, so a test that measured nothing would pass for \
         the wrong reason"
    );
    let letters: Vec<char> = drawn.iter().copied().filter(|c| WAS.contains(c)).collect();
    assert!(
        letters.is_empty(),
        "a fold still draws the letter it used to: {letters:?} among {drawn:?}. #493 is the owner's note that the `v` \
         should collapse the text — the marks are U+25BC `\u{25bc}` and U+25BA `\u{25ba}`, the two chevrons the shipped \
         face has"
    );
}

/// **A press on the chevron itself folds the group**, at the mark's own rect.
///
/// The press goes through `bevy_picking` — a move, a press and a release at the mark's centre — so it has to survive the
/// whole pointer chain: the mark is a text node, and the fold is the button around it. A mark that is `Pickable::IGNORE`,
/// or a button that does not own its own mark, fails here.
#[test]
fn a_press_on_the_chevron_itself_folds_the_group() {
    let mut app = editor("press");
    let marks = marks(&mut app);
    // **The mark the press is aimed at**, which must be a chevron: the note is one fault in two halves — the mark reads
    // as a letter *and* is not the thing that folds — so this test names the shape it presses.
    let mark = marks
        .iter()
        .find(|m| m.drawn == OPEN || m.drawn == CLOSED)
        .unwrap_or_else(|| {
            panic!(
                "the PLACE palette draws no chevron to press: it draws {:?}. #493's `v` is the letter, and a press at a \
                 letter proves nothing about the chevron",
                marks.iter().map(|m| m.drawn).collect::<Vec<_>>()
            )
        });
    let (at, control) = (mark.at, mark.control);
    assert!(
        app.world().resource::<Folded>().0.is_empty(),
        "a fresh editor has nothing folded, so the press below is the only thing that could fold anything: {:?}",
        app.world().resource::<Folded>().0
    );

    point(&mut app, at, PointerAction::Move { delta: Vec2::ZERO });
    // **The pointer is over the mark itself**, not over the row somewhere else: the hit `bevy_ui`'s backend reports for
    // that point is the mark's own node. This is what "the glyph is the hit target" means as a measurement, and it is
    // the half a mark drawn as a non-pickable child of the row would pass without.
    assert!(
        hovered(&app, mark.entity),
        "a move to the chevron's own rect {at:?} did not hover the chevron's node (entity {entity:?}); the pointer is \
         over something else, so a press there proves nothing about the mark. Hovered: {:?}",
        hovered_entities(&app),
        entity = mark.entity,
    );
    point(&mut app, at, PointerAction::Press(PointerButton::Primary));
    point(&mut app, at, PointerAction::Release(PointerButton::Primary));
    settle(&mut app);

    let folded = app.world().resource::<Folded>().0.clone();
    assert!(
        !folded.is_empty(),
        "a press at the chevron's own rect {at:?} (the mark's parent is {control:?}) folded nothing, so the visible \
         affordance is not the hit target. Folded: {folded:?}"
    );
}
