use std::collections::VecDeque;

use bevy::prelude::*;
use strum::IntoEnumIterator;
use swarm_lib::{
    Action,
    ActionId,
    ActionResult,
    ActionStatus,
    ActionStatusDiscriminants,
    ActionWithId,
    Dir,
    Energy,
    Item,
    Team,
};

use crate::{
    bot_update::{BotId, BotIdToEntity},
    types::{GridWorld, Inventory, Tick},
    Pos,
};

/// High-level action queue with actions sent in from bots
#[derive(Component, Default, Deref, DerefMut)]
pub struct CurrentAction(pub Option<ActionContainer>);

/// Past actions that have been applied
#[derive(Component, Default, Deref, DerefMut)]
pub struct PastActions(pub Vec<ActionResult>);

#[derive(Component, Debug)]
pub struct ActionContainer {
    pub kind: Action,
    pub id: ActionId,
    pub state: ActionState,
}

#[derive(Debug)]
pub enum ActionState {
    None,
    MoveTo { idx: usize },
    Harvest { remaining: u32 },
}

pub struct ActionsPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct ActionsSystemSet;

impl Plugin for ActionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (validate_actions, apply_actions)
                .chain()
                .in_set(ActionsSystemSet),
        );
    }
}

fn validate_actions(
    tick: Res<Tick>,
    mut query: Query<(
        &BotId,
        &Pos,
        &Energy,
        &Inventory,
        &mut CurrentAction,
        &mut PastActions,
    )>,
    grid_world: Res<GridWorld>,
) {
    for (
        bot_id,
        pos,
        energy,
        inventory,
        mut current_action,
        mut past_actions,
    ) in query.iter_mut()
    {
        let Some(ActionContainer { kind, state, id }) = &current_action.0
        else {
            // No actions to process, skip
            continue;
        };

        let Some(status) =
            is_action_invalid(pos, energy, kind, state, &grid_world, inventory)
        else {
            // Action is valid, proceed
            continue;
        };

        // Action is invalid, remove from queue and set status
        warn!(?bot_id, action = ?kind, ?id, ?status, "Invalid action");
        let ActionContainer { kind, id, .. } = current_action.0.take().unwrap();

        past_actions.push(ActionResult {
            action: kind,
            id,
            status: ActionStatus::Failure(status),
            completed_tick: tick.0,
        });
    }
}

fn is_action_invalid(
    pos: &Pos,
    energy: &Energy,
    kind: &Action,
    state: &ActionState,
    grid_world: &GridWorld,
    inventory: &Inventory,
) -> Option<String> {
    if *energy < kind.energy_per_tick() {
        return Some("Insufficient Energy".into());
    }

    match kind {
        Action::Noop => {}
        Action::MoveDir(dir) => {
            let Some(new_pos) = *pos + *dir else {
                return Some("Invalid Move: Invalid direction".into());
            };

            if !grid_world.in_bounds(&new_pos) {
                return Some("Invalid Move: Out of bounds".into());
            }

            let cell = grid_world.get_pos(new_pos);
            if !cell.can_enter() {
                return Some("Invalid Move: Cannot enter cell".into());
            }
        }
        Action::MoveTo(path) => {
            if path.is_empty() {
                return Some("Invalid Move: Empty path".into());
            }

            let ActionState::MoveTo { idx } = state else {
                return Some("Invalid Move: Not a move to action".into());
            };

            if path.is_empty() {
                return Some("Invalid Move: Empty path".into());
            }

            let new_pos = path[*idx];
            if !grid_world.in_bounds(&new_pos) {
                return Some("Invalid Move: New pos out of bounds".into());
            }

            let cell = grid_world.get_pos(new_pos);
            if !cell.can_enter() {
                return Some("Invalid Move: Cannot enter new pos".into());
            }

            // Check if new_pos is adjacent to current pos
            let is_adjacent = Dir::iter().any(|dir| {
                if let Some(adjacent_pos) = *pos + dir {
                    adjacent_pos == new_pos
                } else {
                    false
                }
            });

            if !is_adjacent {
                return Some(
                    "Invalid Move: Next position must be adjacent".into(),
                );
            }
        }
        Action::Harvest(dir) => {
            let Some(target_pos) = *pos + *dir else {
                return Some("Invalid Harvest: Invalid direction".into());
            };

            if !grid_world.in_bounds(&target_pos) {
                return Some("Invalid Harvest: Out of bounds".into());
            }

            let cell = grid_world.get_pos(target_pos);
            if cell.item != Some(Item::Truffle) {
                return Some("Invalid Harvest: No truffle".into());
            }
        }
        Action::Pickup((item, dir)) => {
            let item_loc = dir.and_then(|dir| *pos + dir).unwrap_or(*pos);
            if !grid_world.in_bounds(&item_loc) {
                return Some("Invalid Pickup: Out of bounds".into());
            }

            let cell = grid_world.get_pos(item_loc);
            if cell.item != Some(*item) {
                return Some("Invalid Pickup: No item".into());
            }
        }
        Action::Drop((item, dir)) => {
            let item_loc = dir.and_then(|dir| *pos + dir).unwrap_or(*pos);
            if !grid_world.in_bounds(&item_loc) {
                return Some("Invalid Drop: Out of bounds".into());
            }

            let cell = grid_world.get_pos(item_loc);
            if cell.item.is_some() {
                return Some("Invalid Drop: Already has item".into());
            }

            if inventory.0.get(item) == Some(&0) {
                return Some("Invalid Drop: No item".into());
            }
        }
        Action::Transfer((item, dir)) => {
            let Some(item_loc) = *pos + *dir else {
                return Some("Invalid Transfer: Invalid direction".into());
            };

            if !grid_world.in_bounds(&item_loc) {
                return Some("Invalid Transfer: Out of bounds".into());
            }

            let cell = grid_world.get_pos(item_loc);
            if cell.pawn.is_none() {
                return Some("Invalid Transfer: No pawn".into());
            }

            if inventory.0.get(item) == Some(&0) {
                return Some("Invalid Transfer: No item".into());
            }
        }
        Action::Build(dir, building_kind) => todo!(),
        Action::Recharge => todo!(),
        Action::Attack(dir) => todo!(),
    }

    None
}

fn apply_actions(
    tick: Res<Tick>,
    mut query: Query<(
        Entity,
        &BotId,
        &mut Pos,
        &mut CurrentAction,
        &mut PastActions,
        &mut Inventory,
        &mut Energy,
    )>,
    mut grid_world: ResMut<GridWorld>,
) {
    let mut transfers = Vec::new();
    for (
        entity,
        bot_id,
        mut pos,
        mut current_action,
        mut past_actions,
        mut inventory,
        mut energy,
    ) in query.iter_mut()
    {
        // Present action is valid and can be applied without checks
        let Some(ActionContainer { kind, state, id }) = &mut current_action.0
        else {
            continue;
        };

        // Decrease energy
        *energy = (*energy - kind.energy_per_tick()).unwrap();

        let status = match &kind {
            Action::Noop => Some(ActionStatus::Success),
            Action::MoveDir(dir) => {
                grid_world.get_pos_mut(*pos).pawn = None;
                *pos = (*pos + *dir).unwrap();
                grid_world.get_pos_mut(*pos).pawn = Some(entity);
                Some(ActionStatus::Success)
            }
            Action::Harvest(dir) => {
                let target_pos = *pos + *dir;
                grid_world.get_pos_mut(target_pos.unwrap()).item = None;
                *inventory.0.entry(Item::Truffle).or_default() += 1;
                Some(ActionStatus::Success)
            }
            Action::MoveTo(path) => {
                if let ActionState::MoveTo { idx } = state {
                    apply_move_to(entity, &mut pos, path, idx, &mut grid_world)
                } else {
                    Some(ActionStatus::Failure(
                        "Invalid Move: Not a move to action".into(),
                    ))
                }
            }
            Action::Pickup((item, dir)) => {
                let item_loc = dir.and_then(|dir| *pos + dir).unwrap_or(*pos);
                grid_world.get_pos_mut(item_loc).item = None;
                *inventory.0.entry(*item).or_default() += 1;
                Some(ActionStatus::Success)
            }
            Action::Drop((item, dir)) => {
                let item_loc = dir.and_then(|dir| *pos + dir).unwrap_or(*pos);
                grid_world.get_pos_mut(item_loc).item = Some(*item);
                *inventory.0.entry(*item).or_default() -= 1;
                Some(ActionStatus::Success)
            }
            Action::Transfer((item, dir)) => {
                let item_loc = *pos + *dir;
                let pawn = grid_world.get_pos(item_loc.unwrap()).pawn.unwrap();
                *inventory.0.entry(*item).or_default() -= 1;
                transfers.push((pawn, *item));
                Some(ActionStatus::Success)
            }
            Action::Build(dir, building_kind) => todo!(),
            Action::Recharge => todo!(),
            Action::Attack(dir) => todo!(),
        };

        let Some(status) = status else {
            debug!(?bot_id, action = ?kind, ?id, ?state, "Action in progress");
            // Action is still in progress, skip
            continue;
        };

        let ActionContainer { kind, id, .. } = current_action.0.take().unwrap();

        info!(?bot_id, action = ?kind, ?id, ?status, "Applied action");
        past_actions.push(ActionResult {
            action: kind,
            id,
            status,
            completed_tick: tick.0,
        });
    }

    for (pawn, item) in transfers {
        let mut inventory = query.get_mut(pawn).unwrap().5;
        *inventory.0.entry(item).or_default() += 1;
    }
}

fn apply_move_to(
    entity: Entity,
    pos: &mut Pos,
    path: &Vec<Pos>,
    idx: &mut usize,
    grid_world: &mut GridWorld,
) -> Option<ActionStatus> {
    grid_world.get_pos_mut(*pos).pawn = None;
    *pos = path[*idx];
    grid_world.get_pos_mut(*pos).pawn = Some(entity);

    if *idx == path.len() - 1 {
        Some(ActionStatus::Success)
    } else {
        *idx += 1;
        None
    }
}
