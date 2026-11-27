//! **The type scale is declared once, and the editor and gooey read the same number** (#648).
//!
//! # What was declared twice
//!
//! `style/editor.yaml` stated the scale three times over: `skin.text_size: 11.0`, `skin.type_scale:
//! [9, 10, 11, 13, 15]`, and `metrics.type: {title: 15, tab: 13, body: 11, …}` — the same five numbers in
//! two unrelated keys with nothing linking them. The editor's own labels read `metrics.type` through
//! `chrome::font`; every `bevy_gooey::label` reads `skin.text_size`. So the documented way to change the
//! text size (`metrics: {type: {body: 14.0}}`) moved the editor's labels and left gooey's at 11 — the
//! two-body-size split #518 measured at **550 of 602 texts**, back again on the first edit.
//!
//! # What this pins
//!
//! `style::merge` derives `skin.text_size`, `skin.heading_size` and `skin.type_scale` from `metrics.type`
//! on every layer sum, so there is one declaration and no way for the two to disagree — not at the
//! shipped sheet, and not after a person's layer edits a size.

use bevy::text::FontSize;
use bevy_gooey::GooeySkin;

use bevy_dark_factory::chrome::{self, text};
use bevy_dark_factory::style::{self, EditorMetrics, TypeMetrics};

/// **The sheet holds the scale in one place**: no `skin` block in memory carries a size `metrics.type`
/// does not name, and the numbers agree key by key.
#[test]
fn the_shipped_sheet_states_the_type_scale_once() {
    let style = style::compiled_default();
    let type_ = style.metrics.r#type;
    assert_eq!(
        style.skin.text_size, type_.body,
        "the skin's body size is `metrics.type.body`, derived by `style::merge` rather than written beside it"
    );
    assert_eq!(
        style.skin.heading_size, type_.title,
        "the skin's heading size is the title's, likewise derived"
    );
    // **The ladder is the roles' sizes**, ascending — not a second copy of them.
    let mut named = vec![type_.hint, type_.label, type_.body, type_.tab, type_.title];
    named.sort_by(f32::total_cmp);
    let mut ladder = style.skin.type_scale.to_vec();
    ladder.sort_by(f32::total_cmp);
    assert_eq!(
        ladder, named,
        "the skin's `type_scale` is the rungs `metrics.type` names, not a list of its own"
    );
    // **And the sheet file itself no longer writes them**: a second literal cannot be edited into
    // disagreement if it is not there.
    let sheet = include_str!("../src/style/editor.yaml");
    for key in ["text_size:", "heading_size:", "type_scale:"] {
        assert!(
            !sheet.contains(key),
            "`style/editor.yaml` still states `{key}`; the scale is `metrics.type`'s alone (#648)"
        );
    }
}

/// **A layer that edits `metrics.type.body` moves both systems' body size**, which is the assertion that
/// fails today: the editor's `chrome::font` and gooey's `skin.text_size * text_scale` are drawn at one
/// size, on the same frame.
#[test]
fn a_body_size_in_a_layer_draws_one_size_for_the_editor_and_for_gooey() {
    // **The shipped sheet already**: the two agree before any layer, which is the invariant the derivation
    // keeps rather than one it introduces.
    let shipped = style::merge(&[&style::read_layer("base", include_str!("../src/style/editor.yaml")).expect("the sheet reads")])
        .expect("the sheet merges");
    assert_eq!(
        shipped.skin.text_size, shipped.metrics.r#type.body,
        "at the shipped sheet the skin's body is the metrics' body"
    );

    // **And with a person's layer setting `body: 14.0`**: merged over the sheet, `skin.text_size` is 14.0,
    // not the 11.0 the deleted literal used to pin it to.
    let user = style::read_layer("style.yaml", "version: 1\nmetrics: {type: {body: 14.0}}\n").expect("the layer reads");
    let merged = style::merge(&[
        &style::read_layer("base", include_str!("../src/style/editor.yaml")).expect("the sheet reads"),
        &user,
    ])
    .expect("the layers merge");
    assert_eq!(merged.metrics.r#type.body, 14.0, "the person's body size applied");
    assert_eq!(
        merged.skin.text_size, 14.0,
        "**the gooey skin's body size followed the metric** — before #648 it stayed at the sheet's 11.0, so a \
         gooey label and a `chrome::typed(text::BODY)` label drew at two sizes in one window"
    );

    // **The ladder follows too**, so a gooey role resolved by step moves with the same edit.
    let ladder = merged.skin.type_scale;
    assert!(
        ladder.contains(&14.0),
        "the derived `type_scale` carries the new body size: {ladder:?}"
    );
    assert!(
        ladder.windows(2).all(|w| w[0] <= w[1]),
        "the ladder stays ascending, so a role's step never reads a smaller rung after a larger one: {ladder:?}"
    );
}

/// **The editor's own `chrome::font(BODY)` is the number the sheet now holds**, read the way a label is drawn:
/// the shipped `TypeMetrics.body` times the running text scale.
#[test]
fn the_editors_body_label_is_the_metrics_body() {
    let default = EditorMetrics::default();
    let shipped = style::compiled_default();
    assert_eq!(
        default.r#type, shipped.metrics.r#type,
        "the compiled sheet's metrics are the type's defaults unless the sheet states otherwise"
    );
    let size = match chrome::font(text::BODY).font_size {
        FontSize::Px(px) => px,
        other => panic!("a label is drawn in logical px, not {other:?}"),
    };
    assert!(
        (size - TypeMetrics::default().body * chrome::text_scale()).abs() < 1.0e-4,
        "`chrome::font(BODY)` is `metrics.type.body` times the text scale: {size}"
    );
    let _ = GooeySkin::default();
}
