use std::fmt::Write;

use bevy::prelude::*;
use swarm_lib::{BotData, Item, Subsystem};

use crate::{
    game::{
        apply_actions::ActionsSystemSet,
        bot_update::{BotId, BotUpdateSystemSet},
    },
    replay::LiveOrReplay,
    types::{GridWorld, Tick},
};

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct CoreSystemsSet;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct SimSystemsSet;

pub struct CorePlugin;

impl Plugin for CorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Tick>()
            .configure_sets(
                Update,
                (ActionsSystemSet, SimSystemsSet, BotUpdateSystemSet)
                    .chain()
                    .in_set(CoreSystemsSet),
            )
            .configure_sets(
                Update,
                CoreSystemsSet.run_if(in_state(LiveOrReplay::Live)),
            )
            .add_systems(
                Update,
                (pickup_fent, pickup_crumbs, generate_energy)
                    .in_set(SimSystemsSet),
            );
    }
}

fn generate_energy(mut pawns: Query<&mut BotData>) {
    println!("Generating energy");
    for mut bot_data in pawns.iter_mut() {
        let generators = bot_data.subsystems.get(Subsystem::Generator);
        if generators > 0 {
            bot_data.energy = (bot_data.energy + generators as i32)
                .min(bot_data.max_energy());
        }
    }
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
