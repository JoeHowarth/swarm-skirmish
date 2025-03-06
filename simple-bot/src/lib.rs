#![allow(unused_imports)]

use eyre::Result;
use random_walk::RandomWalkBot;
use serde::{Deserialize, Serialize};
use swarm_lib::{
    bot_harness::{Harness, OldBot},
    ctx::Ctx,
    gridworld::{GridWorld, PassableCell},
    BotResponse,
    CellKind,
    CellStateRadar,
    Item,
    Pos,
    RadarBotData,
    RadarData,
    ServerUpdate,
    Team,
};

pub mod crumb;
pub mod manual;
pub mod random_walk;

#[no_mangle]
pub fn test_fn() -> String {
    "Hello, world!".to_string()
}

pub fn run_loop(updater: &mut impl BotUpdate) -> Result<()> {
    // Initialize the bot
    updater.ctx().info("Bot initialized");

    loop {
        // Wait for server update
        let Some(update) = updater.ctx().wait_for_latest_update() else {
            // Bot was killed
            return Ok(());
        };

        let (known_map, known_bots) = updater.known_map();

        update_known_map(known_map, known_bots, &update.radar, update.tick);

        // Log debug info every tick
        updater.ctx().log_debug_info(&update, 1);

        if let Some(response) = updater.update(update) {
            updater.ctx().send_msg(response);
        }
    }
}

pub trait CtxExt {
    fn log_debug_info(&mut self, update: &ServerUpdate, log_every_x_ticks: u32);
}

impl CtxExt for Ctx {
    fn log_debug_info(
        &mut self,
        update: &ServerUpdate,
        log_every_x_ticks: u32,
    ) {
        self.debug(format!("Processing tick {}", update.tick));

        if update.tick % log_every_x_ticks == 0 {
            // Format items as a readable list
            let items_str = if update.items.is_empty() {
                "None".to_string()
            } else {
                update
                    .items
                    .iter()
                    .map(|(item, count)| format!("{}: {}", item, count))
                    .collect::<Vec<_>>()
                    .join(", ")
            };

            self.debug(format!(
                "Bot Status Report [Tick {}]:\n{}\nEnergy: {}\nDetected Bots: \
                 {}\nItems: {}\nTeam: {:?}",
                update.tick,
                update.position,
                update.energy.0,
                update.radar.pawns.len(),
                items_str,
                update.team
            ));

            // The print_radar method now logs internally
            self.print_radar(&update);
        }
    }
}

pub trait BotUpdate {
    fn update(&mut self, update: ServerUpdate) -> Option<BotResponse>;
    fn known_map(
        &mut self,
    ) -> (&mut GridWorld<ClientCellState>, &mut Vec<ClientBotData>);
    fn ctx(&mut self) -> &mut Ctx;
}

/// Updates the bot's known map with fresh radar data
pub fn update_known_map(
    known_map: &mut GridWorld<ClientCellState>,
    known_bots: &mut Vec<ClientBotData>,
    radar: &RadarData,
    current_tick: u32,
) {
    // Update cells from radar data
    for cell in &radar.cells {
        let pos = cell.pos;

        // Get or create the cell in our known map
        let known_cell = known_map.get_pos_mut(pos);

        // Convert pawn index to bot ID if a pawn exists
        let pawn_bot_id = cell.pawn.and_then(|pawn_idx| {
            radar.pawns.get(pawn_idx).map(|bot| bot.bot_id)
        });

        // Update the cell with fresh data
        known_cell.kind = cell.kind;
        known_cell.pawn = pawn_bot_id;
        known_cell.item = cell.item;
        known_cell.last_observed = current_tick;
    }

    // Update bot positions
    for radar_bot in &radar.pawns {
        // Check if we already know about this bot
        let known_bot =
            known_bots.iter_mut().find(|b| b.bot_id == radar_bot.bot_id);

        if let Some(known_bot) = known_bot {
            // If position changed, remove bot from old position in the grid
            if known_bot.pos != radar_bot.pos {
                // Find the cell at the old position and clear its pawn
                let old_cell = known_map.get_pos_mut(known_bot.pos);
                if old_cell.pawn == Some(radar_bot.bot_id) {
                    old_cell.pawn = None;
                }
            }

            // Update existing bot data
            known_bot.pos = radar_bot.pos;
            known_bot.last_observed = current_tick;
        } else {
            // Add new bot data
            known_bots.push(ClientBotData {
                bot_id: radar_bot.bot_id,
                team: radar_bot.team,
                pos: radar_bot.pos,
                last_observed: current_tick,
            });
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientBotData {
    pub bot_id: u32,
    pub team: Team,
    pub last_observed: u32,
    /// World coordinates
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ClientCellState {
    pub kind: CellKind,
    // Optional bot_id
    pub pawn: Option<u32>,
    pub item: Option<Item>,
    pub last_observed: u32,
}

impl PassableCell for ClientCellState {
    fn is_blocked(&self) -> bool {
        self.pawn.is_some() || self.kind == CellKind::Blocked
    }
}

#[cfg(test)]
mod tests {
    use swarm_lib::{
        CellKind,
        CellStateRadar,
        Pos,
        RadarBotData,
        RadarData,
        Team,
    };

    use super::*;

    #[test]
    fn test_update_basic_cell_info() {
        // Create a small grid and empty bot list
        let mut known_map =
            GridWorld::<ClientCellState>::new(5, 5, ClientCellState::default());
        let mut known_bots = Vec::new();

        // Create radar data with one cell but no bots
        let radar = RadarData {
            center_world_pos: Pos::from((2, 2)),
            pawns: vec![],
            cells: vec![CellStateRadar {
                kind: CellKind::Blocked,
                pawn: None,
                item: Some(Item::Crumb),
                pos: Pos::from((2, 3)),
            }],
        };

        // Update map with current tick = 10
        update_known_map(&mut known_map, &mut known_bots, &radar, 10);

        // Verify cell was updated correctly
        let cell = known_map.get_pos(Pos::from((2, 3)));
        assert_eq!(cell.kind, CellKind::Blocked);
        assert_eq!(cell.item, Some(Item::Crumb));
        assert_eq!(cell.pawn, None);
        assert_eq!(cell.last_observed, 10);
        assert!(known_bots.is_empty());
    }

    #[test]
    fn test_update_with_bot_in_cell() {
        let mut known_map =
            GridWorld::<ClientCellState>::new(5, 5, ClientCellState::default());
        let mut known_bots = Vec::new();

        // Create radar data with one cell and one bot
        let radar = RadarData {
            center_world_pos: Pos::from((2, 2)),
            pawns: vec![RadarBotData {
                bot_id: 42,
                pos: Pos::from((1, 1)),
                team: Team::Player,
            }],
            cells: vec![CellStateRadar {
                kind: CellKind::Empty,
                pawn: Some(0), // Index 0 refers to the bot in pawns
                item: None,
                pos: Pos::from((1, 1)),
            }],
        };

        // Update map with current tick = 5
        update_known_map(&mut known_map, &mut known_bots, &radar, 5);

        // Verify bot was added to known_bots
        assert_eq!(known_bots.len(), 1);
        assert_eq!(known_bots[0].bot_id, 42);
        assert_eq!(known_bots[0].team, Team::Player);
        assert_eq!(known_bots[0].pos, Pos::from((1, 1)));
        assert_eq!(known_bots[0].last_observed, 5);

        // Verify cell was updated with bot reference
        let cell = known_map.get_pos(Pos::from((1, 1)));
        assert_eq!(cell.pawn, Some(42));
    }

    #[test]
    fn test_bot_movement() {
        let mut known_map =
            GridWorld::<ClientCellState>::new(5, 5, ClientCellState::default());
        let mut known_bots = vec![ClientBotData {
            bot_id: 42,
            team: Team::Player,
            pos: Pos::from((1, 1)),
            last_observed: 10,
        }];

        // Set initial cell state with bot
        let old_cell = known_map.get_pos_mut(Pos::from((1, 1)));
        old_cell.pawn = Some(42);

        // Create radar with bot in new position
        let radar = RadarData {
            center_world_pos: Pos::from((2, 2)),
            pawns: vec![RadarBotData {
                bot_id: 42,
                team: Team::Player,
                pos: Pos::from((2, 1)), // Bot moved right
            }],
            cells: vec![
                // Old position is now empty
                CellStateRadar {
                    kind: CellKind::Empty,
                    pawn: None,
                    item: None,
                    pos: Pos::from((1, 1)),
                },
                // New position has the bot
                CellStateRadar {
                    kind: CellKind::Empty,
                    pawn: Some(0),
                    item: None,
                    pos: Pos::from((2, 1)),
                },
            ],
        };

        // Update map with current tick = 15
        update_known_map(&mut known_map, &mut known_bots, &radar, 15);

        // Verify bot position was updated
        assert_eq!(known_bots[0].pos, Pos::from((2, 1)));
        assert_eq!(known_bots[0].last_observed, 15);

        // Verify old cell no longer has bot
        let old_cell = known_map.get_pos(Pos::from((1, 1)));
        assert_eq!(old_cell.pawn, None);

        // Verify new cell has bot
        let new_cell = known_map.get_pos(Pos::from((2, 1)));
        assert_eq!(new_cell.pawn, Some(42));
    }

    #[test]
    fn test_partial_radar_view() {
        // This test verifies that cells not in radar view maintain their
        // previous state
        let mut known_map =
            GridWorld::<ClientCellState>::new(5, 5, ClientCellState::default());
        let mut known_bots = Vec::new();

        // Set initial state for a cell not in radar
        let cell_outside_radar = known_map.get_pos_mut(Pos::from((4, 4)));
        cell_outside_radar.kind = CellKind::Blocked;
        cell_outside_radar.last_observed = 5;

        // Create radar that doesn't see (4,4)
        let radar = RadarData {
            center_world_pos: Pos::from((1, 1)),
            pawns: vec![],
            cells: vec![CellStateRadar {
                kind: CellKind::Empty,
                pawn: None,
                item: None,
                pos: Pos::from((1, 1)),
            }],
        };

        // Update map with current tick = 10
        update_known_map(&mut known_map, &mut known_bots, &radar, 10);

        // Verify cell in radar was updated
        let updated_cell = known_map.get_pos(Pos::from((1, 1)));
        assert_eq!(updated_cell.kind, CellKind::Empty);
        assert_eq!(updated_cell.last_observed, 10);

        // Verify cell outside radar maintained state
        let outside_cell = known_map.get_pos(Pos::from((4, 4)));
        assert_eq!(outside_cell.kind, CellKind::Blocked);
        assert_eq!(outside_cell.last_observed, 5); // Still has old timestamp
    }

    #[test]
    fn test_bot_appears_and_disappears() {
        let mut known_map =
            GridWorld::<ClientCellState>::new(5, 5, ClientCellState::default());
        let mut known_bots = Vec::new();

        // First radar with bot visible
        let radar1 = RadarData {
            center_world_pos: Pos::from((2, 2)),
            pawns: vec![RadarBotData {
                bot_id: 42,
                team: Team::Player,
                pos: Pos::from((3, 3)),
            }],
            cells: vec![CellStateRadar {
                kind: CellKind::Empty,
                pawn: Some(0),
                item: None,
                pos: Pos::from((3, 3)),
            }],
        };

        // Update map with current tick = 10
        update_known_map(&mut known_map, &mut known_bots, &radar1, 10);

        // Verify bot was added
        assert_eq!(known_bots.len(), 1);
        assert_eq!(known_bots[0].bot_id, 42);

        // Second radar without the bot (moved out of view)
        let radar2 = RadarData {
            center_world_pos: Pos::from((2, 2)),
            pawns: vec![],
            cells: vec![CellStateRadar {
                kind: CellKind::Empty,
                pawn: None,
                item: None,
                pos: Pos::from((3, 3)),
            }],
        };

        // Update map with current tick = 15
        update_known_map(&mut known_map, &mut known_bots, &radar2, 15);

        // Verify bot still exists in known_bots but with old timestamp
        assert_eq!(known_bots.len(), 1);
        assert_eq!(known_bots[0].last_observed, 10); // Still has old timestamp

        // Verify cell no longer has the bot
        let cell = known_map.get_pos(Pos::from((3, 3)));
        assert_eq!(cell.pawn, None);
        assert_eq!(cell.last_observed, 15);
    }
}
