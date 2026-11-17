//! **Every panel of the Files workspace is inside the window** (#438). Found recording the #417 clip: after "Make a benchmark task" opened the draft's
//! `task.yaml` in the Files workspace, the right-hand panel ran past the window's right edge (1280 x 720). With a text file open, at 1024 x 640 and at
//! 1280 x 720, every panel the workspace puts in the frame's three slots has its right edge at or inside the window's, and its left edge at or inside it.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
use bevy::prelude::*;
use bevy::time::TimeUpdateStrategy;
use bevy::ui::{ComputedNode, UiGlobalTransform};
use bevy_dark_factory::chrome::Frame;
use bevy_dark_factory::files;
use bevy_dark_factory::harness;
use bevy_dark_factory::keys::{Action, binding};
use bevy_dark_factory::workspace::Workspace;

use crate::fixtures;

/// The windows the workspace is laid out in, in logical px.
const WINDOWS_PX: [(u32, u32); 2] = [(1024, 640), (1280, 720)];
/// Frames run after each step, so layout settles.
const SETTLE: usize = 30;
/// One frame of the test clock, in seconds.
const FRAME_S: f32 = 1.0 / 60.0;
/// Rounding the layout may leave on an edge, in logical px.
const EDGE_SLACK_PX: f32 = 0.5;
/// The most a panel may stand in from its slot's edge, in logical px: the panel's own margin (12) twice over. More is an empty box beside it.
const MAX_INSET_PX: f32 = 24.0;
/// The file opened: the benchmark task draft the issue names.
const TASK: &str = "task.yaml";
const TASK_TEXT: &str = "id: draft\ntitle: A drafted task\nprompt: |\n  Place a wall along the north edge of the yard, one cell in from the fence, and name it for the person who reads it.\nchecks:\n  - placement_count >= 4\n";

fn settle(app: &mut App) {
    for _ in 0..SETTLE {
        app.update();
    }
}

fn press(app: &mut App, action: Action) {
    let row = binding(action);
    let mut keys = vec![row.key];
    if row.needs_mod {
        keys.push(if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft });
    }
    for state in [ButtonState::Pressed, ButtonState::Released] {
        for code in &keys {
            app.world_mut().write_message(KeyboardInput {
                key_code: *code,
                logical_key: Key::Unidentified(NativeKey::Unidentified),
                state,
                text: None,
                repeat: false,
                window: Entity::PLACEHOLDER,
            });
        }
        app.update();
        app.update();
    }
    settle(app);
}

/// A node's rect in logical px: `(left, top, right, bottom)`.
fn rect(app: &App, entity: Entity) -> Option<(f32, f32, f32, f32)> {
    let computed = app.world().get::<ComputedNode>(entity)?;
    let at = app.world().get::<UiGlobalTransform>(entity)?.translation;
    let scale = computed.inverse_scale_factor();
    let (w, h) = (computed.size().x * scale, computed.size().y * scale);
    let (cx, cy) = (at.x * scale, at.y * scale);
    Some((cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0))
}

/// **Every panel in the left dock, the viewport slot and the right dock is inside the window**, with a text file open in the Files workspace.
#[test]
fn every_files_panel_is_inside_the_window_at_both_sizes_with_a_text_file_open() {
    let mut refused = Vec::new();
    for (width, height) in WINDOWS_PX {
        let root = fixtures::Fixture::new(&format!("files_fit_{width}_{}", std::process::id())).descriptor("site/wall", "alpha").place("site/wall", (1.5, 1.5)).build("m");
        std::fs::write(root.join(TASK), TASK_TEXT).unwrap_or_else(|e| panic!("{e}"));
        let mut app = harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
        harness::resize_surface(&mut app, width, height).unwrap_or_else(|e| panic!("{e}"));
        app.insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(FRAME_S)));
        settle(&mut app);
        press(&mut app, Action::WorkspaceFiles);
        assert_eq!(*app.world().resource::<Workspace>(), Workspace::Files, "{width}x{height}: the Files workspace is open");
        files::open_file(app.world_mut(), TASK).unwrap_or_else(|e| panic!("{width}x{height}: {TASK} opens: {e}"));
        settle(&mut app);

        let frame = app.world().resource::<Frame>();
        let slots = [frame.left, frame.viewport, frame.right];
        let mut panels = Vec::new();
        for slot in slots {
            if let Some(children) = app.world().get::<Children>(slot) {
                panels.extend(children.iter());
            }
        }
        let mut seen = 0;
        for panel in panels {
            let Some(node) = app.world().get::<Node>(panel) else { continue };
            if node.display == Display::None {
                continue;
            }
            let Some((left, top, right, bottom)) = rect(&app, panel) else { continue };
            if right <= left || bottom <= top {
                continue;
            }
            seen += 1;
            let name = app.world().get::<Name>(panel).map_or_else(|| format!("{panel}"), ToString::to_string);
            if right > width as f32 + EDGE_SLACK_PX || left < -EDGE_SLACK_PX {
                refused.push(format!("{width}x{height}: panel `{name}` spans {left:.1}..{right:.1}, outside the window 0..{width}"));
            }
        }
        assert!(seen >= 2, "{width}x{height}: the Files workspace's panels are laid out ({seen})");

        // **The editor fills the viewport slot**: it starts at the slot's left edge (less its margin) and ends at its right edge. Before #438 an empty
        // `width: 100%` box shared the slot's row with it, so the editor stood in the slot's right-hand part and ran past the window's edge in the
        // real window.
        let slot = rect(&app, app.world().resource::<Frame>().viewport).unwrap_or_else(|| panic!("{width}x{height}: the viewport slot is laid out"));
        let host = {
            let world = app.world_mut();
            world.query_filtered::<Entity, With<files::FilesHost>>().single(world).unwrap_or_else(|e| panic!("{width}x{height}: one editor host: {e}"))
        };
        let host_rect = rect(&app, host).unwrap_or_else(|| panic!("{width}x{height}: the editor host is laid out"));
        if host_rect.0 - slot.0 > MAX_INSET_PX || slot.2 - host_rect.2 > MAX_INSET_PX {
            refused.push(format!("{width}x{height}: the editor {host_rect:?} does not fill its slot {slot:?}: an empty box stands beside it"));
        }
    }
    assert!(refused.is_empty(), "panels outside the window:\n{}", refused.join("\n"));
}
