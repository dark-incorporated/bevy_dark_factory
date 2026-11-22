//! **The sculpted ground and the kept water, drawn in the viewport** (#99, #100).
//!
//! `terrain_stroke` and `water_run` write a heightfield and a water field into the project; until
//! now nothing showed them, so an author sculpted and ran water blind. This draws both from the
//! files, the same way a game would read them, and follows them when they change on disk (a stroke,
//! a run, an undo, an agent's edit): the files are polled twice a second and the meshes rebuilt only
//! when one's modification time moved.
//!
//! The ground sits [`crate::editor::ground_drop`] under the datum, like the bounds floor, so a floor
//! piece lying on flat ground is not z-fighting it. Water is a translucent quad per wet cell at
//! `ground + depth`.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::Face;
use blueprint_model::heightfield::Heightfield;
use blueprint_model::live::{KindVerdict, Outcome};
use blueprint_model::reload_check::{self, Check};
use blueprint_model::volume::Volume;
use blueprint_model::water::WaterField;

use crate::project::{OpenMap, Project};
use crate::project_index::{FileStamp, ProjectIndex, Snapshot as Listing};

/// Water shallower than this, metres, is not drawn: the simulation leaves a film on every wet slope.
pub const MIN_DRAWN_DEPTH_M: f32 = 0.02;

/// Water's tint: translucent, because the ground under it is part of the answer.
const WATER_TINT: [f32; 4] = [0.16, 0.42, 0.78, 0.62]; // CHROME-OK: world ink — a translucent water surface over the ground, not panel text

/// **How far past the field a carved ground reaches**, metres: one chunk each way, which is the padding
/// `volume_mesh` meshes over. Stated here rather than read from that module, which is behind `isomesh`
/// and is not compiled on every build — a number the depth range needs whatever the build.
const CARVE_PAD_M: f32 = 16.0;

/// Which of the two drawn things an entity is.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TerrainView {
    Ground,
    Water,
    /// One chunk of the carved ground (#101); drawn instead of [`TerrainView::Ground`] while the map has volume edits.
    Volume,
}

/// **The ground as a lit mesh**, one vertex per cell, tinted low (green) to high (tan) over the
/// field's own range so relief reads from above.
pub fn ground_mesh(ground: &Heightfield) -> Mesh {
    let (w, d) = (ground.width, ground.depth);
    let (lo, hi) = ground.range();
    let span = (hi - lo).max(1.0e-3);
    let at = |ix: usize, iz: usize| ground.heights[iz.min(d - 1) * w + ix.min(w - 1)];
    let mut positions = Vec::with_capacity(w * d);
    let mut normals = Vec::with_capacity(w * d);
    let mut colors: Vec<[f32; 4]> = Vec::with_capacity(w * d);
    for iz in 0..d {
        for ix in 0..w {
            let h = at(ix, iz);
            positions.push([ground.origin.0 + ix as f32 * ground.cell_m, h, ground.origin.1 + iz as f32 * ground.cell_m]);
            let dx = (at(ix + 1, iz) - at(ix.saturating_sub(1), iz)) / (2.0 * ground.cell_m);
            let dz = (at(ix, iz + 1) - at(ix, iz.saturating_sub(1))) / (2.0 * ground.cell_m);
            normals.push(Vec3::new(-dx, 1.0, -dz).normalize().to_array());
            let t = ((h - lo) / span).clamp(0.0, 1.0);
            colors.push([0.30 + 0.38 * t, 0.46 + 0.14 * t, 0.24 + 0.22 * t, 1.0]); // CHROME-OK: world ink — relief tint
        }
    }
    let mut indices = Vec::with_capacity((w - 1) * (d - 1) * 6);
    for iz in 0..d - 1 {
        for ix in 0..w - 1 {
            let a = (iz * w + ix) as u32;
            let (b, c, e) = (a + 1, a + w as u32, a + w as u32 + 1);
            indices.extend_from_slice(&[a, c, b, b, c, e]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

/// **The kept water as one mesh**: a flat quad over each cell deeper than [`MIN_DRAWN_DEPTH_M`], at
/// the ground's height there plus the depth. `None` when nothing is wet, or when the water was run on
/// a ground of another shape than this one.
pub fn water_mesh(ground: &Heightfield, water: &WaterField) -> Option<Mesh> {
    if (water.width, water.depth) != (ground.width, ground.depth) || water.depth_m.len() != ground.heights.len() {
        return None;
    }
    let half = water.cell_m * 0.5;
    let (mut positions, mut indices) = (Vec::new(), Vec::new());
    for iz in 0..water.depth {
        for ix in 0..water.width {
            let i = iz * water.width + ix;
            if water.depth_m[i] <= MIN_DRAWN_DEPTH_M {
                continue;
            }
            let (x, z, y) = (water.origin.0 + ix as f32 * water.cell_m, water.origin.1 + iz as f32 * water.cell_m, ground.heights[i] + water.depth_m[i]);
            let base = positions.len() as u32;
            positions.extend_from_slice(&[[x - half, y, z - half], [x + half, y, z - half], [x - half, y, z + half], [x + half, y, z + half]]);
            indices.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
        }
    }
    if positions.is_empty() {
        return None;
    }
    let n = positions.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; n]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![WATER_TINT; n]);
    mesh.insert_indices(Indices::U32(indices));
    Some(mesh)
}

/// What the drawn meshes were built from, the three files by the stamp the project index holds of them; a change in any of it rebuilds them.
#[derive(Default, PartialEq)]
pub struct Built {
    map_path: std::path::PathBuf,
    ground: Option<FileStamp>,
    water: Option<FileStamp>,
    volume: Option<FileStamp>,
    origin: [u32; 3],
    drop: u32,
}

/// **The stamp the index holds of the file at `path`**, `None` when it lists none (the file is missing, or the project's `.gitignore` names it).
/// A path outside the project root cannot be answered: it is said and read as no stamp.
fn stamp_of(project: &Project, listing: &Listing, path: &std::path::Path) -> Option<FileStamp> {
    match path.strip_prefix(&project.root) {
        Ok(rel) => listing.stamp(&rel.to_string_lossy().replace('\\', "/")),
        Err(_) => {
            error!("terrain view: {} is not under the project root {}, so the project index does not follow it", path.display(), project.root.display());
            None
        }
    }
}

/// The heightfield ground's plain material (its vertex colours are the relief tint); the carved ground's is [`volume_base`].
pub fn ground_base() -> StandardMaterial {
    StandardMaterial { perceptual_roughness: 1.0, ..default() }
}

/// The carved ground's plain material (#101). A carved chunk has no vertex colours, so what it wears is
/// this base alone: the heightfield's own relief tint, held at the middle of its range. Left white, the
/// carved ground looked unfinished and snow on it (`volume_cover`) could not be told from bare ground.
pub fn volume_base() -> StandardMaterial {
    StandardMaterial { base_color: Color::srgb(0.49, 0.53, 0.35), perceptual_roughness: 1.0, ..default() } // CHROME-OK: world ink — the relief tint's middle
}

/// An entity that is drawn every frame, far off screen, so the pipeline a carved chunk is drawn with is
/// compiled before the first carve (#268). Bevy does not draw an entity whose pipeline is still
/// compiling, and the first carve despawned the ground in the same frame it spawned the chunks: the
/// slab showed through for 0.2 s with the plain material and 0.5 s with the weathered one (measured on
/// big, 15 fps capture).
#[derive(Component)]
pub struct PipelineWarm;

/// **A one-triangle mesh with the vertex layout of a carved chunk** (position and normal, no colour,
/// `u32` indices), so its pipeline is the chunk's.
pub fn warm_mesh() -> Mesh {
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, vec![[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]]);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 1.0, 0.0]; 3]);
    mesh.insert_indices(Indices::U32(vec![0, 2, 1]));
    mesh
}

/// Far under the map, and out of its depth range, so the warm triangle is never seen.
pub const WARM_AT: Vec3 = Vec3::new(0.0, -1.0e5, 0.0);

/// **Keep the plain carved-ground pipeline compiled** (#268): one [`PipelineWarm`] triangle wearing
/// [`volume_base`], spawned entering the editor. `NoFrustumCulling`, because a culled entity queues no
/// pipeline.
pub fn warm_volume_pipeline(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.spawn((
        PipelineWarm,
        bevy::camera::visibility::NoFrustumCulling,
        Mesh3d(meshes.add(warm_mesh())),
        MeshMaterial3d(materials.add(volume_base())),
        Transform::from_translation(WARM_AT),
    ));
}

/// **The last good value of each file the view draws from**, kept in memory (#200): a refused file
/// leaves its slot as it was, so a bad edit never forces a reload from a bad file, and the drawing
/// can be rebuilt (the origin moved, the ground dropped) from what is known to be good.
#[derive(Default)]
pub struct Good {
    ground: Option<Heightfield>,
    water: Option<WaterField>,
    volume: Option<Volume>,
}

/// **Draw the open map's terrain and water**, and redraw when any of its files changes.
///
/// **A file that does not read is refused, and the last good value of it stays** (#187, #200). Each
/// file the view reads (the ground, the water, the carved volume) is decided on its own: one that
/// reads replaces its slot in [`Good`], one that is refused ([`Heightfield::parse`]: heights that do
/// not fill their shape, say) leaves it. The view is redrawn from the slots only when one of them, or
/// the map, its origin or the ground's drop, changed, so a refused edit leaves the entities drawn
/// before it untouched. A map that opens with a file refused has no earlier value and draws without
/// it. Each edit is one decision, recorded in [`crate::live::EditorReloads`] with the file and the
/// field it is wrong at and said once on the status line, because **the files are only read again when the project index says one's stamp
/// moved** (#538; the half-second poll is retired): a frame where the index's generation, the map and the project are as they were looks at
/// nothing. **A change made elsewhere reaches the view after the index's sweep and debounce, up to about a second [INFERENCE: the index's own
/// numbers, not measured on this view]**, and a file the project's `.gitignore` names is read when the view looks but not followed.
#[allow(clippy::too_many_arguments)]
pub fn sync_terrain_view(
    index: Res<ProjectIndex>,
    project: Res<Project>,
    open: Res<OpenMap>,
    mut state: ResMut<crate::editor::EditorState>,
    mut reloads: Option<ResMut<crate::live::EditorReloads>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut drawn_state: Local<(u64, Option<Built>, Vec<Entity>, Good, Option<String>)>,
    mut extent: ResMut<crate::view::DrawnExtent>,
) {
    let (seen_generation, built, drawn, good, said) = &mut *drawn_state;
    if built.is_some() && *seen_generation == index.generation() && !project.is_changed() && !open.is_changed() {
        return;
    }
    let Some(listing) = index.snapshot() else {
        // No listing yet is the index still walking; no listing with a failure is the root that cannot be read.
        if let Some(why) = index.failure()
            && said.as_deref() != Some(why.as_str())
        {
            warn!("terrain view: the project index has no listing ({why}), so the ground, water and volume files are not read");
            *said = Some(why);
        }
        return;
    };
    *said = None;
    *seen_generation = listing.generation;
    let name = open.map_path.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_suffix(".map.yaml")).unwrap_or_default().to_owned();
    let ground_path = project.blueprint_dir.join("terrain").join(format!("{name}.yaml"));
    let water_path = project.blueprint_dir.join("water").join(format!("{name}.field.yaml"));
    let volume_path = project.blueprint_dir.join("volume").join(format!("{name}.yaml"));
    let drop = crate::editor::ground_drop(&project);
    let now = Built {
        map_path: open.map_path.clone(),
        ground: stamp_of(&project, &listing, &ground_path),
        water: stamp_of(&project, &listing, &water_path),
        volume: stamp_of(&project, &listing, &volume_path),
        origin: [open.map.origin.0.to_bits(), open.map.origin.1.to_bits(), open.map.origin.2.to_bits()],
        drop: drop.to_bits(),
    };
    if built.as_ref() == Some(&now) {
        return;
    }
    // Another map has no earlier value of its own: nothing carries over.
    if built.as_ref().is_none_or(|b| b.map_path != now.map_path) {
        *good = Good::default();
    }
    let mut changed = built.as_ref().is_none_or(|b| (b.origin, b.drop) != (now.origin, now.drop));
    // Read each file that exists, and decide it: its value, or the refusal that names file and field.
    let ground = read_kind("terrain", &project, &ground_path, now.ground, Heightfield::parse);
    let water = read_kind("water_field", &project, &water_path, now.water, WaterField::parse);
    let volume = read_kind("volume", &project, &volume_path, now.volume, Volume::parse);
    let verdicts: Vec<KindVerdict> = ground
        .iter()
        .map(|(_, v)| v)
        .chain(water.iter().map(|(_, v)| v))
        .chain(volume.iter().map(|(_, v)| v))
        .cloned()
        .collect();
    if let Some(reloads) = reloads.as_deref_mut() {
        // One decision lists every file read, so a file that is gone is no longer refused.
        reloads.forget(&["terrain", "water_field", "volume"]);
        reloads.decide(verdicts);
    }
    for said in [
        refusal(&ground, good.ground.is_some(), "ground"),
        refusal(&water, good.water.is_some(), "water"),
        refusal(&volume, good.volume.is_some(), "carved ground"),
    ]
    .into_iter()
    .flatten()
    {
        state.status.problem(said);
    }
    adopt(&mut good.ground, ground, &mut changed);
    adopt(&mut good.water, water, &mut changed);
    adopt(&mut good.volume, volume, &mut changed);
    *built = Some(now);
    // **The box the drawing reaches, published whether or not it was redrawn.** The camera's depth range
    // is measured over it (`crate::view::depth_range_over`), so the ground's own origin and height must be
    // known even on the frames this system returns early on.
    let want = extent_of(&open, good.ground.as_ref(), good.volume.as_ref(), drop);
    if extent.0 != want {
        extent.0 = want;
    }
    if !changed {
        // Last good wins: what is drawn is what these values draw, so leave it.
        return;
    }
    for e in drawn.drain(..) {
        commands.entity(e).despawn();
    }
    if let Some(ground) = &good.ground {
        let at = Transform::from_xyz(open.map.origin.0, open.map.origin.1 - drop, open.map.origin.2);
        let carved = volume_chunks(good.volume.as_ref(), ground);
        let material = materials.add(if carved.is_empty() { ground_base() } else { volume_base() });
        if carved.is_empty() {
            drawn.push(commands.spawn((TerrainView::Ground, Surface::Terrain, Mesh3d(meshes.add(ground_mesh(ground))), MeshMaterial3d(material), at)).id());
        } else {
            for mesh in carved {
                drawn.push(commands.spawn((TerrainView::Volume, Surface::Volume, Mesh3d(meshes.add(mesh)), MeshMaterial3d(material.clone()), at)).id());
            }
        }
        if let Some(mesh) = good.water.as_ref().and_then(|w| water_mesh(ground, w)) {
            drawn.push(commands.spawn((
                TerrainView::Water,
                Surface::Water,
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(materials.add(StandardMaterial { alpha_mode: AlphaMode::Blend, perceptual_roughness: 0.15, ..default() })),
                // The ground is dropped; the water is measured from the datum like pieces are.
                Transform::from_xyz(open.map.origin.0, open.map.origin.1, open.map.origin.2),
            )).id());
        }
    }
}

/// **The world box the ground and water reach**, centre then size, as a [`crate::view::DrawnExtent`]
/// carries it: `None` when no ground is drawn.
///
/// The heightfield is authored on its own grid, so this is not the map's `bounds`: on `liches_lair` the
/// field covers `0..80 m` against bounds of `-40..40`, and the drawn ground stands on the field's
/// extent, dropped by the same amount the entity's `Transform` uses. The height is the field's own
/// range plus the datum the drop is measured from. **The volume's carve padding is not read here**: a
/// carve reaches one chunk past the field, which [`DEPTH_SLACK`](crate::view) already covers.
fn extent_of(open: &OpenMap, ground: Option<&Heightfield>, volume: Option<&Volume>, drop: f32) -> Option<(Vec3, Vec3)> {
    let ground = ground?;
    let origin = (open.map.origin.0 + ground.origin.0, open.map.origin.1 - drop, open.map.origin.2 + ground.origin.1);
    let span_x = (ground.width.saturating_sub(1)) as f32 * ground.cell_m;
    let span_z = (ground.depth.saturating_sub(1)) as f32 * ground.cell_m;
    let (mut lo, mut hi) = ground.range();
    // **The carved ground can add above the field and carve below it**, and `volume_mesh` pads one chunk
    // each way, so the carved box is a little taller than the field's own range while it is drawn.
    if volume.is_some_and(|v| !v.is_empty()) {
        lo -= CARVE_PAD_M;
        hi += CARVE_PAD_M;
    }
    Some((
        Vec3::new(origin.0 + span_x * 0.5, origin.1 + (lo + hi) * 0.5, origin.2 + span_z * 0.5),
        Vec3::new(span_x, hi - lo, span_z),
    ))
}

/// **Which of the map's own surfaces a drawn entity is**, by the name [`crate::outliner::HIDABLE`] gives
/// it (#472): the surface, not the view kind, because the answer an agent hides by is the *ground* or
/// the *carved ground*, and which of those an entity is drawn as can change with a single carve.
///
/// **It is a component and not a field of [`TerrainView`]** because the two answer different questions:
/// `TerrainView` says what is drawn (a heightfield, a volume chunk, water) and a test reading it is
/// asking about the drawing; this says which switch hides it, and a `TerrainView::Ground` entity is
/// still `terrain` after a carve replaced it with `Volume` chunks.
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Surface {
    Terrain,
    Volume,
    Water,
}

impl Surface {
    /// The name [`crate::outliner::HIDABLE`] gives it, and what `blueprint/surface_hide`'s `hide` and the
    /// still's own `hide` parameter say.
    pub fn name(self) -> &'static str {
        match self {
            Surface::Terrain => "terrain",
            Surface::Volume => "volume",
            Surface::Water => "water",
        }
    }
}

/// **Hide the map's own surfaces the eye has taken out, and cut the ground's front away** (#472).
///
/// **One writer of these entities' `Visibility` and of their material's `cull_mode` and
/// `double_sided`**, for `hide_carried`'s reason: a second writer of one component would take turns
/// undoing the first.
///
/// # What `terrain` hidden means
///
/// A carved map's rock is meshed as **one solid** by [`crate::volume_mesh`]: the hill's outer skin and
/// the carved rooms inside it are one surface, so despawning it takes the rooms with it. So `terrain`
/// hidden **culls the solid's front faces**, which leaves the faces that bound the *void* drawn — the
/// crypt's floor, its walls, its vault, and the sides of the columns standing in it. That is "the
/// carved void, and nothing else, from the same camera", with no second mesh and no cut plane's magic
/// height.
///
/// **And those faces must be lit, or the picture is the dark shape this issue rejects.** A face kept by
/// culling *front* faces is back-facing to the camera, and `StandardMaterial`'s `double_sided` is
/// `false` by default: the shader leaves a back face's normal pointing into the rock, so the face is
/// lit from the wrong side and reads as a shadow rather than as a floor. Measured on the issue's own
/// words — the agent "mistook the dark olive shapes for a floor". So the cut **also sets
/// `double_sided`**, which makes the shader flip the normal for a back face and light it from the side
/// it is actually seen from. Both values are written together and read back by
/// `an_agent_can_cut_into_a_carved_map`'s material assertions, because the two are one answer.
///
/// **A map with no volume has no inside to show**, so its ground is simply hidden.
///
/// `volume` and `water` are plain visibility: they are separate entities already, and hiding `volume`
/// takes the carved chunks out whether the ground is cut or not.
pub fn hide_hidden_surfaces(
    eyes: Option<Res<crate::outliner::Eyes>>,
    mut plain_materials: ResMut<Assets<StandardMaterial>>,
    mut plain: Query<(&Surface, &MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
) {
    let hidden = |s: Surface| eyes.as_deref().is_some_and(|e| e.surface_hidden(s.name()));
    // **The solid is cut, not hidden**: the chunks still draw, and what they draw is its inside.
    let cut = hidden(Surface::Terrain);
    for (surface, material, mut vis) in &mut plain {
        let want = if surface_wants(*surface, cut, hidden(*surface)) { Visibility::Hidden } else { Visibility::Inherited };
        if *vis != want {
            *vis = want;
        }
        // **A material is shared between entities, so its cull mode is one answer for the drawing**:
        // `cut` is one flag for the whole drawing, so every chunk entity writes the same value and they
        // cannot disagree. `get_mut` re-prepares, so it is written only when it differs.
        if *surface == Surface::Volume
            && let Some(mut m) = plain_materials.get_mut(&material.0)
        {
            let (want_cull, want_two_sided) = interior_faces(cut);
            if m.cull_mode != want_cull || m.double_sided != want_two_sided {
                m.cull_mode = want_cull;
                m.double_sided = want_two_sided;
            }
        }
    }
}

/// **How the carved ground's faces are drawn**: `(cull_mode, double_sided)` for the eye's state.
///
/// **Uncut is the plain drawing** — back faces culled, single-sided — so a map nobody has hidden is
/// exactly what it was before #472. **Cut** keeps the back faces, which are the ones bounding the void,
/// and makes the material two-sided so the shader flips their normals and lights them from the side
/// the camera is on: without it a ceiling seen from above is a face lit from underneath, which is the
/// "dark shape that reads as a floor" the issue names.
///
/// **One function, because the two values are one answer**, and because
/// [`crate::environment::cut_the_weathered_ground`] must reach the same pair for a map wearing a cover.
pub fn interior_faces(cut: bool) -> (Option<Face>, bool) {
    if cut {
        (Some(Face::Front), true)
    } else {
        (Some(Face::Back), false)
    }
}

/// **Whether a drawn surface is hidden, given whether the solid is being cut** (#472): the one rule
/// [`hide_hidden_surfaces`] and [`crate::environment::cut_the_weathered_ground`] both follow, so a map
/// wearing a cover and one without cannot disagree about what the eye did.
pub fn surface_wants(surface: Surface, cut: bool, hidden: bool) -> bool {
    match (surface, cut, hidden) {
        // **The terrain hidden over a carved map** leaves the chunks drawn, cut: the interior is the
        // answer, and hiding the chunks with the skin would be a blank map.
        (Surface::Volume, true, false) => false,
        (_, _, true) => true,
        _ => false,
    }
}

/// What the status line says about a refused file: which file and field, and what becomes of the
/// drawing. `kept` is whether an earlier good value of it is held. `None` for a file that read, or
/// that is not there.
fn refusal<T>(read: &Option<(Option<T>, KindVerdict)>, kept: bool, what: &str) -> Option<String> {
    let (_, verdict) = read.as_ref()?;
    if verdict.outcome != Outcome::Refused {
        return None;
    }
    let sentence = verdict.sentence();
    Some(if kept {
        format!("{sentence}: refused, so the earlier {what} stays")
    } else {
        format!("{sentence}: refused, so no {what} is drawn from it")
    })
}

/// **Keep the last good value of one file**: a file that read replaces the slot, a refused one leaves
/// it, a file that is gone empties it. `changed` is set when the slot's value is now another one.
fn adopt<T: PartialEq>(slot: &mut Option<T>, read: Option<(Option<T>, KindVerdict)>, changed: &mut bool) {
    match read {
        None => *changed |= slot.take().is_some(),
        Some((Some(value), _)) => {
            if slot.as_ref() != Some(&value) {
                *slot = Some(value);
                *changed = true;
            }
        }
        Some((None, _)) => {}
    }
}

/// **One file the view reads**, decided: `None` when it does not exist; otherwise its value when it
/// reads, and the verdict either way. A refusal names the file as the project's directory sees it
/// (`site/terrain/yard.yaml`) and the field the file is wrong at. **A file that cannot be read for a reason other than being absent is refused**,
/// naming why, and what is drawn stays. The file is read whether or not the index lists it (`stamp` is `None` then); that is said.
fn read_kind<T>(
    kind: &'static str,
    project: &Project,
    path: &std::path::Path,
    stamp: Option<FileStamp>,
    parse: fn(&str) -> Result<T, String>,
) -> Option<(Option<T>, KindVerdict)> {
    let file = path.strip_prefix(&project.project_dir).unwrap_or(path).to_string_lossy().replace('\\', "/");
    let text = match std::fs::read_to_string(path) {
        Ok(text) => {
            if stamp.is_none() {
                warn!("terrain view: {file} is not in the project index (the project's .gitignore names it?), so it is read now and later edits to it are not followed");
            }
            text
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
        Err(e) => return Some((None, KindVerdict::refused(kind, file, None, e.to_string()))),
    };
    Some(match parse(&text) {
        Ok(value) => (Some(value), KindVerdict::applied(kind, file)),
        Err(reason) => {
            let field = blueprint_model::reload::kind_by_id(kind).and_then(|k| match reload_check::check(k, &file, &text) {
                Check::Refused(r) => r.field,
                _ => None,
            });
            (None, KindVerdict::refused(kind, file, field, reason))
        }
    })
}

/// **The carved ground's chunk meshes** (#101): empty when the map has no volume edits, or when the
/// `isomesh` feature is off, and the heightfield mesh is drawn instead.
#[cfg(feature = "isomesh")]
fn volume_chunks(volume: Option<&Volume>, ground: &Heightfield) -> Vec<Mesh> {
    let Some(volume) = volume else { return Vec::new() };
    let buffers = crate::volume_mesh::mesh_volume(volume, ground).unwrap_or_default();
    buffers
        .into_iter()
        .map(|b| {
            let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, b.positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, b.normals);
            mesh.insert_indices(Indices::U32(b.indices));
            mesh
        })
        .collect()
}

#[cfg(not(feature = "isomesh"))]
fn volume_chunks(_: Option<&Volume>, _: &Heightfield) -> Vec<Mesh> {
    Vec::new()
}
