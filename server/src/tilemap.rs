use bevy::{
    asset::RenderAssetUsages,
    color::palettes::css,
    gizmos,
    prelude::*,
};
use bevy_ecs_tilemap::prelude::*;
use image::DynamicImage;
use swarm_lib::{
    Action::{self, *},
    BotData,
    CellKind,
    Item,
    Pos,
    Team,
};

use crate::{
    apply_actions::{ActionContainer, ActionState, CurrentAction},
    get_map_size,
    types::{CellState, GridWorld},
    GameState,
    MAP_SIZE,
};

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct TilemapSystemSimUpdateSet;
#[derive(Debug)]
pub enum CellRender {
    Empty,
    Blocked,
    Pawn(bool),
    Item(Item),
}

#[derive(Resource)]
struct AsciiTexture(Handle<Image>);

pub struct TilemapPlugin;

impl Plugin for TilemapPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy_ecs_tilemap::TilemapPlugin);
        app.add_systems(Startup, load_tileset);
        // Initialize the TilemapWorldCoords resource with default values
        app.insert_resource(TilemapWorldCoords {
            transform: Transform::default(),
            grid_size: TilemapGridSize { x: 18.0, y: 18.0 },
            map_type: TilemapType::Square,
        });
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
            (render_grid).in_set(TilemapSystemSimUpdateSet),
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

fn setup_map(mut commands: Commands, ascii_texture: Res<AsciiTexture>) {
    let Some((x, y)) = get_map_size() else {
        return;
    };
    let map_size = TilemapSize {
        x: x as u32,
        y: y as u32,
    };
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
                    texture_index: CellRender::Blocked.cell_to_tile(),
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
            &map_size, &grid_size, &map_type, -1.0,
        ),
        ..Default::default()
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

fn render_grid(
    tile_storage: Query<&mut TileStorage>,
    mut tiles: Query<&mut TileTextureIndex>,
    teams: Query<&BotData>,
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
        // trace!(?pos, ?render, "Render");

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

impl CellRender {
    pub fn cell_to_tile(&self) -> TileTextureIndex {
        TileTextureIndex(match self {
            CellRender::Empty => 0,
            CellRender::Blocked => 219,
            CellRender::Pawn(true) => 1,
            CellRender::Pawn(false) => 2,
            CellRender::Item(Item::Crumb) => 250,
            CellRender::Item(Item::Fent) => 239,
            CellRender::Item(Item::Truffle) => 84, // 'T' in ASCII
        })
    }

    pub fn from_state(
        state: &CellState,
        query: &Query<&BotData>,
    ) -> CellRender {
        if let Some(pawn_id) = state.pawn {
            return CellRender::Pawn(
                query.get(pawn_id).unwrap().team == Team::Player,
            );
        }

        if let Some(item) = state.item {
            return CellRender::Item(item);
        }

        match state.kind {
            CellKind::Empty => CellRender::Empty,
            CellKind::Blocked => CellRender::Blocked,
            CellKind::Unknown => unreachable!(),
        }
    }
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
) {
    let Some(mut coords) = coords else {
        return;
    };

    if let Ok((transform, grid_size, map_type)) = tilemap.get_single() {
        coords.transform = *transform;
        coords.grid_size = *grid_size;
        coords.map_type = *map_type;
    }
}
