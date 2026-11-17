//! **The Agent panel's switch is inside the scroll viewport it is drawn in** (#481).
//!
//! Found in a person's capture of the Agent door (`feedback-1794783669349`, DarkFactory 0.6.8, mac_air, 2560 x 1406 at `ui_scale` 2.4,
//! map `deadite_caverns`): *"The widgets are breaking out of the div here."*, with a pick on a `Toggle`/`ToggleTrack`. The frame
//! measured the panel's content edge at `x≈583`, the scroll viewport's content ending at `x≈673`, and the switch's track drawn from
//! `x=631` to `x=728` — its right end, knob and track body alike, **55 px past the viewport's edge**, in the scrollbar's gutter and
//! over the panel's border. The row's own label ended at `x=629`, so the row was laid out as if the panel were wider than it is.
//!
//! # Why this is a file of its own
//!
//! `no_panel_draws_a_control_past_its_own_content_box.rs` holds the four panels of this family to the **panel's** content box, and
//! `the_left_panes_contents_stay_inside_it.rs` holds the left pane's controls to the **viewport's** content box — but both of them
//! stop their walk at a `Role::ScrollViewport`, on purpose and for a good reason: a viewport's rows are laid out taller than it so
//! that it can scroll, and a row below the fold is meant to be out of sight. In the Agent panel the thing that overflows **is a
//! row of that viewport** — `AgentPane` *is* the viewport, and the block holding `Pause agent writes` is its first child. So the
//! one entity the capture points at is the one entity neither walk reaches, and `chrome::let_a_toggle_label_break` — the fix that
//! makes a `Toggle` take the room its row leaves rather than its intrinsic width — is asserted by no test at all.
//!
//! # What is held here
//!
//! The toggle is measured *where it is drawn*: the viewport's children are walked (that is the point), each node's **drawn ink** is
//! taken from its glyphs for a text and from its box otherwise, and every one of them is held to the viewport's content box less
//! [`bevy_dark_factory::chrome::SCROLL_GUTTER`] — the rail's own band, which is the edge the capture's 55 px crossed. It is measured at
//! the capture's window, at the issue's two interface scales, and with the divider at its leftmost allowed position, which is what
//! narrows the pane to the width the capture shows.

use bevy::prelude::*;
use bevy::text::TextLayoutInfo;
use bevy::ui::{ComputedNode, UiGlobalTransform, UiScale};

use bevy_dark_factory::chrome;
use bevy_dark_factory::agent_view::AgentPane;
use bevy_dark_factory::harness;
use bevy_dark_factory::tiles::Mode;

use crate::fixtures;

/// The capture's window, physical px (`feedback-1794783669349`: 2560 x 1406).
const WINDOW: (u32, u32) = (2560, 1406);

/// The interface scales the issue names: the editor's own at a display scale factor of 1, and the capture's 2.4 over the editor's
/// 1.2 baseline, which is the 2.0 the capture was taken at.
const INTERFACE_SCALES: [f32; 2] = [1.0, 2.0];

/// **The divider at its leftmost allowed position**, which is the capture's case: `0.0` is "the first pane as small as it may be"
/// (`bevy_gooey::SplitFraction`'s own doc), so the pane is as narrow as the editor ever draws it and the switch has the least room.
const LEFTMOST: f32 = 0.0;

/// Frames a layout needs to settle after the width or the scale moves.
const SETTLE_FRAMES: usize = 12;

/// How far a measured edge may stand past the one it is held to, physical px: layout rounds.
const SLACK_PX: f32 = 1.0;

/// How many spills the message reports before it stops being a list and becomes a wall.
const REPORT_LIMIT: usize = 10;

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

/// A node's border-box rect, physical px.
fn border_box(world: &World, entity: Entity) -> Option<Rect> {
    let computed = world.get::<ComputedNode>(entity)?;
    let at = world.get::<UiGlobalTransform>(entity)?.translation;
    Some(Rect::from_center_size(at, computed.size()))
}

/// **The rect a node draws in**: what its glyphs fill, from where its content box starts; or its own box for a node that draws no
/// words.
///
/// A text is measured from its glyphs because a node's box can be narrower than the words drawn in it — that is half of what this
/// family of captures is: `row_value`'s box was inside the pane and its words were not. A node with no `Text` (a track, a knob, a
/// button's box) is measured as its box, which is what the capture measured the switch by.
fn draws_in(world: &World, entity: Entity) -> Option<Rect> {
    let rect = border_box(world, entity)?;
    let Some(text) = world.get::<Text>(entity) else { return Some(rect) };
    if text.0.trim().is_empty() {
        return None;
    }
    let info = world.get::<TextLayoutInfo>(entity)?;
    if info.size.x <= 0.0 || info.size.y <= 0.0 {
        return None;
    }
    let inset = world.get::<ComputedNode>(entity).map(ComputedNode::content_inset).unwrap_or_default();
    let start = rect.min + Vec2::new(inset.min_inset.x, inset.min_inset.y);
    Some(rect.union(Rect::from_corners(start, start + info.size)))
}

/// Every entity under `root`, `root` included — **and deliberately into a scroll viewport's children**, which is the one walk the
/// other two files of this family do not take.
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

/// What a node is, for a message a person can act on: its role, its words, or where it is.
fn name_of(world: &World, entity: Entity) -> String {
    if let Some(role) = world.get::<bevy_gooey::Role>(entity) {
        return format!("a {role:?}");
    }
    match world.get::<Text>(entity) {
        Some(text) if !text.0.trim().is_empty() => format!("`{}`", text.0),
        _ => format!("{entity:?}"),
    }
}

/// The editor on a project with rows in the Agent panel, opened on the Agent door.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("agent_switch_{tag}_{}", std::process::id()))
        .descriptor("crate", "alpha")
        .build("m");
    let mut app = harness::build_headless_at(&root, "m", None, Mode::Agent)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    harness::resize_surface(&mut app, WINDOW.0, WINDOW.1).unwrap_or_else(|e| panic!("{e}"));
    settle(&mut app);
    app
}

/// The shell's one split.
fn split_of(app: &mut App) -> Entity {
    let world = app.world_mut();
    world
        .query_filtered::<Entity, With<chrome::ShellSplit>>()
        .single(world)
        .unwrap_or_else(|e| panic!("the shell has one split: {e}"))
}

/// Put the divider where a drag would, then let the layout settle.
fn move_the_divider(app: &mut App, fraction: f32) {
    let split = split_of(app);
    app.world_mut()
        .get_mut::<bevy_gooey::SplitFraction>(split)
        .unwrap_or_else(|| panic!("the shell's split carries a fraction"))
        .0 = fraction;
    settle(app);
}

/// **The Agent panel's viewport**, found without a mutable world: [`World::query_filtered`] needs `&mut World`, and the callers of
/// [`spills`] hold the layout they are measuring. `AgentPane` *is* the viewport (`chrome::scroll_list` is what spawns it), so a
/// component census over the entities is the same question and leaves the borrow alone.
fn agent_viewport(world: &World) -> Option<Entity> {
    world.iter_entities().map(|entity| entity.id()).find(|entity| world.get::<AgentPane>(*entity).is_some())
}

/// **Every node the Agent panel's viewport draws words in, and the switch it draws**, ends inside the viewport's content box, as
/// sentences naming what did not.
///
/// The viewport is [`AgentPane`] itself: `chrome::scroll_list` spawns gooey's scroll viewport and that marker is what the pane is.
///
/// # What is measured, and why not everything
///
/// Two things only, and both for a reason a reader can check:
///
/// - **a text**, from its glyphs: a node's box can be narrower than the words drawn in it, and words past the edge are what the
///   person saw;
/// - **the switch's own nodes** ([`bevy_gooey::Role::Toggle`], `ToggleTrack`, `ToggleKnob`): the capture measured the *track* at
///   `x=728`, and a track is a box with no words in it. These are named rather than "every node" on purpose — a container inside a
///   scroll viewport is laid out against the viewport's padding box by gooey, so holding containers to the same line reports the
///   scroll's own gutters as overflow instead of the control the note pointed at.
fn spills(app: &App, tag: &str) -> Vec<String> {
    let world = app.world();
    let Some(viewport) = agent_viewport(world) else {
        return vec![format!("{tag}: the Agent panel has no viewport")];
    };
    let Some(box_of) = border_box(world, viewport) else {
        return vec![format!("{tag}: the Agent panel's viewport is not laid out")];
    };
    // **The rail's band is what the switch crossed**, and it is stated rather than read back from the viewport's inset: gooey
    // applies [`chrome::SCROLL_GUTTER`] as padding on its skin pass, so the inset a test reads depends on which pass ran last
    // (`the_left_panes_contents_stay_inside_it.rs` states the same number for the same reason). This is the line #481's acceptance
    // names — *"the scroll viewport's content box"* — and it is the one the capture's 55 px went past.
    let right = box_of.max.x - chrome::SCROLL_GUTTER;
    let left = box_of.min.x;
    let mut refused = Vec::new();
    // **The viewport's subtree, and not the viewport itself**: the viewport is the box everything else is held to, so measuring it
    // against its own gutter says it is one gutter past itself.
    for entity in under(world, viewport).into_iter().filter(|e| *e != viewport) {
        let switch = matches!(
            world.get::<bevy_gooey::Role>(entity),
            Some(bevy_gooey::Role::Toggle | bevy_gooey::Role::ToggleTrack | bevy_gooey::Role::ToggleKnob)
        );
        let words = world.get::<Text>(entity).is_some_and(|t| !t.0.trim().is_empty());
        if !switch && !words {
            continue;
        }
        let Some(rect) = draws_in(world, entity) else { continue };
        if rect.max.x > right + SLACK_PX {
            refused.push(format!(
                "{tag}: {} ends at {:.1}, {:.1} px past the viewport's content edge {right:.1}",
                name_of(world, entity),
                rect.max.x,
                rect.max.x - right
            ));
        }
        if rect.min.x < left - SLACK_PX {
            refused.push(format!(
                "{tag}: {} starts at {:.1}, {:.1} px before the viewport's content edge {left:.1}",
                name_of(world, entity),
                rect.min.x,
                left - rect.min.x
            ));
        }
        if refused.len() > REPORT_LIMIT {
            refused.push(format!("{tag}: …and more; the first {REPORT_LIMIT} are above"));
            return refused;
        }
    }
    refused
}

/// **The switch is drawn, inside the viewport, with the divider at its leftmost position and at both interface scales.**
///
/// A test whose switch was never found, or whose pane never narrowed, would pass on a broken layout: the three assertions before the
/// spill check are what make the measurement be of the thing the capture pointed at.
#[test]
fn a_switch_inside_the_agent_panes_viewport_stays_inside_it() {
    let mut app = editor("fit");
    let mut refused = Vec::new();
    let mut seen_the_switch_at: Vec<(f32, f32)> = Vec::new();
    for scale in INTERFACE_SCALES {
        app.world_mut().resource_mut::<UiScale>().0 = scale;
        move_the_divider(&mut app, LEFTMOST);
        let world = app.world();
        let viewport = agent_viewport(world).unwrap_or_else(|| panic!("the Agent panel has one viewport at interface scale {scale}"));
        // **The switch this capture points at is there**, by the role gooey gives a toggle and by the words it carries: a test
        // that measured a panel with no toggle in it would prove nothing about the toggle.
        let toggles: Vec<Entity> = under(world, viewport)
            .into_iter()
            .filter(|e| world.get::<bevy_gooey::Role>(*e) == Some(&bevy_gooey::Role::Toggle))
            .collect();
        assert!(
            !toggles.is_empty(),
            "the Agent panel draws its `Pause agent writes` switch at interface scale {scale}, and this found none"
        );
        let pause = toggles
            .iter()
            .copied()
            .find(|e| under(world, *e).into_iter().any(|n| world.get::<Text>(n).is_some_and(|t| t.0.contains("Pause agent writes"))))
            .unwrap_or_else(|| panic!("the Agent panel's block carries `Pause agent writes` at interface scale {scale}"));
        // **The track is drawn and is the right-hand end of that row** — the node the capture measured at `x=728`.
        let track = under(world, pause)
            .into_iter()
            .find(|e| world.get::<bevy_gooey::Role>(*e) == Some(&bevy_gooey::Role::ToggleTrack))
            .unwrap_or_else(|| panic!("a toggle draws a track at interface scale {scale}"));
        let track_box = border_box(world, track).unwrap_or_else(|| panic!("the track is laid out at interface scale {scale}"));
        seen_the_switch_at.push((scale, track_box.max.x));
        refused.extend(spills(&app, &format!("[interface scale {scale}, divider at its leftmost]")));
    }
    // **The control is the one on the right of its row**, so the spill check above is of the thing the capture named: a track at the
    // row's far end is what a `SpaceBetween` layout puts there, and a track at the row's start would mean this measured something
    // else. The toggle's own box is the row.
    let world = app.world();
    let viewport = agent_viewport(world).unwrap_or_else(|| panic!("one viewport"));
    let pause = under(world, viewport)
        .into_iter()
        .filter(|e| world.get::<bevy_gooey::Role>(*e) == Some(&bevy_gooey::Role::Toggle))
        .find(|e| under(world, *e).into_iter().any(|n| world.get::<Text>(n).is_some_and(|t| t.0.contains("Pause agent writes"))))
        .unwrap_or_else(|| panic!("the `Pause agent writes` switch is drawn"));
    let row = border_box(world, pause).unwrap_or_else(|| panic!("the switch's row is laid out"));
    let track = under(world, pause)
        .into_iter()
        .find(|e| world.get::<bevy_gooey::Role>(*e) == Some(&bevy_gooey::Role::ToggleTrack))
        .unwrap_or_else(|| panic!("a toggle draws a track"));
    let track_box = border_box(world, track).unwrap_or_else(|| panic!("the track is laid out"));
    assert!(
        track_box.min.x > row.min.x + SLACK_PX,
        "the track stands to the right of its row's label, which is what makes it the end that overflows: the row starts at {:.1} and \
         the track at {:.1}",
        row.min.x,
        track_box.min.x
    );
    assert!(seen_the_switch_at.len() == INTERFACE_SCALES.len(), "the switch was measured at each interface scale");
    assert!(refused.is_empty(), "the Agent panel draws a control outside its scroll viewport:\n{}", refused.join("\n"));
}
