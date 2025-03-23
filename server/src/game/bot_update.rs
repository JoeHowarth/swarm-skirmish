use bevy::{prelude::*, utils::HashMap};
use dlopen2::wrapper::{Container, WrapperApi};
use serde::{Deserialize, Serialize};
use swarm_lib::{
    bot_logger::{BotLogger, LogEntry},
    known_map::{ClientBotData, KnownMap},
    Action,
    ActionResult,
    ActionStatus,
    ActionWithId,
    Bot,
    BotData,
    BotUpdate,
    CellKind,
    Item,
    Pos,
};
use ustr::ustr;

use crate::{
    game::apply_actions::{
        ActionContainer,
        ActionState,
        CurrentAction,
        PastActions,
    },
    types::{GridWorld, Tick},
};

#[derive(WrapperApi)]
struct Api {
    new_bot: fn(bot_logger: BotLogger) -> Box<dyn Bot>,
}

#[derive(Resource)]
struct BotLib(pub Container<Api>);

#[derive(
    Component, Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize,
)]
#[require(CurrentAction, PastActions, BotLogs)]
pub struct BotId(pub u32);

#[derive(Component, Default, Serialize, Deserialize, Clone)]
pub struct BotLogs(pub Vec<LogEntry>);

pub struct BotUpdatePlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct BotUpdateSystemSet;

#[derive(Resource)]
pub struct NextBotId(pub u32);

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
            (update_known_maps, update_bots)
                .chain()
                .in_set(BotUpdateSystemSet),
        )
        .insert_resource(BotLib(cont))
        .insert_resource(NextBotId(0))
        .init_resource::<BotIdToEntity>();

        app.world_mut()
            .register_component_hooks::<BotData>()
            .on_add(|mut world, entity, _| {
                // Get the bot ID from the entity or create a new one
                let bot_id = world
                    .entity(entity)
                    .get::<BotId>()
                    .cloned()
                    .unwrap_or_else(|| {
                        let mut next_bot_id = world.resource_mut::<NextBotId>();
                        let bot_id = BotId(next_bot_id.0);
                        next_bot_id.0 += 1;
                        bot_id
                    });

                // Insert the bot ID -> Entity mapping
                world
                    .resource_mut::<BotIdToEntity>()
                    .0
                    .insert(bot_id, entity);

                if world.entity(entity).get::<BotInstance>().is_none() {
                    info!("Creating new bot instance for bot ID: {}", bot_id.0);
                    let bot = world
                        .resource::<BotLib>()
                        .0
                        .new_bot(BotLogger::new(bot_id.0));

                    // Insert the bot ID and instance into the entity
                    world
                        .commands()
                        .entity(entity)
                        .insert((bot_id, BotInstance { bot }));
                } else {
                    // Insert the bot ID into the entity
                    world.commands().entity(entity).insert(bot_id);
                }
            });
    }
}

#[derive(Component)]
pub struct BotInstance {
    pub bot: Box<dyn Bot>,
}

#[derive(Resource, Default)]
pub struct BotIdToEntity(pub HashMap<BotId, Entity>);

impl BotIdToEntity {
    pub fn mapper<'a>(&'a self) -> impl Fn(BotId) -> Entity + 'a {
        |bot_id| *self.0.get(&bot_id).unwrap()
    }

    pub fn u32<'a>(&'a self) -> impl Fn(u32) -> Entity + 'a {
        |bot_id| *self.0.get(&BotId(bot_id)).unwrap()
    }

    pub fn to_entity(&self, bot_id: BotId) -> Entity {
        *self.0.get(&bot_id).unwrap()
    }
}

fn update_bots(
    tick: Res<Tick>,
    // mut updates: In<HashMap<BotId, BotUpdate>>,
    mut query: Query<(
        Entity,
        &BotId,
        &BotData,
        &mut CurrentAction,
        &mut PastActions,
        &mut BotInstance,
        &mut BotLogs,
    )>,
) {
    for (
        entity,
        bot_id,
        bot_data,
        mut current_action,
        mut past_actions,
        mut bot_instance,
        mut bot_logs,
    ) in query.iter_mut()
    {
        debug!(?bot_id, entity = entity.index(), "Updating bot");
        // let server_update = updates.remove(bot_id).unwrap();

        let server_update = BotUpdate {
            tick: tick.0,
            in_progress_action: {
                current_action.as_ref().as_ref().map(|action_container| {
                    ActionWithId {
                        action: action_container.kind.clone(),
                        id: action_container.id,
                        reason: action_container.reason.as_str(),
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
                            reason: action.reason,
                            completed_tick: tick.0,
                        })
                    } else {
                        None
                    }
                })
            },
            bot_data: bot_data.clone(),
        };

        let (maybe_action, logs) =
            bot_instance.bot.update(server_update.clone());

        bot_logs.0 = logs;

        let Some(action) = maybe_action else {
            debug!("No action from bot ID: {}", bot_id.0);
            continue;
        };

        trace!("Bot ID: {} action: {:?}", bot_id.0, action);
        let action_container = ActionContainer {
            reason: ustr(action.reason),
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
                Action::Msg { .. } => ActionState::None,
                Action::ShareMap { .. } => ActionState::None,
            },
            kind: action.action,
            id: action.id,
        };

        // If there is a current action already, cancel it
        if let Some(action) = current_action.0.replace(action_container) {
            past_actions.push(ActionResult {
                action: action.kind,
                id: action.id,
                reason: action.reason,
                status: ActionStatus::Cancelled,
                completed_tick: server_update.tick,
            });
        }
    }
}

fn update_known_maps(
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
                let new_cell = known_map.get(bot.pos);
                assert_eq!(new_cell.pawn, Some(bot_id.0));
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
