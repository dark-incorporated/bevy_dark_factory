//! **`cauldron-negatives`: draw the negatives on purpose** (DL-05, #464; series labels from use, #459).
//!
//! ```text
//! cauldron-negatives sheet --project <root> --map <name> --out <dir> --renderer <cmd> [--request <words>] [--size clear|near|both]
//! cauldron-negatives run   --project <root> --labels <dir> --scratch <dir> --renderer <cmd>
//! ```
//!
//! `sheet` makes every defect of [`bevy_cauldron::perturb`] on a scratch copy of one scene, draws the scene and each defective copy, and writes
//! `<out>/NN-<defect>-<size>.png`, `<out>/00-kept.png`, `<out>/sheet.json` (what each picture is, with the engine's measurement on both sides)
//! and the records themselves as a ledger in `<out>/ledger/`. `run` perturbs every kept turn of a labels folder and writes the records there.
//!
//! **The renderer** is a program called as `<cmd> <project-root> <map> <out.png>` that draws the map in a real editor and writes the picture
//! (`tools/labels/render_scene.py` does, under a display). It is handed a scratch copy, never the person's project. A renderer that fails
//! leaves the record's picture saying why; it never stops the run.

use std::path::{Path, PathBuf};
use std::process::Command;

use bevy_cauldron::perturb::{self, Defect, Size};
use bevy_cauldron::turn_store::{self, LEDGER_FILE, Line, Shot, Store};

const USAGE: &str = "usage:\n  cauldron-negatives sheet --project <root> --map <name> --out <dir> --renderer <cmd> [--request <words>] [--size clear|near|both]\n  cauldron-negatives run --project <root> --labels <dir> --scratch <dir> --renderer <cmd>";

struct Args(Vec<String>);

impl Args {
    fn get(&self, flag: &str) -> Option<&str> {
        self.0.iter().position(|a| a == flag).and_then(|i| self.0.get(i + 1)).map(String::as_str)
    }

    fn need(&self, flag: &str) -> Result<&str, String> {
        self.get(flag).ok_or_else(|| format!("{flag} is needed\n{USAGE}"))
    }
}

/// **Draw a scene with the renderer program**, keeping the picture in `store`.
fn draw(renderer: &str, store: &mut Store, root: &Path, map: &str) -> Shot {
    let out = std::env::temp_dir().join(format!("cauldron-negatives-{}-{}.png", std::process::id(), map));
    let _ = std::fs::remove_file(&out);
    match Command::new(renderer).arg(root).arg(map).arg(&out).status() {
        Ok(status) if status.success() => match std::fs::read(&out) {
            Ok(png) => {
                let _ = std::fs::remove_file(&out);
                perturb::shot_of_png(store, &png)
            }
            Err(e) => Shot::missing(format!("the renderer wrote no picture at {}: {e}", out.display())),
        },
        Ok(status) => Shot::missing(format!("the renderer `{renderer}` stopped with {status}")),
        Err(e) => Shot::missing(format!("the renderer `{renderer}` could not start: {e}")),
    }
}

fn copy_picture(store: &Store, shot: &Shot, to: &Path) -> Result<Option<String>, String> {
    match &shot.image {
        Some(image) => {
            std::fs::copy(store.image_path(&image.sha256), to).map_err(|e| format!("{}: {e}", to.display()))?;
            Ok(Some(image.sha256.clone()))
        }
        None => Ok(None),
    }
}

fn sheet(args: &Args) -> Result<(), String> {
    let project = PathBuf::from(args.need("--project")?);
    let map = args.need("--map")?;
    let out = PathBuf::from(args.need("--out")?);
    let renderer = args.need("--renderer")?.to_owned();
    let request = args.get("--request").unwrap_or("furnish the room");
    let sizes: &[Size] = match args.get("--size").unwrap_or("clear") {
        "clear" => &[Size::Clear],
        "near" => &[Size::Near],
        "both" => &[Size::Near, Size::Clear],
        other => return Err(format!("--size is clear, near or both, not `{other}`")),
    };
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let out = std::fs::canonicalize(&out).map_err(|e| e.to_string())?;
    let project = std::fs::canonicalize(&project).map_err(|e| format!("{}: {e}", project.display()))?;
    let mut store = Store::new(out.join("ledger"));
    let scratch = out.join("scratch");
    let turn = perturb::kept_turn_from_map(&project, map, request)?;

    let mut entries = Vec::new();
    let kept = perturb::render_kept(&project, &scratch, map, &mut |root, name| draw(&renderer, &mut store, root, name))?;
    let sha = copy_picture(&store, &kept, &out.join("00-kept.png"))?;
    entries.push(serde_json::json!({"file": "00-kept.png", "what": "the scene as kept", "picture": sha, "missing": kept.missing}));

    let mut serial = 0;
    for defect in Defect::ALL {
        for &size in defect.sizes().iter().filter(|s| sizes.contains(s)) {
            serial += 1;
            let made = perturb::perturb(&turn, &project, &scratch, defect, size, &mut |root, name| draw(&renderer, &mut store, root, name), serial);
            match made {
                Ok(record) => {
                    let file = format!("{serial:02}-{}-{}.png", defect.name(), size.name());
                    let sha = copy_picture(&store, &record.after, &out.join(&file))?;
                    entries.push(serde_json::json!({
                        "file": file,
                        "what": format!("{} ({})", defect.name(), size.name()),
                        "defect": record.defect, "size": record.size, "magnitude": record.magnitude, "unit": record.unit,
                        "target": record.target, "facts": record.facts, "picture": sha, "missing": record.after.missing,
                    }));
                    store.append(&Line::perturbed(record))?;
                }
                Err(refused) => {
                    eprintln!("refused: {refused}");
                    entries.push(serde_json::json!({"what": format!("{} ({})", defect.name(), size.name()), "refused": refused.why}));
                }
            }
        }
    }
    let manifest = serde_json::json!({"map": map, "request": request, "pictures": entries});
    std::fs::write(out.join("sheet.json"), serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_dir_all(&scratch);
    println!("wrote {} pictures to {}", manifest["pictures"].as_array().map_or(0, Vec::len), out.display());
    Ok(())
}

fn run(args: &Args) -> Result<(), String> {
    let project = std::fs::canonicalize(args.need("--project")?).map_err(|e| e.to_string())?;
    let labels = PathBuf::from(args.need("--labels")?);
    let scratch = PathBuf::from(args.need("--scratch")?);
    let renderer = args.need("--renderer")?.to_owned();
    let read = turn_store::read_file(&labels.join(LEDGER_FILE))?;
    if let Some(bad) = read.damaged.first() {
        return Err(format!("{} line {} does not read: {}", LEDGER_FILE, bad.line, bad.why));
    }
    let mut store = Store::new(labels);
    let mut shots = Store::new(store.dir().to_path_buf());
    let report = perturb::run(&read, &mut store, &project, &scratch, &mut |root, name| draw(&renderer, &mut shots, root, name))?;
    for refused in &report.refused {
        eprintln!("refused: {refused}");
    }
    println!("{} written, {} refused, {} already there", report.written.len(), report.refused.len(), report.skipped.len());
    Ok(())
}

fn main() {
    let args = Args(std::env::args().skip(1).collect());
    let result = match args.0.first().map(String::as_str) {
        Some("sheet") => sheet(&args),
        Some("run") => run(&args),
        _ => Err(USAGE.to_owned()),
    };
    if let Err(why) = result {
        eprintln!("cauldron-negatives: {why}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::os::unix::fs::PermissionsExt;

    fn digests(root: &Path) -> BTreeMap<String, String> {
        fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, String>) {
            for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, root, out);
                } else if let Ok(bytes) = std::fs::read(&path) {
                    out.insert(path.strip_prefix(root).unwrap_or(&path).to_string_lossy().into_owned(), turn_store::sha256_hex(&bytes));
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(root, root, &mut out);
        out
    }

    /// **`sheet` draws the kept scene and each defect with the renderer it is given**, writes the pictures, the manifest and the records, says why
    /// a defect was not made, and leaves the project it was pointed at byte-for-byte as it was.
    #[test]
    fn a_sheet_draws_the_scene_and_each_defect_and_touches_nothing() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap_or_else(|e| panic!("{e}"));
        let work = std::env::temp_dir().join(format!("cauldron-negatives-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        std::fs::create_dir_all(&work).unwrap_or_else(|e| panic!("{e}"));
        // **A renderer that draws something**: a picture with more than one colour, whatever the map.
        let png = work.join("picture.png");
        image::RgbaImage::from_fn(8, 8, |x, y| image::Rgba([(x * 30) as u8, (y * 30) as u8, 90, 255])).save(&png).unwrap_or_else(|e| panic!("{e}"));
        let renderer = work.join("render.sh");
        std::fs::write(&renderer, format!("#!/bin/sh\ncp '{}' \"$3\"\n", png.display())).unwrap_or_else(|e| panic!("{e}"));
        std::fs::set_permissions(&renderer, std::fs::Permissions::from_mode(0o755)).unwrap_or_else(|e| panic!("{e}"));

        let before = digests(&repo.join("assets"));
        let out = work.join("out");
        let args = Args(
            ["sheet", "--project", repo.to_str().unwrap_or_default(), "--map", "bedroom", "--request", "a bed and a sofa", "--out", out.to_str().unwrap_or_default(), "--renderer", renderer.to_str().unwrap_or_default()]
                .map(str::to_owned)
                .to_vec(),
        );
        sheet(&args).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(digests(&repo.join("assets")), before, "the project was changed");

        let manifest: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(out.join("sheet.json")).unwrap_or_else(|e| panic!("{e}"))).unwrap_or_else(|e| panic!("{e}"));
        let pictures = manifest["pictures"].as_array().unwrap_or_else(|| panic!("{manifest}"));
        assert_eq!(pictures[0]["file"], "00-kept.png");
        assert!(out.join("00-kept.png").is_file());
        let made: Vec<&str> = pictures.iter().filter(|p| p["picture"].is_string() && p["defect"].is_string()).map(|p| p["defect"].as_str().unwrap_or_default()).collect();
        for defect in ["float", "clip", "delete_named", "turn_wrong"] {
            assert!(made.contains(&defect), "{defect} was not drawn: {manifest}");
        }
        for entry in pictures.iter().filter(|p| p["defect"].is_string()) {
            let file = entry["file"].as_str().unwrap_or_default();
            assert!(out.join(file).is_file(), "{file} is not there");
            assert!(entry["facts"]["before"].is_object() && entry["facts"]["after"].is_object(), "{entry}");
        }
        // A defect the scene cannot have is said so, and has no picture.
        assert!(pictures.iter().any(|p| p["refused"].as_str().is_some_and(|why| why.contains("no forest"))), "{manifest}");
        // The records are a ledger that reads, one per picture made.
        let read = turn_store::read_file(&out.join("ledger").join(LEDGER_FILE)).unwrap_or_else(|e| panic!("{e}"));
        assert!(read.damaged.is_empty(), "{:?}", read.damaged);
        assert_eq!(read.perturbed().count(), made.len());
        assert!(read.perturbed().all(|p| p.after.image.is_some()), "every record carries the picture the renderer drew");
        assert!(!out.join("scratch").exists(), "scratch copies were left behind");
        let _ = std::fs::remove_dir_all(&work);
    }

    /// **A renderer that fails leaves a picture that says why**, and the run goes on.
    #[test]
    fn a_renderer_that_fails_is_a_picture_that_says_why() {
        let work = std::env::temp_dir().join(format!("cauldron-negatives-fail-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let mut store = Store::new(work.join("store"));
        let shot = draw("/nonexistent/renderer", &mut store, Path::new("/tmp"), "m");
        assert!(shot.image.is_none() && shot.missing.as_deref().is_some_and(|why| why.contains("could not start")), "{shot:?}");
        let _ = std::fs::remove_dir_all(&work);
    }
}
