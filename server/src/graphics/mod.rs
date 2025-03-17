use bevy::prelude::*;
use render_bots::{RenderBotsPlugin, RenderBotsSystemSet};
use swarm_lib::Team;
use tilemap::{TilemapPlugin, TilemapSystemSimUpdateSet};

use crate::GameState;

pub mod interaction;
pub mod render_bots;
pub mod tilemap;

pub struct GraphicsPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct GraphicsSystemSet;

impl Plugin for GraphicsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TilemapPlugin);
        app.add_plugins(RenderBotsPlugin);
        app.add_plugins(interaction::InteractionPlugin);
        app.insert_resource(MapMode::All);
        app.add_systems(Startup, load_tileset);
        app.configure_sets(
            Update,
            (TilemapSystemSimUpdateSet, RenderBotsSystemSet)
                .in_set(GraphicsSystemSet),
        );
    }
}

#[derive(Resource)]
pub struct Textures {
    // ascii: Handle<Image>,
    terrain: Handle<Image>,
    items: Handle<Image>,
    pawns: (Handle<Image>, Handle<TextureAtlasLayout>),
    fog_of_war: Handle<Image>,
}

#[derive(Resource, PartialEq, Eq)]
pub enum MapMode {
    All,
    Team(Team),
    Bot(Entity),
}

pub fn load_tileset(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    // Load the image data
    // let image_bytes = include_bytes!("../assets/ascii.png");
    // let dynamic_image = image::load_from_memory(image_bytes).unwrap();

    // let processed_image = process_magenta_to_black(dynamic_image);

    // // Convert to Bevy Image
    // let ascii_image = Image::from_dynamic(
    //     processed_image,
    //     true,
    //     RenderAssetUsages::default(),
    // );

    // Store the atlas layout as a resource
    // let ascii_texture = images.add(ascii_image);
    let terrain_texture: Handle<Image> = asset_server.load("Terrain.png");
    let fog_of_war_texture: Handle<Image> = asset_server.load("FogOfWar.png");

    let items_texture: Handle<Image> = asset_server.load("Items.png");
    let pawns_texture: Handle<Image> = asset_server.load("BotFrames1.png");
    let pawns_layout = atlas_layouts.add(TextureAtlasLayout::from_grid(
        (64, 64).into(),
        4,
        1,
        None,
        None,
    ));

    commands.insert_resource(Textures {
        // ascii: ascii_texture,
        terrain: terrain_texture,
        items: items_texture,
        pawns: (pawns_texture, pawns_layout),
        fog_of_war: fog_of_war_texture,
    });
}
