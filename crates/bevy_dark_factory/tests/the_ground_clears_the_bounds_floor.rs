//! **A flat ground does not share a plane with the bounds floor** (#267). The floor slab sat at
//! `origin.y - ground_drop`, and so did the drawn ground at height 0: one plane, which a depth buffer
//! cannot order, so the ground was drawn hatched. Raising the terrain to 3 m hid it, which is how it
//! was found. The slab now sits under the ground by more than the depth buffer can tell apart over
//! the widest depth range the Map camera is given.
//! `cargo test -p bevy_dark_factory --test editor the_ground_clears_the_bounds_floor::`.

use bevy::prelude::*;
use bevy_dark_factory::editor::BoundsFloor;
use bevy_dark_factory::project::OpenMap;
use bevy_dark_factory::terrain_view::TerrainView;
use blueprint_model::heightfield::Heightfield;

use crate::fixtures::Fixture;
use crate::settle_support::Frames;

/// The pause between two frames of the wait for the ground and the floor.
const PAUSE: std::time::Duration = std::time::Duration::from_millis(50);

/// **Frames the editor gets to draw the ground and the floor** (#644): a count, not a wall-clock window, so a starved host does not
/// cut it short; at [`PAUSE`] apart they cover at least 5 s.
const DRAWN_WITHIN: Frames = Frames::covering(std::time::Duration::from_secs(5), PAUSE);

/// The depth buffer is 32-bit float and the Map camera's projection is linear in depth between its
/// `near` and `far`, so two surfaces closer than `(far - near) / 2^22` (four units in the last place
/// of 1.0) can be drawn in either order.
fn indistinguishable_m(near: f32, far: f32) -> f32 {
    (far - near) / (1u32 << 22) as f32
}

#[test]
fn a_flat_ground_at_height_zero_is_drawn_clear_of_the_bounds_floor() {
    let root = Fixture::new("ground_clears_floor").descriptor("flat", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    let project = app.world().resource::<bevy_dark_factory::project::Project>().clone();
    std::fs::create_dir_all(project.blueprint_dir.join("terrain")).unwrap();
    let flat = Heightfield::flat((0.0, 0.0), 1.0, 33, 33, 0.0);
    std::fs::write(project.blueprint_dir.join("terrain/m.yaml"), blueprint_model::yaml::to_string(&flat).unwrap()).unwrap();

    let (mut ground, mut floor) = (None, None);
    for _ in 0..DRAWN_WITHIN.0 {
        app.update();
        let world = app.world_mut();
        ground = world.query::<(&TerrainView, &Transform)>().iter(world).find(|(v, _)| **v == TerrainView::Ground).map(|(_, t)| t.translation.y);
        floor = world.query_filtered::<&Transform, With<BoundsFloor>>().iter(world).next().map(|t| t.translation.y);
        if ground.is_some() && floor.is_some() {
            break;
        }
        std::thread::sleep(PAUSE);
    }
    let (Some(ground_y), Some(floor_y)) = (ground, floor) else {
        panic!("no ground ({ground:?}) or floor ({floor:?}) was drawn (waited {DRAWN_WITHIN:?})");
    };

    // The widest depth range this map's camera is given: the framing's own, from the map's bounds.
    let open = app.world().resource::<OpenMap>();
    let camera = Transform::from_translation(Vec3::new(30.0, 30.0, 30.0)).looking_at(Vec3::ZERO, Vec3::Y);
    let (near, far) = bevy_dark_factory::view::depth_range(&camera, Some(open));
    let needed = indistinguishable_m(near, far);
    let gap = ground_y - floor_y;
    assert!(
        gap > needed && gap >= 0.005,
        "the flat ground is {gap} m above the bounds floor; the depth buffer tells surfaces apart past {needed} m (range {near}..{far})"
    );
}
