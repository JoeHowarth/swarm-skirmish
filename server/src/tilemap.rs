use bevy::{asset::RenderAssetUsages, prelude::*};
use bevy_ecs_tilemap::prelude::*;
use image::DynamicImage;
use serde::{Deserialize, Serialize};
use swarm_lib::Team;

use crate::{gridworld::GridWorld, CellState};

pub struct TilemapPlugin;

impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_ecs_tilemap::TilemapPlugin);
        app.add_systems(Startup, (load_tileset, setup_map).chain());
        app.add_systems(Update, render_grid);
    }
}

pub enum CellRender {
    Empty,
    Blocked,
    Pawn(bool),
}

impl CellRender {
    pub fn cell_to_tile(&self) -> TileTextureIndex {
        TileTextureIndex(match self {
            CellRender::Empty => 0,
            CellRender::Blocked => 179,
            CellRender::Pawn(is_player) => {
                if *is_player {
                    1
                } else {
                    2
                }
            }
        })
    }

    pub fn from_state(state: &CellState, teams: &Query<&Team>) -> CellRender {
        match state {
            CellState::Empty => CellRender::Empty,
            CellState::Blocked => CellRender::Blocked,
            CellState::Pawn(pawn_id) => {
                CellRender::Pawn(teams.get(*pawn_id).unwrap() == &Team::Player)
            }
        }
    }
}


fn render_grid(
    tile_storage: Query<&mut TileStorage>,
    mut tiles: Query<&mut TileTextureIndex>,
    teams: Query<&Team>,
    grid: Res<GridWorld>,
) {
    let tile_storage = tile_storage.single();
    for ((x, y), state) in grid.iter() {
        let pos = TilePos {
            x: x as u32,
            y: y as u32,
        };
        let tile_entity = tile_storage.get(&pos).unwrap();
        let render = CellRender::from_state(state, &teams);

        *tiles.get_mut(tile_entity).unwrap() = render.cell_to_tile();
    }
}

pub fn load_tileset(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // Load the image data
    let image_bytes = include_bytes!("../assets/ascii.png");
    let dynamic_image = image::load_from_memory(image_bytes).unwrap();

    let processed_image = process_magenta_to_black(dynamic_image);

    // Convert to Bevy Image
    let image = Image::from_dynamic(
        processed_image,
        true,
        RenderAssetUsages::default(),
    );

    // Store the atlas layout as a resource
    commands.insert_resource(AsciiTexture(images.add(image)));
}

fn process_magenta_to_black(image: DynamicImage) -> DynamicImage {
    let mut rgba_image = image.to_rgba8();

    for pixel in rgba_image.pixels_mut() {
        // Check if pixel is magenta (FF00FF)
        if pixel[0] == 255 && pixel[1] == 0 && pixel[2] == 255 {
            // Set to black (000000)
            pixel[0] = 0;
            pixel[1] = 0;
            pixel[2] = 0;
            // Keep alpha value unchanged
        }
    }

    DynamicImage::ImageRgba8(rgba_image)
}

#[derive(Resource)]
struct AsciiTexture(Handle<Image>);

fn setup_map(mut commands: Commands, ascii_texture: Res<AsciiTexture>) {
    let map_size = TilemapSize { x: 16, y: 16 };
    let tile_size = TilemapTileSize { x: 18.0, y: 18.0 };
    let grid_size = TilemapGridSize { x: 18.0, y: 18.0 };

    // Create tilemap entity early to register with each tile entity
    let tilemap_entity = commands.spawn_empty().id();

    // Create tile storage
    let mut tile_storage = TileStorage::empty(map_size);

    // Spawn all tiles
    for x in 0..map_size.x {
        for y in 0..map_size.y {
            let tile_pos = TilePos { x, y };
            let tile_entity = commands
                .spawn(TileBundle {
                    position: tile_pos,
                    texture_index: TileTextureIndex(
                        (x + y * map_size.x) as u32 % 256,
                    ), // This will create a pattern of ASCII chars
                    tilemap_id: TilemapId(tilemap_entity),
                    ..default()
                })
                .id();
            tile_storage.set(&tile_pos, tile_entity);
        }
    }

    let map_type = TilemapType::Square;

    // Spawn the map entity with all required components
    commands.entity(tilemap_entity).insert(TilemapBundle {
        grid_size,
        map_type,
        size: map_size,
        storage: tile_storage,
        texture: TilemapTexture::Single(ascii_texture.0.clone()),
        tile_size,
        transform: get_tilemap_center_transform(
            &map_size, &grid_size, &map_type, 0.0,
        ),
        ..Default::default()
    });
}
