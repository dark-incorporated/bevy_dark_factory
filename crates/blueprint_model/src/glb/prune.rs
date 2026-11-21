//! **Keeping one part of a kit export, as a `.glb`** (#503).
//!
//! A kit arrives as one file whose furniture is placed by node translation, so importing it whole puts
//! a chair-and-desk in the palette as one 4 m blob. [`super::Glb::keep_only`] is the choice the author
//! needed: which node, and a file holding that node's subtree and nothing else.
//!
//! # Why this is a JSON edit and a buffer rebuild
//!
//! `blueprint_model` refuses a `bevy` dependency (`tests/engine_free.rs`) and Bevy's glTF writer is not
//! available to it, so the two honest routes were a JSON-only prune here or a descriptor naming a node
//! index and loading the whole file for every piece (`GltfAssetLabel::Node`, which bevy registers for
//! every node). The second is smaller and carries a cost the author meets on every placement: the whole
//! file still loads, once per piece that names it. This is the first.
//!
//! Everything a glTF file addresses is an index, so keeping a part is renumbering. The order matters:
//!
//! 1. **the kept node set** — the chosen node and its descendants, and each one's ancestors, because a
//!    node's place in the graph is its transform;
//! 2. **the meshes** those nodes name, and the accessors and buffer views those meshes use;
//! 3. **the materials** those primitives name, and the textures, images and samplers those materials
//!    sample;
//! 4. **the buffer**, rebuilt as the surviving buffer views' bytes with every `byteOffset` rewritten.
//!
//! A mesh nothing names, a material no surviving primitive uses and a texture no surviving material
//! samples are dropped, which is the cascade a person expects: choosing a chair must not carry the
//! desk's geometry into the palette as dead weight.
//!
//! # The chosen node's transform is baked
//!
//! `extent`, `align.pivot` and `align.y_offset` are all measured from the file's origin, so a part left
//! standing at x = 2 m measures 1.6 m wide and sits two metres off its own footprint — the exact
//! failure `import::measure`'s origin finding describes for a hand-exported mesh. The chosen node's
//! composed world matrix is applied to its vertices and its node transform reset to the identity: the
//! same thing an exporter means by "apply transforms", and for the same reason.
//!
//! **A rotated node makes the baked box larger than the part**, exactly as `bounds` says of its own
//! union, and for the same reason: an axis-aligned box is what a reservation can hold.
//!
//! # What is not preserved
//!
//! Skins and animations. A part a person chose out of a kit is a piece of set dressing, and a rig is
//! `crates/bevy_blueprint_anim`'s subject, with its own reader and its own checks (`rig_check`). A file
//! carrying either is refused by name rather than written without them, because a silently unrigged
//! mesh is worse than a refusal.

use serde_json::{json, Map, Value};

use super::{apply, mul, node_matrix, Glb, IDENTITY};

/// One of a `.glb`'s top-level arrays, or an empty one.
fn array(json: &Value, key: &str) -> Vec<Value> {
    json.get(key).and_then(Value::as_array).cloned().unwrap_or_default()
}

/// **The chosen node, its descendants, and its ancestors** — every node the kept file must hold.
///
/// Ancestors are in because a node's place in the graph is its transform: dropping a parent would put a
/// child at the origin. Descendants are in because they are what the node *is* — a chair assembled from
/// a seat, a back and four legs is one node with six children.
fn kept_nodes(json: &Value, chosen: usize) -> Vec<usize> {
    let nodes = array(json, "nodes");
    let mut keep: Vec<usize> = Vec::new();

    // Downwards: the chosen node and everything under it. A budget, because a malformed file can
    // describe a cycle and listing a part must not hang on one.
    let mut stack = vec![chosen];
    let mut budget = 100_000usize;
    while let Some(ix) = stack.pop() {
        let Some(node) = nodes.get(ix) else { continue };
        if !keep.contains(&ix) {
            keep.push(ix);
        }
        budget = budget.saturating_sub(1);
        if budget == 0 {
            break;
        }
        for kid in node.get("children").and_then(Value::as_array).into_iter().flatten() {
            if let Some(k) = kid.as_u64() {
                stack.push(k as usize);
            }
        }
    }

    // Upwards: a parent's transform is part of where the chosen node stands.
    for (ix, _) in nodes.iter().enumerate() {
        if keep.contains(&ix) {
            continue;
        }
        let is_ancestor = nodes[ix]
            .get("children")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_u64)
            .any(|k| keep.contains(&(k as usize)));
        if is_ancestor {
            keep.push(ix);
        }
    }
    keep.sort_unstable();
    keep
}

/// **The world matrix of a node**, composed down from the scene's roots.
///
/// `super::Glb::for_each_mesh_node` visits meshes, not nodes, so it cannot answer this; the arithmetic
/// is [`node_matrix`] and [`mul`], the same two functions `bounds` composes with, so a baked transform
/// cannot disagree with the measurement that motivated baking it.
fn world_of(json: &Value, target: usize) -> Result<[f32; 16], String> {
    let nodes = array(json, "nodes");
    let roots: Vec<usize> = json
        .get("scenes")
        .and_then(Value::as_array)
        .and_then(|scenes| {
            let ix = json["scene"].as_u64().unwrap_or(0) as usize;
            scenes.get(ix)
        })
        .and_then(|s| s["nodes"].as_array())
        .map(|n| n.iter().filter_map(Value::as_u64).map(|v| v as usize).collect())
        .unwrap_or_else(|| (0..nodes.len()).collect());

    let mut stack: Vec<(usize, [f32; 16])> = roots.into_iter().map(|n| (n, IDENTITY)).collect();
    let mut budget = 100_000usize;
    while let Some((ix, parent)) = stack.pop() {
        budget = budget
            .checked_sub(1)
            .ok_or_else(|| "glb: node hierarchy is cyclic or absurdly deep".to_owned())?;
        let Some(node) = nodes.get(ix) else { continue };
        let world = mul(&parent, &node_matrix(node));
        if ix == target {
            return Ok(world);
        }
        for kid in node.get("children").and_then(Value::as_array).into_iter().flatten() {
            if let Some(k) = kid.as_u64() {
                stack.push((k as usize, world));
            }
        }
    }
    Err(format!("glb: node {target} is not reachable from the scene"))
}

/// **An old-index-to-new-index map**: position in `kept` is the new index, and an index not kept has
/// none — which is what makes a dangling reference a lookup that answers `None` rather than a number
/// that happens to be in range.
fn index_map(kept: &[usize]) -> Vec<Option<usize>> {
    let mut map = vec![None; kept.iter().max().map_or(0, |m| m + 1)];
    for (new, old) in kept.iter().enumerate() {
        map[*old] = Some(new);
    }
    map
}

/// **`kept`, in the order first seen**, so a list built by walking primitives has a stable output
/// order and the map is built once.
fn note(list: &mut Vec<usize>, ix: usize) {
    if !list.contains(&ix) {
        list.push(ix);
    }
}

/// **Keep `chosen` and its subtree as a whole `.glb`.** See the module documentation.
pub(super) fn keep_only(glb: &Glb, chosen: usize) -> Result<Vec<u8>, String> {
    let json = &glb.json;

    // **A rig or an animation is refused rather than written without one.** A part chosen out of a kit
    // is set dressing; a rig has its own reader (`bevy_blueprint_anim`) and its own checks
    // (`rig_check`), and a mesh that quietly lost its skeleton is worse than a refusal.
    if !array(json, "skins").is_empty() || !array(json, "animations").is_empty() {
        return Err(
            "glb: this file carries a skin or an animation, and keeping one part of it would write a \
             mesh that had quietly lost its rig. A rigged model is imported whole, through the rig's \
             own reader."
                .to_owned(),
        );
    }

    let nodes = array(json, "nodes");
    let keep = kept_nodes(json, chosen);

    // ── what the kept nodes address ─────────────────────────────────────────────────────────────────
    let mut meshes: Vec<usize> = Vec::new();
    for ix in &keep {
        let Some(m) = nodes[*ix].get("mesh").and_then(Value::as_u64) else { continue };
        note(&mut meshes, m as usize);
    }
    if meshes.is_empty() {
        let name = nodes.get(chosen).and_then(|n| n["name"].as_str()).unwrap_or("this node");
        return Err(format!(
            "glb: `{name}` (node {chosen}) names no mesh, so there is no geometry to keep"
        ));
    }

    let accessors_all = array(json, "accessors");
    let materials_all = array(json, "materials");
    let textures_all = array(json, "textures");
    let mut accessors: Vec<usize> = Vec::new();
    let mut materials: Vec<usize> = Vec::new();
    let mut textures: Vec<usize> = Vec::new();
    let mut images: Vec<usize> = Vec::new();
    let mut samplers: Vec<usize> = Vec::new();

    for m in &meshes {
        let Some(prims) = json["meshes"][*m]["primitives"].as_array() else {
            return Err(format!("glb: mesh {m} has no primitives"));
        };
        for prim in prims {
            // **Every attribute**, not only POSITION: a normals accessor left behind is a mesh with no
            // shading, and this loop is where that would happen.
            for (_name, ix) in prim["attributes"].as_object().into_iter().flatten() {
                if let Some(ix) = ix.as_u64() {
                    note(&mut accessors, ix as usize);
                }
            }
            // A morph target's accessors are its own list, and dropping them silently changes the
            // mesh. Refused by name rather than pruned: this writer does not carry them.
            if prim.get("targets").is_some_and(|t| t.as_array().is_some_and(|a| !a.is_empty())) {
                return Err(
                    "glb: this mesh has morph targets, and keeping one part of it would write a mesh \
                     whose shapes were dropped. Import the file whole."
                        .to_owned(),
                );
            }
            if let Some(i) = prim.get("indices").and_then(Value::as_u64) {
                note(&mut accessors, i as usize);
            }
            if let Some(mi) = prim.get("material").and_then(Value::as_u64) {
                note(&mut materials, mi as usize);
            }
        }
    }

    // A material's textures, then their images and samplers.
    for mi in &materials {
        for (_slot, tex) in materials_all[*mi].as_object().into_iter().flatten() {
            let Some(index) = tex.get("index").and_then(Value::as_u64) else { continue };
            note(&mut textures, index as usize);
        }
    }
    for ti in &textures {
        if let Some(source) = textures_all[*ti].get("source").and_then(Value::as_u64) {
            note(&mut images, source as usize);
        }
        if let Some(s) = textures_all[*ti].get("sampler").and_then(Value::as_u64) {
            note(&mut samplers, s as usize);
        }
    }

    // **The buffer views the kept accessors read from, and an image's own embedded view.**
    let views_all = array(json, "bufferViews");
    let images_all = array(json, "images");
    let mut views: Vec<usize> = Vec::new();
    for a in &accessors {
        if let Some(v) = accessors_all[*a].get("bufferView").and_then(Value::as_u64) {
            note(&mut views, v as usize);
        }
    }
    for i in &images {
        if let Some(v) = images_all[*i].get("bufferView").and_then(Value::as_u64) {
            note(&mut views, v as usize);
        }
    }

    let buffers_all = array(json, "buffers");
    if buffers_all.len() > 1 {
        return Err(format!(
            "glb: this file has {} buffers, and a `.glb` this reader can keep a part of has one \
             embedded buffer",
            buffers_all.len()
        ));
    }

    // ── the vertices, placed where the graph has them ───────────────────────────────────────────────
    //
    // **Read before anything is renumbered**, so this is the source file's own bytes and the chosen
    // node's own accessors.
    let world = world_of(json, chosen)?;
    let baked = match nodes[chosen]["mesh"].as_u64() {
        Some(m) => positioned_positions(glb, m as usize, &world)?,
        None => return Err(format!("glb: node {chosen} names no mesh")),
    };

    // ── the buffer, rebuilt ─────────────────────────────────────────────────────────────────────────
    let mut new_bin: Vec<u8> = Vec::new();
    let mut view_at: Vec<(usize, usize)> = Vec::with_capacity(views.len());
    for v in &views {
        let view = &views_all[*v];
        let buffer_ix = view["buffer"].as_u64().unwrap_or(0) as usize;
        if buffer_ix != 0 {
            return Err(format!(
                "glb: buffer view {v} reads buffer {buffer_ix}, and a `.glb` this reader keeps a part \
                 of has one embedded buffer"
            ));
        }
        let start = view["byteOffset"].as_u64().unwrap_or(0) as usize;
        let len = view["byteLength"].as_u64().unwrap_or(0) as usize;
        let end = start
            .checked_add(len)
            .ok_or_else(|| format!("glb: buffer view {v} runs past the end of the buffer"))?;
        let bytes = glb
            .bin
            .get(start..end)
            .ok_or_else(|| format!("glb: buffer view {v} runs past the BIN chunk"))?;
        // **Four-byte alignment**, which glTF requires of a view an accessor reads floats from. A copy
        // that landed a float at an odd offset reads as garbage that looks like geometry.
        while !new_bin.len().is_multiple_of(4) {
            new_bin.push(0);
        }
        view_at.push((*v, new_bin.len()));
        new_bin.extend_from_slice(bytes);
    }

    // ── the document ────────────────────────────────────────────────────────────────────────────────
    let mut out: Map<String, Value> = Map::new();
    out.insert("asset".into(), json!({"version": "2.0"}));
    out.insert("scene".into(), json!(0));
    out.insert("scenes".into(), json!([{"nodes": [chosen]}]));

    let node_map = index_map(&keep);
    let mesh_map = index_map(&meshes);
    let accessor_map = index_map(&accessors);
    let view_map = index_map(&views);
    let material_map = index_map(&materials);
    let texture_map = index_map(&textures);
    let image_map = index_map(&images);
    let sampler_map = index_map(&samplers);

    // **The nodes**: the kept set, with `mesh` and `children` renumbered, and the chosen node's own
    // transform reset because its geometry has been baked into the vertices.
    let mut new_nodes: Vec<Value> = Vec::with_capacity(keep.len());
    for ix in &keep {
        let mut node = nodes[*ix].as_object().cloned().unwrap_or_default();
        if *ix == chosen {
            node.remove("matrix");
            node.remove("translation");
            node.remove("rotation");
            node.remove("scale");
        }
        if let Some(old) = node.get("mesh").and_then(Value::as_u64) {
            match mesh_map.get(old as usize).copied().flatten() {
                Some(new) => node.insert("mesh".into(), json!(new)),
                None => return Err(format!("glb: a kept node names mesh {old}, which did not survive the prune")),
            };
        }
        if let Some(kids) = node.get("children").and_then(Value::as_array) {
            let kept: Vec<Value> = kids
                .iter()
                .filter_map(Value::as_u64)
                .filter_map(|k| node_map.get(k as usize).copied().flatten())
                .map(|k| json!(k))
                .collect();
            if kept.is_empty() {
                node.remove("children");
            } else {
                node.insert("children".into(), Value::Array(kept));
            }
        }
        new_nodes.push(Value::Object(node));
    }
    out.insert("nodes".into(), Value::Array(new_nodes));

    // **The meshes**: their primitives' accessors and material renumbered. The chosen mesh's POSITION
    // accessor is a fresh one, written after this loop, because its bytes are new.
    let mut new_meshes: Vec<Value> = Vec::with_capacity(meshes.len());
    for m in &meshes {
        let mut mesh = json["meshes"][*m].as_object().cloned().unwrap_or_default();
        if let Some(prims) = mesh.get_mut("primitives").and_then(Value::as_array_mut) {
            for prim in prims {
                if let Some(attrs) = prim.get_mut("attributes").and_then(Value::as_object_mut) {
                    for (_name, ix) in attrs.iter_mut() {
                        let Some(old) = ix.as_u64() else { continue };
                        match accessor_map.get(old as usize).copied().flatten() {
                            Some(new) => *ix = json!(new),
                            // A kept mesh's attribute that did not survive is a mesh with a hole in
                            // it; refusing names it rather than writing half a primitive.
                            None => return Err(format!("glb: mesh {m} reads accessor {old}, which did not survive the prune")),
                        }
                    }
                }
                if let Some(old) = prim.get("indices").and_then(Value::as_u64) {
                    match accessor_map.get(old as usize).copied().flatten() {
                        Some(new) => prim["indices"] = json!(new),
                        None => return Err(format!("glb: mesh {m} reads index accessor {old}, which did not survive the prune")),
                    }
                }
                if let Some(old) = prim.get("material").and_then(Value::as_u64) {
                    match material_map.get(old as usize).copied().flatten() {
                        Some(new) => prim["material"] = json!(new),
                        None => {
                            prim.as_object_mut().map(|o| o.remove("material"));
                        }
                    }
                }
            }
        }
        new_meshes.push(Value::Object(mesh));
    }

    // ── the baked vertices ──────────────────────────────────────────────────────────────────────────
    //
    // **One fresh accessor per primitive of the chosen mesh**, appended after the kept ones, with its
    // own view at the end of the rebuilt buffer. The chosen mesh is `meshes[0]` when it is the only
    // kept one, which a part usually is; when it is not, the index is found by the map.
    let chosen_mesh = nodes[chosen]["mesh"].as_u64().map(|m| m as usize);
    let mut new_accessors: Vec<Value> = Vec::with_capacity(accessors.len() + 1);
    for a in &accessors {
        let mut acc = accessors_all[*a].as_object().cloned().unwrap_or_default();
        if let Some(v) = acc.get("bufferView").and_then(Value::as_u64) {
            match view_map.get(v as usize).copied().flatten() {
                Some(new) => acc.insert("bufferView".into(), json!(new)),
                None => return Err(format!("glb: accessor {a} reads buffer view {v}, which did not survive the prune")),
            };
        }
        new_accessors.push(Value::Object(acc));
    }

    let mut new_views: Vec<Value> = Vec::with_capacity(views.len() + 1);
    for v in &views {
        let mut view = views_all[*v].as_object().cloned().unwrap_or_default();
        let offset = view_at
            .iter()
            .find_map(|(old, new)| (*old == *v).then_some(*new))
            .unwrap_or(0);
        view.insert("buffer".into(), json!(0));
        view.insert("byteOffset".into(), json!(offset));
        new_views.push(Value::Object(view));
    }

    for (prim_ix, positions) in baked.iter().enumerate() {
        let accessor_ix = new_accessors.len();
        let view_ix = new_views.len();
        while !new_bin.len().is_multiple_of(4) {
            new_bin.push(0);
        }
        let byte_offset = new_bin.len();
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        for p in positions {
            for c in 0..3 {
                lo[c] = lo[c].min(p[c]);
                hi[c] = hi[c].max(p[c]);
            }
            for c in p {
                new_bin.extend_from_slice(&c.to_le_bytes());
            }
        }
        new_views.push(json!({
            "buffer": 0,
            "byteOffset": byte_offset,
            "byteLength": positions.len() * 12,
            "target": 34962,
        }));
        new_accessors.push(json!({
            "bufferView": view_ix,
            "componentType": 5126,
            "count": positions.len(),
            "type": "VEC3",
            "min": [lo[0], lo[1], lo[2]],
            "max": [hi[0], hi[1], hi[2]],
        }));
        // **Point the primitive at it**, in the mesh this primitive belongs to.
        let Some(m) = chosen_mesh else { continue };
        let Some(new_mesh_ix) = mesh_map.get(m).copied().flatten() else { continue };
        if let Some(prims) = new_meshes[new_mesh_ix]["primitives"].as_array_mut()
            && let Some(prim) = prims.get_mut(prim_ix)
        {
            prim["attributes"]["POSITION"] = json!(accessor_ix);
        }
    }
    out.insert("meshes".into(), Value::Array(new_meshes));
    out.insert("accessors".into(), Value::Array(new_accessors));

    while !new_bin.len().is_multiple_of(4) {
        new_bin.push(0);
    }
    out.insert("bufferViews".into(), Value::Array(new_views));
    out.insert("buffers".into(), json!([{"byteOffset": 0, "byteLength": new_bin.len()}]));

    // **The materials, and what they sample.** A material no surviving primitive names is not here at
    // all, which is the cascade the issue asks for.
    if !materials.is_empty() {
        let mut new_materials: Vec<Value> = Vec::with_capacity(materials.len());
        for mi in &materials {
            let mut mat = materials_all[*mi].as_object().cloned().unwrap_or_default();
            for (_slot, tex) in mat.iter_mut() {
                let Some(object) = tex.as_object_mut() else { continue };
                if let Some(ix) = object.get("index").and_then(Value::as_u64) {
                    match texture_map.get(ix as usize).copied().flatten() {
                        Some(new) => object.insert("index".into(), json!(new)),
                        None => object.remove("index"),
                    };
                }
            }
            new_materials.push(Value::Object(mat));
        }
        out.insert("materials".into(), Value::Array(new_materials));

        let mut new_textures: Vec<Value> = Vec::with_capacity(textures.len());
        for ti in &textures {
            let mut t = textures_all[*ti].as_object().cloned().unwrap_or_default();
            if let Some(s) = t.get("source").and_then(Value::as_u64) {
                match image_map.get(s as usize).copied().flatten() {
                    Some(new) => t.insert("source".into(), json!(new)),
                    None => t.remove("source"),
                };
            }
            if let Some(s) = t.get("sampler").and_then(Value::as_u64) {
                match sampler_map.get(s as usize).copied().flatten() {
                    Some(new) => t.insert("sampler".into(), json!(new)),
                    None => t.remove("sampler"),
                };
            }
            new_textures.push(Value::Object(t));
        }
        if !new_textures.is_empty() {
            out.insert("textures".into(), Value::Array(new_textures));
        }
        if !images.is_empty() {
            let mut new_images: Vec<Value> = Vec::with_capacity(images.len());
            for i in &images {
                let mut image = images_all[*i].as_object().cloned().unwrap_or_default();
                if let Some(v) = image.get("bufferView").and_then(Value::as_u64) {
                    match view_map.get(v as usize).copied().flatten() {
                        Some(new) => image.insert("bufferView".into(), json!(new)),
                        None => return Err(format!("glb: image {i} reads buffer view {v}, which did not survive the prune")),
                    };
                }
                new_images.push(Value::Object(image));
            }
            out.insert("images".into(), Value::Array(new_images));
        }
        if !samplers.is_empty() {
            let samplers_all = array(json, "samplers");
            out.insert(
                "samplers".into(),
                Value::Array(samplers.iter().map(|s| samplers_all[*s].clone()).collect()),
            );
        }
    }

    // **`extensionsUsed` / `extensionsRequired` go with their users.** A required extension nothing in
    // the file uses is a claim about a document that no longer holds it, and keeping the list would
    // make a loader look for a capability the pruned file does not need.
    out.remove("extensionsUsed");
    out.remove("extensionsRequired");

    write_container(&Value::Object(out), &new_bin)
}

/// **Each primitive's POSITION vertices of one mesh, through `world`.**
///
/// One `Vec` per primitive, in primitive order, so the caller can write one accessor each — a
/// primitive's vertices are its own accessor, and folding them together would change what it draws.
///
/// A primitive whose `mode` is not triangles yields an empty list: a line list's vertices written back
/// as a triangle strip is nonsense that renders as geometry, which is `triangle_vertices`'s rule and the
/// same one here.
fn positioned_positions(glb: &Glb, mesh_ix: usize, world: &[f32; 16]) -> Result<Vec<Vec<[f32; 3]>>, String> {
    let prims = glb.json["meshes"][mesh_ix]["primitives"]
        .as_array()
        .ok_or_else(|| format!("glb: mesh {mesh_ix} has no primitives"))?;
    let mut out = Vec::with_capacity(prims.len());
    for prim in prims {
        if prim["mode"].as_u64().unwrap_or(4) != 4 {
            out.push(Vec::new());
            continue;
        }
        let ix = prim["attributes"]["POSITION"]
            .as_u64()
            .ok_or_else(|| format!("glb: a primitive of mesh {mesh_ix} has no POSITION"))?;
        let verts = glb.read_vec3(ix as usize)?;
        out.push(verts.iter().map(|v| apply(world, *v)).collect());
    }
    Ok(out)
}

/// **A `.glb` container holding `json` and `bin`**, chunk lengths and padding as the format requires.
fn write_container(json: &Value, bin: &[u8]) -> Result<Vec<u8>, String> {
    let mut j = serde_json::to_vec(json).map_err(|e| format!("the pruned JSON did not encode: {e}"))?;
    while !j.len().is_multiple_of(4) {
        j.push(b' ');
    }
    let mut b = bin.to_vec();
    while !b.len().is_multiple_of(4) {
        b.push(0);
    }
    let total = 12 + 8 + j.len() + 8 + b.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(j.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&j);
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&b);
    Ok(out)
}
