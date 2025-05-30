use bevy::prelude::*;
use render_bots::{RenderBotsPlugin, RenderBotsSystemSet};
use swarm_lib::Team;
use tilemap::{TilemapPlugin, TilemapSystemSimUpdateSet};

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
    _images: ResMut<Assets<Image>>,
    asset_server: Res<AssetServer>,
    mut atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
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
