//! **The PLACE list says what makes a row ready, and its cells line up** (#486).
//!
//! Found in a person's capture of the Map door (`feedback-1794784623926`, DarkFactory 0.6.8, mac_air, 2560 x 1406 at `ui_scale` 2.4,
//! map `deadite_caverns`, the project's library empty): *"What makes these ready to use? And this layout feels a bit jumbled"*, with a
//! pick on the PLACE list's own column.
//!
//! Two defects in one note, and they are two:
//!
//! 1. **"Ready to use" was asserted and never explained.** A row is armed — the one a click would place — and the row is marked by an
//!    accent rail and a fill, with no line anywhere in the panel saying that this is what the mark means. In a project whose library is
//!    empty every row is in the same state and the state has no name, so "is this row chosen, or is it the only one" cannot be answered
//!    from the screen.
//! 2. **The list was jumbled.** A row's triangle count followed its name directly, so the count of a long-named piece stood at a
//!    different x from every other row's — nothing for the eye to run down, which is what "a person can scan it" costs.
//!
//! What is held down here, in a headless editor with the capture's library shapes:
//!
//! - the panel states its rule of readiness exactly once, in the words `chrome` carries for it, above the list and inside the panel's
//!   own content box;
//! - every row's name starts in one column, every row's count ends in one column, and every row's armed mark stands in one column, so
//!   the list reads down as columns;
//! - the cells of a row sit on **one baseline**, measured from the glyphs that are drawn rather than from the boxes around them: a mark
//!   at one type size and a name at another are centred on the same line only by luck, and their baselines are what a reader sees;
//! - a name long enough to wrap grows its own row and no other.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform};

use bevy_dark_factory::chrome::{PanelWidth, Side};
use bevy_dark_factory::editor::{PaletteArmedWords, PaletteCost, PaletteList, PaletteName, PaletteRow};
use bevy_dark_factory::harness;

use crate::fixtures;

/// A name long enough to wrap three times in the panel's name column at the shipped text sizes, with no space in it: the case the
/// break-at-a-letter rule exists for.
const LONG_ID: &str = "a_very_long_piece_name_that_runs_on_and_on_and_on";

/// Frames a layout needs to settle.
const SETTLE_FRAMES: usize = 12;

/// How far two measured edges may stand apart and still be one column, physical px: layout rounds.
const SLACK_PX: f32 = 1.0;

/// How far two **baselines** may stand apart and still read as one line, physical px.
///
/// Larger than [`SLACK_PX`] because two type sizes are legitimately on one line: a 10 px label and
/// an 11 px body have half-leadings that differ by ~2 px, and a reader runs their eye along the
/// words, not along the box. The defect this catches is a cell placed on the *wrong line of a
/// wrapped row*, which is a whole `BODY_LINE_H` (13.2 px) away, not 2.
const BASELINE_SLACK_PX: f32 = 2.5;

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// The editor on a library of one kind: a long-named piece among short ones, so both a wrapping row and its neighbours are drawn.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("place_columns_{tag}_{}", std::process::id()))
        .descriptor(LONG_ID, "alpha")
        .descriptor("crate", "alpha")
        .descriptor("barrel", "alpha")
        .descriptor("wall_low", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    settle(&mut app);
    app
}

/// The PLACE panel: the right-hand panel that holds the map's palette.
fn panel_of(app: &mut App) -> Entity {
    let world = app.world_mut();
    let list = world.query_filtered::<Entity, With<PaletteList>>().single(world).unwrap_or_else(|e| panic!("one palette list: {e}"));
    let mut at = list;
    loop {
        if world.get::<PanelWidth>(at).is_some_and(|p| p.side == Side::Right) {
            return at;
        }
        at = world.get::<ChildOf>(at).unwrap_or_else(|| panic!("the palette is inside a right-hand panel")).parent();
    }
}

/// Every entity under `root`, `root` included.
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

/// A node's border-box rect, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **Where a text's first line sits, from the glyphs it actually draws**: the bottom edge of its first run, in physical px on the
/// screen. That is the line a reader's eye runs along, and it is the one thing a box's centre does not say — two type sizes centred in
/// one row have their glyphs at two heights whenever their metrics differ.
fn first_baseline(world: &World, entity: Entity) -> Option<f32> {
    let info = world.get::<TextLayoutInfo>(entity)?;
    let run = info.run_geometry.first()?;
    let rect = border_box(world, entity)?;
    let inset = world.get::<ComputedNode>(entity).map(ComputedNode::content_inset).unwrap_or_default();
    // The layout is measured from the node's content box, which starts inside its border and padding.
    Some(rect.min.y + inset.min_inset.y + run.bounds.max.y)
}

/// A cell as it is drawn: its words, its box, and the first line's bottom edge **when it draws a glyph at all** — the armed mark's
/// column is blank on most rows, and a blank text has no run to measure.
struct Cell {
    words: String,
    box_: Rect,
    baseline: Option<f32>,
}

/// What one palette row drew: its rect and the three cells of its line.
struct Row {
    ix: usize,
    rect: Rect,
    mark: Cell,
    name: Cell,
    count: Cell,
}

/// The palette rows, in drawn order, with the rects and first baselines of the three cells a row carries.
///
/// **By marker, not by the order a walk happens to visit them.** A cell is what the palette's own components say it is
/// ([`PaletteArmedWords`], [`PaletteName`], [`PaletteCost`]), so a row that drew its cells in another order — or drew a fourth thing
/// beside them — is measured as what it is rather than silently re-labelled by where the walk reached it first.
fn rows(app: &mut App) -> Vec<Row> {
    let world = app.world_mut();
    let mut found: Vec<(usize, Entity)> = world
        .query::<(&PaletteRow, Entity)>()
        .iter(world)
        .map(|(row, entity)| (row.0, entity))
        .collect();
    found.sort_by_key(|(ix, _)| *ix);
    let mut out = Vec::new();
    for (ix, entity) in found {
        let Some(rect) = border_box(world, entity) else { continue };
        let mut mark = None;
        let mut name = None;
        let mut count = None;
        for node in under(world, entity) {
            let Some(text) = world.get::<Text>(node).map(|t| t.0.clone()) else { continue };
            let Some(box_) = border_box(world, node) else { continue };
            let cell = Cell { words: text, box_, baseline: first_baseline(world, node) };
            if world.get::<PaletteArmedWords>(node).is_some() {
                // The column's own box is what lines up down the list; the words inside it are what a baseline is measured on.
                let column = world.get::<ChildOf>(node).map(ChildOf::parent).and_then(|c| border_box(world, c));
                let (words, baseline) = (cell.words, cell.baseline);
                mark = Some(Cell { words, box_: column.unwrap_or(cell.box_), baseline });
            } else if world.get::<PaletteName>(node).is_some() {
                name = Some(cell);
            } else if world.get::<PaletteCost>(node).is_some() {
                count = Some(cell);
            }
        }
        let (Some(mark), Some(name), Some(count)) = (mark, name, count) else {
            panic!("row {ix} does not draw an armed-mark cell, a name and a cost; the palette spawns all three");
        };
        out.push(Row { ix, rect, mark, name, count });
    }
    out
}

/// The widest and narrowest of a list of measurements, as one number: how far they stray from being one column.
fn spread(values: &[f32]) -> f32 {
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for v in values {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    hi - lo
}

/// **The panel states what arms a row, once, above the list and inside the panel.**
#[test]
fn the_place_list_states_what_arms_a_row() {
    let mut app = editor("rule");
    let panel = panel_of(&mut app);
    // **The list's own top edge**, read before the panel is, so the two borrows do not overlap. `UiGlobalTransform` is a centre, UI y grows
    // downwards, and the rule has to end above the highest row.
    let list_top = {
        let world = app.world_mut();
        let mut tops = world.query_filtered::<(&ComputedNode, &UiGlobalTransform), With<PaletteRow>>();
        tops.iter(world)
            .map(|(node, at)| at.translation.y - node.size().y * 0.5)
            .fold(f32::INFINITY, f32::min)
    };
    let world = app.world();
    let mut said: Vec<(Entity, String, Rect)> = Vec::new();
    for entity in under(world, panel) {
        let Some(rule) = world.get::<bevy_dark_factory::chrome::PaletteRule>(entity) else { continue };
        let Some(text) = world.get::<Text>(entity) else { panic!("the rule is drawn, so it has words") };
        let Some(rect) = border_box(world, entity) else { panic!("the rule is laid out") };
        said.push((entity, text.0.clone(), rect));
        assert_eq!(rule.0, text.0, "the rule's own words and what it draws are one value, so a test can read either");
    }
    assert_eq!(said.len(), 1, "the panel states its rule of readiness exactly once; it drew {} of them", said.len());
    let (_, words, rect) = &said[0];
    for word in ["arm", "click", "map"] {
        assert!(
            words.to_lowercase().contains(word),
            "the rule a person can act on names the {word:?} it is about, and the panel says {words:?}"
        );
    }
    let panel_box = border_box(world, panel).unwrap_or_else(|| panic!("the panel is laid out"));
    let inset = world.get::<ComputedNode>(panel).map(ComputedNode::content_inset).unwrap_or_default();
    assert!(
        rect.max.x <= panel_box.max.x - inset.max_inset.x + SLACK_PX,
        "the rule's own box ends at {:.1}, past the panel's content edge {:.1}: it is not readable",
        rect.max.x,
        panel_box.max.x - inset.max_inset.x
    );
    // **Above the list it is about**, not under it or beside it: a caption after its list is read after the thing it explains.
    assert!(list_top.is_finite(), "the palette drew rows to be above");
    assert!(
        rect.max.y <= list_top + SLACK_PX,
        "the rule stands above the list's highest row (the rule ends at {:.1}, the list starts at {list_top:.1})",
        rect.max.y
    );
}

/// **The list reads down as columns, its cells share a baseline, and a name that wraps grows only its own row.**
#[test]
fn the_place_lists_cells_line_up_and_a_long_name_grows_only_its_own_row() {
    let mut app = editor("columns");
    let drawn = rows(&mut app);
    assert!(drawn.len() >= 4, "the fixture's four pieces are drawn as rows, and this list has {}", drawn.len());

    // **One column each.** A count that follows its own name's width is the defect: the eye has nothing to run down, which is what
    // "jumbled" meant. The mark's column is what makes "this row is armed" a shape rather than an accent fill, so it lines up too.
    let name_lefts: Vec<f32> = drawn.iter().map(|r| r.name.box_.min.x).collect();
    let count_rights: Vec<f32> = drawn.iter().map(|r| r.count.box_.max.x).collect();
    let mark_lefts: Vec<f32> = drawn.iter().map(|r| r.mark.box_.min.x).collect();
    assert!(
        spread(&name_lefts) <= SLACK_PX,
        "every row's name starts in one column: the lefts are {name_lefts:?}, a spread of {:.1} px",
        spread(&name_lefts)
    );
    assert!(
        spread(&count_rights) <= SLACK_PX,
        "every row's count ends in one column: the rights are {count_rights:?}, a spread of {:.1} px",
        spread(&count_rights)
    );
    assert!(
        spread(&mark_lefts) <= SLACK_PX,
        "every row's armed mark stands in one column: the lefts are {mark_lefts:?}, a spread of {:.1} px",
        spread(&mark_lefts)
    );

    // **The mark is a shape, and it says whether this row is the one a click would place.** A rail and a fill say it in colour
    // alone, which a person who cannot separate the hues does not read.
    let armed: Vec<usize> = drawn
        .iter()
        .filter(|r| r.mark.words == bevy_dark_factory::chrome::ARMED_MARK)
        .map(|r| r.ix)
        .collect();
    assert_eq!(
        armed.len(),
        1,
        "exactly one row wears the armed mark, and {:?} do: {:?}",
        armed,
        drawn.iter().map(|r| (&r.mark.words, r.ix)).collect::<Vec<_>>()
    );
    for row in &drawn {
        assert!(
            row.mark.box_.width() > 0.0,
            "row {}'s mark column is laid out even when it draws nothing, so the marks line up down the list",
            row.ix
        );
        assert!(
            row.mark.box_.max.x <= row.name.box_.min.x + SLACK_PX,
            "row {}'s mark ends at {:.1}, before its name starts at {:.1}",
            row.ix,
            row.mark.box_.max.x,
            row.name.box_.min.x
        );
    }

    // **One baseline across a row**, measured on the glyphs each cell actually draws. A cell's box centre is not it: the mark is a
    // label's size and the name a body's, so centring their boxes puts their words on two lines whenever the two faces' metrics differ,
    // and it is the words a reader runs their eye along. A cell that draws nothing (an unarmed row's mark column) has no line to share.
    for row in &drawn {
        let drawn_cells: Vec<(&str, &Cell)> = [("mark", &row.mark), ("name", &row.name), ("count", &row.count)]
            .into_iter()
            .filter(|(_, cell)| cell.baseline.is_some())
            .collect();
        let Some((_, first)) = drawn_cells.first() else { continue };
        let first = first.baseline.unwrap_or_default();
        for (what, cell) in &drawn_cells[1..] {
            let at = cell.baseline.unwrap_or_default();
            let off = (first - at).abs();
            assert!(
                off <= BASELINE_SLACK_PX,
                "row {}'s first cell and its {what} do not sit on one baseline: their first lines are {off:.1} px apart ({first:.1} vs {at:.1})",
                row.ix
            );
        }
    }

    // **A long name grows its own row, and no other.**
    let long = drawn
        .iter()
        .find(|r| r.name.words == blueprint_model::naming::display_name(LONG_ID))
        .unwrap_or_else(|| panic!("the long piece is drawn: {:?}", drawn.iter().map(|r| r.name.words.as_str()).collect::<Vec<_>>()));
    let others: Vec<&Row> = drawn.iter().filter(|r| r.ix != long.ix).collect();
    let short: Vec<f32> = others.iter().map(|r| r.rect.height()).collect();
    assert!(
        spread(&short) <= SLACK_PX,
        "the short-named rows are one height: {short:?}, so the height of the wrapped one is the only difference"
    );
    assert!(
        long.rect.height() > short[0] + SLACK_PX,
        "the name {:?} wraps in its own row, so that row is taller than the {:.1} px of a one-line row; it is {:.1} px",
        long.name.words,
        short[0],
        long.rect.height()
    );
    assert!(
        long.name.box_.height() > short[0],
        "and the wrapped name itself is taller than a row: {:.1} px of glyph box against a {:.1} px row",
        long.name.box_.height(),
        short[0]
    );
}
