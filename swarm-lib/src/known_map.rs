use crate::{
    gridworld::{GridWorld, PassableCell},
    CellKind,
    Item,
    Pos,
    RadarData,
    Team,
};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientBotData {
    pub bot_id: u32,
    pub team: Team,
    pub last_observed: u32,
    /// World coordinates
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
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
