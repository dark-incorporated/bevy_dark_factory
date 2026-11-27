//! **The Files page's header readouts stand at its two ends, and its field line is one control with a class of
//! its own** (#542).
//!
//! # What the capture measured
//!
//! `Ln 1, Col 1` drew at **x 800..835** inside a row 462..1344 wide — 500 px from the row's right edge, not on
//! it. The cause was not the readout's `TextLayout::Right` but that neither header text carried a `Role`, and
//! gooey writes a `TextFlow` only on a node it styles: the path's `grow` had never been applied, so a box that
//! only measures its words had nothing to justify within. Under it a bordered **153 x 29** field sat at
//! x 1164..1316 while its placeholder drew at 489..681, 483 px to the left: the field and its words in two
//! different columns. And `blueprint/ui_style_inspect` over the box answered `classes: ["chrome-panel"]` — the
//! page card — so the control answered to no name of its own.
//!
//! # What this pins, and what it cannot
//!
//! The **class** and the **no-file rule** are asserted here, because a `StyleClass` and a `Display` are facts
//! the world holds. The header's pixel alignment is a layout fact this harness does not lay out (measured in
//! #542's own comment: `Device::None` lays nothing out, `Device::Real` panics before the first frame), so the
//! spawn's `TextFlow { align: Some(AlignSelf::End) }` and its `chrome::text` role are asserted as the bundle the
//! node is given — the arrangement `bevy_gooey::apply_skin` writes `align_self` from — and the reading of the
//! drawn rect belongs to a staged editor over BRP.

use bevy::prelude::*;

use bevy_dark_factory::files::{FieldLine, FileHeader, FilePosition, FIELD_LINE_CLASS};
use bevy_dark_factory::harness;
use bevy_dark_factory::workspace::Workspace;
use bevy_gooey::{StyleClass, TextFlow};

use crate::fixtures;
use crate::settle_support;

fn editor(tag: &str) -> App {
    let root = fixtures::Fixture::new(&format!("files_542_{tag}_{}", std::process::id()))
        .descriptor("alpha/floor", "alpha")
        .place_as("floor@0", "alpha/floor", (0.0, 0.0))
        .build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    settle_support::settle(&mut app);
    app
}

fn go_to_files(app: &mut App) {
    use bevy_dark_factory::command_run::Chords;
    use bevy_dark_factory::commands;
    use bevy_dark_factory::keys::Live;
    let command = commands::by_id("workspace.files").unwrap_or_else(|| panic!("`workspace.files` is not a command"));
    let live = *app.world().resource::<Live>();
    app.world_mut().resource_mut::<Chords>().queue(&command, live);
    settle_support::settle(app);
    assert_eq!(app.world().resource::<Workspace>(), &Workspace::Files, "the Files workspace is open");
}

fn one<T: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .iter(app.world())
        .next()
        .unwrap_or_else(|| panic!("the Files page draws no {}", std::any::type_name::<T>()))
}

/// **The path grows and the readout aligns itself to the row's far end** — the two `TextFlow`s whose
/// disagreement was the 500 px.
#[test]
fn the_header_row_sends_its_two_readouts_to_its_two_ends() {
    let mut app = editor("ends");
    go_to_files(&mut app);
    let path = one::<FileHeader>(&mut app);
    let position = one::<FilePosition>(&mut app);

    let path_flow = *app.world().get::<TextFlow>(path).expect("the path carries a `TextFlow`");
    assert_eq!(
        path_flow.grow, Some(1.0),
        "the path takes the row's free width, so the readout's end is the row's end"
    );

    let position_flow = *app.world().get::<TextFlow>(position).expect("the readout carries a `TextFlow`");
    assert_eq!(
        position_flow.align,
        Some(AlignSelf::End),
        "**the readout stands on the row's far edge** — `align_self` is what moves a flex item along the row's \
         main axis; `grow` would share the free width with the path and leave it near the middle"
    );
    // **And both are chrome's own type**, because a `TextFlow` is written only on a node gooey styles — a bare
    // `Text` with no `Role` kept the `TextFlow` it was given and never had it applied.
    for (who, entity) in [("the path", path), ("the readout", position)] {
        assert!(
            app.world().get::<bevy_gooey::Role>(entity).is_some(),
            "{who} carries a `Role`, which is what makes `apply_skin` write its `TextFlow`"
        );
    }
}

/// **The field line carries a class of its own**, so the inspector names the control rather than the card.
#[test]
fn the_field_line_has_a_class_the_inspector_can_name() {
    let mut app = editor("class");
    go_to_files(&mut app);
    let line = one::<FieldLine>(&mut app);
    let classes = app.world().get::<StyleClass>(line).expect("the field line carries a `StyleClass`");
    let text = classes.0.join(" ");
    assert!(
        text.contains(FIELD_LINE_CLASS),
        "the field line's classes are `{text}`; `blueprint/ui_style_inspect` over the box answered \
         `[\"chrome-panel\"]` — the page card — until the row carried `{FIELD_LINE_CLASS}` (#542)"
    );
}

/// **With no file open the field line is not drawn**: there is no field to set, and the box was one a person
/// could tab into that would do nothing.
#[test]
fn the_field_line_is_not_drawn_with_no_file_open() {
    let mut app = editor("nofile");
    go_to_files(&mut app);
    let line = one::<FieldLine>(&mut app);
    let display = app.world().get::<Node>(line).map(|node| node.display);
    assert_eq!(
        display,
        Some(Display::None),
        "no file is open, so the field line takes no room and no focus; it drew a 153 x 29 box that set nothing"
    );
}
