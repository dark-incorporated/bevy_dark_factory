//! **One type scale** (#648): the style sheet declares its type sizes once, under `metrics.type`, and
//! `style::merge` derives gooey's `text_size`, `heading_size` and `type_scale` from them.
//!
//! # The defect this pins
//!
//! The sheet declared the same five sizes twice — `skin.type_scale: [9, 10, 11, 13, 15]` beside
//! `metrics.type: {hint: 9, label: 10, body: 11, tab: 13, title: 15}` — and nothing derived one from
//! the other. The editor's own labels (`chrome::typed`) read `metrics.type`; every gooey label reads
//! `skin.text_size`. So a person editing `metrics.type.body` — the documented way to change the text
//! size — moved one half of the window and not the other, which is the two-body-size state #518
//! measured as 550 of 602 texts at 11.666 px.
//!
//! # What each test is for
//!
//! - a merged style's `skin.text_size`/`heading_size`/`type_scale` **are** `metrics.type`'s numbers,
//!   so the two cannot disagree;
//! - the shipped sheet declares the scale once, so there is no second place to edit;
//! - editing one role in a layer draws a `bevy_gooey::label` where a `chrome::typed` label is, which
//!   is the assertion that failed before this change;
//! - a layer that states one of the three derived fields is refused, naming the field and where the
//!   size is really stated.

use bevy::prelude::*;
use bevy::text::FontSize;

use bevy_dark_factory::chrome::{self, text};
use bevy_dark_factory::harness;
use bevy_dark_factory::style;

use crate::fixtures;
use crate::settle_support;

/// A drawn `TextFont`'s size in logical px; every size here is a `FontSize::Px`.
fn px(font: FontSize) -> f32 {
    match font {
        FontSize::Px(px) => px,
        other => panic!("a drawn text is a `FontSize::Px`, not {other:?}"),
    }
}

/// **The shipped sheet's gooey type fields are its own `metrics.type`**, so the sheet states one scale.
#[test]
fn the_shipped_skins_type_sizes_are_the_metrics_types() {
    let style = style::compiled_default();
    let size = style.metrics.r#type;
    assert_eq!(style.skin.text_size, size.body, "`skin.text_size` is `metrics.type.body`, not a second 11.0");
    assert_eq!(style.skin.heading_size, size.title, "`skin.heading_size` is `metrics.type.title`");
    let mut ladder = [size.hint, size.label, size.body, size.tab, size.title];
    ladder.sort_by(f32::total_cmp);
    assert_eq!(style.skin.type_scale, ladder, "`skin.type_scale` is the five distinct `metrics.type` sizes");
    assert_eq!(style.skin.problems(), vec![], "and the derived skin breaks none of gooey's own rules");
}

/// **The sheet's text declares the scale once**: none of the three derived fields appears under `skin`,
/// so there is no second place to edit and nothing for the two to disagree about.
#[test]
fn the_sheet_declares_no_second_copy_of_the_scale() {
    let sheet = include_str!("../src/style/editor.yaml");
    let skin_block = sheet
        .split_once("\nskin:")
        .expect("the sheet has a skin block")
        .1
        .split_once("\nmetrics:")
        .expect("and a metrics block after it")
        .0;
    let lines: Vec<&str> = skin_block
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .collect();
    for field in style::DERIVED_SKIN_TYPE_FIELDS {
        let key = format!("{field}:");
        let stated: Vec<&&str> = lines.iter().filter(|line| line.starts_with(&key)).collect();
        assert!(
            stated.is_empty(),
            "the shipped sheet still declares `skin.{field}` ({stated:?}). It is derived from `metrics.type` \
             (#648), so a second copy here is a second authority nothing links to the first — editing one \
             leaves the window with two body sizes (#518)."
        );
    }
}

/// **Editing one role moves both halves.** With a layer that sets `type.body: 14.0`, a `bevy_gooey::label`
/// and a `chrome::typed(text::BODY)` label are drawn at the same `FontSize`. Before this change the
/// gooey one stayed at the shipped size, which is the failure this test exists for.
#[test]
fn a_body_size_in_a_layer_draws_the_gooey_label_where_the_editors_own_is() {
    let root = fixtures::Fixture::new(&format!("one_scale_{}", std::process::id()))
        .descriptor("site/wall", "alpha")
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let config = root.join("user_config");
    std::fs::create_dir_all(&config).expect("a config folder");
    let user_style = config.join("style.yaml");
    app.insert_resource(style::UserStyleFile(user_style.clone()));
    settle_support::settle(&mut app);

    let ours = app.world_mut().spawn((Text::new("ours"), chrome::typed(text::BODY))).id();
    let theirs = app.world_mut().spawn(bevy_gooey::label("theirs")).id();
    settle_support::settle(&mut app);
    let size = |app: &App, entity: Entity| -> f32 {
        let font = app.world().get::<TextFont>(entity).map(|font| font.font_size);
        px(font.unwrap_or_else(|| panic!("entity {entity} is drawn")))
    };
    let before = (size(&app, ours), size(&app, theirs));
    assert_eq!(before.0, before.1, "at the shipped sheet the two are one size: {before:?}");

    std::fs::write(&user_style, "version: 1\nmetrics: {type: {body: 14.0}}\n").expect("a user layer");
    settle_support::settle(&mut app);
    assert_eq!(style::metrics().r#type.body, 14.0, "the layer's body size is the running metric");
    // `chrome::resize_the_labels` rewrites the editor's own label off the metric; **`apply_skin` rewrites
    // the gooey one off the skin's `text_size`**, which this change made the same metric.
    settle_support::settle(&mut app);
    let after = (size(&app, ours), size(&app, theirs));
    assert!(
        (after.0 - before.0).abs() > 0.01,
        "the metric moved the editor's own label: {:?} -> {after:?}",
        before
    );
    assert_eq!(
        after.0, after.1,
        "the layer's body size must draw the gooey label where the editor's own is, or the window has two \
         body sizes again (#518): ours {:?}, gooey {:?}",
        after.0, after.1
    );
}

/// **A layer that states a derived field is refused, naming it and saying where the size lives.**
#[test]
fn a_layer_that_states_a_derived_skin_type_field_is_refused() {
    for field in style::DERIVED_SKIN_TYPE_FIELDS {
        let text = if field == "type_scale" {
            "version: 1\nskin: {type_scale: [9.0, 10.0, 11.0, 13.0, 15.0]}\n".to_owned()
        } else {
            format!("version: 1\nskin: {{{field}: 11.0}}\n")
        };
        let refused = style::read_layer("style.yaml", &text)
            .map(|_| ())
            .expect_err(&format!("`skin.{field}` is derived from `metrics.type` and must be refused"));
        assert_eq!(refused.kind, style::KIND, "`skin.{field}`: {}", refused.sentence());
        assert_eq!(
            refused.field.as_deref(),
            Some(format!("skin.{field}").as_str()),
            "the refusal names the field it is at: {}",
            refused.sentence()
        );
        assert!(
            refused.reason.contains("metrics.type"),
            "the refusal says where the size is stated instead: {}",
            refused.sentence()
        );
    }
}
