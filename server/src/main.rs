#![allow(unused_imports, dead_code)]
#![feature(mpmc_channel)]

use std::{net::TcpListener, time::Duration};

use bevy::prelude::*;
use bot_handler::{ActionRecv, BotHandlerPlugin, QueryRecv};
use gridworld::GridWorld;
use swarm_lib::{Action, BotMsg, BotMsgEnvelope, QueryEnvelope, Team};

mod bot_handler;
mod gridworld;
mod tilemap;

#[derive(Event)]
struct MoveEvent {
    entity: Entity,
    dx: i32,
    dy: i32,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins((tilemap::TilemapPlugin, BotHandlerPlugin))
        .add_event::<MoveEvent>()
        .add_systems(Startup, (camera_setup, init_map))
        .add_systems(
            Update,
            (
                keyboard_movement,
                (handle_bot_actions, handle_bot_queries, handle_movement)
                    .chain(),
                exit_system,
            ),
        )
        .run();
}

fn handle_bot_actions(
    mut action_recv: ResMut<ActionRecv>,
    mut move_events: EventWriter<MoveEvent>,
    query: Query<(Entity, &Team)>,
) {
    while let Ok((bot_id, action)) = action_recv.0.try_recv() {
        match action {
            Action::Move(dx, dy) => {
                // Find the entity associated with this bot
                if let Some((entity, &Team::Player)) =
                    query.iter().find(|(_, team)| **team == Team::Player)
                {
                    move_events.send(MoveEvent {
                        entity,
                        dx: dx as i32,
                        dy: dy as i32,
                    });
                }
            }
        }
    }
}

fn handle_bot_queries(
    mut query_recv: ResMut<QueryRecv>,
    grid_world: Res<GridWorld>,
    query: Query<(Entity, &Team)>,
) {
    while let Ok((bot_id, query_envelope)) = query_recv.0.try_recv() {
        // Handle queries here
        // TODO: Implement query handling
    }
}

fn init_map(mut commands: Commands) {
    let mut grid_world = GridWorld::new(16, 16);

    let player = commands.spawn((Pawn, Team::Player)).id();
    let enemy = commands.spawn((Pawn, Team::Enemy)).id();

    grid_world.set(2, 2, CellState::Pawn(player));
    grid_world.set(13, 13, CellState::Pawn(enemy));

    for y in 1..10 {
        grid_world.set(10, y, CellState::Blocked);
    }

    commands.insert_resource(grid_world);
}

fn keyboard_movement(
    keyboard: Res<ButtonInput<KeyCode>>,
    query: Query<Entity, With<Pawn>>,
    mut move_events: EventWriter<MoveEvent>,
    mut cooldown: Local<Option<Timer>>,
    time: Res<Time>,
) {
    if let Some(timer) = cooldown.as_mut() {
        timer.tick(time.delta());
        if timer.finished() {
            *cooldown = None;
        } else {
            return;
        }
    }

    let (dx, dy) = if keyboard.pressed(KeyCode::KeyW) {
        (0, 1)
    } else if keyboard.pressed(KeyCode::KeyS) {
        (0, -1)
    } else if keyboard.pressed(KeyCode::KeyA) {
        (-1, 0)
    } else if keyboard.pressed(KeyCode::KeyD) {
        (1, 0)
    } else {
        return;
    };

    *cooldown =
        Some(Timer::new(Duration::from_secs_f32(0.25), TimerMode::Once));

    if let Some(player) = query.iter().next() {
        move_events.send(MoveEvent {
            entity: player,
            dx,
            dy,
        });
    }
}

fn handle_movement(
    mut move_events: EventReader<MoveEvent>,
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
            let new_x = x as isize + event.dx as isize;
            let new_y = y as isize + event.dy as isize;

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
