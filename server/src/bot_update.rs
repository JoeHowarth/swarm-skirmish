use bevy::{prelude::*, utils::HashMap};
use dlopen2::wrapper::{Container, WrapperApi};
use swarm_lib::{
    bot_logger::BotLogger,
    gridworld::PassableCell,
    known_map::{ClientBotData, ClientCellState, KnownMap},
    Action,
    ActionResult,
    ActionStatus,
    ActionWithId,
    Bot,
    BotData,
    BotUpdate,
    CellKind,
    CellStateRadar,
    Energy,
    FrameKind,
    Item,
    Pos,
    RadarBotData,
    RadarData,
    Subsystems,
    Team,
};

use crate::{
    apply_actions::{ActionContainer, ActionState, CurrentAction, PastActions},
    types::{GridWorld, Tick},
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
                "/Users/jh/personal/swarm-skirmish/target/debug/\
                 libsimple_bots.dylib",
            )
        }
        .expect("Could not open library or load symbols");

        app.add_systems(
            Update,
            (
                ensure_bot_id,
                update_known_maps,
                create_server_updates.pipe(update_bots),
            )
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
    query: Query<Entity, (With<BotData>, Without<BotId>)>,
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
                    idx: 1.min(path.len().saturating_sub(1)),
                },
                Action::Noop => ActionState::None,
                Action::MoveDir(_) => ActionState::None,
                Action::Harvest(_) => ActionState::None,
                Action::Pickup(_) => ActionState::None,
                Action::Drop(_) => ActionState::None,
                Action::Transfer(_) => ActionState::None,
                Action::Build(_dir, _building_kind, _subsystems) => {
                    ActionState::None
                }
                Action::Recharge(_dir) => ActionState::None,
                Action::Attack(_dir) => ActionState::None,
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

pub fn update_known_maps(
    tick: Res<Tick>,
    mut query: Query<(&BotId, &mut BotData)>,
    grid_world: Res<GridWorld>,
) {
    use std::mem::{replace, swap, take};

    // We need to swap the known map and known bots for each bot so that we
    // can pass the known map to the update_known_map function.
    // This is cheap because bot Vec and Array2D are essentially pointers to the
    // heap
    let mut maps = HashMap::new();
    for (bot_id, mut bot_data) in query.iter_mut() {
        let map =
            replace(&mut bot_data.known_map, KnownMap::new(0, 0, default()));
        let known_bots = take(&mut bot_data.known_bots);
        maps.insert(*bot_id, (map, known_bots));
    }

    // Update the known map for each bot
    for (bot_id, bot_data) in query.iter() {
        let (map, known_bots) = maps.get_mut(bot_id).unwrap();
        let _radar_updates = update_known_map(
            known_bots,
            map,
            tick.0,
            &bot_data.pos,
            &grid_world,
            |e| query.get(e).ok(),
        );
    }

    // Swap the known map and known bots back for each bot
    for (bot_id, mut bot_data) in query.iter_mut() {
        let (map, known_bots) = maps.get_mut(bot_id).unwrap();
        swap(map, &mut bot_data.known_map);
        swap(known_bots, &mut bot_data.known_bots);
    }
}

pub fn create_server_updates(
    tick: Res<Tick>,
    query: Query<(&BotId, &BotData, &CurrentAction, &PastActions)>,
) -> HashMap<BotId, BotUpdate> {
    query
        .iter()
        .map(|(bot_id, bot_data, current_action, past_actions)| {
            (
                *bot_id,
                BotUpdate {
                    tick: tick.0,
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
                    bot_data: bot_data.clone(),
                },
            )
        })
        .collect()
}

fn _create_radar_data<'a>(
    pos: &Pos,
    grid_world: &GridWorld,
    get: impl Fn(
        Entity,
    )
        -> Option<(&'a BotId, &'a Team, &'a FrameKind, &'a Subsystems)>,
) -> RadarData {
    // Define the radar range (how far to look in each direction)
    // This gives a view of 11x11 cells centered on the bot (5 in each
    // direction)
    let radar_range = 5;

    // Get bot's world coordinates
    let (bot_world_x, bot_world_y) = pos.as_isize();

    let mut radar = RadarData {
        center_world_pos: *pos,
        pawns: Vec::new(),
        cells: Vec::new(),
    };

    // Use nearby to get cells in radar range with Manhattan distance
    grid_world
        .nearby(*pos, radar_range)
        .for_each(|(pos, cell)| {
            let cell_pos = Pos::from(pos);

            let radar_cell = CellStateRadar {
                kind: match cell.kind {
                    CellKind::Empty => CellKind::Empty,
                    CellKind::Blocked => CellKind::Blocked,
                    CellKind::Unknown => unreachable!(),
                },
                pawn: cell.pawn.map(|e| {
                    let (bot_id, &team, frame, subsystems) = get(e).unwrap();

                    // Store the bot's position in world coordinates
                    radar.pawns.push(RadarBotData {
                        team,
                        pos: cell_pos,
                        bot_id: bot_id.0,
                        frame: frame.clone(),
                        subsystems: subsystems.clone(),
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
        let dx = (cell.pos.x() as isize - bot_world_x).unsigned_abs() as u32;
        let dy = (cell.pos.y() as isize - bot_world_y).unsigned_abs() as u32;
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

#[allow(dead_code)]
enum RadarUpdate {
    NewBlocker { pos: Pos },
    NewItem { item: Item, pos: Pos },
    NewBot { bot_id: BotId, pos: Pos },
    BotMoved { bot_id: BotId, from: Pos, to: Pos },
    BotReseen { bot_id: BotId, pos: Pos },
}

fn update_known_map<'a>(
    known_bots: &mut Vec<ClientBotData>,
    known_map: &mut KnownMap,
    current_tick: u32,
    pos: &Pos,
    grid_world: &GridWorld,
    get_data: impl Fn(Entity) -> Option<(&'a BotId, &'a BotData)>,
) -> Vec<RadarUpdate> {
    // Define the radar range (how far to look in each direction)
    // This gives a view of 11x11 cells centered on the bot (5 in each
    // direction)
    let radar_range = 5;

    // Get bot's world coordinates
    let mut radar_pawn_ents = Vec::new();
    let mut radar_updates = Vec::new();

    for (pos, cell) in grid_world.nearby(*pos, radar_range) {
        let known_cell = known_map.get_mut(pos);

        let was_unknown = known_cell.is_unknown();
        if was_unknown {
            if cell.kind == CellKind::Blocked {
                radar_updates.push(RadarUpdate::NewBlocker { pos });
            }
            if let Some(item) = cell.item {
                radar_updates.push(RadarUpdate::NewItem { item, pos });
            }
        }

        if let Some(e) = cell.pawn {
            radar_pawn_ents.push(e);

            let (bot_id, _) = get_data(e).unwrap();
            known_cell.pawn = Some(bot_id.0);
        }

        known_cell.kind = cell.kind;
        known_cell.item = cell.item;
        known_cell.last_observed = current_tick;
    }

    for radar_bot_e in radar_pawn_ents {
        let (&bot_id, bot) = get_data(radar_bot_e).unwrap();

        // Check if we already know about this bot
        let known_bot = known_bots.iter_mut().find(|b| b.bot_id == bot_id.0);

        if let Some(known_bot) = known_bot {
            // If position changed, remove bot from old position in the grid
            if known_bot.pos != bot.pos {
                let update = if known_bot.last_observed + 1 < current_tick {
                    RadarUpdate::BotReseen {
                        bot_id,
                        pos: bot.pos,
                    }
                } else {
                    RadarUpdate::BotMoved {
                        bot_id,
                        from: known_bot.pos,
                        to: bot.pos,
                    }
                };
                radar_updates.push(update);

                // Find the cell at the old position and clear its pawn
                let old_cell = known_map.get_mut(known_bot.pos);
                if old_cell.pawn == Some(bot_id.0) {
                    old_cell.pawn = None;
                }
            }

            // Update existing bot data
            known_bot.pos = bot.pos;
            known_bot.last_observed = current_tick;
        } else {
            radar_updates.push(RadarUpdate::NewBot {
                bot_id,
                pos: bot.pos,
            });

            // Add new bot data
            known_bots.push(ClientBotData {
                bot_id: bot_id.0,
                team: bot.team,
                pos: bot.pos,
                last_observed: current_tick,
                frame: bot.frame,
                subsystems: bot.subsystems.clone(),
            });
        }
    }

    radar_updates
}
