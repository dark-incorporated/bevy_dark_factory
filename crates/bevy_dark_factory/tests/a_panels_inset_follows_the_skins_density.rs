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

/// **The density reaches a panel as soon as the sheet stops stating an inset** — the lever, measured.
///
/// # Why this edits the *base* sheet and not a user layer
///
/// A later style layer **cannot** remove a declaration: `StyleDecl::padding` is `Sides<Space>` and
/// layers merge per side with `Sides::overlaid`, whose rule is `over.top.or(self.top)` — a side a later
/// layer leaves `None` keeps the value underneath (`bevy_gooey` `src/box_model.rs:117-125`). So a layer
/// that writes `{margin: !Step 2}` leaves the shipped `padding: !Step 2` in force, and one that writes
/// `padding: !Step 0` draws 3 px rather than `skin.pad()`. Neither is the panel's role padding. The
/// **base sheet** is the layer that owns the declaration, and it is a live file the editor watches
/// (`the_style_sheet_reloads_live.rs` edits it the same way), so removing it there is the honest test.
///
/// # What it proves
///
/// With the declaration gone the drawn `Node`'s padding is `skin.pad()` — 6 px at the shipped
/// `Density::Compact`, half the 12 px the rule states — which is the role's arm
/// (`bevy_gooey` `src/skin.rs:4091`) and nothing else. Before #650 the panel was `Role::Sized`, whose
/// arm writes no padding at all, so the same sheet left every panel drawing **0 px**: the lever did
/// not exist.
#[test]
fn without_the_declaration_the_drawn_padding_is_the_skins_pad() {
    let root = fixtures::Fixture::new(&format!("panel_inset_base_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    // A checkout the editor reads its sheet from, so the base layer can be edited without touching
    // this repository (the shape `the_style_sheet_reloads_live.rs` uses).
    let checkout = root.join("checkout");
    let style_dir = checkout.join("crates/bevy_dark_factory/src/style");
    std::fs::create_dir_all(&style_dir).expect("a style folder");
    std::fs::write(checkout.join("crates/bevy_dark_factory/Cargo.toml"), "[package]\nname = \"bevy_dark_factory\"\n")
        .expect("a manifest");
    let shipped = include_str!("../src/style/editor.yaml");
    assert!(
        shipped.contains("\".chrome-panel\": {padding: !Step 2, margin: !Step 2}"),
        "this test edits the panel rule, so it must find it as written"
    );
    let without = shipped.replace(
        "\".chrome-panel\": {padding: !Step 2, margin: !Step 2}",
        "\".chrome-panel\": {margin: !Step 2}",
    );
    std::fs::write(style_dir.join("editor.yaml"), &without).expect("the base sheet, without the declaration");

    // The settings file that names the checkout as `editor_source`.
    let config = root.join("user_config");
    std::fs::create_dir_all(&config).expect("a config folder");
    let settings = config.join("settings.yaml");
    std::fs::write(&settings, format!("version: 1\neditor_source: {}\n", checkout.display())).expect("a settings file");

    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.insert_resource(bevy_dark_factory::settings::UserSettingsFile(settings));
    // **Wait for the checkout's sheet to be the one running**, not merely for the frames to go quiet: the
    // binary starts on its compiled copy of the sheet, and the edited file reaches the skin when the
    // file index has listed it. A panel drawn 12 px in is still the compiled copy's declaration.
    settle_support::until(&mut app, "the checkout's sheet, with the panel's declaration gone", |app| {
        let drawn = panel_paddings(app);
        !drawn.is_empty() && drawn.iter().all(|(_, left, _)| *left != 12.0)
    });

    let skin = app.world().resource::<GooeySkin>().clone();
    assert_eq!(skin.density, Density::Compact, "the base sheet's density is the shipped one");
    let pad = skin.pad();
    assert_eq!(pad, 6.0, "a compact skin's `pad()` is half its 12 px padding");
    let drawn = panel_paddings(&mut app);
    assert!(!drawn.is_empty(), "the editor draws dock panels");
    for (name, left, top) in &drawn {
        assert_eq!(
            (*left, *top),
            (pad, pad),
            "with the sheet's declaration gone, `{name}`'s drawn padding must be `skin.pad()` ({pad} px): \
             the role's own arm, which is what makes the density reach a panel"
        );
    }
}

/// **The role draws the same corner and row gap the deleted rule stated** — the numbers, off the
/// drawn `Node`s.
///
/// The rule used to say `gap: !Step 1` and `radius: !Step 1`, which resolve to `skin.space(1)` and
/// `skin.corner(1)`. `Role::Panel`'s arm says `skin.gap` and `skin.radius` (`bevy_gooey`
/// src/skin.rs:4086-4092). Those are the same two numbers only because the shipped sheet's `gap` is
/// `spacing[1]` and its `radius` is `radii[1]` — an identity a theme can break, which is why this
/// measures rather than asserts it from the file.
#[test]
fn the_roles_corner_and_gap_are_the_numbers_the_rule_stated() {
    // The identity the swap depends on, in the shipped skin.
    let skin = bevy_dark_factory::style::compiled_default().skin;
    assert_eq!(skin.gap, skin.space(1), "the shipped skin's `gap` is spacing step 1, which the rule named");
    assert_eq!(skin.radius, skin.corner(1), "the shipped skin's `radius` is radii step 1, which the rule named");

    let mut app = editor("corner_gap");
    let world = app.world_mut();
    let panels: Vec<Entity> = world.query_filtered::<Entity, With<PanelWidth>>().iter(world).collect();
    assert!(!panels.is_empty(), "the editor draws dock panels");
    for panel in panels {
        let name = world.get::<AccessibleLabel>(panel).map_or_else(|| format!("{panel:?}"), |l| l.0.clone());
        let node = world.get::<Node>(panel).expect("a panel is laid out");
        let gap = px(node.row_gap);
        let corner = px(node.border_radius.top_left.x);
        assert_eq!(gap, skin.space(1), "`{name}`'s row gap is the 5 px the sheet's `!Step 1` stated");
        assert_eq!(corner, skin.corner(1), "`{name}`'s corner is the 4 px the sheet's `!Step 1` stated");
    }
}
