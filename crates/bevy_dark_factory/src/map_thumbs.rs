//! **A picture for each map on the project page** (#506).
//!
//! Every map card wore the same picture: `launcher::project_picture` returns *the newest portrait in
//! `target/blueprint_thumbs`*, which is a **piece** — so the Projects home and the project page showed
//! whichever chair was baked most recently beside the name of a 32 m street. The page is where a person
//! chooses *which map to open*, and it could not show them one.
//!
//! # Save-time capture is the trick, and this is its policy
//!
//! A map's picture is taken at the one moment it costs nothing: **when the map is saved**, the camera is
//! already looking at it (`docs`, #506). This module is the part a headless test can hold: the path a
//! save would write, what the path is keyed on, and — the thing the piece cache already learned — that a
//! card with no picture on disk costs **no filesystem call per frame**.
//!
//! # The key
//!
//! `id | map file | size | mtime_ns | pose | px | CACHE_VERSION`, the same shape as
//! [`crate::thumbs::cache_path`], so a re-saved map rebakes and an untouched one does not. The **pose is
//! in the key** because the picture is only as good as where the camera was left: two saves of a map from
//! two cameras are two pictures, and a card that kept the first would be lying about the second.
//!
//! # What a headless test cannot see
//!
//! Pixels. This crate's harness runs `backends: None`, so the tests here are about the *policy*: the path
//! a save would write, the key's inputs, and that a known miss is remembered. The capture itself is
//! [`Screenshot::image`], the same call `thumbs.rs`, `label_booth.rs` and `report.rs` already make.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use bevy::prelude::*;

/// **Bumped when the framing, the resolution or the crop changes**, so a picture taken under the old
/// rules is not served as if it were taken under the new ones.
pub const MAP_THUMB_CACHE_VERSION: u32 = 1;

/// **The width of a map picture**, pixels. The card is a 16:9 band (`launcher::THUMB`), and this is the
/// width that band is drawn from at the highest `ui_scale` the editor runs at.
pub const MAP_THUMB_PX: u32 = 960;

/// **Where the camera stood when the picture was taken.**
///
/// In the key because it is in the picture: the honest frame at save time is the person's own, and a
/// second save from somewhere else is a second picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CameraPose {
    /// Millimetres, as integers so two runs of the same pose hash the same bytes.
    pub at_mm: [i32; 3],
    /// Look direction, whole degrees about Y (yaw) and about X (pitch).
    pub yaw_deg: i32,
    pub pitch_deg: i32,
}

impl CameraPose {
    /// A pose from a transform, at the precision the key holds.
    pub fn of(translation: Vec3, yaw_deg: f32, pitch_deg: f32) -> CameraPose {
        CameraPose {
            at_mm: [
                (translation.x * 1000.0).round() as i32,
                (translation.y * 1000.0).round() as i32,
                (translation.z * 1000.0).round() as i32,
            ],
            yaw_deg: yaw_deg.round() as i32,
            pitch_deg: pitch_deg.round() as i32,
        }
    }
}

/// **Where `map`'s picture lives**, keyed on everything the picture depends on.
///
/// `None` when the map file cannot be statted: a path that cannot be read is not a picture to trust a
/// cache for, and the card falls back to no picture rather than to another map's.
pub fn map_thumb_path(root: &Path, map_file: &Path, pose: CameraPose, px: u32, version: u32) -> Option<PathBuf> {
    let meta = std::fs::metadata(map_file).ok()?;
    let mtime_ns = meta.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
    let name = map_file.file_name()?.to_string_lossy().into_owned();
    // **Every input the picture depends on, one field each**, so a reader debugging a cache that will
    // not invalidate can see which one is not moving.
    let identity = format!(
        "{name}|{len}|{mtime_ns}|{x}|{y}|{z}|{yaw}|{pitch}|{px}|{version}",
        len = meta.len(),
        x = pose.at_mm[0],
        y = pose.at_mm[1],
        z = pose.at_mm[2],
        yaw = pose.yaw_deg,
        pitch = pose.pitch_deg,
    );
    let key = blueprint_model::glb::fnv1a(identity.as_bytes());
    // **One folder per map**, named after the map's file stem: a card asks for *this* map and gets the
    // newest picture of it, which is what makes two cards two pictures. A single flat folder would need
    // an index to answer the same question, and an index is a file that goes stale in its own way.
    Some(
        root.join("target")
            .join("map_thumbs")
            .join(map_stem(map_file))
            .join(format!("{key:016x}.png")),
    )
}

/// **The folder a map's pictures live in**, named after its file stem (`yard` for `yard.map.yaml`).
fn map_stem(map_file: &Path) -> String {
    let name = map_file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    name.strip_suffix(blueprint_model::map::MAP_SUFFIX)
        .or_else(|| name.strip_suffix(".yaml"))
        .unwrap_or(&name)
        .to_owned()
}

/// **The newest picture for a map**, or `None` when it has never been saved with a camera.
///
/// One directory read, and only when a card asks for the first time — [`MapPictures`] remembers the
/// answer, so this is never called per frame for the same map.
pub fn newest_picture(root: &Path, map_file: &Path) -> Option<PathBuf> {
    let dir = root.join("target").join("map_thumbs").join(map_stem(map_file));
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max()
        .map(|(_, path)| path)
}

/// **The pictures the project page holds**, and the misses it remembers.
///
/// `missing` is the whole point: a card with no picture on disk is checked **once**, and every frame
/// after that costs a hash lookup rather than a `stat`. `scene_thumbnail_registry.rs` in Renzora
/// documents exactly this bug — "a path with nothing on disk is remembered in `missing` so the
/// browser's per-frame sweep does not stat the filesystem once per visible tile per frame".
#[derive(Resource, Default)]
pub struct MapPictures {
    /// The picture per map path, once it has been read.
    pictures: std::collections::HashMap<PathBuf, Handle<Image>>,
    /// The map paths with nothing on disk, so a second frame does not look again.
    missing: HashSet<PathBuf>,
    /// **How many times this has touched the filesystem for a picture**, which is the number the
    /// known-miss test asserts does not move on a second frame.
    pub fs_calls: u32,
}

impl MapPictures {
    /// Forget everything, so a test can measure one frame's calls from zero.
    pub fn forget(&mut self) {
        self.pictures.clear();
        self.missing.clear();
    }

    /// **This map's own picture, or `None`**, resolved and read once and remembered either way.
    ///
    /// Keyed on the **map file**, not the picture: the picture's path is keyed on the map's own mtime,
    /// so a re-saved map is a new picture at a new path. Asking by the map is what lets the answer be
    /// remembered — and a second call for a map with no picture costs a `HashSet` lookup and nothing
    /// else, which is the promise the acceptance names.
    pub fn for_map(&mut self, images: &mut Assets<Image>, root: &Path, map_file: &Path) -> Option<Handle<Image>> {
        if let Some(handle) = self.pictures.get(map_file) {
            return Some(handle.clone());
        }
        if self.missing.contains(map_file) {
            return None;
        }
        self.fs_calls += 1;
        let Some(path) = newest_picture(root, map_file) else {
            self.missing.insert(map_file.to_path_buf());
            return None;
        };
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(_) => {
                self.missing.insert(map_file.to_path_buf());
                return None;
            }
        };
        let image = match Image::from_buffer(
            &bytes,
            ImageType::Extension("png"),
            bevy::image::CompressedImageFormats::NONE,
            true,
            bevy::image::ImageSampler::Default,
            RenderAssetUsages::default(),
        ) {
            Ok(image) => image,
            Err(e) => {
                // **A PNG that will not decode is remembered as a miss too**: it is not going to start
                // decoding on the next frame, and re-reading it every frame is the bug this exists for.
                warn!("{}: unreadable map picture, not showing one: {e}", path.display());
                self.missing.insert(map_file.to_path_buf());
                return None;
            }
        };
        let handle = images.add(image);
        self.pictures.insert(map_file.to_path_buf(), handle.clone());
        Some(handle)
    }
}

use bevy::asset::RenderAssetUsages;
use bevy::image::ImageType;
use bevy::render::view::screenshot::{Screenshot, ScreenshotCaptured};

/// **A map save that wants a picture taken** (#506).
///
/// `save_map` is a plain function called from a key handler, and the editor's key system already takes
/// Bevy's sixteen parameters, so the save cannot spawn a capture itself. It leaves this instead, and
/// [`capture_the_saved_map`] takes the frame.
#[derive(Resource, Default)]
pub struct PaintAfterSave {
    /// The map file just written, and where the camera stood when it was.
    pub pending: Option<(PathBuf, CameraPose)>,
}

/// **Which map file a save just wrote**, or `None` while the question is not being asked.
///
/// `save_map` is a plain function called from a key handler whose signature is already Bevy's sixteen,
/// so it cannot reach the world. It records the file here; [`capture_the_saved_map`] reads it, takes the
/// frame and clears it. One slot, because a save is one thing at a time and a second save landing before
/// the first frame returns should photograph the second map rather than both.
pub fn ask_for_a_picture(pending: &mut PaintAfterSave, project: &crate::project::Project, open: &crate::project::OpenMap) {
    let file = project.maps_dir.join(blueprint_model::naming::map_file_name(&open.map.name));
    // **A pose the capture replaces from the live camera**, recorded so the value is never absent: a
    // build with no surface still says which map wanted a picture.
    pending.pending = Some((file, CameraPose { at_mm: [0, 0, 0], yaw_deg: 0, pitch_deg: 0 }));
}

/// **Take the picture a save asked for**, into the map's own folder.
///
/// The frame is [`crate::surface::Surface`] — the whole application's output, the same image
/// `report.rs` and `thumbs.rs` capture — because on a map the person is looking at exactly the thing
/// the card should show. There is no second staging pass and no booth: the picture is only as good as
/// where they left the camera, which is the trade the issue states and accepts.
pub fn capture_the_saved_map(
    mut commands: Commands,
    mut pending: ResMut<PaintAfterSave>,
    surface: Option<Res<crate::surface::Surface>>,
    project: Option<Res<crate::project::Project>>,
    camera: Option<Single<&GlobalTransform, With<crate::view::MainCamera>>>,
) {
    let Some((map_file, pose)) = pending.pending.take() else { return };
    let (Some(surface), Some(project)) = (surface, project) else {
        // **No surface on a headless build**, and nothing to photograph: the save still happened, and
        // the card keeps whatever picture it had.
        return;
    };
    let pose = camera.as_deref().map_or(pose, |t| {
        let (yaw, pitch, _) = t.rotation().to_euler(bevy::math::EulerRot::YXZ);
        CameraPose::of(t.translation(), yaw.to_degrees(), pitch.to_degrees())
    });
    let root = project.root.clone();
    let Some(path) = map_thumb_path(&root, &map_file, pose, MAP_THUMB_PX, MAP_THUMB_CACHE_VERSION) else {
        return;
    };
    commands.spawn(Screenshot::image(surface.image.clone())).observe(
        move |shot: On<ScreenshotCaptured>| {
            let image = match shot.image.clone().try_into_dynamic() {
                Ok(dynamic) => dynamic.to_rgba8(),
                Err(e) => {
                    warn!("a map's picture could not be taken ({e:?}); the card keeps its last one");
                    return;
                }
            };
            // **Downscaled to the card's band**, because the surface is the whole window and a card
            // is 236 px wide. One resize here rather than a decode of a window-sized PNG per card.
            let card = image::imageops::resize(
                &image,
                MAP_THUMB_PX,
                (MAP_THUMB_PX as f32 / MAP_CARD_ASPECT).round() as u32,
                image::imageops::FilterType::Triangle,
            );
            if let Some(parent) = path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    warn!("could not make {}: {e}", parent.display());
                    return;
                }
            }
            if let Err(e) = card.save(&path) {
                warn!("could not write {}: {e}", path.display());
            }
        },
    );
}

/// **A map card's picture is one 16:9 band** — the shape `launcher::thumb_box` draws its own picture in,
/// so a map's picture is not cropped to a different one than the card has.
pub const MAP_CARD_ASPECT: f32 = 16.0 / 9.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("blueprint-mapthumb-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        dir
    }

    const POSE: CameraPose = CameraPose { at_mm: [0, 2000, 5000], yaw_deg: 0, pitch_deg: -20 };

    /// **The path changes when the map changes, and not when another map does** — the key's whole job.
    #[test]
    fn a_maps_picture_path_follows_its_own_file_and_no_other() {
        let dir = scratch("key");
        let yard = dir.join("yard.map.yaml");
        let hall = dir.join("hall.map.yaml");
        std::fs::write(&yard, "version: 3\nname: yard\n").expect("writes");
        std::fs::write(&hall, "version: 3\nname: hall\n").expect("writes");

        let before = map_thumb_path(&dir, &yard, POSE, MAP_THUMB_PX, MAP_THUMB_CACHE_VERSION).expect("a path");
        // **Another map changing moves nothing**: `hall`'s mtime is not in `yard`'s key.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&hall, "version: 3\nname: hall\nnote: changed\n").expect("writes");
        let after_other = map_thumb_path(&dir, &yard, POSE, MAP_THUMB_PX, MAP_THUMB_CACHE_VERSION).expect("a path");
        assert_eq!(before, after_other, "another map's change moved this map's picture path");

        // **This map changing moves it**: a re-saved map rebakes.
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(&yard, "version: 3\nname: yard\nnote: changed\n").expect("writes");
        let after_own = map_thumb_path(&dir, &yard, POSE, MAP_THUMB_PX, MAP_THUMB_CACHE_VERSION).expect("a path");
        assert_ne!(before, after_own, "the map changed and its picture path did not");
    }

    /// **A move of the camera rebakes it**, because the picture is only as good as where it was taken.
    #[test]
    fn a_different_camera_is_a_different_picture() {
        let dir = scratch("pose");
        let yard = dir.join("yard.map.yaml");
        std::fs::write(&yard, "version: 3\nname: yard\n").expect("writes");
        let a = map_thumb_path(&dir, &yard, POSE, MAP_THUMB_PX, MAP_THUMB_CACHE_VERSION).expect("a path");
        let moved = CameraPose { at_mm: [1000, 2000, 5000], ..POSE };
        let b = map_thumb_path(&dir, &yard, moved, MAP_THUMB_PX, MAP_THUMB_CACHE_VERSION).expect("a path");
        assert_ne!(a, b, "the camera moved a metre and the picture path did not");
    }

    /// **A known miss costs no filesystem call on the next frame** — the counter that proves it.
    #[test]
    fn a_missing_picture_is_remembered_and_not_looked_for_again() {
        let root = scratch("miss");
        let map = root.join("yard.map.yaml");
        std::fs::write(&map, "version: 3\nname: yard\n").expect("writes");
        let mut pictures = MapPictures::default();
        let mut images = Assets::<Image>::default();

        assert!(pictures.for_map(&mut images, &root, &map).is_none(), "a map never saved has no picture");
        let after_first = pictures.fs_calls;
        assert_eq!(after_first, 1, "the first look is a filesystem call");

        // **Ten more frames**, as the page draws them.
        for _ in 0..10 {
            assert!(pictures.for_map(&mut images, &root, &map).is_none());
        }
        assert_eq!(pictures.fs_calls, after_first, "a remembered miss was looked for again");
    }

    /// **Each map's picture is its own**, and one map's never answers for another.
    #[test]
    fn two_maps_have_two_pictures() {
        let root = scratch("two");
        let yard = root.join("yard.map.yaml");
        let hall = root.join("hall.map.yaml");
        std::fs::write(&yard, "version: 3\nname: yard\n").expect("writes");
        std::fs::write(&hall, "version: 3\nname: hall\n").expect("writes");
        // The two folders are where a save writes, and they are different folders.
        let yard_pic = map_thumb_path(&root, &yard, POSE, MAP_THUMB_PX, MAP_THUMB_CACHE_VERSION).expect("a path");
        let hall_pic = map_thumb_path(&root, &hall, POSE, MAP_THUMB_PX, MAP_THUMB_CACHE_VERSION).expect("a path");
        assert_ne!(yard_pic.parent(), hall_pic.parent(), "two maps share one picture folder");
        assert_eq!(yard_pic.parent().and_then(|p| p.file_name()), Some(std::ffi::OsStr::new("yard")));
        assert_eq!(hall_pic.parent().and_then(|p| p.file_name()), Some(std::ffi::OsStr::new("hall")));
    }
}
