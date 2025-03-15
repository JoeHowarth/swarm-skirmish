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
    BotData,
    Dir,
    Energy,
    FrameKind,
    Item,
    Team,
};

use crate::{
    bot_update::{BotId, BotIdToEntity},
    types::{GridWorld, PartiallyBuiltBot, Tick},
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
        Entity,
        &BotId,
        &BotData,
        &mut CurrentAction,
        &mut PastActions,
    )>,
    grid_world: Res<GridWorld>,
    partially_built_bots: Query<&PartiallyBuiltBot>,
) {
    let get_bot_data = |entity: Entity| query.get(entity).unwrap().2;

    let mut entities_with_invalid_action = Vec::new();
    for (entity, _bot_id, bot_data, current_action, _) in query.iter() {
        let Some(ActionContainer { kind, state, .. }) = &current_action.0
        else {
            // No actions to process, skip
            continue;
        };

        if let Err(status) = is_action_invalid(
            kind,
            state,
            &grid_world,
            bot_data,
            get_bot_data,
            &partially_built_bots,
        ) {
            entities_with_invalid_action.push((entity, status));
        };
    }

    for (entity, failure_string) in entities_with_invalid_action {
        let (_, bot_id, _, mut current_action, mut past_actions) =
            query.get_mut(entity).unwrap();

        // Action is invalid, remove from queue and set status
        let ActionContainer { kind, id, .. } = current_action.0.take().unwrap();
        warn!(?bot_id, action = ?kind, ?id, ?failure_string, "Invalid action");

        past_actions.push(ActionResult {
            action: kind,
            id,
            status: ActionStatus::Failure(failure_string),
            completed_tick: tick.0,
        });
    }
}

fn is_action_invalid<'a>(
    kind: &Action,
    state: &ActionState,
    grid_world: &GridWorld,
    bot_data: &BotData,
    get_bot_data: impl Fn(Entity) -> &'a BotData,
    partially_built_bots: &Query<&PartiallyBuiltBot>,
) -> std::result::Result<(), String> {
    if bot_data.energy < kind.energy_per_tick() {
        return Err("Insufficient Energy".into());
    }

    if !bot_data.is_capable_of(kind) {
        return Err("Insufficient Capabilities".into());
    }

    match kind {
        Action::Noop => {}
        Action::MoveDir(dir) => {
            let new_pos = validate_target_pos(bot_data.pos, *dir, grid_world)?;

            let cell = grid_world.get(new_pos);
            if !cell.can_enter() {
                return Err("Invalid Move: Cannot enter cell".into());
            }
        }
        Action::MoveTo(path) => {
            if path.is_empty() {
                return Err("Invalid Move: Empty path".into());
            }

            let ActionState::MoveTo { idx } = state else {
                return Err("Invalid Move: Not a move to action".into());
            };

            if path.is_empty() {
                return Err("Invalid Move: Empty path".into());
            }

            let new_pos = path[*idx];
            if !grid_world.in_bounds(&new_pos) {
                return Err("Invalid Move: New pos out of bounds".into());
            }

            let cell = grid_world.get(new_pos);
            if !cell.can_enter() {
                return Err("Invalid Move: Cannot enter new pos".into());
            }

            // Check if new_pos is adjacent to current pos
            let is_adjacent = Dir::iter().any(|dir| {
                if let Some(adjacent_pos) = bot_data.pos + dir {
                    adjacent_pos == new_pos
                } else {
                    false
                }
            });

            if !is_adjacent {
                return Err(
                    "Invalid Move: Next position must be adjacent".into()
                );
            }
        }
        Action::Harvest(_dir) => {
            todo!()

            // let Some(target_pos) = bot_data.pos + *dir else {
            //     return Some("Invalid Harvest: Invalid direction".into());
            // };

            // if !grid_world.in_bounds(&target_pos) {
            //     return Some("Invalid Harvest: Out of bounds".into());
            // }

            // let cell = grid_world.get_pos(target_pos);
            // if cell.item != Some(Item::Truffle) {
            //     return Some("Invalid Harvest: No truffle".into());
            // }
        }
        Action::Pickup((item, dir)) => {
            let item_loc =
                validate_target_pos_opt_dir(bot_data.pos, *dir, grid_world)?;

            let cell = grid_world.get(item_loc);
            if cell.item != Some(*item) {
                return Err("Invalid Pickup: No item".into());
            }
        }
        Action::Drop((item, dir)) => {
            let item_loc =
                validate_target_pos_opt_dir(bot_data.pos, *dir, grid_world)?;

            let cell = grid_world.get(item_loc);
            if cell.item.is_some() {
                return Err("Invalid Drop: Already has item".into());
            }

            if bot_data.inventory.get(*item) == 0 {
                return Err("Invalid Drop: No item".into());
            }
        }
        Action::Transfer((item, dir)) => {
            let item_loc = validate_target_pos(bot_data.pos, *dir, grid_world)?;

            let cell = grid_world.get(item_loc);
            if cell.pawn.is_none() {
                return Err("Invalid Transfer: No pawn".into());
            }

            if bot_data.inventory.get(*item) == 0 {
                return Err("Invalid Transfer: No item".into());
            }
        }
        Action::Build(dir, frame_kind, subsystems) => {
            let target_pos =
                validate_target_pos(bot_data.pos, *dir, grid_world)?;

            let cell = grid_world.get(target_pos);

            if cell.pawn.is_some() {
                return Err("Invalid Build: Pawn already exists".into());
            }

            let Some(partial_e) = cell.partially_built_bot else {
                if bot_data.inventory.get(Item::Metal) < frame_kind.build_cost()
                {
                    return Err("Invalid Build: Not enough metal".into());
                }
                // No pawn at target position, build a new bot
                return Ok(());
            };

            if let Ok(partially_built_bot) = partially_built_bots.get(partial_e)
            {
                if &partially_built_bot.frame_kind != frame_kind {
                    return Err("Invalid Build: Pawn is already a partially \
                                built bot with a different frame kind"
                        .into());
                }
                if partially_built_bot.team != bot_data.team {
                    return Err("Invalid Build: Pawn is a partially built \
                                bot but is on the wrong team"
                        .into());
                }
                if &partially_built_bot.subsystems != subsystems {
                    debug!(old_subsystems=?partially_built_bot.subsystems, new_subsystems=?subsystems, "Build failed: subsystems mismatch");
                    return Err("Invalid Build: Pawn is a partially built \
                                bot with a different subsystems"
                        .into());
                }
                if partially_built_bot.ticks_remaining == 0
                    && bot_data.energy.0 < 100 + kind.energy_per_tick().0
                {
                    return Err("Invalid Build: Not enough energy to finish \
                                construction. Requires 100 energy and build \
                                action energy cost"
                        .into());
                }
            } else {
                return Err("Invalid Build: Pawn present but is not \
                            apartially built bot"
                    .into());
            }
        }
        Action::Recharge(dir) => {
            let target_pos =
                validate_target_pos(bot_data.pos, *dir, grid_world)?;

            let Some(pawn) = grid_world.get(target_pos).pawn else {
                return Err("Invalid Recharge: No pawn".into());
            };

            let target_data = get_bot_data(pawn);
            if target_data.energy < Energy(10) {
                return Err("Invalid Recharge: Target entity has \
                            insufficient energy"
                    .into());
            }

            if bot_data.energy.0 > bot_data.max_energy().0 - 1 {
                return Err("Invalid Recharge: Bot has excess energy".into());
            }
        }
        Action::Attack(dir) => {
            let target_pos =
                validate_target_pos(bot_data.pos, *dir, grid_world)?;

            let Some(pawn) = grid_world.get(target_pos).pawn else {
                return Err("Invalid Attack: No pawn".into());
            };

            let target_data = get_bot_data(pawn);
            if target_data.team == bot_data.team {
                return Err("Invalid Attack: Target is on same team".into());
            }
        }
    }
    Ok(())
}

fn validate_target_pos_opt_dir(
    pos: Pos,
    dir: Option<Dir>,
    grid_world: &GridWorld,
) -> std::result::Result<Pos, String> {
    let Some(dir) = dir else {
        return Ok(pos);
    };

    validate_target_pos(pos, dir, grid_world)
}

fn validate_target_pos(
    pos: Pos,
    dir: Dir,
    grid_world: &GridWorld,
) -> std::result::Result<Pos, String> {
    let Some(target_pos) = pos + dir else {
        return Err("Invalid Recharge: Out of bounds".into());
    };

    if !grid_world.in_bounds(&target_pos) {
        return Err("Invalid Recharge: Out of bounds".into());
    }

    Ok(target_pos)
}

fn apply_actions(
    mut commands: Commands,
    tick: Res<Tick>,
    mut query: Query<(
        Entity,
        &BotId,
        &mut BotData,
        &mut CurrentAction,
        &mut PastActions,
    )>,
    mut partially_built_bots: Query<&mut PartiallyBuiltBot>,
    mut grid_world: ResMut<GridWorld>,
) {
    let mut transfers = Vec::new();
    let mut recharge_subtractions = Vec::new();
    for (entity, bot_id, mut bot_data, mut current_action, mut past_actions) in
        query.iter_mut()
    {
        // Present action is valid and can be applied without checks
        let Some(ActionContainer { kind, state, id }) = &mut current_action.0
        else {
            continue;
        };

        // Decrease energy
        bot_data.energy = (bot_data.energy - kind.energy_per_tick()).unwrap();

        let status = apply_action_inner(
            &bot_id,
            &kind,
            state,
            entity,
            &mut bot_data,
            &mut grid_world,
            &mut transfers,
            &mut recharge_subtractions,
            &mut commands,
            &mut partially_built_bots,
        );

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
        let inventory = &mut query.get_mut(pawn).unwrap().2.inventory;
        inventory.add(item, 1);
    }

    for (pawn, energy) in recharge_subtractions {
        let bot = &mut query.get_mut(pawn).unwrap().2;
        bot.energy.0 -= energy.0;
    }
}

fn apply_action_inner(
    bot_id: &BotId,
    kind: &Action,
    state: &mut ActionState,
    entity: Entity,
    bot: &mut BotData,
    grid_world: &mut GridWorld,
    transfers: &mut Vec<(Entity, Item)>,
    recharge_subtractions: &mut Vec<(Entity, Energy)>,
    commands: &mut Commands,
    partially_built_bots: &mut Query<&mut PartiallyBuiltBot>,
) -> Option<ActionStatus> {
    match kind {
        Action::Noop => Some(ActionStatus::Success),
        Action::MoveDir(dir) => {
            grid_world.get_mut(bot.pos).pawn = None;
            bot.pos = (bot.pos + *dir).unwrap();
            grid_world.get_mut(bot.pos).pawn = Some(entity);
            Some(ActionStatus::Success)
        }
        Action::Harvest(dir) => {
            let target_pos = bot.pos + *dir;
            grid_world.get_mut(target_pos.unwrap()).item = None;
            bot.inventory.add(Item::Truffle, 1);
            Some(ActionStatus::Success)
        }
        Action::MoveTo(path) => {
            if let ActionState::MoveTo { idx } = state {
                apply_move_to(entity, &mut bot.pos, path, idx, grid_world)
            } else {
                Some(ActionStatus::Failure(
                    "Invalid Move: Not a move to action".into(),
                ))
            }
        }
        Action::Pickup((item, dir)) => {
            let item_loc = dir.and_then(|dir| bot.pos + dir).unwrap_or(bot.pos);
            grid_world.get_mut(item_loc).item = None;
            bot.inventory.add(*item, 1);
            Some(ActionStatus::Success)
        }
        Action::Drop((item, dir)) => {
            let item_loc = dir.and_then(|dir| bot.pos + dir).unwrap_or(bot.pos);
            grid_world.get_mut(item_loc).item = Some(*item);
            bot.inventory.remove(*item, 1);
            Some(ActionStatus::Success)
        }
        Action::Transfer((item, dir)) => {
            let item_loc = bot.pos + *dir;
            let pawn = grid_world.get(item_loc.unwrap()).pawn.unwrap();
            bot.inventory.remove(*item, 1);
            transfers.push((pawn, *item));
            Some(ActionStatus::Success)
        }
        Action::Build(dir, frame_kind, subsystems) => {
            let target_pos = (bot.pos + *dir).unwrap();
            let target_cell = grid_world.get_mut(target_pos);
            debug!(?bot_id, ?target_pos, "Build action target position");

            match target_cell.partially_built_bot {
                None => {
                    debug!(
                        ?bot_id,
                        ?frame_kind,
                        ?subsystems,
                        "Starting new build at empty location"
                    );
                    // Spawn partially built bot
                    let partially_built_e = commands
                        .spawn(PartiallyBuiltBot {
                            frame_kind: frame_kind.clone(),
                            subsystems: subsystems.clone(),
                            pos: target_pos,
                            team: bot.team,
                            ticks_remaining: kind.ticks_to_complete().unwrap()
                                as u32,
                            _ticks_required: kind.ticks_to_complete().unwrap()
                                as u32,
                        })
                        .id();
                    // Update grid world
                    target_cell.partially_built_bot = Some(partially_built_e);
                    debug!(?bot_id, metal_cost=?frame_kind.build_cost(), "Removing metal for build");
                    assert!(bot
                        .inventory
                        .remove(Item::Metal, frame_kind.build_cost())
                        .is_some());
                    None
                }
                Some(e) => match partially_built_bots.get_mut(e) {
                    Ok(mut partially_built_bot) => {
                        debug!(?bot_id, ?e, ticks_remaining=?partially_built_bot.ticks_remaining, "Continuing build of partially built bot");
                        if partially_built_bot.ticks_remaining == 0 {
                            debug!(
                                ?bot_id,
                                ?e,
                                "Completing build of partially built bot"
                            );
                            // Update bot data
                            commands
                                .entity(e)
                                .remove::<PartiallyBuiltBot>()
                                .insert(BotData::new(
                                    partially_built_bot.frame_kind,
                                    partially_built_bot.subsystems.clone(),
                                    partially_built_bot.pos,
                                    partially_built_bot.team,
                                    Energy(100),
                                    bot.known_map.clone(),
                                    bot.known_bots.clone(),
                                ));

                            // Update grid world
                            target_cell.partially_built_bot = None;
                            target_cell.pawn = Some(e);

                            debug!(
                                ?bot_id,
                                "Consuming 100 energy to complete build"
                            );
                            bot.energy.0 -= 100;
                            Some(ActionStatus::Success)
                        } else {
                            debug!(?bot_id, ?entity, old_ticks=?partially_built_bot.ticks_remaining, "Decreasing ticks remaining for partially built bot");
                            partially_built_bot.ticks_remaining -= 1;
                            debug!(?bot_id, ?entity, new_ticks=?partially_built_bot.ticks_remaining, "New ticks remaining for partially built bot");
                            None
                        }
                    }
                    Err(_) => Some(ActionStatus::Failure(
                        "Invalid Build: Pawn is not a partially built bot"
                            .into(),
                    )),
                },
            }
        }
        Action::Recharge(dir) => {
            // Recharge action implementation

            // Get the target position
            let target_pos = (bot.pos + *dir).unwrap();

            // Get the target cell
            let target_cell = grid_world.get(target_pos);

            // Check if there's a pawn at the target position
            let Some(target_pawn_entity) = target_cell.pawn else {
                return Some(ActionStatus::Failure(
                    "Invalid Recharge: No pawn at target position".into(),
                ));
            };

            // Check if the bot is already at max energy
            if bot.energy >= bot.max_energy() {
                return Some(ActionStatus::Failure(
                    "Invalid Recharge: Bot already at max energy".into(),
                ));
            }

            // Recharge the bot
            let energy_to_add = (bot.max_energy().0 - bot.energy.0).min(10);
            bot.energy.0 += energy_to_add;
            recharge_subtractions
                .push((target_pawn_entity, Energy(energy_to_add)));

            debug!(?bot_id, ?target_pawn_entity, energy_added = ?energy_to_add, "Recharged from nearby pawn");

            Some(ActionStatus::Success)
        }
        Action::Attack(_dir) => todo!(),
    }
}

fn apply_move_to(
    entity: Entity,
    pos: &mut Pos,
    path: &Vec<Pos>,
    idx: &mut usize,
    grid_world: &mut GridWorld,
) -> Option<ActionStatus> {
    grid_world.get_mut(*pos).pawn = None;
    *pos = path[*idx];
    grid_world.get_mut(*pos).pawn = Some(entity);

    if *idx == path.len() - 1 {
        Some(ActionStatus::Success)
    } else {
        *idx += 1;
        None
    }
}
