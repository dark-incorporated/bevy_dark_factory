//! **The one request ever sent to a remote model** (#453): the public changelog image and the three
//! questions of `fixtures/request.json`, one of each type. `examples/record_openrouter.rs` records it and
//! `tests/openrouter.rs` replays it; both include this file, so what was recorded and what is replayed
//! cannot drift. Screenshots leave the LAN on a remote endpoint, so **no other image is ever used here**.

use bevy_oracle::{Question, Request, data_url};
use std::path::PathBuf;

/// The public image, relative to the repo root: it is on the project's changelog page.
pub const PUBLIC_IMAGE: &str = "docs/changelog/media/2026-10-24-S1.5.png";

pub fn public_image_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(PUBLIC_IMAGE)
}

/// **The first line of a Git LFS pointer**, which is what a fresh clone has where the image should be: the repo's
/// daily CI is `actions/checkout@v4` with no `git lfs pull`, so `docs/changelog/media/*.png` is a 131-byte text
/// file there. A request built from those bytes is a different request, and a replay of the recorded one would
/// fail — which is a red test on a checkout that is fine, not a missing fixture (review B).
pub const LFS_POINTER_PREFIX: &str = "version https://git-lfs.github.com/spec/v1";

/// Whether `path` is an LFS pointer rather than the file itself.
///
/// The recorder example includes this file and never asks (it reads the image it is about to send), so it is
/// allowed to be unused there rather than split into a second include for one function.
#[allow(dead_code)]
pub fn is_lfs_pointer(path: &std::path::Path) -> bool {
    match std::fs::read(path) {
        Ok(bytes) => bytes.starts_with(LFS_POINTER_PREFIX.as_bytes()),
        Err(_) => false,
    }
}

/// The request, with the public image as a base64 PNG data URL.
pub fn request() -> Request {
    let png = std::fs::read(public_image_path()).expect("the public changelog image is in the repo");
    Request::new("A screenshot of the bevy_blueprint map editor.")
        .with_image(data_url(&png))
        .with_question("has_ui", Question::noul("Does the image show an editor window with panels or a toolbar?"))
        .with_question(
            "dominant",
            Question::choice(
                "What fills most of the image?",
                [
                    ("map_view", "a map or 3D scene"),
                    ("text", "mostly text"),
                    ("blank", "an empty or single-colour frame"),
                ],
            ),
        )
        .with_question(
            "clarity",
            Question::score(
                "How readable and uncluttered is the image?",
                ["unreadable", "poor", "fair", "good", "excellent"],
            ),
        )
}
