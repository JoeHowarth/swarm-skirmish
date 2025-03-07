use bevy::{prelude::*, utils::HashMap};
use dlopen2::wrapper::{Container, WrapperApi};
use swarm_lib::{
    bot_logger::BotLogger,
    Action,
    ActionResult,
    ActionStatus,
    ActionWithId,
    Bot,
    BotUpdate,
    CellKind,
    CellStateRadar,
    Energy,
    Pos,
    RadarBotData,
    RadarData,
    Team,
};

use crate::{
    apply_actions::{ActionContainer, ActionState, CurrentAction, PastActions},
    types::{GridWorld, Inventory, PawnKind, Tick},
};

#[derive(WrapperApi)]
struct Api {
    new_bot:
        fn(bot_logger: BotLogger, map_size: (usize, usize)) -> Box<dyn Bot>,
}

#[derive(Resource)]
struct BotLib(pub Container<Api>);

#[derive(Component, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[require(CurrentAction, PastActions)]
pub struct BotId(pub u32);

pub struct BotUpdatePlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct BotUpdateSystemSet;

impl Plugin for BotUpdatePlugin {
    fn build(&self, app: &mut App) {
        let cont: Container<Api> = unsafe {
            Container::load(
                "/Users/jh/personal/swarm-skirmish/target/debug/libdylib_bot.\
                 dylib",
            )
        }
        .expect("Could not open library or load symbols");

        app.add_systems(
            Update,
            (ensure_bot_id, create_server_updates.pipe(update_bots))
                .chain()
                .in_set(BotUpdateSystemSet),
        )
        .insert_resource(BotLib(cont))
        .init_resource::<BotIdToEntity>();
    }
}

#[derive(Component)]
pub struct BotInstance {
    pub bot: Box<dyn Bot>,
}

#[derive(Resource, Default)]
pub struct BotIdToEntity(pub HashMap<BotId, Entity>);

fn ensure_bot_id(
    mut bot_id_to_entity: ResMut<BotIdToEntity>,
    mut commands: Commands,
    query: Query<Entity, (With<PawnKind>, Without<BotId>)>,
    mut next_id: Local<u32>,
    bot_lib: Res<BotLib>,
    map_size: Query<&bevy_ecs_tilemap::prelude::TilemapSize>,
) {
    let map_size = map_size.single();
    for entity in query.iter() {
        *next_id += 1;
        let bot_id = BotId(*next_id);
        bot_id_to_entity.0.insert(bot_id, entity);

        info!("Creating new bot instance for bot ID: {}", bot_id.0);
        let bot = bot_lib.0.new_bot(
            BotLogger::new(bot_id.0),
            (map_size.x as usize, map_size.y as usize),
        );
        commands
            .entity(entity)
            .insert((bot_id, BotInstance { bot }));
    }
}

fn update_bots(
    mut updates: In<HashMap<BotId, BotUpdate>>,
    mut query: Query<(
        Entity,
        &BotId,
        &mut CurrentAction,
        &mut PastActions,
        &mut BotInstance,
    )>,
) {
    for (
        entity,
        bot_id,
        mut current_action,
        mut past_actions,
        mut bot_instance,
    ) in query.iter_mut()
    {
        debug!(?bot_id, entity = entity.index(), "Updating bot");
        let server_update = updates.remove(bot_id).unwrap();

        let maybe_action = bot_instance.bot.update(server_update.clone());

        let Some(action) = maybe_action else {
            debug!("No action from bot ID: {}", bot_id.0);
            continue;
        };

        trace!("Bot ID: {} action: {:?}", bot_id.0, action);
        let action_container = ActionContainer {
            state: match &action.action {
                Action::MoveTo(path) => ActionState::MoveTo {
                    path: path.iter().skip(1).cloned().collect(),
                },
                Action::Noop => ActionState::None,
                Action::MoveDir(_) => ActionState::None,
                Action::Harvest(_) => ActionState::Harvest { remaining: 1 },
            },
            kind: action.action,
            id: action.id,
        };

        // If there is a current action already, cancel it
        if let Some(action) =
            std::mem::replace(&mut current_action.0, Some(action_container))
        {
            past_actions.push(ActionResult {
                action: action.kind,
                id: action.id,
                status: ActionStatus::Cancelled,
                completed_tick: server_update.tick,
            });
        }
    }
}

pub fn create_server_updates<'a>(
    tick: Res<Tick>,
    query: Query<(
        &BotId,
        &Pos,
        &Team,
        &CurrentAction,
        &PastActions,
        &Inventory,
        &Energy,
    )>,
    grid_world: Res<GridWorld>,
) -> HashMap<BotId, BotUpdate> {
    query
        .iter()
        .map(
            |(
                bot_id,
                pos,
                team,
                current_action,
                past_actions,
                inventory,
                energy,
            )| {
                (
                    *bot_id,
                    BotUpdate {
                        tick: tick.0,
                        team: *team,
                        position: *pos,
                        radar: create_radar_data(pos, &grid_world, &query),
                        in_progress_action: {
                            current_action.as_ref().map(|action_container| {
                                ActionWithId {
                                    action: action_container.kind.clone(),
                                    id: action_container.id,
                                }
                            })
                        },
                        completed_action: {
                            past_actions.last().and_then(|action| {
                                if action.completed_tick == tick.0 {
                                    Some(ActionResult {
                                        action: action.action.clone(),
                                        status: action.status.clone(),
                                        id: action.id,
                                        completed_tick: tick.0,
                                    })
                                } else {
                                    None
                                }
                            })
                        },
                        items: inventory.0.clone(),
                        energy: *energy,
                    },
                )
            },
        )
        .collect()
}

fn create_radar_data(
    pos: &Pos,
    grid_world: &GridWorld,
    query: &Query<(
        &BotId,
        &Pos,
        &Team,
        &CurrentAction,
        &PastActions,
        &Inventory,
        &Energy,
    )>,
) -> RadarData {
    // Define the radar range (how far to look in each direction)
    let radar_range = 5; // This gives a view of 11x11 cells centered on the bot (5 in each
                         // direction)

    // Get bot's world coordinates
    let (bot_world_x, bot_world_y) = pos.as_isize();

    let mut radar = RadarData {
        center_world_pos: *pos,
        pawns: Vec::new(),
        cells: Vec::new(),
    };

    // Use nearby to get cells in radar range with Manhattan distance
    grid_world
        .nearby(bot_world_x as usize, bot_world_y as usize, radar_range)
        .for_each(|(pos, cell)| {
            let cell_pos = Pos::from(pos);

            let radar_cell = CellStateRadar {
                kind: match cell.kind {
                    CellKind::Empty => CellKind::Empty,
                    CellKind::Blocked => CellKind::Blocked,
                    CellKind::Unknown => unreachable!(),
                },
                pawn: cell.pawn.map(|e| {
                    let (bot_id, _, &team, _, _, _, _) = query.get(e).unwrap();

                    // Store the bot's position in world coordinates
                    radar.pawns.push(RadarBotData {
                        team,
                        pos: cell_pos,
                        bot_id: bot_id.0,
                    });

                    radar.pawns.len() - 1
                }),
                item: cell.item,
                pos: cell_pos,
            };

            radar.cells.push(radar_cell);
        });

    // Sort radar cells by manhattan distance from center, with direction as
    // tiebreaker for deterministic ordering
    radar.cells.sort_by_key(|cell| {
        let dx = (cell.pos.x() as isize - bot_world_x).abs() as u32;
        let dy = (cell.pos.y() as isize - bot_world_y).abs() as u32;
        let manhattan_distance = (dx + dy) * 100; // Scale by 100 to make room for direction tiebreaker

        // Calculate direction as u8 for tiebreaking
        let dir_value = if dx == 0 && dy == 0 {
            0 // Center cell
        } else if dx >= dy {
            // Primarily east/west
            if cell.pos.x() as isize >= bot_world_x {
                1
            } else {
                2
            } // East = 1, West = 2
        } else {
            // Primarily north/south
            if cell.pos.y() as isize >= bot_world_y {
                3
            } else {
                4
            } // North = 3, South = 4
        };

        manhattan_distance + dir_value
    });

    radar
}
