use std::collections::VecDeque;

use bevy::prelude::*;
use swarm_lib::{
    Action,
    ActionEnvelope,
    ActionId,
    ActionResult,
    ActionStatus,
    ActionStatusDiscriminants,
    Dir,
    Energy,
    Team,
};

use crate::{
    bot_update::{BotId, BotIdToEntity},
    core::{Inventory, SGridWorld, Tick},
    Pos,
};

/// High-level action queue with actions sent in from bots
#[derive(Component, Default, Deref, DerefMut)]
pub struct ActionQueue(VecDeque<ActionEnvelope>);

/// Expanded sub-actions tied to a specific high-level action
#[derive(Component, Default, Deref, DerefMut)]
pub struct ComputedActionQueue(pub VecDeque<ComputedAction>);

/// Track which high-level action is currently in progress, if any
#[derive(Component, Default, Debug)]
pub struct InProgressAction {
    pub opt: Option<ActionResult>,
}

/// A decomposed sub-action, preserving the parent action’s ID
#[derive(Debug, Clone)]
pub struct ComputedAction {
    pub parent_id: u32,
    pub kind: ComputedActionKind,
}

#[derive(Debug, Clone)]
pub enum ComputedActionKind {
    MoveDir(Dir),
    Harvest(Dir),
}

impl ComputedAction {
    pub fn energy(&self) -> Energy {
        match self.kind {
            ComputedActionKind::MoveDir(d) => Action::MoveDir(d).energy(),
            ComputedActionKind::Harvest(d) => Action::Harvest(d).energy(),
        }
        .unwrap()
    }
}

pub struct ActionsPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct ActionsSystemSet;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                // handle_incoming_bot_actions,
                handle_bot_actions,
                process_computed_action,
            )
                .chain()
                .in_set(ActionsSystemSet),
        );
    }
}

// fn handle_incoming_bot_actions(
//     tick: Res<Tick>,
//     action_recv: Res<ActionRecv>,
//     mut queues: Query<&mut ActionQueue>,
//     bot_id_to_entity: Res<BotIdToEntity>,
// ) {
//     while let Ok((bot_id, sent_tick, action)) = action_recv.0.try_recv() {
//         let entity = bot_id_to_entity.0.get(&bot_id).unwrap();
//         debug!(
//             bot_id = bot_id.0,
//             ?action,
//             entity = entity.index(),
//             sim_tick = tick.0,
//             sent_tick,
//             "Received bot action"
//         );
//         queues.get_mut(*entity).unwrap().0.push_back(action);
//     }
// }

fn handle_bot_actions(
    mut query: Query<(
        &BotId,
        &Pos,
        &mut ActionQueue,
        &mut InProgressAction,
        &mut ComputedActionQueue,
    )>,
    grid_world: Res<SGridWorld>,
) {
    for (bot_id, pos, mut incoming, mut in_progress, mut computed) in
        query.iter_mut()
    {
        dbg!(&in_progress);
        if let Some(result) = std::mem::take(&mut in_progress.opt) {
            if ActionStatusDiscriminants::InProgress == (&result.status).into()
            {
                trace!(
                    ?bot_id,
                    "InProgressAction found, skipping until complete"
                );
                in_progress.opt = Some(result);
                continue;
            }
        }
        assert!(in_progress.opt.is_none());

        let Some(ActionEnvelope { action, id }) = incoming.pop_front() else {
            trace!(?bot_id, "No actions in queue");
            continue;
        };
        debug!(?bot_id, ?action, "Processing action");

        match action {
            Action::MoveDir(dir) => {
                debug!(?bot_id, ?dir, "Moving in direction");
                // move_events.send(MoveEvent { entity, dir });

                computed.push_back(ComputedAction {
                    parent_id: id,
                    kind: ComputedActionKind::MoveDir(dir),
                })
            }
            Action::Harvest(dir) => {
                debug!(?bot_id, ?dir, "Harvesting in direction");

                computed.push_back(ComputedAction {
                    parent_id: id,
                    kind: ComputedActionKind::Harvest(dir),
                })
            }
            Action::MoveTo(goal) => {
                if goal == *pos {
                    debug!(?bot_id, ?goal, "Already at goal position");
                    in_progress.opt = Some(ActionResult {
                        action,
                        id,
                        status: ActionStatus::Success,
                    });
                    continue;
                }

                debug!(?bot_id, ?pos, ?goal, "Finding path to goal");
                let Some(path) = grid_world.find_path(*pos, goal) else {
                    warn!(?goal, ?bot_id, "Invalid goal");
                    in_progress.opt = Some(ActionResult {
                        action,
                        id,
                        status: ActionStatus::Failure("Invalid goal".into()),
                    });
                    continue;
                };

                trace!(?bot_id, ?path, "Path found");

                debug!(
                    ?bot_id,
                    actions_count = path.len().saturating_sub(1),
                    "Adding path actions to queue"
                );

                for window in path.windows(2) {
                    let current = window[0];
                    let next = window[1];
                    let diff = next - current;

                    let Some(dir) = Dir::from_deltas(diff) else {
                        panic!(
                            "Invalid path step from {:?} to {:?}",
                            current, next
                        );
                    };
                    trace!(?current, ?next, ?dir, "Path step");

                    computed.push_back(ComputedAction {
                        parent_id: id,
                        kind: ComputedActionKind::MoveDir(dir),
                    });
                }
            }
        }

        in_progress.opt = Some(ActionResult {
            action,
            id,
            status: ActionStatus::InProgress {
                progress: 0,
                total: computed.len() as u16,
            },
        });
    }
}

fn process_computed_action(
    mut query: Query<(
        Entity,
        &BotId,
        &mut Pos,
        &mut InProgressAction,
        &mut ComputedActionQueue,
        &mut Inventory,
        &mut Energy,
    )>,
    mut grid_world: ResMut<SGridWorld>,
) {
    for (
        entity,
        _id,
        mut pos,
        mut in_progress,
        mut computed_queue,
        mut inventory,
        mut energy,
    ) in query.iter_mut()
    {
        let Some(in_progress) = in_progress.opt.as_mut() else {
            assert!(
                computed_queue.is_empty(),
                "No InProgressAction but computed action queue not empty"
            );
            continue;
        };

        if computed_queue.is_empty() {
            continue;
        }

        let computed = computed_queue.pop_front().unwrap();
        assert_eq!(
            computed.parent_id, in_progress.id,
            "computed.parent_id != in_progress.id"
        );

        if let Some(new_energy) = *energy - computed.energy() {
            *energy = new_energy;
        } else {
            in_progress.status = ActionStatus::Failure(format!(
                "Insufficient energy for action {computed:?}. Has {}, Cost {}",
                *energy,
                computed.energy(),
            ));
            computed_queue.clear();
            continue;
        }

        if let Err(reason) = match computed.kind {
            ComputedActionKind::MoveDir(dir) => {
                handle_movement(entity, dir, &mut pos, &mut grid_world)
            }
            ComputedActionKind::Harvest(dir) => {
                handle_harvest(dir, &pos, &mut grid_world, &mut inventory)
            }
        } {
            in_progress.status = ActionStatus::Failure(reason);
            computed_queue.clear();
            continue;
        }

        if computed_queue.is_empty() {
            in_progress.status = ActionStatus::Success;
            continue;
        }

        if let ActionStatus::InProgress { progress, .. } =
            &mut in_progress.status
        {
            *progress += 1;
        } else {
            in_progress.status = ActionStatus::InProgress {
                progress: 1,
                total: computed_queue.len() as u16,
            };
        }
    }
}

fn handle_movement(
    entity: Entity,
    dir: Dir,
    pos: &mut Pos,
    grid_world: &mut ResMut<SGridWorld>,
) -> Result<(), String> {
    // Compute new position and bounds check
    let new_pos = *pos + dir.to_deltas();
    if !grid_world.in_bounds_i(new_pos) {
        warn!(?new_pos, pos = %*pos, "Movement out of bounds");
        return Err("Movement out of Bounds".into());
    }

    // Ensure we pawn can enter new position
    let new_pos = Pos::from(new_pos);
    if !grid_world.get_pos(new_pos).can_enter() {
        warn!(?new_pos, pos = %*pos, "Cannot enter cell");
        return Err("Cannot enter cell".into());
    }

    // Update cells
    grid_world.get_pos_mut(*pos).pawn = None;
    grid_world.get_pos_mut(new_pos).pawn = Some(entity);

    // Set position to new position
    *pos = new_pos;

    Ok(())
}

fn handle_harvest(
    dir: Dir,
    pos: &Pos,
    grid_world: &mut ResMut<SGridWorld>,
    inventory: &mut Inventory,
) -> Result<(), String> {
    use swarm_lib::Item;

    // Calculate the target position to harvest from
    let target_pos = *pos + dir.to_deltas();
    if !grid_world.in_bounds_i(target_pos) {
        warn!(?target_pos, pos = %*pos, "Harvest target out of bounds");
        return Err("Harvest target out of bounds".into());
    }

    let target_pos = Pos::from(target_pos);
    let cell = grid_world.get_pos(target_pos);

    // Check if the target cell has a truffle
    if cell.item != Some(Item::Truffle) {
        warn!(?target_pos, pos = %*pos, "No truffle to harvest at target position");
        return Err("No truffle to harvest at target position".into());
    }

    // Perform the harvest by marking the cell as no longer having a truffle
    // The actual item pickup happens in the harvest_truffles system in core.rs
    grid_world.get_pos_mut(target_pos).item = None;
    info!(pos = %*pos, target = %target_pos, "Harvested truffle");
    *inventory.0.entry(Item::Truffle).or_default() += 1;

    Ok(())
}
