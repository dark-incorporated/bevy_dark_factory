//! **The DATA panel is a sortable table, and a cell edited in place is one `data_set`** (UI-56, #242).
//!
//! The registered files' leaves were one text row each; they are a gooey table now (file, path, value), so a column sorts and a row
//! opens. Opening a row puts a text field over its value; its final change writes through `data_set`, so the file shows the value,
//! the oplog holds one line for it and one undo takes it back. With no `data.yaml` there is no table, only the line that says so.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_dark_factory::editor::DataRows;
use bevy_dark_factory::harness;
use bevy_dark_factory::ops;
use bevy_dark_factory::project::Project;
use bevy_gooey::testing;
use bevy_gooey::{RowActivated, RowSource, Table, TextInputOf, TextInputValue};

use crate::fixtures;

const TEXT: &str = "# tuning\nmob:\n  speed: 1.50\n  name: orc\n  hp: 7\nfast: false\n";

fn editor(tag: &str, registry: bool) -> (App, std::path::PathBuf) {
    let root = fixtures::Fixture::new(tag).descriptor("site/wall", "alpha").build("m");
    let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.world_mut().spawn((Window::default(), PrimaryWindow));
    for _ in 0..4 {
        app.update();
    }
    let dir = app.world().resource::<Project>().blueprint_dir.clone();
    let file = dir.join("mob.yaml");
    std::fs::write(&file, TEXT).unwrap();
    if registry {
        std::fs::write(dir.join("data.yaml"), "{version: 1, files: [{id: \"tuning/mob\", path: \"mob.yaml\", bounds: {\"mob.speed\": {min: 0.0, max: 9.0}}}]}").unwrap();
    }
    settle(&mut app);
    (app, file)
}

fn settle(app: &mut App) {
    for _ in 0..6 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        app.update();
    }
}

/// **The DATA panel's tables**: the history list (UI-53) is a gooey table too, in the same Map inspector, so it is left out.
fn tables(app: &mut App) -> Vec<Entity> {
    let world = app.world_mut();
    world.query_filtered::<Entity, (With<Table>, Without<bevy_dark_factory::editor::HistoryTree>)>().iter(world).collect()
}

fn texts(app: &mut App) -> Vec<String> {
    let world = app.world_mut();
    world.query::<&Text>().iter(world).map(|t| t.0.clone()).collect()
}

#[test]
fn the_registered_leaves_are_one_table_with_a_row_each() {
    let (mut app, _) = editor(&format!("data_table_rows_{}", std::process::id()), true);
    let table = tables(&mut app);
    assert_eq!(table.len(), 1, "one table");
    let dir = app.world().resource::<Project>().blueprint_dir.clone();
    let want = bevy_dark_factory::ops_data::listing(&dir);
    assert!(!want.is_empty());
    let shown = app.world().get::<Table>(table[0]).unwrap().source::<DataRows>().map(RowSource::len);
    assert_eq!(shown, Some(want.len()), "a row per leaf");
}

#[test]
fn a_project_with_no_data_file_has_no_table_and_says_so() {
    let (mut app, _) = editor(&format!("data_table_none_{}", std::process::id()), false);
    assert!(tables(&mut app).is_empty());
    assert!(texts(&mut app).iter().any(|t| t == "no data files registered (data.yaml)"));
}

#[test]
fn a_cell_edited_in_place_is_one_data_set_in_the_log_and_the_file() {
    let (mut app, file) = editor(&format!("data_table_edit_{}", std::process::id()), true);
    let table = tables(&mut app)[0];
    let key = {
        let rows = app.world().get::<Table>(table).unwrap().source::<DataRows>().expect("the data rows").clone();
        let at = (0..rows.len()).find(|&i| rows.cell(i, 1) == "mob.speed").expect("the speed leaf is a row");
        rows.key(at)
    };
    app.world_mut().commands().trigger(RowActivated { entity: table, key });
    settle(&mut app);
    // The field that has the keys is the cell's: it stands in a text row that holds the leaf's value as the file has it.
    let field = testing::focused(&app).expect("opening the row gives a field the keys");
    let row = app.world().get::<TextInputOf>(field).expect("the focus is on a text field").0;
    assert_eq!(app.world().get::<TextInputValue>(row).map(|v| v.0.as_str()), Some("1.50"), "the field holds the leaf's value");
    for _ in 0.."1.50".len() {
        testing::press(&mut app, KeyCode::Backspace);
    }
    testing::type_text(&mut app, "2.5");
    testing::press(&mut app, KeyCode::Enter);
    settle(&mut app);

    assert_eq!(std::fs::read_to_string(&file).unwrap(), "# tuning\nmob:\n  speed: 2.5\n  name: orc\n  hp: 7\nfast: false\n");
    let project = app.world().resource::<Project>().clone();
    let sets: Vec<_> = ops::read_entries(&ops::oplog_path(&project)).into_iter().filter(|e| e.op == "data_set").collect();
    assert_eq!(sets.len(), 1, "one data_set line: {sets:?}");
    assert_eq!(sets[0].params["path"], "mob.speed");
    assert!(app.world().get_entity(row).is_err(), "the field is gone once it has written");
}

/// A node's border box, physical px.
fn physical_box(app: &App, entity: Entity) -> Rect {
    let world = app.world();
    let computed = world.get::<bevy::ui::ComputedNode>(entity).expect("laid out");
    let at = world.get::<bevy::ui::UiGlobalTransform>(entity).expect("placed").translation;
    Rect::from_center_size(at, computed.size())
}

/// How far a cell's edge may stand past the table's visible edge, physical px: layout rounds every edge to a whole device pixel.
const EDGE_TOLERANCE_PX: f32 = 1.0;

/// **Every header cell and every cell of the DATA table, against the table's own visible edge**: the right edge of its view less the scroll gutter
/// that sits inside it. Returns the sentences for those that stand past it or before the table's left, and the Value column's right edge and the
/// visible edge, physical px.
fn columns_against_the_visible_edge(app: &mut App, tag: &str) -> (Vec<String>, f32, f32) {
    let table = tables(app)[0];
    let (view, cells) = {
        let world = app.world();
        let mut stack = vec![table];
        let (mut view, mut cells) = (None, Vec::new());
        while let Some(entity) = stack.pop() {
            if let Some(kids) = world.get::<Children>(entity) {
                stack.extend(kids.iter());
            }
            match world.get::<bevy_gooey::Role>(entity) {
                Some(bevy_gooey::Role::TableView) => view = Some(entity),
                Some(bevy_gooey::Role::TableHeaderCell | bevy_gooey::Role::TableCell) => cells.push(entity),
                _ => {}
            }
        }
        (view.expect("the table has a view"), cells)
    };
    let view_box = physical_box(app, view);
    let gutter = app.world().get::<bevy::ui::ComputedNode>(view).map(|c| c.content_inset().max_inset.x).unwrap_or_default();
    let visible_right = view_box.max.x - gutter;
    assert!(cells.len() >= 6, "{tag}: a header and at least one row of three cells are drawn ({} cells)", cells.len());
    let mut refused = Vec::new();
    let mut value_right = f32::MIN;
    for cell in cells {
        let cell_box = physical_box(app, cell);
        value_right = value_right.max(cell_box.max.x);
        if cell_box.max.x > visible_right + EDGE_TOLERANCE_PX || cell_box.min.x < view_box.min.x - EDGE_TOLERANCE_PX {
            refused.push(format!("{tag}: a cell is drawn from {:.1} to {:.1}, the table shows {:.1} to {visible_right:.1}", cell_box.min.x, cell_box.max.x, view_box.min.x));
        }
    }
    (refused, value_right, visible_right)
}

/// **The DATA table's three columns fit the table, at the default window and at the narrowest left pane the editor allows** (the divider taken to
/// its leftmost position, which gooey clamps to the pane's floor): no cell's right edge stands past the table's visible edge, within one device pixel.
/// With the old widths (5 + 8 + 9 em) the Value column ended 31 physical px past it at the default window and began past it at the narrowest.
#[test]
fn the_data_tables_columns_stay_inside_the_table_at_every_width_the_pane_allows() {
    let (mut app, _) = editor(&format!("data_table_fit_{}", std::process::id()), true);
    let table = tables(&mut app)[0];
    let default_width = physical_box(&app, table).width();
    let (refused, value_right, visible_right) = columns_against_the_visible_edge(&mut app, "default window");
    assert!(refused.is_empty(), "{refused:?}");
    assert!(visible_right - value_right <= 1.0, "at the default window the last column takes the room left: it ends at {value_right:.1}, the table shows to {visible_right:.1}");

    let split = {
        let world = app.world_mut();
        world.query_filtered::<Entity, With<bevy_dark_factory::chrome::ShellSplit>>().single(world).expect("one shell split")
    };
    app.world_mut().get_mut::<bevy_gooey::SplitFraction>(split).expect("a fraction").0 = 0.0;
    settle(&mut app);
    let table = tables(&mut app)[0];
    let narrow_width = physical_box(&app, table).width();
    assert!(narrow_width < default_width - 20.0, "the divider was taken to its leftmost position and the table is narrower ({narrow_width:.1} against {default_width:.1})");
    let (refused, _, _) = columns_against_the_visible_edge(&mut app, "narrowest pane");
    assert!(refused.is_empty(), "{refused:?}");
}
