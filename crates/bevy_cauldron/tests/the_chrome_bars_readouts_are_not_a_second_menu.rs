//! **The chrome bar's readouts are a strip, not a second menu row** (#480,
//! `feedback-1794783606350`).
//!
//! # What the owner saw
//!
//! *"The debug text is overlayed on the editor top menu like file, edit, view--we should make it
//! distinct."* Measured on the capture: the menu row is `y=0..77`, the chrome bar `y=79..139`, and
//! `y=78` is a 1 px rule between them. Both bands carry words in one texture, so the eye reads a verb
//! (`File`) and a readout (`‹ lavender_hills Cmd+0`, `GAME · deadite_caverns`) as the same kind of
//! thing, and only the rule says where the menu ends.
//!
//! # What draws each band
//!
//! - **The menu's titles are gooey's**, not this crate's: `bevy_gooey::menu_title` spawns
//!   `Role::MenuTitleText`, which the skin draws at `skin.text_size` with the body face
//!   (`skin.rs:5554`). This editor never inserts a `GooeySkin` of its own, so gooey's seed —
//!   **13.0 logical px** (`seed.rs:480`) — is what reaches the screen.
//! - **The bar's readouts were `typed(text::BODY)`** (`chrome.rs`, `spawn_chrome_bar`): this crate's
//!   11 px. Two logical px and one ink apart is not a distinction a person sees at arm's length.
//!
//! # What this file holds down
//!
//! The acceptance is *"a headless test asserts the menu titles and the chrome bar's readouts carry
//! different text roles (or different ink) — the fact stated once, where both are spawned"*, so both
//! are measured **on the drawn nodes**: the size each `TextFont` really has and the ink each
//! `TextColor` really is. Not "the source names a different constant" — the sizes a person sees.

use bevy::prelude::*;
use bevy::ui::UiScale;

use bevy_cauldron::chrome::Frame;
use bevy_cauldron::harness;

use crate::fixtures;

/// **The size a menu title is drawn at, in the skin's own px** — measured, not assumed.
///
/// `bevy_gooey` draws a `MenuTitleText` at `skin.text_size` times `skin.text_scale` (`skin.rs:5554`),
/// and the editor replaces gooey's seed skin with its own (`text_size` 11, `text_scale` 0.9), so a
/// menu title is 9.9 px before layout snapping and measured **9.72** on the drawn node. The first
/// draft of this file assumed gooey's seed 13.0 and found no titles at all, which is the mistake this
/// constant records.
const MENU_TITLE_PX: f32 = 9.72;

/// The least step between the menu's size and a readout's that reads as a difference, logical px.
///
/// Measured on the drawn nodes: the bar's readouts at `text::BODY` came out **9.17** against the menu's
/// **9.72**, a step of 0.55 px that no one sees — which is why the owner read the two bands as one.
/// A whole type-scale step (`text::BODY` 11 to `text::HINT` 9) is what puts the readouts under a menu
/// title by about 1.7 px at this text size, so this is the floor for "a difference a person sees".
const LEAST_STEP_PX: f32 = 1.0;

/// How far two sizes may stand apart and still be called one number, logical px.
const SAME_PX: f32 = 0.01;

/// Frames a layout needs to settle.
const SETTLE_FRAMES: usize = 12;

/// A drawn word: what it says, the size it is drawn at in the skin's own px, and its ink.
struct Drawn {
    entity: Entity,
    words: String,
    px: f32,
    ink: Color,
}

fn settle(app: &mut App) {
    for _ in 0..SETTLE_FRAMES {
        app.update();
    }
}

fn editor(name: &str) -> (std::path::PathBuf, App) {
    let root = fixtures::Fixture::new(name).descriptor("floor", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None)
        .unwrap_or_else(|e| panic!("the fixture project must open: {e}"));
    settle(&mut app);
    (root, app)
}

/// A font's size in the skin's own px: `UiScale` multiplies every font size, so dividing by it puts a
/// bar readout and a menu title in the same units.
fn size_in_skin_px(font: &TextFont, ui_scale: f32) -> Option<f32> {
    match font.font_size {
        FontSize::Px(px) => Some(px / ui_scale),
        // A viewport unit is not a type role; nothing in this editor draws a label with one, and a
        // reader of a failure wants to know that rather than see a number converted wrongly.
        _ => None,
    }
}

/// Every drawn word under `root`, root included, **and not inside a hover card**.
fn drawn_rows(world: &World, root: Entity, ui_scale: f32) -> Vec<Drawn> {
    let mut found = Vec::new();
    let mut stack = vec![root];
    while let Some(entity) = stack.pop() {
        if is_a_hover_card(world, entity) {
            continue;
        }
        if let Some(children) = world.get::<Children>(entity) {
            stack.extend(children.iter());
        }
        let (Some(text), Some(font)) = (world.get::<Text>(entity), world.get::<TextFont>(entity)) else { continue };
        if text.0.trim().is_empty() {
            continue;
        }
        let Some(px) = size_in_skin_px(font, ui_scale) else { continue };
        found.push(Drawn {
            entity,
            words: text.0.clone(),
            px,
            ink: world.get::<TextColor>(entity).map(|c| c.0).unwrap_or(Color::WHITE),
        });
    }
    found
}

/// **The most words gooey's decision LED may draw at the menu's size**: its label and its hidden state
/// word. See the exemption in the test.
const MOST_GOOEY_LED_WORDS: usize = 2;

/// Whether this line is the decision chip's own label, which `bevy_gooey` sizes.
///
/// The chip is `bevy_gooey::led`, whose label is drawn at the skin's `text_size` (`led.rs:100`) — the
/// menu titles' size — and `apply_skin` restyles it, so this crate cannot make it quieter without
/// writing over a widget's own component every frame. See `bevy_gooey#110`.
/// Whether this line belongs to the decision chip: its **entity** is under a `DecisionLed`, which is
/// the editor's own marker on that chip (`decision_model.rs`), so the test asks ancestry rather than
/// matching words. A word-matching version let the model's own name (`d3-flash`) through, because a
/// name is whatever the person chose and cannot be listed.
fn is_a_gooey_led(world: &World, entity: Entity) -> bool {
    let mut at = Some(entity);
    while let Some(current) = at {
        if world.get::<bevy_cauldron::decision_model::DecisionLed>(current).is_some() {
            return true;
        }
        at = world.get::<ChildOf>(current).map(|parent| parent.parent());
    }
    false
}

/// Whether this line is drawn inside a `TooltipBox` the bar owns — a hover card, not a readout.
///
/// gooey spawns the card as a child of the widget that carries the `Tooltip` and copies the line into
/// it (`tooltip.rs:181-189`, `:277`), and the card carries `TooltipBox`. Walking from the **box** is
/// exact: a line drawn in the bar's own row has no `TooltipBox` above it.
fn is_a_hover_card(world: &World, entity: Entity) -> bool {
    let mut at = Some(entity);
    while let Some(current) = at {
        if world.get::<bevy_gooey::TooltipBox>(current).is_some() {
            return true;
        }
        at = world.get::<ChildOf>(current).map(|parent| parent.parent());
    }
    false
}

/// **The menu's titles**, found by the size gooey draws them at.
///
/// The words are a `MenuTitleText` child of a `MenuTitle` button, and the button is gooey's; the size
/// is what identifies them without depending on a private gooey role being re-exported.
fn menu_titles(app: &App) -> Vec<Drawn> {
    let world = app.world();
    let ui_scale = world.resource::<UiScale>().0;
    let mut titles = Vec::new();
    let mut query = world.try_query::<(Entity, &bevy_gooey::MenuTitle)>().expect("`MenuTitle` is a registered component");
    for (button, _) in query.iter(world) {
        for d in drawn_rows(world, button, ui_scale) {
            if (d.px - MENU_TITLE_PX).abs() < SAME_PX {
                titles.push(d);
            }
        }
    }
    titles
}

/// **The bar's readouts and the menu's titles are drawn at different sizes, in different inks.**
#[test]
fn the_chrome_bars_readouts_are_a_quieter_role_than_the_menu_titles() {
    let (_root, app) = editor(&format!("chrome_bar_distinct_{}", std::process::id()));
    let ui_scale = app.world().resource::<UiScale>().0;
    let titles = menu_titles(&app);
    assert!(
        !titles.is_empty(),
        "the editor draws a menu bar of titles; none was drawn at gooey's {MENU_TITLE_PX} px"
    );
    let bar = app.world().resource::<Frame>().chrome_bar;
    // **Only the bar's own row of readouts, not the hover cards gooey hangs under it.** A `Tooltip`
    // is a component and gooey spawns its box as a child (`tooltip.rs:189`), copying the line into it
    // on open (`:277`); a hidden card a person sees only under the pointer is not a band readout, and
    // counting it would fail this test for a caption that is not on screen at all.
    let readouts = drawn_rows(app.world(), bar, ui_scale);
    assert!(!readouts.is_empty(), "the chrome bar draws its readouts; none was found");

    // **1. Every editor-owned readout is smaller than the menu's titles by a step a person can see.**
    //
    // **The decision chip is exempt, by name and counted.** `bevy_gooey::led` draws its label at the
    // skin's `text_size` (`led.rs:100`), which is the same size the menu titles are drawn at, and
    // `apply_skin` restyles `Role::Label` on every state change — so a host that typed the label
    // instead would be overwriting a widget's own component every frame, the same two-writers defect
    // that made the divider's thickness a `bevy_gooey` issue (#108). Measured: a system that set the
    // label's role after `GooeySystems::Style` left it at 9.7 px anyway. Filed as `bevy_gooey#110`.
    let world = app.world();
    let (loud, gooey_led): (Vec<&Drawn>, Vec<&Drawn>) =
        readouts.iter().partition(|r| !is_a_gooey_led(world, r.entity));
    assert!(
        gooey_led.len() <= MOST_GOOEY_LED_WORDS,
        "{} words are drawn at the menu's size by gooey's LED, more than the {MOST_GOOEY_LED_WORDS} that chip \
         has — the exemption for `bevy_gooey#110` now covers more than it was written for: {:?}",
        gooey_led.len(),
        gooey_led.iter().map(|r| &r.words).collect::<Vec<_>>()
    );
    let mut too_loud = Vec::new();
    for readout in &loud {
        if readout.px > MENU_TITLE_PX - LEAST_STEP_PX {
            too_loud.push(format!(
                "`{}` is drawn at {:.1} px against the menu's {MENU_TITLE_PX:.1}",
                readout.words, readout.px
            ));
        }
    }
    assert!(
        too_loud.is_empty(),
        "the bar's readouts are drawn at the menu's size, so the two bands are one texture:\n{}",
        too_loud.join("\n")
    );

    // **2. A different role or a different ink** — the acceptance's own alternative. Both are checked
    // on the drawn nodes: the role as the size it resolves to, the ink as the colour already written.
    let role_by_size = loud.iter().any(|r| (r.px - MENU_TITLE_PX).abs() > SAME_PX);
    let quiet_ink = loud.iter().any(|r| titles.iter().all(|t| t.ink != r.ink));
    assert!(
        role_by_size || quiet_ink,
        "the bar's readouts share the menu's size and its ink, so nothing but the rule at y=78 says \
         where the menu ends.\n  readouts: {:?}\n  titles: {:?}",
        readouts.iter().map(|r| (&r.words, r.px, r.ink)).collect::<Vec<_>>(),
        titles.iter().map(|t| (&t.words, t.px, t.ink)).collect::<Vec<_>>()
    );

    // **3. And the rule the issue says must stay is still there.** It is the bar's own bottom edge
    // (`spawn_frame`: `bar(bevy_gooey::Sides { bottom: Some(1.0), .. })`), which is the line a person
    // reads at `y=78` on the capture — the menu row's last line, drawn by the band under it.
    let edge = app
        .world()
        .get::<Node>(bar)
        .map(|node| node.border.bottom)
        .unwrap_or(Val::Px(0.0));
    assert!(
        !matches!(edge, Val::Px(0.0)),
        "the chrome bar has no bottom edge, so the rule between the menu and the bar ({edge:?}) is gone"
    );
}
