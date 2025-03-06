use rand::{rngs::SmallRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use swarm_lib::{
    bot_logger::BotLogger,
    gridworld::{GridWorld, PassableCell},
    Action,
    ActionStatusDiscriminants,
    Bot,
    BotResp,
    BotUpdate,
    CellKind,
    CellStateRadar,
    Dir,
    Item::{self, *},
    NewBotNoMangeFn,
    Pos,
    RadarData,
    Team,
};

#[no_mangle]
pub fn test_fn() -> String {
    "Hello, world!".to_string()
}

#[no_mangle]
pub fn new_bot(ctx: BotLogger, (map_w, map_h): (usize, usize)) -> Box<dyn Bot> {
    Box::new(CrumbFollower {
        grid: GridWorld::new(map_w, map_h, ClientCellState::default()),
        rng: SmallRng::seed_from_u64(ctx.bot_id as u64),
        ctx,
        default_dir: Dir::Up,
        action_counter: 0,
        seen_bots: Vec::new(),
    })
}

/// Type check
static _X: NewBotNoMangeFn = new_bot;

pub struct CrumbFollower {
    ctx: BotLogger,
    rng: SmallRng,
    default_dir: Dir,
    action_counter: u32,
    grid: GridWorld<ClientCellState>,
    seen_bots: Vec<ClientBotData>,
}

use std::fmt;

impl fmt::Debug for CrumbFollower {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CrumbFollower")
            .field("action_counter", &self.action_counter)
            .field("default_dir", &self.default_dir)
            .field("grid_size", &(self.grid.width(), self.grid.height()))
            .field("seen_bots_count", &self.seen_bots.len())
            .finish()
    }
}

impl CrumbFollower {
    // Helper method to determine the next action with a linear flow instead of
    // nested conditionals
    fn determine_next_action(
        &mut self,
        radar: &RadarData,
        tick: u32,
    ) -> Action {
        // Go to known Fent location
        if let Some((pos, cell)) =
            self.grid.iter().find(|(_, cell)| cell.item == Some(Fent))
        {
            let pos = Pos::from((pos.0, pos.1));
            self.ctx.debug(format!(
                "Going to Fent at position: {}. Last observed {}, {} ticks ago",
                pos,
                cell.last_observed,
                tick - cell.last_observed
            ));
            return Action::MoveTo(Pos::from(pos));
        }

        // Harvest nearby Truffle
        if let Some((dir, _)) =
            radar.find_dirs(CellStateRadar::has_item(Truffle))
        {
            self.ctx.debug(format!("Harvesting Truffle {dir:?}"));
            return Action::Harvest(dir);
        }

        // Go to known Truffle location
        if let Some((pos, cell)) = self
            .grid
            .iter()
            .find(|(_, cell)| cell.item == Some(Truffle))
        {
            let pos = Pos::from((pos.0 + 1, pos.1));
            self.ctx.debug(format!(
                "Going to truffle at position: {}. Last observed {}, {} ticks \
                 ago",
                pos,
                cell.last_observed,
                tick - cell.last_observed
            ));
            return Action::MoveTo(Pos::from(pos));
        }

        // Follow Crumb
        if let Some((_, cell)) = radar.find(CellStateRadar::has_item(Crumb)) {
            self.ctx
                .debug(format!("Found Crumb at world position: {}", cell.pos));
            return Action::MoveTo(cell.pos);
        }

        // Explore random unknown cells
        let unknown_cells: Vec<_> = self
            .grid
            .iter()
            .filter(|(_, cell)| cell.kind == CellKind::Unknown)
            .collect();

        if !unknown_cells.is_empty() {
            let random_index = self.rng.random_range(0..unknown_cells.len());
            let (pos, _) = unknown_cells[random_index];

            self.ctx.debug(format!(
                "Found random unexplored cell at position: {}",
                Pos::from(pos)
            ));
            return Action::MoveTo(Pos::from(pos));
        }

        // Random movement if nothing else to do
        if self.rng.random_bool(0.2)
            || radar
                .get_dir(self.default_dir)
                .map(|cell| cell.kind)
                .unwrap_or(CellKind::Blocked)
                == CellKind::Blocked
        {
            self.ctx.debug("Changing default dir");
            self.default_dir =
                Dir::from_repr(self.rng.random_range(0..=3)).unwrap();
        }

        self.ctx
            .debug("No unexplored cells found, moving to default dir");
        Action::MoveDir(self.default_dir)
    }
}

impl Bot for CrumbFollower {
    fn update(&mut self, update: BotUpdate) -> Option<BotResp> {
        update_known_map(
            &mut self.grid,
            &mut self.seen_bots,
            &update.radar,
            update.tick,
        );

        let radar = &update.radar;
        let action_result = update.action_result;
        if let Some(result) = action_result {
            self.ctx.debug(format!("{result:?}"));

            if ActionStatusDiscriminants::InProgress == (&result.status).into()
            {
                self.ctx
                    .info("Previous action still in progress, waiting...");
                return None;
            }
        }

        // Determine the next action using a linear decision flow
        let action = self.determine_next_action(radar, update.tick);

        // Build and send response with movement action
        self.action_counter += 1;
        Some(
            BotResp::builder()
                .push_action_id(action, self.action_counter)
                .build(),
        )
    }
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
