use std::{fmt::Write, time::Duration};

use bevy::{prelude::*, utils::HashMap};
use strum_macros::Display;
use swarm_lib::{
    gridworld::PassableCell,
    BotData,
    BuildingKind,
    CellKind,
    Energy,
    FrameKind,
    Item,
    Pos,
    Subsystem,
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
            (update_tick, pickup_fent, pickup_crumbs, generate_energy)
                .in_set(CoreSystemsSet),
        );
    }
}

#[derive(Resource)]
pub struct TickSpeed {
    pub ms: u64,
    pub is_paused: bool,
}

pub fn should_tick(
    tick_ms: Res<TickSpeed>,
    time: Res<Time>,
    mut timer: Local<Timer>,
) -> bool {
    if tick_ms.is_paused {
        return false;
    }
    timer.tick(time.delta());
    if timer.just_finished() {
        timer.set_duration(Duration::from_millis(tick_ms.ms));
        timer.reset();
        return true;
    }
    false
}

fn update_tick(mut tick: ResMut<Tick>) {
    tick.0 += 1;
    debug!("Tick: {}", tick.0);
}

fn generate_energy(mut pawns: Query<&mut BotData>) {
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
