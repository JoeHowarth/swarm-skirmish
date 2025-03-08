use rand::{rngs::SmallRng, Rng, SeedableRng};
use swarm_lib::{
    bot_logger::BotLogger,
    gridworld::GridWorld,
    known_map::{update_known_map, ClientBotData, ClientCellState},
    Action,
    ActionWithId,
    Bot,
    BotUpdate,
    CellKind,
    CellStateRadar,
    Dir,
    Item::{self, *},
    NewBotNoMangeFn,
    Pos,
    RadarData,
};

pub struct CrumbFollower {
    pub ctx: BotLogger,
    pub rng: SmallRng,
    pub default_dir: Dir,
    pub action_counter: u32,
    pub grid: GridWorld<ClientCellState>,
    pub seen_bots: Vec<ClientBotData>,
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
        curr_pos: Pos,
        radar: &RadarData,
        tick: u32,
    ) -> Action {
        // Go to known Fent location
        if let Some((pos, cell)) =
            self.grid.iter().find(|(_, cell)| cell.item == Some(Fent))
        {
            let pos = Pos::from((pos.0, pos.1));

            if let Some(path) = self.grid.find_path(curr_pos, pos) {
                self.ctx.debug(format!(
                    "Going to Fent at position: {}. Last observed {}, {} \
                     ticks ago, current pos: {}",
                    pos,
                    cell.last_observed,
                    tick - cell.last_observed,
                    curr_pos
                ));
                return Action::MoveTo(path.into_iter().collect());
            }

            self.ctx.debug("No path to Fent");
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
                 ago, current pos: {}",
                pos,
                cell.last_observed,
                tick - cell.last_observed,
                curr_pos
            ));

            if let Some(path) = self.grid.find_path(curr_pos, pos) {
                return Action::MoveTo(path.into_iter().collect());
            }

            self.ctx.debug("No path to truffle");
        }

        // Follow Crumb
        if let Some((_, cell)) = radar.find(CellStateRadar::has_item(Crumb)) {
            self.ctx
                .debug(format!("Found Crumb at world position: {}", cell.pos));

            if let Some(path) = self.grid.find_path(curr_pos, cell.pos) {
                return Action::MoveTo(path.into_iter().collect());
            }

            self.ctx.debug("No path to crumb");
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

            if let Some(path) = self.grid.find_path(curr_pos, pos) {
                return Action::MoveTo(path.into_iter().collect());
            }

            self.ctx.debug("No path to random unexplored cell");
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
    fn update(&mut self, update: BotUpdate) -> Option<ActionWithId> {
        self.ctx.set_tick(update.tick);
        self.ctx.log_debug_info(&update, 1);

        update_known_map(
            &mut self.grid,
            &mut self.seen_bots,
            &update.radar,
            update.tick,
        );

        if let Some(action) = update.in_progress_action {
            self.ctx.debug(format!(
                "Previous action still in progress, waiting... Action: \
                 {action:?}"
            ));
            return None;
        }

        // Determine the next action using a linear decision flow
        let action = self.determine_next_action(
            update.position,
            &update.radar,
            update.tick,
        );

        self.ctx.flush_buffer_to_stdout();

        // Build and send response with movement action
        self.action_counter += 1;
        Some(ActionWithId {
            id: self.action_counter,
            action,
        })
    }
}
