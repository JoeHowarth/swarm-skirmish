use std::collections::VecDeque;

use bevy::prelude::*;
use swarm_lib::{
    Action,
    Dir,
    Team,
};

use crate::{
    gridworld::GridWorld,
    server::{
        ActionRecv,
        BotId,
        BotIdToEntity,
    },
    Pos,
};

#[derive(Event)]
struct MoveEvent {
    entity: Entity,
    dir: Dir,
}

#[derive(Component, Default)]
pub struct ActionQueue(VecDeque<Action>);

pub struct ActionsPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct ActionsSystemSet;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<MoveEvent>().add_systems(
            Update,
            (
                handle_incoming_bot_actions,
                handle_bot_actions,
                handle_movement,
            )
                .chain()
                .in_set(ActionsSystemSet),
        );
    }
}

fn handle_incoming_bot_actions(
    action_recv: Res<ActionRecv>,
    mut queues: Query<&mut ActionQueue>,
    bot_id_to_entity: Res<BotIdToEntity>,
) {
    while let Ok((bot_id, action)) = action_recv.0.try_recv() {
        let entity = bot_id_to_entity.0.get(&bot_id).unwrap();
        debug!(
            bot_id = bot_id.0,
            ?action,
            entity = entity.index(),
            "Received bot action"
        );
        queues.get_mut(*entity).unwrap().0.push_back(action);
    }
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

                        let Some(dir) = Dir::from_deltas_ivec(diff) else {
                            panic!(
                                "Invalid path step from {:?} to {:?}",
                                current, next
                            );
                        };
                        Action::MoveDir(dir)
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

fn handle_movement(
    mut move_events: EventReader<MoveEvent>,
    mut positions: Query<&mut Pos>,
    mut grid_world: ResMut<GridWorld>,
) {
    for event in move_events.read() {
        let mut current_pos = None;
        for ((x, y), cell) in grid_world.iter() {
            if let Some(e) = cell.pawn {
                if e == event.entity {
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

                if grid_world.get(new_x, new_y).can_enter() {
                    grid_world.get_mut(x, y).pawn = None;
                    grid_world.get_mut(new_x, new_y).pawn = Some(event.entity);
                    positions.get_mut(event.entity).unwrap().0 =
                        UVec2::new(new_x as u32, new_y as u32);
                }
            }
        }
    }
}
