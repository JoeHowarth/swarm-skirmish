use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Write},
};

use bevy::{ecs::entity::EntityHashMap, prelude::*, utils::HashMap};
use serde::{Deserialize, Serialize};
use swarm_lib::BotData;

use crate::{
    game::{
        apply_actions::{CurrentAction, PastActions},
        bot_update::{BotId, BotIdToEntity},
    },
    graphics::tilemap::MapSize,
    types::*,
    GameState,
};

#[derive(Clone, Serialize, Deserialize, Resource)]
struct Replay {
    ticks: Vec<TickData>,
    replay_entity_to_bot_id: EntityHashMap<BotId>,
}

#[derive(Clone, Serialize, Deserialize)]
struct TickData {
    tick: u32,
    bot_data: HashMap<BotId, BotComponents>,
    grid_world: GridWorld,
    partially_built_bots: EntityHashMap<PartiallyBuiltBot>,
}

#[derive(Clone, Serialize, Deserialize)]
struct BotComponents {
    bot_data: BotData,
    current_action: CurrentAction,
    past_actions: PastActions,
}

pub struct ReplayPlugin {
    // pub save_replay: String,
    pub load_replay: Option<String>,
}

#[derive(States, Hash, Eq, PartialEq, Clone, Debug)]
pub enum LiveOrReplay {
    Replay,
    Live,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct ReplaySystemSet;

impl Plugin for ReplayPlugin {
    fn build(&self, app: &mut App) {
        if let Some(load_replay_file) = &self.load_replay {
            let replay = load_replay(load_replay_file);
            app.insert_state(LiveOrReplay::Replay);
            app.add_systems(
                OnEnter(GameState::Idle),
                |mut next_state: ResMut<NextState<GameState>>| {
                    next_state.set(GameState::InGame);
                },
            );
            app.insert_resource(MapSize {
                x: replay.ticks[0].grid_world.width() as u32,
                y: replay.ticks[0].grid_world.height() as u32,
            });
            app.insert_resource(replay.ticks[0].grid_world.clone());
            app.insert_resource(replay);
        } else {
            app.insert_resource(Replay {
                ticks: Vec::new(),
                replay_entity_to_bot_id: EntityHashMap::default(),
            });
            app.insert_state(LiveOrReplay::Live);
        }

        app.insert_resource(ReplayEntityToLiveEntity(EntityHashMap::default()));
        app.add_systems(
            Update,
            restore_replay_at_tick
                .in_set(ReplaySystemSet)
                .run_if(in_state(LiveOrReplay::Replay)),
        );
        app.add_systems(
            Update,
            (extract_live_data, save_replay)
                .in_set(ReplaySystemSet)
                .run_if(in_state(LiveOrReplay::Live)),
        );
    }
}

fn load_replay(path: &str) -> Replay {
    let file = BufReader::new(File::open(path).unwrap());

    let mut replay = Replay {
        ticks: Vec::new(),
        replay_entity_to_bot_id: EntityHashMap::default(),
    };

    for line in file.lines() {
        let line = line.unwrap();
        let tick_data: TickData = serde_json::from_str(&line).unwrap();
        replay.ticks.push(tick_data);
    }

    let mapping_path = path.replace(".json", "_entity_to_live_entity.json");
    let mut file = BufReader::new(File::open(mapping_path).unwrap());
    replay.replay_entity_to_bot_id =
        serde_json::from_reader(&mut file).unwrap();
    replay
}

fn save_replay(replay: Res<Replay>) {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("replays/replay.json")
        .unwrap();
    let mut file = BufWriter::new(file);

    let bytes = serde_json::to_vec(&replay.ticks.last().unwrap()).unwrap();
    file.write_all(&bytes).unwrap();
    file.write_all(b"\n").unwrap();
    file.flush().unwrap();

    let mut file = BufWriter::new(
        File::create("replays/replay_entity_to_live_entity.json").unwrap(),
    );
    serde_json::to_writer(&mut file, &replay.replay_entity_to_bot_id).unwrap();
}

fn extract_live_data(
    mut replay: ResMut<Replay>,
    tick: Res<Tick>,
    bots: Query<(&BotId, &BotData, &CurrentAction, &PastActions)>,
    partially_built_bots: Query<(Entity, &PartiallyBuiltBot)>,
    grid_world: Res<GridWorld>,
) {
    let bot_data = bots
        .iter()
        .map(|(&bot_id, bot_data, current_action, past_actions)| {
            (
                bot_id,
                BotComponents {
                    bot_data: bot_data.clone(),
                    current_action: current_action.clone(),
                    past_actions: past_actions.clone(),
                },
            )
        })
        .collect();

    let partially_built_bots = partially_built_bots
        .iter()
        .map(|(replay_entity, partially_built_bot)| {
            (replay_entity, partially_built_bot.clone())
        })
        .collect();

    replay.ticks.push(TickData {
        tick: tick.0,
        bot_data,
        grid_world: grid_world.clone(),
        partially_built_bots,
    });
}

#[derive(Resource)]
struct ReplayEntityToLiveEntity(pub EntityHashMap<Entity>);

/// Set up game state from replay at a given tick
fn restore_replay_at_tick(
    mut commands: Commands,
    tick: Res<Tick>,
    replay: Res<Replay>,
    bot_id_to_entity: Res<BotIdToEntity>,
    mut bots: Query<(&mut BotData, &mut CurrentAction, &mut PastActions)>,
    mut partially_built_bots: Query<&mut PartiallyBuiltBot>,
    mut replay_entity_to_live_entity: ResMut<ReplayEntityToLiveEntity>,
    mut grid_world: ResMut<GridWorld>,
) {
    let Some(tick_data) = replay.ticks.get(tick.0 as usize) else {
        warn!("No tick data found for tick {}", tick.0);
        return;
    };
    for (bot_id, components) in tick_data.bot_data.iter() {
        // Look up the entity in the replay. This may or may not be the same as
        // the entity in *this* bevy world
        let cell = tick_data.grid_world.get(components.bot_data.pos);
        let replay_entity = cell.pawn.unwrap();

        // If the entity exists in this bevy world, update the bot data
        let entity = bot_id_to_entity.0.get(bot_id);
        if let Some((mut bot_data, mut current_action, mut past_actions)) =
            entity.and_then(|entity| bots.get_mut(*entity).ok())
        {
            // Make sure the entity in the replay is mapped correctly to the
            // entity in this bevy world
            assert_eq!(
                replay_entity_to_live_entity.0.get(&replay_entity),
                entity,
                "Replay entity {} should map to live entity {}. Bot ID: {}",
                replay_entity,
                entity.unwrap(),
                bot_id.0
            );
            *bot_data = components.bot_data.clone();
            current_action.0 = components.current_action.0.clone();
            past_actions.0 = components.past_actions.0.clone();
            continue;
        }

        // If the entity doesn't exist in this bevy world, spawn a new one
        let live_entity = commands
            .spawn((
                components.bot_data.clone(),
                components.current_action.clone(),
                components.past_actions.clone(),
            ))
            .id();

        // Map the replay entity to the live entity
        replay_entity_to_live_entity
            .0
            .insert(replay_entity, live_entity);
    }

    // Create partially built bots
    for (replay_entity, partial) in &tick_data.partially_built_bots {
        // If the entity exists in this bevy world, update the partially built
        // bot
        if let Some(live_entity) =
            replay_entity_to_live_entity.0.get(replay_entity)
        {
            *partially_built_bots.get_mut(*live_entity).unwrap() =
                partial.clone();
            continue;
        }

        // If the entity doesn't exist in this bevy world, spawn a new one
        let live_entity = commands.spawn(partial.clone()).id();

        // Map the replay entity to the live entity
        replay_entity_to_live_entity
            .0
            .insert(*replay_entity, live_entity);
    }

    // Update the grid world
    *grid_world = tick_data.grid_world.clone();

    // Resolve entity mappings
    for (x, y) in tick_data.grid_world.grid.indices_row_major() {
        let cell = grid_world.get_tuple_mut(x, y);

        if let Some(replay_entity) = cell.pawn {
            let live_entity = replay_entity_to_live_entity
                .0
                .get(&replay_entity)
                .expect("Should have a live entity for the replay entity");
            cell.pawn = Some(*live_entity);
        }

        if let Some(replay_entity) = cell.partially_built_bot {
            let live_entity = replay_entity_to_live_entity
                .0
                .get(&replay_entity)
                .expect("Should have a live entity for the replay entity");
            cell.partially_built_bot = Some(*live_entity);
        }
    }
}
