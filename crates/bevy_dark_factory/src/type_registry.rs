//! **The running editor's type registry, for operations.**
//!
//! An operation runs on a [`crate::ops::Ctx`] with no world in hand (the same call serves a key, the
//! menu and an agent over MCP), but `family_set` and `placement_set_component` must read a value as the
//! crate's own type, which only the registry knows. The editor plugin puts the app's
//! [`AppTypeRegistry`] here when it is built; it is an `Arc`, so types registered after that are seen.
//! The newest app wins, which is what a test process that builds several apps needs.

use std::sync::RwLock;

use bevy::prelude::AppTypeRegistry;

static REGISTRY: RwLock<Option<AppTypeRegistry>> = RwLock::new(None);

/// Keep `registry` as the one operations read.
pub fn set(registry: AppTypeRegistry) {
    if let Ok(mut held) = REGISTRY.write() {
        *held = Some(registry);
    }
}

/// The registry operations read, or `None` before an editor app was built.
pub fn get() -> Option<AppTypeRegistry> {
    REGISTRY.read().ok().and_then(|held| held.clone())
}
