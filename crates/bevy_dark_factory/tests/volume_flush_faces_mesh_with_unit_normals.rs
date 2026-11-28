//! **A box standing on the ground, and a column standing on a carved floor, mesh with a unit normal at every vertex**
//! (#701). Two faces on one plane with solid on both sides are a zero of the field that peaks there (`min(t, -t)`), so
//! central differences of the whole field read no slope at all: isomesh's debug assertion "zero gradient at a surface
//! vertex" fired on the vertices of that plane, and a release build gave them a normal that is not a number. Both scenes
//! put the shared plane on a sample plane (whole metres, cells of one metre), the way an author writes them.
//! Needs the `isomesh` feature.
#![cfg(feature = "isomesh")]

use blueprint_model::heightfield::Heightfield;
use blueprint_model::volume::{Edit, Mode, Shape, Volume};
use bevy_dark_factory::volume_mesh::mesh_volume;

fn a_box(center: [f32; 3], half_m: [f32; 3], mode: Mode) -> Edit {
    Edit { shape: Shape::Box { center, half_m }, mode, smooth_m: 0.0 }
}

/// **`edits` over flat ground at `ground_m`, meshed as the editor draws them**, failing on the first vertex whose normal
/// is not one long; the number of vertices.
fn every_normal_is_unit(edits: &[Edit], ground_m: f32) -> usize {
    let ground = Heightfield::flat((0.0, 0.0), 1.0, 33, 33, ground_m);
    let mut volume = Volume::default();
    for e in edits {
        volume.push(*e).expect("an edit the model takes");
    }
    let mut vertices = 0;
    for mesh in mesh_volume(&volume, &ground).expect("the volume meshes") {
        assert_eq!(mesh.normals.len(), mesh.positions.len(), "one normal per vertex");
        for (p, n) in mesh.positions.iter().zip(&mesh.normals) {
            let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            assert!((length - 1.0).abs() < 1e-5, "the vertex at {p:?} has the normal {n:?}, {length} long");
        }
        vertices += mesh.positions.len();
    }
    vertices
}

#[test]
fn a_box_standing_on_the_ground_meshes_with_a_unit_normal_at_every_vertex() {
    // An 8 x 6 x 8 m block whose bottom face is the ground's plane, y = 2, and beside it a pit whose top face is.
    let vertices = every_normal_is_unit(
        &[a_box([16.0, 5.0, 16.0], [4.0, 3.0, 4.0], Mode::Add), a_box([5.0, 0.0, 5.0], [3.0, 2.0, 3.0], Mode::Carve)],
        2.0,
    );
    assert!(vertices > 100, "the block and the ground mesh ({vertices} vertices)");
}

#[test]
fn a_column_standing_on_a_carved_floor_meshes_with_a_unit_normal_at_every_vertex() {
    // A room carved under 10 m of ground, its floor at 2 m and its ceiling at 8, and a 3 x 3 m column from the floor's
    // plane to the ceiling's: #472's columns, standing where an author puts them.
    let vertices = every_normal_is_unit(
        &[a_box([16.0, 5.0, 16.0], [6.0, 3.0, 6.0], Mode::Carve), a_box([16.0, 5.0, 16.0], [1.5, 3.0, 1.5], Mode::Add)],
        10.0,
    );
    assert!(vertices > 100, "the room and its column mesh ({vertices} vertices)");
}
