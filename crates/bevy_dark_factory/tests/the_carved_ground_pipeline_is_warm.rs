//! **The carved ground's pipeline is compiled before the first carve** (#268). Bevy draws nothing for
//! an entity whose pipeline is still compiling, and the first carve despawned the heightfield ground
//! in the frame it spawned the chunks, so the bounds slab showed through for 0.2 s (plain material) to
//! 0.5 s (weathered), measured by a 15 fps capture on big. A triangle with the chunks' vertex layout
//! and material is drawn every frame from the moment the editor opens, so the chunks' pipeline is
//! already there. The warm triangle is only as good as its layout: it must be the chunks' layout.
//! `cargo test -p bevy_dark_factory --features isomesh --test editor the_carved_ground_pipeline_is_warm::`.

use bevy::prelude::*;
use bevy_dark_factory::terrain_view::PipelineWarm;

use crate::fixtures::Fixture;

#[test]
fn an_open_editor_draws_a_warm_triangle_in_the_carved_grounds_material() {
    let root = Fixture::new("pipeline_warm").descriptor("flat", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let world = app.world_mut();
    let warm: Vec<Handle<StandardMaterial>> = world.query_filtered::<&MeshMaterial3d<StandardMaterial>, With<PipelineWarm>>().iter(world).map(|m| m.0.clone()).collect();
    assert_eq!(warm.len(), 1, "one warm triangle wears the plain carved-ground material");
    let want = bevy_dark_factory::terrain_view::volume_base();
    let worn = world.resource::<Assets<StandardMaterial>>().get(&warm[0]).expect("the warm material exists");
    assert_eq!(worn.base_color, want.base_color);
    assert_eq!(worn.perceptual_roughness, want.perceptual_roughness);
    let at = world.query_filtered::<&Transform, With<PipelineWarm>>().single(world).expect("one warm triangle").translation;
    assert!(at.y < -1.0e4, "the warm triangle is far under the map, never in view: {at}");
}

#[cfg(feature = "isomesh")]
#[test]
fn the_warm_triangle_has_the_vertex_layout_of_a_carved_chunk() {
    use crate::settle_support::Frames;
    use bevy_dark_factory::ops::{self, Applied, Ctx};
    use bevy_dark_factory::terrain_view::TerrainView;
    use blueprint_model::heightfield::Heightfield;
    use serde_json::json;

    /// How long one frame of the wait for the carved chunk sleeps.
    const PAUSE: std::time::Duration = std::time::Duration::from_millis(50);
    /// The frames the carved chunk is given to be drawn: at least five seconds at [`PAUSE`], counted in frames so a starved host is not
    /// read as a chunk that was never drawn (#644).
    const DRAWN_WITHIN: Frames = Frames::covering(std::time::Duration::from_secs(5), PAUSE);

    let root = Fixture::new("pipeline_layout").descriptor("flat", "alpha").build("m");
    let mut app = bevy_dark_factory::harness::build_headless(&root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..4 {
        app.update();
    }
    let project = app.world().resource::<bevy_dark_factory::project::Project>().clone();
    std::fs::create_dir_all(project.blueprint_dir.join("terrain")).unwrap();
    std::fs::write(project.blueprint_dir.join("terrain/m.yaml"), blueprint_model::yaml::to_string(&Heightfield::flat((0.0, 0.0), 1.0, 33, 33, 0.3)).unwrap()).unwrap();
    let mut ctx = Ctx { project: Some(project), session: "test".to_owned(), ..Ctx::default() };
    let carve = json!({"map": "m", "edit": {"shape": {"Sphere": {"center": [16.0, 0.3, 16.0], "radius_m": 5.0}}, "mode": "Carve"}});
    match ops::dispatch(&mut ctx, "volume_edit", &carve).unwrap() {
        Applied::Done { .. } => {}
        Applied::Refused(e) => panic!("refused: {}", e.0),
    }
    let mut frames = 0;
    let chunk = loop {
        app.update();
        let world = app.world_mut();
        let found = world.query::<(&TerrainView, &Mesh3d)>().iter(world).find(|(v, _)| **v == TerrainView::Volume).map(|(_, m)| m.0.clone());
        if let Some(handle) = found {
            break handle;
        }
        frames += 1;
        assert!(frames < DRAWN_WITHIN.0, "no carved chunk was drawn within {DRAWN_WITHIN:?}");
        std::thread::sleep(PAUSE);
    };
    let meshes = app.world().resource::<Assets<Mesh>>();
    let chunk = meshes.get(&chunk).expect("the chunk's mesh");
    let warm = bevy_dark_factory::terrain_view::warm_mesh();
    let layout = |m: &Mesh| m.attributes().map(|(a, _)| a.name).collect::<std::collections::BTreeSet<_>>();
    assert_eq!(layout(&warm), layout(chunk), "the warm triangle must carry exactly the attributes a carved chunk does, or it compiles another pipeline");
    assert_eq!(warm.indices().map(|i| matches!(i, bevy::mesh::Indices::U32(_))), chunk.indices().map(|i| matches!(i, bevy::mesh::Indices::U32(_))));
}
