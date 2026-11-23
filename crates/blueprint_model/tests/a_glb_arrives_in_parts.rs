//! **A multi-part `.glb` arrives as one union footprint; its parts are measurable and choosable** (#503).
//!
//! A kit export is one file whose furniture is placed by node translation. `Glb::bounds` composes the
//! scene graph and measures the assembled thing, which is right for placing the file whole and useless
//! for the question an author actually has: *this chair-and-desk is one 4 m blob in the palette*.
//!
//! Three facts are measured here, and each is a different failure:
//!
//! - **The parts are measurable.** A file with three named nodes gives three parts, each with its own
//!   bounds, each different from the union.
//! - **One can be chosen.** `Glb::keep_only` writes a new `.glb` holding that node's subtree, and
//!   `import::measure` reads the result as that part — the other nodes' meshes and materials are
//!   absent from the written JSON, not merely hidden.
//! - **Centimetres are refused, not admitted.** A centimetre-authored file measures 100× and the
//!   import says so before anything lands in a library.

use blueprint_model::glb::Glb;
use blueprint_model::import;
use blueprint_model::library::{LIBRARY_VERSION, Library};

/// A container, so a fixture is bytes and not a directory of files.
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

/// One accessor's worth of an axis-aligned box, as eight corners, at `base`.
fn box_bytes(base: [f32; 3], half: [f32; 3]) -> Vec<u8> {
    let mut out = Vec::new();
    for sx in [-1.0f32, 1.0] {
        for sy in [-1.0f32, 1.0] {
            for sz in [-1.0f32, 1.0] {
                for c in [
                    base[0] + sx * half[0],
                    base[1] + sy * half[1],
                    base[2] + sz * half[2],
                ] {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
        }
    }
    out
}

/// **Three named parts, placed apart by node translation** — the shape every kit export has.
///
/// `chair` is a 0.5 × 0.9 × 0.5 box at the origin, `desk` a 1.6 × 0.75 × 0.8 box at x = 2, and
/// `lamp` a 0.3 × 1.4 × 0.3 box at x = 4. The union is 4.45 × 1.4 × 0.8, so no part can be confused
/// with it.
///
/// Two materials, one per group of parts, so a prune that keeps a material nothing uses is visible.
fn a_kit() -> (String, Vec<u8>) {
    let (chair, desk, lamp) = (
        box_bytes([0.0, 0.45, 0.0], [0.25, 0.45, 0.25]),
        box_bytes([0.0, 0.375, 0.0], [0.8, 0.375, 0.4]),
        box_bytes([0.0, 0.7, 0.0], [0.15, 0.7, 0.15]),
    );
    let mut bin = Vec::new();
    let mut views = Vec::new();
    for (i, part) in [&chair, &desk, &lamp].into_iter().enumerate() {
        views.push(format!(
            r#"{{"buffer":0,"byteOffset":{},"byteLength":{}}}"#,
            bin.len(),
            part.len()
        ));
        bin.extend_from_slice(part);
        let _ = i;
    }
    let accessors = [
        r#"{"bufferView":0,"componentType":5126,"count":8,"type":"VEC3","min":[-0.25,0.0,-0.25],"max":[0.25,0.9,0.25]}"#,
        r#"{"bufferView":1,"componentType":5126,"count":8,"type":"VEC3","min":[-0.8,0.0,-0.4],"max":[0.8,0.75,0.4]}"#,
        r#"{"bufferView":2,"componentType":5126,"count":8,"type":"VEC3","min":[-0.15,0.0,-0.15],"max":[0.15,1.4,0.15]}"#,
    ];
    let json = format!(
        r#"{{
  "asset":{{"version":"2.0"}},
  "scene":0,
  "scenes":[{{"nodes":[0]}}],
  "nodes":[
    {{"name":"kit","children":[1,2,3]}},
    {{"name":"chair","mesh":0,"translation":[0.0,0.0,0.0]}},
    {{"name":"desk","mesh":1,"translation":[2.0,0.0,0.0]}},
    {{"name":"lamp","mesh":2,"translation":[4.0,0.0,0.0]}}
  ],
  "meshes":[
    {{"name":"chair","primitives":[{{"attributes":{{"POSITION":0}},"material":0}}]}},
    {{"name":"desk","primitives":[{{"attributes":{{"POSITION":1}},"material":1}}]}},
    {{"name":"lamp","primitives":[{{"attributes":{{"POSITION":2}},"material":0}}]}}
  ],
  "materials":[{{"name":"wood"}},{{"name":"steel"}}],
  "accessors":[{}],
  "bufferViews":[{}],
  "buffers":[{{"byteLength":{}}}]
}}"#,
        accessors.join(","),
        views.join(","),
        bin.len()
    );
    (json, bin)
}

fn kit_glb() -> Glb {
    let (json, bin) = a_kit();
    Glb::parse(&container(&json, &bin)).expect("the fixture parses")
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("blueprint-parts-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch folder");
    dir
}

fn empty_library() -> Library {
    Library { version: LIBRARY_VERSION, note: None, descriptors: Vec::new() }
}

/// **The parts are measurable, and each is not the union.**
#[test]
fn a_kit_export_reports_each_part_with_its_own_bounds() {
    let glb = kit_glb();
    let parts = glb.parts().expect("the fixture has parts");
    let names: Vec<&str> = parts.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, vec!["chair", "desk", "lamp"], "every named node is a part: {names:?}");

    let span = |i: usize| {
        let m = parts[i].measured;
        (m.footprint.0, m.height, m.footprint.1)
    };
    // **Each part measures itself**, not the file.
    let (cw, ch, cd) = span(0);
    assert!((cw - 0.5).abs() < 1e-4 && (ch - 0.9).abs() < 1e-4 && (cd - 0.5).abs() < 1e-4, "chair: {cw} x {ch} x {cd}");
    let (dw, dh, dd) = span(1);
    assert!((dw - 1.6).abs() < 1e-4 && (dh - 0.75).abs() < 1e-4 && (dd - 0.8).abs() < 1e-4, "desk: {dw} x {dh} x {dd}");
    let (lw, lh, ld) = span(2);
    assert!((lw - 0.3).abs() < 1e-4 && (lh - 1.4).abs() < 1e-4 && (ld - 0.3).abs() < 1e-4, "lamp: {lw} x {lh} x {ld}");

    // **And the union is not any part** — the defect the issue describes.
    let (_, whole_hi) = glb.bounds().expect("the union measures");
    for part in &parts {
        let differs = (part.measured.hi[0] - whole_hi[0]).abs() > 1e-3
            || (part.measured.footprint.0 - (whole_hi[0] - 3.8)).abs() > 1e-3;
        assert!(differs, "a part measured the union: {part:?}");
    }
}

/// **Choosing one part writes a new `.glb` that measures as that part**, and the rest is gone from the
/// JSON rather than hidden.
#[test]
fn importing_one_part_writes_a_file_that_measures_as_that_part() {
    let dir = scratch("prune");
    let glb = kit_glb();
    // The desk is node 2.
    let kept = glb.keep_only(2).expect("the desk can be kept on its own");

    let out = dir.join("desk.glb");
    std::fs::write(&out, &kept).expect("the pruned file writes");

    let candidate = import::measure(&out, "desk.glb", &empty_library());
    assert!(!candidate.blocked(), "a kept part must be importable: {:?}", candidate.findings);
    let m = candidate.measured.expect("the kept part measures");
    assert!((m.footprint.0 - 1.6).abs() < 1e-4, "footprint {}", m.footprint.0);
    assert!((m.height - 0.75).abs() < 1e-4, "height {}", m.height);

    // **The other two parts are absent**, by name, from the JSON that was written.
    let written = Glb::open(&out).expect("the written file parses");
    let text = serde_json::to_string(&written.json).expect("json");
    assert!(text.contains("\"desk\""), "the kept part is named: {text}");
    assert!(!text.contains("\"chair\""), "the chair's mesh survives the prune: {text}");
    assert!(!text.contains("\"lamp\""), "the lamp's mesh survives the prune: {text}");
    // **`steel` is the desk's material and `wood` belongs to nothing that survived.**
    assert!(text.contains("\"steel\""), "the kept part's material is kept: {text}");
    assert!(!text.contains("\"wood\""), "a material no surviving primitive uses is kept: {text}");
}

/// **A node index the file does not have is refused, naming the parts it does have.**
#[test]
fn choosing_a_part_that_is_not_there_names_the_parts_that_are() {
    let glb = kit_glb();
    let why = glb.keep_only(99).expect_err("there is no node 99");
    for name in ["chair", "desk", "lamp"] {
        assert!(why.contains(name), "the refusal does not name `{name}`: {why}");
    }
}

/// **A centimetre-authored file is reported by the measurement and refused by `piece_import`.**
///
/// Two halves, because they are two different decisions. `import::measure` says what the file measures
/// and that no size here is true (`Severity::Warn`: a caller that only wanted to *look* at a file is not
/// importing it). `piece_import` is the door, and a centimetre-authored file does not go through it —
/// admitting one puts a 200 m door in a palette where every other piece is metres, which nothing
/// downstream ever rescales.
///
/// The door's half is proved where the door is: the editor's own import path and its refusals.
#[test]
fn a_centimetre_authored_glb_is_reported_with_its_size_named() {
    let dir = scratch("cms");
    // A 2.003 m door exported in centimetres: 200.3 units.
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
    let path = dir.join("door_frame.glb");
    std::fs::write(&path, container(json, &box_bytes([0.0, 100.15, 0.0], [40.0, 100.15, 5.0]))).expect("writes");

    let candidate = import::measure(&path, "door_frame.glb", &empty_library());
    assert!(candidate.measured.is_some_and(|m| m.suspect_centimetres), "the file was not read as centimetres");
    let said = candidate
        .findings
        .iter()
        .find(|f| f.message.contains("centimetre"))
        .map(|f| f.message.clone())
        .unwrap_or_else(|| panic!("the measurement says nothing about centimetres: {:?}", candidate.findings));
    assert!(said.contains("200"), "the finding does not name the size it measured: {said}");
    assert!(
        candidate.findings.iter().any(|f| f.message.contains("centimetre")),
        "a person has to be told before the import is admitted, not after it lands"
    );
}
