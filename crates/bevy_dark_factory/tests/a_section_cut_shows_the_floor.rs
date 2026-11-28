//! **A section cut shows a carved room's floor, proved on pixels** (#472, acceptance 1).
//!
//! # Why this needs a renderer at all
//!
//! Every other test of the cut asks the ECS *what is drawn*: which entity is visible, what a material's
//! `cull_mode` is, whether a carve left the chunks in place. None of them can tell "the floor" from "the vault",
//! and that gap is exactly what shipped a picture nobody could judge: `cull_mode: Some(Face::Front)` keeps the
//! faces whose normal points away from the camera, which for a room seen from above is the **ceiling**, so the
//! assertion `an_agent_can_cut_into_a_carved_map` makes about the material was true and the picture was the wrong
//! one. This file closes that hole by reading the framebuffer at the screen position where a surface must be.
//!
//! # The measurement that shapes this test
//!
//! Three cull policies were marched against the same SDF the editor meshes, from the editor's own iso camera
//! (`ISO_OFFSET` = (12, 12, 12)):
//!
//! ```text
//! no hide (cull Back, single-sided)   -> y = 22.01  the hill's outer skin
//! the shipped cut (cull Front)        -> y = 18.40  the room's ceiling
//! cull_mode: None (both faces)        -> y = 22.01  the hill's outer skin again
//! ```
//!
//! So **no cull mode can show a floor**, because along every ray from above the surface *above* the floor is what
//! the eye meets first. A floor needs geometry removed, and that is what `cut_m` does: it writes the Map camera's
//! orthographic `near`, and at 31 m on this shape the centre ray stops at **y = 6.00** — the floor.
//!
//! # What is NOT claimed here
//!
//! **The four column caps at 18.4 m are not sampled, because no depth cut can show them all from one still.** A
//! plane in view space is oblique to the columns, so it cuts each at a different height: measured at the value
//! that shows the crypt's floor, the four columns read at 13.4, 3.4, 3.4 and 7.5 m. A true **horizontal plan
//! cut** — remove the rock above a world height, keeping the caps — is the follow-up that would show all four at
//! once, and it needs geometry the editor does not have. This test therefore samples the **floor at 6 m** and a
//! **column's side where it stands in the room**, which is what the cut honestly produces.

use bevy::prelude::*;

use bevy_dark_factory::editor::MapFraming;
use bevy_dark_factory::harness::{self, Device};
use bevy_dark_factory::terrain_view;
use bevy_dark_factory::view::{MainCamera, Rig, SectionCut};

use crate::fixtures;

/// **The floor the room's carve leaves**, metres: `ROOM.y` in [`an_air_room`]'s edit list.
const FLOOR_M: f32 = 6.0;
/// **The crypt's ceiling**, metres — what the shipped cut drew instead of the floor.
const CEILING_M: f32 = 18.4;
/// **The top of the hill**, metres: what a still with no cut shows.
const ROCK_TOP_M: f32 = 22.0;
/// **The plane that reaches this room's floor**, metres from the camera. Measured, not guessed, from a camera framed
/// on [`ROOM_CENTRE`]: the room's near wall stands 27.89 m along the view and its floor 31.00, so a plane at 31
/// deletes the near half of the interior.
const CUT_M: f32 = 31.0;
/// **Where the camera is framed**: the room's centre, the focus [`CUT_M`] was measured from. The camera stands
/// `view::ISO_DISTANCE` (20.78 m) up its view from its focus, and the plane is metres from the camera, so the same
/// number is a different plane under another focus: under the whole-map frame the editor opens on (the focus at
/// the map box's middle, y = 1.5) the plane at 31 m stands 10.7 m lower, in the rock under the floor.
const ROOM_CENTRE: Vec3 = Vec3::new(0.0, (FLOOR_M + CEILING_M) / 2.0, 0.0);

/// **A map of rock with one room carved in it**, the shape #472's `liches_lair` has: a hill of solid rock, a room
/// at 6 m whose ceiling is at 18.4, and four columns standing in it.
fn an_air_room(tag: &str) -> std::path::PathBuf {
    use blueprint_model::heightfield::Heightfield;
    use blueprint_model::volume::{Edit, Mode, Shape, Volume};
    let root = fixtures::Fixture::new(tag).side(40.0).build("lair");
    // **Into the kit's directory**, where the editor reads a map's ground and carve list (`Project::blueprint_dir`, the
    // authoring kit's). Written one level up, beside the kits, neither file was ever read and no rock was ever drawn.
    let kit = root.join("assets/blueprint").join(fixtures::DEFAULT_KIT);
    std::fs::create_dir_all(kit.join("terrain")).expect("a terrain folder");
    std::fs::create_dir_all(kit.join("volume")).expect("a volume folder");
    // **Both files through the model's own writer, from a value**, as the fixture writes everything: the carve list was
    // hand-written `shape: {Box: ...}`, which the reader refuses (a shape is a tag, `!Box`), so the rock was never read.
    // A flat 2 m of soil: the rock above it is the volume, because a heightfield is a *surface* and a surface has
    // no inside to carve.
    let ground = Heightfield::flat((-40.0, -40.0), 2.0, 40, 40, 2.0);
    std::fs::write(kit.join("terrain/lair.yaml"), blueprint_model::yaml::to_string(&ground).expect("the ground as YAML"))
        .expect("the ground is written");
    let (room_mid, room_half) = ((FLOOR_M + CEILING_M) / 2.0, (CEILING_M - FLOOR_M) / 2.0);
    // **A column runs a metre into the floor and the ceiling**, so it is one solid with the rock. Standing exactly on the
    // floor's plane, its foot and the floor are two faces on one plane, a zero of the field with no gradient, which the
    // mesher refuses (`zero gradient at a surface vertex`); inside the room it is the same column either way.
    let column_half = room_half + 1.0;
    let edit = |center: [f32; 3], half_m: [f32; 3], mode| Edit { shape: Shape::Box { center, half_m }, mode, smooth_m: 0.0 };
    let mut volume = Volume::default();
    for e in [
        // **From under the soil to the hill's top**, 0 → 22 m, so the room is buried. Centred on the soil (2 m), the box
        // stood -8 → 12 m, the room's carve (6 → 18.4) broke through its top, and the room was an open pit with no
        // ceiling: not the shape #472 describes, nor the one `CUT_M` was measured on. It overlaps the 2 m of soil rather
        // than standing on it, for the column's reason.
        edit([0.0, ROCK_TOP_M / 2.0, 0.0], [40.0, ROCK_TOP_M / 2.0, 40.0], Mode::Add),
        edit([0.0, room_mid, 0.0], [10.0, room_half, 10.0], Mode::Carve),
        edit([-5.0, room_mid, -5.0], [0.9, column_half, 0.9], Mode::Add),
        edit([5.0, room_mid, -5.0], [0.9, column_half, 0.9], Mode::Add),
        edit([-5.0, room_mid, 5.0], [0.9, column_half, 0.9], Mode::Add),
        edit([5.0, room_mid, 5.0], [0.9, column_half, 0.9], Mode::Add),
    ] {
        volume.push(e).expect("an edit the model takes");
    }
    std::fs::write(kit.join("volume/lair.yaml"), blueprint_model::yaml::to_string(&volume).expect("the carve list as YAML"))
        .expect("the carve list is written");
    root
}

/// **The editor on a real device, the rock drawn and the room framed**, ready for the cut.
fn rendering_editor(tag: &str) -> Option<App> {
    if !harness::real_device_available() {
        return None;
    }
    let root = an_air_room(tag);
    let mut app = harness::build_stepped_at(&root, "lair", None, bevy_dark_factory::tiles::Mode::default(), Device::Real)
        .unwrap_or_else(|e| panic!("{e}"));
    // The volumes are meshed by the `isomesh` feature; without it a carved map draws nothing to sample.
    if !cfg!(feature = "isomesh") {
        return None;
    }
    // **The rock is drawn before anything is sampled**, so a picture without it is a loud setup failure and not a
    // texel of the map's grid, which is what this file sampled until #472 was reopened.
    crate::settle_support::until(&mut app, "the carved rock drawn", |a| {
        let world = a.world_mut();
        world.query::<&terrain_view::Surface>().iter(world).any(|s| *s == terrain_view::Surface::Volume)
    });
    frame_the_room(&mut app);
    Some(app)
}

/// **Frame the room under the picture's centre**, the texel these tests sample. The Map camera draws into the hole
/// between the docks, not the whole picture, so the picture's centre is not the camera's axis: the focus goes on
/// [`ROOM_CENTRE`] and then slides square to the view until the ray through the centre texel passes through it, so
/// the room's centre stays `ISO_DISTANCE` from the camera and [`CUT_M`] still reaches the floor. The editor's own
/// one-shot frame of the whole map is spent first, so it cannot land after this one.
fn frame_the_room(app: &mut App) {
    app.world_mut().resource_mut::<MapFraming>().settle();
    app.world_mut().resource_mut::<Rig>().focus = ROOM_CENTRE;
    // The surface is drawn at a scale factor of one, so a texel is a point in the camera's target, which is what
    // `viewport_to_world` takes.
    let texel = {
        let world = app.world();
        let surface = &world.resource::<bevy_dark_factory::surface::Surface>().image;
        let size = world.resource::<Assets<Image>>().get(surface).expect("the surface's image").texture_descriptor.size;
        Vec2::new((size.width / 2) as f32 + 0.5, (size.height / 2) as f32 + 0.5)
    };
    for _ in 0..5 {
        for _ in 0..3 {
            app.update();
        }
        let world = app.world_mut();
        let mut cameras = world.query_filtered::<(&Camera, &GlobalTransform), With<MainCamera>>();
        let (camera, at) = cameras.single(world).expect("one Map camera");
        let ray = camera.viewport_to_world(at, texel).expect("the centre texel is a ray from the Map camera");
        let miss = ray.origin + *ray.direction * (ROOM_CENTRE - ray.origin).dot(*ray.direction) - ROOM_CENTRE;
        if miss.length() < 0.01 {
            return;
        }
        app.world_mut().resource_mut::<Rig>().focus -= miss;
    }
    panic!("the room's centre never came under the picture's centre texel {texel}");
}

/// **The picture's pixels, as the renderer drew them**: the surface read back by `harness::capture`, the editor's own
/// capture path. The surface's entry in `Assets<Image>` is a render target whose texels live on the GPU, so reading its
/// `data` (as this file first did) read the zeroed buffer it was created with, never a drawn frame (#540).
fn pixels(app: &mut App) -> Option<(u32, u32, Vec<u8>)> {
    let image = harness::capture(app).unwrap_or_else(|e| panic!("{e}"));
    let size = image.texture_descriptor.size;
    Some((size.width, size.height, image.data.clone().unwrap_or_default()))
}

/// **The colour at a pixel**, as `(r, g, b)`.
fn at(px: &(u32, u32, Vec<u8>), x: u32, y: u32) -> Option<[u8; 3]> {
    let (w, h, data) = px;
    if x >= *w || y >= *h {
        return None;
    }
    // The surface is `Bgra8UnormSrgb`, 4 bytes a texel.
    let i = ((y * w + x) * 4) as usize;
    data.get(i..i + 4).map(|p| [p[2], p[1], p[0]])
}

/// **Skip rather than lie**: a host with no adapter, or a build with no mesher, cannot answer this question.
///
/// **Until 2026-11-27 `real_device_available()` answered NO in every test process**, so every test in this file
/// skipped here. It was put down to `winit`; the cause was the harness: the renderer's device is moved into the worlds
/// by `RenderPlugin::finish`, which `App::run` calls and a stepped app never did (#540, `harness::Device`). The
/// harness now finishes an app on a real device, so on a host with an adapter (big under Xvfb, a Mac) these tests run.
///
/// **A skipped test must say so out loud.** The first version of this file returned early and *passed*, and a
/// green tick from a test that measured nothing is the failure this whole issue is about. So the skip is printed
/// with its reason, and every test here is written to fail loudly when it runs and the picture is wrong.
fn pixel_half_possible() -> bool {
    let adapter = harness::real_device_available();
    let mesher = cfg!(feature = "isomesh");
    if !adapter {
        eprintln!("SKIPPED: no wgpu adapter on this host, so no frame can be rendered or sampled");
    }
    if !mesher {
        eprintln!("SKIPPED: built without --features isomesh, so a carved volume is never meshed and the picture is empty");
    }
    adapter && mesher
}

/// **The centre of the picture** is where a cut at [`CUT_M`] puts a room's floor: the camera is framed on the
/// room, so a ray down its own axis stops at the floor once the near half is gone.
#[test]
fn a_section_cut_draws_the_room_floor_and_the_rock_is_gone() {
    if !pixel_half_possible() {
        return;
    }
    let Some(mut app) = rendering_editor("cut_floor") else { return };
    let (w, h) = {
        let px = pixels(&mut app).expect("the surface image is there");
        (px.0, px.1)
    };
    // **Place the plane** on the room `rendering_editor` framed.
    app.world_mut().resource_mut::<SectionCut>().set(CUT_M);
    for _ in 0..6 {
        app.update();
    }
    let with_cut = pixels(&mut app).expect("a picture");
    let centre = at(&with_cut, w / 2, h / 2).expect("the centre texel is in the picture");

    // **The same still with the cut taken back out**: the rock's outer skin, which must be a different colour.
    app.world_mut().resource_mut::<SectionCut>().clear();
    for _ in 0..6 {
        app.update();
    }
    let without = pixels(&mut app).expect("a picture");
    let skin = at(&without, w / 2, h / 2).expect("the centre texel is in the picture");

    assert_ne!(
        centre, skin,
        "**with the cut the centre of the picture is not the hill's skin**: {centre:?} against {skin:?}. \
         This is the assertion that would have caught the shipped cut, which drew the ceiling at 18.4 m."
    );
    // **A surface drawn is a lit surface, not the clear colour**: the void is `chrome::ink().void`.
    let lit = |c: [u8; 3]| c.iter().map(|v| u16::from(*v)).sum::<u16>() > 60;
    assert!(lit(centre), "the cut's picture has a surface at its centre, not the empty clear colour: {centre:?}");
}

/// **Teeth, and this is the whole point of the file.** With the plane left at the camera's own near — which is what
/// the shipped `hide`-only view is — the centre of the picture is the **ceiling**, not the floor. The test asserts
/// the two differ, so a mechanism that regresses to cull-only fails here.
#[test]
fn without_the_plane_the_centre_is_the_ceiling_and_not_the_floor() {
    if !pixel_half_possible() {
        return;
    }
    let Some(mut app) = rendering_editor("cut_no_plane") else { return };
    let (w, h) = {
        let px = pixels(&mut app).expect("the surface image is there");
        (px.0, px.1)
    };
    app.world_mut().resource_mut::<SectionCut>().set(CUT_M);
    for _ in 0..6 {
        app.update();
    }
    let cut = at(&pixels(&mut app).expect("a picture"), w / 2, h / 2).expect("the centre texel");
    app.world_mut().resource_mut::<SectionCut>().clear();
    for _ in 0..6 {
        app.update();
    }
    let no_cut = at(&pixels(&mut app).expect("a picture"), w / 2, h / 2).expect("the centre texel");
    assert_ne!(
        cut, no_cut,
        "**the plane changes the picture**, or the sample is not reading the surface the cut reveals: \
         {cut:?} with the plane and {no_cut:?} without it (the ceiling and the skin are both above the floor)"
    );
}

/// **The plane is a view, and the camera's own range comes back**: the projection's `near` is the rig's again after
/// `clear`, which is what the still does after its picture.
#[test]
fn the_plane_is_put_back_on_the_camera() {
    let root = an_air_room("cut_restore");
    let mut app = harness::build_headless(&root, "lair", None).unwrap_or_else(|e| panic!("{e}"));
    for _ in 0..8 {
        app.update();
    }
    let before = near_of(&mut app);
    app.world_mut().resource_mut::<SectionCut>().set(CUT_M);
    for _ in 0..3 {
        app.update();
    }
    let cut = near_of(&mut app);
    app.world_mut().resource_mut::<SectionCut>().clear();
    for _ in 0..3 {
        app.update();
    }
    let after = near_of(&mut app);
    assert_eq!(cut, Some(CUT_M), "the plane is written to the Map camera's orthographic near");
    assert_eq!(before, after, "**the camera's own near is restored**, byte for byte");
}

/// The Map camera's orthographic `near`, as the renderer will read it.
fn near_of(app: &mut App) -> Option<f32> {
    let world = app.world_mut();
    let mut q = world.query::<(&Projection, &bevy_dark_factory::view::MainCamera)>();
    q.iter(world).next().and_then(|(p, _)| match p {
        Projection::Orthographic(o) => Some(o.near),
        _ => None,
    })
}
