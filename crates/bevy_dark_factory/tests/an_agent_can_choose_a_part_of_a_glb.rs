//! **Choosing one part of a multi-part `.glb`, and refusing a centimetre-authored one** (#503).
//!
//! `an_agent_can_import_a_mesh.rs` proves the import's shape: a mesh goes in, a piece comes out, undo
//! takes it back. This is the two decisions `piece_import` gained, each a different failure:
//!
//! - **a kit export is one file holding several pieces**, and `part` names one — the import writes a
//!   `.glb` holding that part and the descriptor measures it, so a chair is a chair and not a 4 m blob;
//! - **a centimetre-authored file is refused at the door**, because `import::measure` can only warn and
//!   admitting it puts a 200 m door in a palette where every other piece is metres.
//!
//! Its fixture is written here rather than by `fixtures::Fixture`, because a *multi-part* `.glb` is
//! exactly what the fixture generator does not make: it is one mesh per descriptor.

use bevy::prelude::*;
use bevy_dark_factory::harness;
use bevy_dark_factory::project::Project;

use crate::fixtures;

/// A `.glb` container, so a fixture is bytes rather than a folder of files.
fn container(json: &str, bin: &[u8]) -> Vec<u8> {
    let mut j = json.as_bytes().to_vec();
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
    out
}

/// Eight corners of a box at `base` with half-extents `half`.
fn box_bytes(base: [f32; 3], half: [f32; 3]) -> Vec<u8> {
    let mut out = Vec::new();
    for sx in [-1.0f32, 1.0] {
        for sy in [-1.0f32, 1.0] {
            for sz in [-1.0f32, 1.0] {
                for c in [base[0] + sx * half[0], base[1] + sy * half[1], base[2] + sz * half[2]] {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
        }
    }
    out
}

/// **Two named parts placed apart by node translation**, one 1 m box and one 2 m box, so the union and
/// either part differ from each other.
fn a_two_part_kit() -> Vec<u8> {
    let (small, large) = (
        box_bytes([0.0, 0.5, 0.0], [0.5, 0.5, 0.5]),
        box_bytes([0.0, 0.5, 0.0], [1.0, 0.5, 1.0]),
    );
    let mut bin = Vec::new();
    let mut views = Vec::new();
    for part in [&small, &large] {
        views.push(format!(
            r#"{{"buffer":0,"byteOffset":{},"byteLength":{}}}"#,
            bin.len(),
            part.len()
        ));
        bin.extend_from_slice(part);
    }
    let json = format!(
        r#"{{
  "asset":{{"version":"2.0"}},
  "scene":0,
  "scenes":[{{"nodes":[0]}}],
  "nodes":[
    {{"name":"kit","children":[1,2]}},
    {{"name":"stool","mesh":0}},
    {{"name":"table","mesh":1,"translation":[3.0,0.0,0.0]}}
  ],
  "meshes":[
    {{"name":"stool","primitives":[{{"attributes":{{"POSITION":0}}}}]}},
    {{"name":"table","primitives":[{{"attributes":{{"POSITION":1}}}}]}}
  ],
  "accessors":[
    {{"bufferView":0,"componentType":5126,"count":8,"type":"VEC3","min":[-0.5,0.0,-0.5],"max":[0.5,1.0,0.5]}},
    {{"bufferView":1,"componentType":5126,"count":8,"type":"VEC3","min":[-1.0,0.0,-1.0],"max":[1.0,1.0,1.0]}}
  ],
  "bufferViews":[{}],
  "buffers":[{{"byteLength":{}}}]
}}"#,
        views.join(","),
        bin.len()
    );
    container(&json, &bin)
}

fn project_holding(name: &str, file: &str, bytes: &[u8]) -> std::path::PathBuf {
    let root = fixtures::Fixture::new(name).descriptor("crate", "alpha").build("m");
    let inbox = root.join("assets").join("inbox");
    std::fs::create_dir_all(&inbox).unwrap_or_else(|e| panic!("{e}"));
    std::fs::write(inbox.join(file), bytes).unwrap_or_else(|e| panic!("{e}"));
    root
}

fn editor(root: &std::path::Path) -> App {
    let mut app = harness::build_headless(root, "m", None).unwrap_or_else(|e| panic!("{e}"));
    app.add_plugins(bevy_debugger_bevy::DebuggerPlugin::default());
    app.add_plugins(bevy_dark_factory::ops_brp::OpsBridgePlugin);
    app
}

fn call(app: &mut App, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
    use bevy::remote::{RemoteMethodSystemId, RemoteMethods};
    let id = match app.world().resource::<RemoteMethods>().get(method) {
        Some(RemoteMethodSystemId::Instant(id)) => *id,
        _ => return Err(format!("no method `{method}` is registered")),
    };
    app.world_mut()
        .run_system_with(id, Some(params))
        .map_err(|e| format!("{method}: {e:?}"))?
        .map_err(|e| format!("{method}: {e:?}"))
}

fn apply(app: &mut App, parameters: serde_json::Value) -> serde_json::Value {
    call(
        app,
        "blueprint/operation_apply",
        serde_json::json!({"operation": "piece_import", "parameters": parameters, "dry_run": false}),
    )
    .unwrap_or_else(|e| panic!("the apply answers: {e}"))
}

/// **Choosing a part imports that part alone, measured as itself.**
#[test]
fn choosing_a_part_imports_that_part_and_not_the_union() {
    let root = project_holding("partchoose", "kit.glb", &a_two_part_kit());
    let mut app = editor(&root);

    let applied = apply(&mut app, serde_json::json!({"glb": "assets/inbox/kit.glb", "id": "stool", "part": "stool"}));
    assert_eq!(applied["outcome"], "applied", "the part was not imported: {applied}");

    let project = app.world().resource::<Project>();
    let piece = project.library.get("stool").unwrap_or_else(|| panic!("`stool` is not in the library: {applied}"));
    // **The stool's own 1 m footprint**, not the whole file's 5 m union (a 1 m box plus a 2 m box three
    // metres away).
    let (w, d) = piece.extent.footprint.expect("the piece has a footprint");
    assert!((w - 1.0).abs() < 1e-3, "the imported footprint is {w} m wide, not the stool's 1 m");
    assert!((d - 1.0).abs() < 1e-3, "the imported footprint is {d} m deep, not the stool's 1 m");

    // **The descriptor names a new file, not the kit it came out of** — so the palette holds a stool and
    // opening it loads a stool.
    let mesh = piece.mesh.clone().unwrap_or_default();
    assert_ne!(mesh, "inbox/kit.glb", "the descriptor still names the whole kit");
    let written = root.join("assets").join(&mesh);
    assert!(written.is_file(), "the descriptor names `{mesh}`, which is not on disk at {}", written.display());

    // **And the table is absent from what was written**, by name.
    let kept = std::fs::read(&written).expect("the kept file reads");
    let text = String::from_utf8_lossy(&kept);
    assert!(text.contains("stool"), "the kept part is not named in its own file");
    assert!(!text.contains("table"), "the table's mesh survived the prune: {text}");
}

/// **A part name the file does not hold is refused, listing the parts it does.**
#[test]
fn a_part_name_the_file_does_not_hold_lists_the_parts_it_does() {
    let root = project_holding("partnope", "kit.glb", &a_two_part_kit());
    let mut app = editor(&root);

    let refused = apply(&mut app, serde_json::json!({"glb": "assets/inbox/kit.glb", "id": "nope", "part": "lamp"}));
    assert_eq!(refused["outcome"], "refused", "{refused}");
    let why = refused["refusal"].as_str().unwrap_or_default();
    for name in ["stool", "table"] {
        assert!(why.contains(name), "the refusal does not name `{name}`: {why}");
    }
}

/// **A centimetre-authored file is refused at the door**, naming the size it measured.
#[test]
fn a_centimetre_authored_glb_is_refused_by_the_import() {
    // A 2.003 m door exported in centimetres: 200.3 units tall.
    let json = r#"{
      "asset":{"version":"2.0"},
      "scene":0,
      "scenes":[{"nodes":[0]}],
      "nodes":[{"name":"door","mesh":0}],
      "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],
      "accessors":[{"componentType":5126,"count":8,"type":"VEC3","bufferView":0,
                    "min":[-40.0,0.0,-5.0],"max":[40.0,200.3,5.0]}],
      "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":96}],
      "buffers":[{"byteLength":96}]
    }"#;
    let bytes = container(json, &box_bytes([0.0, 100.15, 0.0], [40.0, 100.15, 5.0]));
    let root = project_holding("cmdoor", "door_frame.glb", &bytes);
    let mut app = editor(&root);

    let refused = apply(&mut app, serde_json::json!({"glb": "assets/inbox/door_frame.glb", "id": "door_frame"}));
    assert_eq!(refused["outcome"], "refused", "a 200 m door was admitted: {refused}");
    let why = refused["refusal"].as_str().unwrap_or_default();
    assert!(why.contains("centimetre"), "the refusal does not say centimetres: {why}");
    assert!(why.contains("200"), "the refusal does not name the size it measured: {why}");

    // **And nothing landed in the library.**
    let project = app.world().resource::<Project>();
    assert!(project.library.get("door_frame").is_none(), "the refused piece is in the library anyway");
}
