use std::fmt::Write;

use bevy::{prelude::*, utils::HashMap};
use strum_macros::Display;
use swarm_lib::{
    gridworld::PassableCell,
    BotData,
    CellKind,
    Energy,
    FrameKind,
    Item,
    Pos,
};

use crate::{
    apply_actions::CurrentAction,
    bot_update::BotId,
    types::{GridWorld, Tick},
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
    mut pawns: Query<(&BotId, &mut BotData)>,
) {
    for (bot_id, mut bot_data) in pawns.iter_mut() {
        let cell = grid_world.get_mut(bot_data.pos);
        if let Some(Item::Crumb) = cell.item {
            info!(%bot_data.pos, ?bot_id, "Picking up Crumb");
            bot_data.inventory.add(Item::Crumb, 1);
            cell.item = None;
        };
    }
}

fn pickup_fent(
    mut grid_world: ResMut<GridWorld>,
    mut pawns: Query<(&BotId, &mut BotData)>,
) {
    for (bot_id, mut bot_data) in pawns.iter_mut() {
        let cell = grid_world.get_mut(bot_data.pos);
        if let Some(Item::Fent) = cell.item {
            info!(%bot_data.pos, ?bot_id, "Picking up Fent");
            bot_data.inventory.add(Item::Fent, 1);
            cell.item = None;
        };
    }
}
