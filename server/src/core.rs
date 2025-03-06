use std::fmt::Write;

use bevy::{prelude::*, utils::HashMap};
use strum_macros::Display;
use swarm_lib::{gridworld::PassableCell, CellKind, Energy, Item, Pos};

use crate::{
    apply_actions::{ActionQueue, ComputedActionQueue, InProgressAction},
    bot_update::BotId,
    types::{GridWorld, Inventory, PawnKind, Tick},
};

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct CoreSystemsSet;

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tick>().add_systems(
            Update,
            (update_tick, pickup_fent, pickup_crumbs).in_set(CoreSystemsSet),
        );
    }
}

fn update_tick(mut tick: ResMut<Tick>) {
    tick.0 += 1;
    debug!("Tick: {}", tick.0);
}

fn pickup_crumbs(
    mut grid_world: ResMut<GridWorld>,
    mut pawns: Query<(&PawnKind, &BotId, &mut Inventory, &Pos)>,
) {
    for (pawn_kind, bot_id, mut inventory, pos) in pawns.iter_mut() {
        let cell = grid_world.get_pos_mut(*pos);
        if let Some(Item::Crumb) = cell.item {
            info!(%pos, ?bot_id, %pawn_kind, "Picking up Crumb");
            *inventory.0.entry(Item::Crumb).or_default() += 1;
            cell.item = None;
        };
    }
}

fn pickup_fent(
    mut grid_world: ResMut<GridWorld>,
    mut pawns: Query<(&PawnKind, &BotId, &mut Inventory, &Pos)>,
) {
    for (pawn_kind, bot_id, mut inventory, pos) in pawns.iter_mut() {
        let cell = grid_world.get_pos_mut(*pos);
        if let Some(Item::Fent) = cell.item {
            info!(%pos, ?bot_id, %pawn_kind, "Picking up Fent");
            *inventory.0.entry(Item::Fent).or_default() += 1;
            cell.item = None;
        };
    }
}
