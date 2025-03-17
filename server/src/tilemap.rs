use bevy::{
    asset::RenderAssetUsages,
    color::palettes::css,
    gizmos,
    prelude::*,
};
use bevy_ecs_tilemap::prelude::*;
use image::DynamicImage;
use swarm_lib::{
    known_map::ClientCellState,
    Action::{self, *},
    BotData,
    BuildingKind,
    CellKind,
    FrameKind,
    Item,
    Pos,
    Team,
};

use crate::{
    apply_actions::{ActionContainer, ActionState, CurrentAction},
    bot_update::BotIdToEntity,
    get_map_size,
    types::{CellState, GridWorld},
    GameState,
    MAP_SIZE,
};

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct TilemapSystemSimUpdateSet;

#[derive(Resource)]
struct Textures {
    // ascii: Handle<Image>,
    terrain: Handle<Image>,
    items: Handle<Image>,
    pawns: (Handle<Image>, Handle<TextureAtlasLayout>),
    fog_of_war: Handle<Image>,
}

#[derive(Resource)]
struct TilemapLayers {
    terrain: Entity,
    fow: Entity,
    items: Entity,
}

#[derive(Resource)]
enum MapMode {
    All,
    Team(Team),
    Bot(Entity),
}

enum FogOfWarLevel {
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
        app.add_systems(Startup, load_tileset);
        // Initialize the TilemapWorldCoords resource with default values
        app.insert_resource(TilemapWorldCoords {
            transform: Transform::default(),
            grid_size: TilemapGridSize { x: 32.0, y: 32.0 },
            map_type: TilemapType::Square,
        });
        app.insert_resource(MapMode::All);
        // Add system to update the resource
        app.add_systems(OnEnter(GameState::InGame), setup_map);
        app.add_systems(OnExit(GameState::InGame), remove_map);
        app.add_systems(
            Update,
            (render_move_to, update_tilemap_world_coords)
                .run_if(in_state(GameState::InGame)),
        );
        app.add_systems(
            Update,
            (ensure_bot_sprite, render_bots, render_grid)
                .in_set(TilemapSystemSimUpdateSet),
        );
    }
}

fn render_move_to(
    mut gizmos: Gizmos,
    actions: Query<(&BotData, &CurrentAction)>,
    tilemap_coords: Option<Res<TilemapWorldCoords>>,
) {
    let Some(tilemap_coords) = tilemap_coords else {
        return;
    };

    for (bot_data, current_action) in actions.iter() {
        let Some(ActionContainer { kind, state, .. }) = &current_action.0
        else {
            continue;
        };

        if let (Action::MoveTo(path), ActionState::MoveTo { idx }) =
            (kind, state)
        {
            let mut pos = bot_data.pos;
            for dst in &path[*idx..] {
                let src_world = tilemap_coords.pos_to_world(&pos);
                let dst_world = tilemap_coords.pos_to_world(dst);

                gizmos.line_2d(src_world, dst_world, css::RED);

                pos = *dst;
            }
        }
    }
}

fn setup_map(mut commands: Commands, textures: Res<Textures>) {
    let Some((x, y)) = get_map_size() else {
        return;
    };
    let map_size = TilemapSize {
        x: x as u32,
        y: y as u32,
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

fn ensure_bot_sprite(
    mut commands: Commands,
    bots: Query<(Entity, &BotData), Without<Sprite>>,
    textures: Res<Textures>,
    tilemap_coords: Res<TilemapWorldCoords>,
) {
    for (entity, bot_data) in bots.iter() {
        commands.entity(entity).insert((
            Sprite::from_atlas_image(
                textures.pawns.0.clone(),
                TextureAtlas {
                    layout: textures.pawns.1.clone(),
                    index: match bot_data.frame {
                        FrameKind::Flea => 0,
                        FrameKind::Tractor => 1,
                        FrameKind::Building(BuildingKind::Small) => 2,
                    },
                },
            ),
            Transform::from_xyz(
                tilemap_coords.pos_to_world(&bot_data.pos).x,
                tilemap_coords.pos_to_world(&bot_data.pos).y,
                1.0,
            )
            .with_scale(Vec3::new(0.5, 0.5, 1.0)),
        ));
    }
}

fn render_bots(
    bot_data_q: Query<&BotData>,
    mut bots_with_sprites: Query<
        (Entity, &mut Transform, &mut Visibility),
        With<Sprite>,
    >,
    tilemap_coords: Res<TilemapWorldCoords>,
    map_mode: Res<MapMode>,
) {
    for (entity, mut transform, mut visibility) in bots_with_sprites.iter_mut()
    {
        let bot_data = bot_data_q.get(entity).unwrap();
        match *map_mode {
            MapMode::All => {
                *visibility = Visibility::Visible;
                let pos = tilemap_coords.pos_to_world(&bot_data.pos);
                transform.translation.x = pos.x;
                transform.translation.y = pos.y;
            }
            MapMode::Team(team) => {
                todo!()
            }
            MapMode::Bot(bot_e) => {
                todo!()
            }
        }
    }
}

fn render_grid(
    tile_storage: Query<&TileStorage>,
    layers: Res<TilemapLayers>,
    mut tiles: Query<&mut TileTextureIndex>,
    grid: Res<GridWorld>,
    map_mode: Res<MapMode>,
    bot_data: Query<&BotData>,
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
                    CellKind::Blocked => TileTextureIndex(3),
                    CellKind::Unknown => TileTextureIndex(2),
                };
                let terrain_tile_entity =
                    terrain_storage.get(&tile_pos).unwrap();
                *tiles.get_mut(terrain_tile_entity).unwrap() =
                    terrain_texture_index;

                // Fog of War
                let fow_tile_entity = fow_storage.get(&tile_pos).unwrap();
                let fow_level = match cell.last_observed {
                    0 => FogOfWarLevel::None,
                    x if x < 10 => FogOfWarLevel::OneThird,
                    _ if cell.is_unknown() => FogOfWarLevel::Full,
                    _ => FogOfWarLevel::TwoThirds,
                };
                *tiles.get_mut(fow_tile_entity).unwrap() = fow_level.into();

                let item_tile_entity = items_storage.get(&tile_pos).unwrap();
                *tiles.get_mut(item_tile_entity).unwrap() =
                    TileTextureIndex(77);
            }
        }
    }
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
