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

const LONG_VALUE: &str = "the quick brown fox jumps over the lazy dog and then keeps running across a field of very long unbroken tuning values";

/// **A long value wraps inside its own cell** (`Column::wrap`, bevy_gooey#109): the cell holding it is drawn on several lines and every glyph of it
/// stands inside the cell's box, where a one-line cell draws it out past the column (the defect #109 was filed for). Measured from the glyphs the cell
/// paints (`TextLayoutInfo`), against the cell's own border box. (Measured while writing this: even wrapped, the Value cell's glyphs end 3.1 px past the
/// table's own right edge at the default window, because the three fixed column widths sum to more than the table is wide; that is not what this holds.)
#[test]
fn a_long_value_is_drawn_inside_its_own_cell() {
    let (mut app, file) = editor(&format!("data_table_wrap_{}", std::process::id()), true);
    std::fs::write(&file, format!("{TEXT}note: \"{LONG_VALUE}\"\n")).unwrap();
    settle(&mut app);
    settle(&mut app);
    let table = tables(&mut app)[0];
    let world = app.world();
    let rows = world.get::<Table>(table).unwrap().source::<DataRows>().expect("the data rows").clone();
    assert!((0..rows.len()).any(|i| rows.cell(i, 2).contains("quick brown fox")), "the long value is a row of the table");

    let box_of = |entity: Entity| {
        let computed = world.get::<bevy::ui::ComputedNode>(entity)?;
        let at = world.get::<bevy::ui::UiGlobalTransform>(entity)?.translation;
        Some(Rect::from_center_size(at, computed.size()))
    };
    let mut stack = vec![table];
    let mut cell = None;
    while let Some(entity) = stack.pop() {
        if let Some(kids) = world.get::<Children>(entity) {
            stack.extend(kids.iter());
        }
        if world.get::<bevy_gooey::Role>(entity) == Some(&bevy_gooey::Role::TableCellText) && world.get::<Text>(entity).is_some_and(|t| t.0.contains("quick brown fox")) {
            cell = Some(entity);
        }
    }
    let cell = cell.expect("the long value's cell is drawn (a table with a wrapping column binds every row)");
    let info = world.get::<bevy::text::TextLayoutInfo>(cell).expect("the cell has laid its text out");
    let computed = world.get::<bevy::ui::ComputedNode>(cell).unwrap();
    let start = box_of(cell).unwrap().min + computed.content_inset().min_inset;
    let right = info.glyphs.iter().map(|g| (start + g.position).x + g.atlas_info.rect.size().x).fold(f32::MIN, f32::max);
    let lines = info.glyphs.iter().map(|g| g.line_index).collect::<std::collections::BTreeSet<_>>().len();
    assert!(lines > 1, "the long value is wrapped onto several lines, not drawn on one ({lines} line)");
    let cell_box = box_of(cell).unwrap();
    assert!(right <= cell_box.max.x + 1.0, "the long value is drawn to {right:.1}, past its own cell's right edge {:.1}", cell_box.max.x);
}
