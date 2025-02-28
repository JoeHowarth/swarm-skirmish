#![allow(unused_imports)]
#![feature(mpmc_channel)]

use std::{collections::VecDeque, net::TcpListener, time::Duration};

use array2d::Array2D;
use bevy::{
    prelude::*,
    time::common_conditions::on_timer,
    utils::{HashMap, HashSet},
};
use bot_handler::{
    ActionRecv,
    BotHandlerPlugin,
    BotIdToEntity,
    ServerUpdates,
    SubscriptionRecv,
};
use gridworld::GridWorld;
use swarm_lib::{
    Action,
    BotMsgEnvelope,
    CellStateRadar,
    Dir,
    RadarBotData,
    RadarData,
    ServerUpdate,
    ServerUpdateEnvelope,
    SubscriptionType,
    Team,
};

mod bot_handler;
mod gridworld;
mod tilemap;

#[derive(Event)]
struct MoveEvent {
    entity: Entity,
    dir: Dir,
}

#[derive(Component)]
pub struct Pos(pub UVec2);

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                resolution: (500.0, 500.0).into(),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((tilemap::TilemapPlugin, BotHandlerPlugin))
        .add_event::<MoveEvent>()
        .add_systems(Startup, (camera_setup, init_map))
        .init_resource::<Tick>()
        .add_systems(
            Update,
            (
                (
                    update_tick,
                    send_server_updates,
                    handle_bot_subscriptions,
                    handle_incoming_bot_actions,
                    handle_bot_actions,
                    handle_movement,
                )
                    .chain()
                    .run_if(on_timer(Duration::from_millis(250))),
                exit_system,
            ),
        )
        .run();
}

fn update_tick(mut tick: ResMut<Tick>) {
    tick.0 += 1;
}

#[derive(Component, Debug, Clone, Copy, Hash, PartialEq, Eq)]
#[require(ActionQueue, Subscriptions)]
pub struct BotId(pub u32);

#[derive(Resource, Default)]
pub struct Tick(pub u32);

#[derive(Component, Default)]
pub struct ActionQueue(VecDeque<Action>);

#[derive(Component, Default)]
pub struct Subscriptions(HashSet<SubscriptionType>);

fn handle_incoming_bot_actions(
    action_recv: Res<ActionRecv>,
    mut queues: Query<&mut ActionQueue>,
    bot_id_to_entity: Res<BotIdToEntity>,
) {
    while let Ok((bot_id, action)) = action_recv.0.try_recv() {
        let entity = bot_id_to_entity.0.get(&bot_id).unwrap();
        queues.get_mut(*entity).unwrap().0.push_back(action);
    }
}

fn send_server_updates(
    update_tx: Res<ServerUpdates>,
    tick: Res<Tick>,
    query: Query<(&BotId, &Pos, &Team, &Subscriptions)>,
    grid_world: Res<GridWorld>,
) {
    for (bot_id, pos, team, subscriptions) in query.iter() {
        let update = ServerUpdateEnvelope {
            bot_id: bot_id.0,
            seq: 0,
            response: ServerUpdate {
                tick: tick.0,
                team: subscriptions
                    .0
                    .get(&SubscriptionType::Team)
                    .map(|_| *team),
                position: subscriptions
                    .0
                    .get(&SubscriptionType::Position)
                    .map(|_| pos.0),
                radar: subscriptions
                    .0
                    .get(&SubscriptionType::Radar)
                    .map(|_| create_radar_data(pos, &grid_world, &query)),
            },
        };

        update_tx.0.send(update).unwrap();
    }
}

fn create_radar_data(
    pos: &Pos,
    grid_world: &GridWorld,
    query: &Query<(&BotId, &Pos, &Team, &Subscriptions)>,
) -> RadarData {
    // Create a radar with a 10x10 grid centered on the bot
    let radar_size = 10;
    let radar_center = radar_size / 2; // Center point (5 for a 10x10 grid)

    let mut radar = RadarData {
        bots: Vec::new(),
        cells: Array2D::filled_with(
            CellStateRadar::Unknown,
            radar_size,
            radar_size,
        ),
    };

    // Get bot's world coordinates
    let bot_world_x = pos.0.x as isize;
    let bot_world_y = pos.0.y as isize;

    // Use nearby to get cells in radar range with Manhattan distance
    grid_world
        .nearby(bot_world_x as usize, bot_world_y as usize, radar_center)
        .for_each(|((world_x, world_y), cell)| {
            // Calculate radar coordinates (relative to bot at center)
            let world_x = world_x as isize;
            let world_y = world_y as isize;

            let dx = world_x - bot_world_x;
            let dy = world_y - bot_world_y;

            let radar_x = (radar_center as isize + dx) as usize;
            let radar_y = (radar_center as isize + dy) as usize;

            // Skip if outside radar bounds
            if radar_x >= radar_size || radar_y >= radar_size {
                return;
            }

            let radar_cell = match cell {
                CellState::Empty => CellStateRadar::Empty,
                CellState::Blocked => CellStateRadar::Blocked,
                CellState::Pawn(entity) => {
                    // Get the bot's team
                    let team = match query.get(*entity) {
                        Ok((_, _, team, _)) => *team,
                        Err(_) => {
                            warn!("Entity not found in query: {:?}", entity);
                            return; // Skip this entity
                        }
                    };

                    // Store the bot's position in radar coordinates
                    radar.bots.push(RadarBotData {
                        team,
                        pos: UVec2::new(radar_x as u32, radar_y as u32),
                    });

                    CellStateRadar::Bot {
                        idx: radar.bots.len() - 1,
                    }
                }
            };

            radar.cells.set(radar_x, radar_y, radar_cell).unwrap();
        });

    // Set all cells that would be outside the map bounds to Blocked instead of
    // Unknown
    let map_width = grid_world.width() as isize;
    let map_height = grid_world.height() as isize;

    // Iterate through all radar cells
    for radar_x in 0..radar_size {
        for radar_y in 0..radar_size {
            // Calculate the corresponding world coordinates
            let world_x =
                bot_world_x + (radar_x as isize - radar_center as isize);
            let world_y =
                bot_world_y + (radar_y as isize - radar_center as isize);

            // Check if the world coordinates are outside the map bounds
            if world_x < 0
                || world_x >= map_width
                || world_y < 0
                || world_y >= map_height
            {
                // If the cell is still Unknown (i.e., it wasn't filled by a
                // valid map cell), mark it as Blocked
                if let CellStateRadar::Unknown =
                    radar.cells.get(radar_x, radar_y).unwrap()
                {
                    radar
                        .cells
                        .set(radar_x, radar_y, CellStateRadar::Blocked)
                        .unwrap();
                }
            }
        }
    }

    radar
}

fn handle_bot_actions(
    mut move_events: EventWriter<MoveEvent>,
    mut query: Query<(Entity, &BotId, &Team, &Pos, &mut ActionQueue)>,
    grid_world: Res<GridWorld>,
) {
    for (entity, bot_id, _team, pos, mut actions) in query.iter_mut() {
        let Some(action) = actions.0.pop_front() else {
            continue;
        };
        match action {
            Action::MoveDir(dir) => {
                move_events.send(MoveEvent { entity, dir });
            }
            Action::MoveTo(goal) => {
                let Some(path) = grid_world.find_path(pos.0, goal) else {
                    warn!(?goal, ?bot_id, "Invalid goal");
                    continue;
                };

                // Convert path coordinates to relative movement directions
                let move_actions: Vec<Action> = path
                    .windows(2)
                    .map(|window| {
                        let current = window[0];
                        let next = window[1];
                        let diff = next.as_ivec2() - current.as_ivec2();

                        match (diff.x, diff.y) {
                            (0, 1) => Action::MoveDir(Dir::Down),
                            (0, -1) => Action::MoveDir(Dir::Up),
                            (1, 0) => Action::MoveDir(Dir::Right),
                            (-1, 0) => Action::MoveDir(Dir::Left),
                            _ => {
                                warn!(
                                    "Invalid path step from {:?} to {:?}",
                                    current, next
                                );
                                Action::MoveDir(Dir::Right) // Fallback direction
                            }
                        }
                    })
                    .collect();

                // Add all movement actions to the queue
                for action in move_actions {
                    actions.0.push_back(action);
                }
            }
        }
    }
}

fn handle_bot_subscriptions(
    mut commands: Commands,
    query_recv: ResMut<SubscriptionRecv>,
    mut query: Query<&mut Subscriptions>,
    bot_id_to_entity: Res<BotIdToEntity>,
) {
    while let Ok((bot_id, new_subscriptions)) = query_recv.0.try_recv() {
        let entity = bot_id_to_entity.0.get(&bot_id).unwrap();
        let mut set = query.get_mut(*entity).unwrap();
        set.0.extend(new_subscriptions);
    }
}

fn init_map(mut commands: Commands) {
    let mut grid_world = GridWorld::new(16, 16);

    let player = commands
        .spawn((Pawn, Team::Player, Pos((2, 2).into())))
        .id();
    // let enemy = commands
    //     .spawn((Pawn, Team::Enemy, Pos((13, 13).into())))
    //     .id();

    grid_world.set(2, 2, CellState::Pawn(player));
    // grid_world.set(13, 13, CellState::Pawn(enemy));

    for y in 1..10 {
        grid_world.set(10, y, CellState::Blocked);
    }

    commands.insert_resource(grid_world);
}

// fn keyboard_movement(
//     keyboard: Res<ButtonInput<KeyCode>>,
//     query: Query<(Entity, &Team, &mut ActionQueue)>,
//     mut cooldown: Local<Option<Timer>>,
//     time: Res<Time>,
// ) {
//     if let Some(timer) = cooldown.as_mut() {
//         timer.tick(time.delta());
//         if timer.finished() {
//             *cooldown = None;
//         } else {
//             return;
//         }
//     }

//     let dir = if keyboard.pressed(KeyCode::KeyW) {
//         Dir::Up
//     } else if keyboard.pressed(KeyCode::KeyS) {
//         Dir::Down
//     } else if keyboard.pressed(KeyCode::KeyA) {
//         Dir::Left
//     } else if keyboard.pressed(KeyCode::KeyD) {
//         Dir::Right
//     } else {
//         return;
//     };

//     *cooldown =
//         Some(Timer::new(Duration::from_secs_f32(0.25), TimerMode::Once));

//     if let Some(player) = query.iter().next() {
//         move_events.send(MoveEvent {
//             entity: player,
//             dir,
//         });
//     }
// }

fn handle_movement(
    mut move_events: EventReader<MoveEvent>,
    mut positions: Query<&mut Pos>,
    mut grid_world: ResMut<GridWorld>,
) {
    for event in move_events.read() {
        let mut current_pos = None;
        for ((x, y), cell) in grid_world.iter() {
            if let CellState::Pawn(e) = cell {
                if *e == event.entity {
                    current_pos = Some((x, y));
                    break;
                }
            }
        }

        if let Some((x, y)) = current_pos {
            let d = event.dir.to_deltas();

            let new_x = x as isize + d.0;
            let new_y = y as isize + d.1;

            if new_x >= 0
                && new_x < grid_world.width() as isize
                && new_y >= 0
                && new_y < grid_world.height() as isize
            {
                let new_x = new_x as usize;
                let new_y = new_y as usize;

                if grid_world.get(new_x, new_y) == CellState::Empty {
                    grid_world.set(x, y, CellState::Empty);
                    grid_world.set(new_x, new_y, CellState::Pawn(event.entity));
                    positions.get_mut(event.entity).unwrap().0 =
                        UVec2::new(new_x as u32, new_y as u32);
                }
            }
        }
    }
}

#[derive(Component)]
struct Pawn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CellState {
    #[default]
    Empty,
    Blocked,
    Pawn(Entity),
}

pub fn camera_setup(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        bevy_pancam::PanCam {
            move_keys: bevy_pancam::DirectionKeys::arrows(),
            grab_buttons: vec![MouseButton::Right],
            ..default()
        },
    ));
}

pub fn exit_system(
    keys: Res<ButtonInput<KeyCode>>,
    mut exit: EventWriter<AppExit>,
) {
    if keys.all_pressed([KeyCode::ControlLeft, KeyCode::KeyC]) {
        exit.send(AppExit::Success);
    }
}
