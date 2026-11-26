//! **The density a skin declares reaches a dock panel's padding** (#650).
//!
//! # What was wrong
//!
//! `chrome::panel_root` spawns every dock panel in the editor — sixteen call sites in eleven files —
//! as a hand-built box with a `.chrome-panel` class. The class rule
//! (`src/style/editor.yaml:132`) states `padding: !Step 2`, a literal 12 px off the skin's spacing
//! scale, and gooey applies a node's `Spacing` **after** the role's own arm. So the panel's inset was
//! the class's 12 px at every density: `GooeySkin::pad()` is what a `Role::Panel` pads by, and
//! `pad()` is `padding` for `Density::Comfortable` and half of it for `Density::Compact`
//! (`bevy_gooey` `src/skin.rs:1221-1226`). The editor's sheet declares `density: Compact`, so
//! `pad()` was 6 px and nothing a person could see was 6 px: two answers to one question, and the
//! skin's was the one ignored.
//!
//! # What this file holds down
//!
//! Three things, each of which fails on the code before this change:
//!
//! - every dock panel carries `Role::Panel`, so the skin styles it and a reader can enter it;
//! - the shipped sheet's panels are still drawn 12 px in, so adoption is not a redesign;
//! - with a person's layer that drops the rule's stated override, the drawn inset becomes
//!   `skin.pad()` — which is the proof that the role's padding is what is underneath. Before the
//!   change there was no role padding to fall back to, and the panel drew **0 px**.
//!
//! **The padding is read from the node's own `Node`**, not recomputed from `skin.pad()`: a test that
//! recomputed it would agree with a bug in the application order. `apply_skin` writes `Node::padding`
//! on the entity, and that is what Bevy lays out.

use bevy::prelude::*;
use bevy_dark_factory::chrome::PanelWidth;
use bevy_gooey::{Density, GooeySkin, Role};

use crate::fixtures;
use crate::settle_support;

/// A dock panel's name and its drawn padding, left and top, logical px.
type PanelPadding = (String, f32, f32);

/// Every dock panel in the editor with the padding the layout actually has, cheapest first.
fn panel_paddings(app: &mut App) -> Vec<PanelPadding> {
    let world = app.world_mut();
    let mut panels = world.query_filtered::<(Entity, &Node), With<PanelWidth>>();
    let mut out: Vec<PanelPadding> = panels
        .iter(world)
        .map(|(entity, node)| {
            let name = world
                .get::<AccessibleLabel>(entity)
                .map_or_else(|| format!("{entity:?}"), |label| label.0.clone());
            (name, px(node.padding.left), px(node.padding.top))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

fn px(value: Val) -> f32 {
    match value {
        Val::Px(px) => px,
        _ => 0.0,
    }
}

/// An editor on the shipped skin, laid out.
fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("panel_inset_{tag}_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    app
}

/// **Every dock panel carries gooey's `Role::Panel`.** The role is what makes the box a region in the
/// accessibility tree a reader can enter, and what derives its fill from `skin.surface`; without it
/// gooey's `apply_skin` never styles the panel root at all.
#[test]
fn every_dock_panel_carries_gooeys_panel_role() {
    let mut app = editor("role");
    let world = app.world_mut();
    let panels: Vec<Entity> = world.query_filtered::<Entity, With<PanelWidth>>().iter(world).collect();
    assert!(!panels.is_empty(), "the editor draws dock panels");
    let mut without = Vec::new();
    for panel in panels {
        let name = world
            .get::<AccessibleLabel>(panel)
            .map_or_else(|| format!("{panel:?}"), |label| label.0.clone());
        if world.get::<Role>(panel) != Some(&Role::Panel) {
            without.push(format!("{name} ({:?})", world.get::<Role>(panel)));
        }
    }
    assert!(
        without.is_empty(),
        "{} of {} dock panels do not carry `Role::Panel`, so the skin never styles their inset, \
         corner or fill: {without:?}",
        without.len(),
        panels.len()
    );
}

/// **The shipped density's inset is unchanged by the role** — adoption, not a redesign.
///
/// `Role::Panel` pads by `skin.pad()`, and the shipped sheet is `Density::Compact`, so the role's own
/// inset is half the spacing scale's step 2. The editor's panels have always been drawn 12 px in, and
/// they still are: the rule's stated override is what keeps that, and this reads it off the two drawn
/// panels the acceptance names rather than out of the sheet.
#[test]
fn the_shipped_densitys_panel_inset_is_the_twelve_pixels_it_always_was() {
    let mut app = editor("shipped");
    let drawn = panel_paddings(&mut app);
    assert!(!drawn.is_empty(), "the editor draws dock panels");
    let step = bevy_dark_factory::chrome::space(bevy_dark_factory::chrome::STEP_PAD);
    assert_eq!(step, 12.0, "spacing step 2 is the editor's 12 px");
    let skin = app.world().resource::<GooeySkin>().clone();
    assert_eq!(skin.density, Density::Compact, "the shipped sheet is compact");
    assert_eq!(skin.pad(), 6.0, "and a compact skin's `pad()` is half its padding, so the role alone would draw 6 px");
    for (name, left, top) in &drawn {
        assert_eq!(
            (*left, *top),
            (step, step),
            "`{name}` is drawn {left} px in on each side; the editor has always drawn a panel {step} px in, \
             so adopting the role must not move it",
            left
        );
    }
}

/// **The density reaches a panel the moment the sheet stops overriding it.**
///
/// The shipped rule states the panel's inset (`padding: !Step 2`, this editor's deliberate 12 px over
/// `skin.pad()`), and gooey applies a node's `Spacing` **after** `Role::Panel`'s arm — so a stated
/// override wins, which is what keeps the editor's look while `density: Compact` halves `pad()`. The
/// proof that the role's `pad()` is what is underneath, and not a dead end, is a person's style layer
/// that drops the override: the drawn `Node`'s padding then **is** `skin.pad()`, and it is half as
/// much at `Compact` as at `Comfortable`.
///
/// This is the half a role-presence test cannot make, and the half that failed before #650: with a
/// `Role::Sized` panel there was no role padding at all, so dropping the rule left the panel with
/// **no inset** — 0 px, not `pad()`.
#[test]
fn without_the_override_the_drawn_padding_is_the_skins_pad() {
    let root = fixtures::Fixture::new(&format!("panel_inset_layer_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let config = root.join("user_config");
    std::fs::create_dir_all(&config).expect("a config folder");
    let style = config.join("style.yaml");
    // A layer whose panel rule is empty: the role's own inset, nothing on top of it.
    std::fs::write(&style, "version: 1\nrules: {\".chrome-panel\": {margin: !Step 2}}\n").expect("a style layer");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy_dark_factory::style::UserStyleFile(style));
    settle_support::settle(&mut app);

    let skin = app.world().resource::<GooeySkin>().clone();
    assert_eq!(skin.density, Density::Compact, "the shipped sheet's density, unchanged by the layer");
    let pad = skin.pad();
    assert_ne!(pad, skin.padding, "a compact skin's `pad()` is not its `padding`: {pad} against {}", skin.padding);
    let drawn = panel_paddings(&mut app);
    assert!(!drawn.is_empty(), "the editor draws dock panels");
    for (name, left, top) in &drawn {
        assert_eq!(
            (*left, *top),
            (pad, pad),
            "with the rule's override gone, `{name}`'s drawn padding must be `skin.pad()` ({pad} px): \
             the role's own arm, which is what makes the density reach a panel"
        );
    }
}
