use array2d::Array2D;
use bevy::prelude::*;
use swarm_lib::{
    CellKindRadar,
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
    core::{CellKind, Inventory, Tick},
    gridworld::GridWorld,
    server::{BotId, BotIdToEntity, ServerUpdates, SubscriptionRecv},
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
    query: Query<(&BotId, &Pos, &Team, &Subscriptions, &InProgressAction, &Inventory)>,
    grid_world: Res<GridWorld>,
) {
    for (bot_id, pos, team, _subscriptions, in_progress_action, inventory) in query.iter() {
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
    query: &Query<(&BotId, &Pos, &Team, &Subscriptions, &InProgressAction, &Inventory)>,
) -> RadarData {
    // Create a radar with a 11x11 grid centered on the bot
    let radar_size = 11;
    let radar_center = radar_size / 2; // Center point (5 for a 11x11 grid)

    // Get bot's world coordinates
    let (bot_world_x, bot_world_y) = pos.as_isize();

    let mut radar = RadarData {
        bots: Vec::new(),
        cells: Array2D::filled_with(
            CellStateRadar::default(),
            radar_size,
            radar_size,
        ),
        center_world_pos: *pos,
    };

    // Use nearby to get cells in radar range with Manhattan distance
    grid_world
        .nearby(bot_world_x as usize, bot_world_y as usize, radar_center)
        .for_each(|((world_x, world_y), cell)| {
            // Calculate radar coordinates (relative to bot at center)
            let world_x = world_x as isize;
            let world_y = world_y as isize;

            let dx = world_x - bot_world_x;
            let dy = world_y - bot_world_y;

            let radar_x = (radar_center as isize + dx) as usize;
            let radar_y = (radar_center as isize + dy) as usize;

            // Skip if outside radar bounds
            if radar_x >= radar_size || radar_y >= radar_size {
                return;
            }

            let radar_cell = CellStateRadar {
                kind: match cell.kind {
                    CellKind::Empty => CellKindRadar::Empty,
                    CellKind::Blocked => CellKindRadar::Blocked,
                },
                pawn: cell.pawn.map(|e| {
                    let (bot_id, _, &team, _, _, _) = query.get(e).unwrap();

                    // Store the bot's position in radar coordinates
                    radar.bots.push(RadarBotData {
                        team,
                        pos: Pos::from((radar_x, radar_y)),
                        bot_id: bot_id.0,
                    });

                    radar.bots.len() - 1
                }),
                item: cell.item,
            };

            radar.cells.set(radar_x, radar_y, radar_cell).unwrap();
        });

    // Set all cells that would be outside the map bounds to Blocked instead of
    // Unknown
    let map_width = grid_world.width() as isize;
    let map_height = grid_world.height() as isize;

    // Iterate through all radar cells
    for radar_x in 0..radar_size {
        for radar_y in 0..radar_size {
            // Calculate the corresponding world coordinates
            let world_x =
                bot_world_x + (radar_x as isize - radar_center as isize);
            let world_y =
                bot_world_y + (radar_y as isize - radar_center as isize);

            // Check if the world coordinates are outside the map bounds
            if world_x < 0
                || world_x >= map_width
                || world_y < 0
                || world_y >= map_height
            {
                // If the cell is still Unknown (i.e., it wasn't filled by a
                // valid map cell), mark it as Blocked
                if let CellKindRadar::Unknown =
                    radar.cells.get(radar_x, radar_y).unwrap().kind
                {
                    radar
                        .cells
                        .set(
                            radar_x,
                            radar_y,
                            CellStateRadar {
                                kind: CellKindRadar::Blocked,
                                ..default()
                            },
                        )
                        .unwrap();
                }
            }
        }
    }

    radar
}
