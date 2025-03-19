use bevy::prelude::*;
use bevy_ecs_tilemap::prelude::*;
use image::DynamicImage;
use swarm_lib::{BotData, CellKind, Item, Pos};

use super::{MapMode, Textures};
use crate::{
    types::{GridWorld, Tick},
    GameState,
};

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct TilemapSystemSimUpdateSet;

#[derive(Resource)]
pub struct TilemapLayers {
    terrain: Entity,
    fow: Entity,
    items: Entity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FogOfWarLevel {
    Full,
    TwoThirds,
    OneThird,
    None,
}

impl From<FogOfWarLevel> for TileTextureIndex {
    fn from(value: FogOfWarLevel) -> Self {
        TileTextureIndex(match value {
            FogOfWarLevel::Full => 0,
            FogOfWarLevel::TwoThirds => 1,
            FogOfWarLevel::OneThird => 2,
            FogOfWarLevel::None => 3,
        })
    }
}

pub struct TilemapPlugin;

impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_ecs_tilemap::TilemapPlugin);
        // Initialize the TilemapWorldCoords resource with default values
        app.insert_resource(TilemapWorldCoords {
            transform: Transform::default(),
            grid_size: TilemapGridSize { x: 32.0, y: 32.0 },
            map_type: TilemapType::Square,
        });
        // Add system to update the resource
        app.add_systems(OnEnter(GameState::InGame), setup_map);
        app.add_systems(OnExit(GameState::InGame), remove_map);
        app.add_systems(
            Update,
            (update_tilemap_world_coords, render_grid)
                .run_if(in_state(GameState::InGame))
                .in_set(TilemapSystemSimUpdateSet),
        );
    }
}

fn render_grid(
    tile_storage: Query<&TileStorage>,
    layers: Res<TilemapLayers>,
    mut tiles: Query<&mut TileTextureIndex>,
    grid: Res<GridWorld>,
    map_mode: Res<MapMode>,
    bot_data: Query<&BotData>,
    current_tick: Res<Tick>,
) {
    let terrain_storage = tile_storage.get(layers.terrain).unwrap();
    let fow_storage = tile_storage.get(layers.fow).unwrap();
    let items_storage = tile_storage.get(layers.items).unwrap();

    match *map_mode {
        MapMode::All => {
            for ((x, y), state) in grid.iter() {
                let tile_pos = TilePos {
                    x: x as u32,
                    y: y as u32,
                };

                let terrain_texture_index = match state.kind {
                    CellKind::Empty => TileTextureIndex(0),
                    CellKind::Blocked => TileTextureIndex(2),
                    CellKind::Unknown => TileTextureIndex(1),
                };
                let terrain_tile_entity =
                    terrain_storage.get(&tile_pos).unwrap();
                *tiles.get_mut(terrain_tile_entity).unwrap() =
                    terrain_texture_index;

                // Ensure Fog of War is None
                let fow_tile_entity = fow_storage.get(&tile_pos).unwrap();
                *tiles.get_mut(fow_tile_entity).unwrap() =
                    FogOfWarLevel::None.into();

                // Items
                let item_texture_index = match state.item {
                    Some(Item::Metal) => TileTextureIndex(0),
                    _ => TileTextureIndex(5),
                };

                let item_tile_entity = items_storage.get(&tile_pos).unwrap();
                *tiles.get_mut(item_tile_entity).unwrap() = item_texture_index;
            }
        }
        MapMode::Team(_team) => todo!(),
        MapMode::Bot(bot_id) => {
            let bot_data = bot_data.get(bot_id).unwrap();

            for ((x, y), cell) in bot_data.known_map.iter() {
                let tile_pos = TilePos {
                    x: x as u32,
                    y: y as u32,
                };

                // Terrain
                let terrain_texture_index = match cell.kind {
                    CellKind::Empty => TileTextureIndex(0),
                    CellKind::Blocked => TileTextureIndex(2),
                    CellKind::Unknown => TileTextureIndex(1),
                };
                let terrain_tile_entity =
                    terrain_storage.get(&tile_pos).unwrap();
                *tiles.get_mut(terrain_tile_entity).unwrap() =
                    terrain_texture_index;

                // Fog of War
                let fow_tile_entity = fow_storage.get(&tile_pos).unwrap();

                let fow_level = match current_tick.0 - cell.last_observed {
                    _ if cell.is_unknown() => FogOfWarLevel::Full,
                    0 => FogOfWarLevel::None,
                    x if x < 10 => FogOfWarLevel::OneThird,
                    _ => FogOfWarLevel::TwoThirds,
                };

                *tiles.get_mut(fow_tile_entity).unwrap() = fow_level.into();

                // Items
                let item_texture_index = match cell.item {
                    Some(Item::Metal) => TileTextureIndex(0),
                    _ => TileTextureIndex(5),
                };

                let item_tile_entity = items_storage.get(&tile_pos).unwrap();
                *tiles.get_mut(item_tile_entity).unwrap() = item_texture_index;
            }
        }
    }
}

#[derive(Resource)]
pub struct MapSize {
    pub x: u32,
    pub y: u32,
}

fn setup_map(
    mut commands: Commands,
    textures: Res<Textures>,
    tile_map_size: Res<MapSize>,
) {
    let map_size = TilemapSize {
        x: tile_map_size.x,
        y: tile_map_size.y,
    };
    let tile_size = TilemapTileSize { x: 32.0, y: 32.0 };
    let grid_size = TilemapGridSize { x: 32.0, y: 32.0 };
    let map_type = TilemapType::Square;

    // Create tilemap entity early to register with each tile entity
    let terrain_entity = commands.spawn_empty().id();
    let mut terrain_storage = TileStorage::empty(map_size);

    // Spawn all tiles
    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };
            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    texture_index: TileTextureIndex(2),
                    tilemap_id: TilemapId(terrain_entity),
                    ..default()
                })
                .id();
            terrain_storage.set(&tile_pos, tile_entity);
        }
    }

    // Spawn the map entity with all required components
    commands.entity(terrain_entity).insert(TilemapBundle {
        grid_size,
        map_type,
        size: map_size,
        storage: terrain_storage,
        texture: TilemapTexture::Single(textures.terrain.clone()),
        tile_size,
        transform: get_tilemap_center_transform(
            &map_size, &grid_size, &map_type, -10.0,
        ),
        ..Default::default()
    });

    let fow_entity = commands.spawn_empty().id();
    let mut fow_storage = TileStorage::empty(map_size);

    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };
            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    texture_index: TileTextureIndex(3),
                    tilemap_id: TilemapId(fow_entity),
                    ..default()
                })
                .id();
            fow_storage.set(&tile_pos, tile_entity);
        }
    }

    commands.entity(fow_entity).insert(TilemapBundle {
        grid_size,
        map_type,
        size: map_size,
        storage: fow_storage,
        texture: TilemapTexture::Single(textures.fog_of_war.clone()),
        tile_size,
        transform: get_tilemap_center_transform(
            &map_size, &grid_size, &map_type, -9.0,
        ),
        ..Default::default()
    });

    let ascii_entity = commands.spawn_empty().id();
    let mut ascii_storage = TileStorage::empty(map_size);

    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };
            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    texture_index: TileTextureIndex(0),
                    tilemap_id: TilemapId(ascii_entity),
                    ..default()
                })
                .id();
            ascii_storage.set(&tile_pos, tile_entity);
        }
    }

    commands.entity(ascii_entity).insert(TilemapBundle {
        grid_size,
        map_type,
        size: map_size,
        storage: ascii_storage,
        texture: TilemapTexture::Single(textures.items.clone()),
        tile_size,
        transform: get_tilemap_center_transform(
            &map_size, &grid_size, &map_type, -8.0,
        ),
        ..Default::default()
    });

    commands.insert_resource(TilemapLayers {
        terrain: terrain_entity,
        fow: fow_entity,
        items: ascii_entity,
    });
}

fn remove_map(
    mut commands: Commands,
    tilemap: Query<Entity, With<TilemapTexture>>,
) {
    for tilemap in tilemap.iter() {
        commands.entity(tilemap).despawn_recursive();
    }
}

fn _process_magenta_to_black(image: DynamicImage) -> DynamicImage {
    let mut rgba_image = image.to_rgba8();

    for pixel in rgba_image.pixels_mut() {
        // Check if pixel is magenta (FF00FF)
        if pixel[0] == 255 && pixel[1] == 0 && pixel[2] == 255 {
            // Set to black (000000)
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 0;
            pixel[3] = 50;
        }
    }

    DynamicImage::ImageRgba8(rgba_image)
}

/// Resource that stores the components needed for world coordinate conversion
#[derive(Resource)]
pub struct TilemapWorldCoords {
    pub transform: Transform,
    pub grid_size: TilemapGridSize,
    pub map_type: TilemapType,
}

impl TilemapWorldCoords {
    /// Converts a game position (Pos) to world coordinates (Vec2)
    pub fn pos_to_world(&self, pos: &Pos) -> Vec2 {
        let tile_pos = TilePos {
            x: pos.x() as u32,
            y: pos.y() as u32,
        };

        // Get the position in tilemap's local space
        let local_pos =
            tile_pos.center_in_world(&self.grid_size, &self.map_type);

        // Add the tilemap's translation to get world coordinates
        local_pos + self.transform.translation.xy()
    }
}

/// System to update the TilemapWorldCoords resource with the latest component
/// values
fn update_tilemap_world_coords(
    tilemap: Query<(&Transform, &TilemapGridSize, &TilemapType)>,
    coords: Option<ResMut<TilemapWorldCoords>>,
    layers: Res<TilemapLayers>,
) {
    let Some(mut coords) = coords else {
        return;
    };

    if let Ok((transform, grid_size, map_type)) = tilemap.get(layers.terrain) {
        coords.transform = *transform;
        coords.grid_size = *grid_size;
        coords.map_type = *map_type;
    }
}
