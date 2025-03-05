use bevy::prelude::*;
use swarm_lib::{
    CellKind,
    CellStateRadar,
    Pos,
    RadarBotData,
    RadarData,
    ServerUpdate,
    ServerUpdateEnvelope,
    Team,
};

use crate::{
    actions::InProgressAction,
    core::{Inventory, SGridWorld as GridWorld, Tick},
    server::{BotId, BotIdToEntity, ServerUpdates},
};

#[derive(Component, Default)]
pub struct Subscriptions;

pub struct SubscriptionsPlugin;

#[derive(Debug, Hash, PartialEq, Eq, Clone, SystemSet)]
pub struct SubscriptionsSystemSet;

impl Plugin for SubscriptionsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            send_server_updates.in_set(SubscriptionsSystemSet),
        );
    }
}

fn send_server_updates(
    update_tx: Res<ServerUpdates>,
    tick: Res<Tick>,
    query: Query<(
        &BotId,
        &Pos,
        &Team,
        &Subscriptions,
        &InProgressAction,
        &Inventory,
    )>,
    grid_world: Res<GridWorld>,
) {
    for (bot_id, pos, team, _subscriptions, in_progress_action, inventory) in
        query.iter()
    {
        let update = ServerUpdateEnvelope {
            bot_id: bot_id.0,
            seq: 0,
            response: ServerUpdate {
                tick: tick.0,
                team: *team,
                position: *pos,
                radar: create_radar_data(pos, &grid_world, &query),
                action_result: in_progress_action.opt.clone(),
                items: inventory.0.clone(),
            },
        };

        update_tx.0.send(update).unwrap();
    }
}

fn create_radar_data(
    pos: &Pos,
    grid_world: &GridWorld,
    query: &Query<(
        &BotId,
        &Pos,
        &Team,
        &Subscriptions,
        &InProgressAction,
        &Inventory,
    )>,
) -> RadarData {
    // Define the radar range (how far to look in each direction)
    let radar_range = 5; // This gives a view of 11x11 cells centered on the bot (5 in each
                         // direction)

    // Get bot's world coordinates
    let (bot_world_x, bot_world_y) = pos.as_isize();

    let mut radar = RadarData {
        center_world_pos: *pos,
        bots: Vec::new(),
        cells: Vec::new(),
    };

    // Use nearby to get cells in radar range with Manhattan distance
    grid_world
        .nearby(bot_world_x as usize, bot_world_y as usize, radar_range)
        .for_each(|(pos, cell)| {
            let cell_pos = Pos::from(pos);

            let radar_cell = CellStateRadar {
                kind: match cell.kind {
                    CellKind::Empty => CellKind::Empty,
                    CellKind::Blocked => CellKind::Blocked,
                    CellKind::Unknown => unreachable!(),
                },
                pawn: cell.pawn.map(|e| {
                    let (bot_id, _, &team, _, _, _) = query.get(e).unwrap();

                    // Store the bot's position in world coordinates
                    radar.bots.push(RadarBotData {
                        team,
                        pos: cell_pos,
                        bot_id: bot_id.0,
                    });

                    radar.bots.len() - 1
                }),
                item: cell.item,
                pos: cell_pos,
            };

            radar.cells.push(radar_cell);
        });

    radar
}
