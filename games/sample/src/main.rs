use bevy::prelude::*;
use bevy_blueprint::GamePlugin;

fn main() {
    App::new().add_plugins((DefaultPlugins, GamePlugin)).run();
}
