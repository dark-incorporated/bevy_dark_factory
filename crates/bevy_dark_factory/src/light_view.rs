//! **A placed torch flickers in the viewport** (#103).
//!
//! A piece that carries a `bevy_lights` `Flicker` or `Fire` component (`placement_set_component`) has that
//! component on its entity in the viewport, put there by reflection like any other authoring component
//! ([`crate::piece_components`]), and the crate's own plugins do the rest: `Flicker` adds its `PointLight`
//! and `FlickerPlugin` drives its intensity every frame, and a window's editor ([`FirePreviewPlugin`]) lets
//! `Fire` make its flame and its pooled light. So the author sees what the game draws, from the same component.

use bevy::prelude::*;
use bevy_lights::{FirePlugin, FlamePlugin, FlickerPlugin, LightPoolPlugin};

/// Lets the crate flicker every piece that carries a `Flicker`.
pub struct LightViewPlugin;

impl Plugin for LightViewPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FlickerPlugin>() {
            app.add_plugins(FlickerPlugin);
        }
    }
}

/// **The flame and the pooled light of a placed `Fire`, in a running editor** (#103). `bevy_lights`' `FlamePlugin` needs a
/// render app, so only the binary adds this (the headless harness holds the `Fire` and nothing more). The pool measures from the
/// map camera, the way a game's pool measures from its player.
pub struct FirePreviewPlugin;

impl Plugin for FirePreviewPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FlickerPlugin>() {
            app.add_plugins(FlickerPlugin);
        }
        app.add_plugins((FlamePlugin, LightPoolPlugin::<crate::view::MainCamera>::default(), FirePlugin));
    }
}
